// Entry point: imports all modules, wires callbacks, registers event listeners, boots the app.

import {
  state,
  TAB_KEY,
  PROFILES_KEY,
  ACTIVE_PROFILE_KEY,
  WOW_KEY,
  OPT_SYMLINKS_KEY,
  OPT_XATTR_KEY,
  OPT_CLOCK12_KEY,
  OPT_THEME_KEY,
  OPT_FRIZ_FONT_KEY,
  OPT_AUTOCHECK_KEY,
  OPT_AUTOCHECK_MINUTES_KEY,
  OPT_CACHE_KEEP_KEY,
  DEFAULT_USE_FRIZ_FONT,
  DEFAULT_AUTO_CHECK_ENABLED,
  DEFAULT_CACHE_KEEP_VERSIONS,
  LOG_WRAP_KEY,
  LOG_AUTOSCROLL_KEY,
  LOG_LEVEL_KEY,
  WUDDLE_REPO_URL,
  WUDDLE_RELEASES_URL,
} from "./state.js";

import { $ } from "./utils.js";

import { safeInvoke } from "./commands.js";

import {
  log,
  renderLog,
  setLogLevel,
  setLogQuery,
  setLogWrap,
  setLogAutoscroll,
  copyLogToClipboard,
} from "./logs.js";

import {
  normalizeThemeId,
  setTheme,
  setUiFontStyle,
  closeThemedSelectMenus,
  ensureThemedSelect,
  syncThemedSelect,
  renderBusy,
  renderAutoCheckSettings,
  scheduleAutoCheckTimer,
  bindDialogOutsideToClose,
  setUiCallbacks,
} from "./ui.js";

import {
  normalizeProfileId,
  normalizeAutoCheckMinutes,
  normalizeLaunchConfig,
  defaultProfileWowDir,
  makeDefaultProfile,
  readProjectViewByProfile,
  syncProjectViewFromActiveProfile,
  persistProfiles,
  activeProfile,
  renderProfileTabs,
  renderInstanceList,
  setBackendActiveProfile,
  setProfileCallbacks,
  renderInstanceSettingsLaunchFields,
  clearInstanceSettingsDraft,
  saveInstanceSettingsFromDialog,
  confirmRemoveInstance,
  addInstance,
  profileWowDir,
} from "./profiles.js";

import {
  renderProjectViewButtons,
  renderAddPresets,
  applyAddDialogContext,
  renderProjectSearch,
  renderRepos,
  renderLastChecked,
  openUrl,
  openPath,
  setProjectView,
  openAddDialog,
  openAddDialogFor,
  addRepo,
  updateAll,
  handleUpdateAction,
  retryFailedFetches,
  rescanAddonDirectory,
  refreshAll,
  setFilter,
  toggleSort,
  setRepoCallbacks,
  closeActionsMenu,
  positionOpenMenu,
  confirmRemove,
} from "./repos.js";

import {
  renderGithubAuth,
  refreshGithubAuthStatus,
  saveGithubToken,
  clearGithubToken,
  connectGithub,
  renderGithubAuthHealth,
  setAuthCallbacks,
} from "./auth.js";

import {
  renderAboutInfo,
  refreshAboutInfo,
  updateWuddleInPlace,
  setAboutCallbacks,
} from "./about.js";

import { renderHome, launchGameFromHome, setHomeCallbacks } from "./home.js";
import { bindTurtleListeners } from "./turtle.js";

// ============================================================================
// render() — top-level coordinator (stays here to break the repos ↔ home cycle)
// ============================================================================

function render() {
  closeThemedSelectMenus();
  applyAddDialogContext();
  const addDialog = $("dlgAdd");
  if (addDialog?.open) {
    renderAddPresets();
  }

  if (state.tab === "home") {
    renderHome();
  }

  renderProjectViewButtons();
  const hasProfile = !!activeProfile();
  $("btnAddOpen").disabled = !hasProfile;
  const rescanBtn = $("btnRescanAddons");
  if (rescanBtn) {
    rescanBtn.classList.toggle("hidden", state.projectView !== "addons");
    rescanBtn.disabled = !hasProfile;
  }
  $("btnUpdateAll").disabled = !hasProfile;
  $("btnViewMods").disabled = !hasProfile;
  $("btnViewAddons").disabled = !hasProfile;

  if (state.tab !== "projects") return;

  renderProjectSearch();
  renderRepos();
}

