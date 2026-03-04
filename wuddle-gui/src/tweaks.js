// Tweaks tab: UI logic for the WoW.exe patcher (based on vanilla-tweaks by brndd).

import { state } from "./state.js";
import { $ } from "./utils.js";
import { safeInvoke } from "./commands.js";
import { activeProfile, profileWowDir } from "./profiles.js";
import { openUrl } from "./repos.js";

const VANILLA_TWEAKS_URL = "https://github.com/brndd/vanilla-tweaks";

// Tweak definition table: maps UI element IDs to their roles.
const SLIDER_TWEAKS = [
  { enableId: "tweakFovEnabled", sliderId: "tweakFovValue", displayId: "tweakFovDisplay" },
  { enableId: "tweakFarclipEnabled", sliderId: "tweakFarclipValue", displayId: "tweakFarclipDisplay" },
  { enableId: "tweakFrilldistanceEnabled", sliderId: "tweakFrilldistanceValue", displayId: "tweakFrilldistanceDisplay" },
  { enableId: "tweakNameplateEnabled", sliderId: "tweakNameplateValue", displayId: "tweakNameplateDisplay" },
];

const TWEAKS_STORAGE_PREFIX = "wuddle.tweaks.";

function storageKey() {
  return TWEAKS_STORAGE_PREFIX + (state.activeProfileId || "default");
}

function saveTweakSettings() {
  const data = {
    fovEnabled: $("tweakFovEnabled").checked,
    fovValue: $("tweakFovValue").value,
    farclipEnabled: $("tweakFarclipEnabled").checked,
    farclipValue: $("tweakFarclipValue").value,
    frilldistanceEnabled: $("tweakFrilldistanceEnabled").checked,
    frilldistanceValue: $("tweakFrilldistanceValue").value,
    nameplateEnabled: $("tweakNameplateEnabled").checked,
    nameplateValue: $("tweakNameplateValue").value,
    cameraSkipFix: $("tweakCameraSkipFix").checked,
    maxCameraEnabled: $("tweakMaxCameraEnabled").checked,
    maxCameraValue: $("tweakMaxCameraValue").value,
    soundBackground: $("tweakSoundBackground").checked,
    soundChannelsEnabled: $("tweakSoundChannelsEnabled").checked,
    soundChannelsValue: $("tweakSoundChannelsValue").value,
    quickloot: $("tweakQuickloot").checked,
    largeAddress: $("tweakLargeAddress").checked,
  };
  localStorage.setItem(storageKey(), JSON.stringify(data));
}

function loadTweakSettings() {
  try {
    const raw = localStorage.getItem(storageKey());
    if (!raw) return;
    const d = JSON.parse(raw);
    if (typeof d.fovEnabled === "boolean") $("tweakFovEnabled").checked = d.fovEnabled;
    if (d.fovValue != null) $("tweakFovValue").value = d.fovValue;
    if (typeof d.farclipEnabled === "boolean") $("tweakFarclipEnabled").checked = d.farclipEnabled;
    if (d.farclipValue != null) $("tweakFarclipValue").value = d.farclipValue;
    if (typeof d.frilldistanceEnabled === "boolean") $("tweakFrilldistanceEnabled").checked = d.frilldistanceEnabled;
    if (d.frilldistanceValue != null) $("tweakFrilldistanceValue").value = d.frilldistanceValue;
    if (typeof d.nameplateEnabled === "boolean") $("tweakNameplateEnabled").checked = d.nameplateEnabled;
    if (d.nameplateValue != null) $("tweakNameplateValue").value = d.nameplateValue;
    if (typeof d.cameraSkipFix === "boolean") $("tweakCameraSkipFix").checked = d.cameraSkipFix;
    if (typeof d.maxCameraEnabled === "boolean") $("tweakMaxCameraEnabled").checked = d.maxCameraEnabled;
    if (d.maxCameraValue != null) $("tweakMaxCameraValue").value = d.maxCameraValue;
    if (typeof d.soundBackground === "boolean") $("tweakSoundBackground").checked = d.soundBackground;
    if (typeof d.soundChannelsEnabled === "boolean") $("tweakSoundChannelsEnabled").checked = d.soundChannelsEnabled;
    if (d.soundChannelsValue != null) $("tweakSoundChannelsValue").value = d.soundChannelsValue;
    if (typeof d.quickloot === "boolean") $("tweakQuickloot").checked = d.quickloot;
    if (typeof d.largeAddress === "boolean") $("tweakLargeAddress").checked = d.largeAddress;
  } catch (_) {}
}

function getWowDir() {
  const prof = activeProfile();
  if (!prof) return null;
  return profileWowDir(prof) || null;
}

