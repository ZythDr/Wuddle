# Changelog

All notable changes to Wuddle are documented in this file.

## v3.7.0-beta.12

### New Features

- **Backup and Restore**
  - Export a complete Wuddle settings backup containing profiles, preferences, launch configuration, and each profile's tracked addon, mod, and MPQ metadata.
  - Restore either a Wuddle backup ZIP or an older Wuddle installation through a read-only preview and an all-or-nothing restore. Selecting the main installation folder is supported, including bounded discovery of older nested/versioned data layouts.
  - Database snapshots are checked for integrity, the complete restore is staged before restart, and the previous data directory is retained as a rollback copy.
  - Game files, installed projects, logs, caches, GitHub tokens, and auto-login passwords are not copied. Existing same-machine vault credentials remain available through their saved non-secret references.
  - Action tooltips explain backup filenames, ZIP and older-install imports, the confirmation required before a reset, and why restore remains unavailable until a source is selected.
- **Reset Wuddle**
  - Added an explicitly confirmed reset that restarts Wuddle before removing its current and known legacy settings, profile databases, logs, and caches.
  - The warning lets users choose whether saved GitHub and auto-login credentials should also be removed from the operating system vault or retained for a later same-computer restore.
  - Old Windows AppData and legacy Tauri data are cleared so they cannot be imported again after the reset. WoW installations and deployed addon, mod, and MPQ files remain untouched.

### Bug Fixes

- **Windows Self-Updates**
  - Fixed Windows portable updates failing with `os error 32` while Wuddle inspected its own staged executable.
  - Staged executables are still validated before installation, now through the file handle Wuddle already owns rather than a conflicting second handle.

- **Windows users updating from an affected beta may need to download this release manually once.**
  - Extract it over the existing Wuddle folder, or use the new installation's Backup and Restore dialog to import the settings from an old Wuddle folder.

## v3.7.0-beta.11

### New Features

- **Text Field Context Menus**
  - Right-click editable text fields throughout Wuddle to open a themed menu for copying selected text or pasting from the clipboard.
  - Paste replaces the current selection or inserts at the existing caret position; opening the menu does not move the caret or alter the selection.
  - Secure password fields allow pasting without exposing their contents through Copy or diagnostic output.

## v3.7.0-beta.10

### Bug Fixes

- **Git Addon Modification Detection**
  - Addon rescans now identify local changes in Wuddle- and GAM-managed Git addons.
  - Detects edited or deleted files and newly added files or folders using the locally checked-out Git revision, without consuming GitHub API requests.
  - Manual and locally installed addons without a Git baseline are excluded.
  - Modified status persists through normal update checks and clears after a clean rescan, reinstall, or update.
  - Warning text and privacy-safe diagnostics now explain why an addon was flagged.

## v3.7.0-beta.9

### Improvements

- **Locally Modified Addons**
  - When an update would overwrite local addon changes, Wuddle now lets you cancel, ignore future updates, or explicitly overwrite and update.
  - Update All continues updating unaffected projects, then groups modified addons into one confirmation dialog.
  - Approved updates still use Wuddle's staged and rollback-aware installation process.
- **Ignore Updates**
  - Ignored projects are now excluded from both automatic and manual update checks before any network or GitHub API requests are made.
  - Ignore settings remain isolated to the relevant profile.

### Bug Fixes

