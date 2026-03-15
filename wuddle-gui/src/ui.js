import { state, SUPPORTED_THEMES, DEFAULT_THEME_ID } from "./state.js";
import { maybePollSelfUpdateInfo } from "./about.js";
import { $ } from "./utils.js";

const themedSelectBindings = new WeakMap();

function normalizeThemeId(raw) {
  const value = String(raw || "").trim().toLowerCase();
  if (value === "dark") return DEFAULT_THEME_ID;
  return SUPPORTED_THEMES.has(value) ? value : DEFAULT_THEME_ID;
}
export { normalizeThemeId };

export function renderBusy() {
  const busy = state.pending > 0;
  document.body.classList.toggle("busy", busy);
  $("busyIndicator").classList.toggle("hidden", !busy);
}

export async function withBusy(work) {
  state.pending += 1;
  renderBusy();
  try {
    return await work();
  } finally {
    state.pending = Math.max(0, state.pending - 1);
    renderBusy();
  }
}

export function setTheme(themeId = state.theme) {
  const next = normalizeThemeId(themeId);
  state.theme = next;
  document.documentElement.setAttribute("data-theme", next);
  renderThemePicker();
  invalidateFadeColors();
}

export function setUiFontStyle(enabled = state.useFrizFont) {
  const useFriz = !!enabled;
  state.useFrizFont = useFriz;
  document.documentElement.setAttribute("data-font-style", useFriz ? "friz" : "default");
}

export function renderThemePicker() {
  try {
    const hidden = $("optTheme");
    if (hidden) hidden.value = state.theme;
    document.querySelectorAll("#themePicker .theme-swatch").forEach((btn) => {
      const themeId = normalizeThemeId(btn.getAttribute("data-theme") || "");
      const active = themeId === state.theme;
      btn.setAttribute("aria-checked", active ? "true" : "false");
    });
  } catch (_) {}
}

export function closeThemedSelectMenus(except = null) {
  document.querySelectorAll(".select-menu.open").forEach((menu) => {
    if (!(menu instanceof HTMLElement)) return;
    if (except && menu === except) return;
    menu.classList.remove("open");
    const select = menu.previousElementSibling;
    if (!(select instanceof HTMLSelectElement)) return;
    const binding = themedSelectBindings.get(select);
    if (!binding) return;
    binding.pop.classList.remove("open");
    if (binding.pop.parentElement !== binding.menu) {
      binding.menu.appendChild(binding.pop);
    }
  });
}

function themedSelectPortalFor(select) {
  if (!(select instanceof HTMLElement)) return document.body;
  const dialog = select.closest("dialog");
  if (dialog instanceof HTMLDialogElement && dialog.open) return dialog;
  if (dialog instanceof HTMLElement) return dialog;
  return document.body;
}

function closeThemedSelectMenu(select) {
  const binding = themedSelectBindings.get(select);
  if (!binding) return;
  binding.menu.classList.remove("open");
  binding.pop.classList.remove("open");
  if (binding.pop.parentElement !== binding.menu) {
    binding.menu.appendChild(binding.pop);
  }
}

function positionThemedSelectMenu(select) {
  const binding = themedSelectBindings.get(select);
  if (!binding) return;
  if (!binding.menu.classList.contains("open")) return;
  const btnRect = binding.btn.getBoundingClientRect();
  const viewportW = window.innerWidth || document.documentElement.clientWidth || 0;
  const viewportH = window.innerHeight || document.documentElement.clientHeight || 0;
  const margin = 8;
  const preferredWidth = Math.max(btnRect.width, 160);
  binding.pop.style.width = `${preferredWidth}px`;
  binding.pop.style.maxHeight = `${Math.max(120, Math.min(320, viewportH - 24))}px`;
  const popRect = binding.pop.getBoundingClientRect();
  const popH = popRect.height || 220;
  let left = btnRect.left;
  if (left + preferredWidth > viewportW - margin) {
    left = Math.max(margin, viewportW - preferredWidth - margin);
  }
  if (left < margin) left = margin;
  const roomBelow = viewportH - btnRect.bottom - margin;
  const roomAbove = btnRect.top - margin;
  let top = btnRect.bottom + 2;
  if (roomBelow < popH && roomAbove > roomBelow) {
    top = Math.max(margin, btnRect.top - popH - 2);
  }
  binding.pop.style.left = `${Math.round(left)}px`;
  binding.pop.style.top = `${Math.round(top)}px`;
}

