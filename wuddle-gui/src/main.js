const invoke = window.__TAURI__?.core?.invoke;
if (!invoke) {
  document.body.innerHTML =
    "<pre style='padding:16px'>ERROR: window.__TAURI__ missing. Start with: npm run tauri dev</pre>";
  throw new Error("window.__TAURI__ missing");
}

const $ = (id) => document.getElementById(id);

const WOW_KEY = "wuddle.wow_dir";
const PROFILES_KEY = "wuddle.profiles";
const ACTIVE_PROFILE_KEY = "wuddle.profile.active";
const TAB_KEY = "wuddle.tab";
const PROJECT_VIEW_BY_PROFILE_KEY = "wuddle.project_view.by_profile";
const OPT_SYMLINKS_KEY = "wuddle.opt.symlinks";
const OPT_XATTR_KEY = "wuddle.opt.xattr";
const OPT_CLOCK12_KEY = "wuddle.opt.clock12";
const OPT_THEME_KEY = "wuddle.opt.theme";
const OPT_FRIZ_FONT_KEY = "wuddle.opt.frizfont";
const OPT_AUTOCHECK_KEY = "wuddle.opt.autocheck";
const OPT_AUTOCHECK_MINUTES_KEY = "wuddle.opt.autocheck.minutes";
const LOG_WRAP_KEY = "wuddle.log.wrap";
const LOG_AUTOSCROLL_KEY = "wuddle.log.autoscroll";
const LOG_LEVEL_KEY = "wuddle.log.level";
const WUDDLE_REPO_URL = "https://github.com/ZythDr/Wuddle";
const WUDDLE_RELEASES_URL = "https://github.com/ZythDr/Wuddle/releases";
const WUDDLE_RELEASES_API_URL = "https://api.github.com/repos/ZythDr/Wuddle/releases/latest";
const MAX_PARALLEL_UPDATES = 5;
const DEFAULT_THEME_ID = "cata";
const DEFAULT_USE_FRIZ_FONT = true;
const DEFAULT_AUTO_CHECK_ENABLED = false;
const DEFAULT_AUTO_CHECK_MINUTES = 30;
const MIN_AUTO_CHECK_MINUTES = 1;
const MAX_AUTO_CHECK_MINUTES = 240;
const SELF_UPDATE_POLL_MINUTES = 30;
const SUPPORTED_THEMES = new Set(["cata", "obsidian", "emerald", "ashen", "wowui"]);

const state = {
  repos: [],
  plans: [],
  planByRepoId: new Map(),
  branchOptionsByRepoId: new Map(),
  branchOptionsLoading: new Set(),
  openMenuRepoId: null,
  tab: "home",
  pending: 0,
  refreshInFlight: null,
  removeTargetRepo: null,
  removeTargetProfile: null,
  githubAuth: null,
  initialAutoCheckDone: false,
  loggedNoTokenAutoSkip: false,
  filter: "all",
  projectSearchQuery: "",
  sortKey: "name",
  sortDir: "asc",
  lastCheckedAt: null,
  clock12: false,
  theme: DEFAULT_THEME_ID,
  useFrizFont: DEFAULT_USE_FRIZ_FONT,
  autoCheckEnabled: DEFAULT_AUTO_CHECK_ENABLED,
  autoCheckMinutes: DEFAULT_AUTO_CHECK_MINUTES,
  autoCheckTimerId: null,
  lastUpdateNotifyKey: "",
  lastSelfUpdateNotifyVersion: "",
  nextSelfUpdatePollAt: 0,
  logLines: [],
  logLevel: "all",
  logQuery: "",
  logAutoScroll: true,
  logWrap: false,
  profiles: [],
  activeProfileId: "default",
  projectViewByProfile: {},
  projectView: "mods",
  authHealthSeenSession: false,
  authHealthActiveIssue: "",
  presetExpanded: new Set(),
  aboutInfo: null,
  aboutLoaded: false,
  aboutRefreshedAt: null,
  aboutLatestVersion: null,
  aboutSelfUpdate: null,
  aboutSelfUpdateBusy: false,
  launchDiagnostics: null,
};

const themedSelectBindings = new WeakMap();

const CURATED_MOD_PRESETS = [
  {
    id: "vanillafixes",
    name: "VanillaFixes",
    url: "https://github.com/hannesmann/vanillafixes",
    mode: "auto",
    description:
      "A client modification for World of Warcraft 1.6.1-1.12.1 to eliminate stutter and animation lag.",
    longDescription:
      "A client modification for World of Warcraft 1.6.1-1.12.1 to eliminate stutter and animation lag.\nVanillaFixes also acts as a launcher (start game via VanillaFixes.exe instead of Wow.exe) and DLL mod loader which loads DLL files listed in dlls.txt found in the WoW install directory.",
    categories: ["Performance"],
    recommended: true,
  },
  {
    id: "interact",
    name: "Interact",
    url: "https://github.com/lookino/Interact",
    mode: "auto",
    description:
      "Legacy WoW client mod that brings Dragonflight-style interact key support to Vanilla.",
    longDescription:
      "Legacy WoW client mod for 1.12 that brings a Dragonflight-style interact key workflow to Vanilla, reducing click friction and improving moment-to-moment interaction quality.",
    categories: ["QoL"],
    recommended: false,
  },
  {
    id: "unitxp_sp3",
    name: "UnitXP_SP3",
    url: "https://codeberg.org/konaka/UnitXP_SP3",
    mode: "auto",
    description:
      "Adds camera offset, proper nameplates, improved tab-targeting, LoS/distance checks, and more.",
    longDescription:
      "Adds optional camera offset, proper nameplates (showing only with LoS), improved tab-targeting keybind behavior, LoS and distance checks in Lua, screenshot format options, network tweaks, background notifications, and additional QoL features.",
    categories: ["QoL", "API"],
    recommended: true,
  },
  {
    id: "nampower",
    name: "nampower",
    url: "https://gitea.com/avitasia/nampower",
    mode: "auto",
    description:
      "Reduces cast downtime caused by 1.12 client spell-completion delay, improving effective DPS.",
    longDescription:
      "Addresses a 1.12 client casting flow limitation where follow-up casts wait on round-trip completion feedback. The result is reduced cast downtime and better effective DPS, especially on higher-latency realm routes.",
    companionLinks: [
      {
        label: "nampowersettings",
        url: "https://gitea.com/avitasia/nampowersettings",
      },
    ],
    categories: ["API"],
    recommended: true,
  },
  {
    id: "superwow",
    name: "SuperWoW",
    url: "https://github.com/balakethelock/SuperWoW",
    mode: "auto",
    description:
      "Fixes 1.12.1 client bugs and expands addon API; required or beneficial for many addons.",
    longDescription:
      "Client mod for WoW 1.12.1 that fixes engine/client bugs and expands the Lua API used by addons. Some addons require SuperWoW directly, and many others gain improved functionality when it is present.",
    warning:
      "Known issue: SuperWoW will trigger antivirus false-positive alerts on Windows.",
    companionLinks: [
      {
        label: "SuperAPI",
        url: "https://github.com/balakethelock/SuperAPI",
      },
      {
        label: "SuperAPI_Castlib",
        url: "https://github.com/balakethelock/SuperAPI_Castlib",
      },
    ],
    expandedNotes: [
      "SuperAPI improves compatibility with the default interface and adds a minimap icon for persistent mod settings.",
      "It exposes settings like autoloot, clickthrough corpses, GUID in combat log/events, adjustable FoV, enable background sound, uncapped sound channels, and targeting circle style.",
      "SuperAPI_Castlib adds default-style nameplate castbars. If you're using pfUI/shaguplates, you do not need this module.",
    ],
    categories: ["QoL", "API"],
    recommended: true,
  },
  {
    id: "dxvk_gplasync",
    name: "DXVK (GPLAsync fork)",
    url: "https://gitlab.com/Ph42oN/dxvk-gplasync",
    mode: "auto",
    description:
      "Vulkan translation layer for D3D 8/9/10/11; often improves FPS and smoothness in Vanilla WoW.",
    longDescription:
      "DXVK can massively improve performance in old Direct3D titles (including WoW 1.12) by using Vulkan. This fork includes Async + GPL options aimed at further reducing stutters. Async/GPL behavior is controlled through dxvk.conf, so users can keep default behavior if they prefer.",
    categories: ["Performance"],
    recommended: true,
  },
  {
    id: "perf_boost",
    name: "perf_boost",
    url: "https://gitea.com/avitasia/perf_boost",
    mode: "auto",
    description:
      "Performance optimization DLL for WoW 1.12.1 with advanced render-distance controls.",
    longDescription:
      "Performance-focused DLL for WoW 1.12.1 intended to improve FPS in crowded areas and raids. Uses advanced render-distance controls.",
    companionLinks: [
      {
        label: "PerfBoostSettings",
        url: "https://gitea.com/avitasia/PerfBoostSettings",
      },
    ],
    categories: ["Performance"],
    recommended: false,
  },
  {
    id: "vanillahelpers",
    name: "VanillaHelpers",
    url: "https://github.com/isfir/VanillaHelpers",
    mode: "auto",
    description:
      "Helper library for Vanilla WoW with file ops, minimap features, memory/texture upgrades, and morph tools.",
    longDescription:
      "Utility library for WoW 1.12 adding file read/write helpers, minimap blip customization, larger allocator capacity, higher-resolution texture/skin support, and character morph-related functionality.",
    categories: ["API", "Performance"],
    recommended: true,
  },
];

const PRESET_CATEGORY_CLASS = {
  qol: "cat-qol",
  api: "cat-api",
  performance: "cat-performance",
};

function normalizeProfileId(value) {
  const base = String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return base || "default";
}

function defaultProfileWowDir() {
  return localStorage.getItem(WOW_KEY) || "";
}

function makeDefaultProfile() {
  return {
    id: "default",
    name: "WoW1",
    wowDir: defaultProfileWowDir(),
    launch: defaultLaunchConfig(),
  };
}

function defaultLaunchConfig() {
  return {
    method: "auto",
    lutrisTarget: "",
    wineCommand: "wine",
    wineArgs: "",
    customCommand: "",
    customArgs: "",
    workingDir: "",
    envText: "",
  };
}

function normalizeLaunchConfig(raw) {
  const input = raw && typeof raw === "object" ? raw : {};
  const methodRaw = String(input.method || "auto").trim().toLowerCase();
  const method = new Set(["auto", "lutris", "wine", "custom"]).has(methodRaw)
    ? methodRaw
    : "auto";
  return {
    method,
    lutrisTarget: String(input.lutrisTarget || "").trim(),
    wineCommand: String(input.wineCommand || "wine").trim() || "wine",
    wineArgs: String(input.wineArgs || "").trim(),
    customCommand: String(input.customCommand || "").trim(),
    customArgs: String(input.customArgs || "").trim(),
    workingDir: String(input.workingDir || "").trim(),
    envText: String(input.envText || "").trim(),
  };
}

function profileLaunch(profile) {
  return normalizeLaunchConfig(profile?.launch);
}

