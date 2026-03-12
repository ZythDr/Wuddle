# Changelog

All notable changes to Wuddle are documented in this file.

## v2.5.0

### Add Dialog Overhaul
- **Repo README preview:** Pasting a repo URL in the Add dialog now fetches and displays the repository's README directly in-app
- **Repo info panel:** Shows repository description, star count, clickable fork count, language, and license alongside the README
- **File tree panel:** Browse the repository's top-level file/folder structure (expandable one level deep) before adding
- **Quick Add + README shared frame:** Quick Add presets and README preview share a single bordered content frame with a swappable header label
- **Advanced mode toggle:** Footer checkbox to show/hide the install mode dropdown, keeping the default flow cleaner

### Scroll Fade Design Language
- **Scroll-aware edge fading:** Scrollable frames now show a subtle gradient fade at the top/bottom edges to indicate more content — appears only when content overflows in that direction
- **Theme-aware fade colors:** Fade overlays automatically match the effective background color of their container, with live re-sync on theme change
- **Applied globally:** Add dialog content frame, dialog bodies, file tree panel, and all dialog scroll regions use the new fade system

### Sticky Dialog Footers
- **All dialogs restructured:** Instance Settings, Changelog, Addon Conflict, and SuperWoW Warning dialogs now use a consistent head/body/foot flex layout with non-scrolling sticky footers

### Performance
- **Shared HTTP client:** Backend Tauri commands reuse a single connection-pooled HTTP client instead of creating one per request
- **Branch dropdown targeted updates:** Loading branch lists for addon repos now updates only the affected dropdown instead of rebuilding the entire repo list — eliminates UI freezes on the Addons tab with many repos
- **Consolidated MutationObserver:** Single observer handles both DOM additions and dialog open-attribute changes
- **Cached fade colors:** WeakMap-based cache with generation counter avoids redundant `getComputedStyle` walks on every scroll event
- **LRU cache limits:** README, repo info, and file tree caches are capped at 30 entries to bound memory usage

## v2.4.6

- **Auto-clear WDB cache:** Per-instance toggle to delete the WDB folder before each launch — fixes stale server-cache bugs common on Turtle WoW
- **Collapsible advanced launch options:** Working directory and environment variable fields are now tucked inside a collapsible "Advanced" section
- **Improved desktop notifications:** Switched from browser Notification API to Tauri notification plugin for reliable cross-platform support
- **Assets-pending detection:** Self-update now shows a "building…" state when a new release exists but CI hasn't finished uploading assets yet
- **Hotfix release detection:** Suffixed version tags (e.g. `v2.4.6-fix`) are now correctly detected as updates
- **Ignore updates:** Right-click any mod or addon to ignore its updates — ignored repos are skipped by "Update All", excluded from update counts and notifications, and shown with an "Ignored" badge

## v2.4.5

- **Desktop notifications:** Optional OS-level notifications when mod/addon updates are found — enable via the new toggle in Settings
- **Turtle WoW links:** Added Armory (official) and Turtlogs (community) buttons to the Home page
- **Quick Add improvements:** Mod descriptions now always show in full instead of requiring click-to-expand; cleaned up wording throughout
- **Simplified cache setting:** Removed the "cached versions to keep" option — Wuddle now always keeps one previous version
- **GPLv3 label:** Added license label to GitAddonsManager credit in About page

## v2.4.4

- **In-app changelog viewer:** View the latest changelog from within Wuddle via the About page — fetched live from GitHub so older versions can see what's new
- **Fix external links in AppImage:** Comprehensive env-var cleanup for `xdg-open` so links open reliably across all desktop environments (KDE, GNOME, XFCE, etc.)
- **Removed .deb and .rpm builds:** Linux releases now ship as AppImage and portable tar.gz only

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
