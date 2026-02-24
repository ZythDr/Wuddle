# Changelog

All notable changes to Wuddle are documented in this file.

## v2.0.0 (v2-dev snapshot)

### Core direction
- Shifted Wuddle toward a launcher + manager model while keeping DLL mod workflows intact.
- Added profile/instance-aware navigation with dedicated Home, Mods, and Addons views.
- Unified app behavior around per-instance state and explicit update actions.

### UI / UX
- Major visual overhaul toward a launcher-style interface.
- Reworked top navigation:
  - Home / Mods / Addons view tabs.
  - Profile switcher dropdown in top bar.
  - Icon-based utility tabs/buttons for settings/logs/about.
- Added Home view with:
  - Update overview for mods + addons.
  - Context actions (Add new, Check for updates, Update all).
  - PLAY button integration.
- Added/updated multiple interaction quality improvements:
  - Better busy indicator behavior.
  - Reduced UI lock-ups during background operations.
  - Consistent dropdown styling across dialogs and tables.
  - Fixed dropdown z-index/stacking behavior (including branch dropdowns).
  - Improved row/header layering and clipping behavior in project tables.
  - Adjusted typography and branding (including LifeCraft title font).

### Mods management
- Expanded curated Quick Add list and card UX.
- Added richer warning flow for SuperWoW false-positive AV behavior.
- Improved remove/confirm flows and conflict handling.
- Improved status badges, filters, and footer actions.
- Added clearer operation logging for add/update/reinstall actions.

### Addons management (new)
- Added real addon git-sync mode (`addon_git`) using clone/fetch/pull workflows.
- Added addon branch support:
  - Per-addon branch selector in Addons view.
  - Branch listing and selection persistence.
- Added import path for existing local git-based addons into Wuddle tracking.
- Added conflict detection/confirmation flow for existing addon directories.

### Addon folder detection / deployment
- Implemented `.toc`-driven addon root detection for deployment into `Interface/AddOns`.
- Added TOC suffix normalization and canonicalization behavior for expansion/channel variants.
- Improved handling of multi-TOC repos and reduced duplicate target folder creation.
- Added cleanup for stale tracked addon targets when canonical target set changes.

### Launcher behavior
- Added instance-level launch configuration model:
  - Auto launch behavior.
  - Optional Lutris target flow.
  - Wine/custom command support hooks.
  - Optional working directory + environment override fields.
- Added PLAY action plumbing in app for selected instance context.

### Logging / observability
- Added structured per-operation step logs surfaced in GUI logs:
  - Source URL/mode/branch context.
  - Asset/sync steps where relevant.
  - Target installation paths (addon/dll/raw installs).
- Improved error visibility for addon conflicts and retry/cancel paths.

### Build / release workflow
- Introduced/adjusted branch-based development workflow (`v2-dev` line).
- Updated app version metadata to `2.0.0` for GUI/package/build config.
- Improved project metadata and documentation around credits/inspiration.

### Credits / attribution
- Added explicit attribution comments for logic inspired by:
  - GitAddonsManager (addon TOC/suffix and git-addon workflow ideas).
  - WoWRetroLauncher (UI direction inspiration).

