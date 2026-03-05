# Changelog

All notable changes to Wuddle are documented in this file.

## v2.4.3

- **Fix external links in AppImage:** Switched from env-var blacklist to whitelist approach for `xdg-open`, fixing links silently failing to open in browser

## v2.4.2

- **Linux AppImage self-update:** In-app update support for Linux AppImage builds — download, replace, and restart automatically from the About page
- **Clean AppImage naming:** Release AppImage is now named `Wuddle.AppImage` instead of versioned names, preventing confusion after in-place updates

## v2.4.1

- **Adaptive update frequency:** Mods are classified by release age (Active/Stable/Dormant) and less frequently updated mods are checked less often to conserve GitHub API requests — only active when no GitHub token is configured
- **Self-update restart fix (Windows):** Fixed self-update failing when the running executable is locked, using atomic rename instead of delete
- **Cross-platform latest version display:** About page now shows the latest Wuddle release version on all platforms, not just Windows
- **Template error messages:** GitHub API errors no longer expose raw response bodies; friendly messages guide users to add/re-save their token
- **Self-update poll interval:** Reduced from every 30 minutes to every 60 minutes
- **Windows portable token persistence:** GitHub token is saved to a local file in portable mode so it persists across updates
- **Tweaks reliability fix:** Tweaks are now applied from a clean backup copy to avoid compounding patches
- **About page layout:** Fixed grid alignment and added GitAddonsManager credit
- **RaidRes community link:** Added RaidRes button to Turtle WoW community links

## v2.4.0

- **Tweaks tab:** Patch WoW.exe with quality-of-life improvements directly from Wuddle, powered by [vanilla-tweaks by brndd](https://github.com/brndd/vanilla-tweaks)
  - Widescreen FoV, Farclip, Frilldistance, Nameplate Distance, Camera Skip Fix, Max Camera Distance, Sound in Background, Sound Channels, Quickloot, Large Address Aware
- **Read Current:** read actual tweak values from an existing WoW.exe
- **Reset to Default:** one-click restore to recommended tweak settings
- **Automatic WoW.exe backup** before the first patch, with one-click restore
- **Per-profile tweak settings:** each instance remembers its own configuration

## v2.3.3

- Renamed product from "wuddle-gui" to "Wuddle" in About page, desktop entries, and package bundles
- Fixed About page displaying "wuddle-gui" instead of "Wuddle"
- Updated app description to "All-in-one manager and launcher for World of Warcraft"

## v2.3.2

- Relocated busy spinner next to title, increased size
- Enlarged title text, reduced topbar padding
- Turtle WoW links: adaptive column layout based on section height
- Options grid: always side-by-side layout
- Profile picker: hidden when only one instance exists
- AV false-positive warning tags on VanillaFixes and UnitXP_SP3 presets
- Search clears when switching between Mods/Addons views
- Added "Ignore Error" menu item for errored repos with "Ignored" badge
- Removed subtitle text from header

## v2.3.1

- Fixed Quick Add presets showing "Add instance first" despite having an active instance
- Fixed auto-check interval defaulting to 1 minute instead of 60 on fresh installs
- Enabled auto-check for updates by default
- Fixed links not opening on Linux AppImage (AppImage env var cleanup before xdg-open)
- Fixed busy spinner using hardcoded blue instead of theme primary color

## v2.3.0

- Mod file integrity checking: detects externally modified mods via SHA-256, shows warning badge, skips in bulk updates
- Automatic cache cleanup: configurable versions to keep (0-10, default 3)
- Addon conflict detection dialog when repos share addon folders
- Auto-check for updates with configurable interval
- Turtle WoW home section with curated community links
- Visual theme picker with color swatches
- Codebase modularization: main.js split into focused ES modules
- Zip path traversal security fix

## v2.1.0

- Visual themes (including WoW UI inspired theme)
- Search UX improvements with clear button
- Add/install flow polish
- GitHub auth health monitoring

## v2.0.0

- Evolved from DLL updater into launcher + manager
- Addon management with Git clone/pull and branch selection
- Home tab with update overview and PLAY button
- Per-instance launch methods (Auto/Lutris/Wine/Custom)
- VanillaFixes support in Quick Add
- Multi-instance profile switching
- Conflict handling on addon install/update

## v1.1.0

- External link handling fixes
- VanillaFixes support
- Enhanced About version info

## v1.0.8

- Initial stable release
