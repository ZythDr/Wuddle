// Repo management: keys, conflicts, presets, filtering, sorting, rendering, operations

import { state, MAX_PARALLEL_UPDATES, IGNORED_ERRORS_KEY, IGNORED_UPDATES_KEY } from "./state.js";
import { CURATED_MOD_PRESETS, PRESET_CATEGORY_CLASS } from "./presets.js";
import { safeInvoke } from "./commands.js";
import { $, formatTime } from "./utils.js";
import { log } from "./logs.js";
import { withBusy, ensureThemedSelect, syncThemedSelect, rebuildThemedSelect, showToast, refreshScrollFade } from "./ui.js";
import { changelogToHtml } from "./about.js";
import { highlightCode } from "./vendor/highlight.bundle.js";
import {
  setBackendActiveProfile,
  activeProfile,
  readWowDir,
  currentWowDirStrict,
  installOptions,
  ensureActiveProfile,
  normalizeProfileId,
  normalizeProjectView,
  persistProjectViewByProfile,
} from "./profiles.js";

// ============================================================================
// Repo Identity / Key
// ============================================================================

export function repoKeyFromUrl(url) {
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

export function addonNameFromUrl(url) {
  try {
    const parsed = new URL(String(url ?? "").trim());
    const segs = parsed.pathname
      .split("/")
      .map((s) => s.trim())
      .filter(Boolean);
    if (segs.length < 2) return "";
    return segs[1].replace(/\.git$/i, "").trim();
  } catch (_) {
    return "";
  }
}

export function loadIgnoredErrors() {
  try {
    const raw = localStorage.getItem(IGNORED_ERRORS_KEY);
    if (raw) state.ignoredErrorRepoIds = new Set(JSON.parse(raw));
  } catch (_) {}
}

function persistIgnoredErrors() {
  localStorage.setItem(IGNORED_ERRORS_KEY, JSON.stringify([...state.ignoredErrorRepoIds]));
}

export function loadIgnoredUpdates() {
  try {
    const raw = localStorage.getItem(IGNORED_UPDATES_KEY);
    if (raw) state.ignoredUpdateRepoIds = new Set(JSON.parse(raw));
  } catch (_) {}
}

function persistIgnoredUpdates() {
  localStorage.setItem(IGNORED_UPDATES_KEY, JSON.stringify([...state.ignoredUpdateRepoIds]));
}

export function parseRepoUrlInfo(url) {
  const text = String(url ?? "").trim();
  try {
    const parsed = new URL(text);
    const segs = parsed.pathname
      .split("/")
      .map((s) => s.trim())
      .filter(Boolean);
    return {
      host: parsed.hostname.toLowerCase(),
      owner: segs[0] || "",
      name: (segs[1] || "").replace(/\.git$/i, ""),
      url: text,
    };
  } catch (_) {
    return { host: "", owner: "", name: "", url: text };
  }
}

export function repoKeyFromRepo(repo) {
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

// ============================================================================
// Repo Classification
// ============================================================================

export function isAddonRepo(repo) {
  const mode = String(repo?.mode || "")
    .trim()
    .toLowerCase();
  return mode === "addon" || mode === "addon_git";
}

export function reposForView(view) {
  const addonsView = normalizeProjectView(view) === "addons";
  return state.repos.filter((repo) => (addonsView ? isAddonRepo(repo) : !isAddonRepo(repo)));
}

export function reposForCurrentView() {
  return reposForView(state.projectView);
}

// ============================================================================
// Project View Management
// ============================================================================

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
  if (normalized !== state.projectView) {
    state.projectSearchQuery = "";
    const input = $("projectSearchInput");
    if (input) input.value = "";
    if (
      (normalized === "addons" && state.filter === "disabled") ||
      (normalized === "mods" && state.filter === "ignored")
    ) {
      state.filter = "all";
    }
  }
  state.projectView = normalized;
  const profile = activeProfile();
  if (persist && profile) {
    const profileId = normalizeProfileId(profile.id);
    state.projectViewByProfile[profileId] = normalized;
    persistProjectViewByProfile();
  }
  render();
}

// ============================================================================
// Conflict Detection
// ============================================================================

export function trackedAddonConflictsForUrl(url) {
  const key = repoKeyFromUrl(url);
  const addonName = addonNameFromUrl(url).toLowerCase();
  if (!addonName) return [];
  return state.repos.filter((repo) => {
    if (!isAddonRepo(repo)) return false;
    const existingKey = repoKeyFromRepo(repo);
    if (key && existingKey && key === existingKey) return false;
    const existingName = String(repo?.name || "")
      .trim()
      .toLowerCase();
    return existingName === addonName;
  });
}

export function formatAddonProbeConflictDetails(conflicts, ignoreRepoIds = new Set()) {
  const details = [];
  const conflictingRepoIds = new Set();
  let hasLocalOnlyConflicts = false;

  for (const conflict of Array.isArray(conflicts) ? conflicts : []) {
    const addonName = String(conflict?.addonName || conflict?.addon_name || "").trim() || "addon";
    const targetPath = String(conflict?.targetPath || conflict?.target_path || "").trim();
    const owners = Array.isArray(conflict?.owners) ? conflict.owners : [];

    const filteredOwners = owners.filter((owner) => {
      const repoId = Number(owner?.repoId ?? owner?.repo_id ?? NaN);
      return Number.isFinite(repoId) && !ignoreRepoIds.has(repoId);
    });

    for (const owner of filteredOwners) {
      const repoId = Number(owner?.repoId ?? owner?.repo_id ?? NaN);
      if (Number.isFinite(repoId)) conflictingRepoIds.add(repoId);
    }

    if (owners.length > 0 && filteredOwners.length === 0) {
      continue;
    }

    if (filteredOwners.length > 0) {
      const ownerText = filteredOwners
        .map((owner) => {
          const ownerName = String(owner?.owner || "").trim();
          const repoName = String(owner?.name || "").trim();
          return ownerName && repoName ? `${ownerName}/${repoName}` : "tracked addon";
        })
        .join(", ");
      details.push(
        `${addonName}${targetPath ? ` (${targetPath})` : ""} [already tracked by ${ownerText}]`,
      );
      continue;
    }

    hasLocalOnlyConflicts = true;
    details.push(
      `${addonName}${targetPath ? ` (${targetPath})` : ""} [local files already exist]`,
    );
  }

  return { details, conflictingRepoIds, hasLocalOnlyConflicts };
}

export function parseAddonConflictItems(details) {
  const rawItems = String(details || "")
    .split(";")
    .map((s) => s.trim())
    .filter(Boolean);

  const parsed = [];
  const ownerPairs = new Set();
  for (const raw of rawItems) {
    const m = raw.match(/^(.*?)(?:\s*\((.*?)\))?\s*\[(.*?)\]\s*$/);
    if (!m) {
      parsed.push({
        text: raw,
        addonName: "",
        targetPath: "",
        owners: [],
        localOnly: false,
      });
      continue;
    }
    const addonName = String(m[1] || "").trim();
    const targetPath = String(m[2] || "").trim();
    const tag = String(m[3] || "").trim();
    const owners = [];
    let localOnly = false;
    const trackedPrefix = "already tracked by ";
    if (tag.toLowerCase().startsWith(trackedPrefix)) {
      const labels = tag.slice(trackedPrefix.length).split(",");
      for (const label of labels) {
        const full = String(label || "").trim();
        if (!full) continue;
        const parts = full.split("/");
        const owner = String(parts[0] || "").trim();
        const name = String(parts[1] || "").trim();
        if (owner && name) {
          owners.push({ owner, name });
          ownerPairs.add(`${owner}/${name}`);
        } else {
          owners.push({ owner: "", name: full });
        }
      }
    } else if (/local files already exist/i.test(tag)) {
      localOnly = true;
    }
    parsed.push({
      text: raw,
      addonName,
      targetPath,
      owners,
      localOnly,
    });
  }

  return {
    items: parsed,
    ownerPairs: Array.from(ownerPairs),
  };
}

export const ADDON_CONFLICT_PREFIX = "ADDON_CONFLICT:";

export function parseAddonConflictError(message) {
  const text = String(message || "").trim();
  if (!text.startsWith(ADDON_CONFLICT_PREFIX)) return null;
  const details = text.slice(ADDON_CONFLICT_PREFIX.length).trim();
  return details || "Existing addon files were found in the destination folder.";
}

export async function confirmAddonConflict(repo, details) {
  const name = `${repo.owner}/${repo.name}`;
  const dlg = $("dlgAddonConflict");
  if (!dlg || typeof dlg.showModal !== "function") {
    return window.confirm(
      `Addon install conflict for ${name}.\n\n${details}\n\nClick OK to delete conflicting addon folders and continue, or Cancel to keep existing files and stop this install.`,
    );
  }

  const toTitle = $("addonConflictToTitle");
  const toMeta = $("addonConflictToMeta");
  const fromTitle = $("addonConflictFromTitle");
  const fromMeta = $("addonConflictFromMeta");
  const parsedRepo = parseRepoUrlInfo(repo?.url || "");
  const incomingTitle =
    String(repo?.name || "").trim() ||
    String(parsedRepo.name || "").trim() ||
    String(name || "").trim();
  const incomingOwner = String(repo?.owner || parsedRepo.owner || "").trim();
  const incomingHost = String(repo?.host || parsedRepo.host || repo?.forge || "").trim();
  if (toTitle) toTitle.textContent = incomingTitle;
  if (toMeta) {
    const parts = [incomingOwner, incomingHost].filter(Boolean);
    toMeta.textContent = parts.length ? parts.join(" • ") : "incoming addon";
  }

  const parsedDetails = parseAddonConflictItems(details);
  if (fromTitle || fromMeta) {
    if (parsedDetails.ownerPairs.length === 1) {
      const [pair] = parsedDetails.ownerPairs;
      const parts = pair.split("/");
      if (fromTitle) fromTitle.textContent = parts[1] || pair;
      if (fromMeta) fromMeta.textContent = `${parts[0] || "tracked"} • tracked addon`;
    } else if (parsedDetails.ownerPairs.length > 1) {
      if (fromTitle) fromTitle.textContent = `Multiple addons (${parsedDetails.ownerPairs.length})`;
      if (fromMeta) fromMeta.textContent = parsedDetails.ownerPairs.join(" • ");
    } else {
      if (fromTitle) fromTitle.textContent = "Existing local files";
      if (fromMeta) fromMeta.textContent = "untracked content in Interface/AddOns";
    }
  }

  const listEl = $("addonConflictList");
  if (listEl) {
    listEl.innerHTML = "";
    const items = parsedDetails.items;
    if (!items.length) {
      const li = document.createElement("li");
      li.textContent = String(details || "Existing addon files were found.");
      listEl.appendChild(li);
    } else {
      for (const item of items) {
        const li = document.createElement("li");
        const target = String(item.targetPath || "").trim();
        const addon = String(item.addonName || "").trim();
        if (addon && target) {
          li.textContent = `${addon} (${target})`;
        } else {
          li.textContent = String(item.text || "").trim();
        }
        listEl.appendChild(li);
      }
    }
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

// ============================================================================
// SuperWoW Detection
// ============================================================================

export function isSuperWoWUrl(url) {
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

export async function confirmSuperWoWRisk() {
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

// ============================================================================
// Presets
// ============================================================================

export function isPresetInstalled(preset) {
  const presetKey = repoKeyFromUrl(preset?.url);
  if (!presetKey) return false;
  return state.repos.some((repo) => repoKeyFromRepo(repo) === presetKey);
}

export function isPresetExpanded(preset) {
  return state.presetExpanded.has(preset.id);
}

export function togglePresetExpanded(preset) {
  if (isPresetExpanded(preset)) state.presetExpanded.delete(preset.id);
  else state.presetExpanded.add(preset.id);
}

// ============================================================================
// Repo Status / Data
// ============================================================================

export function getPlanForRepo(repoId) {
  return state.planByRepoId.get(repoId) || null;
}

export function versionLabel(value) {
  const v = String(value ?? "").trim();
  if (!v) return "—";
  if (v === "unknown") return "—";
  return v;
}

export function repoStatus(repo) {
  if (!repo.enabled) return { kind: "muted", text: "Disabled" };

  const plan = getPlanForRepo(repo.id);
  if (!plan) return { kind: "muted", text: "Unknown" };
  if (plan.error && !state.ignoredErrorRepoIds.has(repo.id))
    return { kind: "bad", text: "Fetch error" };
  if (plan.error && state.ignoredErrorRepoIds.has(repo.id))
    return { kind: "muted", text: "Ignored" };

  if (plan.externally_modified) return { kind: "warn", text: "Modified" };
  if (plan.repair_needed) return { kind: "warn", text: "Repair needed" };
  if (plan.has_update && state.ignoredUpdateRepoIds.has(repo.id))
    return { kind: "muted", text: "Ignored" };
  if (plan.has_update) return { kind: "warn", text: "Update available" };
  return { kind: "good", text: "Up to date" };
}

export function classifyFetchErrorHint(errorText) {
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

export function formatRepoStatusTooltip(repo, plan) {
  if (!repo.enabled) {
    return "Project is disabled in Wuddle. Enable it to include it in update/install operations.";
  }
  if (!plan) {
    return "No update data yet. Click \u201cCheck for updates\u201d.";
  }
  if (plan.error) {
    return `Fetch error: ${plan.error}\n\nHint: ${classifyFetchErrorHint(plan.error)}`;
  }
  if (plan.repair_needed) {
    return "Installed files look incomplete or mismatched. Use \u201cReinstall / Repair\u201d.";
  }
  if (plan.has_update) {
    return `Update available: ${versionLabel(plan.current)} \u2192 ${versionLabel(plan.latest)}.`;
  }
  return `Up to date at ${versionLabel(plan.latest)}.`;
}

export function displayForge(repo) {
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

// Forge icons from Simple Icons (https://simpleicons.org) — CC0 1.0 / public domain
const FORGE_ICONS = {
  github: '<svg width="18" height="18" viewBox="0 0 16 16" fill="currentColor"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>',
  gitlab: '<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="m23.6 9.593-.034-.086L20.3.981a.851.851 0 00-.336-.405.875.875 0 00-1 .054.875.875 0 00-.29.44l-2.206 6.748H7.538L5.332.818a.857.857 0 00-.29-.441.875.875 0 00-1-.054.859.859 0 00-.336.405L.433 9.502l-.032.086a6.066 6.066 0 002.012 7.01l.011.009.03.021 4.976 3.727 2.462 1.863 1.5 1.132a1.009 1.009 0 001.22 0l1.5-1.132 2.461-1.863 5.006-3.749.013-.01a6.068 6.068 0 002.009-7.003z"/></svg>',
  gitea: '<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M4.209 4.603c-.247 0-.525.02-.84.088-.333.07-1.28.283-2.054 1.027C-.403 7.25.035 9.685.089 10.052c.065.446.263 1.687 1.21 2.768 1.749 2.141 5.513 2.092 5.513 2.092s.462 1.103 1.168 2.119c.955 1.263 1.936 2.248 2.89 2.367 2.406 0 7.212-.004 7.212-.004s.458.004 1.08-.394c.535-.324 1.013-.893 1.013-.893s.492-.527 1.18-1.73c.21-.37.385-.729.538-1.068 0 0 2.107-4.471 2.107-8.823-.042-1.318-.367-1.55-.443-1.627-.156-.156-.366-.153-.366-.153s-4.475.252-6.792.306c-.508.011-1.012.023-1.512.027v4.474l-.634-.301c0-1.39-.004-4.17-.004-4.17-1.107.016-3.405-.084-3.405-.084s-5.399-.27-5.987-.324c-.187-.011-.401-.032-.648-.032zm.354 1.832h.111s.271 2.269.6 3.597C5.549 11.147 6.22 13 6.22 13s-.996-.119-1.641-.348c-.99-.324-1.409-.714-1.409-.714s-.73-.511-1.096-1.52C1.444 8.73 2.021 7.7 2.021 7.7s.32-.859 1.47-1.145c.395-.106.863-.12 1.072-.12zm8.33 2.554c.26.003.509.127.509.127l.868.422-.529 1.075a.686.686 0 00-.614.359.685.685 0 00.072.756l-.939 1.924a.69.69 0 00-.66.527.687.687 0 00.347.763.686.686 0 00.867-.206.688.688 0 00-.069-.882l.916-1.874a.667.667 0 00.237-.02.657.657 0 00.271-.137 8.826 8.826 0 011.016.512.761.761 0 01.286.282c.073.21-.073.569-.073.569-.087.29-.702 1.55-.702 1.55a.692.692 0 00-.676.477.681.681 0 101.157-.252c.073-.141.141-.282.214-.431.19-.397.515-1.16.515-1.16.035-.066.218-.394.103-.814-.095-.435-.48-.638-.48-.638-.467-.301-1.116-.58-1.116-.58s0-.156-.042-.27a.688.688 0 00-.148-.241l.516-1.062 2.89 1.401s.48.218.583.619c.073.282-.019.534-.069.657-.24.587-2.1 4.317-2.1 4.317s-.232.554-.748.588a1.065 1.065 0 01-.393-.045l-.202-.08-4.31-2.1s-.417-.218-.49-.596c-.083-.31.104-.691.104-.691l2.073-4.272s.183-.37.466-.497a.855.855 0 01.35-.077z"/></svg>',
  codeberg: '<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M11.999.747A11.974 11.974 0 000 12.75c0 2.254.635 4.465 1.833 6.376L11.837 6.19c.072-.092.251-.092.323 0l4.178 5.402h-2.992l.065.239h3.113l.882 1.138h-3.674l.103.374h3.86l.777 1.003h-4.358l.135.483h4.593l.695.894h-5.038l.165.589h5.326l.609.785h-5.717l.182.65h6.038l.562.727h-6.397l.183.65h6.717A12.003 12.003 0 0024 12.75 11.977 11.977 0 0011.999.747zm3.654 19.104.182.65h5.326c.173-.204.353-.433.513-.65zm.385 1.377.18.65h3.563c.233-.198.485-.428.712-.65zm.383 1.377.182.648h1.203c.356-.204.685-.412 1.042-.648z"/></svg>',
};

function forgeIconSvg(forge) {
  return FORGE_ICONS[forge] || FORGE_ICONS.github;
}

export function branchOptionsForRepo(repo) {
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

export async function loadRepoBranches(repo) {
  if (!isAddonRepo(repo)) return;
  if (state.branchOptionsByRepoId.has(repo.id)) return;
  if (state.branchOptionsLoading.has(repo.id)) return;

  state.branchOptionsLoading.add(repo.id);
  try {
    const branches = await safeInvoke("wuddle_list_repo_branches", { id: repo.id }, { timeoutMs: 20000 });
    state.branchOptionsByRepoId.set(repo.id, Array.isArray(branches) ? branches : []);

    // Targeted update: refresh only this repo's branch <select> instead of
    // rebuilding the entire list (avoids 67× full re-renders on first load).
    const select = document.querySelector(`select.branch-select[data-repo-id="${repo.id}"]`);
    if (select) {
      const options = branchOptionsForRepo(repo);
      const selected = select.value;
      select.innerHTML = "";
      for (const opt of options) {
        const el = document.createElement("option");
        el.value = opt.value;
        el.textContent = opt.label;
        if (opt.value === selected) el.selected = true;
        select.appendChild(el);
      }
      select.disabled = false;
      rebuildThemedSelect(select);
    }
  } catch (e) {
    log(`ERROR branches ${repo.owner}/${repo.name}: ${e.message}`);
  } finally {
    state.branchOptionsLoading.delete(repo.id);
  }
}

export async function setRepoBranch(repo, branch) {
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

export function canUpdateRepo(repo) {
  if (!repo.enabled) return false;
  if (state.ignoredUpdateRepoIds.has(repo.id)) return false;
  const plan = getPlanForRepo(repo.id);
  if (!plan) return false;
  if (plan.error) return false;
  if (plan.externally_modified) return false;
  return !!plan.has_update;
}

export function updateDisabledReason(repo) {
  if (!repo.enabled) return "Project is disabled.";
  const plan = getPlanForRepo(repo.id);
  if (!plan) return "No update data yet.";
  if (plan.error) {
    return `Update unavailable: fetch failed. ${classifyFetchErrorHint(plan.error)}`;
  }
  if (plan.externally_modified) return "Files were modified externally. Use Reinstall / Repair or click the download button to restore.";
  if (plan.repair_needed) return "Use Reinstall / Repair from the actions menu.";
  if (!plan.has_update) return "No update available.";
  return "";
}

export function updateCounts() {
  const mods = reposForView("mods").filter((repo) => canUpdateRepo(repo)).length;
  const addons = reposForView("addons").filter((repo) => canUpdateRepo(repo)).length;
  return { mods, addons, total: mods + addons };
}

// ============================================================================
// Filtering / Sorting
// ============================================================================

export function statusRank(repo) {
  const st = repoStatus(repo);
  if (st.text === "Fetch error") return 0;
  if (st.text === "Update available") return 1;
  if (st.text === "Repair needed") return 2;
  if (st.text === "Disabled") return 3;
  if (st.text === "Ignored") return 3;
  return 4;
}

export function compareVersionText(a, b) {
  return a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" });
}

export function matchesFilter(repo) {
  if (state.filter === "all") return true;
  if (state.filter === "disabled") return !repo.enabled;
  const plan = getPlanForRepo(repo.id);
  if (state.filter === "updates") return !!plan?.has_update;
  if (state.filter === "errors") return !!plan?.error && !state.ignoredErrorRepoIds.has(repo.id);
  if (state.filter === "ignored") {
    return (!!plan?.error && state.ignoredErrorRepoIds.has(repo.id))
      || state.ignoredUpdateRepoIds.has(repo.id);
  }
  return true;
}

export function matchesProjectSearch(repo) {
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

export function sortedFilteredRepos() {
  const list = reposForCurrentView().filter((repo) => matchesFilter(repo) && matchesProjectSearch(repo));

  const defaultCompare = (a, b) => {
    const aUpdate = canUpdateRepo(a) ? 1 : 0;
    const bUpdate = canUpdateRepo(b) ? 1 : 0;
    if (aUpdate !== bUpdate) return bUpdate - aUpdate;

    const aRank = statusRank(a);
    const bRank = statusRank(b);
    if (aRank !== bRank) return aRank - bRank;

    return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  };

  if (state.sortDir === "none") {
    list.sort(defaultCompare);
    return list;
  }

  const dir = state.sortDir === "desc" ? -1 : 1;
  list.sort((a, b) => {
    let cmp = 0;
    if (state.sortKey === "name") {
      cmp = a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    } else if (state.sortKey === "current") {
      const av = state.projectView === "addons"
        ? String(a.gitBranch || "").trim() || "default"
        : versionLabel(getPlanForRepo(a.id)?.current);
      const bv = state.projectView === "addons"
        ? String(b.gitBranch || "").trim() || "default"
        : versionLabel(getPlanForRepo(b.id)?.current);
      cmp = compareVersionText(av, bv);
    } else if (state.sortKey === "latest") {
      const av = versionLabel(getPlanForRepo(a.id)?.latest);
      const bv = versionLabel(getPlanForRepo(b.id)?.latest);
      cmp = compareVersionText(av, bv);
    } else if (state.sortKey === "status") {
      cmp = statusRank(a) - statusRank(b);
    }

    if (cmp === 0) return defaultCompare(a, b);
    return dir * cmp;
  });

  return list;
}

// ============================================================================
// Project Summary / Status Strip
// ============================================================================

export function getProjectSummary() {
  const viewRepos = reposForCurrentView();
  const total = viewRepos.length;
  const enabled = viewRepos.filter((repo) => repo.enabled).length;
  const disabled = total - enabled;
  const updates = viewRepos.filter((repo) => {
    const plan = getPlanForRepo(repo.id);
    return repo.enabled && !!plan?.has_update && !plan?.error;
  }).length;
  const errors = viewRepos.filter((repo) => {
    const p = getPlanForRepo(repo.id);
    return !!p?.error && !state.ignoredErrorRepoIds.has(repo.id);
  }).length;
  const ignored = viewRepos.filter((repo) => {
    return (!!getPlanForRepo(repo.id)?.error && state.ignoredErrorRepoIds.has(repo.id))
      || state.ignoredUpdateRepoIds.has(repo.id);
  }).length;
  const rateLimited = viewRepos.some((repo) => {
    const error = getPlanForRepo(repo.id)?.error || "";
    return /rate[\s-]?limit|http 403|http 429/i.test(error);
  });

  return { total, enabled, disabled, updates, errors, ignored, rateLimited };
}

export function getUpdateActionState() {
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

// ============================================================================
// Notification Helpers
// ============================================================================

export function openRelevantUpdatesView(counts = updateCounts()) {
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

export function maybeNotifyProjectUpdates(source, notify) {
  if (!notify) return;
  const updates = state.repos.filter((repo) => canUpdateRepo(repo));
  const counts = updateCounts();

  if (source === "manual") {
    if (!updates.length) {
      state.lastUpdateNotifyKey = "";
      showToast("No updates available.", { kind: "info" });
    } else {
      const noun = counts.total === 1 ? "update" : "updates";
      showToast(`${counts.total} ${noun} available. Mods: ${counts.mods}, Addons: ${counts.addons}.`, {
        kind: "info",
        onAction: () => openRelevantUpdatesView(counts),
      });
    }

    const modifiedMods = state.plans.filter((p) => p.externally_modified);
    if (modifiedMods.length > 0) {
      const names = modifiedMods.map((p) => p.name).join(", ");
      showToast(`${modifiedMods.length} mod(s) modified externally: ${names}`, { kind: "warn" });
    }
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
    onAction: () => openRelevantUpdatesView(counts),
  });

  if (state.desktopNotifyEnabled) {
    try {
      const np = window.__TAURI__?.notification;
      if (np) {
        const body = `${counts.total} update${counts.total === 1 ? "" : "s"} available. Mods: ${counts.mods}, Addons: ${counts.addons}.`;
        np.isPermissionGranted().then((ok) => {
          const send = () => np.sendNotification({ title: "Wuddle", body });
          if (ok) { send(); return; }
          np.requestPermission().then((p) => { if (p === "granted") send(); });
        });
      }
    } catch (_) {}
  }

  const modifiedMods = state.plans.filter((p) => p.externally_modified);
  if (modifiedMods.length > 0) {
    const names = modifiedMods
      .map((p) => p.name)
      .join(", ");
    showToast(`${modifiedMods.length} mod(s) modified externally: ${names}`, {
      kind: "warn",
    });
  }
}

// ============================================================================
// Rendering
// ============================================================================

export function escapeHtml(s) {
  return String(s ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

export function renderProjectSearch() {
  const input = $("projectSearchInput");
  if (!(input instanceof HTMLInputElement)) {
    return;
  }
  if (input.value !== state.projectSearchQuery) {
    input.value = state.projectSearchQuery;
  }
  const hasValue = String(state.projectSearchQuery || "").trim().length > 0;
  const wrap = input.closest(".project-search");
  if (wrap instanceof HTMLElement) {
    wrap.classList.toggle("has-value", hasValue);
  }
  const clearBtn = $("projectSearchClear");
  if (clearBtn instanceof HTMLButtonElement) {
    clearBtn.disabled = !hasValue;
    clearBtn.setAttribute("aria-hidden", hasValue ? "false" : "true");
  }
}

export function renderFilterButtons() {
  const summary = getProjectSummary();
  const isAddons = state.projectView === "addons";
  const labels = {
    all: `All (${summary.total})`,
    updates: `Updates (${summary.updates})`,
    errors: `Errors (${summary.errors})`,
  };
  const fourthKey = isAddons ? "ignored" : "disabled";
  const fourthLabel = isAddons
    ? `Ignored (${summary.ignored})`
    : `Disabled (${summary.disabled})`;
  document.querySelectorAll(".filter-btn[data-filter]").forEach((btn) => {
    let key = btn.getAttribute("data-filter");
    if (key === "disabled" || key === "ignored") {
      btn.setAttribute("data-filter", fourthKey);
      key = fourthKey;
      btn.textContent = fourthLabel;
    }
    btn.classList.toggle("active", key === state.filter);
    if (key && Object.prototype.hasOwnProperty.call(labels, key)) {
      btn.textContent = labels[key];
    }
  });
}

export function renderProjectViewButtons() {
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

export function renderSortHeaders() {
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
    state.sortKey = "name";
    if (state.sortDir !== "none") {
      state.sortDir = "asc";
    }
  }

  document.querySelectorAll("#repoThead .th.sortable").forEach((th) => {
    if (addonsView && th.id === "thLatest") {
      th.classList.add("col-hidden");
      return;
    }
    th.classList.remove("col-hidden");
    const key = th.getAttribute("data-sort");
    const selected = key === state.sortKey;
    const active = selected && state.sortDir !== "none";
    th.classList.toggle("active", active);
    th.classList.toggle("unsorted", selected && state.sortDir === "none");
    th.setAttribute("data-dir", active ? state.sortDir : "");
  });
}

export function renderLastChecked() {
  $("lastChecked").textContent = `Last checked: ${formatTime(state.lastCheckedAt)}`;
}

export function renderProjectStatusStrip() {
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

export function closeActionsMenu() {
  if (state.openMenuRepoId === null) return;
  state.openMenuRepoId = null;
  render();
}

export function toggleActionsMenu(repoId) {
  state.openMenuRepoId = state.openMenuRepoId === repoId ? null : repoId;
  render();
}

export function positionOpenMenu() {
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

export function openUrl_external(url) {
  return openUrl(url);
}

// ============================================================================
// External URL / Path helpers
// ============================================================================

export function confirmExternalOpen(kind, target) {
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

export async function openUrl(url) {
  const target = String(url ?? "").trim();
  if (!target) {
    log("ERROR open url: URL is empty.");
    return;
  }
  try {
    await safeInvoke("wuddle_open_url", { url: target });
  } catch (err) {
    log(`ERROR open url: ${err?.message || String(err)}`);
  }
}

export async function openPath(path) {
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

// ============================================================================
// Remove Dialog
// ============================================================================

export function openRemoveDialog(repo) {
  state.removeTargetRepo = repo;
  $("removeRepoName").textContent = `${repo.owner}/${repo.name}`;
  $("removeLocalFiles").checked = false;
  $("dlgRemove").showModal();
}

export async function confirmRemove() {
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

// ============================================================================
// Repo Operations
// ============================================================================

export async function loadRepos() {
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

export function logOperationResult(result) {
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

export async function setRepoEnabled(repo, enabled) {
  try {
    const wowDir = readWowDir() || null;
    const msg = await safeInvoke("wuddle_set_repo_enabled", { id: repo.id, enabled, wowDir });
    log(`${repo.owner}/${repo.name}: ${msg}`);
    await refreshAll();
  } catch (e) {
    log(`ERROR toggling repo: ${e.message}`);
  }
}

export async function updateRepo(repo) {
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
      logOperationResult(result);
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

export async function reinstallRepo(repo) {
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

// ============================================================================
// Repo Row Rendering
// ============================================================================

export function renderRepos() {
  const host = $("repoRows");
  host.innerHTML = "";

  renderFilterButtons();
  renderSortHeaders();
  renderLastChecked();
  renderProjectStatusStrip();
  renderGithubAuthHealth();
  const failedCount = reposForCurrentView().filter((r) => !!getPlanForRepo(r.id)?.error).length;
  $("btnRetryFailed").classList.toggle("hidden", failedCount === 0);
  $("btnRetryFailed").disabled = failedCount === 0 || !activeProfile();
  $("btnRetryFailed").title = failedCount ? `Retry ${failedCount} failed fetch(es)` : "No failed fetches";

  const updateActionState = getUpdateActionState();
  const updateActionBtn = $("btnUpdateAll");
  updateActionBtn.textContent = updateActionState.label;
  updateActionBtn.title = updateActionState.title;
  updateActionBtn.classList.toggle("primary", updateActionState.primary);
  updateActionBtn.disabled = !activeProfile() || updateActionState.disabled;
  if (!activeProfile()) {
    updateActionBtn.textContent = "Check for updates";
    updateActionBtn.title = "Add an instance in Options first.";
  }

  if (!activeProfile()) {
    return;
  }

  const visibleRepos = sortedFilteredRepos();

  if (!visibleRepos.length) {
    const div = document.createElement("div");
    div.className = "empty";
    const totalForView = reposForCurrentView().length;
    const noun = state.projectView === "addons" ? "addons" : "mods";
    div.textContent = totalForView
      ? `No ${noun} match the current filter.`
      : `No ${noun} yet. Click "\uFF0B Add".`;
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
    nameLink.title = `View details for ${r.name}`;
    nameLink.addEventListener("click", (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      openRepoDetailDialog(r);
    });

    const nameSub = document.createElement("div");
    nameSub.className = "name-sub";
    nameSub.textContent = `${r.owner} \u2022 ${forgeLabel}${r.enabled ? "" : " \u2022 disabled"}`;

    nameHeader.appendChild(nameLink);
    nameMain.appendChild(nameHeader);
    nameMain.appendChild(nameSub);
    nameCell.appendChild(nameMain);

    const status = document.createElement("div");
    status.innerHTML = `<span class="badge ${st.kind}">${escapeHtml(st.text)}</span>`;
    if (plan?.error) {
      status.title = plan.error;
    } else if (plan?.externally_modified) {
      status.title = "This mod has been modified outside of Wuddle and will not be managed by Wuddle until it's been reinstalled. Click the update button to reinstall.";
    }

    const addonsView = state.projectView === "addons";
    const currentCell = document.createElement("div");
    if (addonsView) {
      currentCell.className = "branch-cell";
      const select = document.createElement("select");
      select.className = "branch-select";
      select.dataset.repoId = String(r.id);
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
      select.disabled = !!state.branchOptionsLoading.has(r.id);
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

    const isModified = plan?.externally_modified;
    const updateBtn = document.createElement("button");
    updateBtn.className = "btn icon action-update";
    updateBtn.textContent = "\u2913";
    updateBtn.setAttribute("aria-label", "Update");
    updateBtn.disabled = !canUpdateRepo(r) && !isModified;
    updateBtn.title = isModified
      ? "Reinstall to restore modified files"
      : updateBtn.disabled
        ? updateDisabledReason(r)
        : "Update now";
    updateBtn.addEventListener("click", async (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      state.openMenuRepoId = null;
      if (isModified) {
        await reinstallRepo(r);
      } else {
        await updateRepo(r);
      }
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
    menuBtn.textContent = "\u22EE";
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
    if (plan?.error) {
      const isIgnored = state.ignoredErrorRepoIds.has(r.id);
      const ignore = document.createElement("button");
      ignore.className = "menu-item";
      ignore.textContent = isIgnored ? "Unignore Error" : "Ignore Error";
      ignore.addEventListener("click", (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        state.openMenuRepoId = null;
        if (isIgnored) {
          state.ignoredErrorRepoIds.delete(r.id);
        } else {
          state.ignoredErrorRepoIds.add(r.id);
        }
        persistIgnoredErrors();
        render();
      });
      menu.appendChild(ignore);
    }
    {
      const isUpdateIgnored = state.ignoredUpdateRepoIds.has(r.id);
      const ignoreUpdates = document.createElement("button");
      ignoreUpdates.className = "menu-item";
      ignoreUpdates.textContent = isUpdateIgnored ? "Unignore Updates" : "Ignore Updates";
      ignoreUpdates.addEventListener("click", (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        state.openMenuRepoId = null;
        if (isUpdateIgnored) {
          state.ignoredUpdateRepoIds.delete(r.id);
        } else {
          state.ignoredUpdateRepoIds.add(r.id);
        }
        persistIgnoredUpdates();
        render();
      });
      menu.appendChild(ignoreUpdates);
    }
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

// ============================================================================
// Presets Rendering
// ============================================================================

export function renderAddPresets() {
  const host = $("addPresetList");
  if (!host) return;
  host.innerHTML = "";
  const hasProfile = !!activeProfile();

  for (const preset of CURATED_MOD_PRESETS) {
    const installed = !preset.placeholder && isPresetInstalled(preset);
    const description = String(preset.description || "").trim();
    const warning = String(preset.warning || "").trim();
    const card = document.createElement("div");
    card.className = `preset-card${preset.placeholder ? " placeholder" : ""}${installed ? " installed" : ""} expanded`;

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
    descText.textContent = description;
    desc.appendChild(descText);

    const companionLinks = Array.isArray(preset.companionLinks)
      ? preset.companionLinks
      : [];

    if (Array.isArray(preset.expandedNotes) && preset.expandedNotes.length > 0) {
      const notes = document.createElement("div");
      notes.className = "preset-desc-notes";
      for (const rawLine of preset.expandedNotes) {
        const line = String(rawLine || "").trim();
        if (!line) continue;
        const row = document.createElement("div");
        row.className = "preset-desc-note";
        row.textContent = `\u2022 ${line}`;
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
          sep.textContent = "\u2022";
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

export function applyAddDialogContext() {
  const addons = state.projectView === "addons";
  const addButton = $("btnAddOpen");
  const addTitle = $("addDialogTitle");
  const addHint = $("addDialogHint");
  const quickAddField = $("quickAddField");
  const repoUrlLabel = $("addRepoUrlLabel");
  const modeSelect = $("mode");

  if (addButton) {
    addButton.textContent = "\uFF0B Add";
  }
  if (addTitle) {
    addTitle.textContent = addons ? "Add an addon repo" : "Add a repo";
  }
  if (addHint) {
    addHint.textContent = addons
      ? "Paste a Git repository URL below. Wuddle will automatically download and install the addon for you."
      : "Quick-add from the mods listed, or add your own Git repo URL below.";
  }
  if (repoUrlLabel) {
    repoUrlLabel.textContent = addons ? "Addon Repo URL" : "Repo URL";
  }
  const repoUrlInput = $("repoUrl");
  if (repoUrlInput) {
    repoUrlInput.placeholder = addons
      ? "(e.g. https://github.com/pepopo978/BigWigs)"
      : "(e.g. https://gitea.com/avitasia/nampower)";
  }
  if (quickAddField) {
    quickAddField.classList.toggle("hidden", addons);
  }
  const contentFrameLabel = $("contentFrameLabel");
  const contentFrame = contentFrameLabel?.nextElementSibling;
  if (contentFrameLabel) {
    contentFrameLabel.classList.toggle("hidden", addons);
  }
  if (contentFrame && addons) {
    contentFrame.classList.add("hidden");
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

export function setFilter(filter) {
  const allowed = new Set(["all", "updates", "errors", "disabled", "ignored"]);
  state.filter = allowed.has(filter) ? filter : "all";
  render();
}

export function toggleSort(sortKey) {
  const allowed = new Set(["name", "current", "latest", "status"]);
  if (!allowed.has(sortKey)) return;
  if (state.sortKey === sortKey) {
    if (state.sortDir === "asc") state.sortDir = "desc";
    else if (state.sortDir === "desc") state.sortDir = "none";
    else state.sortDir = "asc";
  } else {
    state.sortKey = sortKey;
    state.sortDir = "asc";
  }
  render();
}

// ============================================================================
// Deferred circular-dependency refs (set by main.js at startup)
// ============================================================================

let render = () => {};
let renderGithubAuthHealth = () => {};
let setTab = () => {};
let showProjectsPanel = () => {};

export function setRepoCallbacks(cbs) {
  if (cbs.render) render = cbs.render;
  if (cbs.renderGithubAuthHealth) renderGithubAuthHealth = cbs.renderGithubAuthHealth;
  if (cbs.setTab) setTab = cbs.setTab;
  if (cbs.showProjectsPanel) showProjectsPanel = cbs.showProjectsPanel;
}

// ============================================================================
// Add dialog helpers
// ============================================================================

export function focusAddDialogUrlInput() {
  const input = $("repoUrl");
  if (!(input instanceof HTMLInputElement)) return;
  requestAnimationFrame(() => {
    input.focus();
    input.select();
  });
}

// ============================================================================
// README preview for Add dialog
// ============================================================================

const CACHE_MAX = 30;
function cappedSet(map, key, value) {
  if (map.size >= CACHE_MAX) { map.delete(map.keys().next().value); }
  map.set(key, value);
}
const _readmeCache = new Map();
const _repoInfoCache = new Map();
const _repoTreeCache = new Map();
let _readmeDebounceTimer = null;
let _readmeAbortKey = 0;

export function setupReadmePreviewListener() {
  const input = $("repoUrl");
  if (!input) return;
  input.addEventListener("input", () => {
    clearTimeout(_readmeDebounceTimer);
    _readmeDebounceTimer = setTimeout(() => {
      void fetchAllRepoPreviews(input.value);
    }, 600);
  });
}

/** Kick off README, repo info, and file tree fetches in parallel. */
async function fetchAllRepoPreviews(rawUrl) {
  const url = String(rawUrl ?? "").trim();
  const info = parseRepoUrlInfo(url);

  if (!info.host || !info.owner || !info.name) {
    hideAllRepoPreviews();
    return;
  }

  _activeReadmeRepoUrl = url;

  // Fire all three in parallel
  fetchAndShowReadme(url, info);
  fetchAndShowRepoInfo(url, info);
  fetchAndShowRepoTree(url, info);
}

function hideAllRepoPreviews() {
  hideReadmePanel();
  const sidePanel = $("repoSidePanel");
  if (sidePanel) sidePanel.classList.add("hidden");
  const aboutSec = $("repoAboutSection");
  if (aboutSec) {
    aboutSec.classList.add("hidden");
    $("repoAbout").innerHTML = "";
  }
  const treeSec = $("repoTreeSection");
  if (treeSec) {
    treeSec.classList.add("hidden");
    $("repoTree").innerHTML = "";
  }

}

/** Show the side panel if at least one section is visible. */
function updateSidePanelVisibility() {
  const panel = $("repoSidePanel");
  if (!panel) return;
  const aboutVisible = !$("repoAboutSection")?.classList.contains("hidden");
  const treeVisible = !$("repoTreeSection")?.classList.contains("hidden");
  if (aboutVisible || treeVisible) {
    panel.classList.remove("hidden");
  } else {
    panel.classList.add("hidden");
  }
}

/** Show README wrap and hide Quick Add (they share the same frame). */
function showReadmePanel() {
  // Dismiss file preview if active
  const fp = $("addFilePreview");
  if (fp) { fp.innerHTML = ""; fp.classList.add("hidden"); }
  const wrap = $("readmePreviewWrap");
  if (wrap) wrap.classList.remove("hidden");
  const quickAdd = $("quickAddField");
  if (quickAdd) quickAdd.classList.add("hidden");
  const lbl = $("contentFrameLabel");
  if (lbl) {
    lbl.textContent = "README";
    lbl.classList.remove("hidden");
    lbl.classList.remove("frame-label-back");
    lbl.onclick = null;
  }
  const frame = wrap?.closest(".scroll-fade");
  if (frame) {
    frame.classList.remove("hidden");
    requestAnimationFrame(() => refreshScrollFade(frame));
  }
}

/** Hide README wrap and restore Quick Add (mods only — addons have no presets). */
function hideReadmePanel() {
  const wrap = $("readmePreviewWrap");
  if (wrap) {
    wrap.classList.add("hidden");
    $("readmePreview").innerHTML = "";
  }
  const isAddons = state.projectView === "addons";
  const quickAdd = $("quickAddField");
  if (quickAdd) quickAdd.classList.toggle("hidden", isAddons);
  const lbl = $("contentFrameLabel");
  if (lbl) {
    lbl.textContent = isAddons ? "" : "Quick Add";
    lbl.classList.toggle("hidden", isAddons);
  }
  const frame = wrap?.closest(".scroll-fade");
  if (frame) {
    frame.classList.toggle("hidden", isAddons);
    requestAnimationFrame(() => refreshScrollFade(frame));
  }
}

async function fetchAndShowReadme(url, info) {
  const wrap = $("readmePreviewWrap");
  const container = $("readmePreview");
  if (!wrap || !container) return;

  const cacheKey = `${info.host}|${info.owner}|${info.name}`.toLowerCase();
  if (_readmeCache.has(cacheKey)) {
    container.innerHTML = _readmeCache.get(cacheKey);
    wireReadmeLinks(container);
    showReadmePanel();
    return;
  }

  showReadmePanel();
  container.innerHTML = '<div class="readme-preview-loading">Loading README\u2026</div>';

  const generation = ++_readmeAbortKey;

  try {
    const html = await safeInvoke("wuddle_fetch_repo_readme", { url }, { timeoutMs: 15000 });
    if (generation !== _readmeAbortKey) return;

    let rendered;
    if (html.startsWith("<!--md-->")) {
      rendered = changelogToHtml(html.slice(9));
    } else {
      rendered = sanitizeReadmeHtml(html);
    }

    cappedSet(_readmeCache, cacheKey, rendered);
    container.innerHTML = rendered;
    wireReadmeLinks(container);
    showReadmePanel();

  } catch (_) {
    if (generation !== _readmeAbortKey) return;
    hideReadmePanel();
  }
}

// ---------------------------------------------------------------------------
// Repo info (About panel)
// ---------------------------------------------------------------------------

async function fetchAndShowRepoInfo(url, info) {
  const section = $("repoAboutSection");
  const container = $("repoAbout");
  if (!section || !container) return;

  const cacheKey = `${info.host}|${info.owner}|${info.name}`.toLowerCase();
  if (_repoInfoCache.has(cacheKey)) {
    container.innerHTML = _repoInfoCache.get(cacheKey);
    wireAboutLinks(container);
    section.classList.remove("hidden");
    updateSidePanelVisibility();
    return;
  }

  container.innerHTML = '<span class="repo-info-loading">Loading\u2026</span>';
  section.classList.remove("hidden");
  updateSidePanelVisibility();

  try {
    const json = await safeInvoke("wuddle_fetch_repo_info", { url }, { timeoutMs: 15000 });
    const data = JSON.parse(json);
    const html = renderRepoAbout(data);
    cappedSet(_repoInfoCache, cacheKey, html);
    container.innerHTML = html;
    wireAboutLinks(container);
  } catch (_) {
    container.innerHTML = "";
    section.classList.add("hidden");
    updateSidePanelVisibility();
  }
}

function renderRepoAbout(data) {
  const desc = data.description
    ? `<p class="repo-description">${escapeHtml(data.description)}</p>`
    : '<p class="repo-description empty">No description provided.</p>';

  const stats = [];

  if (typeof data.stars === "number") {
    stats.push(`<div class="repo-stat"><span class="repo-stat-icon">\u2b50</span>${data.stars.toLocaleString()} star${data.stars !== 1 ? "s" : ""}</div>`);
  }

  if (typeof data.forks === "number" && data.forksUrl) {
    stats.push(`<div class="repo-stat"><span class="repo-stat-icon">\ud83c\udf74</span><a href="#" class="repo-forks-link" data-href="${escapeHtml(data.forksUrl)}">${data.forks.toLocaleString()} fork${data.forks !== 1 ? "s" : ""}</a></div>`);
  }

  if (data.language) {
    stats.push(`<div class="repo-stat"><span class="repo-stat-icon">\ud83d\udcbb</span>${escapeHtml(data.language)}</div>`);
  }

  if (data.license) {
    stats.push(`<div class="repo-stat"><span class="repo-stat-icon">\ud83d\udccb</span>${escapeHtml(data.license)}</div>`);
  }

  return desc + (stats.length ? `<div class="repo-stats">${stats.join("")}</div>` : "");
}

function wireAboutLinks(container) {
  container.querySelectorAll(".repo-forks-link").forEach((a) => {
    a.addEventListener("click", (ev) => {
      ev.preventDefault();
      void openUrl(a.dataset.href);
    });
  });
}

// ---------------------------------------------------------------------------
// Repo tree (Files panel)
// ---------------------------------------------------------------------------

let _treeRepoUrl = "";
let _treeClickContext = null;
const MAX_TREE_DEPTH = 10;

async function fetchAndShowRepoTree(url, info) {
  const section = $("repoTreeSection");
  const container = $("repoTree");
  if (!section || !container) return;

  _treeRepoUrl = url;
  _treeClickContext = { type: "remote", repoUrl: url, target: "add" };

  const cacheKey = `${info.host}|${info.owner}|${info.name}`.toLowerCase();
  if (_repoTreeCache.has(cacheKey)) {
    container.innerHTML = "";
    container.appendChild(_repoTreeCache.get(cacheKey).cloneNode(true));
    wireTreeToggles(container);
    wireTreeFileClicks(container, _treeClickContext);
    section.classList.remove("hidden");
    updateSidePanelVisibility();
    return;
  }

  container.innerHTML = '<span class="repo-info-loading">Loading\u2026</span>';
  section.classList.remove("hidden");
  updateSidePanelVisibility();

  try {
    const json = await safeInvoke("wuddle_fetch_repo_tree", { url }, { timeoutMs: 30000 });
    const entries = JSON.parse(json);
    const frag = renderTreeEntries(entries, "", _treeClickContext);
    cappedSet(_repoTreeCache, cacheKey, frag.cloneNode(true));
    container.innerHTML = "";
    container.appendChild(frag);
    wireTreeToggles(container);
    wireTreeFileClicks(container, _treeClickContext);
    refreshScrollFade(section);
  } catch (_) {
    container.innerHTML = "";
    section.classList.add("hidden");
    updateSidePanelVisibility();
  }
}

function sortTreeEntries(entries) {
  return [...entries].sort((a, b) => {
    if (a.type === "dir" && b.type !== "dir") return -1;
    if (a.type !== "dir" && b.type === "dir") return 1;
    return a.name.localeCompare(b.name);
  });
}

function renderTreeEntries(entries, parentPath, context) {
  const frag = document.createDocumentFragment();
  const sorted = sortTreeEntries(entries);
  const prefix = parentPath || "";
  for (const entry of sorted) {
    const fullPath = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.type === "dir") {
      const details = document.createElement("details");
      details.className = "tree-dir-details";
      const summary = document.createElement("summary");
      summary.className = "tree-entry dir";
      summary.innerHTML = `<span class="tree-icon">\ud83d\udcc1</span><span class="tree-name">${escapeHtml(entry.name)}</span>`;
      details.appendChild(summary);
      // Placeholder for lazy-loaded children
      const childWrap = document.createElement("div");
      childWrap.className = "tree-children";
      childWrap.dataset.path = fullPath;
      childWrap.dataset.loaded = "false";
      details.appendChild(childWrap);
      frag.appendChild(details);
    } else {
      const div = document.createElement("div");
      div.className = "tree-entry file";
      div.innerHTML = `<span class="tree-icon">\ud83d\udcc4</span><span class="tree-name">${escapeHtml(entry.name)}</span>`;
      if (context) {
        div.addEventListener("click", () => previewFile(context, fullPath, entry.name));
      }
      frag.appendChild(div);
    }
  }
  return frag;
}

/** Wire click handlers on file entries in a tree container. */
function wireTreeFileClicks(container, context) {
  if (!context) return;
  container.querySelectorAll(".tree-entry.file").forEach((el) => {
    // Find the full path from the closest tree-children parent chain
    const parts = [];
    let node = el;
    while (node) {
      const parent = node.closest(".tree-children");
      if (!parent) break;
      if (parent.dataset.path) parts.unshift(parent.dataset.path);
      node = parent.parentElement;
    }
    const fileName = el.querySelector(".tree-name")?.textContent || "";
    const fullPath = parts.length ? `${parts[parts.length - 1]}/${fileName}` : fileName;
    el.addEventListener("click", () => previewFile(context, fullPath, fileName));
  });
}

/** Wire up toggle events on all <details> in a tree container for lazy loading. */
function wireTreeToggles(container) {
  const scrollParent = container.closest(".scroll-fade");
  container.querySelectorAll("details.tree-dir-details").forEach((det) => {
    det.addEventListener("toggle", () => {
      if (!det.open) return;
      const childWrap = det.querySelector(".tree-children");
      if (!childWrap || childWrap.dataset.loaded === "true") return;
      void loadTreeChildren(childWrap);
    }, { once: true });
    det.addEventListener("toggle", () => {
      if (scrollParent) requestAnimationFrame(() => refreshScrollFade(scrollParent));
    });
  });
}

async function loadTreeChildren(childWrap) {
  const path = childWrap.dataset.path || "";
  const depth = (path.match(/\//g) || []).length + 1;
  if (depth > MAX_TREE_DEPTH) {
    childWrap.innerHTML = '<span class="repo-info-loading">Max depth reached</span>';
    childWrap.dataset.loaded = "true";
    return;
  }

  childWrap.innerHTML = '<span class="repo-info-loading">Loading\u2026</span>';
  childWrap.dataset.loaded = "true";

  try {
    const json = await safeInvoke(
      "wuddle_fetch_repo_contents",
      { url: _treeRepoUrl, path },
      { timeoutMs: 15000 },
    );
    const entries = JSON.parse(json);
    const sorted = sortTreeEntries(entries);
    childWrap.innerHTML = "";
    for (const entry of sorted) {
      if (entry.type === "dir") {
        const details = document.createElement("details");
        details.className = "tree-dir-details";
        const summary = document.createElement("summary");
        summary.className = "tree-entry dir";
        summary.innerHTML = `<span class="tree-icon">\ud83d\udcc1</span><span class="tree-name">${escapeHtml(entry.name)}</span>`;
        details.appendChild(summary);
        const innerWrap = document.createElement("div");
        innerWrap.className = "tree-children";
        innerWrap.dataset.path = `${path}/${entry.name}`;
        innerWrap.dataset.loaded = "false";
        details.appendChild(innerWrap);
        // Wire toggle for this new folder
        details.addEventListener("toggle", () => {
          if (!details.open) return;
          if (innerWrap.dataset.loaded === "true") return;
          void loadTreeChildren(innerWrap);
        }, { once: true });
        const scrollParent = childWrap.closest(".scroll-fade");
        details.addEventListener("toggle", () => {
          if (scrollParent) requestAnimationFrame(() => refreshScrollFade(scrollParent));
        });
        childWrap.appendChild(details);
      } else {
        const div = document.createElement("div");
        div.className = "tree-entry file";
        div.innerHTML = `<span class="tree-icon">\ud83d\udcc4</span><span class="tree-name">${escapeHtml(entry.name)}</span>`;
        if (_treeClickContext) {
          const filePath = `${path}/${entry.name}`;
          div.addEventListener("click", () => previewFile(_treeClickContext, filePath, entry.name));
        }
        childWrap.appendChild(div);
      }
    }
  } catch (_) {
    childWrap.innerHTML = '<span class="repo-info-loading">Failed to load</span>';
  }
  const scrollParent = childWrap.closest(".scroll-fade");
  if (scrollParent) requestAnimationFrame(() => refreshScrollFade(scrollParent));
}

function sanitizeReadmeHtml(html) {
  return html
    .replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, "")
    .replace(/\s+on\w+\s*=\s*("[^"]*"|'[^']*'|[^\s>]*)/gi, "")
    // Remove GitHub clipboard-copy buttons
    .replace(/<clipboard-copy[^>]*>[\s\S]*?<\/clipboard-copy>/gi, "");
}

/** Currently active repo URL for README URL resolution (detail dialog or add dialog). */
let _activeReadmeRepoUrl = "";

/** Resolve a possibly-relative URL against the current repo context. */
function resolveReadmeUrl(raw) {
  if (!raw) return raw;
  if (raw.startsWith("http://") || raw.startsWith("https://")) return raw;
  const repoUrl = _activeReadmeRepoUrl || ($("repoUrl")?.value || "").trim();
  if (!repoUrl) return raw;
  try {
    // Build a raw content base for relative paths (works for GitHub, Gitea, GitLab)
    const u = new URL(repoUrl);
    if (u.hostname === "github.com") {
      const parts = u.pathname.replace(/\.git$/, "").split("/").filter(Boolean);
      if (parts.length >= 2) {
        const base = `https://raw.githubusercontent.com/${parts[0]}/${parts[1]}/HEAD/`;
        return new URL(raw, base).href;
      }
    }
    // Fallback: resolve against the repo page URL
    return new URL(raw, repoUrl + "/").href;
  } catch (_) {
    return raw;
  }
}

/** Make links clickable and resolve relative image/video/source URLs. */
function wireReadmeLinks(container) {
  // Resolve image sources
  container.querySelectorAll("img[src]").forEach((img) => {
    const src = img.getAttribute("src") || "";
    img.setAttribute("src", resolveReadmeUrl(src));
  });

  // Resolve video sources
  container.querySelectorAll("video[src]").forEach((v) => {
    v.setAttribute("src", resolveReadmeUrl(v.getAttribute("src") || ""));
  });
  container.querySelectorAll("video source[src]").forEach((s) => {
    s.setAttribute("src", resolveReadmeUrl(s.getAttribute("src") || ""));
  });

  // Wire links to open in system browser
  container.querySelectorAll("a[href]").forEach((a) => {
    const href = a.getAttribute("href") || "";
    if (href.startsWith("#")) return;
    a.addEventListener("click", (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      void openUrl(resolveReadmeUrl(href));
    });
  });
}

export function openAddDialog() {
  const dlg = $("dlgAdd");
  if (!dlg) return;
  $("repoUrl").value = "";
  renderAddPresets();
  hideAllRepoPreviews();
  if (!dlg.open) {
    dlg.showModal();
  }
  focusAddDialogUrlInput();
}

export function openAddDialogFor(view) {
  setProjectView(view);
  showProjectsPanel();
  applyAddDialogContext();
  openAddDialog();
}

// ============================================================================
// Core async operations
// ============================================================================

async function loadReposLocal() {
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

export async function checkUpdates(options = {}) {
  const notify = !!options.notify;
  const source = String(options.source || "refresh");
  const checkMode = options.checkMode || "force";
  const prevByRepo = new Map(state.plans.map((p) => [p.repo_id, p]));
  const wowDir = readWowDir() || null;
  const next = await safeInvoke("wuddle_check_updates", { wowDir, checkMode }, { timeoutMs: 30000 });
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

export async function refreshAll(options = {}) {
  const forceCheck = !!options.forceCheck;
  const notify = !!options.notify;
  const source = String(options.source || "refresh");
  const checkMode = options.checkMode || "force";
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
      const hasToken = !!(state.githubAuth?.tokenStored || state.githubAuth?.envTokenPresent);
      const shouldCheckUpdates = forceCheck || hasToken || allowInitial;

      if (shouldCheckUpdates) {
        await checkUpdates({ notify, source, checkMode });
        state.initialAutoCheckDone = true;
        state.loggedNoTokenAutoSkip = false;
      } else if (!state.loggedNoTokenAutoSkip) {
        log("Skipping auto update check (no GitHub token). Use \u201cCheck for updates\u201d manually.");
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

export async function updateAll() {
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

export async function handleUpdateAction() {
  const action = getUpdateActionState();
  if (action.mode === "update_all") {
    await updateAll();
    return;
  }
  await refreshAll({ forceCheck: true, notify: true, source: "manual", checkMode: "manual" });
}

export async function addRepo(urlOverride = null, modeOverride = null, label = "") {
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
      const isAddonGitMode = String(mode || "")
        .trim()
        .toLowerCase() === "addon_git";
      let replaceAddonConflictsOnInstall = false;
      if (isAddonGitMode) {
        const incomingInfo = parseRepoUrlInfo(url);
        const wowDirForProbe = currentWowDirStrict();
        const incomingRepoKey = repoKeyFromUrl(url);
        const sameRepoIds = new Set(
          state.repos
            .filter((repo) => {
              if (!incomingRepoKey) return false;
              return repoKeyFromRepo(repo) === incomingRepoKey;
            })
            .map((repo) => Number(repo.id))
            .filter((id) => Number.isFinite(id)),
        );
        if (wowDirForProbe) {
          try {
            const probe = await safeInvoke(
              "wuddle_probe_addon_repo",
              { url, wowDir: wowDirForProbe, branch: null },
              { timeoutMs: 30000 },
            );
            const parsed = formatAddonProbeConflictDetails(probe?.conflicts, sameRepoIds);
            if (parsed.details.length > 0) {
              const proceed = await confirmAddonConflict(
                {
                  owner: incomingInfo.owner || "incoming",
                  name:
                    addonNameFromUrl(url) || incomingInfo.name || String(url || "").trim(),
                  host: incomingInfo.host || "",
                  url,
                },
                parsed.details.join("; "),
              );
              if (!proceed) {
                log("Add cancelled (existing addon files kept).");
                return false;
              }

              for (const repoId of parsed.conflictingRepoIds) {
                const conflictRepo = state.repos.find((repo) => Number(repo.id) === Number(repoId));
                const conflictLabel = conflictRepo
                  ? `${conflictRepo.owner}/${conflictRepo.name}`
                  : `repo #${repoId}`;
                try {
                  const removeMsg = await safeInvoke("wuddle_remove_repo", {
                    id: repoId,
                    removeLocalFiles: true,
                    wowDir: wowDirForProbe,
                  });
                  log(`${conflictLabel}: ${removeMsg}`);
                } catch (removeErr) {
                  log(`ERROR replace conflict ${conflictLabel}: ${removeErr.message}`);
                  return false;
                }
              }
              if (parsed.conflictingRepoIds.size > 0) {
                await loadRepos();
                render();
              }
              if (parsed.hasLocalOnlyConflicts || parsed.conflictingRepoIds.size > 0) {
                replaceAddonConflictsOnInstall = true;
              }
            }
          } catch (probeErr) {
            log(`WARNING add: addon conflict pre-check failed (${probeErr.message}).`);
          }
        }

        const sameAddonConflicts = trackedAddonConflictsForUrl(url);
        if (sameAddonConflicts.length > 0) {
          const wowDirForReplace = currentWowDirStrict();
          if (!wowDirForReplace) {
            log(
              "ERROR add: WoW path is required to replace an existing conflicting addon. Configure path first.",
            );
            return false;
          }
          const conflictDetails = sameAddonConflicts
            .map((r) => `${r.name} [already tracked by ${r.owner}/${r.name}]`)
            .join("; ");
          const proceed = await confirmAddonConflict(
            {
              owner: incomingInfo.owner || "incoming",
              name: addonNameFromUrl(url) || incomingInfo.name || String(url || "").trim(),
              host: incomingInfo.host || "",
              url,
            },
            conflictDetails,
          );
          if (!proceed) {
            log("Add cancelled (existing addon kept).");
            return false;
          }

          for (const conflictRepo of sameAddonConflicts) {
            try {
              const removeMsg = await safeInvoke("wuddle_remove_repo", {
                id: conflictRepo.id,
                removeLocalFiles: true,
                wowDir: wowDirForReplace,
              });
              log(`${conflictRepo.owner}/${conflictRepo.name}: ${removeMsg}`);
            } catch (removeErr) {
              log(
                `ERROR replace conflict ${conflictRepo.owner}/${conflictRepo.name}: ${removeErr.message}`,
              );
              return false;
            }
          }
          replaceAddonConflictsOnInstall = true;
          await loadRepos();
          render();
        }
      }

      const knownIds = new Set(state.repos.map((r) => r.id));
      const id = await safeInvoke("wuddle_add_repo", { url, mode }, { timeoutMs: 30000 });
      const alreadyTracked = knownIds.has(id);
      if (alreadyTracked) {
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
            ...installOptions({
              replaceAddonConflicts: replaceAddonConflictsOnInstall,
            }),
          });
          if (result) {
            logOperationResult(result);
          } else {
            const reinstallResult = await safeInvoke("wuddle_reinstall_repo", {
              id,
              wowDir,
              ...installOptions({
                replaceAddonConflicts: replaceAddonConflictsOnInstall,
              }),
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
              if (!alreadyTracked) {
                try {
                  await safeInvoke("wuddle_remove_repo", {
                    id,
                    removeLocalFiles: false,
                    wowDir: null,
                  });
                  log(
                    `${repoLabel}: install cancelled (existing addon files kept). Removed new conflicting repo from tracking.`,
                  );
                } catch (removeErr) {
                  log(
                    `${repoLabel}: cancelled install (existing addon files kept), but failed to remove newly added conflicting repo: ${removeErr.message}`,
                  );
                }
              } else {
                log(`${repoLabel}: cancelled install (existing addon files kept).`);
              }
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

export async function retryFailedFetches() {
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

export async function rescanAddonDirectory() {
  const profile = ensureActiveProfile();
  if (!profile) return;
  const wowDir = readWowDir();
  if (!wowDir) {
    log("ERROR addon rescan: WoW path is empty for active instance.");
    return;
  }

  log("Rescanning Interface/AddOns…");
  await withBusy(async () => {
    try {
      await setBackendActiveProfile();
      await loadRepos();
      render();
      log(`Addon rescan complete. Loaded ${state.repos.length} repo(s).`);
    } catch (e) {
      log(`ERROR addon rescan: ${e.message}`);
    }
  });
}

// ============================================================================
// Repo Detail Dialog
// ============================================================================

let _detailAbortKey = 0;
let _detailShowChangelog = false;
let _filePreviewActive = false;
let _filePreviewPrevView = null; // "readme" or "releasenotes"
const _releaseNotesCache = new Map();

function openRepoDetailDialog(repo) {
  const dlg = $("dlgRepoDetail");
  if (!dlg) return;

  // Header — title is a clickable link to the repo URL
  const titleEl = $("detailTitle");
  titleEl.textContent = repo.name;
  titleEl.onclick = (ev) => {
    ev.preventDefault();
    openUrl(repo.url);
  };
  titleEl.title = `Open ${repo.url}`;

  const forgeLabel = displayForge(repo);
  $("detailSubtitle").textContent = `${repo.owner} \u2022 ${forgeLabel}`;

  // Footer — forge icon button
  const forgeName = forgeLabel.charAt(0).toUpperCase() + forgeLabel.slice(1);
  const openForgeBtn = $("btnDetailOpenForge");
  openForgeBtn.innerHTML = forgeIconSvg(forgeLabel);
  openForgeBtn.title = `Open on ${forgeName}`;
  openForgeBtn.onclick = () => openUrl(repo.url);

  // Footer — "Release Notes" toggle button
  const isMod = !isAddonRepo(repo);
  _detailShowChangelog = isMod; // mods default to Release Notes view
  _filePreviewActive = false;
  _filePreviewPrevView = null;
  const clBtn = $("btnDetailChangelog");
  clBtn.textContent = isMod ? "README" : "Release Notes";
  clBtn.onclick = () => toggleDetailReleaseNotes();

  // Clear previous content
  $("detailReadme").innerHTML = "";
  $("detailChangelog").innerHTML = "";
  const fpEl = $("detailFilePreview");
  if (fpEl) { fpEl.innerHTML = ""; fpEl.classList.add("hidden"); }
  $("detailFrameLabel").onclick = null;
  $("detailFrameLabel").classList.remove("frame-label-back");

  if (isMod) {
    $("detailReadme").classList.add("hidden");
    $("detailChangelog").classList.remove("hidden");
    $("detailFrameLabel").textContent = "Release Notes";
  } else {
    $("detailChangelog").classList.add("hidden");
    $("detailReadme").classList.remove("hidden");
    $("detailFrameLabel").textContent = "README";
  }
  $("detailAbout").innerHTML = "";
  $("detailFiles").innerHTML = "";
  $("detailAboutSection").classList.add("hidden");
  $("detailFilesSection").classList.add("hidden");
  $("detailSidePanel").classList.add("hidden");

  // Set repo URL context for README relative URL resolution
  _activeReadmeRepoUrl = repo.url;

  // Open dialog first so layout is ready for scroll-fade
  if (!dlg.open) dlg.showModal();

  // Wire close buttons
  $("btnDetailCloseX").onclick = () => dlg.close();
  $("btnDetailClose").onclick = () => dlg.close();

  // Fire all fetches in parallel
  const generation = ++_detailAbortKey;
  fetchDetailReadme(repo, generation);
  fetchDetailAbout(repo, generation);
  fetchDetailFiles(repo, generation);
  fetchDetailChangelog(repo, generation);
}

// ---------------------------------------------------------------------------
// Release Notes toggle
// ---------------------------------------------------------------------------

function toggleDetailReleaseNotes() {
  // If file preview is active, close it first
  if (_filePreviewActive) {
    hideFilePreview();
    return;
  }

  _detailShowChangelog = !_detailShowChangelog;
  const readme = $("detailReadme");
  const changelog = $("detailChangelog");
  const label = $("detailFrameLabel");
  const btn = $("btnDetailChangelog");

  if (_detailShowChangelog) {
    readme.classList.add("hidden");
    changelog.classList.remove("hidden");
    label.textContent = "Release Notes";
    label.classList.remove("frame-label-back");
    label.onclick = null;
    btn.textContent = "README";
  } else {
    changelog.classList.add("hidden");
    readme.classList.remove("hidden");
    label.textContent = "README";
    label.classList.remove("frame-label-back");
    label.onclick = null;
    btn.textContent = "Release Notes";
  }

  // Reset scroll and refresh fade after switching views
  const frame = readme.closest(".scroll-fade");
  if (frame) {
    frame.scrollTop = 0;
    requestAnimationFrame(() => refreshScrollFade(frame));
  }
}

// ---------------------------------------------------------------------------
// File preview in main content area
// ---------------------------------------------------------------------------

const MD_EXTENSIONS = new Set(["md", "markdown", "mdown", "mkd"]);

function showFilePreview(target, name, contentHtml) {
  const isDetail = target === "detail";
  const previewEl = $(isDetail ? "detailFilePreview" : "addFilePreview");
  const labelEl = $(isDetail ? "detailFrameLabel" : "contentFrameLabel");
  if (!previewEl || !labelEl) return;

  if (isDetail) {
    if (!_filePreviewActive) {
      _filePreviewPrevView = _detailShowChangelog ? "releasenotes" : "readme";
    }
    _filePreviewActive = true;
    $("detailReadme").classList.add("hidden");
    $("detailChangelog").classList.add("hidden");
  } else {
    const wrap = $("readmePreviewWrap");
    if (wrap) wrap.classList.add("hidden");
    const qa = $("quickAddField");
    if (qa) qa.classList.add("hidden");
  }

  previewEl.innerHTML = contentHtml;
  previewEl.classList.remove("hidden");

  labelEl.textContent = `\u2190 ${name}`;
  labelEl.classList.add("frame-label-back");
  labelEl.onclick = () => hideFilePreview(target);

  const frame = previewEl.closest(".scroll-fade");
  if (frame) {
    frame.scrollTop = 0;
    requestAnimationFrame(() => refreshScrollFade(frame));
  }
}

function hideFilePreview(target) {
  const isDetail = target === "detail";
  const previewEl = $(isDetail ? "detailFilePreview" : "addFilePreview");
  const labelEl = $(isDetail ? "detailFrameLabel" : "contentFrameLabel");
  if (!previewEl || !labelEl) return;

  previewEl.innerHTML = "";
  previewEl.classList.add("hidden");
  labelEl.classList.remove("frame-label-back");
  labelEl.onclick = null;

  if (isDetail) {
    _filePreviewActive = false;
    if (_filePreviewPrevView === "releasenotes") {
      $("detailChangelog").classList.remove("hidden");
      labelEl.textContent = "Release Notes";
    } else {
      $("detailReadme").classList.remove("hidden");
      labelEl.textContent = "README";
    }
  } else {
    const wrap = $("readmePreviewWrap");
    if (wrap && wrap.querySelector(".readme-preview")?.innerHTML) {
      wrap.classList.remove("hidden");
      labelEl.textContent = "README";
    } else {
      const qa = $("quickAddField");
      if (qa) qa.classList.remove("hidden");
      labelEl.textContent = "Quick Add";
    }
  }

  const frame = previewEl.closest(".scroll-fade");
  if (frame) {
    frame.scrollTop = 0;
    requestAnimationFrame(() => refreshScrollFade(frame));
  }
}

function renderFileContent(name, text) {
  const ext = (name.split(".").pop() || "").toLowerCase();
  if (MD_EXTENSIONS.has(ext)) {
    return changelogToHtml(text);
  }
  const highlighted = highlightCode(text, name);
  if (highlighted) {
    return `<pre class="file-preview-code hljs">${highlighted}</pre>`;
  }
  return `<pre class="file-preview-code">${escapeHtml(text)}</pre>`;
}

async function previewFile(context, path, name) {
  const target = context.target || "detail";
  showFilePreview(target, name, '<div class="readme-preview-loading">Loading\u2026</div>');
  try {
    let content;
    if (context.type === "local") {
      content = await safeInvoke("wuddle_read_local_file", { wowDir: context.wowDir, path }, { timeoutMs: 10000 });
    } else {
      content = await safeInvoke("wuddle_fetch_repo_file", { url: context.repoUrl, path }, { timeoutMs: 15000 });
    }
    showFilePreview(target, name, renderFileContent(name, content));
  } catch (e) {
    const msg = String(e).replace(/^Error:\s*/i, "");
    showFilePreview(target, name, `<div class="readme-preview-loading">${escapeHtml(msg)}</div>`);
  }
}

// ---------------------------------------------------------------------------
// Detail fetchers
// ---------------------------------------------------------------------------

async function fetchDetailReadme(repo, generation) {
  const container = $("detailReadme");
  if (!container) return;

  const cacheKey = repoKeyFromUrl(repo.url);
  if (cacheKey && _readmeCache.has(cacheKey)) {
    container.innerHTML = _readmeCache.get(cacheKey);
    wireReadmeLinks(container);
    return;
  }

  container.innerHTML = '<div class="readme-preview-loading">Loading README\u2026</div>';

  try {
    const html = await safeInvoke("wuddle_fetch_repo_readme", { url: repo.url }, { timeoutMs: 15000 });
    if (generation !== _detailAbortKey) return;

    let rendered;
    if (html.startsWith("<!--md-->")) {
      rendered = changelogToHtml(html.slice(9));
    } else {
      rendered = sanitizeReadmeHtml(html);
    }

    if (cacheKey) cappedSet(_readmeCache, cacheKey, rendered);
    container.innerHTML = rendered;
    wireReadmeLinks(container);
  } catch (_) {
    if (generation !== _detailAbortKey) return;
    container.innerHTML = '<div class="readme-preview-loading">README not available.</div>';
  }
}

async function fetchDetailAbout(repo, generation) {
  const section = $("detailAboutSection");
  const container = $("detailAbout");
  if (!section || !container) return;

  const cacheKey = repoKeyFromUrl(repo.url);
  if (cacheKey && _repoInfoCache.has(cacheKey)) {
    container.innerHTML = _repoInfoCache.get(cacheKey);
    wireAboutLinks(container);
    section.classList.remove("hidden");
    updateDetailSidePanelVisibility();
    return;
  }

  container.innerHTML = '<span class="repo-info-loading">Loading\u2026</span>';
  section.classList.remove("hidden");
  updateDetailSidePanelVisibility();

  try {
    const json = await safeInvoke("wuddle_fetch_repo_info", { url: repo.url }, { timeoutMs: 15000 });
    if (generation !== _detailAbortKey) return;
    const data = JSON.parse(json);
    const html = renderRepoAbout(data);
    if (cacheKey) cappedSet(_repoInfoCache, cacheKey, html);
    container.innerHTML = html;
    wireAboutLinks(container);
  } catch (_) {
    if (generation !== _detailAbortKey) return;
    container.innerHTML = "";
    section.classList.add("hidden");
    updateDetailSidePanelVisibility();
  }
}

async function fetchDetailChangelog(repo, generation) {
  const container = $("detailChangelog");
  if (!container) return;

  const cacheKey = repoKeyFromUrl(repo.url);
  if (cacheKey && _releaseNotesCache.has(cacheKey)) {
    container.innerHTML = _releaseNotesCache.get(cacheKey);
    return;
  }

  container.innerHTML = '<div class="readme-preview-loading">Loading release notes\u2026</div>';

  try {
    const json = await safeInvoke("wuddle_fetch_repo_releases", { url: repo.url }, { timeoutMs: 20000 });
    if (generation !== _detailAbortKey) return;
    const releases = JSON.parse(json);

    if (!releases.length) {
      const html = '<div class="readme-preview-loading">No release notes found.</div>';
      if (cacheKey) cappedSet(_releaseNotesCache, cacheKey, html);
      container.innerHTML = html;
      return;
    }

    const html = renderReleases(releases);
    if (cacheKey) cappedSet(_releaseNotesCache, cacheKey, html);
    container.innerHTML = html;
  } catch (_) {
    if (generation !== _detailAbortKey) return;
    container.innerHTML = '<div class="readme-preview-loading">Could not load release notes.</div>';
  }
}

function renderReleases(releases) {
  let html = "";
  for (const rel of releases) {
    const tag = escapeHtml(rel.tag || "");
    const name = escapeHtml(rel.name || "");
    const title = name && name !== tag ? `${tag} \u2014 ${name}` : tag;

    let meta = "";
    if (rel.publishedAt) {
      try {
        const d = new Date(rel.publishedAt);
        meta = d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
      } catch (_) {
        meta = escapeHtml(rel.publishedAt);
      }
    }
    if (rel.prerelease) {
      meta += '<span class="release-pre-badge">pre-release</span>';
    }

    let body = "";
    if (rel.body && rel.body.trim()) {
      body = `<div class="release-body">${changelogToHtml(rel.body)}</div>`;
    }

    html += `<div class="release-entry">`;
    html += `<div class="release-tag">${title}</div>`;
    if (meta) html += `<div class="release-meta">${meta}</div>`;
    html += body;
    html += `</div>`;
  }
  return html;
}

// ---------------------------------------------------------------------------
// Detail files with path context
// ---------------------------------------------------------------------------

async function fetchDetailFiles(repo, generation) {
  const section = $("detailFilesSection");
  const container = $("detailFiles");
  if (!section || !container) return;

  const wowDir = readWowDir();
  if (!wowDir) {
    section.classList.add("hidden");
    updateDetailSidePanelVisibility();
    return;
  }

  container.innerHTML = '<span class="repo-info-loading">Loading\u2026</span>';
  section.classList.remove("hidden");
  updateDetailSidePanelVisibility();

  try {
    const installsJson = await safeInvoke("wuddle_list_repo_installs", { id: repo.id }, { timeoutMs: 10000 });
    if (generation !== _detailAbortKey) return;
    const installs = JSON.parse(installsJson);

    if (!installs.length) {
      container.innerHTML = '<span class="repo-info-loading">No installed files found.</span>';
      refreshScrollFade(section);
      return;
    }

    const frag = document.createDocumentFragment();

    for (const entry of installs) {
      if (entry.kind === "addon") {
        // For addon dirs, fetch local directory contents
        const parentPath = entry.path.includes("/")
          ? entry.path.substring(0, entry.path.lastIndexOf("/") + 1)
          : "";
        const dirName = entry.path.split("/").pop() || entry.path;

        try {
          const filesJson = await safeInvoke(
            "wuddle_list_local_files",
            { wowDir, basePath: entry.path },
            { timeoutMs: 5000 },
          );
          if (generation !== _detailAbortKey) return;
          const files = JSON.parse(filesJson);

          const details = document.createElement("details");
          details.className = "tree-dir-details";
          details.open = true;
          const summary = document.createElement("summary");
          summary.className = "tree-entry dir";
          summary.innerHTML = `<span class="tree-icon">\ud83d\udcc1</span>${parentPath ? `<span class="tree-path-prefix">${escapeHtml(parentPath)}</span>` : ""}<span class="tree-name">${escapeHtml(dirName)}</span>`;
          details.appendChild(summary);

          const localCtx = { type: "local", wowDir, target: "detail" };
          const childWrap = document.createElement("div");
          childWrap.className = "tree-children";
          childWrap.appendChild(renderTreeEntries(files, entry.path, localCtx));
          details.appendChild(childWrap);
          frag.appendChild(details);
        } catch (_) {
          const div = document.createElement("div");
          div.className = "tree-entry dir";
          div.innerHTML = `<span class="tree-icon">\ud83d\udcc1</span><span class="tree-name">${escapeHtml(entry.path)}</span>`;
          frag.appendChild(div);
        }
      } else {
        // For dll/raw files, show with full path
        const localCtx = { type: "local", wowDir, target: "detail" };
        const div = document.createElement("div");
        div.className = "tree-entry file";
        div.innerHTML = `<span class="tree-icon">\ud83d\udcc4</span><span class="tree-name">${escapeHtml(entry.path)}</span>`;
        div.addEventListener("click", () => previewFile(localCtx, entry.path, entry.path));
        frag.appendChild(div);
      }
    }

    container.innerHTML = "";
    container.appendChild(frag);
    refreshScrollFade(section);
  } catch (_) {
    if (generation !== _detailAbortKey) return;
    container.innerHTML = "";
    section.classList.add("hidden");
    updateDetailSidePanelVisibility();
  }
}

function updateDetailSidePanelVisibility() {
  const panel = $("detailSidePanel");
  if (!panel) return;
  const aboutVisible = !$("detailAboutSection")?.classList.contains("hidden");
  const filesVisible = !$("detailFilesSection")?.classList.contains("hidden");
  if (aboutVisible || filesVisible) {
    panel.classList.remove("hidden");
  } else {
    panel.classList.add("hidden");
  }
}
