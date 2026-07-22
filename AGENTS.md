# Wuddle — Development Context

> This file provides architectural context, product direction, invariants, and development guidance for AI assistants working on Wuddle.

## Product Vision

Wuddle is a native desktop launcher and manager for legacy World of Warcraft clients.

It aims to provide:

- A GitAddonsManager-compatible addon manager with safer installation flows and additional collection-management features.
- DLL mod installation, updating, repair, removal, and enable/disable controls.
- Separate game profiles, each with its own WoW directory, tracked projects, database, and launch configuration.
- Client-aware support for Vanilla 1.12.1, TBC 2.4.3, and WotLK 3.3.5.
- Straightforward game launching through Auto, Lutris, Wine, or Custom commands.
- Optional secure WotLK 3.3.5 auto-login through Awesome WotLK.
- Useful diagnostics that users can safely attach to GitHub issues.
- A polished, self-explanatory interface for users who may not be comfortable managing Git repositories, DLLs, Wine commands, or addon folders manually.

The addon-management goal is effectively “GitAddonsManager plus additional features and a friendlier UX.” When Wuddle and GAM support equivalent behavior, Wuddle should remain compatible with GAM’s filesystem layouts. Wuddle-specific features may add metadata outside GAM-managed worktrees when GAM has no equivalent concept.

## Product Principles

- Preserve user data and existing installations.
- Prefer transactional operations that can be cancelled or rolled back safely.
- Keep profiles completely isolated from one another.
- Never expose credentials, tokens, command arguments, or private paths in logs.
- Keep advanced behavior understandable through clear labels, dialogs, tooltips, and actionable errors.
- Support Linux and Windows deliberately; do not treat either platform as an afterthought.
- Prefer compatibility with existing user setups over forcing migrations or reinstalls.
- Avoid server-specific assumptions. Wuddle manages legacy WoW clients, not one particular private server.
- Prefer a modular architecture where it improves cohesion, replaceability, testability, and isolation between unrelated features.

## Architecture

Wuddle consists of three independent Rust crates. There is no root Cargo workspace.

### Modularity Direction

Wuddle should remain a modular monolith. Shared application coordination may stay centralized, but substantial features should be implemented behind focused engine and frontend boundaries so they can be repaired, replaced, disabled, or removed without destabilizing unrelated behavior.

- Keep `Engine`, `App`, and `Message` as coordinators rather than eliminating them solely for modularity.
- Avoid continuing to grow broad files such as `lib.rs`, `app/mod.rs`, and `service.rs` when new behavior has a clear feature or domain boundary.
- Prefer focused engine modules for domain, security, compatibility, and filesystem behavior.
- Prefer focused frontend modules for feature state, update handling, service adapters, dialogs, and reusable views where the feature is substantial enough to justify them.
- Extract existing monolithic areas incrementally when related behavior is already being changed. Do not perform risky repository-wide rewrites purely to reduce line counts.
- Modularize around cohesive responsibilities, independent testing, and change isolation—not arbitrary file-size targets or layers of abstraction.
- Keep trivial shared behavior close to its caller when extracting it would only add indirection.

### Crate Structure

```text
wuddle-engine/                 Core Rust library; no UI dependencies
├── Cargo.toml                 Engine features and dependencies
└── src/
    ├── lib.rs                 Engine API, scanning, updates, staging, conflicts, rollback
    ├── db.rs                  SQLite schema and migrations
    ├── model.rs               Repository, release, asset, and installation types
    ├── install.rs             Archive extraction and final file deployment
    ├── direct.rs              Direct archive URL support
    ├── gam_compat.rs          GitAddonsManager discovery and layout compatibility
    ├── diagnostics.rs         Privacy-safe diagnostic event interface
    ├── auto_login.rs          Optional secure auto-login domain and vault logic
    └── forge/
        ├── mod.rs             Forge detection and dispatch
        ├── github.rs          GitHub API support
        ├── gitlab.rs          GitLab API support
        ├── gitea.rs           Gitea and Codeberg API support
        └── git_sync.rs        Git clone, update, branch, and remote handling

wuddle-iced/                   Active native desktop application built with Iced
├── Cargo.toml                 Application version, features, and dependencies
├── assets/                    Fonts, icons, and bundled visual assets
└── src/
    ├── main.rs                Startup, storage, single-window setup, diagnostics, Iced setup
    ├── app/mod.rs             Main application state, subscriptions, and view composition
    ├── message.rs             Application messages and events
    ├── service.rs             Asynchronous bridge to wuddle-engine
    ├── settings.rs            Global preferences and profile metadata
    ├── storage.rs             Platform-specific data placement and migration
    ├── single_instance.rs     Existing-window activation and process coordination
    ├── diagnostics.rs         Logging, sanitization, rotation, and ZIP export
    ├── auto_login.rs          Thin auto-login UI adapter
    ├── tweaks.rs              WoW executable inspection and binary tweaks
    ├── update/                State transitions grouped by feature area
    ├── panels/                Main page views
    ├── dialogs/               Focused dialog views
    ├── components/            Reusable UI components and Quick Add catalog
    └── types/                 Shared frontend state and dialog types

wuddle-launcher/               Stable Windows launcher for versioned portable builds
├── Cargo.toml
└── src/
    └── main.rs                Version selection, activation, and handoff to Wuddle
```

