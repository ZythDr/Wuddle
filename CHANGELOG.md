# Changelog

All notable changes to Wuddle are documented in this file.

## v2.5.5

### GIF Playback Fix
- **Correct GIF animation speed:** Animated GIFs in README previews now play at their intended frame rate. WebKitGTK doesn't clamp low frame delays like other browsers, causing some GIFs to play extremely fast. Wuddle now detects problematic GIFs and renders them on a canvas with correct timing.

### Search Debounce
- **Debounced search inputs:** Project search and log search now wait 500ms after the last keystroke before updating results, reducing unnecessary re-renders while still feeling responsive.

## v2.5.4

### Desktop Notifications
- **Fixed desktop notifications:** Notifications stopped working in v2.4.6 when the implementation was switched to the Tauri notification plugin (which doesn't work without a frontend bundler). Now uses `notify-rust` to send notifications directly via D-Bus on Linux, with proper app name and icon
- **Notification icon:** Desktop notifications now display the Wuddle app icon
- **Notifications on manual check:** Clicking "Check for updates" now sends a desktop notification for both "updates available" and "no updates available" results (previously only background checks triggered notifications)
- **Simplified notification logic:** Unified the manual/auto/startup notification paths into a single clean flow with dedup-key tracking for background checks

### Launch Environment Fix
- **Clean environment for child processes:** All launch modes (Lutris, Wine, Custom, Auto) now strip AppImage/Tauri-injected environment variables (`LD_LIBRARY_PATH`, `GDK_BACKEND`, etc.) before spawning, fixing Lutris launch failures in AppImage builds
- **Process group detachment:** Launched games now run in their own process group, preventing Wuddle's taskbar icon from appearing on the game window and ensuring the game survives if Wuddle is closed
- **Refactored env cleanup:** The AppImage env-cleaning logic (previously only used for `xdg-open`) is now shared across all child process launches

### Other
- **AI context file:** Added `CONTEXT.md` documenting project architecture, conventions, pitfalls, and priorities for AI-assisted development
- **Removed unused Tauri notification plugin:** Dropped `tauri-plugin-notification` crate and its capability permission (replaced by `notify-rust`)

## v2.5.3

### Clickable File Preview
- **Preview any file from the tree:** Click any file in the Installed Files tree (detail dialog) or the repo file tree (add dialog) to preview its contents in the main content area — works for `.lua`, `.xml`, `.toc`, `.md`, `.css`, `.js`, `.txt`, and more
- **Syntax highlighting:** File previews include language-aware syntax highlighting for Lua, XML/HTML, Markdown, CSS, JavaScript, INI/TOC, and Diff formats, using a VS Code-inspired color theme
- **Back navigation:** A clickable `← filename` header lets you return to the previous view (README or Release Notes)

### Release Notes Rename
- **"Changelog" → "Release Notes":** The changelog button in the detail dialog is now labeled "Release Notes" and shows only forge release entries — no more CHANGELOG.md fallback or README extraction
- **Mods default to Release Notes:** Opening a mod's detail dialog now shows Release Notes by default instead of README (addons still default to README)

### Repo Name Casing Fix
- **Preserved original casing:** Repo owner and name are no longer lowercased when added — display names now match the actual repository casing on GitHub/GitLab/Gitea
- **One-time DB migration:** Existing repos that were lowercased by the v2.5.2 dedup migration are automatically corrected by fetching the proper casing from each forge API on first startup

### Other
- **Remote file fetch:** New backend command to fetch any file by path from GitHub, GitLab, or Gitea repositories (used by file preview in the add dialog)
- **Local file read:** New backend command to read local text files from the WoW directory with size and binary guards (used by file preview in the detail dialog)
- **Symlink-safe file reading:** Local file reads no longer break when the WoW directory is a symlink

## v2.5.2

### Addon Deduplication
- **Case-insensitive repo matching:** Host, owner, and repo name are now normalized to lowercase, preventing duplicate entries when the same repo is added from differently-cased URLs
- **Folder-level dedup on import:** The addon auto-import scan now checks whether an addon's install folders are already tracked by another repo before importing, preventing duplicate entries from forks that deploy to the same directories
- **Cross-fork dedup on startup:** On each load, Wuddle verifies that each tracked addon repo matches the actual git remote on disk — stale entries from old forks are automatically cleaned up
- **DB migration v4:** Existing databases are automatically normalized (lowercase keys, duplicate merging, case-insensitive unique index)

### Add Dialog Improvements
- **README image and video support:** Images and videos in repo README previews now display correctly — relative URLs are resolved against `raw.githubusercontent.com` for GitHub repos
- **URL input cleared on open:** The URL field and all preview panels are now reset every time the Add dialog is opened
- **Responsive side panel:** The About/Files side panel now shrinks before the main dialog content when the window is narrow, with a minimum width of 180px
- **Addon-friendly text:** The addon Add dialog shows a clearer subtitle and a contextual placeholder URL (BigWigs for addons, nampower for mods)

### Fixes
- **Changelog h3 headers:** `###` markdown headers in the in-app changelog viewer now render correctly instead of showing as raw text

## v2.5.1

- **Add dialog: hide Quick Add for addons:** The Add dialog no longer shows the mod Quick Add presets when adding addons — only the URL input and repo preview panels are shown
- **Quick Add always expanded:** Preset cards now display their full descriptions and companion links by default, removing the click-to-expand interaction
- **Quick Add label simplified:** Header text changed from "Quick add (click to expand)" to "Quick Add"
- **Scroll fade fix on tree collapse:** Collapsing or expanding folders in the file tree now recalculates scroll fading, preventing stale fade overlays on short lists
- **Home tab on startup:** Wuddle now always opens on the Home tab instead of restoring the last active tab

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
