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
const OPT_SYMLINKS_KEY = "wuddle.opt.symlinks";
const OPT_XATTR_KEY = "wuddle.opt.xattr";
const OPT_CLOCK12_KEY = "wuddle.opt.clock12";
const LOG_WRAP_KEY = "wuddle.log.wrap";
const LOG_AUTOSCROLL_KEY = "wuddle.log.autoscroll";
const LOG_LEVEL_KEY = "wuddle.log.level";
const WUDDLE_REPO_URL = "https://github.com/ZythDr/Wuddle";
const WUDDLE_RELEASES_URL = "https://github.com/ZythDr/Wuddle/releases";
const WUDDLE_RELEASES_API_URL = "https://api.github.com/repos/ZythDr/Wuddle/releases/latest";

const state = {
  repos: [],
  plans: [],
  planByRepoId: new Map(),
  openMenuRepoId: null,
  tab: "projects",
  pending: 0,
  refreshInFlight: null,
  removeTargetRepo: null,
  removeSelectedIds: [],
  removeTargetProfile: null,
  githubAuth: null,
  initialAutoCheckDone: false,
  loggedNoTokenAutoSkip: false,
  filter: "all",
  sortKey: "name",
  sortDir: "asc",
  lastCheckedAt: null,
  lastCheckedByRepo: new Map(),
  clock12: false,
  logLines: [],
  logLevel: "all",
  logQuery: "",
  logAutoScroll: true,
  logWrap: false,
  selectedRepoIds: new Set(),
  profiles: [],
  activeProfileId: "default",
  authHealthSeenSession: false,
  authHealthActiveIssue: "",
  presetExpanded: new Set(),
  aboutInfo: null,
  aboutLoaded: false,
  aboutRefreshedAt: null,
  aboutLatestVersion: null,
};

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
      "No instances configured yet. Add an instance, then choose the folder where wow.exe is located.";
    host.appendChild(empty);
    return;
  }

  for (const profile of state.profiles) {
    const row = document.createElement("div");
    row.className = `instance-row${profile.id === state.activeProfileId ? " active" : ""}`;

    const nameField = document.createElement("label");
    nameField.className = "field";
    const nameLabel = document.createElement("div");
    nameLabel.className = "label";
    nameLabel.textContent = "Name";
    const nameInput = document.createElement("input");
    nameInput.placeholder = "WoW1";
    nameInput.value = profile.name || "";
    nameInput.addEventListener("input", () => {
      saveProfileName(profile.id, nameInput.value);
    });
    nameField.appendChild(nameLabel);
    nameField.appendChild(nameInput);

    const pathField = document.createElement("label");
    pathField.className = "field grow";
    const pathLabel = document.createElement("div");
    pathLabel.className = "label";
    pathLabel.textContent = "Path";
    const pathInput = document.createElement("input");
    pathInput.placeholder = "/path/to/WoW";
    pathInput.value = profile.wowDir || "";
    pathInput.addEventListener("input", () => {
      saveProfileWowDir(profile.id, pathInput.value);
    });
    pathInput.addEventListener("change", async () => {
      if (profile.id === state.activeProfileId) {
        await refreshAll();
      }
    });
    pathField.appendChild(pathLabel);
    pathField.appendChild(pathInput);

    const actions = document.createElement("div");
    actions.className = "instance-actions";

    const chooseBtn = document.createElement("button");
    chooseBtn.className = "btn";
    chooseBtn.textContent = "Choose...";
    chooseBtn.addEventListener("click", async () => {
      await pickWowDirForProfile(profile.id);
    });

    const openBtn = document.createElement("button");
    openBtn.className = "btn";
    openBtn.textContent = "Open Directory";
    openBtn.disabled = !profile.wowDir;
    openBtn.addEventListener("click", async () => {
      await openPath(profile.wowDir);
    });

    const activateBtn = document.createElement("button");
    activateBtn.className = "btn";
    activateBtn.textContent = profile.id === state.activeProfileId ? "Active" : "Open";
    activateBtn.disabled = profile.id === state.activeProfileId;
    activateBtn.addEventListener("click", async () => {
      await selectProfile(profile.id);
    });

    const removeBtn = document.createElement("button");
    removeBtn.className = "btn danger";
    removeBtn.textContent = "Remove";
    removeBtn.addEventListener("click", async () => {
      openRemoveInstanceDialog(profile);
    });

    actions.appendChild(chooseBtn);
    actions.appendChild(openBtn);
    actions.appendChild(activateBtn);
    actions.appendChild(removeBtn);

    row.appendChild(nameField);
    row.appendChild(pathField);
    row.appendChild(actions);
    host.appendChild(row);
  }
}

