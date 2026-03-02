// Home tab: update summary display and game launch
import { state } from "./state.js";
import { $ } from "./utils.js";
import { safeInvoke } from "./commands.js";
import { log } from "./logs.js";
import { withBusy } from "./ui.js";
import {
  activeProfile,
  readWowDir,
  profileExecutablePath,
  profileLikesTurtles,
  launchSummary,
  launchPayload,
} from "./profiles.js";
import {
  reposForView,
  canUpdateRepo,
  updateCounts,
  escapeHtml,
} from "./repos.js";

let _refreshAll = async () => {};
let _openAddDialogFor = (_view) => {};
export function setHomeCallbacks(cbs) {
  if (cbs.refreshAll) _refreshAll = cbs.refreshAll;
  if (cbs.openAddDialogFor) _openAddDialogFor = cbs.openAddDialogFor;
}

export function renderHome() {
  const profile = activeProfile();
  const hasProfile = !!profile;
  const turtlesEnabled = profileLikesTurtles(profile);
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

  const turtleSection = $("homeTurtleSection");
  if (turtleSection) {
    turtleSection.classList.toggle("hidden", !hasProfile || !turtlesEnabled);
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

export async function launchGameFromHome() {
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