function buildOpts() {
  return {
    fov: $("tweakFovEnabled").checked ? parseFloat($("tweakFovValue").value) : null,
    farclip: $("tweakFarclipEnabled").checked ? parseFloat($("tweakFarclipValue").value) : null,
    frilldistance: $("tweakFrilldistanceEnabled").checked ? parseFloat($("tweakFrilldistanceValue").value) : null,
    nameplateDistance: $("tweakNameplateEnabled").checked ? parseFloat($("tweakNameplateValue").value) : null,
    soundChannels: $("tweakSoundChannelsEnabled").checked ? parseInt($("tweakSoundChannelsValue").value, 10) : null,
    maxCameraDistance: $("tweakMaxCameraEnabled").checked ? parseFloat($("tweakMaxCameraValue").value) : null,
    quickloot: $("tweakQuickloot").checked,
    soundInBackground: $("tweakSoundBackground").checked,
    largeAddressAware: $("tweakLargeAddress").checked,
    cameraSkipFix: $("tweakCameraSkipFix").checked,
  };
}

function fovDisplayText(val) {
  const deg = Math.round(parseFloat(val) * (180 / Math.PI));
  return `${val} (~${deg}°)`;
}

function syncSliderDisplays() {
  for (const t of SLIDER_TWEAKS) {
    const slider = $(t.sliderId);
    const display = $(t.displayId);
    if (!slider || !display) continue;
    if (t.sliderId === "tweakFovValue") {
      display.textContent = fovDisplayText(slider.value);
    } else {
      display.textContent = slider.value;
    }
  }
}

function syncConditionalInputs() {
  $("tweakMaxCameraValue").disabled = !$("tweakMaxCameraEnabled").checked;
  $("tweakSoundChannelsValue").disabled = !$("tweakSoundChannelsEnabled").checked;
  for (const t of SLIDER_TWEAKS) {
    $(t.sliderId).disabled = !$(t.enableId).checked;
  }
}

export function renderTweaks() {
  const wowDir = getWowDir();
  const hasDir = !!wowDir;
  $("tweaksNoDir").classList.toggle("hidden", hasDir);
  $("tweaksGrid").classList.toggle("hidden", !hasDir);
  $("btnApplyTweaks").disabled = !hasDir;
  $("btnReadTweaks").disabled = !hasDir;

  if (hasDir) {
    void checkBackup(wowDir);
  } else {
    $("btnRestoreTweaks").disabled = true;
  }

  loadTweakSettings();
  syncSliderDisplays();
  syncConditionalInputs();
}

async function checkBackup(wowDir) {
  try {
    const has = await safeInvoke("wuddle_has_tweaks_backup", { wowDir });
    $("btnRestoreTweaks").disabled = !has;
  } catch (_) {
    $("btnRestoreTweaks").disabled = true;
  }
}

async function applyTweaks() {
  const wowDir = getWowDir();
  if (!wowDir) return;
  const btn = $("btnApplyTweaks");
  btn.disabled = true;
  btn.textContent = "Applying…";
  try {
    const opts = buildOpts();
    const result = await safeInvoke("wuddle_apply_tweaks", { wowDir, opts });
    btn.textContent = "Applied!";
    saveTweakSettings();
    void checkBackup(wowDir);
    setTimeout(() => { btn.textContent = "Apply"; btn.disabled = false; }, 2000);
    alert(result);
  } catch (e) {
    btn.textContent = "Apply";
    btn.disabled = false;
    alert("Failed to apply tweaks: " + e.message);
  }
}

async function restoreBackup() {
  const wowDir = getWowDir();
  if (!wowDir) return;
  const btn = $("btnRestoreTweaks");
  btn.disabled = true;
  btn.textContent = "Restoring…";
  try {
    const result = await safeInvoke("wuddle_restore_tweaks_backup", { wowDir });
    btn.textContent = "Restore";
    void checkBackup(wowDir);
    alert(result);
  } catch (e) {
    btn.textContent = "Restore";
    btn.disabled = false;
    alert("Failed to restore backup: " + e.message);
  }
}

async function readCurrent() {
  const wowDir = getWowDir();
  if (!wowDir) return;
  const btn = $("btnReadTweaks");
  btn.disabled = true;
  btn.textContent = "Reading…";
  try {
    const v = await safeInvoke("wuddle_read_tweaks", { wowDir });

    // Populate slider tweaks — enable all checkboxes and set values
    $("tweakFovEnabled").checked = true;
    $("tweakFovValue").value = v.fov;
    $("tweakFarclipEnabled").checked = true;
    $("tweakFarclipValue").value = v.farclip;
    $("tweakFrilldistanceEnabled").checked = true;
    $("tweakFrilldistanceValue").value = v.frilldistance;
    $("tweakNameplateEnabled").checked = true;
    $("tweakNameplateValue").value = v.nameplateDistance;

    // Number input tweaks
    $("tweakMaxCameraEnabled").checked = true;
    $("tweakMaxCameraValue").value = v.maxCameraDistance;
    $("tweakSoundChannelsEnabled").checked = true;
    $("tweakSoundChannelsValue").value = v.soundChannels;

    // Boolean tweaks
    $("tweakCameraSkipFix").checked = v.cameraSkipFix;
    $("tweakSoundBackground").checked = v.soundInBackground;
    $("tweakQuickloot").checked = v.quickloot;
    $("tweakLargeAddress").checked = v.largeAddressAware;

    syncSliderDisplays();
    syncConditionalInputs();
    saveTweakSettings();

    btn.textContent = "Read!";
    setTimeout(() => { btn.textContent = "Read Current"; btn.disabled = false; }, 2000);
  } catch (e) {
    btn.textContent = "Read Current";
    btn.disabled = false;
    alert("Failed to read tweaks: " + e.message);
  }
}