function renderProfileTabs() {
  const host = $("profileTabs");
  const divider = $("topTabsDivider");
  host.innerHTML = "";
  for (const profile of state.profiles) {
    const btn = document.createElement("button");
    btn.className = "tab-btn";
    if (state.tab === "projects" && profile.id === state.activeProfileId) {
      btn.classList.add("active");
    }
    btn.textContent = profile.name || "WoW";
    btn.title = profile.wowDir || "";
    btn.addEventListener("click", async () => {
      await selectProfile(profile.id);
    });
    host.appendChild(btn);
  }
  if (divider) {
    divider.classList.toggle("hidden", state.profiles.length === 0);
  }
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

function setTheme() {
  document.documentElement.setAttribute("data-theme", "dark");
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
      state.aboutLatestVersion = await fetchLatestWuddleReleaseTag();
    } catch (latestErr) {
      state.aboutLatestVersion = null;
      log(`ERROR latest version check: ${latestErr.message || latestErr}`);
    }
    state.aboutLoaded = true;
    state.aboutRefreshedAt = new Date();
    renderAboutInfo();
    const latestHint = state.aboutLatestVersion ? "" : " Latest version unavailable.";
    setAboutStatus(`Detected at ${formatTime(state.aboutRefreshedAt)}.${latestHint}`, "status-ok");
  } catch (e) {
    setAboutStatus(`Could not load application details: ${e.message}`, "status-warn");
    log(`ERROR about: ${e.message}`);
  }
}

