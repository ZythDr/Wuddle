import { state, WOW_KEY, PROFILES_KEY, ACTIVE_PROFILE_KEY, PROJECT_VIEW_BY_PROFILE_KEY, DEFAULT_AUTO_CHECK_MINUTES, MIN_AUTO_CHECK_MINUTES, MAX_AUTO_CHECK_MINUTES } from "./state.js";
import { $ } from "./utils.js";
import { log } from "./logs.js";
import { safeInvoke } from "./commands.js";
import { syncThemedSelect, withBusy } from "./ui.js";

let _render = () => {};
let _refreshAll = async () => {};
let _setTab = (_tab) => {};
export function setProfileCallbacks(cbs) {
  if (cbs.render) _render = cbs.render;
  if (cbs.refreshAll) _refreshAll = cbs.refreshAll;
  if (cbs.setTab) _setTab = cbs.setTab;
}

export function normalizeProfileId(value) {
  const base = String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return base || "default";
}

export function normalizeProjectView(value) {
  return String(value || "").toLowerCase() === "addons" ? "addons" : "mods";
}

export function normalizeAutoCheckMinutes(value) {
  if (value === null || value === undefined || value === "") return DEFAULT_AUTO_CHECK_MINUTES;
  const num = Number(value);
  if (!Number.isFinite(num) || num <= 0) return DEFAULT_AUTO_CHECK_MINUTES;
  return Math.max(MIN_AUTO_CHECK_MINUTES, Math.min(MAX_AUTO_CHECK_MINUTES, Math.floor(num)));
}

export function defaultLaunchConfig() {
  return {
    method: "auto",
    lutrisTarget: "",
    wineCommand: "wine",
    wineArgs: "",
    customCommand: "",
    customArgs: "",
    workingDir: "",
    envText: "",
    clearWdb: false,
  };
}

export function normalizeLaunchConfig(raw) {
  const input = raw && typeof raw === "object" ? raw : {};
  const methodRaw = String(input.method || "auto").trim().toLowerCase();
  const method = new Set(["auto", "lutris", "wine", "custom"]).has(methodRaw) ? methodRaw : "auto";
  return {
    method,
    lutrisTarget: String(input.lutrisTarget || "").trim(),
    wineCommand: String(input.wineCommand || "wine").trim() || "wine",
    wineArgs: String(input.wineArgs || "").trim(),
    customCommand: String(input.customCommand || "").trim(),
    customArgs: String(input.customArgs || "").trim(),
    workingDir: String(input.workingDir || "").trim(),
    envText: String(input.envText || "").trim(),
    clearWdb: !!input.clearWdb,
  };
}

export function profileLaunch(profile) {
  return normalizeLaunchConfig(profile?.launch);
}

export function profileLikesTurtles(profile) {
  return !!(profile && profile.likesTurtles);
}

export function rawWowPath(profile) {
  return String(profile?.wowDir || "").trim();
}

function parentDirOfPath(path) {
  const value = String(path || "").trim();
  if (!value) return "";
  const cut = Math.max(value.lastIndexOf("/"), value.lastIndexOf("\\"));
  if (cut < 1) return "";
  return value.slice(0, cut);
}

function isExePath(path) {
  return /\.exe$/i.test(String(path || "").trim());
}

export function profileExecutablePath(profile) {
  const raw = rawWowPath(profile);
  return isExePath(raw) ? raw : "";
}

export function profileWowDir(profile) {
  const raw = rawWowPath(profile);
  if (!raw) return "";
  if (isExePath(raw)) return parentDirOfPath(raw);
  return raw;
}

export function launchSummary(profile) {
  const launch = profileLaunch(profile);
  if (launch.method === "lutris") {
    return launch.lutrisTarget ? `Lutris: ${launch.lutrisTarget}` : "Lutris";
  }
  if (launch.method === "wine") {
    return launch.wineCommand ? `Wine: ${launch.wineCommand}` : "Wine";
  }
  if (launch.method === "custom") {
    return launch.customCommand ? `Custom: ${launch.customCommand}` : "Custom command";
  }
  return "Auto";
}

