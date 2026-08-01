# Wuddle

Wuddle is a desktop WoW launcher/manager primarily focusing Vanilla clients, with support for:

- DLL mod management (install/update)
- Git-based addon management (inspired by [GitAddonsManager](https://gitlab.com/woblight/GitAddonsManager))
- Multi-instance profiles
- One-click game launch per instance

> [!WARNING]
> Please scroll down and read the Important Note before downloading.

[Screencast_20260414_150926.webm](https://github.com/user-attachments/assets/231d99f9-1809-49e8-b6ba-6117876c08bc)


# Important Note (Anti-virus false positives)
Various DLL mods such as SuperWoW, UnitXP_SP3 and Nampower for the vanilla 1.12 clients are known to trigger false-positives in many antivirus products.  
Wuddle displays a warning before adding known false-positive triggering mods from it's Quick Add section. If any false-positive mod is installed through Wuddle, antivirus tools may attribute the detection to `Wuddle.exe` because Wuddle performs the download/install action.  
If this happens, you need to add the game installation folder to your Anti-virus' exclusions/whitelist or else the files will keep being deleted.

### Core Features

- **Addon management:** install, update, reinstall/repair, remove
- **DLL mod management:** install, update, reinstall/repair, remove
- **Multi-forge support:** GitHub, Codeberg, Gitea, GitLab
- **Quick Add catalog:** common Vanilla client mods with descriptions
- **Companion addon links/info:** shown directly in quick-add entries
- **`dlls.txt` management:** enable/disable mods in Wuddle without having to uninstall them
- **Multi-instance profiles:** each profile has its own tracked mods/addons + launch config
- **GitHub auth token (optional):** helps avoid anonymous API limits (60 per hour)
- **Logs panel:** operational visibility and copyable logs

### What's New in v3.7.0-beta.10

- **Reliable Modified Addon Detection** — Rescanning now identifies edited, deleted, and newly added files in Wuddle- and GAM-managed Git addons.
- **No Extra API Usage** — Modification detection compares against the local checked-out Git revision without contacting GitHub.
- **Manual Addons Left Alone** — Addons without a local Git baseline are excluded from modification scanning.
- **Clearer Warnings and Diagnostics** — Modified status is retained reliably, while prompts and privacy-safe logs better explain why an addon was flagged.

See the [full changelog](CHANGELOG.md#v370-beta10) for the technical details.

<details>
<summary><strong>v3.x Changelog</strong></summary>

### v3.7.0-beta.9

- **Safer Updates for Modified Addons** — Wuddle now asks whether to cancel, ignore future updates, or explicitly overwrite when an addon contains local changes.
- **Better Batch Behavior** — Update All completes unaffected projects before grouping modified addons into one confirmation dialog.
- **Real Update Ignoring** — Ignored projects are excluded from automatic and manual checks before any network or GitHub API request.
- **Windows Line-Ending Fix** — Normal CRLF/LF differences are no longer mistaken for local addon edits, further addressing [Issue #18](https://github.com/ZythDr/Wuddle/issues/18).

### v3.7.0-beta.8

- **Faster Update Checks**
  - Git repositories, release-based projects, and curated MPQ patches now check concurrently through separate bounded workloads.
  - New checks begin as soon as a worker becomes available without weakening cancellation or timeout safeguards.
- **More Reliable Repository Updates**
  - Stale and duplicate cached update entries are discarded when repositories are removed, replaced, or reloaded.
  - Update All now skips vanished repositories instead of aborting the remaining batch.
  - This is intended to address [Issue #18](https://github.com/ZythDr/Wuddle/issues/18), which remains open pending reporter confirmation.
- **Reliable Lutris Launching**
  - Lutris launches are isolated from saved Custom command fields, preventing hidden Custom values from replacing the Lutris executable or arguments.

### v3.7.0-beta.7

This beta completes a full security, reliability, and data-integrity review of Wuddle, addressing 75 documented findings.

- **Safer Installs and Removals** — Addons, mods, DLLs, and MPQs now use stronger staging, ownership checks, backups, and rollback handling.
- **Stronger Security and Privacy** — Hardened downloads, archives, README previews, redirects, credentials, diagnostic exports, and release assets.
- **Reliable Profile Isolation** — Delayed background work can no longer affect the wrong profile, database, dialog, or WoW installation.
- **More Dependable Updates** — Added bounded Git operations, cancellation, stronger cache validation, accurate multi-file checks, and safer API budgeting.
- **Verified Self-Updates** — Linux and Windows updates now validate exact packages and digests while retaining rollback copies.
- **Launcher and Platform Hardening** — Improved Windows version selection, restart handoff, single-instance ownership, Linux portable paths, and window restoration.
- **Better MPQ Behavior** — MPQ changes are more transactional, local patches are no longer incorrectly treated as remote repositories, and multi-file packages now have clean names plus a unified package editor.
- **Expanded Diagnostics** — Meaningful actions, file changes, metadata commits, failures, and rollbacks are now easier to trace without exposing private data.

### v3.7.0-beta.6

#### Improvements
- **Window Position Memory**
  - Added an optional setting to remember Wuddle’s window size and position across restarts.
- **Improved Platform Integration**
  - Added proper Windows executable icons, metadata, and application identity.
  - Improved Wuddle’s icon identification on Linux desktops and Wayland.

#### Bug Fixes
- **Update Check Deadlock** — Fixed an expired GitHub rate-limit record potentially leaving Wuddle permanently stuck checking for updates.
- **Linux Update Restart** — Fixed Wuddle closing without restarting after installing an AppImage update.

### v3.7.0-beta.5

#### Fixes & Improvements
- **More Reliable Update Checks**
  - Prevented local file and antivirus scanning from indefinitely blocking update checks.
  - Added a 30-second timeout and disabled further checks until restart after a timeout.
  - Kept missing-file detection within the explicit Rescan/Repair workflow.
- **Clearer Busy Indicator** — Hovering the spinner now explains what Wuddle is working on, including repository progress and elapsed time.

### v3.7.0-beta.4

#### Improvements
- **Conserve GitHub API** — Added an optional setting for reducing anonymous GitHub API usage.
  - Infrequently updated projects follow a longer update schedule.
  - Their status now persists across profile switches.
  - The setting automatically becomes inactive when unnecessary.
- **Improved Diagnostics** — Verbose logs now record privacy-safe update-check stages, timings, outcomes, and file-verification progress.
  - Parallel repository checks are tracked independently.
  - Exported summaries include the active operation when diagnostics are created.
- **Improved Notifications** — Added a smooth lifetime indicator to notifications.
  - Hovering pauses and resets the timer.
  - Lengthy tooltips and API-limit warnings are formatted for easier reading.

#### Bug Fixes
- **Reliable Shutdown** — Closing Wuddle now reliably terminates blocked background work on Windows and Linux, preventing invisible processes from blocking future launches.
- **Update Check Stability** — Duplicate update checks are ignored, preventing overlapping operations after waking from sleep or repeatedly pressing the update button.

### v3.7.0-beta.3

#### Improvements
- **Clearer GitHub API Limit Feedback**
  - Wuddle now explains when GitHub's anonymous hourly limit has been reached and approximately when it resets.
  - Notifications link directly to GitHub Token settings and consistently cover previews, updates, Quick Add, curated patches, and other GitHub operations.
  - Invalid or expired token errors now provide clearer guidance.
- **Consistent Settings Icons**
  - Replaced platform-rendered cogwheel emoji with a bundled SVG across profile editing, Auto-Login, configuration actions, and file details.
  - Added theme-aware idle and hover colors with consistent sizing across UI scales while keeping the main Options icon visually distinct.

#### Bug Fixes
- **Profile Database Recovery** — Fixed migration errors that could prevent affected profiles from loading or installing mods and patches. Partially migrated beta databases now repair themselves automatically without losing tracked projects or installed-file records.

### v3.7.0-beta.2

#### New Features
- **Curated Epoch Water Patch** — Added Project Epoch's water replacement to MPQ Quick Add, with README previews and update support.
- **Project Details** — Mods, addons, and patches now share a Details dialog for reviewing installed files and browsing their locations.

#### Improvements
- **Expanded MPQ Management**
  - Manage custom MPQs directly from the Patches tab, including names, locations, classifications, protection, and enabled states.
  - Curated patch updates preserve filenames and locations chosen through Wuddle.
  - Improved Quick Add statuses, README access, menus, browsing, and management controls.
- **Friendlier Profile Management**
  - Profile cards now switch profiles when clicked, with a separate cogwheel for editing.
  - Replaced user-facing "Instance" wording with the clearer "Profile."
  - Redesigned Profile Settings for improved readability, keyboard navigation, and smaller screens.
  - Cached client detection prevents incompatible tabs from briefly appearing while switching profiles.
- **Clearer, More Consistent Dialogs** — Standardized labels, descriptions, buttons, close controls, tooltips, and read-only README previews across Wuddle.
- **Channel-Aware Changelogs** — The About page now shows stable notes on Stable and individual prerelease notes on Beta, with clearer Beta guidance and duplicate headings removed.

#### Bug Fixes
- **Safe Profile Removal** — Removing a profile now closes its editor immediately, preventing an accidental Save from recreating it.
- **Curated Patch Updates** — Renaming WDM or Epoch Water files no longer creates false update notifications or loses the custom filename during updates.
- **Browse and README Actions** — Browse now opens the relevant installed location, and Awesome WotLK's README button no longer triggers an addon-folder installation prompt.

### v3.7.0-beta.1

#### New Features
- **MPQ Patch Management** — Added a dedicated Patches tab for installing and managing MPQ-based client patches.
  - Install local `.MPQ`, `.zip`, and `.7z` packages through a staged and validated installation workflow.
  - Detect existing custom, disabled, locale-specific, and core-client MPQs.
  - Classify, protect, label, rename, enable, disable, and remove supported MPQs.
  - Protect existing files by default, with conflict review, backups, rollback, and restoration when replacements are approved.
  - Place locale-named patches in matching `Data/<locale>/` directories while defaulting generic patches to `Data/`.
  - Install and update WDM for WoW 3.3.5a, including optional Caverns & Mines content and the companion addon.
- **Per-Profile Tab Visibility** — Instance Settings can hide Mods, Addons, Patches, or Tweaks for profiles where those management areas are unnecessary or unsupported.

#### Improvements
- **Instance Settings Layout** — Launch Method now uses a compact dropdown, making the dialog easier to scan and leaving the segmented controls for selecting which management tabs are visible.

### v3.6.2

#### New Features
- **Verbose Diagnostics** — Record detailed internal operations and export a rolling, privacy-sanitized diagnostic ZIP for issue reports.

#### Improvements
- **GitAddonsManager Compatibility Layer** — Recognizes GAM root addons, modular repositories, `.repo` collision worktrees, linked or moved modules, backup folders, mixed-case names, and arbitrary Git remotes.
- **GAM-Compatible Deployment** — Uses Wuddle's safer staging and conflict approval while finalizing new addon-git installs with GAM-compatible worktree names and module exposure.
- **Git Remote Preservation** — Follows the checked-out branch's configured upstream and preserves existing remotes instead of rewriting `origin`.
- **Complete Addon Reinstalls** — Reinstall / Repair prepares a fresh clone in staging, removes stale files, and preserves tracked repository settings.
- **Notification Controls and Animation** — Right-click in-app notifications to dismiss them, with subtle fade-and-slide transitions when they appear or close.
- **Quick Add Catalog Polish** — Adds LuaBoost as a companion addon for wow-optimize and clarifies the DXVK and antivirus warning text.

#### Bug Fixes
- **Cancelled Conflict Installs** — Cancelling an addon conflict no longer leaves a tracked repository or staged addon files behind.
- **Conflict-Safe Finalization** — Addon files and GAM metadata reach `Interface/AddOns` only after conflict checks are accepted.
- **Addon Repository Switching** — Replacing an addon with a same-named fork now keeps the newly selected repository through refresh and rescan. Should fix #17.
- **Multi-TOC Addon Selection** — Installing or reinstalling repositories such as Questie now requires an explicit main TOC choice and offers a client-aware suggestion.

### v3.6.0 / v3.6.1

#### New Features
- **WotLK Auto-Login** — Save multiple accounts per game profile using Linux Secret Service or Windows Credential Manager, then launch WoW 3.3.5 through Awesome WotLK without storing credentials in Wuddle's settings.
- **Client-Aware Quick Add** — See compatible Vanilla 1.12.1, TBC 2.4.3, or WotLK 3.3.5 presets, including Awesome WotLK and wow-optimize.
- **WotLK Mod Tools** — Patch Awesome WotLK safely with an automatic executable backup and launch wow-optimize's configuration interface from Wuddle.
- **Single-Window Launching** — Opening Wuddle again now focuses the existing window instead of starting another copy.

#### Improvements
- **Instance Management Refresh** — Compact wrapping instance cards, clearer active-profile highlighting, and focused Auto-login controls make larger profile collections easier to manage.
- **Collection Selection Tools** — Add New and Manage Collection dialogs now include Select all and Clear all controls.
- **Auto-Login Footer Polish** — The account selector and borderless management cog include explanatory tooltips and cleaner launch feedback.

#### Bug Fixes
- **Beta In-App Updates** — Pre-release versions now compare correctly. The v3.6.1 version number was used to deliver this updater hotfix to existing v3.6.0 beta installations.
- **Collection Conflict Replacement** — Overwriting a conflict removes only the conflicting addon folders from an existing collection instead of deleting the entire collection.
- **Case-Insensitive Auto Launch Targets** — Explicit game executables resolve even when filename capitalization differs from the path saved in Wuddle.

### v3.5.0

#### New Features
- **Local Archive Installs** — Add addons from local `.zip` or `.7z` files through the Add New Addon dialog.
- **Archive Drag-and-Drop** — Supported desktops can install addon archives by dropping them directly onto Wuddle. (does *not* work on Wayland)

#### Improvements
- **GAM Compatibility** — Rescan now better recognizes regular GitAddonsManager installs.
- **Rescan Visibility** — Rescan now logs what it is working on when a scan takes longer than expected.

### v3.4.0

#### New Features
- **Direct Archive Links** — Add addons from direct HTTPS `.zip` or `.7z` download links, even when there is no repo to track for updates.
- **Release Asset Picker** — GitHub release pages with multiple compatible archives now ask which one you want to install.

#### Improvements
- **Release Tag Installs** — Links to a specific GitHub release tag now stay on that release instead of drifting to the latest one later.

### v3.3.1

#### Improvements
- **Main TOC Selection** — Addon repos with multiple root `.toc` files now let you choose which TOC defines the installed addon folder.

#### Bug Fixes
- **Bundled Addon Libraries** — Fixed single-addon repos with bundled library TOCs, such as Questie, being treated like collections or falling back to the wrong root TOC.

### v3.3.0

#### New Features
- **7z Release Support** — Mods can now install from `.7z` release archives in addition to `.zip` archives and direct `.dll` assets.

#### Improvements
- **Add Dialog URL Flow** — Add New Mod/Add New Addon URLs now resolve automatically after typing stops briefly, while Enter still resolves immediately.
- **Addon-Git Branch Display** — Branch selectors now show the branch that was actually installed when using a repository's default branch.

#### Bug Fixes
- **Add Dialog Focus** — Fixed Repo URL fields losing focus while previews load or resolve.
- **Forked Addon Installs** — Fixed addon-git forks with non-master default branches installing from the wrong branch.

### v3.2.7

#### New Features
- **Mods Safety Warning** — Added a per-profile warning when opening the Mods tab, with a "do not show again" option for each profile.

#### Improvements
- **Profile-Local Databases** — New profile databases now initialize from that profile's own `Interface/AddOns` folder instead of borrowing state from another profile.
- **Add Dialog URL Flow** — Repo previews now resolve after pressing Enter, so incomplete URLs no longer interrupt typing or steal focus.
- **Project Row Layout** — Cleaned up tracked addon/mod row sizing, column widths, and expandable-row alignment.
- **Collection Row Controls** — Collection badges now open collection management, while the chevron or empty row space still expands and collapses the row.
- **Per-Profile Update State** — Ignored updates are now stored separately per profile.

#### Bug Fixes
- **Profile Isolation** — Fixed cross-profile addon leakage caused by old shared database fallback behavior.
- **Overlapping Addon Folder Names** — Fixed cases where different repos installing to the same folder name could make the wrong tracked project appear.

### v3.2.6

#### Bug Fixes
- **Persistent Option State** — Fixed an issue where "Auto check for updates," "Desktop notifications," and other preference toggles would revert to their default states after restarting the application.

### v3.2.5

#### New Features
- **Intelligent Addon Suggestions** — Automatically badges the most compatible addon version during installation based on the detected WoW client version.
- **Fast GitHub Repository Probing** — Utilizes the GitHub Tree API to analyze repository structures and detect nested addons in milliseconds without requiring a full git clone.
- **Primary Addon Selection UI** — A new selection dialog for repositories with multiple .toc files that allows users to explicitly choose which version defines the addon folder name.

#### Improvements
- **Multi-TOC Health Support** — Updated tracked addon health checks to correctly support folders containing multiple .toc files.
- **Robust Manual Pruning** — Enhanced maintenance logic to protect manual repositories with multiple expansion versions from incorrect database pruning.
- **Refined Dialog Aesthetics** — Removed redundant internal borders and increased internal padding for a cleaner, more spacious dialog interface.
- **Enhanced Visual Feedback** — Added a translucent suggestion badge style with high-contrast outlines and descriptive tooltips explaining the suggestion logic.
- **Optimized Secondary Selection** — Refined the "Install as Collection instead" button with subtle bordering and dimmed text for a more distinct secondary action.
- **Engine Reliability** — Improved error handling and folder detection robustness in the `wuddle-engine` library for complex repository structures.

### v3.2.4

#### New Features
- **Up-To-Date Status Tooltips** — The "Up to date" status badge now features an informative hover tooltip displaying the latest version (or commit ID) alongside the exact local installation timestamp.

#### Improvements
- **Streamlined Conflict Resolution UI** — The file tree preview in the "Addon Conflict" dialog has been significantly cleaned up to exclusively display directories and filter out hidden system files/folders (such as `.git` and `.editorconfig`).

### v3.2.3

#### Improvements
- **Symlink Option Clarification** — Added a tooltip clarifying that `Use symlink installs when possible` applies to DLL and other non-`addon_git` installs only.
- **Recursive Collection Selection** — Top-level collection folder selections now resolve to nested `.toc` addon folders, and manage-collection checkboxes correctly reflect inherited and partial selection state even when the background probe is unavailable or still loading.
- **Install Toast Timing** — Add/install success toasts now fire only after the installation step actually completes, so large collection installs no longer report success before the work finishes.
- **GAM-Compatible Addon-Git Unpack** — `addon_git` installs now follow GitAddonsManager-style unpack/move behavior across Linux and Windows for collections and multi-directory single addons, instead of exposing sub-addon folders from the `.repo` worktree as symlinks or junctions.

#### Bug Fixes
- **Windows Close Handling** — Fixed a Windows issue where closing Wuddle while it was still working could leave `Wuddle.exe` running in the background and keep files locked until the process was killed manually.
- **Busy State Recovery** — Fixed stuck busy/spinner states caused by update flows not always clearing their in-progress state after failures or no-op results.
- **Collection Selection Fallback** — Fixed addon-git collection installs so explicit collection selections are preserved even when the addon probe fails before submit, instead of silently falling back to the wrong install set.
- **Collection Removal on Windows** — Fixed tracked collection removal with `Delete local files` so junction-backed addon folders and `.repo` worktrees are removed instead of being left behind on disk.
- **Collection Child Removal on Windows** — Fixed removing a single addon from an installed collection so Windows junction-backed addon entries are deleted as links instead of recursing into the backing worktree and failing with `Access is denied`.
- **Windows Directory Link Cleanup** — Fixed collection uninstall paths to remove directory symlinks and junctions using Windows link-aware deletion instead of generic file or recursive directory removal.
- **Collection Conflict Prompting** — Changing a collection selection now opens a repo-aware overwrite confirmation instead of failing with an `ADDON_CONFLICT` error toast. The dialog shows which tracked addon folders would be removed and which conflicting folders would be installed, and the attempted selection is rolled back until the overwrite is confirmed.
- **Windows Launcher Icon** — Added the Wuddle icon resource to the Windows launcher executable so `Wuddle.exe` no longer shows the generic placeholder icon.

### v3.2.2

#### New Features
- **Inline Mode Selector** — The Add Repo dialog now features a compact "Single Addon / Collection" dropdown inline with the Repo URL field. Hovering the dropdown shows a tooltip explaining the difference between the two modes.

#### Improvements
- **Grouped Collections** — Addons installed from a Collection now appear under a single expandable repository row with a badge showing how many addons belong to the collection (for example, "12 addons").
- **Grouped Modular Single Addons** — Single addons that include multiple modules now appear as an expandable group similar to Collections, with a badge showing how many modules were installed (for example, "6 modules").

#### Bug Fixes
- None.

### v3.2.1

#### Bug Fixes
- **Collection Folder Checkboxes** — Fixed collection folder checkboxes not appearing until the background addon probe completed (which could take 10–30+ seconds). Checkboxes now appear immediately when opening Manage Collection.
- **Collection Toggle Silently Dropped** — Fixed folder checkbox clicks being silently discarded in manage mode when the probe hadn't loaded yet. The selected state is now correctly updated on every click.
- **Collection Matching Robustness** — Improved the folder-to-addon matching fallback chain so checkboxes correctly reflect keep/remove state even before the probe finishes.
- **Dialog Overlay** — Fixed a gap where clicks on the dialog scrim could interact with content behind the dialog.

### v3.2.0

#### New Features
- **Collection Addon Management** — Treat addon-git repositories as real collections, choose which addon folders to keep directly in the Add Repo preview, and manage installed collections later without re-adding the repo.
- **Nested Addon Discovery** — Wuddle now detects addon folders with `.toc` files up to 5 levels deep in addon-git repositories.

#### Improvements
- **Custom Executable Targeting** — Profiles can now target renamed or irregularly named game executables for Auto launch and Tweaks instead of only relying on `Wow.exe` or `VanillaFixes.exe`.
- **Targeted Tweaks Feedback** — Tweaks now reports which executable is being inspected and clearly explains when the selected client is not compatible with legacy 1.12.1 patching.

#### Bug Fixes
- **Collection Matching Fixes** — Fixed collection management for repositories whose folder names differ from the installed addon name, including common GitHub suffixes like `-master` and `-main`.
- **Nested Install Linking** — Fixed nested addon installs and repair flows so the correct repo-relative folder is linked or moved.

#### Removed
- **Legacy Radio UI** — Removed the in-app radio player and its related settings UI.
- **Turtle-Specific Home Links** — Removed the Turtle-only links section from the Home tab.
- **`I like turtles` Profile Flag** — Removed the old profile toggle that controlled Turtle-themed home content.

### v3.1.0
- **Browse to Folder** — Quickly open the local folder for any tracked addon or mod directly from the UI.
- **Linux Stabilization** — Addon path tracking is now case-insensitive, preventing re-import issues and "ghost" entries on Linux filesystems.
- **Non-Blocking Rescan** — Broken path repair runs asynchronously during Rescan, preventing UI freezes during intensive repair operations.
- **Targeted Link Repair** — Normal refresh and launch only verify tracked addon links and repair broken entries on demand instead of scanning broadly.
- **Focused Startup Checks** — Automatic update checks now stay on the network/version path instead of running addon maintenance work first.
- **Rescan Phase Visibility** — Rescan now logs repair, cleanup, prune, import, and dedup phases with timing details in the Logs tab.
- **Cleaner Scans** — Improved manual scanning logic now ignores metadata and non-addon folders by strictly validating for `.toc` files.
- **Case-Insensitive Database** — Implemented `COLLATE NOCASE` in SQLite for repository lookups to prevent duplicate entries from varying URL casings.

### v3.0.7
- **API Transparency & Log Filtering** — Introduced a new `[API]` log category with a dedicated filter button and Cyan highlighting for technical budget tracking.
- **Immediate UI Refresh** — Successfully updated repositories are now instantly cleared from the Home tab's update list.
- **Restored Detailed Logging** — Verbose per-repository update reporting has been re-implemented for both single and bulk updates.

### v3.0.6
- **Token-Aware Update Checking** — Authenticated users now always perform full repository checks, while anonymous users benefit from optimized selective checks to stay within API limits.
- **Visual De-cluttering** — Removed "Infrequent Mod" indicators for authenticated users.
- **Reliability Fixes** — Corrected check timestamp logic to ensure fresh update results across both manual and auto-check modes.

### v3.0.5
- **Anti-Virus safety warnings** — Restored and generalized the warning dialog for mods known to trigger security heuristics (SuperWoW, VanillaFixes, UnitXP_SP3). 
- **Optimized Dialog Layout** — Increased warning dialog width to 650px for better readability and refined installation logic to prevent uninformed mod additions.

### v3.0.4

- **Turtle WoW background artwork** — Restored the Turtle WoW artwork background on the Home tab for when the "I like turtles" preset is enabled.
- **API Usage background sync** — Integrated a 60-second background subscription to monitor GitHub API rate limits, keeping rate-limit tooltips accurate without consuming user quota.


### v3.0.2

- **Live DLL Documentation** — Wuddle now pulls "live" documentation for individual WeirdUtils modules directly from the project's README on Codeberg. Usage instructions and commands will now always be up-to-date.
- **Improved Codebase Architecture** — Complete refactor into a professional, modular structure for better maintenance and transparency.
- **Fixed Scroll Stability** — Resolved a persistent issue where clicking inline code blocks in descriptions would trigger unwanted list scrolling.
- **Decoupled System Feedback** — Radio connection states and other background tasks are now decoupled from the main UI thread for a smoother experience.

### v3.0

Wuddle v3 is a complete frontend rewrite from Tauri/WebView to [Iced 0.14](https://iced.rs), rendering natively via wgpu (Vulkan/Metal/DX12). No WebView, no browser engine overhead.

- **In-game radio player** — stream the Everlook Broadcasting Co. radio with play/stop, volume controls (click-to-mute, scroll-to-adjust), auto-connect, auto-play, and configurable buffer via Radio Settings.
- **DXVK Configurator** — interactive `dxvk.conf` editor with per-setting tooltips, syntax-highlighted preview, and Turtle WoW-specific presets.
- **Version pinning** — per-mod dropdown to lock to a specific release tag while still tracking the latest version.
- **Merge updates mode** — per-repo toggle to keep existing files and only overwrite matching ones during updates.
- **DLL count mismatch warning** — prompts for Merge vs Clean update when the number of DLLs changes between releases.
- **Multi-DLL expand/collapse** — mods installing multiple DLLs appear as expandable parent rows with per-DLL enable/disable toggles.
- **Remove dialog with file preview** — scrollable file tree of every installed file before confirming removal.
- **GitHub-flavored admonitions** — README previews render `[!NOTE]`, `[!TIP]`, `[!WARNING]`, etc. with colored accents and icons.
- **Auto-scaling for smaller monitors** — detects monitor resolution and scales the UI automatically, with manual scale buttons (75%–120%) in Options.
- **GAM-compatible addon deployment** — addons are now cross-compatible with GitAddonsManager and the TurtleWoW launcher out of the box.
- **Mod cache in WoW directory** — simplifies antivirus whitelisting on Windows.

</details>

<details>
<summary><strong>v2.x Changelog</strong></summary>

### v2.5

- Bidirectional settings sync between Tauri and Iced frontends
- Release channel selector (Stable/Beta) with seamless v3 upgrade path
- Add dialog repo preview with README, file tree, and About panel
- Clickable file preview with syntax highlighting
- Addon deduplication with case-insensitive matching
- Fixed desktop notifications on Linux (D-Bus via notify-rust)
- Scroll-aware edge fading, sticky dialog footers
- Performance improvements (shared HTTP client, targeted updates, LRU caches)

### v2.4

- **Tweaks tab** — patch WoW.exe with quality-of-life improvements (FoV, Farclip, Quickloot, Camera fixes, etc.)
- Desktop notifications for mod/addon updates
- In-app changelog viewer
- Linux AppImage self-update
- Adaptive update frequency
- Auto-clear WDB cache, ignore updates per-repo
- Assets-pending detection for self-update

### v2.3

- Mod file integrity checking via SHA-256
- Automatic cache cleanup
- Addon conflict detection dialog
- Auto-check for updates with configurable interval
- Turtle WoW home section with community links
- Visual theme picker

### v2.0

- Evolved from DLL updater into launcher + manager
- Addon management with Git clone/pull and branch selection
- Home tab with PLAY button and per-instance launch methods
- Multi-instance profile switching
- Visual themes

</details>

## Credits / Inspiration

Wuddle is its own implementation, but parts of the functionality and UX were inspired by:

- **[GitAddonsManager](https://gitlab.com/woblight/GitAddonsManager)** by WobLight  
  Git addon update workflows, `.toc`-driven addon deployment ideas, and branch-oriented addon management.  


- **[WoWRetroLauncher](https://github.com/Parquelle/WoWRetroLauncher)** by Parquelle  
  Sparked the idea for Wuddle's themes.  


- **[vanilla-tweaks](https://github.com/brndd/vanilla-tweaks)** by brndd  
  WoW.exe binary patching logic for the Tweaks tab (FoV, farclip, quickloot, camera fixes, etc.).  
