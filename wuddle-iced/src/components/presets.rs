//! Quick Add preset data and card rendering.
//!
//! Mirrors the preset list from `wuddle-gui/src/presets.js`.
//! `build_quick_add_presets` renders the preset grid shown inside the
//! AddRepo dialog when the URL field is empty.

use iced::widget::{button, column, container, row, text, Space};
use iced::{Element, Length};

use crate::Message;
use crate::service::RepoRow;
use crate::theme::{self, ThemeColors};
use crate::components::helpers::{badge_tag, tip};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

pub struct Preset {
    pub name: &'static str,
    pub url: &'static str,
    pub description: &'static str,
    pub categories: &'static [&'static str],
    pub recommended: bool,
    pub warning: Option<&'static str>,
    pub companion_links: &'static [(&'static str, &'static str)],
    pub expanded_notes: &'static [&'static str],
    pub is_addon: bool,
}

pub const WEIRD_UTILS_DLLS: [&str; 11] = [
    "weirdutils.dll", "worldmarkers.dll", "pngscreenshots.dll", "transmogfix.dll", "customassets.dll", 
    "minimapicons.dll", "clickthrough.dll", "logsessions.dll", "healtextfix.dll", 
    "bigcursor.dll", "weirdperformance.dll"
];

pub static WEIRD_UTILS_DESCRIPTIONS: [(&str, &str); 11] = [
    ("weirdutils.dll", "The all-in-one package containing all WeirdUtils features. Includes performance optimizations, PNG screenshots, custom assets loading, and all quality-of-life modules from the project collection."),
    ("worldmarkers.dll", "Place up to 5 animated colored markers (Cataclysm style) at any position in the world, useful for raid positioning, pull planning, or route marking. Requires party/raid leader or raid assist.\n\n- `/worldmarker 1` through `/worldmarker 5` (or `/wm 1`) -- place a marker where your cursor is pointing\n- `/worldmarker 1 target` -- place a marker on a unit (player, target, mouseover, etc.)\n- `/clearworldmarker` (or `/cwm`) -- remove all markers\n- `/clearworldmarker 2` -- remove a specific marker\n\nKeybindings for placing each marker and clearing all markers are available in the Key Bindings menu.\n\nMarkers automatically sync with group members who also have WeirdUtils installed. When a leader/assist places or clears a marker, all group members see it. Markers persist across zone transitions and respawn when you return to the area."),
    ("pngscreenshots.dll", "Saves screenshots as compressed PNG files instead of the default uncompressed TGA format. Runs on a background thread with no frame drops.\n\nControlled via the `screenshotQuality` CVar (saved to `config.wtf`):\n- `/script SetCVar(\"screenshotQuality\", \"6\")` -- set compression level (1 = fast, 9 = smallest, default 6)\n- `/script SetCVar(\"screenshotQuality\", \"0\")` -- disable PNG, use original TGA format"),
    ("transmogfix.dll", "Eliminates FPS drops caused by rapid equipment visual updates when transmogged items lose durability. No configuration needed, install and forget."),
    ("customassets.dll", "Enables loading loose game asset files (models, textures, etc.) from the `Data/` directory without repacking MPQ archives. Place files in `Data/` mirroring the game's internal paths (e.g. `Data/Character/Troll/Female/TrollFemale.m2`) and they will be used instead of the MPQ version.\n\nAlso allows multi-character patch archive names (e.g. `patch-12.mpq`, `patch-jimbo.mpq`).\n\nPatch archives are sorted case-insensitively by filename - last in the sort gets highest priority, and all patches override the base archives.\n\nNo configuration needed, install and forget."),
    ("minimapicons.dll", "Adds TBC/WotLK-style minimap tracking icons for NPC types, game objects, and quest givers.\nReplaces the native tracking dropdown with a combined menu showing both spell tracking and NPC category tracking.\nCan be disabled from the normal AddOn menu. Preferences saved per-character.\n\n- Click the minimap **tracking icon** to open the dropdown\n- Check/uncheck **NPC categories** to toggle their minimap icons\n- **Spell tracking** (Hunter tracking, Find Herbs, etc.) remains available alongside NPC tracking\n- **\"Hide in Cities\"** toggle suppresses NPC icons in capital cities"),
    ("clickthrough.dll", "Smart cursor targeting that prioritizes useful interactions. Instead of always selecting the nearest object under the cursor, the module finds the most useful target along the ray in priority order.\n\n- **Lootable corpses** first, then interactable game objects/portals, then interactable NPCs, then normal selection\n- Dead non-lootable corpses can still be selected when nothing more useful is behind them\n- Disabled inside **battlegrounds** to prevent targeting objectives through enemy players\n\nCan replaces SuperWoW's `Clickthrough()` toggle with always-on smart targeting that doesn't require a manual toggle and preserves the ability to select dead bodies when needed. Disable corpse-clickthrough in SuperAPI if you want this."),
    ("logsessions.dll", "Organizes the combat, raw combat, and chat logs into per-character directories with timestamped filenames:\n\n`Logs\\<Realm>\\<Character>\\WoWChatLog_YYYYMMDD_HHMMSS.txt`\n`Logs\\<Realm>\\<Character>\\WoWCombatLog_YYYYMMDD_HHMMSS.txt`\n`Logs\\<Realm>\\<Character>\\WoWRawCombatLog_YYYYMMDD_HHMMSS.txt` (superwow only)\n\nEvery character login begins with a marker line identifying the character and realm.\nIf a log file for the same character was written to within the last 60 minutes, the same logfile will be used instead of creating a new one."),
    ("healtextfix.dll", "Fixes duplicate floating heal numbers caused by **SuperWoW 1.5**. Only relevant if you use SuperWoW. No configuration needed, install and forget."),
    ("bigcursor.dll", "Upscales the hardware cursor for improved visibility without losing sharpness. Supports fractional scales from 1.0 (off) to 4.0.\n\n- `/script SetCursorScale(1.2)` -- set cursor scale (default 1.2x)\n- `/script SetCursorScale(1)` -- disable (use original 32x32 cursor) \n\nThis value is saved to the `cursorScale` CVar in tenths: `/script SetCVar(\"cursorScale\", \"15\")` for 1.5x."),
    ("weirdperformance.dll", "Engine-level optimizations that reduce CPU time on math, rendering helpers, file lookups, and data decompression. No visual difference, no configuration needed.\n\n- **SIMD Math** - replaces 20+ internal math functions with SSE/AVX equivalents covering skeletal animation, particle rendering, frustum culling, collision detection, text glyph caching, and float-to-integer conversion\n- **Data Decompression** - swaps the game's 2004-era zlib with a modern library (2.2x faster)\n- **MPQ File Cache** - caches archive file lookups to skip archive chain walk\n- **Timer Calibration** - recalibrates the OS performance counter for accurate animation timing. Ported from [VanillaFixes](https://github.com/hannesmann/vanillafixes)"),
];