export function launchPayload(profile) {
  const launch = profileLaunch(profile);
  const env = {};
  const lines = launch.envText.split(/\r?\n/);
  for (const line of lines) {
    const text = String(line || "").trim();
    if (!text || text.startsWith("#")) continue;
    const idx = text.indexOf("=");
    if (idx <= 0) continue;
    const key = text.slice(0, idx).trim();
    const value = text.slice(idx + 1).trim();
    if (!key) continue;
    env[key] = value;
  }
  return {
    method: launch.method,
    executablePath: profileExecutablePath(profile),
    lutrisTarget: launch.lutrisTarget,
    wineCommand: launch.wineCommand,
    wineArgs: launch.wineArgs,
    customCommand: launch.customCommand,
    customArgs: launch.customArgs,
    workingDir: launch.workingDir,
    clearWdb: launch.clearWdb,
    env,
  };
}

export function defaultProfileWowDir() {
  return localStorage.getItem(WOW_KEY) || "";
}

export function makeDefaultProfile() {
  return {
    id: "default",
    name: "WoW1",
    wowDir: defaultProfileWowDir(),
    launch: defaultLaunchConfig(),
    likesTurtles: false,
  };
}

export function persistProfiles() {
  localStorage.setItem(PROFILES_KEY, JSON.stringify(state.profiles));
  if (state.activeProfileId) {
    localStorage.setItem(ACTIVE_PROFILE_KEY, state.activeProfileId);
  } else {
    localStorage.removeItem(ACTIVE_PROFILE_KEY);
  }
}

export function activeProfile() {
  return state.profiles.find((p) => p.id === state.activeProfileId) || state.profiles[0] || null;
}

