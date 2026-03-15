// About tab: info display, self-update, version polling, changelog
import { state, SELF_UPDATE_POLL_MINUTES } from "./state.js";
import { $, formatTime, escapeHtml } from "./utils.js";
import { safeInvoke } from "./commands.js";
import { log, logOperationResult } from "./logs.js";
import { showToast } from "./ui.js";
import { openUrl } from "./repos.js";

let _setTab = (_tab) => {};
export function setAboutCallbacks(cbs) {
  if (cbs.setTab) _setTab = cbs.setTab;
}

function aboutValue(value, fallback = "Unknown") {
  if (value === null || value === undefined) return fallback;
  const text = String(value).trim();
  return text || fallback;
}

export function setAboutStatus(message, kind = "") {
  const status = $("aboutStatus");
  status.classList.remove("status-ok", "status-warn");
  if (kind) status.classList.add(kind);
  status.textContent = message;
}

export function renderAboutInfo() {
  const info = state.aboutInfo || {};
  $("aboutAppVersion").textContent = aboutValue(info.appVersion, "Unknown");
  const latestEl = $("aboutLatestVersion");
  const latest = String(state.aboutLatestVersion || "").trim();
  if (latest) {
    latestEl.textContent = latest;
    latestEl.disabled = false;
    latestEl.title = "Open GitHub Releases";
  } else {
    latestEl.textContent = "Unknown";
    latestEl.disabled = true;
    latestEl.title = "Latest release version unavailable.";
  }
  $("aboutPackageName").textContent = aboutValue(info.packageName, "Unknown");
  renderAboutUpdateAction();
}

export function renderAboutUpdateAction() {
  const btn = $("btnAboutUpdate");
  if (!btn) return;

  const updateInfo = state.aboutSelfUpdate;
  if (state.aboutSelfUpdateBusy) {
    btn.disabled = true;
    btn.classList.remove("primary");
    btn.textContent = "Updating…";
    btn.title = "Downloading and staging update…";
    return;
  }

  if (!updateInfo || !updateInfo.supported) {
    btn.disabled = true;
    btn.classList.remove("primary");
    btn.textContent = "Self-update unavailable";
    btn.title =
      updateInfo?.message ||
      "In-app updates are currently available only in Windows portable launcher builds.";
    return;
  }

  if (updateInfo.updateAvailable) {
    const latest = String(updateInfo.latestVersion || "").trim();
    btn.disabled = false;
    btn.classList.add("primary");
    btn.textContent = latest ? `Update to ${latest}` : "Update Wuddle";
    btn.title = "Download latest release and restart Wuddle.";
    return;
  }

  if (updateInfo.assetsPending) {
    const latest = String(updateInfo.latestVersion || "").trim();
    btn.disabled = true;
    btn.classList.remove("primary");
    btn.textContent = latest ? `${latest} building\u2026` : "Update building\u2026";
    btn.title = "Release assets are still being built by CI. Click Refresh to check again.";
    return;
  }

  if (!updateInfo.latestVersion) {
    btn.disabled = true;
    btn.classList.remove("primary");
    btn.textContent = "Update check failed";
    btn.title = updateInfo.message || "Could not determine latest version.";
    return;
  }

  btn.disabled = true;
  btn.classList.remove("primary");
  btn.textContent = "Up to date";
  btn.title = updateInfo.message || "No newer release detected.";
}

export async function refreshAboutInfo({ force = false } = {}) {
  if (state.aboutLoaded && !force && state.aboutLatestVersion) {
    renderAboutInfo();
    setAboutStatus(
      `Detected at ${formatTime(state.aboutRefreshedAt || new Date())}.`,
      "status-ok",
    );
    return;
  }
  setAboutStatus("Loading application details…");
  try {
    const info = await safeInvoke("wuddle_about_info", {}, { timeoutMs: 4000 });
    state.aboutInfo = info && typeof info === "object" ? info : {};
    try {
      const updateInfo = await safeInvoke("wuddle_self_update_info", {}, { timeoutMs: 12000 });
      state.aboutSelfUpdate = updateInfo && typeof updateInfo === "object" ? updateInfo : null;
      state.aboutLatestVersion = String(state.aboutSelfUpdate?.latestVersion || "").trim() || null;
    } catch (selfUpdateErr) {
      state.aboutSelfUpdate = null;
      log(`ERROR self-update info: ${selfUpdateErr.message || selfUpdateErr}`);
    }
  } catch (e) {
    setAboutStatus(`Could not load application details: ${e.message}`, "status-warn");
    log(`ERROR about: ${e.message}`);
    return;
  }

  state.aboutLoaded = true;
  state.aboutRefreshedAt = new Date();
  renderAboutInfo();
  const latestHint = state.aboutLatestVersion ? "" : " Latest version unavailable.";
  const updaterHint =
    state.aboutSelfUpdate && state.aboutSelfUpdate.message
      ? ` ${state.aboutSelfUpdate.message}`
      : "";
  const statusKind = state.aboutSelfUpdate?.assetsPending ? "status-warn" : "status-ok";
  setAboutStatus(
    `Detected at ${formatTime(state.aboutRefreshedAt)}.${latestHint}${updaterHint}`,
    statusKind,
  );
}