const DEFAULTS = {
  fovEnabled: true, fovValue: "1.925",
  farclipEnabled: true, farclipValue: "1000",
  frilldistanceEnabled: true, frilldistanceValue: "300",
  nameplateEnabled: true, nameplateValue: "41",
  cameraSkipFix: true,
  maxCameraEnabled: true, maxCameraValue: "50",
  soundBackground: true,
  soundChannelsEnabled: true, soundChannelsValue: "64",
  quickloot: true,
  largeAddress: true,
};

function resetToDefault() {
  $("tweakFovEnabled").checked = DEFAULTS.fovEnabled;
  $("tweakFovValue").value = DEFAULTS.fovValue;
  $("tweakFarclipEnabled").checked = DEFAULTS.farclipEnabled;
  $("tweakFarclipValue").value = DEFAULTS.farclipValue;
  $("tweakFrilldistanceEnabled").checked = DEFAULTS.frilldistanceEnabled;
  $("tweakFrilldistanceValue").value = DEFAULTS.frilldistanceValue;
  $("tweakNameplateEnabled").checked = DEFAULTS.nameplateEnabled;
  $("tweakNameplateValue").value = DEFAULTS.nameplateValue;
  $("tweakCameraSkipFix").checked = DEFAULTS.cameraSkipFix;
  $("tweakMaxCameraEnabled").checked = DEFAULTS.maxCameraEnabled;
  $("tweakMaxCameraValue").value = DEFAULTS.maxCameraValue;
  $("tweakSoundBackground").checked = DEFAULTS.soundBackground;
  $("tweakSoundChannelsEnabled").checked = DEFAULTS.soundChannelsEnabled;
  $("tweakSoundChannelsValue").value = DEFAULTS.soundChannelsValue;
  $("tweakQuickloot").checked = DEFAULTS.quickloot;
  $("tweakLargeAddress").checked = DEFAULTS.largeAddress;

  syncSliderDisplays();
  syncConditionalInputs();
  saveTweakSettings();
}

export function bindTweaksListeners() {
  $("btnApplyTweaks").addEventListener("click", applyTweaks);
  $("btnRestoreTweaks").addEventListener("click", restoreBackup);
  $("btnReadTweaks").addEventListener("click", readCurrent);
  $("btnResetTweaks").addEventListener("click", resetToDefault);
  $("btnTweaksAttribution").addEventListener("click", async () => {
    await openUrl(VANILLA_TWEAKS_URL);
  });
  $("btnAboutVanillaTweaks").addEventListener("click", async () => {
    await openUrl(VANILLA_TWEAKS_URL);
  });

  // Slider tweaks: sync display on input, save on change
  for (const t of SLIDER_TWEAKS) {
    $(t.sliderId).addEventListener("input", () => {
      const val = $(t.sliderId).value;
      $(t.displayId).textContent = t.sliderId === "tweakFovValue" ? fovDisplayText(val) : val;
    });
    $(t.sliderId).addEventListener("change", saveTweakSettings);
    $(t.enableId).addEventListener("change", () => {
      $(t.sliderId).disabled = !$(t.enableId).checked;
      saveTweakSettings();
    });
  }

  // Max camera distance: toggle enables/disables number input
  $("tweakMaxCameraEnabled").addEventListener("change", () => {
    $("tweakMaxCameraValue").disabled = !$("tweakMaxCameraEnabled").checked;
    saveTweakSettings();
  });
  $("tweakMaxCameraValue").addEventListener("change", saveTweakSettings);

  // Sound channels: toggle enables/disables number input
  $("tweakSoundChannelsEnabled").addEventListener("change", () => {
    $("tweakSoundChannelsValue").disabled = !$("tweakSoundChannelsEnabled").checked;
    saveTweakSettings();
  });
  $("tweakSoundChannelsValue").addEventListener("change", saveTweakSettings);

  // Simple checkboxes
  $("tweakCameraSkipFix").addEventListener("change", saveTweakSettings);
  $("tweakSoundBackground").addEventListener("change", saveTweakSettings);
  $("tweakQuickloot").addEventListener("change", saveTweakSettings);
  $("tweakLargeAddress").addEventListener("change", saveTweakSettings);
}