export function syncThemedSelect(select) {
  const binding = themedSelectBindings.get(select);
  if (!binding) return;
  const selectedOption = select.options[select.selectedIndex] || null;
  const text = selectedOption?.textContent?.trim() || selectedOption?.value || "";
  binding.value.textContent = text;
  binding.btn.title = text;
  binding.btn.disabled = !!select.disabled;
  binding.menu.classList.toggle("disabled", !!select.disabled);
  binding.items.forEach((item) => {
    item.classList.toggle("active", item.dataset.value === select.value);
  });
}

export function rebuildThemedSelect(select) {
  const binding = themedSelectBindings.get(select);
  if (!binding) return;
  binding.pop.innerHTML = "";
  binding.items = [];
  for (const option of Array.from(select.options || [])) {
    const item = document.createElement("button");
    item.type = "button";
    item.className = "select-menu-item";
    item.dataset.value = option.value;
    item.textContent = option.textContent || option.value;
    item.disabled = !!option.disabled;
    item.addEventListener("click", (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      if (select.value !== option.value) {
        select.value = option.value;
        select.dispatchEvent(new Event("change", { bubbles: true }));
      }
      closeThemedSelectMenu(select);
      syncThemedSelect(select);
    });
    binding.pop.appendChild(item);
    binding.items.push(item);
  }
  syncThemedSelect(select);
}

export function ensureThemedSelect(select, extraClass = "") {
  if (!(select instanceof HTMLSelectElement)) return null;
  let binding = themedSelectBindings.get(select);
  if (!binding) {
    const menu = document.createElement("div");
    menu.className = "select-menu";
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "select-menu-btn";
    const value = document.createElement("span");
    value.className = "select-menu-value";
    btn.appendChild(value);
    const pop = document.createElement("div");
    pop.className = "select-menu-pop";
    menu.appendChild(btn);
    menu.appendChild(pop);
    select.classList.add("native-select-hidden");
    select.insertAdjacentElement("afterend", menu);
    binding = { menu, btn, pop, value, items: [] };
    themedSelectBindings.set(select, binding);

    btn.addEventListener("click", (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      if (select.disabled) return;
      const willOpen = !menu.classList.contains("open");
      closeThemedSelectMenus(menu);
      const portal = themedSelectPortalFor(select);
      if (willOpen && binding.pop.parentElement !== portal) {
        portal.appendChild(binding.pop);
      }
      menu.classList.toggle("open", willOpen);
      binding.pop.classList.toggle("open", willOpen);
      if (!willOpen && binding.pop.parentElement !== menu) {
        menu.appendChild(binding.pop);
      }
      if (willOpen) {
        requestAnimationFrame(() => positionThemedSelectMenu(select));
      }
    });
    select.addEventListener("change", () => {
      syncThemedSelect(select);
    });
  }
  if (extraClass) binding.menu.classList.add(extraClass);
  rebuildThemedSelect(select);
  return binding;
}

