# Changelog

All notable changes to Wuddle are documented in this file.

## v3.2.0

### New Features
- **Collection Addon Management** — Treat addon-git repositories as real collections, choose which addon folders to keep directly in the Add Repo preview, and manage installed collections later without re-adding the repo.
- **Nested Addon Discovery** — Wuddle now detects addon folders with `.toc` files up to 5 levels deep in addon-git repositories.

### Improvements
- **Executable-Aware Tweaks** — Profiles can target a specific game executable for Auto launch and Tweaks instead of assuming a single default client file.
- **Targeted Tweaks Feedback** — Tweaks now reports which executable is being inspected and clearly explains when the selected client is not compatible with legacy 1.12.1 patching.

### Bug Fixes
- **Collection Matching Fixes** — Fixed collection management for repositories whose folder names differ from the installed addon name, including common GitHub suffixes like `-master` and `-main`.
- **Nested Install Linking** — Fixed nested addon installs and repair flows so the correct repo-relative folder is linked or moved.

### Removed
- **Legacy Radio UI** — Removed the in-app radio player and its related settings UI.
- **Turtle-Specific Home Links** — Removed the Turtle-only links section from the Home tab.
- **`I like turtles` Profile Flag** — Removed the old profile toggle that controlled Turtle-themed home content.

## v3.1.0

### New Features
- **Browse Option** — Added a "Browse..." option to the triple-dot menu for tracked addons and mods, allowing users to quickly open the relevant folder or file on their system.

### Improvements
- **GAM Path Fidelity** — Achieved 1:1 functional parity with GitAddonsManager (GAM). Wuddle now mimics GAM's cloning, directory naming, and subfolder handling logic exactly, ensuring seamless interoperability on Linux.
- **Auto-Correcting Casing** — Implemented a self-healing mechanism that synchronizes database repository names with their actual filesystem casing on Linux, resolving legacy lowercase discrepancies.
- **Hybrid Addon Discovery** — Enhanced the addon scanner to support repositories containing both a root-level addon and additional subfolder-level addons, matching GAM's detection behavior.
- **Targeted Addon Link Verification** — Standard refresh and launch flows now perform a cheap verification pass for tracked addon links and only escalate to repo-local repair when a broken entry is actually found.
- **Case-Insensitive Path Tracking** — Re-implemented addon path discovery and pruning to be case-insensitive. This prevents "ghost" entries and redundant re-imports on Linux when addon folder casing changes.
- **Improved Invalid URL Logging** — Update check failures now include the specific Addon ID in the logs (e.g., `Fetch versions failed for id=404: invalid URL`), making it easier to identify problematic repositories.
- **Rescan Phase Logging** — Rescan now logs repair, casing cleanup, prune, import, and dedup phases with timings and counts in the Logs tab.
- **Strict Manual Addon Validation** — The manual scan now strictly requires a `.toc` file to be present in a directory before considering it a valid addon, preventing `.git`, `.repo`, and other non-addon folders from being imported.

### Bug Fixes
- **Async Repair Flow** — Made the broken path repair and casing correction mechanisms asynchronous, preventing UI freezes during intensive rescan operations.
- **Case-Sensitive Collision Fix** — Fixed a bug where Wuddle would create duplicate lowercase `.repo` folders if a repository was managed by both Wuddle and GAM.
- **Endless Startup Spinner** — Fixed a regression where launch auto-checks could appear stuck forever because addon maintenance work was incorrectly running inside the normal update-check path.
- **Ghost Addon Entries** — Resolved an issue where renaming an addon folder on disk would cause Wuddle to lose track of the path and display a generic "addon" placeholder in the removal dialog.
- **Path Resolution Fallback** — Implemented a robust fallback mechanism for resolving addon paths that ensures the "Browse..." and "Remove" features work even if the database entry becomes slightly out of sync with the disk.
- **Timed Remote Checks** — Added explicit timeouts to git remote-head and release lookups so unresponsive hosts no longer block update checks indefinitely.

## v3.0.7

### API Transparency & Log Filtering
- **Dedicated API Log Category** — Introduced a new `[API]` log level with a dedicated filter button in the Log Panel.
- **Cyan Highlighting** — API-related events are now distinctly colored in Cyan for better scannability.
- **Detailed Quota Tracking** — Update summaries now show precisely how many GitHub API points were spent vs. cached, alongside your remaining hourly budget and reset timer.
- **Transparency** — Self-update and version checks are now explicitly logged under the `[API]` category to clarify background budget consumption.

