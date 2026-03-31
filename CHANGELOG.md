# Changelog

All notable changes to Wuddle are documented in this file.

## v3.0.0

Complete frontend rewrite from Tauri/WebView to [Iced 0.14](https://iced.rs), rendering natively via wgpu (Vulkan/Metal/DX12). No WebView, no browser engine overhead. App data (profiles, tracked mods, settings) is fully forward/backward compatible with v2.x.

### New Features

- **Native GPU-rendered UI** — pure Rust frontend using Iced 0.14, replacing the Tauri/WebView stack entirely.
- **In-game radio player** — stream the Everlook Broadcasting Co. radio directly inside Wuddle with play/stop, volume controls (click-to-mute, scroll-to-adjust), reconnect, auto-connect, auto-play, persistent volume, and configurable read-ahead buffer via a dedicated Radio Settings dialog.
- **DXVK Configurator** — interactive dialog to generate and edit `dxvk.conf` with per-setting tooltips, tristate/pick-list controls, syntax-highlighted file preview, and Turtle WoW-specific presets including `dxvk.enableAsync` toggle.
- **Version pinning** — per-mod inline dropdown to lock to a specific release tag. The latest version is still tracked so "Update available" continues to show.
- **Merge updates mode** — per-repo toggle that keeps existing installed files and only overwrites matching ones during updates. Designed for repos that ship partial releases (e.g. WeirdUtils bug-fix releases).
- **DLL count mismatch warning** — when the number of DLLs changes between releases, a dialog prompts for Merge Update vs Clean Update.
- **Multi-DLL expand/collapse** — mods installing multiple DLLs appear as expandable parent rows with per-DLL enable/disable toggles and `dlls.txt` block markers.
- **Remove dialog with file preview** — scrollable file tree showing every installed file before confirming removal, with an optional "delete local files" checkbox.
- **GitHub-flavored admonition rendering** — README previews render `[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, and `[!CAUTION]` blocks with colored accents and icons.
- **GitHub API rate limit tooltip** — hover the "API status" text in Mods/Addons tabs to see remaining requests, total limit, and reset time.
- **Auto-scaling for smaller monitors** — detects monitor resolution at startup and scales the UI automatically. Manual scale buttons (75%–120%) available in Options.
- **Comprehensive tooltips** — nearly every button across all panels now shows a descriptive tooltip on hover.
- **Clickable update notifications** — toast notifications for new Wuddle releases navigate to the About tab when clicked.

### Improvements

- **Redesigned adaptive update checking** — repos with no recent releases (older than 3 days) are checked every 4 hours instead of every cycle, with an hourglass badge indicator. Previous update plans are cached and merged for skipped repos.
- **Improved "Modified" status detection** — uses SHA256 hash comparison against stored install hashes for more reliable external modification detection.
- **Per-profile update plan cache** — switching profiles restores the previous update state instead of clearing it.
- **Rate limit conservation** — post-update re-checks are skipped when no GitHub token is configured, preserving the 60 req/hr unauthenticated limit.
- **Verbose logging** — nearly every user action emits a log entry viewable in the Logs tab.

### Engine Changes (wuddle-engine)

- **GAM-compatible addon deployment** — git repos clone directly into `Interface/AddOns/{name}/` with `.git` inside the addon folder, cross-compatible with GitAddonsManager and the TurtleWoW launcher.
- **Multi-addon repo symlinks** — repos with multiple `.toc` subfolders get symlinked into AddOns, matching GAM's behavior.
- **Automatic staging-area migration** — old `.wuddle/addon_git/` clones are moved to the new location on first update.
- **Mod cache in WoW directory** — release downloads cached in `{wow_dir}/.wuddle/cache/` instead of system app-data, simplifying antivirus whitelisting on Windows.
- **DB schema v7** — adds `merge_installs` and `pinned_version` columns.

### Bug Fixes

- Fixed context menu toggle/dismiss race condition
- Fixed profile switching showing stale data from the previous profile
- Fixed auto install mode failing for single-file DLL releases
- Fixed window size not applying on startup
- Fixed CMD window appearing on Windows release builds
- Fixed addon branch dropdown centering
- Fixed Add dialog flashing stale preview content
- Fixed status column not refreshing after individual mod updates
- Fixed duplicate notifications on silent post-update re-checks

<details>
<summary><strong>v2.x Changelog</strong></summary>

## v2.5.10

### Bug Fixes

- **White screen on Linux AppImage** — the AppImage was built on Ubuntu 22.04 whose bundled WebKit libraries were incompatible with newer system WebKit versions (e.g. webkit2gtk 2.50+ on Arch/CachyOS). Moved CI build to Ubuntu 24.04 for better WebKit compatibility with modern distros.
- **Resilient boot sequence** — the async settings loader now has a 5-second IPC timeout and the boot is wrapped in error handling, so the UI always renders even if `settings.json` can't be read.

## v2.5.8

### New Features

- **Bidirectional settings sync with Iced v3** — `settings.json` is now the primary source of truth for both Tauri and Iced. On startup, Tauri reads profiles and options from `settings.json` (falling back to localStorage for first-time migration). All option saves write back to `settings.json` so changes made in either frontend are immediately visible to the other.
- **Profile database fallback** — when a profile-specific database has no repos, Tauri now falls back to `wuddle.sqlite` (the default Iced profile DB), ensuring mods installed via either frontend remain visible after switching.

## v2.5.7

### New Features

- **Release channel selector** on the About tab — choose between **Stable** (latest non-pre-release) and **Beta** (includes pre-releases) to control which version the update check reports.
- **Seamless upgrade path to Wuddle v3 (Iced)** — switching to the Beta channel and clicking Update will download and stage the Iced v3 build, then restart via the launcher into the new version.

## v2.5.6

### Add Dialog Enhancements
- **Forge icon and Release Notes in Add dialog:** When previewing a repo in the Add dialog (via Quick Add or URL), forge icon and Release Notes buttons now appear in the footer — matching the detail dialog experience.
- **"No README" placeholder:** Repos without a README.md now show a clear placeholder message instead of silently hiding the preview area.

### Markdown Code Block Support
- **Fenced code blocks:** README previews from Gitea/GitLab repos now render fenced code blocks with proper styling.
- **Inline code:** Inline code now renders with monospace background styling in markdown READMEs.

### Link Fixes
- **README links open correctly:** Links in README previews now open in the system browser as intended.

### Input UX
- **Clearable input fields:** All text inputs now have a clear button (✕).
- **DMA-BUF rendering toggle:** Added an experimental settings toggle for DMA-BUF rendering on Linux.

## v2.5.5

- **Correct GIF animation speed:** Animated GIFs in README previews now play at their intended frame rate.
- **Debounced search inputs:** Project search and log search now wait 500ms after the last keystroke before updating results.

## v2.5.4

- **Fixed desktop notifications:** Now uses `notify-rust` to send notifications directly via D-Bus on Linux.
- **Clean environment for child processes:** All launch modes now strip AppImage/Tauri-injected environment variables before spawning.
- **Process group detachment:** Launched games now run in their own process group.

## v2.5.3

- **Clickable file preview:** Click any file in the Installed Files or repo file tree to preview its contents with syntax highlighting.
- **"Changelog" → "Release Notes":** Renamed and simplified to show only forge release entries.
- **Repo name casing fix:** Display names now match the actual repository casing.

## v2.5.2

- **Addon deduplication:** Prevents duplicate addon entries with case-insensitive matching and cross-fork cleanup.
- **README media support:** Images and videos in repo README previews now display correctly.
- **Responsive side panel:** The About/Files panel shrinks gracefully on narrow windows.

## v2.5.0

- **Add dialog repo preview:** Pasting a repo URL shows README, file tree, and About panel.
- **Quick Add + README shared frame:** Presets and README share a single content region.
- **Scroll-aware edge fading:** Scrollable frames fade at the top/bottom edges to indicate overflow.
- **Sticky dialog footers:** Consistent head/body/foot layouts with non-scrolling footers.
- **Performance improvements:** Shared HTTP client, targeted branch-dropdown updates, LRU-capped caches.

## v2.4.6

- Auto-clear WDB cache per-instance toggle
- Collapsible advanced launch options
- Assets-pending detection for self-update
- Ignore updates per-repo via right-click menu

## v2.4.5

- Desktop notifications for mod/addon updates
- Turtle WoW links (Armory, Turtlogs)
- Quick Add improvements

## v2.4.4

- In-app changelog viewer on the About page
- Fix external links in AppImage

## v2.4.2

- Linux AppImage self-update support

## v2.4.1

- Adaptive update frequency (Active/Stable/Dormant)
- Self-update restart fix (Windows)
- Cross-platform latest version display

## v2.4.0

- **Tweaks tab:** Patch WoW.exe with quality-of-life improvements (FoV, Farclip, Quickloot, Camera fixes, etc.)
- Read Current, Reset to Default, Automatic backup, Per-profile tweak settings

## v2.3.0

- Mod file integrity checking via SHA-256
- Automatic cache cleanup
- Addon conflict detection dialog
- Auto-check for updates with configurable interval
- Turtle WoW home section
- Visual theme picker

## v2.1.0

- Visual themes (including WoW UI inspired theme)
- Search UX improvements

## v2.0.0

- Evolved from DLL updater into launcher + manager
- Addon management with Git clone/pull and branch selection
- Home tab with update overview and PLAY button
- Per-instance launch methods (Auto/Lutris/Wine/Custom)
- Multi-instance profile switching

## v1.0.8

- Initial stable release

</details>