// ============================================================================
// setTab() — coordinator for tab switching
// ============================================================================

function setTab(tab) {
  if (tab === "home") state.tab = "home";
  else if (tab === "options") state.tab = "options";
  else if (tab === "logs") state.tab = "logs";
  else if (tab === "about") state.tab = "about";
  else state.tab = "projects";
  localStorage.setItem(TAB_KEY, state.tab);

  $("panelHome").classList.toggle("hidden", state.tab !== "home");
  $("panelProjects").classList.toggle("hidden", state.tab !== "projects");
  $("panelOptions").classList.toggle("hidden", state.tab !== "options");
  $("panelLogs").classList.toggle("hidden", state.tab !== "logs");
  $("panelAbout").classList.toggle("hidden", state.tab !== "about");

  $("tabHome").classList.toggle("active", state.tab === "home");
  $("tabOptions").classList.toggle("active", state.tab === "options");
  $("tabLogs").classList.toggle("active", state.tab === "logs");
  $("tabAbout").classList.toggle("active", state.tab === "about");
  renderProfileTabs();
  renderProjectViewButtons();

  if (state.tab === "options") {
    renderInstanceList();
    void refreshGithubAuthStatus();
  } else if (state.tab === "home") {
    renderHome();
  } else if (state.tab === "about") {
    void refreshAboutInfo();
  } else if (state.tab === "projects") {
    render();
  } else if (state.tab === "logs" && state.logDirty) {
    renderLog();
  }
}

// ============================================================================
// showProjectsPanel()
// ============================================================================

function showProjectsPanel() {
  if (state.tab !== "projects") {
    setTab("projects");
  }
}

// ============================================================================
// loadSettings()
// ============================================================================

