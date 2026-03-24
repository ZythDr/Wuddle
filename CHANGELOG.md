# Changelog

All notable changes to Wuddle are documented in this file.

## v2.5.7

### New Features

- **Release channel selector** on the About tab — choose between **Stable** (latest non-pre-release) and **Beta** (includes pre-releases such as v3.0.0-beta.1) to control which version the update check reports. Defaults to Stable.
- **Seamless upgrade path to Wuddle v3 (Iced beta)** — switching to the Beta channel and clicking Update will download and stage the Iced v3 portable build, then restart via the launcher into the new version. All settings and profiles carry over automatically.
- **Settings written to `settings.json` on every startup** — preferences (theme, clock format, auto-check interval, etc.) are now synced to the shared data directory on launch, so Wuddle v3 (Iced) inherits them without requiring a manual save.

### Changes

- Update launcher now accepts both `Wuddle-bin.exe` and `wuddle.exe` inside version folders, making it forward-compatible with Iced release packages.

## v3.0.0-beta.1 (Iced frontend)

First public beta of the Iced v3 frontend — a native GPU-rendered rewrite of Wuddle using [Iced 0.14](https://iced.rs). Replaces the Tauri/WebView stack with a pure Rust UI while sharing the same `wuddle-engine` backend. App data (profiles, tracked mods, settings) lives in the same location as v2 and is fully forward/backward compatible.

### New Features

- **Native GPU-rendered UI** — no WebView, no Electron. Iced 0.14 renders directly via wgpu (Vulkan/Metal/DX12), resulting in a lighter process with no browser engine overhead.
- **DXVK config generator** — interactive dialog to generate a `dxvk.conf` tailored for Turtle WoW. Includes syntax-highlighted file preview with selectable text, per-setting tooltips explaining each option, and a `dxvk.enableAsync` toggle for the gplasync fork (with a side-effect warning for 2D portrait users).
- **Remove dialog with file preview** — before confirming removal, a scrollable file tree shows every installed file with type icons. An optional "also delete local files" checkbox controls whether files are removed from disk alongside the database entry.
- **Multi-DLL mod support** — mods that install multiple DLLs (e.g. WeirdUtils) appear as expandable parent rows with per-DLL enable/disable toggles. `dlls.txt` block markers (`# == RepoName ==`) are written on install.
- **Colored status badge pills** in the projects table: Up to date · Update available · Error · Disabled · Ignored.
- **"Ignore Updates"** per-repo toggle accessible from the ⋮ context menu; ignored repos are excluded from update counts and filterable via a dedicated tab.
- **In-app changelog dialog** — fetches content from GitHub on click with an embedded fallback for offline use.
- **Self-update check** on launch, on About tab navigation, and on an hourly subscription when no token is set.
- **Release channel selector** on the About tab — choose Stable (latest non-pre-release) or Beta (latest including pre-releases) to control which version the update check reports.

### Changes

- **Overlay-anchored context menus** — exact position, scroll-immune. No positional drift when scrolling the project list.
- **Per-profile update plan cache** — switching profiles restores the previous update state instead of clearing it.
- **Branch-fetch errors** condensed to human-readable messages including the repo name and numeric error code.
- **Disabled repos fully skipped** during update checks — no git pull, no API call, no log entry generated.
- **Codeberg repos labeled correctly** — repos previously mis-labeled as "gitea" are corrected on next load without re-adding.

### Bug Fixes

- Fixed ⋮ menu toggle/dismiss race condition where the menu could reopen immediately after being closed.
- Fixed profile switching showing stale mod/addon data loaded from the previously active profile's database.
- Fixed auto install mode failing for single-file DLL releases that ship no accompanying zip asset.

### Engine (wuddle-engine)

- **`prune_missing_repos(wow_dir)`** — removes database tracking entries for repos whose installed files no longer exist on disk. Database-only operation; never deletes user files. Ensures profile isolation when switching between instances with different WoW directories.

## v3.0.0-alpha.4 (Iced frontend)

### Multi-DLL Mod Support

- **Expandable rows:** Mods that install multiple DLL files (e.g. WeirdUtils) now appear as a single collapsible parent row. Click anywhere in the Name column to expand/collapse child DLL rows. A `›`/`⌄` SVG chevron and a "N DLLs" badge indicate the expandable state.
- **Per-DLL enable toggles:** Each child DLL row has its own enable checkbox to comment/uncomment individual entries in `dlls.txt`. Toggling the parent row's Enable checkbox now also toggles all child DLLs in sync.
- **dlls.txt block markers:** Multi-DLL repos now write `# == RepoName ==` / `# == /RepoName ==` block markers around their entries in `dlls.txt`, grouping them visually and making them easy to identify.
- **Auto mode detection:** The Auto install mode now correctly identifies multi-DLL releases (no zip asset present) and downloads all `.dll` assets, not just the first one.

### Remove Dialog — File Preview

- **"Also delete local files" checkbox:** The Remove dialog now includes an optional checkbox to delete the mod's installed files from disk alongside removing it from the database.
- **File tree preview:** When the delete checkbox is enabled, a scrollable file tree lists every installed file with type icons (⚙ dll · 📁 addon folder · 📄 raw file), so nothing is deleted by surprise.

### Forge Label Fix

- **Codeberg correctly labelled:** Repos hosted on `codeberg.org` were previously displayed with the forge label `gitea`. They now show `codeberg`. Existing repos are corrected on next load without needing to be re-added.

### Update Check Improvements

- **Disabled repos skipped:** Disabled mods and addons are now completely skipped during update checks — no git pull, no API call, no error log entry.
- **Cleaner error messages:** Git/network errors in the Logs tab are now condensed into human-readable messages (e.g. "Repository not found or requires authentication (Error Code -16)") instead of raw libgit2 error chains.
- **Addon name in error lines:** Branch-fetch errors now include the affected addon name (e.g. `Failed to fetch branches for mrrosh/sqminimapfix: …`) so errors are immediately identifiable without cross-referencing the repo list.

### Logs Tab

- **Color-coded lines:** `[ERROR]` lines are highlighted in red; `[INFO]` lines use the default text color. Uses the `text_editor` widget's `highlight_with` API so text remains fully selectable and copyable.

## v3.0.0-alpha.3 (Iced frontend)

### Feature Parity — All Buttons Wired

- **Open URL:** All external links (home page, forum, Discord, armory, GitHub repos, credits) open in the system browser via `open::that`. Link buttons on the Home tab now show a tooltip with the full URL on hover.
- **Game launch:** The PLAY button is fully wired. Resolves VanillaFixes.exe → Wow.exe fallback, respects the active profile's launch method (Auto / Lutris / Wine / Custom), and spawns the game process detached from Wuddle.
- **GitHub token:** Save and Forget token buttons are wired. Token is stored in the system keyring (with a file fallback for portable mode) and loaded into `wuddle_engine` on startup.
- **WoW directory picker:** Browse button in Instance Settings opens a native folder picker (`rfd`). Picked path is applied to the open dialog immediately.
- **Copy to clipboard:** "Copy Log" button on the Logs tab copies the full log text to the system clipboard via `arboard`.

### Changelog Dialog

- **In-app changelog:** "Changelog" button on the About tab opens a scrollable in-app dialog instead of opening GitHub in a browser. Content is fetched from the GitHub raw URL on click; falls back to the embedded `CHANGELOG.md` if the fetch fails or no network is available.

### Self-Update Check

- **On launch:** Wuddle checks for a new release on startup (after repos load).
- **On About tab:** Navigating to the About tab triggers a fresh update check.
- **Hourly subscription:** When no GitHub token is set, an hourly background check fires automatically so unauthenticated users see current version info without hitting rate limits.
- **"Refresh" button:** Renamed from "Refresh details" — triggers an immediate version check.
- **Dynamic version display:** Latest version and update status on the About tab now reflect live check results instead of hardcoded values.

### Projects — Status Badges & Filtering

- **Colored status pills:** Each mod/addon row now shows a color-coded badge (Up to date · Update available · Error · Disabled · Ignored) instead of plain text, using a semi-transparent background and matching border.
- **Ignore Updates:** The ⋮ context menu now includes "Ignore Updates" / "Unignore Updates". Ignored repos are excluded from the Updates filter and update counts, and shown under a dedicated Ignored filter.
- **API health indicator:** The filter-row status text is now color-coded — green when authenticated, amber when anonymous, red when rate-limited or erroring.

### Tweaks Panel

- **Fully wired:** Read Current, Reset to Defaults, Restore Backup, and Apply Tweaks buttons are all connected to the engine's tweak functions.
- **Sliders:** FoV, Farclip, Frill Distance, Nameplate Distance, Max Camera Distance, and Sound Channels each have a live slider or number input. Changing a slider marks that tweak as selected.
- **Disable when no WoW dir:** All tweak controls are disabled when no WoW directory is configured.

### Instance Settings Dialog

- **Remove button always visible:** When editing an existing instance, the Remove button is always shown at the bottom-left of the dialog. When the selected instance is the active profile it is dimmed (no `on_press`) with a tooltip: "Cannot remove the active instance". Previously it was hidden entirely for the active profile.
- **Profile cards simplified:** Instance cards in the Options tab are now clean clickable cards (no embedded Remove button). Remove is only accessible through the edit dialog.

### About Tab Layout

- **Side-by-side cards:** Application and Credits cards are displayed side-by-side instead of stacked.
- **Tooltipped header buttons:** All header buttons (Refresh, Changelog, Open on GitHub, update status) are wrapped in descriptive tooltips.

### Toolbar & Icon Fixes

- **Settings icon:** Replaced the Unicode ⚙ glyph with a proper Feather-style stroke SVG gear icon matching the visual weight of other icons.
- **About icon:** Restored the ⓘ Unicode character (U+24D8). Icon height is now constrained with `line_height(1.0)` so the About button matches the height of the SVG icon buttons exactly.
- **Spinner centering:** The loading spinner is now vertically centered with the "Wuddle" title text in the topbar.

### Porting Checklist

- **PORTING_CHECKLIST.md:** Added a feature-by-feature cross-reference table comparing the Tauri and Iced implementations (✅ Done / 🔶 Partial / ❌ Missing).

## v3.0.0-alpha.2 (Iced frontend)

### Overlay Context Menus
- **Anchored overlay system:** The triple-dot (⋮) context menu in the Mods/Addons list now uses Iced's built-in `Widget::overlay()` system (the same mechanism used by `pick_list` dropdowns), giving it exact pixel-accurate positioning anchored to the button regardless of scroll position. Previous approaches using row-index estimation and cursor tracking both had drift errors.
- **2px gap:** A small visual gap separates the menu popup from the button that opened it.
- **Toggle to close:** Clicking the ⋮ button a second time now closes the menu (first click = open, second click = close). Fixed a race condition where the overlay's dismiss message and the button's toggle message fired simultaneously causing the menu to reopen.

### Tab Button Improvements
- **Fixed-width tabs:** Home, Mods, Addons, and Tweaks tabs are now a uniform fixed width (114px) instead of shrinking to content, giving a consistent topbar layout.
- **Centered tab labels:** Tab button text is now horizontally centered within fixed-width buttons (previously left-aligned).

### Toolbar Layout
- **Single-row toolbar:** Filter buttons (All/Updates/Errors/Ignored) and the Search/Rescan/Add controls now sit on one row — filters on the left, actions on the right — instead of two stacked rows.
- **Vertical alignment fix:** All toolbar controls use consistent padding so they align to the same vertical center. Equal 8px spacing above and below the toolbar row.

### Branch Column Spacing
- **Right padding on branch dropdown:** The branch `pick_list` in the Addons table now has equal padding on both sides of the dropdown, matching the spacing between the Name and Branch columns.

### Profile Switching — Update State Preserved
- **Cached plans per profile:** Detected updates are now remembered per profile. Switching profiles restores the previously checked update state for that profile without requiring a new network check. The cache is updated after each successful update check or update-all operation.

### Engine Improvements
- **Prune logging:** Added diagnostic log when a tracked addon is pruned due to a missing git worktree.
- **Install path resolution:** Improved install path existence checks.

### Project
- **ICED_DOCUMENTATION.md:** Added a reference document covering Iced 0.14 API specifics, layout patterns, the overlay system, and a table of what worked vs. what didn't during the Iced port.
- **Windows support:** Clarified in project guidelines that Windows support is equally important alongside Linux.

## v2.5.6

### Add Dialog Enhancements
- **Forge icon and Release Notes in Add dialog:** When previewing a repo in the Add dialog (via Quick Add or URL), forge icon and Release Notes buttons now appear in the footer — matching the detail dialog experience.
- **"No README" placeholder:** Repos without a README.md now show a clear placeholder message instead of silently hiding the preview area.

### Markdown Code Block Support
- **Fenced code blocks:** README previews from Gitea/GitLab repos now render `` ``` `` fenced code blocks with proper `<pre><code>` styling instead of showing raw backtick fences.
- **Inline code:** Single and double backtick inline code (`` `code` ``) now renders with monospace background styling in markdown READMEs.

### Link Fixes
- **README links open correctly:** Links in README previews now open in the system browser as intended. Previously, some markdown-generated links had `href="#"` which broke click handling and showed `http://127.0.0.1:1430/#` on right-click.
- **Right-click "Copy URL" works:** All README links now have the resolved URL in their `href` attribute so the browser context menu shows the correct destination.

### Input UX
- **Clearable input fields:** All text inputs (repo URL, search, instance settings) now have a generic clear button (✕) matching the existing project search style.
- **DMA-BUF rendering toggle:** Added an experimental settings toggle for DMA-BUF rendering on Linux (disabled by default) with crash detection auto-fallback.
- **Linux-only options:** The xattr and DMA-BUF settings are now only shown on Linux.

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