export function showToast(message, { kind = "info", onAction = null } = {}) {
  const host = $("toastHost");
  if (!(host instanceof HTMLElement)) return;

  const item = document.createElement("div");
  item.className = `toast toast-${kind}`;
  const AUTO_HIDE_MS = 6000;
  const LEAVE_MS = 180;
  let remainingMs = AUTO_HIDE_MS;
  let hideTimerId = null;
  let leaveTimerId = null;
  let startedAtMs = 0;

  const clearTimers = () => {
    if (hideTimerId !== null) { window.clearTimeout(hideTimerId); hideTimerId = null; }
    if (leaveTimerId !== null) { window.clearTimeout(leaveTimerId); leaveTimerId = null; }
  };
  const dismiss = () => { clearTimers(); item.remove(); };
  const startLeaving = () => {
    if (item.classList.contains("is-leaving")) return;
    clearTimers();
    item.classList.add("is-leaving");
    leaveTimerId = window.setTimeout(() => item.remove(), LEAVE_MS);
  };
  const startHideTimer = () => {
    if (item.classList.contains("is-leaving")) return;
    if (hideTimerId !== null) return;
    startedAtMs = Date.now();
    hideTimerId = window.setTimeout(startLeaving, Math.max(0, remainingMs));
  };
  const pauseHideTimer = () => {
    if (hideTimerId === null) return;
    const elapsed = Date.now() - startedAtMs;
    remainingMs = Math.max(120, remainingMs - elapsed);
    window.clearTimeout(hideTimerId);
    hideTimerId = null;
  };
  const resumeHideTimer = () => {
    if (item.classList.contains("is-leaving")) return;
    startHideTimer();
  };

  const text = document.createElement("div");
  text.className = "toast-text";
  text.textContent = String(message || "").trim();
  item.appendChild(text);

  if (typeof onAction === "function") {
    item.classList.add("toast-clickable");
    item.addEventListener("click", (e) => {
      if (e.target.closest(".toast-close")) return;
      try { onAction(); } finally { dismiss(); }
    });
  }

  const close = document.createElement("button");
  close.type = "button";
  close.className = "toast-close";
  close.setAttribute("aria-label", "Dismiss notification");
  close.innerHTML = '<svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="2" y1="2" x2="12" y2="12"/><line x1="12" y1="2" x2="2" y2="12"/></svg>';
  close.addEventListener("click", dismiss);
  item.appendChild(close);

  item.addEventListener("mouseenter", pauseHideTimer);
  item.addEventListener("mouseleave", resumeHideTimer);
  item.addEventListener("focusin", pauseHideTimer);
  item.addEventListener("focusout", resumeHideTimer);

  host.appendChild(item);
  while (host.childElementCount > 4) {
    const oldest = host.firstElementChild;
    if (oldest instanceof HTMLElement) { oldest.remove(); } else { break; }
  }
  startHideTimer();
}

// ---------------------------------------------------------------------------
// Auto-check timer
// ---------------------------------------------------------------------------

let _refreshAll = async () => {};
export function setUiCallbacks(cbs) {
  if (cbs.refreshAll) _refreshAll = cbs.refreshAll;
}

export function renderAutoCheckSettings() {
  const { normalizeAutoCheckMinutes } = state;
  const enabled = !!state.autoCheckEnabled;
  const input = $("optAutoCheckMinutes");
  const inline = $("optAutoCheckInline");
  $("optAutoCheck").checked = enabled;
  const minutes = state.autoCheckMinutes;
  const num = Number(minutes);
  const normalized = Number.isFinite(num)
    ? Math.max(1, Math.min(240, Math.floor(num)))
    : 60;
  if (input instanceof HTMLInputElement) {
    input.value = String(normalized);
    input.disabled = !enabled;
  }
  inline?.classList.toggle("disabled", !enabled);
}

export function clearAutoCheckTimer() {
  if (state.autoCheckTimerId !== null) {
    window.clearTimeout(state.autoCheckTimerId);
    state.autoCheckTimerId = null;
  }
}

export function scheduleAutoCheckTimer() {
  clearAutoCheckTimer();
  if (!state.autoCheckEnabled) return;
  const minutes = state.autoCheckMinutes;
  const num = Number(minutes);
  const normalized = Number.isFinite(num)
    ? Math.max(1, Math.min(240, Math.floor(num)))
    : 60;
  const delayMs = normalized * 60 * 1000;
  state.autoCheckTimerId = window.setTimeout(async () => {
    state.autoCheckTimerId = null;
    state.autoCheckCycle = (state.autoCheckCycle || 0) + 1;
    await _refreshAll({
      forceCheck: true,
      notify: true,
      source: "auto",
      checkMode: `auto:${state.autoCheckCycle}`,
    });
    await maybePollSelfUpdateInfo({ notify: true });
    scheduleAutoCheckTimer();
  }, delayMs);
}

// ---------------------------------------------------------------------------
// Dialog utility
// ---------------------------------------------------------------------------

export function bindDialogOutsideToClose(dlg) {
  dlg.addEventListener("click", (ev) => {
    if (ev.target !== dlg) return;
    dlg.close();
  });
}

// ---------------------------------------------------------------------------
// Scroll-fade: toggle fade-top / fade-bottom based on scroll position
// ---------------------------------------------------------------------------

const FADE_THRESHOLD = 4;
const _wiredScrollFade = new WeakSet();
const _fadeColorCache = new WeakMap();       // el → computed color string
const _scrollRafPending = new WeakSet();     // throttle: one rAF per element
let _fadeColorGeneration = 0;                // bumped on theme change to invalidate cache