function loadSettings() {
  let profiles = [];
  const rawProfiles = localStorage.getItem(PROFILES_KEY);
  try {
    if (rawProfiles !== null) {
      const parsed = JSON.parse(rawProfiles);
      if (Array.isArray(parsed)) {
        profiles = parsed.map((p) => ({
          id: normalizeProfileId(p?.id || p?.name || "default"),
          name: String(p?.name || "").trim() || "WoW",
          wowDir: String(p?.wowDir || "").trim(),
          launch: normalizeLaunchConfig(p?.launch),
          likesTurtles: !!p?.likesTurtles,
        }));
      }
    }
  } catch (_) {}
  if (!profiles.length && rawProfiles === null) {
    // Last-resort migration path from single-profile storage.
    const wowDir = defaultProfileWowDir();
    if (wowDir) profiles = [makeDefaultProfile()];
  }
  const ids = new Set();
  for (const p of profiles) {
    let id = p.id;
    let n = 2;
    while (ids.has(id)) {
      id = `${p.id}-${n++}`;
    }
    p.id = id;
    ids.add(id);
  }
  state.profiles = profiles;
  const wanted = normalizeProfileId(localStorage.getItem(ACTIVE_PROFILE_KEY) || "default");
  state.activeProfileId = state.profiles.some((p) => p.id === wanted)
    ? wanted
    : (state.profiles[0]?.id || "");
  state.projectViewByProfile = readProjectViewByProfile();
  syncProjectViewFromActiveProfile();

  const symlinks = localStorage.getItem(OPT_SYMLINKS_KEY) === "true";
  const xattr = localStorage.getItem(OPT_XATTR_KEY) === "true";
  const clock12 = localStorage.getItem(OPT_CLOCK12_KEY) === "true";
  const savedTheme = normalizeThemeId(localStorage.getItem(OPT_THEME_KEY));
  const rawFriz = localStorage.getItem(OPT_FRIZ_FONT_KEY);
  const useFrizFont = rawFriz === null ? DEFAULT_USE_FRIZ_FONT : rawFriz === "true";
  const rawAutoCheck = localStorage.getItem(OPT_AUTOCHECK_KEY);
  const autoCheckEnabled =
    rawAutoCheck === null ? DEFAULT_AUTO_CHECK_ENABLED : rawAutoCheck === "true";
  const autoCheckMinutes = normalizeAutoCheckMinutes(
    localStorage.getItem(OPT_AUTOCHECK_MINUTES_KEY),
  );
  const rawCacheKeep = localStorage.getItem(OPT_CACHE_KEEP_KEY);
  const cacheKeepVersions = rawCacheKeep !== null
    ? Math.max(0, Math.min(10, parseInt(rawCacheKeep, 10) || DEFAULT_CACHE_KEEP_VERSIONS))
    : DEFAULT_CACHE_KEEP_VERSIONS;
  $("optSymlinks").checked = symlinks;
  $("optXattr").checked = xattr;
  $("optClock12").checked = clock12;
  if ($("optTheme")) $("optTheme").value = savedTheme;
  $("optFrizFont").checked = useFrizFont;
  $("optAutoCheck").checked = autoCheckEnabled;
  const autoCheckMinutesInput = $("optAutoCheckMinutes");
  if (autoCheckMinutesInput instanceof HTMLInputElement) {
    autoCheckMinutesInput.value = String(autoCheckMinutes);
  }
  state.clock12 = clock12;
  state.theme = savedTheme;
  state.useFrizFont = useFrizFont;
  state.autoCheckEnabled = autoCheckEnabled;
  state.autoCheckMinutes = autoCheckMinutes;
  state.cacheKeepVersions = cacheKeepVersions;
  const cacheKeepInput = $("optCacheKeep");
  if (cacheKeepInput instanceof HTMLInputElement) {
    cacheKeepInput.value = String(cacheKeepVersions);
  }
  setTheme(savedTheme);
  setUiFontStyle(useFrizFont);
  renderAutoCheckSettings();

  const savedTab = localStorage.getItem(TAB_KEY) || "home";
  setTab(new Set(["home", "projects", "options", "logs", "about"]).has(savedTab) ? savedTab : "home");

  const logWrap = localStorage.getItem(LOG_WRAP_KEY) === "true";
  const logAutoScrollRaw = localStorage.getItem(LOG_AUTOSCROLL_KEY);
  const logAutoScroll = logAutoScrollRaw === null ? true : logAutoScrollRaw === "true";
  const logLevel = localStorage.getItem(LOG_LEVEL_KEY) || "all";
  state.logWrap = logWrap;
  state.logAutoScroll = logAutoScroll;
  state.logLevel = new Set(["all", "info", "error"]).has(logLevel) ? logLevel : "all";
  $("optLogWrap").checked = logWrap;
  $("optLogAutoscroll").checked = logAutoScroll;
  renderProfileTabs();
  renderInstanceList();
  persistProfiles();
  scheduleAutoCheckTimer();
}

// ============================================================================
// saveOptionFlags()
// ============================================================================

