export const CURATED_MOD_PRESETS = [
  {
    id: "vanillafixes",
    name: "VanillaFixes",
    url: "https://github.com/hannesmann/vanillafixes",
    mode: "auto",
    description:
      "A client modification for World of Warcraft 1.6.1-1.12.1 to eliminate stutter and animation lag. VanillaFixes also acts as a launcher (start game via VanillaFixes.exe instead of Wow.exe) and DLL mod loader which loads DLL files listed in dlls.txt found in the WoW install directory.",
    warning:
      "VanillaFixes may trigger antivirus false-positive alerts on Windows.",
    categories: ["Performance"],
    recommended: true,
  },
  {
    id: "interact",
    name: "Interact",
    url: "https://github.com/lookino/Interact",
    mode: "auto",
    description:
      "Legacy WoW client mod for 1.12 that brings Dragonflight-style interact key support to Vanilla, reducing click friction and improving moment-to-moment gameplay.",
    categories: ["QoL"],
    recommended: false,
  },
  {
    id: "unitxp_sp3",
    name: "UnitXP_SP3",
    url: "https://codeberg.org/konaka/UnitXP_SP3",
    mode: "auto",
    description:
      "Adds optional camera offset, proper nameplates (showing only with LoS), improved tab-targeting keybind behavior, LoS and distance checks in Lua, screenshot format options, network tweaks, background notifications, and additional QoL features.",
    warning:
      "UnitXP_SP3 may trigger antivirus false-positive alerts on Windows.",
    categories: ["QoL", "API"],
    recommended: true,
  },
  {
    id: "nampower",
    name: "nampower",
    url: "https://gitea.com/avitasia/nampower",
    mode: "auto",
    description:
      "Addresses a 1.12 client casting limitation where follow-up casts wait on round-trip completion feedback. The result is reduced cast downtime and better effective DPS, especially on higher-latency connections.",
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
      "Utility library for WoW 1.12 adding file read/write helpers, minimap blip customization, larger allocator capacity, higher-resolution texture/skin support, and character morph-related functionality.",
    categories: ["API", "Performance"],
    recommended: true,
  },
];

export const PRESET_CATEGORY_CLASS = {
  qol: "cat-qol",
  api: "cat-api",
  performance: "cat-performance",
};