// ---------------------------------------------------------------------------
// Preset list
// ---------------------------------------------------------------------------

pub fn create_quick_add_presets() -> Vec<Preset> {
    vec![
        Preset {
            name: "VanillaFixes",
            url: "https://github.com/hannesmann/vanillafixes",
            description: "A client modification for World of Warcraft 1.6.1-1.12.1 to eliminate stutter and animation lag. VanillaFixes also acts as a launcher (start game via VanillaFixes.exe instead of Wow.exe) and DLL mod loader which loads DLL files listed in dlls.txt found in the WoW install directory.",
            categories: &["Performance"],
            recommended: true,
            warning: Some("VanillaFixes may trigger antivirus false-positive alerts on Windows."),
            companion_links: &[],
            expanded_notes: &[],
            is_addon: false,
        },
        Preset {
            name: "Interact",
            url: "https://github.com/lookino/Interact",
            description: "Legacy WoW client mod for 1.12 that brings Dragonflight-style interact key support to Vanilla, reducing click friction and improving moment-to-moment gameplay.",
            categories: &["QoL"],
            recommended: false,
            warning: None,
            companion_links: &[],
            expanded_notes: &[],
            is_addon: false,
        },
        Preset {
            name: "UnitXP_SP3",
            url: "https://codeberg.org/konaka/UnitXP_SP3",
            description: "Adds optional camera offset, proper nameplates (showing only with LoS), improved tab-targeting keybind behavior, LoS and distance checks in Lua, screenshot format options, network tweaks, background notifications, and additional QoL features.",
            categories: &["QoL", "API"],
            recommended: true,
            warning: Some("UnitXP_SP3 may trigger antivirus false-positive alerts on Windows."),
            companion_links: &[],
            expanded_notes: &[],
            is_addon: true,
        },
        Preset {
            name: "nampower",
            url: "https://gitea.com/avitasia/nampower",
            description: "Addresses a 1.12 client casting limitation where follow-up casts wait on round-trip completion feedback. The result is reduced cast downtime and better effective DPS, especially on higher-latency connections.",
            categories: &["API"],
            recommended: true,
            warning: None,
            companion_links: &[("nampowersettings", "https://gitea.com/avitasia/nampowersettings")],
            expanded_notes: &[],
            is_addon: false,
        },
        Preset {
            name: "SuperWoW",
            url: "https://github.com/balakethelock/SuperWoW",
            description: "Client mod for WoW 1.12.1 that fixes engine/client bugs and expands the Lua API used by addons. Some addons require SuperWoW directly, and many others gain improved functionality when it is present.",
            categories: &["QoL", "API"],
            recommended: true,
            warning: Some("SuperWoW may trigger antivirus false-positive alerts on Windows."),
            companion_links: &[
                ("SuperAPI", "https://github.com/balakethelock/SuperAPI"),
                ("SuperAPI_Castlib", "https://github.com/balakethelock/SuperAPI_Castlib"),
            ],
            expanded_notes: &[
                "SuperAPI improves compatibility with the default interface and adds a minimap icon for persistent mod settings.",
                "It exposes settings like autoloot, clickthrough corpses, GUID in combat log/events, adjustable FoV, enable background sound, uncapped sound channels, and targeting circle style.",
                "SuperAPI_Castlib adds default-style nameplate castbars. If you're using pfUI/shaguplates, you do not need this module.",
            ],
            is_addon: false,
        },
        Preset {
            name: "DXVK (GPLAsync fork)",
            url: "https://gitlab.com/Ph42oN/dxvk-gplasync",
            description: "DXVK can massively improve performance in old Direct3D titles (including WoW 1.12) by using Vulkan. This fork includes Async + GPL options aimed at further reducing stutters. Async/GPL behavior is controlled through dxvk.conf, so users can keep default behavior if they prefer.",
            categories: &["Performance"],
            recommended: true,
            warning: None,
            companion_links: &[],
            expanded_notes: &[],
            is_addon: false,
        },
        Preset {
            name: "perf_boost",
            url: "https://gitea.com/avitasia/perf_boost",
            description: "Performance-focused DLL for WoW 1.12.1 intended to improve FPS in crowded areas and raids. Uses advanced render-distance controls.",
            categories: &["Performance"],
            recommended: false,
            warning: None,
            companion_links: &[("PerfBoostSettings", "https://gitea.com/avitasia/PerfBoostSettings")],
            expanded_notes: &[],
            is_addon: false,
        },
        Preset {
            name: "VanillaHelpers",
            url: "https://github.com/isfir/VanillaHelpers",
            description: "Utility library for WoW 1.12 adding file read/write helpers, minimap blip customization, larger allocator capacity, higher-resolution texture/skin support, and character morph-related functionality.",
            categories: &["API", "Performance"],
            recommended: true,
            warning: None,
            companion_links: &[],
            expanded_notes: &[],
            is_addon: false,
        },
        Preset {
            name: "WeirdUtils",
            url: "https://codeberg.org/MarcelineVQ/WeirdUtils",
            description: "WeirdUtils is a DLL mods package which provides many pre-built DLLs for enhancing the vanilla 1.12 client WoW gameplay experience, aimed in particular at ease of use and accessibility but also bug fixes.\n\nYou may get all features by installing weirdutils.dll, or choose any selection of features via individual DLLs.",
            categories: &["QoL", "Performance"],
            recommended: false,
            warning: None,
            companion_links: &[],
            expanded_notes: &[],
            is_addon: false,
        },
    ]
}