function saveOptionFlags() {
  localStorage.setItem(OPT_SYMLINKS_KEY, $("optSymlinks").checked ? "true" : "false");
  localStorage.setItem(OPT_XATTR_KEY, $("optXattr").checked ? "true" : "false");
  localStorage.setItem(OPT_CLOCK12_KEY, $("optClock12").checked ? "true" : "false");
  const autoCheckEnabled = !!$("optAutoCheck").checked;
  const autoCheckMinutes = normalizeAutoCheckMinutes($("optAutoCheckMinutes").value);
  const selectedTheme = normalizeThemeId(state.theme || $("optTheme")?.value);
  const useFrizFont = !!$("optFrizFont")?.checked;
  localStorage.setItem(OPT_AUTOCHECK_KEY, autoCheckEnabled ? "true" : "false");
  localStorage.setItem(OPT_AUTOCHECK_MINUTES_KEY, String(autoCheckMinutes));
  const cacheKeep = Math.max(0, Math.min(10, parseInt($("optCacheKeep")?.value, 10) || DEFAULT_CACHE_KEEP_VERSIONS));
  localStorage.setItem(OPT_CACHE_KEEP_KEY, String(cacheKeep));
  localStorage.setItem(OPT_THEME_KEY, selectedTheme);
  localStorage.setItem(OPT_FRIZ_FONT_KEY, useFrizFont ? "true" : "false");
  setTheme(selectedTheme);
  setUiFontStyle(useFrizFont);
  state.clock12 = $("optClock12").checked;
  state.autoCheckEnabled = autoCheckEnabled;
  state.autoCheckMinutes = autoCheckMinutes;
  state.cacheKeepVersions = cacheKeep;
  renderAutoCheckSettings();
  scheduleAutoCheckTimer();
  renderLastChecked();
  render();
  renderLog();
}

// ============================================================================
// Cross-module callback wiring
// ============================================================================

setRepoCallbacks({ render, renderGithubAuthHealth, setTab, showProjectsPanel });
setProfileCallbacks({ render, refreshAll, setTab });
setAboutCallbacks({ setTab });
setAuthCallbacks({ refreshAll });
setHomeCallbacks({ refreshAll, openAddDialogFor });
setUiCallbacks({ refreshAll });

// ============================================================================
// Event listeners
// ============================================================================

$("btnUpdateAll").addEventListener("click", handleUpdateAction);
$("btnAddInstance").addEventListener("click", async () => {
  await addInstance();
});
$("btnRetryFailed").addEventListener("click", retryFailedFetches);
$("btnViewMods").addEventListener("click", () => {
  setProjectView("mods");
  showProjectsPanel();
});
$("btnViewAddons").addEventListener("click", () => {
  setProjectView("addons");
  showProjectsPanel();
});

$("tabHome").addEventListener("click", () => setTab("home"));
$("tabOptions").addEventListener("click", () => setTab("options"));
$("tabLogs").addEventListener("click", () => setTab("logs"));
$("tabAbout").addEventListener("click", () => setTab("about"));

$("homeBtnUpdateAll").addEventListener("click", updateAll);
$("homeBtnRefreshOnly").addEventListener("click", () =>
  refreshAll({ forceCheck: true, notify: true, source: "manual" }),
);
$("homeBtnPlay").addEventListener("click", launchGameFromHome);
$("homeBtnAddMod").addEventListener("click", () => {
  const menu = $("homeAddMenu");
  if (menu) menu.open = false;
  openAddDialogFor("mods");
});
$("homeBtnAddAddon").addEventListener("click", () => {
  const menu = $("homeAddMenu");
  if (menu) menu.open = false;
  openAddDialogFor("addons");
});
bindTurtleListeners();
$("btnRescanAddons").addEventListener("click", async () => {
  await rescanAddonDirectory();
});