function updateScrollFade(el) {
  const canScrollUp = el.scrollTop > FADE_THRESHOLD;
  const canScrollDown = el.scrollTop + el.clientHeight < el.scrollHeight - FADE_THRESHOLD;
  el.classList.toggle("fade-top", canScrollUp);
  el.classList.toggle("fade-bottom", canScrollDown);
}

/** Wire all `.scroll-fade` elements, and observe future ones. */
export function initScrollFade() {
  for (const el of document.querySelectorAll(".scroll-fade")) wireScrollFade(el);

  // Single observer: watch for new .scroll-fade nodes AND dialog open attributes
  const observer = new MutationObserver((mutations) => {
    for (const m of mutations) {
      // Dialog opened → refresh its fade elements
      if (m.type === "attributes" && m.attributeName === "open") {
        const dlg = m.target;
        if (dlg.hasAttribute("open")) {
          requestAnimationFrame(() => {
            for (const el of dlg.querySelectorAll(".scroll-fade")) {
              syncFadeColor(el);
              updateScrollFade(el);
            }
          });
        }
        continue;
      }
      // New nodes added → wire any .scroll-fade elements
      for (const node of m.addedNodes) {
        if (node.nodeType !== 1) continue;
        if (node.classList?.contains("scroll-fade")) wireScrollFade(node);
        else if (node.querySelectorAll) {
          for (const child of node.querySelectorAll(".scroll-fade")) wireScrollFade(child);
        }
      }
    }
  });
  observer.observe(document.body, { childList: true, subtree: true });
  // Also watch dialog open/close attribute changes
  for (const dlg of document.querySelectorAll("dialog")) {
    observer.observe(dlg, { attributes: true, attributeFilter: ["open"] });
  }
}

const _rgbaRe = /rgba?\(\s*([\d.]+),\s*([\d.]+),\s*([\d.]+)(?:,\s*([\d.]+))?\)/;

function computeEffectiveBg(el) {
  const layers = [];
  let node = el;
  while (node && node !== document.documentElement) {
    const bg = getComputedStyle(node).backgroundColor;
    if (bg && bg !== "transparent" && bg !== "rgba(0, 0, 0, 0)") {
      layers.push(bg);
      const m = bg.match(_rgbaRe);
      if (m && (m[4] === undefined || parseFloat(m[4]) >= 1)) break;
    }
    node = node.parentElement;
  }
  if (!layers.length) return null;
  let r = 0, g = 0, b = 0;
  for (let i = layers.length - 1; i >= 0; i--) {
    const m = layers[i].match(_rgbaRe);
    if (!m) continue;
    const a = m[4] !== undefined ? parseFloat(m[4]) : 1;
    r = r * (1 - a) + parseFloat(m[1]) * a;
    g = g * (1 - a) + parseFloat(m[2]) * a;
    b = b * (1 - a) + parseFloat(m[3]) * a;
  }
  return `rgb(${Math.round(r)},${Math.round(g)},${Math.round(b)})`;
}

function wireScrollFade(el) {
  if (_wiredScrollFade.has(el)) return;
  _wiredScrollFade.add(el);
  el.addEventListener("scroll", () => {
    if (_scrollRafPending.has(el)) return;
    _scrollRafPending.add(el);
    requestAnimationFrame(() => {
      _scrollRafPending.delete(el);
      updateScrollFade(el);
    });
  }, { passive: true });
  requestAnimationFrame(() => {
    syncFadeColor(el);
    updateScrollFade(el);
  });
}

function syncFadeColor(el) {
  // Use cached value if theme hasn't changed
  const cached = _fadeColorCache.get(el);
  if (cached && cached.gen === _fadeColorGeneration) {
    el.style.setProperty("--fade-color", cached.color);
    return;
  }
  const bg = computeEffectiveBg(el);
  if (bg) {
    _fadeColorCache.set(el, { color: bg, gen: _fadeColorGeneration });
    el.style.setProperty("--fade-color", bg);
  }
}

/** Invalidate all fade color caches (called on theme change). */
export function invalidateFadeColors() {
  _fadeColorGeneration++;
  requestAnimationFrame(() => {
    for (const el of document.querySelectorAll(".scroll-fade")) syncFadeColor(el);
  });
}

/** Re-check scroll fade state for an element (call after content changes). */
export function refreshScrollFade(el) {
  if (!el?.classList?.contains("scroll-fade")) return;
  wireScrollFade(el);
  syncFadeColor(el);
  updateScrollFade(el);
}