export async function updateWuddleInPlace() {
  const updateInfo = state.aboutSelfUpdate;
  if (!updateInfo?.supported) {
    log("Self-update is unavailable for this build.");
    return;
  }
  if (!updateInfo.updateAvailable) {
    log("Wuddle is already up to date.");
    return;
  }

  const latest = String(updateInfo.latestVersion || "").trim() || "latest";
  const restartNote = updateInfo.launcherLayout
    ? "then restart via launcher"
    : "then restart";
  const proceed = window.confirm(
    `Wuddle will download and stage ${latest}, ${restartNote}.\n\nContinue?`,
  );
  if (!proceed) {
    log("Cancelled self-update.");
    return;
  }

  state.aboutSelfUpdateBusy = true;
  renderAboutUpdateAction();
  try {
    const result = await safeInvoke("wuddle_self_update_apply", {}, { timeoutMs: 180000 });
    await logOperationResult(result);
    log("Restarting Wuddle to finish update…");
    await safeInvoke("wuddle_self_update_restart", {}, { timeoutMs: 5000 });
  } catch (e) {
    log(`ERROR self-update: ${e.message}`);
    await refreshAboutInfo({ force: true });
  } finally {
    state.aboutSelfUpdateBusy = false;
    renderAboutUpdateAction();
  }
}

export async function maybePollSelfUpdateInfo({ notify = false } = {}) {
  const now = Date.now();
  if (now < state.nextSelfUpdatePollAt) return;
  state.nextSelfUpdatePollAt = now + SELF_UPDATE_POLL_MINUTES * 60 * 1000;

  try {
    const info = await safeInvoke("wuddle_self_update_info", {}, { timeoutMs: 12000 });
    state.aboutSelfUpdate = info && typeof info === "object" ? info : null;
    const latest = String(state.aboutSelfUpdate?.latestVersion || "").trim();
    if (latest) state.aboutLatestVersion = latest;
    if (state.tab === "about") renderAboutInfo();

    if (!notify) return;
    if (!state.aboutSelfUpdate?.supported || !state.aboutSelfUpdate?.updateAvailable || !latest) return;
    if (state.lastSelfUpdateNotifyVersion === latest) return;
    state.lastSelfUpdateNotifyVersion = latest;

    showToast(`Wuddle ${latest} is available.`, {
      kind: "warn",
      onAction: () => _setTab("about"),
    });
  } catch (_) {
    // Silent by design to avoid repeated noisy errors on background polling.
  }
}

// ---------------------------------------------------------------------------
// Changelog viewer
// ---------------------------------------------------------------------------

/** Convert simple changelog markdown to HTML. */
export function changelogToHtml(md) {
  const lines = md.split("\n");
  let html = "";
  let inList = false;
  let inCode = false;
  let codeLang = "";

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trimEnd();

    // Fenced code blocks (``` or ~~~)
    const fenceMatch = line.match(/^(`{3,}|~{3,})\s*(\S*)/);
    if (fenceMatch) {
      if (!inCode) {
        if (inList) { html += "</ul>"; inList = false; }
        codeLang = fenceMatch[2] || "";
        const cls = codeLang ? ` class="language-${escapeHtml(codeLang)}"` : "";
        html += `<pre><code${cls}>`;
        inCode = true;
      } else {
        html += "</code></pre>";
        inCode = false;
      }
      continue;
    }

    if (inCode) {
      html += escapeHtml(lines[i]) + "\n";
      continue;
    }

    if (line.startsWith("# ") && !line.startsWith("## ")) {
      if (inList) { html += "</ul>"; inList = false; }
      html += `<h1>${escapeHtml(line.slice(2))}</h1>`;
      continue;
    }
    if (line.startsWith("### ")) {
      if (inList) { html += "</ul>"; inList = false; }
      html += `<h3>${inlineFormat(line.slice(4))}</h3>`;
      continue;
    }
    if (line.startsWith("## ")) {
      if (inList) { html += "</ul>"; inList = false; }
      html += `<h2>${escapeHtml(line.slice(3))}</h2>`;
      continue;
    }
    if (line.startsWith("- ")) {
      if (!inList) { html += "<ul>"; inList = true; }
      html += `<li>${inlineFormat(line.slice(2))}</li>`;
      continue;
    }
    if (line.startsWith("  - ")) {
      if (!inList) { html += "<ul>"; inList = true; }
      html += `<li class="nested">${inlineFormat(line.slice(4))}</li>`;
      continue;
    }
    if (line.trim() === "") {
      if (inList) { html += "</ul>"; inList = false; }
      continue;
    }
    if (inList) { html += "</ul>"; inList = false; }
    html += `<p>${inlineFormat(line)}</p>`;
  }
  if (inCode) html += "</code></pre>";
  if (inList) html += "</ul>";
  return html;
}

/** Apply inline markdown formatting (bold, code, links) with HTML escaping. */
function inlineFormat(text) {
  let out = escapeHtml(text);
  // ``code`` (double backtick)
  out = out.replace(/``(.+?)``/g, "<code>$1</code>");
  // `code` (single backtick)
  out = out.replace(/`(.+?)`/g, "<code>$1</code>");
  // **bold**
  out = out.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
  // [text](url)
  out = out.replace(
    /\[([^\]]+)\]\((https?:\/\/[^)]+)\)/g,
    '<a href="$2" class="changelog-link" data-href="$2">$1</a>',
  );
  return out;
}

export async function showChangelog() {
  const dlg = $("dlgChangelog");
  const content = $("changelogContent");
  if (!dlg || !content) return;

  content.innerHTML = '<p class="hint">Loading changelog…</p>';
  dlg.showModal();

  try {
    const md = await safeInvoke("wuddle_fetch_changelog", {}, { timeoutMs: 15000 });
    content.innerHTML = changelogToHtml(md);

    // Wire up links to open via system browser.
    content.querySelectorAll(".changelog-link").forEach((a) => {
      a.addEventListener("click", (ev) => {
        ev.preventDefault();
        void openUrl(a.dataset.href);
      });
    });
  } catch (err) {
    content.innerHTML = `<p class="hint">Failed to load changelog: ${escapeHtml(err?.message || String(err))}</p>`;
  }
}