$("instanceSettingsLaunchMethod").addEventListener("change", () => {
  renderInstanceSettingsLaunchFields($("instanceSettingsLaunchMethod").value);
  syncThemedSelect($("instanceSettingsLaunchMethod"));
});
$("btnInstanceSettingsChoosePath").addEventListener("click", async () => {
  try {
    const res = await safeInvoke(
      "plugin:dialog|open",
      {
        options: {
          directory: false,
          multiple: false,
          title: "Select game executable (Wow.exe or VanillaFixes.exe)",
          filters: [
            {
              name: "Windows executable",
              extensions: ["exe"],
            },
          ],
        },
      },
      { timeoutMs: 0 },
    );
    const picked = Array.isArray(res) ? res[0] : res;
    if (!picked) return;
    $("instanceSettingsPath").value = String(picked);
    $("btnInstanceSettingsOpenPath").disabled = !profileWowDir({
      wowDir: String(picked),
    });
  } catch (e) {
    log(`ERROR picker: ${e.message}`);
  }
});
$("btnInstanceSettingsOpenPath").addEventListener("click", async () => {
  const path = profileWowDir({ wowDir: $("instanceSettingsPath").value });
  await openPath(path);
});
$("instanceSettingsPath").addEventListener("input", () => {
  $("btnInstanceSettingsOpenPath").disabled = !profileWowDir({
    wowDir: $("instanceSettingsPath").value,
  });
});
$("btnInstanceSettingsSave").addEventListener("click", async (ev) => {
  ev.preventDefault();
  const profileId = normalizeProfileId($("instanceSettingsId").value || "");
  const draft = state.instanceSettingsDraft;
  const isCreatingNew =
    !!draft?.isNew && normalizeProfileId(draft.id || "") === profileId;
  const wasActiveBeforeSave = profileId === state.activeProfileId;
  const ok = saveInstanceSettingsFromDialog();
  if (!ok) return;
  $("dlgInstanceSettings").close();
  if (isCreatingNew || wasActiveBeforeSave) {
    await setBackendActiveProfile();
    await refreshAll();
  }
});
$("btnInstanceSettingsCancel").addEventListener("click", () => {
  clearInstanceSettingsDraft();
});
$("dlgInstanceSettings").addEventListener("close", () => {
  clearInstanceSettingsDraft();
});

$("optSymlinks").addEventListener("change", saveOptionFlags);
$("optXattr").addEventListener("change", saveOptionFlags);
$("optClock12").addEventListener("change", saveOptionFlags);
$("optFrizFont").addEventListener("change", saveOptionFlags);
$("optAutoCheck").addEventListener("change", saveOptionFlags);
$("optAutoCheckMinutes").addEventListener("change", saveOptionFlags);
$("optCacheKeep").addEventListener("change", saveOptionFlags);
$("themePicker")?.addEventListener("click", (ev) => {
  const target = ev.target;
  if (!(target instanceof Element)) return;
  const btn = target.closest(".theme-swatch");
  if (!(btn instanceof HTMLButtonElement)) return;
  const selectedTheme = normalizeThemeId(btn.getAttribute("data-theme") || "");
  if (selectedTheme === state.theme) return;
  setTheme(selectedTheme);
  saveOptionFlags();
});
$("btnConnectGithub").addEventListener("click", connectGithub);
$("btnSaveGithubToken").addEventListener("click", saveGithubToken);
$("btnClearGithubToken").addEventListener("click", clearGithubToken);
$("btnAboutRefresh").addEventListener("click", () => {
  void refreshAboutInfo({ force: true });
});
$("btnAboutUpdate").addEventListener("click", () => {
  void updateWuddleInPlace();
});
$("btnAboutGithub").addEventListener("click", async () => {
  await openUrl(WUDDLE_REPO_URL);
});
$("aboutLatestVersion").addEventListener("click", async (ev) => {
  ev.preventDefault();
  if (!$("aboutLatestVersion").disabled) {
    await openUrl(WUDDLE_RELEASES_URL);
  }
});
$("superwowGuideLink")?.addEventListener("click", async (ev) => {
  ev.preventDefault();
  await openUrl("https://github.com/pepopo978/SuperwowInstallation");
});

const dlgAdd = $("dlgAdd");
async function submitAddFromDialog() {
  const url = String($("repoUrl").value ?? "").trim();
  if (url) {
    dlgAdd.close();
  }
  await addRepo();
}

$("btnAddOpen").addEventListener("click", () => {
  applyAddDialogContext();
  openAddDialog();
});
$("btnAdd").addEventListener("click", async (ev) => {
  ev.preventDefault();
  await submitAddFromDialog();
});
$("repoUrl").addEventListener("keydown", async (ev) => {
  if (ev.key !== "Enter") return;
  ev.preventDefault();
  await submitAddFromDialog();
});

$("projectSearchInput").addEventListener("input", (ev) => {
  const target = ev.target;
  if (!(target instanceof HTMLInputElement)) return;
  state.projectSearchQuery = target.value;
  render();
});
$("projectSearchClear").addEventListener("click", (ev) => {
  ev.preventDefault();
  state.projectSearchQuery = "";
  render();
  const input = $("projectSearchInput");
  if (input instanceof HTMLInputElement) {
    input.focus();
  }
});