- **Windows Addon Updates**
  - Git line-ending differences such as CRLF versus LF are no longer mistaken for local addon modifications.
  - Improved change detection for GAM-compatible moved addon folders while preserving genuine user edits.
  - Further addresses [Issue #18](https://github.com/ZythDr/Wuddle/issues/18); the issue will remain open until the reporter confirms the fix.

## v3.7.0-beta.8

### Improvements

- **Faster Update Checks**
  - Git repositories, release-based projects, and curated MPQ patches now check concurrently through separate bounded workloads.
  - Replenishing schedulers start the next waiting check as soon as one finishes, while retaining cancellation, deadlines, and joined-worker safety.

### Bug Fixes

- **Repository Replacement and Batch Updates**
  - Repository reloads now discard stale and duplicate cached update plans instead of keeping entries for repositories that were removed or replaced.
  - Update All deduplicates its targets and safely skips a repository that disappears after the batch was prepared rather than aborting every remaining update.
  - This is intended to address [Issue #18](https://github.com/ZythDr/Wuddle/issues/18); the issue remains open pending confirmation from the reporter.
- **Isolated Launch Methods**
  - Lutris now always launches its saved Lutris target through the Lutris executable.
  - Hidden values retained for Custom launching can no longer replace the Lutris command or leak Custom arguments into a Lutris launch.

## v3.7.0-beta.7

This release completes a codebase-wide security, reliability, and data-integrity audit covering 75 documented findings.

### Security & Privacy

- **Safer Network Boundaries**
  - GitHub credentials are attached only to exact trusted GitHub hosts and are never forwarded to lookalike hosts or unsafe redirects.
  - README images require credential-free standard HTTPS, public network destinations, and revalidation after every redirect, preventing private-network and metadata-service requests.
  - External README links are limited to safe credential-free web URLs; local files, custom protocols, embedded credentials, and unsafe schemes are blocked.
- **Bounded Downloads and Image Previews**
  - Release assets, curated MPQs, README images, and self-update packages now stream with strict size limits instead of buffering unbounded responses in memory.
  - README images are checked for excessive dimensions, animation frames, and decoded memory use before rendering.
  - Redirects, declared sizes, actual streamed sizes, signatures, and available SHA-256 metadata are validated before downloaded files are trusted.
- **Hardened Archive Handling**
  - ZIP and 7z packages now share one preflight policy covering traversal, links, reparse points, unsafe names, duplicate or case-colliding paths, excessive nesting, file counts, expanded sizes, and compression ratios.
  - An unsafe archive is rejected before extraction begins, and incomplete staging directories are removed after failure.
- **Safer Release Assets and Repository URLs**
  - Primary and secondary release assets must use safe cross-platform filenames and pass the same host, size, signature, digest, and cache-integrity checks.
  - HTTP repository and direct-download URLs cannot contain or retain embedded credentials. Stored legacy URLs are sanitized without altering ordinary SSH identities needed by Git repositories.
  - Git failures report privacy-safe host and error categories instead of exposing credentials, signed URLs, private remotes, or local paths.
- **Credential Storage and Authentication**
  - Linux portable installations now use Secret Service rather than a plaintext token file.
  - Legacy plaintext GitHub tokens are removed only after a verified vault write and readback; failed migration leaves the original available for recovery without activating it.
  - GitHub authentication distinguishes validated, rejected, temporarily unverifiable, and merely stored credentials, and late validation results cannot overwrite a newer token state.
  - GitHub-token and Auto-Login credential mutations are serialized so a timed-out or superseded operation cannot commit later.
- **Diagnostic Privacy**
  - Diagnostic directories and exported files now receive restrictive Unix permissions.
  - Export sanitation removes credentials, private paths, complete remotes, URL details, settings, and account information while retaining safe project labels needed to identify a failing operation.
- **Dependency and Release Supply-Chain Updates**
  - Updated advised Git, TLS, QUIC, concurrency, notification, and Wayland parser dependencies.
  - Release workflow actions and Linux packaging tools are pinned and verified rather than fetched from mutable references.

### Profile, Settings & Workflow Safety

- **Strict Profile Isolation**
  - Repository loads, update checks, installs, conflicts, removals, toggles, repairs, previews, pickers, and MPQ operations carry their originating profile and operation generation.
  - Late or superseded results are discarded before they can change another profile, reopen an old dialog, or start follow-up work against the wrong database or WoW directory.
  - Switching profiles invalidates profile-specific dialogs and work without allowing an older Profile A result to become valid after switching A → B → A.
- **Recoverable Settings**
  - Settings are written through a synchronized same-directory temporary file and atomically replaced.
  - Wuddle retains a known-good backup, preserves damaged settings for recovery, restores valid backups when possible, and reports load or save failures instead of silently resetting.
- **Retry-Safe Profile and Account Removal**
  - New profiles use persistent, non-reusable identifiers so recreating a similarly named profile cannot reconnect to an old database.
  - Profile deletion uses a durable retry state and removes the complete SQLite database family only after vault and filesystem cleanup succeed.
  - Auto-Login account deletion similarly records pending cleanup before changing the system vault and resumes interrupted work safely at startup.
- **Serialized Profile Mutations**
  - Installs, updates, reinstalls, removals, enable-state changes, MPQ edits, and rescans share one mutation boundary per profile while separate profiles remain independent.
  - Duplicate operations against the same repository are rejected, and Update All performs mutations sequentially.
- **Bounded Rescans and Shutdown**
  - Repository scans have a cooperative deadline, operation-scoped progress, and cancellation checks between major phases.
  - Timed-out or cancelled scans cannot publish stale progress or overlap later filesystem/database work.
  - Closing Wuddle during active work now shows a finishing state, saves settings, invalidates cancellable work, synchronizes diagnostics, and uses one bounded shutdown policy on Windows and Linux.

### Installation, Repair & Removal

- **Transactional Addon-Git Updates**
  - Addon-Git updates clone and validate a staged worktree before changing the live installation.
  - Dirty worktrees are preserved during routine updates, while Reinstall / Repair remains the explicit way to replace local changes.
  - Existing remotes, push URLs, refspecs, branches, and configured upstreams are carried into staged replacements instead of rewriting `origin`.
  - Accepted conflicts, collection changes, updates, and reinstalls use rollback backups plus one coherent metadata transaction, restoring the previous installation after any failure.
- **Transactional Mod Deployment**
  - Release archives, direct DLLs, raw files, secondary DLLs, stale-file cleanup, and `dlls.txt` changes are prepared before one rollback-aware deployment.
  - Managed, modified, shared, or untracked target collisions require explicit approval before live files change.
  - Displaced files receive ownership-aware backups and can be restored when the replacing mod is removed.
- **Shared File Ownership**
  - DLL and raw-file ownership is tracked across the profile so two mods cannot silently overwrite or remove each other’s files.
  - Ambiguous or disabled owners fail safely, and mods involved in an active shared-file replacement cannot be toggled into an inconsistent state.
- **Safer Repair and Removal**
  - Targeted addon repair builds and validates a replacement before removing the live entry and now reports every partial failure.
  - Reinstall / Repair preserves all secondary DLL assets rather than leaving untracked files behind.
  - Repository removal verifies tracked worktree, remote, manifest, link, and file identity before deleting anything; modified or ambiguous paths are preserved.
  - Ambiguous case-equivalent addon worktrees are never deleted automatically.
- **Atomic Enable and Disable Operations**
  - Multi-DLL and MPQ toggles preflight every rename, roll back partial filesystem changes, and restore files if metadata persistence fails.
  - MPQ editor locks are enforced inside the engine for tracked and untracked files and continue following `.disabled` filename changes.
- **Coherent MPQ State**
  - MPQ manifests, backups, protection state, custom filenames, destinations, and curated release metadata commit together and roll back together.
  - Cancelled or superseded MPQ inspection, target review, WDM resolution, installation, and removal results cannot affect a newer dialog or profile.
  - Once an MPQ deployment reaches its commit phase, the dialog cannot be dismissed until the transaction finishes.
  - Local multi-file packages now show clean, source-derived package names instead of exposing collision-safe internal identifiers.
  - A transactional package editor can rename the package and update every child MPQ's friendly name, filename, destination, and enabled state together.
  - Disabled tracked MPQs remain editable without their `.disabled` suffix being mistaken for an invalid MPQ filename.
- **GAM and Generic Git Compatibility**
  - Manual addon imports retain the directory that actually exists instead of repeatedly renaming or re-importing it from the TOC name.
  - Unknown and self-hosted Git servers remain generic Git sources with their nested namespace and remote format preserved.
  - Case-insensitive repository identity is enforced during migration while legacy installs, manifests, dependencies, and GAM moved-folder layouts remain intact.
  - Truncated GitHub tree responses fall back to the staged Git probe rather than being treated as a complete addon layout.

### Updates, Launching & Platform Reliability

- **Bounded Update Checks**
  - Git operations receive connection, server-I/O, and overall operation deadlines with cooperative cancellation and bounded concurrency.
  - Timed-out checks retain and await their workers instead of leaving detached Git activity able to hold resources or publish late results.
  - Curated patch checks also stop before starting another request after cancellation.
- **More Accurate Update Decisions**
  - Unavailable pinned releases remain pinned and produce an actionable error instead of silently falling back to Latest.
  - Multi-DLL updates compare the complete selected DLL set, including added or removed secondary files.
  - Cached ZIP, 7z, and DLL assets are revalidated and corrupt entries are discarded before retrying.
  - GitHub API conservation applies only to GitHub; GitLab, Gitea, Codeberg, generic Git, and direct sources are not skipped.
  - Infrequent-update schedules persist independently per profile.
  - Generic local MPQs are no longer sent through remote release-version fetching, while WDM and Epoch Water retain their dedicated update checks.
- **Verified Self-Updates**
  - Update selection requires a matching release tag, platform, architecture, asset name, size, and GitHub SHA-256 digest.
  - Linux updates validate the downloaded x86_64 AppImage, commit it atomically, retain one rollback image, and restore it if relaunch fails.
  - Windows updates validate and stage both the stable launcher and versioned runtime before changing `current.json`, preserving the previous launcher/runtime for rollback.
- **Windows Launcher Reliability**
  - Version selection now follows SemVer, including correct beta ordering and stable promotion, while malformed or incomplete version directories cannot outrank valid releases.
  - Restarts wait for the confirmed parent process before acquiring single-instance ownership.
  - Old runtimes are pruned only after a confirmed successful launch and never through linked, malformed, local-development, staged, or still-active paths.
- **Single-Instance Hardening**
  - Primary ownership now uses an operating-system file lock with a nonce-authenticated activation handshake.
  - Stale markers, unrelated localhost listeners, crashes, and ambiguous ownership fail safely instead of opening competing Wuddle processes.
- **Launching and Window Restoration**
  - Wine, Lutris, Custom, and bundled-tool arguments now share one non-shell parser supporting quoted values, escapes, empty arguments, and Windows paths.
  - Saved window positions are checked against connected displays, preserving valid negative-coordinate secondary-monitor positions and recovering windows stranded off-screen.
  - Linux portable and AppImage data roots now resolve from the actual executable or host AppImage rather than directory-name assumptions.

### Diagnostics, Maintenance & Release Quality

- **Expanded Operation Diagnostics**
  - Verbose diagnostics now cover meaningful MPQ, mod, DLL, and repository requests, decisions, filesystem changes, metadata commits, cancellations, errors, and rollbacks.
  - MPQ enable/disable, lock changes, renames, moves, classification, protection, installation, removal, and rescans now identify the affected safe project/component and outcome.
  - Mod and DLL toggles plus repository removals report their requested state, mechanism, affected-file count, and final result.
- **Interface Polish**
  - README preview scrollbars now remain close to the content without obscuring the surrounding preview frame.
  - Multi-file MPQ package editing keeps its header and actions visible while additional child files scroll within the dialog body.
- **Reliable Message Routing**
  - Frontend messages are classified by reference and moved into exactly one owning feature handler instead of being cloned through every router.
  - Dialog-sensitive archive messages retain the correct addon-versus-MPQ workflow, and internal routing mismatches are diagnosed rather than silently ignored.
- **Cleaner Internal Boundaries**
  - Added focused archive, deployment, network-safety, URL-safety, self-update, profile-operation, desktop-notification, and message-routing modules.
  - Removed obsolete scratch/resource files and completed crate metadata and Windows executable metadata.
- **Stronger Release Validation**
  - Releases now require exact agreement between the Git tag, Cargo manifest, lockfile, isolated changelog heading, and README version heading.
  - Stable and prerelease eligibility use explicit SemVer selection, and release packages receive structural smoke tests before upload.
  - Engine, Auto-Login, frontend, no-default-features, launcher, release-validator, formatting, dependency-audit, and strict Clippy coverage were expanded across the completed fixes.

## v3.7.0-beta.6

### Improvements

- **Window Position Memory**
  - Added an optional setting to remember Wuddle’s window size and position across restarts.
- **Improved Platform Integration**
  - Added proper Windows executable icons, metadata, and application identity.
  - Improved Wuddle’s icon identification on Linux desktops and Wayland.

### Bug Fixes

- **Update Check Deadlock** — Fixed an expired GitHub rate-limit record potentially leaving Wuddle permanently stuck checking for updates.
- **Linux Update Restart** — Fixed Wuddle closing without restarting after installing an AppImage update.

## v3.7.0-beta.5

### Fixes & Improvements

- **More Reliable Update Checks**
  - Prevented local file and antivirus scanning from indefinitely blocking update checks.
  - Added a 30-second timeout and disabled further checks until restart after a timeout.
  - Kept missing-file detection within the explicit Rescan/Repair workflow.
- **Clearer Busy Indicator** — Hovering the spinner now explains what Wuddle is working on, including repository progress and elapsed time.

## v3.7.0-beta.4

### Improvements

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

### Bug Fixes

- **Reliable Shutdown** — Closing Wuddle now reliably terminates blocked background work on Windows and Linux, preventing invisible processes from blocking future launches.
- **Update Check Stability** — Duplicate update checks are ignored, preventing overlapping operations after waking from sleep or repeatedly pressing the update button.

## v3.7.0-beta.3

### Improvements
- **Clearer GitHub API Limit Feedback**
  - Wuddle now explains when GitHub's anonymous 60-request hourly limit has been reached and approximately when it resets.
  - Rate-limit notifications link directly to GitHub Token settings and appear consistently for README previews, updates, Quick Add, curated patches, and other GitHub operations.
  - Invalid or expired token errors now provide clearer guidance.
- **Consistent Settings Icons**
  - Replaced platform-rendered cogwheel emoji with a bundled SVG across profile editing, Auto-Login, configuration actions, and file details.
  - Added theme-aware idle and hover colors with consistent sizing across UI scales.
  - Kept secondary settings controls visually distinct from the main Options icon.

### Bug Fixes
- **Profile Database Recovery**
  - Fixed `duplicate column name: fingerprint` errors preventing affected profiles from loading or installing mods and patches.
  - Partially migrated beta databases now repair themselves without losing tracked repositories or installed-file records.
  - Database initialization and migrations are serialized to prevent simultaneous operations from applying the same schema change twice.

## v3.7.0-beta.2

### New Features
- **Curated Epoch Water Patch** — Added Project Epoch's water replacement to MPQ Quick Add, with README previews and update support.
- **Project Details** — Mods, addons, and patches now have a shared Details dialog for reviewing installed files, expanding folders, and browsing their locations.

### Improvements
- **Expanded MPQ Management**
  - Custom MPQs now appear in the Patches tab and can be labelled, renamed, moved, classified, protected, enabled, or disabled.
  - Curated patch updates preserve filenames and locations chosen through Wuddle.
  - Improved Quick Add statuses, README access, menus, browsing, and management controls.
- **Friendlier Profile Management**
  - Profile cards now switch profiles when clicked, with a separate cogwheel for editing.
  - Replaced user-facing "Instance" wording with the clearer "Profile."
  - Redesigned Profile Settings with larger text, clearer spacing, keyboard tab navigation, and pinned headers and action buttons around a scrollable settings area.
  - Cached client detection prevents incompatible tabs from briefly appearing while switching profiles.
- **Clearer, More Consistent Dialogs**
  - Standardized field labels, descriptions, footer buttons, close buttons, and hover tooltips across Wuddle.
  - Addon installation now uses clearer Install and Update button wording.
  - README buttons consistently open a read-only preview without entering an installation flow.
- **Channel-Aware Changelogs** — The About page now shows stable notes on Stable and individual prerelease notes on Beta, with clearer Beta risk guidance and duplicate headings removed.

### Bug Fixes
- **Safe Profile Removal** — Removing a profile now closes its editor immediately, preventing an accidental Save from recreating it.
- **Curated Patch Updates** — Renaming WDM or Epoch Water files through Wuddle no longer creates false update notifications or loses the custom filename during updates.
- **Browse and README Actions** — Browse now opens the relevant installed files or folder instead of an unrelated companion, and Awesome WotLK's README button no longer triggers an addon-folder installation prompt.

## v3.7.0-beta.1

### New Features
- **MPQ Patch Management** — Added a dedicated Patches tab for installing and managing MPQ-based client patches.
  - Install local `.MPQ`, `.zip`, and `.7z` packages through a staged and validated installation workflow.
  - Detect existing custom, disabled, locale-specific, and core-client MPQs.
  - Classify, protect, label, rename, enable, disable, and remove supported MPQs.
  - Protect existing files by default, with conflict review, backups, rollback, and restoration when replacements are approved.
  - Place locale-named patches in matching `Data/<locale>/` directories while defaulting generic patches to `Data/`.
  - Install and update WDM for WoW 3.3.5a, including optional Caverns & Mines content and the companion addon.
- **Per-Profile Tab Visibility** — Instance Settings can hide Mods, Addons, Patches, or Tweaks for profiles where those management areas are unnecessary or unsupported.

### Improvements
- **Instance Settings Layout** — Launch Method now uses a compact dropdown, making the dialog easier to scan and leaving the segmented controls for selecting which management tabs are visible.

## v3.6.2

### New Features
- **Verbose Diagnostics** — The Logs page can now record detailed internal operations and export a rolling diagnostic ZIP for issue reports. Exported logs redact registered game paths, profile details, local archive paths, credentials, tokens, command arguments, account details, raw settings, and database contents.

### Improvements
- **GitAddonsManager Compatibility Layer** — Wuddle now recognizes GAM root addons, modular repositories, `.repo` collision worktrees, linked or moved modules, mixed-case names, remote-less worktrees, and arbitrary Git remotes without requiring an existing Wuddle database entry.
- **GAM-Compatible Deployment** — New addon-git installs still use Wuddle's staging and conflict approval, then finalize with GAM-compatible worktree names and module exposure. Unix prefers relative links while Windows retains the real-folder fallback.
- **Git Remote Preservation** — Existing addon worktrees now follow the checked-out branch's configured upstream first and preserve `origin` and other remotes instead of rewriting them.
- **Generic Git Hosting** — Self-hosted, local, SSH, and otherwise unknown Git repositories remain manageable as neutral Git sources without being misidentified as a specific forge.
- **Non-Destructive GAM Import** — GAM `.bak` and `.bak.N` folders are ignored during active-addon import, while valid linked and moved module layouts are preserved rather than needlessly repaired.
- **Complete Addon Reinstalls** — Reinstall / Repair now prepares a fresh addon-git clone in staging before replacing the live installation, removes stale or untracked files, and preserves the repository's tracked identity and settings.
- **Notification Controls and Animation** — In-app notifications can now be dismissed by right-clicking anywhere on them and use subtle fade-and-slide transitions when appearing or closing.
- **Quick Add Catalog Polish** — LuaBoost is now linked as a companion addon for wow-optimize, while the DXVK description and antivirus false-positive tooltip use clearer client-neutral wording.

### Bug Fixes
- **Cancelled Conflict Installs** — Cancelling an addon conflict now removes the pending repository from Wuddle's tracking, and new addon-git worktrees remain in staging until conflicts are accepted.
- **Conflict-Safe Finalization** — Addon files and GAM metadata reach `Interface/AddOns` only after conflict checks pass, preventing cancelled installs from leaving a second addon copy behind.
- **Addon Repository Switching** — Replacing an installed addon with a same-named fork now installs from the newly selected repository instead of continuing to use the old repository's Git remote. Refreshing or rescanning no longer removes the replacement and restores the old source. Should fix #17.
- **Multi-TOC Addon Selection** — Installing or reinstalling a single-addon repository with multiple root `.toc` files now requires an explicit main TOC choice, with a client-aware suggestion such as `Questie-335.toc` for WotLK profiles.

## v3.6.1

### New Features
- **Single-Window Launching** — Opening Wuddle while it is already running now focuses the existing window instead of starting another copy.

### Improvements
- **Auto-Login Footer Polish** — The account picker and account-management cog now have explanatory tooltips, the cog uses a cleaner borderless style, and opening the picker no longer overlaps its tooltip.
- **Clearer Launch Feedback** — The PLAY button now remains visibly pressed for at least one second while Wuddle hands the launch request to the operating system.

### Bug Fixes
- **Beta In-App Updates** — Pre-release version numbers now compare correctly, so each newer beta can be installed through Wuddle without a manual download. This hotfix is published as `v3.6.1-beta.1` so existing `v3.6.0-beta.2` installations can receive it.
- **Collection Conflict Replacement** — Overwriting an addon conflict now removes only the conflicting addon folders from an existing collection and updates that collection's selection, leaving its other addons installed.

## v3.6.0

### New Features
- **Client-Aware Quick Add** — Quick Add now shows compatible presets for Vanilla 1.12.1, TBC 2.4.3, or WotLK 3.3.5 profiles.
- **WotLK Performance Presets** — Added Awesome WotLK and wow-optimize to the WotLK Quick Add list.
- **Awesome WotLK Patching** — Wuddle can now back up `wow.exe` as `original_wow.exe` and patch it automatically. The patch action is available from the mod’s menu.
- **wow-optimize Configuration** — Added a Configure button for launching wow-optimize’s GUI.
- **Collection Selection Tools** — Add New and Manage Collection dialogs now include Select all and Clear all controls.
- **WotLK Auto-Login (Requires Awesome WotLK)** — Save multiple Auto-login accounts per instance in Linux Secret Service or Windows Credential Manager (no plaintext credentials are stored within Wuddle's directories). Wuddle can supply Awesome WotLK login arguments for WoW 3.3.5 through Auto, Wine, or Custom launch methods; Lutris remains Manual Login only.

### Improvements
- **Safer Mod Warnings** — Security notices now use clearer, client-neutral wording and include an **Open on Forge** button so you can inspect a mod’s repository first.
- **WotLK DLL Controls** — Disabling a DLL-based mod on non-Vanilla clients now renames its DLL to `.disabled` instead of relying on `dlls.txt`.
- **Collection Browser Polish** — The addon list is roomier, long names show their full text on hover, and the selection toolbar is easier to use.
- **Stable Release Notes View** — The Add New dialog keeps its preview area and buttons in place even when a repository has no release notes.
- **Compact Navigation** — Removed update counts from the Mods and Addons tabs and tightened their width for better large-scale UI support.
- **Easier Dialog Dismissal** — Routine dialogs such as Add New and profile settings can now be closed by clicking outside them. Warning and confirmation dialogs remain protected.
- **GitHub Token Feedback** — Wuddle now confirms that a saved token can be read back and shows a clear secure-storage error instead of silently falling back to anonymous GitHub access.
- **Self-Contained Windows Storage** — Windows settings and profile databases now live in `wuddle-data` beside `Wuddle.exe`. Existing AppData is copied over on first launch and kept as a rollback backup.
- **Updated Quick Add Sources** — Refreshed the Nampower and PerfBoost repository links.
- **Instance Management Refresh** — Instance cards are now compact, wrap across rows, and highlight the active instance. Instance Settings also has clearer Auto-login controls and account management access.

### Bug Fixes
- **Version Downgrades** — Selecting an older version now installs that selected version correctly and updates the Installed column.
- **WotLK Mod Filtering** — Vanilla-only Quick Add entries no longer appear for TBC or WotLK clients.
- **Awesome WotLK Backup Handling** — Existing `original_wow.exe` backups are preserved instead of being overwritten.
- **Windows GitHub Tokens** — Fixed GitHub tokens being written to a temporary in-memory credential store instead of Windows Credential Manager.
- **Case-Insensitive Auto Launch Targets** — Explicit game executable selections now resolve even when the configured filename capitalization differs from the file on disk.

## v3.5.0

### New Features
- **Local Archive Installs** — Addons can now be installed from local `.zip` or `.7z` files through the Add New Addon dialog.
- **Archive Drag-and-Drop** — Supported desktops can install addon archives by dropping them directly onto Wuddle. (does *not* work on wayland)

### Improvements
- **Manual Archive Tracking** — Local archive installs are tracked for removal, but treated as manual installs since they have no update source.
- **GAM Compatibility** — Rescan now better recognizes regular GitAddonsManager installs without turning them into duplicate manual entries.
- **Rescan Visibility** — Rescan now reports the phase and folder it is working on, making stuck scans easier to diagnose.

### Bug Fixes
- **Rescan Reliability** — Fixed cases where rescan could get stuck while importing manual folders or repairing broken installs.
- **Busy Close Handling** — Closing Wuddle while it is still working *should* now log the active task and exit more reliably.

## v3.4.0

### New Features
- **Direct Archive Links** — You can now add addons from direct HTTPS `.zip` or `.7z` download links, even when there is no repo to track for updates.
- **Release Asset Picker** — GitHub release pages with multiple compatible archives now ask which one you want to install.

### Improvements
- **Release Tag Installs** — Links to a specific GitHub release tag now stay on that release instead of drifting to the latest one later.

## v3.3.1

### Improvements
- **Main TOC Selection** — Addon repos with multiple root `.toc` files now let you choose which TOC defines the installed addon folder.

### Bug Fixes
- **Bundled Addon Libraries** — Fixed single-addon repos with bundled library TOCs, such as Questie, being treated like collections or falling back to the wrong root TOC.

## v3.3.0

### New Features
- **7z Release Support** — Mods can now install from `.7z` release archives in addition to `.zip` archives and direct `.dll` assets.

### Improvements
- **Add Dialog URL Flow** — Add New Mod/Add New Addon URLs now resolve automatically after typing stops briefly, while Enter still resolves immediately.
- **Addon-Git Branch Display** — Branch selectors now show the branch that was actually installed when using a repository's default branch.

### Bug Fixes
- **Add Dialog Focus** — Fixed Repo URL fields losing focus while previews load or resolve.
- **Forked Addon Installs** — Fixed addon-git forks with non-master default branches installing from the wrong branch.

## v3.2.7

### New Features
- **Mods Safety Warning** — Added a per-profile warning when opening the Mods tab, with a "do not show again" option for each profile.

### Improvements
- **Profile-Local Databases** — New profile databases now initialize from that profile's own `Interface/AddOns` folder instead of borrowing state from another profile.
- **Add Dialog URL Flow** — Repo previews now resolve after pressing Enter, so incomplete URLs no longer interrupt typing or steal focus.
- **Project Row Layout** — Cleaned up tracked addon/mod row sizing, column widths, and expandable-row alignment.
- **Collection Row Controls** — Collection badges now open collection management, while the chevron or empty row space still expands and collapses the row.
- **Per-Profile Update State** — Ignored updates are now stored separately per profile.

### Bug Fixes
- **Profile Isolation** — Fixed cross-profile addon leakage caused by old shared database fallback behavior.
- **Overlapping Addon Folder Names** — Fixed cases where different repos installing to the same folder name could make the wrong tracked project appear.

## v3.2.6

### Bug Fixes
- **Persistent Option State** — Fixed an issue where "Auto check for updates," "Desktop notifications," and other preference toggles would revert to their default states after restarting the application.

## v3.2.5

### New Features
- **Primary Addon Selection UI** — A new selection dialog for repositories with multiple .toc files that allows users to explicitly choose which version defines the addon folder name. This is intended for addons like pfQuest where Vanilla 1.12, TBC 2.4.3, and WotLK 3.3.5 versions of the addon are all included in the same reposi

### Improvements
- **Faster GitHub Repository Probing** — Utilizes the GitHub Tree API to analyze repository structures and detect nested addons in milliseconds without requiring a full git clone.
- **Multi-TOC Health Support** — Updated tracked addon health checks to correctly support folders containing multiple .toc files.
- **Robust Manual Pruning** — Enhanced maintenance logic to protect manual repositories with multiple expansion versions from incorrect database pruning.
- **Engine Reliability** — Improved error handling and folder detection robustness in the `wuddle-engine` library for complex repository structures.


## v3.2.4

### New Features
- **Up-To-Date Status Tooltips** — The "Up to date" status badge now features an informative hover tooltip displaying the latest version (or commit ID) alongside the exact local installation timestamp.

### Improvements
- **Streamlined Conflict Resolution UI** — The file tree preview in the "Addon Conflict" dialog has been significantly cleaned up to exclusively display directories and filter out hidden system files/folders (such as `.git` and `.editorconfig`).

## v3.2.3

### Improvements
- **Symlink Option Clarification** — Added a tooltip clarifying that `Use symlink installs when possible` applies to DLL and other non-`addon_git` installs only.
- **Recursive Collection Selection** — Top-level collection folder selections now resolve to nested `.toc` addon folders, and manage-collection checkboxes correctly reflect inherited and partial selection state even when the background probe is unavailable or still loading.
- **Install Toast Timing** — Add/install success toasts now fire only after the installation step actually completes, so large collection installs no longer report success before the work finishes.
- **GAM-Compatible Addon-Git Unpack** — `addon_git` installs now follow GitAddonsManager-style unpack/move behavior across Linux and Windows for collections and multi-directory single addons, instead of exposing sub-addon folders from the `.repo` worktree as symlinks or junctions.

### Bug Fixes
- **Windows Close Handling** — Fixed a Windows issue where closing Wuddle while it was still working could leave `Wuddle.exe` running in the background and keep files locked until the process was killed manually.
- **Busy State Recovery** — Fixed stuck busy/spinner states caused by update flows not always clearing their in-progress state after failures or no-op results.
- **Collection Selection Fallback** — Fixed addon-git collection installs so explicit collection selections are preserved even when the addon probe fails before submit, instead of silently falling back to the wrong install set.
- **Collection Removal on Windows** — Fixed tracked collection removal with `Delete local files` so junction-backed addon folders and `.repo` worktrees are removed instead of being left behind on disk.
- **Collection Child Removal on Windows** — Fixed removing a single addon from an installed collection so Windows junction-backed addon entries are deleted as links instead of recursing into the backing worktree and failing with `Access is denied`.
- **Windows Directory Link Cleanup** — Fixed collection uninstall paths to remove directory symlinks and junctions using Windows link-aware deletion instead of generic file or recursive directory removal.
- **Collection Conflict Prompting** — Changing a collection selection now opens a repo-aware overwrite confirmation instead of failing with an `ADDON_CONFLICT` error toast. The dialog shows which tracked addon folders would be removed and which conflicting folders would be installed, and the attempted selection is rolled back until the overwrite is confirmed.
- **Windows Launcher Icon** — Added the Wuddle icon resource to the Windows launcher executable so `Wuddle.exe` no longer shows the generic placeholder icon.

## v3.2.2

### New Features
- **Inline Mode Selector** — The Add Repo dialog now features a compact "Single Addon / Collection" dropdown inline with the Repo URL field. Hovering the dropdown shows a tooltip explaining the difference between the two modes.

### Improvements
- **Grouped Collections** — Addons installed from a Collection now appear under a single expandable repository row with a badge showing how many addons belong to the collection (for example, "12 addons").
- **Grouped Modular Single Addons** — Single addons that include multiple modules now appear as an expandable group similar to Collections, with a badge showing how many modules were installed (for example, "6 modules").

### Bug Fixes
- None.

## v3.2.1

### Bug Fixes
- **Collection Folder Checkboxes** — Fixed collection folder checkboxes not appearing until the background addon probe completed (which could take 10–30+ seconds). Checkboxes now appear immediately when opening Manage Collection.
- **Collection Toggle Silently Dropped** — Fixed folder checkbox clicks being silently discarded in manage mode when the probe hadn't loaded yet. The selected state is now correctly updated on every click.
- **Collection Matching Robustness** — Improved the folder-to-addon matching fallback chain so checkboxes correctly reflect keep/remove state even before the probe finishes.
- **Dialog Overlay** — Fixed a gap where clicks on the dialog scrim could interact with content behind the dialog.

## v3.2.0

### New Features
- **Collection Addon Management** — Treat addon-git repositories as real collections, choose which addon folders to keep directly in the Add Repo preview, and manage installed collections later without re-adding the repo.
- **Nested Addon Discovery** — Wuddle now detects addon folders with `.toc` files up to 5 levels deep in addon-git repositories.

### Improvements
- **Custom Executable Targeting** — Profiles can now target renamed or irregularly named game executables for Auto launch and Tweaks instead of only relying on `Wow.exe` or `VanillaFixes.exe`.
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
