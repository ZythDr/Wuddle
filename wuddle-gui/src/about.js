// About tab: info display, self-update, version polling
import { state, WUDDLE_RELEASES_API_URL, SELF_UPDATE_POLL_MINUTES } from "./state.js";
import { $, formatTime } from "./utils.js";
import { safeInvoke } from "./commands.js";
import { log, logOperationResult } from "./logs.js";
import { showToast } from "./ui.js";

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

export async function fetchLatestWuddleReleaseTag() {
  const ctrl = new AbortController();
  const timer = window.setTimeout(() => ctrl.abort(), 4500);
  try {
    const resp = await fetch(WUDDLE_RELEASES_API_URL, {
      method: "GET",
      headers: { Accept: "application/vnd.github+json" },
      signal: ctrl.signal,
    });
    if (!resp.ok) {
      throw new Error(`HTTP ${resp.status}`);
    }
    const data = await resp.json();
    const tag = String(data?.tag_name || "").trim();
    if (!tag) {
      throw new Error("Latest release tag not found");
    }
    return tag;
  } finally {
    window.clearTimeout(timer);
  }
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
      state.aboutLatestVersion = await fetchLatestWuddleReleaseTag();
      log(`ERROR self-update info: ${selfUpdateErr.message || selfUpdateErr}`);
    }
  } catch (e) {
    setAboutStatus(`Could not load application details: ${e.message}`, "status-warn");
    log(`ERROR about: ${e.message}`);
    return;
  }

  try {
    if (!state.aboutLatestVersion) {
      state.aboutLatestVersion = await fetchLatestWuddleReleaseTag();
    }
  } catch (latestErr) {
    if (!state.aboutLatestVersion) {
      state.aboutLatestVersion = null;
    }
    log(`ERROR latest version check: ${latestErr.message || latestErr}`);
  }

  state.aboutLoaded = true;
  state.aboutRefreshedAt = new Date();
  renderAboutInfo();
  const latestHint = state.aboutLatestVersion ? "" : " Latest version unavailable.";
  const updaterHint =
    state.aboutSelfUpdate && state.aboutSelfUpdate.message
      ? ` ${state.aboutSelfUpdate.message}`
      : "";
  setAboutStatus(
    `Detected at ${formatTime(state.aboutRefreshedAt)}.${latestHint}${updaterHint}`,
    "status-ok",
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
  const proceed = window.confirm(
    `Wuddle will download and stage ${latest}, then restart via launcher.\n\nContinue?`,
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

export async function maybePollSelfUpdateInfo({ force = false, notify = false } = {}) {
  const now = Date.now();
  if (!force && now < state.nextSelfUpdatePollAt) return;
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