function setTab(tab) {
  if (tab === "options") state.tab = "options";
  else if (tab === "logs") state.tab = "logs";
  else if (tab === "about") state.tab = "about";
  else state.tab = "projects";
  localStorage.setItem(TAB_KEY, state.tab);

  $("panelProjects").classList.toggle("hidden", state.tab !== "projects");
  $("panelOptions").classList.toggle("hidden", state.tab !== "options");
  $("panelLogs").classList.toggle("hidden", state.tab !== "logs");
  $("panelAbout").classList.toggle("hidden", state.tab !== "about");

  $("tabOptions").classList.toggle("active", state.tab === "options");
  $("tabLogs").classList.toggle("active", state.tab === "logs");
  $("tabAbout").classList.toggle("active", state.tab === "about");
  renderProfileTabs();

  if (state.tab === "options") {
    renderInstanceList();
    void refreshGithubAuthStatus();
  } else if (state.tab === "about") {
    void refreshAboutInfo();
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

  setTheme();

  const symlinks = localStorage.getItem(OPT_SYMLINKS_KEY) === "true";
  const xattr = localStorage.getItem(OPT_XATTR_KEY) === "true";
  const clock12 = localStorage.getItem(OPT_CLOCK12_KEY) === "true";
  $("optSymlinks").checked = symlinks;
  $("optXattr").checked = xattr;
  $("optClock12").checked = clock12;
  state.clock12 = clock12;

  const savedTab = localStorage.getItem(TAB_KEY) || "projects";
  setTab(savedTab);

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
}

function saveOptionFlags() {
  localStorage.setItem(OPT_SYMLINKS_KEY, $("optSymlinks").checked ? "true" : "false");
  localStorage.setItem(OPT_XATTR_KEY, $("optXattr").checked ? "true" : "false");
  localStorage.setItem(OPT_CLOCK12_KEY, $("optClock12").checked ? "true" : "false");
  state.clock12 = $("optClock12").checked;
  renderLastChecked();
  render();
  renderLog();
}

function installOptions() {
  return {
    useSymlinks: $("optSymlinks").checked,
    setXattrComment: $("optXattr").checked,
  };
}

function readWowDir() {
  const profile = activeProfile();
  return profile?.wowDir?.trim() || "";
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
  state.selectedRepoIds.clear();
  state.lastCheckedByRepo = new Map();
  state.lastCheckedAt = null;
  const selected = activeProfile();
  if (selected?.wowDir) {
    localStorage.setItem(WOW_KEY, selected.wowDir);
  }
  persistProfiles();
  renderProfileTabs();
  renderInstanceList();
  setTab("projects");
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
  state.profiles.push({ id, name, wowDir });
  state.activeProfileId = id;
  persistProfiles();
  renderProfileTabs();
  renderInstanceList();
  await setBackendActiveProfile();
  log(`Created instance ${name}.`);
  render();
}

async function removeInstance(profileId) {
  const id = normalizeProfileId(profileId);
  const before = state.profiles.length;
  state.profiles = state.profiles.filter((p) => p.id !== id);
  if (state.profiles.length === before) return;

  if (state.activeProfileId === id) {
    state.activeProfileId = state.profiles[0]?.id || "";
  }
  state.selectedRepoIds.clear();
  state.lastCheckedByRepo = new Map();
  state.lastCheckedAt = null;
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

function setGithubAuthStatus(message, kind = "") {
  const statusLine = $("ghAuthStatus");
  statusLine.classList.remove("status-ok", "status-warn");
  if (kind) statusLine.classList.add(kind);
  statusLine.textContent = message;
}

function pruneSelectedRepos() {
  const valid = new Set(state.repos.map((repo) => repo.id));
  for (const id of state.selectedRepoIds) {
    if (!valid.has(id)) {
      state.selectedRepoIds.delete(id);
    }
  }
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
  if (plan.error) return "Update is unavailable because release fetch failed.";
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

function sortedFilteredRepos() {
  const dir = state.sortDir === "desc" ? -1 : 1;
  const list = state.repos.filter(matchesFilter);

  list.sort((a, b) => {
    if (state.sortKey === "name") {
      return dir * a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    }
    if (state.sortKey === "current") {
      const av = versionLabel(getPlanForRepo(a.id)?.current);
      const bv = versionLabel(getPlanForRepo(b.id)?.current);
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

function renderSortHeaders() {
  document.querySelectorAll("#repoThead .th.sortable").forEach((th) => {
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
  const total = state.repos.length;
  const enabled = state.repos.filter((repo) => repo.enabled).length;
  const disabled = total - enabled;
  const updates = state.repos.filter((repo) => {
    const plan = getPlanForRepo(repo.id);
    return repo.enabled && !!plan?.has_update && !plan?.error;
  }).length;
  const errors = state.repos.filter((repo) => !!getPlanForRepo(repo.id)?.error).length;
  const rateLimited = state.repos.some((repo) => {
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
    // reveal_item_in_dir does not require filesystem scope entries like open_path does.
    await safeInvoke("plugin:opener|reveal_item_in_dir", { paths: [target] });
  } catch (revealErr) {
    try {
      await safeInvoke("plugin:opener|open_path", { path: target });
    } catch (openErr) {
      const r = revealErr?.message || String(revealErr);
      const o = openErr?.message || String(openErr);
      log(`ERROR open dir: ${r} (fallback failed: ${o})`);
    }
  }
}

function openRemoveDialog(repo) {
  state.removeTargetRepo = repo;
  $("removeRepoName").textContent = `${repo.owner}/${repo.name}`;
  $("removeLocalFiles").checked = false;
  $("dlgRemove").showModal();
}

function openRemoveSelectedDialog() {
  const selected = state.repos.filter((repo) => state.selectedRepoIds.has(repo.id));
  if (!selected.length) return;
  state.removeSelectedIds = selected.map((repo) => repo.id);

  const count = selected.length;
  $("removeSelectedCount").textContent = `${count} project${count === 1 ? "" : "s"} selected`;
  const preview = selected
    .slice(0, 4)
    .map((repo) => `${repo.owner}/${repo.name}`)
    .join(", ");
  const more = count > 4 ? ` +${count - 4} more` : "";
  $("removeSelectedNames").textContent = preview ? `${preview}${more}` : "";
  $("removeSelectedLocalFiles").checked = false;
  $("dlgRemoveSelected").showModal();
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

async function confirmRemoveSelected() {
  const selectedIds = Array.isArray(state.removeSelectedIds)
    ? state.removeSelectedIds.filter((id) => state.repos.some((repo) => repo.id === id))
    : [];
  if (!selectedIds.length) {
    state.removeSelectedIds = [];
    $("dlgRemoveSelected").close();
    return;
  }

  const removeLocalFiles = $("removeSelectedLocalFiles").checked;
  const wowDir = removeLocalFiles ? readWowDir() : null;
  if (removeLocalFiles && !wowDir) {
    log("ERROR remove selected: WoW directory is required to remove local files.");
    return;
  }

  await withBusy(async () => {
    const repoById = new Map(state.repos.map((repo) => [repo.id, repo]));
    let removed = 0;
    let failed = 0;
    for (const id of selectedIds) {
      const repo = repoById.get(id);
      const label = repo ? `${repo.owner}/${repo.name}` : `repo id=${id}`;
      try {
        const msg = await safeInvoke(
          "wuddle_remove_repo",
          { id, removeLocalFiles, wowDir },
          { timeoutMs: 45000 },
        );
        state.selectedRepoIds.delete(id);
        log(msg);
        removed += 1;
      } catch (e) {
        failed += 1;
        log(`ERROR remove ${label}: ${e.message}`);
      }
    }
    state.removeSelectedIds = [];
    $("dlgRemoveSelected").close();
    if (failed > 0) log(`Removed ${removed} selected project(s); ${failed} failed.`);
    else log(`Removed ${removed} selected project(s).`);
    await refreshAll();
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
      const msg = await safeInvoke("wuddle_update_repo", {
        id: repo.id,
        wowDir,
        ...installOptions(),
      });
      log(msg);
      await refreshAll({ forceCheck: true });
    } catch (e) {
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
      const msg = await safeInvoke("wuddle_reinstall_repo", {
        id: repo.id,
        wowDir,
        ...installOptions(),
      });
      log(msg);
      await refreshAll({ forceCheck: true });
    } catch (e) {
      log(`ERROR reinstall ${repo.owner}/${repo.name}: ${e.message}`);
    }
  });
}

function render() {
  renderAddPresets();
  const host = $("repoRows");
  host.innerHTML = "";
  const profile = activeProfile();
  const hasProfile = !!profile;
  $("btnAddOpen").disabled = !hasProfile;
  $("btnUpdateAll").disabled = !hasProfile;

  renderFilterButtons();
  renderSortHeaders();
  renderLastChecked();
  renderProjectStatusStrip();
  renderGithubAuthHealth();
  const failedCount = state.repos.filter((r) => !!getPlanForRepo(r.id)?.error).length;
  $("btnRetryFailed").classList.toggle("hidden", failedCount === 0);
  $("btnRetryFailed").disabled = failedCount === 0 || !hasProfile;
  $("btnRetryFailed").title = failedCount ? `Retry ${failedCount} failed fetch(es)` : "No failed fetches";

  const selectedCount = Array.from(state.selectedRepoIds).filter((id) =>
    state.repos.some((repo) => repo.id === id),
  ).length;
  const updateActionState = getUpdateActionState();
  const updateActionBtn = $("btnUpdateAll");
  const removeSelectedBtn = $("btnRemoveSelected");
  updateActionBtn.textContent = updateActionState.label;
  updateActionBtn.title = updateActionState.title;
  updateActionBtn.classList.toggle("primary", updateActionState.primary);
  updateActionBtn.disabled = !hasProfile || updateActionState.disabled;
  if (removeSelectedBtn) {
    removeSelectedBtn.disabled = !hasProfile || selectedCount === 0;
    removeSelectedBtn.textContent =
      selectedCount > 0 ? `Remove Selected (${selectedCount})` : "Remove Selected";
    removeSelectedBtn.title =
      selectedCount > 0
        ? `Remove selected projects (${selectedCount})`
        : hasProfile
          ? "Select one or more projects first."
          : "Add an instance in Options first.";
  }
  if (!hasProfile) {
    updateActionBtn.textContent = "Check for updates";
    updateActionBtn.title = "Add an instance in Options first.";
    if (removeSelectedBtn) {
      removeSelectedBtn.textContent = "Remove Selected";
    }
  }

  const visibleRepos = sortedFilteredRepos();
  const visibleSelectedCount = visibleRepos.filter((repo) => state.selectedRepoIds.has(repo.id)).length;
  const selectAll = $("selectAllRepos");
  selectAll.checked = visibleRepos.length > 0 && visibleSelectedCount === visibleRepos.length;
  selectAll.indeterminate = visibleSelectedCount > 0 && visibleSelectedCount < visibleRepos.length;
  selectAll.disabled = !hasProfile;

  if (!hasProfile) {
    return;
  }

  if (!visibleRepos.length) {
    const div = document.createElement("div");
    div.className = "empty";
    div.textContent = state.repos.length
      ? "No projects match the current filter."
      : "No repos yet. Click “＋ Add”.";
    host.appendChild(div);
    return;
  }

  for (const r of visibleRepos) {
    const st = repoStatus(r);

    const row = document.createElement("div");
    row.className = "trow";

    const selectCell = document.createElement("div");
    selectCell.className = "select-col";
    const selectInput = document.createElement("input");
    selectInput.type = "checkbox";
    selectInput.checked = state.selectedRepoIds.has(r.id);
    selectInput.setAttribute("aria-label", `Select ${r.owner}/${r.name}`);
    selectInput.addEventListener("change", (ev) => {
      ev.stopPropagation();
      if (selectInput.checked) state.selectedRepoIds.add(r.id);
      else state.selectedRepoIds.delete(r.id);
      render();
    });
    selectCell.appendChild(selectInput);

    const nameCell = document.createElement("div");
    nameCell.className = "namecell";

    const nameMain = document.createElement("div");
    const forgeLabel = displayForge(r);
    const plan = getPlanForRepo(r.id);
    const checkedAt = state.lastCheckedByRepo.get(r.id);
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
    nameSub.textContent = `${r.owner} • ${forgeLabel} • ${r.mode}${r.enabled ? "" : " • disabled"} • checked ${formatTime(checkedAt)}`;

    nameHeader.appendChild(nameLink);
    nameMain.appendChild(nameHeader);
    nameMain.appendChild(nameSub);
    nameCell.appendChild(nameMain);

    const status = document.createElement("div");
    status.innerHTML = `<span class="badge ${st.kind}">${escapeHtml(st.text)}</span>`;
    if (plan?.error) {
      status.title = plan.error;
    }

    const currentCell = document.createElement("div");
    currentCell.className = "version-cell";
    currentCell.textContent = versionLabel(plan?.current);

    const latestCell = document.createElement("div");
    latestCell.className = "version-cell";
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
    enabledCell.className = "enabled-col";
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
    menu.appendChild(del);
    menuWrap.appendChild(menuBtn);
    menuWrap.appendChild(menu);

    actions.appendChild(updateBtn);
    actions.appendChild(menuWrap);

    row.appendChild(selectCell);
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

  const selectedRepos = state.repos.filter((repo) => state.selectedRepoIds.has(repo.id));
  const selectedCount = selectedRepos.length;
  const selectedUpdatableCount = selectedRepos.filter((repo) => canUpdateRepo(repo)).length;
  const updatableCount = state.repos.filter((repo) => canUpdateRepo(repo)).length;

  if (selectedCount > 0) {
    if (selectedUpdatableCount > 0) {
      return {
        mode: "update_selected",
        label: `Update (${selectedUpdatableCount})`,
        title: `Update selected projects with available updates (${selectedUpdatableCount}).`,
        primary: true,
        disabled: false,
      };
    }
    return {
      mode: "reinstall_selected",
      label: `Reinstall Selected (${selectedCount})`,
      title: `Reinstall selected projects (${selectedCount}).`,
      primary: true,
      disabled: false,
    };
  }

  if (updatableCount > 0) {
    return {
      mode: "update_all",
      label: `Update (${updatableCount})`,
      title: `Update all projects with available updates (${updatableCount}).`,
      primary: true,
      disabled: false,
    };
  }

  return {
    mode: "check",
    label: "Check for updates",
    title: "Check all tracked projects for updates.",
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
  state.repos = await safeInvoke("wuddle_list_repos", {}, { timeoutMs: 12000 });
  pruneSelectedRepos();
}

async function checkUpdates() {
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
  for (const plan of next) {
    state.lastCheckedByRepo.set(plan.repo_id, checkedAt);
  }

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
}

async function refreshAll(options = {}) {
  const forceCheck = !!options.forceCheck;
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
      state.lastCheckedByRepo = new Map();
      render();
      return;
    }
    log("Refreshing…");
    try {
      await setBackendActiveProfile();
      await loadRepos();
      const allowInitial = !state.initialAutoCheckDone;
      const shouldCheckUpdates = forceCheck || hasGithubAuthToken() || allowInitial;

      if (shouldCheckUpdates) {
        await checkUpdates();
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

  log("Updating all…");
  await withBusy(async () => {
    try {
      const msg = await safeInvoke("wuddle_update_all", {
        wowDir,
        ...installOptions(),
      });
      log(msg);
      await refreshAll({ forceCheck: true });
    } catch (e) {
      log(`ERROR update: ${e.message}`);
    }
  });
}

async function updateSelected() {
  const wowDir = currentWowDirStrict();
  if (!wowDir) return;

  const selected = state.repos.filter((repo) => state.selectedRepoIds.has(repo.id));
  if (!selected.length) {
    log("No selected projects.");
    return;
  }

  const updatable = selected.filter((repo) => canUpdateRepo(repo));
  if (!updatable.length) {
    log("No selected projects have updates available.");
    return;
  }

  log(`Updating selected (${updatable.length})…`);
  await withBusy(async () => {
    let updated = 0;
    let failed = 0;
    for (const repo of updatable) {
      try {
        const msg = await safeInvoke("wuddle_update_repo", {
          id: repo.id,
          wowDir,
          ...installOptions(),
        });
        if (/^Updated\b/i.test(msg)) updated += 1;
        log(msg);
      } catch (e) {
        failed += 1;
        log(`ERROR update selected ${repo.owner}/${repo.name}: ${e.message}`);
      }
    }
    if (failed > 0) {
      log(`Done. Updated ${updated} selected repo(s); ${failed} failed.`);
    } else {
      log(`Done. Updated ${updated} selected repo(s).`);
    }
    await refreshAll({ forceCheck: true });
  });
}

async function reinstallSelected() {
  const wowDir = currentWowDirStrict();
  if (!wowDir) return;

  const selected = state.repos.filter((repo) => state.selectedRepoIds.has(repo.id));
  if (!selected.length) {
    log("No selected projects.");
    return;
  }

  log(`Reinstalling selected (${selected.length})…`);
  await withBusy(async () => {
    let reinstalled = 0;
    let failed = 0;
    for (const repo of selected) {
      try {
        const msg = await safeInvoke("wuddle_reinstall_repo", {
          id: repo.id,
          wowDir,
          ...installOptions(),
        });
        if (/^Reinstalled\b/i.test(msg)) reinstalled += 1;
        log(msg);
      } catch (e) {
        failed += 1;
        log(`ERROR reinstall selected ${repo.owner}/${repo.name}: ${e.message}`);
      }
    }
    if (failed > 0) {
      log(`Done. Reinstalled ${reinstalled} selected repo(s); ${failed} failed.`);
    } else {
      log(`Done. Reinstalled ${reinstalled} selected repo(s).`);
    }
    await refreshAll({ forceCheck: true });
  });
}

async function handleUpdateAction() {
  const action = getUpdateActionState();
  if (action.mode === "update_selected") {
    await updateSelected();
    return;
  }
  if (action.mode === "update_all") {
    await updateAll();
    return;
  }
  if (action.mode === "reinstall_selected") {
    await reinstallSelected();
    return;
  }
  await refreshAll({ forceCheck: true });
}

function toggleSelectAllVisible() {
  const visibleRepos = sortedFilteredRepos();
  if (!visibleRepos.length) return;
  const allVisibleSelected = visibleRepos.every((repo) => state.selectedRepoIds.has(repo.id));
  for (const repo of visibleRepos) {
    if (allVisibleSelected) state.selectedRepoIds.delete(repo.id);
    else state.selectedRepoIds.add(repo.id);
  }
  render();
}

async function addRepo(urlOverride = null, modeOverride = null, label = "") {
  if (!ensureActiveProfile()) return false;
  const url = String(urlOverride ?? $("repoUrl").value ?? "").trim();
  const mode = String(modeOverride ?? $("mode").value ?? "auto");

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
      await refreshAll();
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
  const before = state.repos.filter((r) => !!getPlanForRepo(r.id)?.error).length;
  if (!before) {
    log("No failed fetches to retry.");
    return;
  }

  log(`Retrying failed fetches (${before})…`);
  await withBusy(async () => {
    try {
      await checkUpdates();
      render();
      const after = state.repos.filter((r) => !!getPlanForRepo(r.id)?.error).length;
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
$("btnRemoveSelected").addEventListener("click", openRemoveSelectedDialog);
$("btnAddInstance").addEventListener("click", async () => {
  await addInstance();
});
$("btnRetryFailed").addEventListener("click", retryFailedFetches);
$("selectAllRepos").addEventListener("change", toggleSelectAllVisible);

$("tabOptions").addEventListener("click", () => setTab("options"));
$("tabLogs").addEventListener("click", () => setTab("logs"));
$("tabAbout").addEventListener("click", () => setTab("about"));

$("optSymlinks").addEventListener("change", saveOptionFlags);
$("optXattr").addEventListener("change", saveOptionFlags);
$("optClock12").addEventListener("change", saveOptionFlags);
$("btnConnectGithub").addEventListener("click", connectGithub);
$("btnSaveGithubToken").addEventListener("click", saveGithubToken);
$("btnClearGithubToken").addEventListener("click", clearGithubToken);
$("btnAboutRefresh").addEventListener("click", () => {
  void refreshAboutInfo({ force: true });
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
$("btnAddOpen").addEventListener("click", () => dlgAdd.showModal());
$("btnAdd").addEventListener("click", async (ev) => {
  ev.preventDefault();
  const ok = await addRepo();
  if (ok) dlgAdd.close();
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
$("btnRemoveSelectedConfirm").addEventListener("click", async (ev) => {
  ev.preventDefault();
  await confirmRemoveSelected();
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
bindDialogOutsideToClose($("dlgRemoveSelected"));
bindDialogOutsideToClose($("dlgRemoveInstance"));

document.addEventListener("click", (ev) => {
  if (state.openMenuRepoId === null) return;
  if (!(ev.target instanceof Element)) return;
  if (ev.target.closest(".menu-wrap")) return;
  closeActionsMenu();
});

document.addEventListener("keydown", (ev) => {
  if (ev.key !== "Escape") return;
  if (state.openMenuRepoId === null) return;
  closeActionsMenu();
});

window.addEventListener("resize", () => {
  if (state.openMenuRepoId === null) return;
  requestAnimationFrame(positionOpenMenu);
});

const tableScroller = document.querySelector(".table-scroll");
if (tableScroller instanceof HTMLElement) {
  tableScroller.addEventListener("scroll", () => {
    if (state.openMenuRepoId === null) return;
    closeActionsMenu();
  });
}

loadSettings();
renderAddPresets();
renderBusy();
renderLog();
renderGithubAuth(null);
renderAboutInfo();
void refreshGithubAuthStatus();
log("Ready.");
refreshAll();