$("btnCopyLog").addEventListener("click", copyLogToClipboard);
$("btnClearLog").addEventListener("click", () => {
  state.logLines = [];
  renderLog();
});
$("optLogWrap").addEventListener("change", () => {
  setLogWrap($("optLogWrap").checked);
});
$("optLogAutoscroll").addEventListener("change", () => {
  setLogAutoscroll($("optLogAutoscroll").checked);
});
$("logSearch").addEventListener("input", () => {
  setLogQuery($("logSearch").value);
});
$("btnRemoveConfirm").addEventListener("click", async (ev) => {
  ev.preventDefault();
  await confirmRemove();
});
$("btnRemoveInstanceConfirm").addEventListener("click", async (ev) => {
  ev.preventDefault();
  await confirmRemoveInstance();
});

document.querySelectorAll(".filter-btn[data-filter]").forEach((btn) => {
  btn.addEventListener("click", () => {
    setFilter(btn.getAttribute("data-filter") || "all");
  });
});

document.querySelectorAll("#repoThead .th.sortable").forEach((th) => {
  th.addEventListener("click", () => {
    toggleSort(th.getAttribute("data-sort") || "");
  });
});

document.querySelectorAll(".filter-btn[data-log-level]").forEach((btn) => {
  btn.addEventListener("click", () => {
    setLogLevel(btn.getAttribute("data-log-level") || "all");
  });
});

bindDialogOutsideToClose(dlgAdd);
bindDialogOutsideToClose($("dlgAddonConflict"));
bindDialogOutsideToClose($("dlgRemove"));
bindDialogOutsideToClose($("dlgRemoveInstance"));
bindDialogOutsideToClose($("dlgInstanceSettings"));

document.addEventListener("click", (ev) => {
  if (state.openMenuRepoId === null) return;
  if (!(ev.target instanceof Element)) return;
  if (ev.target.closest(".menu-wrap")) return;
  closeActionsMenu();
});

document.addEventListener("click", (ev) => {
  const menu = $("homeAddMenu");
  if (!menu || !menu.open) return;
  if (!(ev.target instanceof Element)) return;
  if (ev.target.closest("#homeAddMenu")) return;
  menu.open = false;
});

document.addEventListener("click", (ev) => {
  const menu = $("profilePicker");
  if (!(menu instanceof HTMLElement) || !menu.classList.contains("open")) return;
  if (!(ev.target instanceof Element)) return;
  if (ev.target.closest("#profilePicker")) return;
  menu.classList.remove("open");
});

document.addEventListener("click", (ev) => {
  if (!(ev.target instanceof Element)) return;
  if (ev.target.closest(".select-menu")) return;
  closeThemedSelectMenus();
});

document.addEventListener("keydown", (ev) => {
  if (ev.key !== "Escape") return;
  closeThemedSelectMenus();
  if (state.openMenuRepoId === null) return;
  closeActionsMenu();
});

window.addEventListener("resize", () => {
  closeThemedSelectMenus();
  if (state.openMenuRepoId === null) return;
  requestAnimationFrame(positionOpenMenu);
});

window.addEventListener(
  "scroll",
  () => {
    closeThemedSelectMenus();
  },
  true,
);

const tableScroller = document.querySelector(".table-scroll");
if (tableScroller instanceof HTMLElement) {
  tableScroller.addEventListener("scroll", () => {
    if (state.openMenuRepoId === null) return;
    closeActionsMenu();
  });
}

// ============================================================================
// Boot
// ============================================================================

loadSettings();
ensureThemedSelect($("mode"));
ensureThemedSelect($("instanceSettingsLaunchMethod"));
renderAddPresets();
renderBusy();
renderLog();
renderGithubAuth(null);
renderAboutInfo();
void refreshGithubAuthStatus();
log("Ready.");
refreshAll({ notify: true, source: "startup" });