export function readProjectViewByProfile() {
  try {
    const raw = localStorage.getItem(PROJECT_VIEW_BY_PROFILE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    const out = {};
    for (const [key, value] of Object.entries(parsed)) {
      out[normalizeProfileId(key)] = normalizeProjectView(value);
    }
    return out;
  } catch (_) {
    return {};
  }
}

export function persistProjectViewByProfile() {
  localStorage.setItem(PROJECT_VIEW_BY_PROFILE_KEY, JSON.stringify(state.projectViewByProfile));
}

export function syncProjectViewFromActiveProfile() {
  const profile = activeProfile();
  if (!profile) {
    state.projectView = "mods";
    return;
  }
  const profileId = normalizeProfileId(profile.id);
  state.projectView = normalizeProjectView(state.projectViewByProfile[profileId] || "mods");
}

export function setProjectView(view, { persist = true } = {}) {
  const normalized = normalizeProjectView(view);
  state.projectView = normalized;
  const profile = activeProfile();
  if (persist && profile) {
    const profileId = normalizeProfileId(profile.id);
    state.projectViewByProfile[profileId] = normalized;
    persistProjectViewByProfile();
  }
  _render();
}

export function getProfileById(profileId) {
  const id = normalizeProfileId(profileId);
  return state.profiles.find((p) => p.id === id) || null;
}

export function ensureActiveProfile() {
  const profile = activeProfile();
  if (!profile) {
    log("ERROR: No WoW instance selected. Add one in Options.");
    return null;
  }
  return profile;
}

export function readWowDir() {
  const profile = activeProfile();
  return profileWowDir(profile);
}

export function currentWowDirStrict() {
  const profile = ensureActiveProfile();
  if (!profile) return null;
  const wowDir = readWowDir();
  if (!wowDir) {
    log(`ERROR: WoW directory is empty for ${profile.name || "active instance"}.`);
    return null;
  }
  return wowDir;
}

export function installOptions(overrides = {}) {
  return {
    useSymlinks: $("optSymlinks").checked,
    setXattrComment: $("optXattr").checked,
    replaceAddonConflicts: false,
    cacheKeepVersions: state.cacheKeepVersions,
    ...overrides,
  };
}

export async function setBackendActiveProfile() {
  const profile = activeProfile();
  if (!profile) return;
  const profileId = normalizeProfileId(profile.id);
  try {
    const normalized = await safeInvoke(
      "wuddle_set_active_profile",
      { profileId },
      { timeoutMs: 5000 },
    );
    state.activeProfileId = normalizeProfileId(normalized || profileId);
  } catch (e) {
    log(`ERROR profile: ${e.message}`);
  }
}

export function saveProfileName(profileId, value) {
  const profile = getProfileById(profileId);
  if (!profile) return;
  profile.name = value.trim() || "WoW";
  persistProfiles();
  renderProfileTabs();
}

export function saveProfileWowDir(profileId, value) {
  const profile = getProfileById(profileId);
  if (!profile) return;
  profile.wowDir = value.trim();
  if (profile.id === state.activeProfileId) {
    localStorage.setItem(WOW_KEY, profile.wowDir);
  }
  persistProfiles();
}

export function clearInstanceSettingsDraft() {
  state.instanceSettingsDraft = null;
}

export function renderInstanceList() {
  const host = $("instanceList");
  host.innerHTML = "";

  if (!state.profiles.length) {
    const empty = document.createElement("div");
    empty.className = "instance-empty hint";
    empty.textContent =
      "No instances configured yet. Add an instance, then choose the game executable path.";
    host.appendChild(empty);
    return;
  }

  for (const profile of state.profiles) {
    const card = document.createElement("div");
    card.className = `instance-card${profile.id === state.activeProfileId ? " active" : ""}`;
    card.addEventListener("click", () => {
      openInstanceSettingsDialog(profile);
    });

    const head = document.createElement("div");
    head.className = "instance-card-head";
    const title = document.createElement("div");
    title.className = "instance-card-title";
    title.textContent = profile.name || "WoW";
    head.appendChild(title);
    if (profile.id === state.activeProfileId) {
      const badge = document.createElement("span");
      badge.className = "stat-pill";
      badge.textContent = "Active";
      head.appendChild(badge);
    }

    const path = document.createElement("div");
    path.className = "instance-card-path";
    path.textContent = profile.wowDir || "No WoW path configured";
    path.title = profile.wowDir || "";

    const launch = document.createElement("div");
    launch.className = "instance-card-launch";
    launch.textContent = `Launch: ${launchSummary(profile)}${profileLikesTurtles(profile) ? " • turtles on" : ""}`;

    const actions = document.createElement("div");
    actions.className = "instance-actions";
    const leftActions = document.createElement("div");
    leftActions.className = "instance-actions-left";
    const rightActions = document.createElement("div");
    rightActions.className = "instance-actions-right";

    const activateBtn = document.createElement("button");
    activateBtn.className = "btn";
    activateBtn.textContent = profile.id === state.activeProfileId ? "Active" : "Switch";
    activateBtn.disabled = profile.id === state.activeProfileId;
    activateBtn.addEventListener("click", async (ev) => {
      ev.stopPropagation();
      await selectProfile(profile.id);
    });

    const removeBtn = document.createElement("button");
    removeBtn.className = "btn danger";
    removeBtn.textContent = "Remove";
    removeBtn.addEventListener("click", (ev) => {
      ev.stopPropagation();
      openRemoveInstanceDialog(profile);
    });

    leftActions.appendChild(activateBtn);
    rightActions.appendChild(removeBtn);
    actions.appendChild(leftActions);
    actions.appendChild(rightActions);

    card.appendChild(head);
    card.appendChild(path);
    card.appendChild(launch);
    card.appendChild(actions);
    host.appendChild(card);
  }
}

export function renderProfileTabs() {
  const host = $("profilePickerHost");
  if (!host) return;
  host.innerHTML = "";

  const divider = $("profilePickerDivider");
  if (state.profiles.length <= 1) {
    host.classList.add("hidden");
    if (divider) divider.classList.add("hidden");
    return;
  }
  host.classList.remove("hidden");
  if (divider) divider.classList.remove("hidden");

  const menu = document.createElement("div");
  menu.id = "profilePicker";
  menu.className = "profile-menu";

  const summary = document.createElement("button");
  summary.type = "button";
  summary.className = "profile-picker profile-menu-btn";
  const selected = activeProfile();
  summary.textContent = selected ? (selected.name || "WoW") : "No instances configured";
  summary.title = selected?.wowDir || "";
  menu.appendChild(summary);

  const pop = document.createElement("div");
  pop.className = "profile-menu-pop";

  if (!state.profiles.length) {
    menu.classList.add("disabled");
    const item = document.createElement("button");
    item.type = "button";
    item.className = "menu-item profile-menu-item";
    item.textContent = "No instances configured";
    item.disabled = true;
    pop.appendChild(item);
  } else {
    for (const profile of state.profiles) {
      const item = document.createElement("button");
      item.type = "button";
      item.className = `menu-item profile-menu-item${
        profile.id === state.activeProfileId ? " active" : ""
      }`;
      item.textContent = profile.name || "WoW";
      item.title = profile.wowDir || "";
      item.addEventListener("click", async (ev) => {
        ev.preventDefault();
        menu.classList.remove("open");
        if (profile.id === state.activeProfileId) return;
        await selectProfile(profile.id);
      });
      pop.appendChild(item);
    }
  }

  summary.addEventListener("click", (ev) => {
    if (menu.classList.contains("disabled")) {
      ev.preventDefault();
      return;
    }
    ev.stopPropagation();
    menu.classList.toggle("open");
  });

  menu.appendChild(pop);
  host.appendChild(menu);
}

export function renderInstanceSettingsLaunchFields(method) {
  const current = String(method || "auto").toLowerCase();
  $("instanceSettingsLaunchAuto").classList.toggle("hidden", current !== "auto");
  $("instanceSettingsLaunchLutris").classList.toggle("hidden", current !== "lutris");
  $("instanceSettingsLaunchWine").classList.toggle("hidden", current !== "wine");
  $("instanceSettingsLaunchCustom").classList.toggle("hidden", current !== "custom");
}

export function openInstanceSettingsDialog(profile, options = {}) {
  if (!profile) return;
  const launch = profileLaunch(profile);
  state.instanceSettingsDraft = {
    isNew: !!options.isNew,
    id: normalizeProfileId(profile.id || ""),
  };
  $("instanceSettingsId").value = profile.id;
  $("instanceSettingsName").value = profile.name || "";
  $("instanceSettingsPath").value = rawWowPath(profile);
  $("instanceSettingsLaunchMethod").value = launch.method;
  syncThemedSelect($("instanceSettingsLaunchMethod"));
  $("instanceSettingsLutrisTarget").value = launch.lutrisTarget || "";
  $("instanceSettingsWineCommand").value = launch.wineCommand || "wine";
  $("instanceSettingsWineArgs").value = launch.wineArgs || "";
  $("instanceSettingsCustomCommand").value = launch.customCommand || "";
  $("instanceSettingsCustomArgs").value = launch.customArgs || "";
  $("instanceSettingsLikeTurtles").checked = profileLikesTurtles(profile);
  $("instanceSettingsClearWdb").checked = !!launch.clearWdb;
  $("instanceSettingsWorkingDir").value = launch.workingDir || "";
  $("instanceSettingsEnv").value = launch.envText || "";
  const advancedDetails = $("instanceSettingsAdvanced");
  if (advancedDetails) {
    advancedDetails.open = !!(launch.workingDir || launch.envText);
  }
  $("btnInstanceSettingsOpenPath").disabled = !profileWowDir(profile);
  renderInstanceSettingsLaunchFields(launch.method);
  $("dlgInstanceSettings").showModal();
}

export function saveInstanceSettingsFromDialog() {
  const id = normalizeProfileId($("instanceSettingsId").value || "");
  const draft = state.instanceSettingsDraft;
  let profile = getProfileById(id);
  const creatingNew = !!draft?.isNew && draft.id === id && !profile;
  if (!profile && !creatingNew) return false;

  const nextName = String($("instanceSettingsName").value || "").trim() || profile?.name || "WoW";
  const nextPath = String($("instanceSettingsPath").value || "").trim();
  const launch = normalizeLaunchConfig({
    method: $("instanceSettingsLaunchMethod").value,
    lutrisTarget: $("instanceSettingsLutrisTarget").value,
    wineCommand: $("instanceSettingsWineCommand").value,
    wineArgs: $("instanceSettingsWineArgs").value,
    customCommand: $("instanceSettingsCustomCommand").value,
    customArgs: $("instanceSettingsCustomArgs").value,
    workingDir: $("instanceSettingsWorkingDir").value,
    envText: $("instanceSettingsEnv").value,
    clearWdb: !!$("instanceSettingsClearWdb")?.checked,
  });
  const likesTurtles = !!$("instanceSettingsLikeTurtles")?.checked;

  if (!profile) {
    profile = { id, name: nextName, wowDir: nextPath, launch, likesTurtles };
    state.profiles.push(profile);
    state.activeProfileId = id;
    if (!state.projectViewByProfile[id]) {
      state.projectViewByProfile[id] = "mods";
    }
    syncProjectViewFromActiveProfile();
    persistProjectViewByProfile();
    log(`Created instance ${nextName}.`);
  } else {
    profile.name = nextName;
    profile.wowDir = nextPath;
    profile.launch = launch;
    profile.likesTurtles = likesTurtles;
  }

  if (profile.id === state.activeProfileId) {
    localStorage.setItem(WOW_KEY, profile.wowDir || "");
  }
  persistProfiles();
  renderProfileTabs();
  renderInstanceList();
  _render();
  clearInstanceSettingsDraft();
  return true;
}

export function openRemoveInstanceDialog(profile) {
  state.removeTargetProfile = profile;
  const profileLabel = profile?.name || "WoW";
  const wowDir = profile?.wowDir ? ` - ${profile.wowDir}` : "";
  $("removeInstanceName").textContent = `${profileLabel}${wowDir}`;
  $("removeInstanceLocalFiles").checked = false;
  $("dlgRemoveInstance").showModal();
}

export async function confirmRemoveInstance() {
  const profile = state.removeTargetProfile;
  if (!profile) {
    $("dlgRemoveInstance").close();
    return;
  }
  const removeLocalFiles = $("removeInstanceLocalFiles").checked;

  try {
    const wowDir = removeLocalFiles ? (profile.wowDir || "").trim() : null;
    if (removeLocalFiles && !wowDir) {
      log("ERROR remove instance: Path is required to remove installed mods.");
      return;
    }
    const msg = await safeInvoke("wuddle_delete_profile", {
      profileId: profile.id,
      removeLocalFiles,
      wowDir,
    });
    await removeInstance(profile.id);
    log(msg || `Removed instance ${profile.name || profile.id}.`);
  } finally {
    state.removeTargetProfile = null;
    $("dlgRemoveInstance").close();
  }
}

export async function selectProfile(profileId) {
  const id = normalizeProfileId(profileId);
  if (!state.profiles.some((p) => p.id === id)) return;
  state.activeProfileId = id;
  state.lastCheckedAt = null;
  state.branchOptionsByRepoId.clear();
  state.branchOptionsLoading.clear();
  syncProjectViewFromActiveProfile();
  const selected = activeProfile();
  if (selected?.wowDir) {
    localStorage.setItem(WOW_KEY, selected.wowDir);
  }
  persistProfiles();
  renderProfileTabs();
  renderInstanceList();
  _setTab("home");
  await setBackendActiveProfile();
  await _refreshAll();
}

export async function addInstance() {
  const baseName = `WoW${state.profiles.length + 1}`;
  const name = baseName;
  const idBase = normalizeProfileId(name);
  let id = idBase;
  let n = 2;
  while (state.profiles.some((p) => p.id === id)) {
    id = `${idBase}-${n++}`;
  }
  openInstanceSettingsDialog(
    { id, name, wowDir: "", launch: defaultLaunchConfig(), likesTurtles: false },
    { isNew: true },
  );
}

export async function removeInstance(profileId) {
  const id = normalizeProfileId(profileId);
  const before = state.profiles.length;
  state.profiles = state.profiles.filter((p) => p.id !== id);
  if (state.profiles.length === before) return;
  delete state.projectViewByProfile[id];

  if (state.activeProfileId === id) {
    state.activeProfileId = state.profiles[0]?.id || "";
  }
  state.lastCheckedAt = null;
  state.branchOptionsByRepoId.clear();
  state.branchOptionsLoading.clear();
  syncProjectViewFromActiveProfile();
  persistProjectViewByProfile();
  if (!state.profiles.length) {
    localStorage.removeItem(WOW_KEY);
  } else if (state.activeProfileId) {
    const active = activeProfile();
    if (active?.wowDir) localStorage.setItem(WOW_KEY, active.wowDir);
  }
  persistProfiles();
  renderProfileTabs();
  renderInstanceList();
  await setBackendActiveProfile();
  await _refreshAll();
}

export async function pickWowDirForProfile(profileId) {
  try {
    const res = await safeInvoke(
      "plugin:dialog|open",
      {
        options: {
          directory: true,
          multiple: false,
          title: "Select WoW folder",
        },
      },
      { timeoutMs: 0 },
    );

    let picked = null;
    if (Array.isArray(res)) picked = res[0];
    else picked = res;

    if (!picked) return;

    saveProfileWowDir(profileId, String(picked));
    renderInstanceList();
    if (state.activeProfileId === normalizeProfileId(profileId)) {
      await _refreshAll();
    }
  } catch (e) {
    log(`ERROR picker: ${e.message}`);
  }
}