/// Returns true if the given URL corresponds to a preset with an "AV false-positive" warning.
pub fn is_av_false_positive(url: &str) -> bool {
    let url = url.trim_end_matches('/');
    create_quick_add_presets().iter().any(|p| {
        p.url.trim_end_matches('/').eq_ignore_ascii_case(url) && p.warning.is_some()
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Build the Quick Add preset card list (shown when URL input is empty in mods dialog).
pub fn build_quick_add_presets<'a>(repos: &[RepoRow], colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;

    let presets = create_quick_add_presets();
    let cards: Vec<Element<Message>> = presets.iter().map(|preset| {
        let already_installed = repos.iter().any(|r| {
            r.url.trim_end_matches('/').eq_ignore_ascii_case(preset.url.trim_end_matches('/'))
        });

        let preset_url = preset.url.to_string();
        let title_btn = button(
            iced::widget::rich_text::<(), _, _, _>([
                iced::widget::span(preset.name)
                    .underline(true)
                    .font(iced::Font { weight: iced::font::Weight::Bold, ..Default::default() })
                    .color(c.link)
                    .size(22.0_f32),
            ])
        )
        .on_press(Message::SetAddRepoUrl(preset_url.clone()))
        .padding(0)
        .style(move |_t, _s| button::Style {
            background: None,
            text_color: c.link,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        });

        // Category/flag tags
        let mut tags: Vec<Element<Message>> = Vec::new();
        if preset.recommended {
            tags.push(badge_tag(
                "Recommended",
                iced::Color::from_rgb8(0x34, 0xd3, 0x99),
                iced::Color::from_rgb8(0x10, 0xb9, 0x81),
            ));
        }
        if preset.warning.is_some() {
            tags.push(tip(
                badge_tag(
                    "AV false-positive",
                    iced::Color::from_rgb8(0xfc, 0xa5, 0xa5),
                    iced::Color::from_rgb8(0xef, 0x44, 0x44),
                ),
                "This mod is known to trigger an antivirus false-positive.",
                iced::widget::tooltip::Position::Top,
                colors,
            ));
        }
        for cat in preset.categories {
            let (text_col, base_col, tooltip_text) = match *cat {
                "Performance" => (
                    iced::Color::from_rgb8(0xc4, 0xb5, 0xfd),
                    iced::Color::from_rgb8(0xa8, 0x55, 0xf7),
                    "This mod aims to increase the game's performance.",
                ),
                "QoL" => (
                    iced::Color::from_rgb8(0x93, 0xc5, 0xfd),
                    iced::Color::from_rgb8(0x3b, 0x82, 0xf6),
                    "This mod adds Quality of Life improvements to the game.",
                ),
                "API" => (
                    iced::Color::from_rgb8(0xfd, 0xe6, 0x8a),
                    iced::Color::from_rgb8(0xfa, 0xcc, 0x15),
                    "Adds new API which certain addons may rely on to improve their functionality.",
                ),
                _ => (c.muted, c.muted, ""),
            };
            let badge = badge_tag(cat, text_col, base_col);
            if !tooltip_text.is_empty() {
                tags.push(tip(badge, tooltip_text, iced::widget::tooltip::Position::Top, colors));
            } else {
                tags.push(badge);
            }
        }

        let tags_row = row(tags).spacing(4).align_y(iced::Alignment::Center);

        // Description + notes + optional warning
        let mut desc_col: Vec<Element<Message>> = vec![
            text(preset.description).size(16).color(colors.title).into(),
        ];
        for note in preset.expanded_notes {
            desc_col.push(
                row![
                    text("\u{2022}").size(15).color(c.text),
                    text(*note).size(15).color(c.text),
                ]
                .spacing(4)
                .into()
            );
        }
        if !preset.companion_links.is_empty() {
            let companions: Vec<Element<Message>> = preset.companion_links.iter().map(|(label, lurl)| {
                let l = lurl.to_string();
                button(
                    iced::widget::rich_text::<(), _, _, _>([
                        iced::widget::span(*label)
                            .underline(true)
                            .color(c.link)
                            .size(16.0_f32),
                    ])
                )
                .on_press(Message::OpenUrl(l))
                .padding(0)
                .style(move |_t, _s| button::Style {
                    background: None,
                    text_color: c.link,
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: true,
                })
                .into()
            }).collect();
            desc_col.push(
                row![
                    text("Companion addons:").size(16).color(c.muted),
                    row(companions).spacing(8),
                ].spacing(4).into()
            );
        }

        // Action button
        let action_btn: Element<Message> = if already_installed {
            container(
                text("Installed").size(12).color(iced::Color::from_rgb8(0x34, 0xd3, 0x99))
            )
            .padding([4, 10])
            .style(move |_t| container::Style {
                background: Some(iced::Background::Color(
                    iced::Color::from_rgba8(0x10, 0xb9, 0x81, 0.15)
                )),
                border: iced::Border {
                    color: iced::Color::from_rgba8(0x10, 0xb9, 0x81, 0.4),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            })
            .into()
        } else {
            let pu = preset.url.to_string();
            button(text("Add").size(12))
                .on_press(Message::QuickInstallPreset(pu))
                .padding([4, 14])
                .style(move |_t, _s| theme::tab_button_active_style(c))
                .into()
        };

        let card_content = column![
            row![title_btn, tags_row].spacing(8).align_y(iced::Alignment::Center),
            column(desc_col).spacing(3),
            row![Space::new().width(Length::Fill), action_btn]
                .align_y(iced::Alignment::Center),
        ]
        .spacing(6);

        container(card_content)
            .width(Length::Fill)
            .padding([10, 14])
            .style(move |_t| theme::card_style(c))
            .into()
    }).collect();

    column(cards).spacing(6).width(Length::Fill).into()
}