### Engine Boundaries

Core addon, mod, repository, database, installation, compatibility, and security logic belongs in `wuddle-engine`. It must not depend on Iced or other frontend code.

The `auto-login` feature is optional. The engine must continue compiling and working without it.

### Frontend Boundaries

The Iced frontend adapts engine operations into application state, tasks, dialogs, messages, and views. It should not reimplement engine domain logic.

`service.rs` is the asynchronous boundary between Iced and the engine. Blocking engine, filesystem, Git, and credential-vault work must not run on the UI thread.

### Windows Launcher

`wuddle-launcher` is the stable entry point for versioned Windows portable installations and in-app updates. Only change its crate version when the launcher itself changes.

## Data and Profile Isolation

`settings.json` contains application preferences and non-secret profile metadata.

Each game profile has its own SQLite database:

- The default profile uses `wuddle.sqlite`.
- Other profiles use `wuddle-{profile_id}.sqlite`.

A profile must always resolve its own database directly. Never fall back to another profile’s database, even when the expected database is missing or empty.

Profile IDs are persistent identities and must not be inferred from list position or display name. Renaming, deleting, or reordering profiles must not cause data from one profile to appear in another.

Database changes require versioned migrations through `PRAGMA user_version`. Existing installations must remain readable without destructive migration.

## Addon Management Invariants

### GitAddonsManager Compatibility

Wuddle must recognize layouts created by GitAddonsManager, including:

- Immediate Git worktrees inside `Interface/AddOns`.
- Root addons named after their selected `.toc`.
- Modular repositories.
- `.repo` collision worktrees.
- Relative symlinked modules.
- Modules moved out of their worktree.
- Mixed-case directory and `.toc` names.
- `.bak` and `.bak.N` backup folders.
- Non-`origin` upstreams and multiple remotes.
- SSH, SCP-style, local, self-hosted, and otherwise generic Git remotes.
- Worktrees without a usable remote.

Do not rewrite valid GAM layouts merely because Wuddle would have created them differently.

Preserve configured upstreams and existing remotes. Follow the checked-out branch’s upstream first, then use `origin`, then another usable remote only when necessary.

GAM backup folders are not active addons. Ignore them during automatic import, but never delete them automatically.

### Staging and Conflict Safety

New addon-git installations must be cloned and prepared in staging before modifying `Interface/AddOns`.

Do not place a new worktree, module, `.repo` directory, or database entry into its final active state until:

1. The repository has been downloaded successfully.
2. Its addon structure has been detected.
3. Any `.toc`, collection, or release-asset choices have been resolved.
4. Conflicts have been shown to the user.
5. The user has accepted the installation.

Cancelling a conflict must leave neither installed files nor a tracked repository behind.

Reinstall/Repair should perform a fresh staged installation and replace stale managed files only after the replacement is ready.

### Collections and Multi-TOC Addons

Collections are a Wuddle enhancement. Users may install and maintain only a selected subset of a repository’s addons.

Replacing a conflicting addon from a collection must remove or deselect only the conflicting addon folders, not the entire collection.

Repositories containing multiple candidate root `.toc` files must ask the user which one defines the primary addon. The selected `.toc` may determine the final worktree folder name.

Where enhanced Wuddle behavior cannot be represented by GAM, correctness and user intent take priority over producing a perfectly GAM-reversible layout.

## Mod and Source Handling

Wuddle supports:

- Release-based mods from GitHub, GitLab, Gitea, and Codeberg.
- Generic Git repositories and arbitrary Git hosts.
- Direct `.zip` and `.7z` archive URLs.
- Local addon archives.
- Git-based single addons, modular addons, and collections.

Do not guess that an unknown Git host is GitLab or Gitea. Represent it as generic Git while preserving its full namespace and clone URL.

Repository owner and project casing should be preserved for display. Use normalized values only for identity comparison and deduplication.

Quick Add is a curated, client-aware catalog. Presets should declare the client families they support and should not appear for incompatible or unknown clients.

## Credentials and Privacy

GitHub tokens and auto-login passwords must be stored through the operating system’s credential vault:

- Linux: Secret Service.
- Windows: Windows Credential Manager.

There must be no plaintext fallback.

`settings.json` may contain account IDs, labels, and selected-account metadata, but never passwords, tokens, encrypted credential blobs, or ordinary credential strings.

Secure auto-login:

- Is currently supported only for WoW 3.3.5 clients.
- Depends on Awesome WotLK’s command-line login support; Wuddle does not implement login inside the game client itself.
- Is optional and feature-gated.
- Supports Auto, Wine, and Custom launch methods.
- Does not support credentialed Lutris launches.
- Must use separate process arguments and must never invoke a shell.
- Must not log launch arguments or include credentials in errors.
- Must load credentials only when preparing a launch.
- Must zeroize owned secret argument data where practical.

Deleting an account or profile must remove its matching vault credentials before deleting metadata. Vault cleanup failures must stop the metadata deletion so the operation remains retryable.

## Diagnostics

Verbose diagnostics are intended for users to enable, reproduce a problem, and export a ZIP for a GitHub issue.

Diagnostic events should record:

- Operation names.
- Safe identifiers.
- Counts and decisions.
- Timing.
- Success, cancellation, rollback, and error categories.

Diagnostic events must not record:

- Credentials or account details.
- GitHub tokens or authorization headers.
- Process command arguments.
- Raw settings.
- Database contents.
- Game paths, archive paths, home-directory paths, or other identifying filesystem values.
- Unfiltered remote responses that may contain private data.

All exported diagnostic files must be sanitized again during export, even if they were sanitized when first written.

## Iced Conventions

- Keep `App` state in `app/mod.rs`.
- Add messages to `message.rs`.
- Put state transitions in the appropriate `update/` module.
- Keep reusable domain-specific state in `types/` or a focused feature module.
- Keep panel and dialog functions primarily concerned with rendering.
- Run blocking engine, filesystem, Git, and credential operations away from the UI thread through `service.rs`.
- Use `Task<Message>` for asynchronous UI operations.
- New settings fields require safe defaults through `#[serde(default)]`.
- Reuse shared theme, tooltip, button, and dialog helpers.
- Provide visible feedback for success, cancellation, and failure; avoid silent no-ops.
- Protect destructive or security-sensitive dialogs from accidental scrim dismissal.
- User-facing text must say “Wuddle,” never internal crate names.

## Cross-Platform Rules

- Preserve both symlink and real-folder addon layouts.
- On Unix, prefer relative addon-module links when appropriate.
- On Windows, support junctions and real-folder fallbacks.
- Treat links as links during deletion; never recursively delete through a link target.
- Do not use `canonicalize()` where resolving a user’s symlinked WoW directory would change its intended identity.
- Do not assume executable filename casing.
- Never require a console window for normal Windows operation.
- Keep Linux Wayland/X11 differences in mind, particularly file drag-and-drop behavior.

## Change Discipline

- Preserve unrelated local changes in a dirty worktree.
- Keep edits focused and avoid repository-wide formatting churn.
- Do not perform destructive Git operations without explicit approval.
- Do not add dependencies unless the benefit justifies the maintenance and platform cost.
- Add regression tests for bugs involving data loss, profile isolation, filesystem deployment, conflict handling, security, or compatibility.
- Prefer focused modules over adding unrelated behavior to `lib.rs`, `app/mod.rs`, or `service.rs`.
- Do not hard-code temporary version numbers or release status into this document.

## Validation

There is no root workspace, so run commands with explicit manifests.

Typical validation:

```sh
cargo test --manifest-path wuddle-engine/Cargo.toml
cargo test --manifest-path wuddle-engine/Cargo.toml --features auto-login
cargo test --manifest-path wuddle-iced/Cargo.toml
cargo check --manifest-path wuddle-iced/Cargo.toml --no-default-features
git diff --check
```

Run platform-specific tests where relevant. The single-instance frontend test may require permission to create a local IPC listener.

The no-default-features build is an architectural check: Wuddle must retain normal manual-login behavior without compiling the auto-login capability.

## Release Process

Stable releases are built from `main`. Prerelease tags may be built from `beta`.

For an Iced release:

1. Update the version in `wuddle-iced/Cargo.toml`.
2. Update `wuddle-iced/Cargo.lock` through a normal Cargo build or check.
3. Update the matching release section in `CHANGELOG.md`. Each beta release gets its own exact prerelease heading and lists only the changes introduced by that beta. Merge those prerelease notes into one stable section when the version is promoted to `main`.
4. Update the `What's New in vX.Y.Z` section in `README.md` to describe the new release.
5. Move the previous top-level README release notes into the collapsed `v3.x Changelog` section, following the existing newest-first format.
6. Run the engine and frontend validation appropriate to the changes.
7. Commit only the intended files.
8. Create a `vX.Y.Z` or prerelease `vX.Y.Z-beta.N` tag.
9. Push the correct branch and tag.

`.github/workflows/iced-release.yml` builds Linux AppImage/tar.gz and Windows portable artifacts from eligible tags, then publishes the GitHub release.

The workflow extracts release notes from the exact tag heading first—for example, `v3.7.0-beta.2` uses `## v3.7.0-beta.2`—and falls back to the stable version heading for older changelog entries.
