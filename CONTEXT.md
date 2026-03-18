# Wuddle - AI Development Context

> This file provides context for AI assistants working on the Wuddle codebase.
> It documents architecture, conventions, common pitfalls, and priorities.

## Project Overview

Wuddle is a **desktop WoW (World of Warcraft) launcher and manager** built with **Tauri v2** (Rust backend + HTML/CSS/JS frontend). It primarily targets Vanilla/classic WoW clients (especially Turtle WoW) and provides:

- **DLL mod management** — install, update, repair, and remove binary mods from GitHub, GitLab, and Gitea/Codeberg releases
- **Git-based addon management** — clone/pull addon repos with branch selection (inspired by [GitAddonsManager](https://gitlab.com/woblight/GitAddonsManager))
- **Multi-instance profiles** — each profile tracks its own mods/addons, launch config, and WoW directory
- **One-click game launch** — per-instance launch methods (Auto, Lutris, Wine, Custom)
- **Quick Add catalog** — curated mod presets with metadata and companion addon links
- **WoW.exe patching (Tweaks tab)** — binary patches for FoV, farclip, quickloot, camera fixes, etc.

## Architecture

### Crate Structure

```
wuddle-engine/     Rust library crate — core logic, no UI dependency
  src/
    lib.rs         Engine struct, public API (add_repo, check_updates, install, etc.)
    db.rs          SQLite database (repos, installs, rate_limits tables)
    model.rs       Data types: Repo, InstallMode, AddonProbeResult, etc.
    install.rs     Asset download, extraction, DLL/addon installation
    util.rs        Shared helpers
    forge/         Per-forge API implementations
      mod.rs       detect_repo(), URL parsing, forge dispatch
      github.rs    GitHub Releases API
      gitlab.rs    GitLab Releases/Packages API
      gitea.rs     Gitea/Codeberg Releases API
      git_sync.rs  Git clone/pull for addon_git mode

wuddle-gui/        Tauri v2 desktop app
  src-tauri/
    src/lib.rs     All #[tauri::command] handlers, shared HTTP client, app setup
    src/self_update.rs  In-app self-update logic (AppImage replacement)
    src/tweaks.rs  WoW.exe binary patching
    Cargo.toml     Rust dependencies
    tauri.conf.json  Tauri config (productName, identifier, window settings)
    capabilities/default.json  Permission grants for frontend
  src/             Frontend (served directly, NO bundler in production)
    index.html     Single-page app shell with all dialog markup
    main.js        App initialization, settings, tab switching, event wiring
    repos.js       Repo management: list, add, update, detail dialog, file preview
    profiles.js    Multi-instance profile management
    home.js        Home tab with update overview and launcher
    presets.js     Quick Add catalog data and rendering
    turtle.js      Turtle WoW-specific home section links
    tweaks.js      Tweaks tab UI
    about.js       About page
    auth.js        GitHub token management
    logs.js        Logs panel
    state.js       Shared reactive state, localStorage keys, defaults
    commands.js    Tauri invoke wrapper (safeInvoke)
    ui.js          Dialog helpers, toast system, scroll fading, theme management
    utils.js       Shared frontend utilities
    highlight.js   highlight.js source module (bundled separately)
    vendor/highlight.bundle.js  Pre-bundled highlight.js (81KB ES module)
    styles.css     All CSS (single file)

wuddle-launcher/   Thin launcher binary (launches wuddle-gui, used for self-update)
```

### Key Architectural Decisions

1. **No frontend bundler in production**: `tauri.conf.json` sets `"frontendDist": "../src"` — the `src/` directory is served directly. This means:
   - No `node_modules` access at runtime
   - ES module imports must reference local files (not npm packages)
   - Any npm library needing runtime use must be **pre-bundled** into `src/vendor/` (e.g., highlight.js)
   - Use `<script type="module">` imports between local `.js` files

2. **`withGlobalTauri: true`**: The Tauri core API is available as `window.__TAURI__` (used for `invoke` via `commands.js`)

3. **Shared HTTP client**: A single `reqwest::blocking::Client` via `OnceLock` is reused across all Tauri commands to avoid repeated TLS setup

4. **`run_blocking` pattern**: All `#[tauri::command] async fn` handlers wrap their logic in `tauri::async_runtime::spawn_blocking()` because the `Engine` (holding `rusqlite::Connection`) is `Send` but not `Sync`

5. **Desktop notifications**: Use the **Browser Notification API** (`new Notification(...)`) — NOT the Tauri notification plugin (it doesn't work with `withGlobalTauri` and no frontend bundler)

6. **SQLite migrations**: `db.rs` uses `PRAGMA user_version` for versioned schema migrations (currently at v6)

7. **Case-insensitive repo dedup**: The unique index uses `COLLATE NOCASE`; owner/name preserve original casing from the forge but comparisons are case-insensitive

## Conventions and Style

### Rust
- Tauri commands use `#[allow(non_snake_case)]` when accepting `camelCase` parameters from JS (e.g., `wowDir`)
- Error handling: commands return `Result<T, String>` — use `.map_err(|e| e.to_string())?` or `.map_err(|e| format!(...))?`
- Forge API calls always include `User-Agent: Wuddle/{version}` header
- GitHub API calls attach bearer auth token when available (`wuddle_engine::github_token()`)

### JavaScript
- No framework — plain vanilla JS with ES module imports
- DOM querying: `$("id")` helper (alias for `document.getElementById`)
- All Tauri calls go through `safeInvoke(command, args)` from `commands.js`
- Generation-based abort pattern: `_detailAbortKey` increments to cancel stale async results
- LRU-capped `Map` caches with `cappedSet()` (30 entries max)
- `requestAnimationFrame` throttling for scroll event handlers
- State is in `state.js` — exported mutable variables shared across modules

### CSS
- Single `styles.css` file, no preprocessor
- CSS custom properties for theming: `--bg`, `--text`, `--accent`, `--project-link`, etc.
- Theme definitions in `:root` and `[data-theme="..."]` selectors
- Scroll-aware edge fading with `::after` pseudo-elements and CSS gradients

### Naming
- **User-facing**: Always "Wuddle" — never "wuddle-gui" or "wuddle-engine"
- Cargo package names (`wuddle-gui`, `wuddle-engine`) are internal only
- `productName: "Wuddle"` in tauri.conf.json controls the release binary name

## Common Pitfalls

1. **Don't use npm imports at runtime** — the app has no bundler. If you need a library, bundle it into `src/vendor/` first (see `highlight.js` → `highlight.bundle.js` as an example)

2. **Don't use `canonicalize()` for path validation** — it resolves symlinks, breaking the path check when the WoW directory is a symlink. Use simple `..` string guards instead (see `wuddle_read_local_file` and `wuddle_list_local_files`)

3. **Don't lowercase owner/name for display** — `detect_repo()` preserves original casing for `owner` and `name`. Only `canonical_url` and `project_path` should be lowercased (for dedup/matching)

4. **Don't use the Tauri notification plugin** — it doesn't work with the no-bundler setup. Use the Browser Notification API instead

5. **When cloning DOM subtrees** (e.g., file tree caching), event listeners are lost — use `wireTreeFileClicks()` or similar re-wiring functions after `cloneNode(true)`

6. **Rate limiting**: GitHub API has rate limits (60/hr unauthenticated). The engine tracks rate-limit resets in the `rate_limits` table. Always support and encourage GitHub token auth

7. **Don't add `use_frameworks`-style Tauri plugins** without confirming they work with direct `src/` serving and `withGlobalTauri: true`

## What to Prioritize

- **User experience**: Wuddle should feel fast, responsive, and self-explanatory. Toast messages for feedback, no silent failures
- **Correctness over cleverness**: Simple, readable code. Avoid over-abstraction
- **Cross-forge consistency**: Features should work across GitHub, GitLab, and Gitea/Codeberg
- **Linux-first but cross-platform**: Primary target is Linux (AppImage). Windows support is secondary but maintained and equally important.
- **Minimal dependencies**: Don't add npm packages or Rust crates unless clearly necessary
- **Backward compatibility**: DB migrations must handle upgrades from any previous version gracefully

## What to Avoid

- **Over-engineering**: Don't add features, abstractions, or error handling beyond what's needed
- **Breaking existing DB data**: Always add migrations, never assume a clean database
- **Exposing internal names to users**: "wuddle-gui" and "wuddle-engine" are internal. Users see "Wuddle"
- **Unnecessary bundler/framework additions**: The no-bundler architecture is intentional and should be preserved
- **Force-pushing or destructive git operations** without explicit user approval

## Release Process

1. Bump version in: `wuddle-gui/package.json`, `wuddle-gui/src-tauri/Cargo.toml`, `wuddle-gui/src-tauri/tauri.conf.json`
2. Update `CHANGELOG.md` with a new version section at the top
3. Update `README.md` "What's New" section (move previous version into `<details>` collapsible)
4. Run `cargo check` to regenerate `Cargo.lock`
5. Commit, tag (`vX.Y.Z`), push with tags
6. Create GitHub release with categorized release notes
7. CI builds AppImage and other artifacts automatically

## Current State (v2.5.3+)

- SQLite schema version: 6
- Tauri v2 with plugins: opener, dialog
- Frontend: vanilla ES modules served from `src/`
- Supported forges: GitHub, GitLab, Gitea/Codeberg
- Install modes: `release` (DLL mods via releases), `addon_git` (git clone/pull)