### Update Reliability & UI Polish
- **Immediate UI Refresh** — Successfully updated repositories are now instantly cleared from the Home tab and update indicators without requiring a re-scan.
- **Restored Verbose Logging** — Returned to detailed per-repository logging (e.g., `Updating Owner/Repo...`) for both single and bulk updates.
- **User Experience** — Hidden "Infrequently Updated" (4h interval) warnings for authenticated users with a GitHub token.
- **Cleaner Errors** — Integrated `simplify_git_error` into all update flows for more human-readable logs when something goes wrong.

## v3.0.6

### Update Reliability & Quota Management
- **Token-Aware Update Checking** — Authenticated users with a GitHub token now always perform a full repository check, bypassing all throttles and skips. 
- **Selective Manual Checks** — For anonymous users, the "Check for updates" button now strictly skips infrequently updated mods (> 3 days stable) to preserve the 60 req/hr API quota.
- **Improved Adaptive Skipping** — All background and manual checks for unauthenticated users now focus on addons and recently updated mods first.
- **Check Persistence Fix** — Resolved a bug where the 4-hour "infrequent check" window could fail to reset, leading to either redundant checks or permanently stale results.

### UI/UX
- **Visual De-cluttering** — Hidden the "Infrequently Updated" hourglass icon and tooltip for authenticated users, reflecting that the 4-hour cooldown no longer applies to them.

## v3.0.5

### Security & Safety
- **Anti-Virus False-Positive Warnings** — Restored and generalized the warning dialog for mods known to frequently trigger security heuristics (SuperWoW, VanillaFixes, UnitXP_SP3). 
- **Informed Installation Flow** — Safety warnings are now integrated into both the "Quick Add" catalog and the manual "Add Repo" workflows. Installation is blocked until the user explicitly acknowledges the potential for false-positive detections.

### UI/UX
- **Optimized Dialog Layout** — Increased the warning dialog width to **650px** and refined internal padding to improve readability and eliminate unnecessary line wrapping for detailed warning text.

### Developer Experience
- **Architecture Documentation** — Expanded `ICED_DOCUMENTATION.md` with technical details on the generalized AV detection logic and the Iced dialog sizing system.

## v3.0.4

### Artwork & Aesthetics
- **Restored UI Artwork** — The Turtle WoW background artwork is now fully restored on the Home tab's "Quick Links" section when the "I like turtles" preset is enabled.
- **Atmospheric Vignette** — Implemented a triple-layered linear gradient system (Vertical + Horizontal + Base) that creates a premium, theme-aware radial fade.
- **Glass-Frame Polish** — Refined the "Quick Links" section with a 10% opacity border that sits on top of the artwork for a crisp, framed glass effect, matching the current theme correctly.
- **Dynamic Clipping** — Enabled hardware-accelerated clipping to ensure background images stay perfectly within UI card boundaries.

### Performance & Stability
- **Race-Free Image Loading** — Implemented a `OnceLock` singleton pattern for the turtle artwork, resolving intermittent cold-start rendering issues and flickering.
- **Cargo Optimization** — Enabled hardware-accelerated `jpeg` and `png` decoding features in the build configuration.
- **API Intelligence** — Integrated a 60-second background subscription to monitor GitHub API rate limits, keeping rate-limit tooltips accurate without consuming user quota.

### Developer Experience
- **Iced Knowledge Base** — Expanded `ICED_DOCUMENTATION.md` with new sections on background image handling, simulated radial gradients, and border-occlusion strategies for future Iced 0.14 development.

## v3.0.2

### New Features

- **Live WeirdUtils Documentation** — Wuddle now pulls "live" documentation for individual WeirdUtils modules directly from the project's README on Codeberg. Usage instructions and commands will now always be up-to-date.
- **Expanded WeirdUtils Recognition** — Added native support for `worldmarkers.dll`, which now correctly displays its help icon and live documentation.

### Improvements

- **Major Architecture Refactor** — The monolithic application logic has been modularized into specialized components (`src/app/`, `src/types/`, `src/components/`, etc.). This significantly improves codebase transparency and maintenance for future contributors.
- **Decoupled Radio Spinner** — The radio connection status spinner is now decoupled from the global UI update loop, ensuring smoother interface performance during network negotiations.
- **Unified Logic Consolidation** — Shared logic for font selection (`name_font`), mod detection (`is_mod`), and component-specific presets has been moved to centralized service and theme modules.

### Bug Fixes

- **Project List Scroll Stability** — Resolved an issue where clicking inline code blocks (e.g. commands) in repository descriptions could trigger unwanted scrolling in the project list.
- **Font Rendering Fallbacks** — Optimized bold weight fallbacks for Noto Sans fonts consistently across all themes.

## v3.0.1

### Bug Fixes

- **Self-update not detecting stable releases** — the version comparison logic treated `3.0.0` as older than `3.0.0-beta.8` because pre-release suffixes added extra numeric segments. The comparison now correctly recognizes that a stable release is always newer than a pre-release of the same version.

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