function rawWowPath(profile) {
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

function profileExecutablePath(profile) {
  const raw = rawWowPath(profile);
  return isExePath(raw) ? raw : "";
}

function profileWowDir(profile) {
  const raw = rawWowPath(profile);
  if (!raw) return "";
  if (isExePath(raw)) return parentDirOfPath(raw);
  return raw;
}

function launchSummary(profile) {
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

function launchPayload(profile) {
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
    env,
  };
}

function persistProfiles() {
  localStorage.setItem(PROFILES_KEY, JSON.stringify(state.profiles));
  if (state.activeProfileId) {
    localStorage.setItem(ACTIVE_PROFILE_KEY, state.activeProfileId);
  } else {
    localStorage.removeItem(ACTIVE_PROFILE_KEY);
  }
}

function activeProfile() {
  return state.profiles.find((p) => p.id === state.activeProfileId) || state.profiles[0] || null;
}

function normalizeProjectView(value) {
  return String(value || "").toLowerCase() === "addons" ? "addons" : "mods";
}

function readProjectViewByProfile() {
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

function persistProjectViewByProfile() {
  localStorage.setItem(PROJECT_VIEW_BY_PROFILE_KEY, JSON.stringify(state.projectViewByProfile));
}

function syncProjectViewFromActiveProfile() {
  const profile = activeProfile();
  if (!profile) {
    state.projectView = "mods";
    return;
  }
  const profileId = normalizeProfileId(profile.id);
  state.projectView = normalizeProjectView(state.projectViewByProfile[profileId] || "mods");
}

function setProjectView(view, { persist = true } = {}) {
  const normalized = normalizeProjectView(view);
  state.projectView = normalized;
  const profile = activeProfile();
  if (persist && profile) {
    const profileId = normalizeProfileId(profile.id);
    state.projectViewByProfile[profileId] = normalized;
    persistProjectViewByProfile();
  }
  render();
}

function isAddonRepo(repo) {
  const mode = String(repo?.mode || "")
    .trim()
    .toLowerCase();
  return mode === "addon" || mode === "addon_git";
}

function reposForView(view) {
  const addonsView = normalizeProjectView(view) === "addons";
  return state.repos.filter((repo) => (addonsView ? isAddonRepo(repo) : !isAddonRepo(repo)));
}

function reposForCurrentView() {
  return reposForView(state.projectView);
}

function getProfileById(profileId) {
  const id = normalizeProfileId(profileId);
  return state.profiles.find((p) => p.id === id) || null;
}

function ensureActiveProfile() {
  const profile = activeProfile();
  if (!profile) {
    log("ERROR: No WoW instance selected. Add one in Options.");
    return null;
  }
  return profile;
}

function saveProfileName(profileId, value) {
  const profile = getProfileById(profileId);
  if (!profile) return;
  profile.name = value.trim() || "WoW";
  persistProfiles();
  renderProfileTabs();
}

function saveProfileWowDir(profileId, value) {
  const profile = getProfileById(profileId);
  if (!profile) return;
  profile.wowDir = value.trim();
  if (profile.id === state.activeProfileId) {
    localStorage.setItem(WOW_KEY, profile.wowDir);
  }
  persistProfiles();
}

function renderInstanceList() {
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
    launch.textContent = `Launch: ${launchSummary(profile)}`;

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

function renderProfileTabs() {
  const host = $("profilePickerHost");
  if (!host) return;
  host.innerHTML = "";

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

function closeThemedSelectMenus(except = null) {
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

function syncThemedSelect(select) {
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

function rebuildThemedSelect(select) {
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

function ensureThemedSelect(select, extraClass = "") {
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

function repoKeyFromUrl(url) {
  try {
    const parsed = new URL(String(url ?? "").trim());
    const segs = parsed.pathname
      .split("/")
      .map((s) => s.trim())
      .filter(Boolean);
    if (segs.length < 2) return null;
    const owner = segs[0].toLowerCase();
    const name = segs[1].replace(/\.git$/i, "").toLowerCase();
    return `${parsed.hostname.toLowerCase()}|${owner}|${name}`;
  } catch (_) {
    return null;
  }
}

function repoKeyFromRepo(repo) {
  let host = String(repo?.host ?? "").trim().toLowerCase();
  if (!host) {
    try {
      host = new URL(String(repo?.url ?? "").trim()).hostname.toLowerCase();
    } catch (_) {
      host = "";
    }
  }
  const owner = String(repo?.owner ?? "").trim().toLowerCase();
  const name = String(repo?.name ?? "").trim().replace(/\.git$/i, "").toLowerCase();
  if (!host || !owner || !name) return null;
  return `${host}|${owner}|${name}`;
}

function isPresetInstalled(preset) {
  const presetKey = repoKeyFromUrl(preset?.url);
  if (!presetKey) return false;
  return state.repos.some((repo) => repoKeyFromRepo(repo) === presetKey);
}

function isPresetExpanded(preset) {
  return state.presetExpanded.has(preset.id);
}

function togglePresetExpanded(preset) {
  if (isPresetExpanded(preset)) state.presetExpanded.delete(preset.id);
  else state.presetExpanded.add(preset.id);
}

function isSuperWoWUrl(url) {
  const text = String(url || "").trim();
  if (!text) return false;
  try {
    const parsed = new URL(text);
    const host = parsed.hostname.toLowerCase();
    const path = parsed.pathname.toLowerCase().replace(/\/+$/, "");
    if (host.endsWith("github.com") && path.includes("/balakethelock/superwow")) return true;
    return false;
  } catch (_) {
    return /balakethelock\/superwow/i.test(text);
  }
}

async function confirmSuperWoWRisk() {
  const message =
    "SuperWoW is known to trigger false-positives as malware in many antivirus products.\n\nInstalling SuperWoW can trigger AV warnings that reference Wuddle.exe because Wuddle performs the download/install.\n\nDo you want to continue adding SuperWoW?";
  const dlg = $("dlgSuperwowRisk");
  if (!dlg || typeof dlg.showModal !== "function") {
    return window.confirm(message);
  }
  if (dlg.open) dlg.close("cancel");
  dlg.returnValue = "cancel";
  return await new Promise((resolve) => {
    dlg.addEventListener(
      "close",
      () => {
        resolve(dlg.returnValue === "ok");
      },
      { once: true },
    );
    dlg.showModal();
  });
}

const ADDON_CONFLICT_PREFIX = "ADDON_CONFLICT:";

function parseAddonConflictError(message) {
  const text = String(message || "").trim();
  if (!text.startsWith(ADDON_CONFLICT_PREFIX)) return null;
  const details = text.slice(ADDON_CONFLICT_PREFIX.length).trim();
  return details || "Existing addon files were found in the destination folder.";
}

async function confirmAddonConflict(repo, details) {
  const name = `${repo.owner}/${repo.name}`;
  return window.confirm(
    `Addon install conflict for ${name}.\n\n${details}\n\nClick OK to delete conflicting addon folders and continue, or Cancel to keep existing files and stop this install.`,
  );
}

function renderAddPresets() {
  const host = $("addPresetList");
  if (!host) return;
  host.innerHTML = "";
  const hasProfile = !!activeProfile();

  for (const preset of CURATED_MOD_PRESETS) {
    const installed = !preset.placeholder && isPresetInstalled(preset);
    const expanded = isPresetExpanded(preset);
    const longDescription = String(preset.longDescription || "").trim();
    const shortDescription = String(preset.description || "").trim();
    const warning = String(preset.warning || "").trim();
    const canExpand = !preset.placeholder && !!longDescription && longDescription !== shortDescription;
    const card = document.createElement("div");
    card.className = `preset-card${preset.placeholder ? " placeholder" : ""}${installed ? " installed" : ""}${expanded ? " expanded" : ""}${canExpand ? " can-expand" : ""}`;
    if (canExpand) {
      card.addEventListener("click", (ev) => {
        if (!(ev.target instanceof Element)) return;
        if (ev.target.closest(".preset-actions")) return;
        if (ev.target.closest(".preset-title-link")) return;
        if (ev.target.closest(".preset-inline-link")) return;
        togglePresetExpanded(preset);
        renderAddPresets();
      });
    }

    const head = document.createElement("div");
    head.className = "preset-head";

    if (!preset.placeholder && preset.url) {
      const titleLink = document.createElement("button");
      titleLink.type = "button";
      titleLink.className = "preset-title-link";
      titleLink.textContent = preset.name;
      titleLink.addEventListener("click", async (ev) => {
        ev.stopPropagation();
        await openUrl(preset.url);
      });
      head.appendChild(titleLink);
    } else {
      const title = document.createElement("div");
      title.className = "preset-title";
      title.textContent = preset.name;
      head.appendChild(title);
    }

    const flags = document.createElement("div");
    flags.className = "preset-flags";
    if (!preset.placeholder && preset.recommended) {
      const recommendedTag = document.createElement("span");
      recommendedTag.className = "preset-flag recommended";
      recommendedTag.textContent = "Recommended";
      flags.appendChild(recommendedTag);
    }
    if (warning) {
      const warningTag = document.createElement("span");
      warningTag.className = "preset-flag warning";
      warningTag.textContent = "AV false-positive";
      warningTag.title = warning;
      flags.appendChild(warningTag);
    }
    if (!preset.placeholder) {
      const categories = Array.isArray(preset.categories) ? preset.categories : [];
      for (const rawCategory of categories) {
        const category = String(rawCategory || "").trim();
        if (!category) continue;
        const key = category.toLowerCase();
        const cls = PRESET_CATEGORY_CLASS[key] || "";
        const tag = document.createElement("span");
        tag.className = `preset-flag category${cls ? ` ${cls}` : ""}`;
        tag.textContent = category;
        flags.appendChild(tag);
      }
    }
    if (flags.childElementCount > 0) {
      head.appendChild(flags);
    }

    const desc = document.createElement("div");
    desc.className = "preset-desc";
    const descText = document.createElement("div");
    descText.className = "preset-desc-text";
    descText.textContent = expanded && longDescription ? longDescription : shortDescription;
    desc.appendChild(descText);

    const companionLinks = Array.isArray(preset.companionLinks)
      ? preset.companionLinks
      : [];

    if (expanded && Array.isArray(preset.expandedNotes) && preset.expandedNotes.length > 0) {
      const notes = document.createElement("div");
      notes.className = "preset-desc-notes";
      for (const rawLine of preset.expandedNotes) {
        const line = String(rawLine || "").trim();
        if (!line) continue;
        const row = document.createElement("div");
        row.className = "preset-desc-note";
        row.textContent = `• ${line}`;
        notes.appendChild(row);
      }
      if (notes.childElementCount > 0) {
        desc.appendChild(notes);
      }
    }

    if (!preset.placeholder && companionLinks.length > 0) {
      const linksWrap = document.createElement("div");
      linksWrap.className = "preset-desc-links";
      const label = document.createElement("span");
      label.textContent = "Companion addons:";
      linksWrap.appendChild(label);
      companionLinks.forEach((entry, idx) => {
        const url = String(entry?.url || "").trim();
        const text = String(entry?.label || "").trim();
        if (!url || !text) return;
        if (idx > 0) {
          const sep = document.createElement("span");
          sep.textContent = "•";
          sep.className = "preset-desc-sep";
          linksWrap.appendChild(sep);
        }
        const linkBtn = document.createElement("button");
        linkBtn.type = "button";
        linkBtn.className = "preset-inline-link";
        linkBtn.textContent = text;
        linkBtn.title = url;
        linkBtn.addEventListener("click", async (ev) => {
          ev.stopPropagation();
          await openUrl(url);
        });
        linksWrap.appendChild(linkBtn);
      });
      if (linksWrap.childElementCount > 1) {
        desc.appendChild(linksWrap);
      }
    }

    const actions = document.createElement("div");
    actions.className = "preset-actions";
    const addBtn = document.createElement("button");
    addBtn.type = "button";
    addBtn.className = "btn";
    if (preset.placeholder || !preset.url) {
      addBtn.textContent = "Coming soon";
      addBtn.disabled = true;
    } else if (installed) {
      addBtn.textContent = "Installed";
      addBtn.classList.add("installed-state");
      addBtn.disabled = true;
    } else if (!hasProfile) {
      addBtn.textContent = "Add instance first";
      addBtn.disabled = true;
    } else {
      addBtn.textContent = "Add";
      addBtn.addEventListener("click", async (ev) => {
        ev.stopPropagation();
        const ok = await addRepo(preset.url, preset.mode, preset.name);
        if (ok) renderAddPresets();
      });
    }
    addBtn.addEventListener("click", (ev) => ev.stopPropagation());
    actions.appendChild(addBtn);

    card.appendChild(head);
    card.appendChild(desc);
    card.appendChild(actions);
    host.appendChild(card);
  }
}

function applyAddDialogContext() {
  const addons = state.projectView === "addons";
  const addButton = $("btnAddOpen");
  const addTitle = $("addDialogTitle");
  const addHint = $("addDialogHint");
  const quickAddField = $("quickAddField");
  const repoUrlLabel = $("addRepoUrlLabel");
  const modeSelect = $("mode");

  if (addButton) {
    addButton.textContent = addons ? "＋ Add New Addon" : "＋ Add New Mod";
  }
  if (addTitle) {
    addTitle.textContent = addons ? "Add an addon repo" : "Add a repo";
  }
  if (addHint) {
    addHint.textContent = addons
      ? "Add a Git repo URL for an addon. Wuddle will clone/pull it for this instance."
      : "Quick-add from the mods listed, or add your own Git repo URL below.";
  }
  if (repoUrlLabel) {
    repoUrlLabel.textContent = addons ? "Addon Repo URL" : "Repo URL";
  }
  if (quickAddField) {
    quickAddField.classList.toggle("hidden", addons);
  }
  if (modeSelect && addons) {
    modeSelect.value = "addon_git";
  } else if (modeSelect && modeSelect.value === "addon_git") {
    modeSelect.value = "auto";
  }
  if (modeSelect) {
    syncThemedSelect(modeSelect);
  }
}

async function setBackendActiveProfile() {
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

function formatTime(value) {
  if (!value) return "never";
  const d = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(d.getTime())) return "never";
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: state.clock12,
  }).format(d);
}

function log(line) {
  const text = String(line ?? "");
  const level = /(^|\s)ERROR\b/i.test(text) ? "error" : "info";
  state.logLines.push({
    at: new Date(),
    text,
    level,
  });
  if (state.logLines.length > 4000) {
    state.logLines.shift();
  }
  renderLog();
}

function logOperationResult(result) {
  if (typeof result === "string") {
    log(result);
    return result;
  }
  if (!result || typeof result !== "object") {
    const msg = String(result ?? "");
    if (msg) log(msg);
    return msg;
  }
  const steps = Array.isArray(result.steps) ? result.steps : [];
  for (const step of steps) {
    if (step) log(step);
  }
  const msg = String(result.message ?? "");
  if (msg) log(msg);
  return msg;
}

function filteredLogLines() {
  const query = state.logQuery.trim().toLowerCase();
  return state.logLines.filter((entry) => {
    if (state.logLevel !== "all" && entry.level !== state.logLevel) return false;
    if (!query) return true;
    return entry.text.toLowerCase().includes(query);
  });
}

function renderLogLevelButtons() {
  document.querySelectorAll(".filter-btn[data-log-level]").forEach((btn) => {
    const level = btn.getAttribute("data-log-level");
    btn.classList.toggle("active", level === state.logLevel);
  });
}

function renderLog() {
  const el = $("log");
  const lines = filteredLogLines();
  el.textContent = lines.map((entry) => `[${formatTime(entry.at)}] ${entry.text}`).join("\n");
  el.classList.toggle("wrap", state.logWrap);
  renderLogLevelButtons();
  if (state.logAutoScroll) {
    el.scrollTop = el.scrollHeight;
  }
}

async function safeInvoke(cmd, args = {}, opts = {}) {
  const timeoutMs =
    typeof opts.timeoutMs === "number" && opts.timeoutMs > 0 ? opts.timeoutMs : null;
  let timer = null;
  try {
    if (!timeoutMs) {
      return await invoke(cmd, args);
    }
    const timeout = new Promise((_, reject) => {
      timer = window.setTimeout(() => {
        reject(new Error(`Request timed out (${cmd})`));
      }, timeoutMs);
    });
    return await Promise.race([invoke(cmd, args), timeout]);
  } catch (e) {
    const msg = typeof e === "string" ? e : (e?.message ?? JSON.stringify(e));
    throw new Error(msg);
  } finally {
    if (timer !== null) {
      window.clearTimeout(timer);
    }
  }
}

function renderBusy() {
  const busy = state.pending > 0;
  document.body.classList.toggle("busy", busy);
  $("busyIndicator").classList.toggle("hidden", !busy);
}

function normalizeThemeId(raw) {
  const value = String(raw || "")
    .trim()
    .toLowerCase();
  if (value === "dark") return DEFAULT_THEME_ID; // migration from old theme key usage
  return SUPPORTED_THEMES.has(value) ? value : DEFAULT_THEME_ID;
}

async function withBusy(work) {
  state.pending += 1;
  renderBusy();
  try {
    return await work();
  } finally {
    state.pending = Math.max(0, state.pending - 1);
    renderBusy();
  }
}

function setTheme(themeId = state.theme) {
  const next = normalizeThemeId(themeId);
  state.theme = next;
  document.documentElement.setAttribute("data-theme", next);
}

function setUiFontStyle(enabled = state.useFrizFont) {
  const useFriz = !!enabled;
  state.useFrizFont = useFriz;
  document.documentElement.setAttribute("data-font-style", useFriz ? "friz" : "default");
}

function aboutValue(value, fallback = "Unknown") {
  if (value === null || value === undefined) return fallback;
  const text = String(value).trim();
  return text || fallback;
}

function setAboutStatus(message, kind = "") {
  const status = $("aboutStatus");
  status.classList.remove("status-ok", "status-warn");
  if (kind) status.classList.add(kind);
  status.textContent = message;
}

function renderAboutInfo() {
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

function renderAboutUpdateAction() {
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

function updateCounts() {
  const mods = reposForView("mods").filter((repo) => canUpdateRepo(repo)).length;
  const addons = reposForView("addons").filter((repo) => canUpdateRepo(repo)).length;
  return { mods, addons, total: mods + addons };
}

function normalizeAutoCheckMinutes(value) {
  const num = Number(value);
  if (!Number.isFinite(num)) return DEFAULT_AUTO_CHECK_MINUTES;
  return Math.max(
    MIN_AUTO_CHECK_MINUTES,
    Math.min(MAX_AUTO_CHECK_MINUTES, Math.floor(num)),
  );
}

function renderAutoCheckSettings() {
  const enabled = !!state.autoCheckEnabled;
  const input = $("optAutoCheckMinutes");
  $("optAutoCheck").checked = enabled;
  input.value = String(normalizeAutoCheckMinutes(state.autoCheckMinutes));
  input.disabled = !enabled;
}

function clearAutoCheckTimer() {
  if (state.autoCheckTimerId !== null) {
    window.clearTimeout(state.autoCheckTimerId);
    state.autoCheckTimerId = null;
  }
}

function scheduleAutoCheckTimer() {
  clearAutoCheckTimer();
  if (!state.autoCheckEnabled) return;
  const delayMs = normalizeAutoCheckMinutes(state.autoCheckMinutes) * 60 * 1000;
  state.autoCheckTimerId = window.setTimeout(async () => {
    state.autoCheckTimerId = null;
    await refreshAll({ forceCheck: true, notify: true, source: "auto" });
    scheduleAutoCheckTimer();
  }, delayMs);
}

function showToast(message, { kind = "info", actionLabel = "", onAction = null } = {}) {
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
    if (hideTimerId !== null) {
      window.clearTimeout(hideTimerId);
      hideTimerId = null;
    }
    if (leaveTimerId !== null) {
      window.clearTimeout(leaveTimerId);
      leaveTimerId = null;
    }
  };

  const dismiss = () => {
    clearTimers();
    item.remove();
  };

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

  if (actionLabel && typeof onAction === "function") {
    const action = document.createElement("button");
    action.type = "button";
    action.className = "toast-action";
    action.textContent = actionLabel;
    action.addEventListener("click", () => {
      try {
        onAction();
      } finally {
        dismiss();
      }
    });
    item.appendChild(action);
  }

  const close = document.createElement("button");
  close.type = "button";
  close.className = "toast-close";
  close.setAttribute("aria-label", "Dismiss notification");
  close.textContent = "✕";
  close.addEventListener("click", dismiss);
  item.appendChild(close);

  item.addEventListener("mouseenter", pauseHideTimer);
  item.addEventListener("mouseleave", resumeHideTimer);
  item.addEventListener("focusin", pauseHideTimer);
  item.addEventListener("focusout", resumeHideTimer);

  host.appendChild(item);
  while (host.childElementCount > 4) {
    const oldest = host.firstElementChild;
    if (oldest instanceof HTMLElement) {
      oldest.remove();
    } else {
      break;
    }
  }
  startHideTimer();
}

function openRelevantUpdatesView(counts = updateCounts()) {
  const mods = Number(counts?.mods || 0);
  const addons = Number(counts?.addons || 0);

  if (mods > 0 && addons === 0) {
    setProjectView("mods");
    showProjectsPanel();
    setFilter("updates");
    return;
  }
  if (addons > 0 && mods === 0) {
    setProjectView("addons");
    showProjectsPanel();
    setFilter("updates");
    return;
  }

  setTab("home");
}

function maybeNotifyProjectUpdates(source, notify) {
  if (!notify) return;
  const updates = state.repos.filter((repo) => canUpdateRepo(repo));
  const counts = updateCounts();

  if (source === "manual") {
    if (!updates.length) {
      state.lastUpdateNotifyKey = "";
      showToast("No updates available.", { kind: "info" });
      return;
    }

    const noun = counts.total === 1 ? "update" : "updates";
    showToast(`${counts.total} ${noun} available. Mods: ${counts.mods}, Addons: ${counts.addons}.`, {
      kind: "info",
      actionLabel: "Open",
      onAction: () => openRelevantUpdatesView(counts),
    });
    return;
  }

  if (!updates.length) {
    state.lastUpdateNotifyKey = "";
    return;
  }

  const ids = updates.map((repo) => repo.id).sort((a, b) => a - b);
  const key = `${state.activeProfileId}:${ids.join(",")}`;
  if (key === state.lastUpdateNotifyKey) return;
  state.lastUpdateNotifyKey = key;

  const prefix =
    source === "startup"
      ? "Updates detected."
      : source === "auto"
        ? "New updates available."
        : "Updates available.";
  showToast(`${prefix} Mods: ${counts.mods}, Addons: ${counts.addons}.`, {
    kind: "info",
    actionLabel: "Open",
    onAction: () => openRelevantUpdatesView(counts),
  });
}

async function maybePollSelfUpdateInfo({ force = false, notify = false } = {}) {
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
      actionLabel: "Update",
      onAction: () => setTab("about"),
    });
  } catch (_) {
    // Silent by design to avoid repeated noisy errors on background polling.
  }
}

function openAddDialogFor(view) {
  setProjectView(view);
  showProjectsPanel();
  applyAddDialogContext();
  openAddDialog();
}

function focusAddDialogUrlInput() {
  const input = $("repoUrl");
  if (!(input instanceof HTMLInputElement)) return;
  requestAnimationFrame(() => {
    input.focus();
    input.select();
  });
}

function openAddDialog() {
  const dlg = $("dlgAdd");
  if (!dlg) return;
  if (!dlg.open) {
    dlg.showModal();
  }
  focusAddDialogUrlInput();
}

function renderHome() {
  const profile = activeProfile();
  const hasProfile = !!profile;
  const counts = updateCounts();
  const modUpdates = reposForView("mods").filter((repo) => canUpdateRepo(repo));
  const addonUpdates = reposForView("addons").filter((repo) => canUpdateRepo(repo));

  $("homeModsUpdateCount").textContent = String(modUpdates.length);
  $("homeAddonsUpdateCount").textContent = String(addonUpdates.length);

  const homeUpdateAllBtn = $("homeBtnUpdateAll");
  homeUpdateAllBtn.textContent = `Update All (${counts.total})`;
  homeUpdateAllBtn.disabled = !hasProfile || counts.total <= 0;
  homeUpdateAllBtn.classList.toggle("primary", hasProfile && counts.total > 0);
  homeUpdateAllBtn.title =
    counts.total > 0
      ? `Update all mods/addons with available updates (${counts.total}).`
      : "No updates available.";

  const homeAddMenu = $("homeAddMenu");
  if (homeAddMenu) {
    homeAddMenu.classList.toggle("disabled", !hasProfile);
    if (!hasProfile) homeAddMenu.open = false;
  }

  const wowDir = readWowDir();
  const explicitExe = profileExecutablePath(profile);
  const canPlay = hasProfile && !!wowDir;
  const playBtn = $("homeBtnPlay");
  playBtn.disabled = !canPlay;
  playBtn.title = canPlay
    ? (explicitExe
      ? "Launch the executable configured in Instance Settings."
      : "Launch VanillaFixes.exe if present, otherwise Wow.exe.")
    : "Set a valid WoW directory in Options first.";
  const launchStatus = $("homeLaunchStatus");
  if (!hasProfile) {
    launchStatus.textContent = "No instance selected.";
  } else if (!wowDir) {
    launchStatus.textContent = "Set your WoW path in Options to enable launching.";
  } else if (explicitExe) {
    launchStatus.textContent = `Launch mode: ${launchSummary(profile)}. Executable: ${explicitExe}.`;
  } else {
    launchStatus.textContent = `Launch mode: ${launchSummary(profile)}. Target executable fallback: VanillaFixes.exe -> Wow.exe.`;
  }

  const modListEl = $("homeModList");
  if (modListEl) {
    if (!modUpdates.length) {
      modListEl.innerHTML = `<div class="home-update-empty">${
        hasProfile ? "No mod updates." : "Add an instance in Options."
      }</div>`;
    } else {
      modListEl.innerHTML = modUpdates
        .slice(0, 12)
        .map(
          (repo) =>
            `<div class="home-update-line" title="${escapeHtml(repo.owner)}/${escapeHtml(repo.name)}">${escapeHtml(repo.name)}</div>`,
        )
        .join("");
    }
  }

  const addonListEl = $("homeAddonList");
  if (addonListEl) {
    if (!addonUpdates.length) {
      addonListEl.innerHTML = `<div class="home-update-empty">${
        hasProfile ? "No addon updates." : "Add an instance in Options."
      }</div>`;
    } else {
      addonListEl.innerHTML = addonUpdates
        .slice(0, 12)
        .map(
          (repo) =>
            `<div class="home-update-line" title="${escapeHtml(repo.owner)}/${escapeHtml(repo.name)}">${escapeHtml(repo.name)}</div>`,
        )
        .join("");
    }
  }

  $("homeBtnRefreshOnly").disabled = !hasProfile;
}

async function launchGameFromHome() {
  const profile = activeProfile();
  if (!profile) {
    log("ERROR launch: No active instance selected.");
    return;
  }
  const wowDir = readWowDir();
  if (!wowDir) {
    log("ERROR launch: WoW directory is not set.");
    return;
  }
  await withBusy(async () => {
    try {
      const msg = await safeInvoke(
        "wuddle_launch_game",
        { wowDir, launch: launchPayload(profile) },
        { timeoutMs: 8000 },
      );
      log(msg || "Launched game.");
    } catch (e) {
      log(`ERROR launch: ${e.message}`);
    }
  });
}

async function fetchLatestWuddleReleaseTag() {
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

async function refreshAboutInfo({ force = false } = {}) {
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

async function updateWuddleInPlace() {
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
  }
}

function showProjectsPanel() {
  if (state.tab !== "projects") {
    setTab("projects");
  }
}

function loadSettings() {
  let profiles = [];
  const rawProfiles = localStorage.getItem(PROFILES_KEY);
  try {
    if (rawProfiles !== null) {
      const parsed = JSON.parse(rawProfiles);
      if (Array.isArray(parsed)) {
        profiles = parsed
          .map((p) => ({
            id: normalizeProfileId(p?.id || p?.name || "default"),
            name: String(p?.name || "").trim() || "WoW",
            wowDir: String(p?.wowDir || "").trim(),
            launch: normalizeLaunchConfig(p?.launch),
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
  $("optSymlinks").checked = symlinks;
  $("optXattr").checked = xattr;
  $("optClock12").checked = clock12;
  $("optTheme").value = savedTheme;
  $("optFrizFont").checked = useFrizFont;
  $("optAutoCheck").checked = autoCheckEnabled;
  $("optAutoCheckMinutes").value = String(autoCheckMinutes);
  state.clock12 = clock12;
  state.theme = savedTheme;
  state.useFrizFont = useFrizFont;
  state.autoCheckEnabled = autoCheckEnabled;
  state.autoCheckMinutes = autoCheckMinutes;
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

function saveOptionFlags() {
  localStorage.setItem(OPT_SYMLINKS_KEY, $("optSymlinks").checked ? "true" : "false");
  localStorage.setItem(OPT_XATTR_KEY, $("optXattr").checked ? "true" : "false");
  localStorage.setItem(OPT_CLOCK12_KEY, $("optClock12").checked ? "true" : "false");
  const autoCheckEnabled = !!$("optAutoCheck").checked;
  const autoCheckMinutes = normalizeAutoCheckMinutes($("optAutoCheckMinutes").value);
  const selectedTheme = normalizeThemeId($("optTheme")?.value);
  const useFrizFont = !!$("optFrizFont")?.checked;
  localStorage.setItem(OPT_AUTOCHECK_KEY, autoCheckEnabled ? "true" : "false");
  localStorage.setItem(OPT_AUTOCHECK_MINUTES_KEY, String(autoCheckMinutes));
  localStorage.setItem(OPT_THEME_KEY, selectedTheme);
  localStorage.setItem(OPT_FRIZ_FONT_KEY, useFrizFont ? "true" : "false");
  setTheme(selectedTheme);
  setUiFontStyle(useFrizFont);
  state.clock12 = $("optClock12").checked;
  state.autoCheckEnabled = autoCheckEnabled;
  state.autoCheckMinutes = autoCheckMinutes;
  renderAutoCheckSettings();
  scheduleAutoCheckTimer();
  renderLastChecked();
  render();
  renderLog();
}

function installOptions(overrides = {}) {
  return {
    useSymlinks: $("optSymlinks").checked,
    setXattrComment: $("optXattr").checked,
    replaceAddonConflicts: false,
    ...overrides,
  };
}

function readWowDir() {
  const profile = activeProfile();
  return profileWowDir(profile);
}

function currentWowDirStrict() {
  const profile = ensureActiveProfile();
  if (!profile) return null;
  const wowDir = readWowDir();
  if (!wowDir) {
    log(`ERROR: WoW directory is empty for ${profile.name || "active instance"}.`);
    return null;
  }
  return wowDir;
}

async function selectProfile(profileId) {
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
  setTab("home");
  await setBackendActiveProfile();
  await refreshAll();
}

async function addInstance() {
  const baseName = `WoW${state.profiles.length + 1}`;
  const name = baseName;
  const idBase = normalizeProfileId(name);
  let id = idBase;
  let n = 2;
  while (state.profiles.some((p) => p.id === id)) {
    id = `${idBase}-${n++}`;
  }
  const wowDir = "";
  state.profiles.push({ id, name, wowDir, launch: defaultLaunchConfig() });
  state.activeProfileId = id;
  state.projectViewByProfile[id] = "mods";
  syncProjectViewFromActiveProfile();
  persistProjectViewByProfile();
  persistProfiles();
  renderProfileTabs();
  renderInstanceList();
  await setBackendActiveProfile();
  log(`Created instance ${name}.`);
  render();
  openInstanceSettingsDialog(getProfileById(id));
}

async function removeInstance(profileId) {
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
  await refreshAll();
}

async function pickWowDirForProfile(profileId) {
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
      await refreshAll();
    }
  } catch (e) {
    log(`ERROR picker: ${e.message}`);
  }
}

function renderInstanceSettingsLaunchFields(method) {
  const current = String(method || "auto").toLowerCase();
  $("instanceSettingsLaunchAuto").classList.toggle("hidden", current !== "auto");
  $("instanceSettingsLaunchLutris").classList.toggle("hidden", current !== "lutris");
  $("instanceSettingsLaunchWine").classList.toggle("hidden", current !== "wine");
  $("instanceSettingsLaunchCustom").classList.toggle("hidden", current !== "custom");
}

function openInstanceSettingsDialog(profile) {
  if (!profile) return;
  const launch = profileLaunch(profile);
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
  $("instanceSettingsWorkingDir").value = launch.workingDir || "";
  $("instanceSettingsEnv").value = launch.envText || "";
  $("btnInstanceSettingsOpenPath").disabled = !profileWowDir(profile);
  renderInstanceSettingsLaunchFields(launch.method);
  $("dlgInstanceSettings").showModal();
}

function saveInstanceSettingsFromDialog() {
  const id = normalizeProfileId($("instanceSettingsId").value || "");
  const profile = getProfileById(id);
  if (!profile) return false;

  const nextName = String($("instanceSettingsName").value || "").trim() || profile.name || "WoW";
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
  });

  profile.name = nextName;
  profile.wowDir = nextPath;
  profile.launch = launch;
  if (profile.id === state.activeProfileId) {
    localStorage.setItem(WOW_KEY, profile.wowDir || "");
  }
  persistProfiles();
  renderProfileTabs();
  renderInstanceList();
  render();
  return true;
}

function setGithubAuthStatus(message, kind = "") {
  const statusLine = $("ghAuthStatus");
  statusLine.classList.remove("status-ok", "status-warn");
  if (kind) statusLine.classList.add(kind);
  statusLine.textContent = message;
}

function renderGithubAuth(status) {
  state.githubAuth = status;
  const statusKnown = status && typeof status === "object";
  const keychainAvailable = !!status?.keychainAvailable;
  const tokenStored = !!status?.tokenStored;
  const envTokenPresent = !!status?.envTokenPresent;

  const input = $("githubToken");
  const saveBtn = $("btnSaveGithubToken");
  const clearBtn = $("btnClearGithubToken");
  const hint = $("ghAuthHint");
  const badge = $("ghAuthBadge");

  badge.className = "auth-badge";

  // Keep auth controls available even while probing keychain.
  input.disabled = false;
  saveBtn.disabled = false;
  clearBtn.disabled = false;

  if (!statusKnown) {
    hint.textContent = "Optional: add a GitHub token to avoid anonymous API rate limits.";
    badge.textContent = "Checking…";
    badge.classList.add("info");
    setGithubAuthStatus("Checking keychain availability…");
    renderGithubAuthHealth();
    return;
  }

  if (!keychainAvailable) {
    hint.textContent =
      "System keychain unavailable. Saving still works for this session (env token).";
    if (envTokenPresent) {
      badge.textContent = "Env token";
      badge.classList.add("info");
    } else {
      badge.textContent = "No token";
      badge.classList.add("warn");
    }
    setGithubAuthStatus(
      envTokenPresent
        ? "Environment token detected (authenticated requests enabled)."
        : "No token detected.",
      envTokenPresent ? "status-ok" : "status-warn",
    );
    state.loggedNoTokenAutoSkip = false;
    renderGithubAuthHealth();
    return;
  }

  hint.textContent = "Optional: add a GitHub token to avoid anonymous API rate limits.";
  if (tokenStored) {
    badge.textContent = "Keychain token";
    badge.classList.add("ok");
    setGithubAuthStatus("Token saved in system keychain.", "status-ok");
    state.loggedNoTokenAutoSkip = false;
  } else if (envTokenPresent) {
    badge.textContent = "Env token";
    badge.classList.add("info");
    setGithubAuthStatus("Environment token detected (not stored in keychain).", "status-ok");
    state.loggedNoTokenAutoSkip = false;
  } else {
    badge.textContent = "No token";
    badge.classList.add("warn");
    setGithubAuthStatus("No token saved (optional).", "status-warn");
  }
  renderGithubAuthHealth();
}

function hasGithubAuthToken() {
  return !!state.githubAuth?.tokenStored || !!state.githubAuth?.envTokenPresent;
}

function detectGithubAuthIssue() {
  if (!hasGithubAuthToken()) return null;
  const authIssuePattern =
    /bad credentials|requires authentication|http\s*401|http\s*403|rate[\s-]?limit|forbidden/i;
  for (const plan of state.plans) {
    const error = String(plan?.error || "").trim();
    if (!error) continue;
    if (authIssuePattern.test(error)) {
      return error;
    }
  }
  return null;
}

function renderGithubAuthHealth() {
  const el = $("ghAuthHealth");
  if (!el) return;
  if (state.tab !== "options") return;

  const issue = detectGithubAuthIssue();
  if (!issue) {
    el.textContent = "";
    el.className = "auth-health hidden";
    state.authHealthActiveIssue = "";
    return;
  }

  const issueKey = issue.toLowerCase().trim();
  if (!state.authHealthActiveIssue) {
    if (state.authHealthSeenSession) {
      el.textContent = "";
      el.className = "auth-health hidden";
      return;
    }
    state.authHealthSeenSession = true;
    state.authHealthActiveIssue = issueKey;
  } else if (state.authHealthActiveIssue !== issueKey) {
    // Keep a stable, non-repeating banner within the current session.
    el.textContent = "";
    el.className = "auth-health hidden";
    return;
  }

  const preview = issue.length > 190 ? `${issue.slice(0, 187)}...` : issue;
  el.className = "auth-health warn";
  el.textContent =
    "Token is configured, but GitHub still reports auth/rate-limit errors. Re-save token or create a new classic token (no scopes). Latest: "
    + preview;
}

async function refreshGithubAuthStatus() {
  try {
    const status = await safeInvoke("wuddle_github_auth_status", {}, { timeoutMs: 5000 });
    renderGithubAuth(status);
  } catch (e) {
    renderGithubAuth(null);
    $("ghAuthHint").textContent = "Could not read GitHub auth status.";
    setGithubAuthStatus(e.message);
  }
}

async function saveGithubToken() {
  const token = $("githubToken").value.trim();
  if (!token) {
    log("ERROR auth: GitHub token is empty.");
    return;
  }

  await withBusy(async () => {
    try {
      await safeInvoke("wuddle_github_auth_set_token", { token }, { timeoutMs: 8000 });
      $("githubToken").value = "";
      await refreshGithubAuthStatus();
      if (state.githubAuth?.tokenStored) {
        log("GitHub token saved to system keychain.");
      } else if (state.githubAuth?.envTokenPresent) {
        log("GitHub token set for current session (keychain unavailable).");
      } else {
        log("GitHub token saved.");
      }
      await refreshAll({ forceCheck: true });
    } catch (e) {
      log(`ERROR auth save: ${e.message}`);
    }
  });
}

async function clearGithubToken() {
  await withBusy(async () => {
    try {
      await safeInvoke("wuddle_github_auth_clear_token", {}, { timeoutMs: 8000 });
      $("githubToken").value = "";
      await refreshGithubAuthStatus();
      log("GitHub token cleared.");
    } catch (e) {
      log(`ERROR auth clear: ${e.message}`);
    }
  });
}

async function connectGithub() {
  await openUrl("https://github.com/settings/tokens");
}

function repoStatus(repo) {
  if (!repo.enabled) return { kind: "muted", text: "Disabled" };

  const plan = getPlanForRepo(repo.id);
  if (!plan) return { kind: "muted", text: "Unknown" };
  if (plan.error) return { kind: "bad", text: "Fetch error" };

  if (plan.repair_needed) return { kind: "warn", text: "Repair needed" };
  if (plan.has_update) return { kind: "warn", text: "Update available" };
  return { kind: "good", text: "Up to date" };
}

function classifyFetchErrorHint(errorText) {
  const error = String(errorText || "").trim();
  const lower = error.toLowerCase();
  if (!lower) {
    return "Check Logs for details.";
  }
  if (
    /rate[\s-]?limit|http\s*403|http\s*429|forbidden|bad credentials|requires authentication/.test(
      lower,
    )
  ) {
    return "GitHub API/auth issue. Open Settings > GitHub Authentication and save a valid token.";
  }
  if (/tls|ssl|certificate|connect remote|no tls stream/.test(lower)) {
    return "Network/TLS connection issue while contacting remote. Check internet/proxy/firewall.";
  }
  if (/timed out|timeout|deadline exceeded/.test(lower)) {
    return "Request timed out. Try again or reduce concurrent network load.";
  }
  if (/could not resolve|dns|name or service not known|no such host/.test(lower)) {
    return "DNS/host resolution failed. Verify URL and network DNS.";
  }
  if (/not found|http\s*404/.test(lower)) {
    return "Repository/release not found. URL may be wrong or private.";
  }
  return "Check Logs for detailed error output.";
}

function formatRepoStatusTooltip(repo, plan) {
  if (!repo.enabled) {
    return "Project is disabled in Wuddle. Enable it to include it in update/install operations.";
  }
  if (!plan) {
    return "No update data yet. Click “Check for updates”.";
  }
  if (plan.error) {
    return `Fetch error: ${plan.error}\n\nHint: ${classifyFetchErrorHint(plan.error)}`;
  }
  if (plan.repair_needed) {
    return "Installed files look incomplete or mismatched. Use “Reinstall / Repair”.";
  }
  if (plan.has_update) {
    return `Update available: ${versionLabel(plan.current)} → ${versionLabel(plan.latest)}.`;
  }
  return `Up to date at ${versionLabel(plan.latest)}.`;
}

function displayForge(repo) {
  let host = (repo?.host || "").toLowerCase();
  if (!host) {
    try {
      host = new URL(repo?.url || "").hostname.toLowerCase();
    } catch (_) {}
  }

  if (host === "codeberg.org") {
    return "codeberg";
  }
  return repo?.forge || "unknown";
}

function getPlanForRepo(repoId) {
  return state.planByRepoId.get(repoId) || null;
}

function branchOptionsForRepo(repo) {
  const cached = state.branchOptionsByRepoId.get(repo.id);
  const out = [];
  const seen = new Set();
  const selected = String(repo?.gitBranch || "").trim() || "master";

  out.push({ value: "master", label: "master (default)" });
  seen.add("master");

  if (selected && !seen.has(selected.toLowerCase())) {
    out.push({ value: selected, label: selected });
    seen.add(selected.toLowerCase());
  }

  for (const b of cached || []) {
    const v = String(b || "").trim();
    if (!v) continue;
    const key = v.toLowerCase();
    if (seen.has(key)) continue;
    out.push({ value: v, label: v });
    seen.add(key);
  }
  return out;
}

async function loadRepoBranches(repo) {
  if (!isAddonRepo(repo)) return;
  if (state.branchOptionsByRepoId.has(repo.id)) return;
  if (state.branchOptionsLoading.has(repo.id)) return;

  state.branchOptionsLoading.add(repo.id);
  try {
    const branches = await safeInvoke("wuddle_list_repo_branches", { id: repo.id }, { timeoutMs: 20000 });
    state.branchOptionsByRepoId.set(repo.id, Array.isArray(branches) ? branches : []);
    render();
  } catch (e) {
    log(`ERROR branches ${repo.owner}/${repo.name}: ${e.message}`);
  } finally {
    state.branchOptionsLoading.delete(repo.id);
  }
}

async function setRepoBranch(repo, branch) {
  const normalized = String(branch || "").trim();
  const chosen = normalized || "master";
  try {
    const msg = await safeInvoke("wuddle_set_repo_branch", {
      id: repo.id,
      branch: chosen,
    });
    log(`${repo.owner}/${repo.name}: ${msg}`);
    const current = state.repos.find((r) => r.id === repo.id);
    if (current) {
      current.gitBranch = chosen;
    }
    await refreshAll({ forceCheck: true });
  } catch (e) {
    log(`ERROR set branch ${repo.owner}/${repo.name}: ${e.message}`);
  }
}

function canUpdateRepo(repo) {
  if (!repo.enabled) return false;
  const plan = getPlanForRepo(repo.id);
  if (!plan) return false;
  if (plan.error) return false;
  return !!plan.has_update;
}

function updateDisabledReason(repo) {
  if (!repo.enabled) return "Project is disabled.";
  const plan = getPlanForRepo(repo.id);
  if (!plan) return "No update data yet.";
  if (plan.error) {
    return `Update unavailable: fetch failed. ${classifyFetchErrorHint(plan.error)}`;
  }
  if (plan.repair_needed) return "Use Reinstall / Repair from the actions menu.";
  if (!plan.has_update) return "No update available.";
  return "";
}

function versionLabel(value) {
  const v = String(value ?? "").trim();
  if (!v) return "—";
  if (v === "unknown") return "—";
  return v;
}

function statusRank(repo) {
  const st = repoStatus(repo);
  if (st.text === "Fetch error") return 0;
  if (st.text === "Update available") return 1;
  if (st.text === "Repair needed") return 2;
  if (st.text === "Disabled") return 3;
  return 4;
}

function compareVersionText(a, b) {
  return a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" });
}

function matchesFilter(repo) {
  if (state.filter === "all") return true;
  if (state.filter === "disabled") return !repo.enabled;
  const plan = getPlanForRepo(repo.id);
  if (state.filter === "updates") return !!plan?.has_update;
  if (state.filter === "errors") return !!plan?.error;
  return true;
}

function matchesProjectSearch(repo) {
  const raw = String(state.projectSearchQuery || "").trim().toLowerCase();
  if (!raw) return true;

  const haystack = [
    String(repo?.name || ""),
    String(repo?.owner || ""),
    String(repo?.forge || ""),
    String(repo?.host || ""),
    String(repo?.url || ""),
  ]
    .join(" ")
    .toLowerCase();

  const terms = raw.split(/\s+/).filter(Boolean);
  return terms.every((term) => haystack.includes(term));
}

function renderProjectSearch() {
  const input = $("projectSearchInput");
  if (!(input instanceof HTMLInputElement)) {
    return;
  }
  if (input.value !== state.projectSearchQuery) {
    input.value = state.projectSearchQuery;
  }
}

function sortedFilteredRepos() {
  const dir = state.sortDir === "desc" ? -1 : 1;
  const list = reposForCurrentView().filter((repo) => matchesFilter(repo) && matchesProjectSearch(repo));

  list.sort((a, b) => {
    if (state.sortKey === "name") {
      return dir * a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    }
    if (state.sortKey === "current") {
      const av = state.projectView === "addons"
        ? String(a.gitBranch || "").trim() || "default"
        : versionLabel(getPlanForRepo(a.id)?.current);
      const bv = state.projectView === "addons"
        ? String(b.gitBranch || "").trim() || "default"
        : versionLabel(getPlanForRepo(b.id)?.current);
      return dir * compareVersionText(av, bv);
    }
    if (state.sortKey === "latest") {
      const av = versionLabel(getPlanForRepo(a.id)?.latest);
      const bv = versionLabel(getPlanForRepo(b.id)?.latest);
      return dir * compareVersionText(av, bv);
    }
    if (state.sortKey === "status") {
      return dir * (statusRank(a) - statusRank(b));
    }
    return 0;
  });

  return list;
}

function renderFilterButtons() {
  const summary = getProjectSummary();
  const labels = {
    all: `All (${summary.total})`,
    updates: `Updates (${summary.updates})`,
    errors: `Errors (${summary.errors})`,
    disabled: `Disabled (${summary.disabled})`,
  };
  document.querySelectorAll(".filter-btn[data-filter]").forEach((btn) => {
    const key = btn.getAttribute("data-filter");
    btn.classList.toggle("active", key === state.filter);
    if (key && Object.prototype.hasOwnProperty.call(labels, key)) {
      btn.textContent = labels[key];
    }
  });
}

function renderProjectViewButtons() {
  const modsBtn = $("btnViewMods");
  const addonsBtn = $("btnViewAddons");
  if (!modsBtn || !addonsBtn) return;

  const modsUpdates = reposForView("mods").filter((repo) => canUpdateRepo(repo)).length;
  const addonsUpdates = reposForView("addons").filter((repo) => canUpdateRepo(repo)).length;
  modsBtn.textContent = `Mods (${modsUpdates})`;
  addonsBtn.textContent = `Addons (${addonsUpdates})`;

  if (state.tab !== "projects") {
    modsBtn.classList.remove("active");
    addonsBtn.classList.remove("active");
    return;
  }

  const addons = state.projectView === "addons";
  modsBtn.classList.toggle("active", !addons);
  addonsBtn.classList.toggle("active", addons);
}

function renderSortHeaders() {
  const addonsView = state.projectView === "addons";
  const panel = $("panelProjects");
  panel?.classList.toggle("addons-mode", addonsView);

  const thCurrent = $("thCurrent");
  const thLatest = $("thLatest");
  const thEnabled = $("thEnabled");
  if (thCurrent) thCurrent.textContent = addonsView ? "Branch" : "Current";
  if (thLatest) thLatest.classList.toggle("col-hidden", addonsView);
  if (thEnabled) thEnabled.classList.toggle("col-hidden", addonsView);

  if (addonsView && state.sortKey === "latest") {
    // Keep sort predictable when switching view.
    state.sortKey = "name";
    state.sortDir = "asc";
  }

  document.querySelectorAll("#repoThead .th.sortable").forEach((th) => {
    if (addonsView && th.id === "thLatest") {
      th.classList.add("col-hidden");
      return;
    }
    th.classList.remove("col-hidden");
    const key = th.getAttribute("data-sort");
    const active = key === state.sortKey;
    th.classList.toggle("active", active);
    th.setAttribute("data-dir", active ? state.sortDir : "");
  });
}

function renderLastChecked() {
  $("lastChecked").textContent = `Last checked: ${formatTime(state.lastCheckedAt)}`;
}

function getProjectSummary() {
  const viewRepos = reposForCurrentView();
  const total = viewRepos.length;
  const enabled = viewRepos.filter((repo) => repo.enabled).length;
  const disabled = total - enabled;
  const updates = viewRepos.filter((repo) => {
    const plan = getPlanForRepo(repo.id);
    return repo.enabled && !!plan?.has_update && !plan?.error;
  }).length;
  const errors = viewRepos.filter((repo) => !!getPlanForRepo(repo.id)?.error).length;
  const rateLimited = viewRepos.some((repo) => {
    const error = getPlanForRepo(repo.id)?.error || "";
    return /rate[\s-]?limit|http 403|http 429/i.test(error);
  });

  return { total, enabled, disabled, updates, errors, rateLimited };
}

function renderProjectStatusStrip() {
  const hasProfile = !!activeProfile();
  const summary = getProjectSummary();

  const apiEl = $("statApiState");
  if (!apiEl) return;
  apiEl.className = "stat-pill";
  if (!hasProfile) {
    apiEl.textContent = "API status: no instance";
    apiEl.classList.add("muted");
  } else if (summary.rateLimited) {
    apiEl.textContent = "API status: rate limited";
    apiEl.classList.add("warn");
  } else if (summary.errors > 0) {
    apiEl.textContent = "API status: partial errors";
    apiEl.classList.add("bad");
  } else {
    apiEl.textContent = "API status: healthy";
    apiEl.classList.add("good");
  }
  apiEl.title = `${summary.enabled}/${summary.total} enabled`;
}

function closeActionsMenu() {
  if (state.openMenuRepoId === null) return;
  state.openMenuRepoId = null;
  render();
}

function toggleActionsMenu(repoId) {
  state.openMenuRepoId = state.openMenuRepoId === repoId ? null : repoId;
  render();
}

function positionOpenMenu() {
  const wrap = document.querySelector(".menu-wrap.open");
  if (!(wrap instanceof HTMLElement)) return;
  const menu = wrap.querySelector(".menu-pop");
  if (!(menu instanceof HTMLElement)) return;

  const wrapRect = wrap.getBoundingClientRect();
  const menuRect = menu.getBoundingClientRect();
  const margin = 8;
  let left = wrapRect.right - menuRect.width;
  left = Math.max(margin, Math.min(left, window.innerWidth - menuRect.width - margin));

  let top = wrapRect.bottom + 6;
  if (top + menuRect.height > window.innerHeight - margin) {
    top = wrapRect.top - menuRect.height - 6;
    if (top < margin) {
      top = Math.max(margin, window.innerHeight - menuRect.height - margin);
    }
  }

  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
}

function bindDialogOutsideToClose(dlg) {
  dlg.addEventListener("click", (ev) => {
    if (ev.target !== dlg) return;
    dlg.close();
  });
}

function confirmExternalOpen(kind, target) {
  const value = String(target ?? "").trim();
  if (!value) return false;
  if (kind === "path") {
    return window.confirm(
      `Wuddle is about to open this directory in your file manager:\n\n${value}`,
    );
  }
  return window.confirm(
    `Wuddle is about to open this link in your default browser:\n\n${value}`,
  );
}

async function openUrl(url) {
  const target = String(url ?? "").trim();
  if (!target) {
    log("ERROR open url: URL is empty.");
    return;
  }
  try {
    await safeInvoke("plugin:opener|open_url", { url: target });
  } catch (err) {
    log(`ERROR open url: ${err?.message || String(err)}`);
  }
}

async function openPath(path) {
  const target = String(path ?? "").trim();
  if (!target) {
    log("ERROR open dir: Path is empty.");
    return;
  }
  if (!confirmExternalOpen("path", target)) {
    log("Cancelled opening directory.");
    return;
  }
  try {
    await safeInvoke("wuddle_open_directory", { path: target });
  } catch (err) {
    log(`ERROR open dir: ${err?.message || String(err)}`);
  }
}

function openRemoveDialog(repo) {
  state.removeTargetRepo = repo;
  $("removeRepoName").textContent = `${repo.owner}/${repo.name}`;
  $("removeLocalFiles").checked = false;
  $("dlgRemove").showModal();
}

function openRemoveInstanceDialog(profile) {
  state.removeTargetProfile = profile;
  const profileLabel = profile?.name || "WoW";
  const wowDir = profile?.wowDir ? ` - ${profile.wowDir}` : "";
  $("removeInstanceName").textContent = `${profileLabel}${wowDir}`;
  $("removeInstanceLocalFiles").checked = false;
  $("dlgRemoveInstance").showModal();
}

async function confirmRemove() {
  const repo = state.removeTargetRepo;
  if (!repo) {
    $("dlgRemove").close();
    return;
  }

  const removeLocalFiles = $("removeLocalFiles").checked;
  const wowDir = removeLocalFiles ? readWowDir() : null;
  if (removeLocalFiles && !wowDir) {
    log("ERROR remove: WoW directory is required to remove local files.");
    return;
  }

  await withBusy(async () => {
    try {
      const msg = await safeInvoke("wuddle_remove_repo", {
        id: repo.id,
        removeLocalFiles,
        wowDir,
      });
      log(`${repo.owner}/${repo.name}: ${msg}`);
      $("dlgRemove").close();
      state.removeTargetRepo = null;
      await refreshAll();
    } catch (e) {
      log(`ERROR removing: ${e.message}`);
    }
  });
}

async function confirmRemoveInstance() {
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

async function setRepoEnabled(repo, enabled) {
  try {
    const wowDir = readWowDir() || null;
    const msg = await safeInvoke("wuddle_set_repo_enabled", { id: repo.id, enabled, wowDir });
    log(`${repo.owner}/${repo.name}: ${msg}`);
    await refreshAll();
  } catch (e) {
    log(`ERROR toggling repo: ${e.message}`);
  }
}

async function updateRepo(repo) {
  const wowDir = currentWowDirStrict();
  if (!wowDir) return;

  log(`Updating ${repo.owner}/${repo.name}...`);
  await withBusy(async () => {
    try {
      const result = await safeInvoke("wuddle_update_repo", {
        id: repo.id,
        wowDir,
        ...installOptions(),
      });
      const msg = logOperationResult(result);
      await refreshAll({ forceCheck: true });
    } catch (e) {
      const conflict = isAddonRepo(repo) ? parseAddonConflictError(e.message) : null;
      if (conflict) {
        const proceed = await confirmAddonConflict(repo, conflict);
        if (!proceed) {
          log(`${repo.owner}/${repo.name}: cancelled install (existing addon files kept).`);
          return;
        }
        try {
          const retryResult = await safeInvoke("wuddle_update_repo", {
            id: repo.id,
            wowDir,
            ...installOptions({ replaceAddonConflicts: true }),
          });
          logOperationResult(retryResult);
          await refreshAll({ forceCheck: true });
          return;
        } catch (retryErr) {
          log(`ERROR update ${repo.owner}/${repo.name}: ${retryErr.message}`);
          return;
        }
      }
      log(`ERROR update ${repo.owner}/${repo.name}: ${e.message}`);
    }
  });
}

async function reinstallRepo(repo) {
  const wowDir = currentWowDirStrict();
  if (!wowDir) return;

  log(`Reinstalling ${repo.owner}/${repo.name}...`);
  await withBusy(async () => {
    try {
      const result = await safeInvoke("wuddle_reinstall_repo", {
        id: repo.id,
        wowDir,
        ...installOptions(),
      });
      logOperationResult(result);
      await refreshAll({ forceCheck: true });
    } catch (e) {
      const conflict = isAddonRepo(repo) ? parseAddonConflictError(e.message) : null;
      if (conflict) {
        const proceed = await confirmAddonConflict(repo, conflict);
        if (!proceed) {
          log(`${repo.owner}/${repo.name}: cancelled reinstall (existing addon files kept).`);
          return;
        }
        try {
          const retryResult = await safeInvoke("wuddle_reinstall_repo", {
            id: repo.id,
            wowDir,
            ...installOptions({ replaceAddonConflicts: true }),
          });
          logOperationResult(retryResult);
          await refreshAll({ forceCheck: true });
          return;
        } catch (retryErr) {
          log(`ERROR reinstall ${repo.owner}/${repo.name}: ${retryErr.message}`);
          return;
        }
      }
      log(`ERROR reinstall ${repo.owner}/${repo.name}: ${e.message}`);
    }
  });
}

function render() {
  closeThemedSelectMenus();
  renderAddPresets();
  renderHome();
  renderProjectSearch();
  const host = $("repoRows");
  host.innerHTML = "";
  renderProjectViewButtons();
  applyAddDialogContext();
  const profile = activeProfile();
  const hasProfile = !!profile;
  $("btnAddOpen").disabled = !hasProfile;
  $("btnUpdateAll").disabled = !hasProfile;
  $("btnViewMods").disabled = !hasProfile;
  $("btnViewAddons").disabled = !hasProfile;

  renderFilterButtons();
  renderSortHeaders();
  renderLastChecked();
  renderProjectStatusStrip();
  renderGithubAuthHealth();
  const failedCount = reposForCurrentView().filter((r) => !!getPlanForRepo(r.id)?.error).length;
  $("btnRetryFailed").classList.toggle("hidden", failedCount === 0);
  $("btnRetryFailed").disabled = failedCount === 0 || !hasProfile;
  $("btnRetryFailed").title = failedCount ? `Retry ${failedCount} failed fetch(es)` : "No failed fetches";

  const updateActionState = getUpdateActionState();
  const updateActionBtn = $("btnUpdateAll");
  updateActionBtn.textContent = updateActionState.label;
  updateActionBtn.title = updateActionState.title;
  updateActionBtn.classList.toggle("primary", updateActionState.primary);
  updateActionBtn.disabled = !hasProfile || updateActionState.disabled;
  if (!hasProfile) {
    updateActionBtn.textContent = "Check for updates";
    updateActionBtn.title = "Add an instance in Options first.";
  }

  const visibleRepos = sortedFilteredRepos();

  if (!hasProfile) {
    return;
  }

  if (!visibleRepos.length) {
    const div = document.createElement("div");
    div.className = "empty";
    const totalForView = reposForCurrentView().length;
    const noun = state.projectView === "addons" ? "addons" : "mods";
    div.textContent = totalForView
      ? `No ${noun} match the current filter.`
      : `No ${noun} yet. Click “＋ Add”.`;
    host.appendChild(div);
    return;
  }

  for (const r of visibleRepos) {
    const st = repoStatus(r);

    const row = document.createElement("div");
    row.className = "trow";

    const nameCell = document.createElement("div");
    nameCell.className = "namecell";

    const nameMain = document.createElement("div");
    const forgeLabel = displayForge(r);
    const plan = getPlanForRepo(r.id);
    nameMain.className = "name-main";
    nameMain.title = r.url;

    const nameHeader = document.createElement("div");
    nameHeader.className = "name-header";

    const nameLink = document.createElement("button");
    nameLink.className = "name-link";
    nameLink.textContent = r.name;
    nameLink.title = `Open ${r.url}`;
    nameLink.addEventListener("click", async (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      await openUrl(r.url);
    });

    const nameSub = document.createElement("div");
    nameSub.className = "name-sub";
    nameSub.textContent = `${r.owner} • ${forgeLabel}${r.enabled ? "" : " • disabled"}`;

    nameHeader.appendChild(nameLink);
    nameMain.appendChild(nameHeader);
    nameMain.appendChild(nameSub);
    nameCell.appendChild(nameMain);

    const status = document.createElement("div");
    status.innerHTML = `<span class="badge ${st.kind}">${escapeHtml(st.text)}</span>`;
    if (plan?.error) {
      status.title = plan.error;
    }

    const addonsView = state.projectView === "addons";
    const currentCell = document.createElement("div");
    if (addonsView) {
      currentCell.className = "branch-cell";
      const select = document.createElement("select");
      select.className = "branch-select";
      const options = branchOptionsForRepo(r);
      const selected = String(r.gitBranch || "").trim() || "master";
      for (const opt of options) {
        const el = document.createElement("option");
        el.value = opt.value;
        el.textContent = opt.label;
        if ((selected || "") === opt.value) {
          el.selected = true;
        }
        select.appendChild(el);
      }
      if (state.branchOptionsLoading.has(r.id)) {
        select.disabled = true;
      } else {
        select.disabled = false;
      }
      select.addEventListener("click", (ev) => {
        ev.stopPropagation();
      });
      select.addEventListener("change", async (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        await setRepoBranch(r, select.value);
      });
      currentCell.appendChild(select);
      ensureThemedSelect(select, "branch-select-menu");
      void loadRepoBranches(r);
    } else {
      currentCell.className = "version-cell";
      currentCell.textContent = versionLabel(plan?.current);
    }

    const latestCell = document.createElement("div");
    latestCell.className = `version-cell${addonsView ? " col-hidden" : ""}`;
    latestCell.textContent = versionLabel(plan?.latest);

    const actions = document.createElement("div");
    actions.className = "right";

    const updateBtn = document.createElement("button");
    updateBtn.className = "btn icon action-update";
    updateBtn.textContent = "⤓";
    updateBtn.setAttribute("aria-label", "Update");
    updateBtn.disabled = !canUpdateRepo(r);
    updateBtn.title = updateBtn.disabled ? updateDisabledReason(r) : "Update now";
    updateBtn.addEventListener("click", async (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      state.openMenuRepoId = null;
      await updateRepo(r);
    });

    const enableBtn = document.createElement("button");
    enableBtn.className = `toggle-btn${r.enabled ? " on" : ""}`;
    enableBtn.title = r.enabled
      ? "Disable this project. Wuddle comments it out in dlls.txt so it will not load in-game."
      : "Enable this project. Wuddle uncomments/adds it in dlls.txt so it can load in-game.";
    enableBtn.setAttribute("aria-label", r.enabled ? "Disable project" : "Enable project");
    enableBtn.setAttribute("aria-pressed", r.enabled ? "true" : "false");
    enableBtn.addEventListener("click", async (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      state.openMenuRepoId = null;
      await setRepoEnabled(r, !r.enabled);
    });

    const enabledCell = document.createElement("div");
    enabledCell.className = `enabled-col${addonsView ? " col-hidden" : ""}`;
    enabledCell.appendChild(enableBtn);

    const menuWrap = document.createElement("div");
    menuWrap.className = "menu-wrap";
    if (state.openMenuRepoId === r.id) {
      menuWrap.classList.add("open");
    }

    const menuBtn = document.createElement("button");
    menuBtn.className = "btn icon menu-trigger";
    menuBtn.title = "More actions";
    menuBtn.textContent = "⋮";
    menuBtn.setAttribute("aria-haspopup", "menu");
    menuBtn.setAttribute("aria-expanded", state.openMenuRepoId === r.id ? "true" : "false");
    menuBtn.addEventListener("click", (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      toggleActionsMenu(r.id);
    });

    const menu = document.createElement("div");
    menu.className = "menu-pop";
    menu.setAttribute("role", "menu");

    const reinstall = document.createElement("button");
    reinstall.className = "menu-item";
    reinstall.textContent = "Reinstall / Repair";
    reinstall.addEventListener("click", async (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      state.openMenuRepoId = null;
      await reinstallRepo(r);
    });

    const del = document.createElement("button");
    del.className = "menu-item menu-danger";
    del.textContent = "Remove";
    del.addEventListener("click", async (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      state.openMenuRepoId = null;
      openRemoveDialog(r);
    });

    menu.appendChild(reinstall);
    if (!isAddonRepo(r)) {
      const toggle = document.createElement("button");
      toggle.className = "menu-item";
      toggle.textContent = r.enabled ? "Disable" : "Enable";
      toggle.addEventListener("click", async (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        state.openMenuRepoId = null;
        await setRepoEnabled(r, !r.enabled);
      });
      menu.appendChild(toggle);
    }
    menu.appendChild(del);
    menuWrap.appendChild(menuBtn);
    menuWrap.appendChild(menu);

    actions.appendChild(updateBtn);
    actions.appendChild(menuWrap);

    row.appendChild(nameCell);
    row.appendChild(currentCell);
    row.appendChild(latestCell);
    row.appendChild(enabledCell);
    row.appendChild(status);
    row.appendChild(actions);

    host.appendChild(row);
  }

  requestAnimationFrame(positionOpenMenu);
}

function getUpdateActionState() {
  const hasProfile = !!activeProfile();
  if (!hasProfile) {
    return {
      mode: "check",
      label: "Check for updates",
      title: "Add an instance in Options first.",
      primary: false,
      disabled: true,
    };
  }

  const updatableCount = state.repos.filter((repo) => canUpdateRepo(repo)).length;

  if (updatableCount > 0) {
    return {
      mode: "update_all",
      label: `Update (${updatableCount})`,
      title: `Update all tracked mods/addons with available updates (${updatableCount}).`,
      primary: true,
      disabled: false,
    };
  }

  return {
    mode: "check",
    label: "Check for updates",
    title: "Check tracked mods/addons for updates.",
    primary: false,
    disabled: false,
  };
}

function escapeHtml(s) {
  return String(s ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

async function loadRepos() {
  const wowDir = readWowDir() || null;
  state.repos = await safeInvoke("wuddle_list_repos", { wowDir }, { timeoutMs: 12000 });
  const known = new Set(state.repos.map((r) => r.id));
  state.branchOptionsByRepoId = new Map(
    Array.from(state.branchOptionsByRepoId.entries()).filter(([id]) => known.has(id)),
  );
  state.branchOptionsLoading = new Set(
    Array.from(state.branchOptionsLoading.values()).filter((id) => known.has(id)),
  );
}

async function checkUpdates(options = {}) {
  const notify = !!options.notify;
  const source = String(options.source || "refresh");
  const prevByRepo = new Map(state.plans.map((p) => [p.repo_id, p]));
  const wowDir = readWowDir() || null;
  const next = await safeInvoke("wuddle_check_updates", { wowDir }, { timeoutMs: 30000 });
  const checkedAt = new Date();

  state.plans = next.map((plan) => {
    if (!plan.not_modified) return plan;

    const prev = prevByRepo.get(plan.repo_id);
    if (!prev) return plan;

    if (prev.has_update && prev.current === plan.current) {
      return {
        ...plan,
        has_update: true,
        latest: prev.latest || plan.latest,
        asset_name: prev.asset_name || plan.asset_name,
        repair_needed: prev.repair_needed || plan.repair_needed,
        error: plan.error ?? prev.error ?? null,
      };
    }

    return plan;
  });
  state.planByRepoId = new Map(state.plans.map((plan) => [plan.repo_id, plan]));

  state.lastCheckedAt = checkedAt;

  const repoById = new Map(state.repos.map((repo) => [repo.id, repo]));
  for (const plan of state.plans) {
    if (plan.not_modified) continue;
    const prevError = prevByRepo.get(plan.repo_id)?.error || "";
    const nextError = plan.error || "";
    if (nextError && nextError !== prevError) {
      const repo = repoById.get(plan.repo_id);
      const label = repo ? `${repo.owner}/${repo.name}` : `repo #${plan.repo_id}`;
      log(`ERROR fetch ${label}: ${nextError}`);
      continue;
    }
    if (!nextError && prevError) {
      const repo = repoById.get(plan.repo_id);
      const label = repo ? `${repo.owner}/${repo.name}` : `repo #${plan.repo_id}`;
      log(`${label}: fetch recovered.`);
    }
  }

  maybeNotifyProjectUpdates(source, notify);
}

async function refreshAll(options = {}) {
  const forceCheck = !!options.forceCheck;
  const notify = !!options.notify;
  const source = String(options.source || "refresh");
  if (state.refreshInFlight) {
    return;
  }

  state.refreshInFlight = withBusy(async () => {
    const profile = activeProfile();
    if (!profile) {
      state.repos = [];
      state.plans = [];
      state.planByRepoId = new Map();
      state.lastCheckedAt = null;
      render();
      return;
    }
    log("Refreshing…");
    try {
      await setBackendActiveProfile();
      await loadRepos();
      render();
      const allowInitial = !state.initialAutoCheckDone;
      const shouldCheckUpdates = forceCheck || hasGithubAuthToken() || allowInitial;

      if (shouldCheckUpdates) {
        await checkUpdates({ notify, source });
        await maybePollSelfUpdateInfo({ force: forceCheck || source === "startup", notify });
        state.initialAutoCheckDone = true;
        state.loggedNoTokenAutoSkip = false;
      } else if (!state.loggedNoTokenAutoSkip) {
        log("Skipping auto update check (no GitHub token). Use “Check for updates” manually.");
        state.loggedNoTokenAutoSkip = true;
      }
      render();
      log(`Loaded ${state.repos.length} repo(s).`);
    } catch (e) {
      log(`ERROR refresh: ${e.message}`);
    }
  });

  try {
    await state.refreshInFlight;
  } finally {
    state.refreshInFlight = null;
  }
}

async function updateAll() {
  const wowDir = currentWowDirStrict();
  if (!wowDir) return;

  const updatable = state.repos.filter((repo) => canUpdateRepo(repo));
  if (!updatable.length) {
    log("No updates available.");
    return;
  }

  log("Updating mods/addons…");
  await withBusy(async () => {
    let updated = 0;
    let failed = 0;
    const addonConflicts = [];
    const limit = Math.max(1, Math.min(MAX_PARALLEL_UPDATES, updatable.length));
    let nextIndex = 0;
    const workers = Array.from({ length: limit }, async () => {
      while (true) {
        const idx = nextIndex++;
        if (idx >= updatable.length) return;
        const repo = updatable[idx];
        try {
          const result = await safeInvoke("wuddle_update_repo", {
            id: repo.id,
            wowDir,
            ...installOptions(),
          });
          const msg = logOperationResult(result);
          if (/^Updated\b/i.test(msg)) updated += 1;
        } catch (e) {
          const conflict = isAddonRepo(repo) ? parseAddonConflictError(e.message) : null;
          if (conflict) {
            addonConflicts.push({ repo, conflict });
            continue;
          }
          failed += 1;
          log(`ERROR update ${repo.owner}/${repo.name}: ${e.message}`);
        }
      }
    });
    await Promise.all(workers);

    for (const { repo, conflict } of addonConflicts) {
      const proceed = await confirmAddonConflict(repo, conflict);
      if (!proceed) {
        log(`${repo.owner}/${repo.name}: cancelled install (existing addon files kept).`);
        continue;
      }
      try {
        const result = await safeInvoke("wuddle_update_repo", {
          id: repo.id,
          wowDir,
          ...installOptions({ replaceAddonConflicts: true }),
        });
        const msg = logOperationResult(result);
        if (/^Updated\b/i.test(msg)) updated += 1;
      } catch (e) {
        failed += 1;
        log(`ERROR update ${repo.owner}/${repo.name}: ${e.message}`);
      }
    }

    if (failed > 0) log(`Done. Updated ${updated} repo(s); ${failed} failed.`);
    else log(`Done. Updated ${updated} repo(s).`);
    await refreshAll({ forceCheck: true });
  });
}

async function handleUpdateAction() {
  const action = getUpdateActionState();
  if (action.mode === "update_all") {
    await updateAll();
    return;
  }
  await refreshAll({ forceCheck: true, notify: true, source: "manual" });
}

async function addRepo(urlOverride = null, modeOverride = null, label = "") {
  if (!ensureActiveProfile()) return false;
  const url = String(urlOverride ?? $("repoUrl").value ?? "").trim();
  const defaultMode = state.projectView === "addons" ? "addon_git" : "auto";
  const mode = String(modeOverride ?? $("mode").value ?? defaultMode);

  if (!url) {
    log("ERROR: Repo URL is empty.");
    return false;
  }

  if (isSuperWoWUrl(url)) {
    const proceed = await confirmSuperWoWRisk();
    if (!proceed) {
      log("Add cancelled for SuperWoW.");
      return false;
    }
  }

  log(label ? `Adding ${label}…` : "Adding repo…");
  return await withBusy(async () => {
    try {
      const knownIds = new Set(state.repos.map((r) => r.id));
      const id = await safeInvoke("wuddle_add_repo", { url, mode }, { timeoutMs: 30000 });
      if (knownIds.has(id)) {
        log(label ? `${label} is already tracked (id=${id}).` : `Repo already tracked (id=${id}).`);
      } else {
        log(label ? `Added ${label} (id=${id}).` : `Added repo id=${id}`);
      }
      if (!urlOverride) $("repoUrl").value = "";
      await loadRepos();
      render();
      const addedRepo = state.repos.find((repo) => repo.id === id) || null;
      const wowDir = currentWowDirStrict();
      if (!wowDir) {
        log("Install skipped. Set a valid WoW path for this instance first.");
      } else {
        const repoLabel = addedRepo ? `${addedRepo.owner}/${addedRepo.name}` : `repo id=${id}`;
        log(`Installing ${repoLabel}...`);
        try {
          const result = await safeInvoke("wuddle_update_repo", {
            id,
            wowDir,
            ...installOptions(),
          });
          if (result) {
            logOperationResult(result);
          } else {
            const reinstallResult = await safeInvoke("wuddle_reinstall_repo", {
              id,
              wowDir,
              ...installOptions(),
            });
            logOperationResult(reinstallResult);
          }
        } catch (e) {
          const conflict = parseAddonConflictError(e.message);
          if (conflict) {
            const proceed = await confirmAddonConflict(
              addedRepo || { owner: "repo", name: String(id) },
              conflict,
            );
            if (!proceed) {
              log(`${repoLabel}: cancelled install (existing addon files kept).`);
            } else {
              try {
                const retryResult = await safeInvoke("wuddle_update_repo", {
                  id,
                  wowDir,
                  ...installOptions({ replaceAddonConflicts: true }),
                });
                if (retryResult) {
                  logOperationResult(retryResult);
                } else {
                  const reinstallRetryResult = await safeInvoke("wuddle_reinstall_repo", {
                    id,
                    wowDir,
                    ...installOptions({ replaceAddonConflicts: true }),
                  });
                  logOperationResult(reinstallRetryResult);
                }
              } catch (retryErr) {
                log(`ERROR install ${repoLabel}: ${retryErr.message}`);
              }
            }
          } else {
            log(`ERROR install ${repoLabel}: ${e.message}`);
          }
        }
      }
      await refreshAll({ forceCheck: true });
      return true;
    } catch (e) {
      log(label ? `ERROR add ${label}: ${e.message}` : `ERROR add: ${e.message}`);
      return false;
    }
  });
}

async function copyLogToClipboard() {
  const text = $("log").textContent || "";
  if (!text.trim()) {
    log("Log is empty.");
    return;
  }

  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.setAttribute("readonly", "");
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.focus();
      ta.select();
      const ok = document.execCommand("copy");
      ta.remove();
      if (!ok) {
        throw new Error("Clipboard copy command failed");
      }
    }
    log("Copied log to clipboard.");
  } catch (e) {
    log(`ERROR copy log: ${e.message || e}`);
  }
}

async function retryFailedFetches() {
  if (!ensureActiveProfile()) return;
  const before = reposForCurrentView().filter((r) => !!getPlanForRepo(r.id)?.error).length;
  if (!before) {
    log("No failed fetches to retry.");
    return;
  }

  log(`Retrying failed fetches (${before})…`);
  await withBusy(async () => {
    try {
      await checkUpdates();
      render();
      const after = reposForCurrentView().filter((r) => !!getPlanForRepo(r.id)?.error).length;
      if (!after) {
        log("All fetch errors cleared.");
      } else {
        log(`${after} fetch error(s) remain.`);
      }
    } catch (e) {
      log(`ERROR retry failed: ${e.message}`);
    }
  });
}

function setLogLevel(level) {
  const allowed = new Set(["all", "info", "error"]);
  state.logLevel = allowed.has(level) ? level : "all";
  localStorage.setItem(LOG_LEVEL_KEY, state.logLevel);
  renderLog();
}

function setLogQuery(value) {
  state.logQuery = String(value ?? "");
  renderLog();
}

function setLogWrap(enabled) {
  state.logWrap = !!enabled;
  localStorage.setItem(LOG_WRAP_KEY, state.logWrap ? "true" : "false");
  renderLog();
}

function setLogAutoscroll(enabled) {
  state.logAutoScroll = !!enabled;
  localStorage.setItem(LOG_AUTOSCROLL_KEY, state.logAutoScroll ? "true" : "false");
  renderLog();
}

function setFilter(filter) {
  const allowed = new Set(["all", "updates", "errors", "disabled"]);
  state.filter = allowed.has(filter) ? filter : "all";
  render();
}

function toggleSort(sortKey) {
  const allowed = new Set(["name", "current", "latest", "status"]);
  if (!allowed.has(sortKey)) return;
  if (state.sortKey === sortKey) {
    state.sortDir = state.sortDir === "asc" ? "desc" : "asc";
  } else {
    state.sortKey = sortKey;
    state.sortDir = "asc";
  }
  render();
}

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
  const isActive = profileId === state.activeProfileId;
  const ok = saveInstanceSettingsFromDialog();
  if (!ok) return;
  $("dlgInstanceSettings").close();
  if (isActive) {
    await refreshAll();
  }
});

$("optSymlinks").addEventListener("change", saveOptionFlags);
$("optXattr").addEventListener("change", saveOptionFlags);
$("optClock12").addEventListener("change", saveOptionFlags);
$("optTheme").addEventListener("change", saveOptionFlags);
$("optFrizFont").addEventListener("change", saveOptionFlags);
$("optAutoCheck").addEventListener("change", saveOptionFlags);
$("optAutoCheckMinutes").addEventListener("change", saveOptionFlags);
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

loadSettings();
ensureThemedSelect($("mode"));
ensureThemedSelect($("instanceSettingsLaunchMethod"));
ensureThemedSelect($("optTheme"));
renderAddPresets();
renderBusy();
renderLog();
renderGithubAuth(null);
renderAboutInfo();
void refreshGithubAuthStatus();
log("Ready.");
refreshAll({ notify: true, source: "startup" });
