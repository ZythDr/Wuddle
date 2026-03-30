# Wuddle - AI Development Context

> This file provides context for AI assistants working on the Wuddle codebase.
> It documents architecture, conventions, common pitfalls, and priorities.

## Project Overview

Wuddle is a **desktop WoW (World of Warcraft) launcher and manager** primarily targeting Vanilla/classic WoW clients (especially Turtle WoW). It provides:

- **DLL mod management** — install, update, repair, and remove binary mods from GitHub, GitLab, and Gitea/Codeberg releases
- **Git-based addon management** — clone/pull addon repos with branch selection (inspired by [GitAddonsManager](https://gitlab.com/woblight/GitAddonsManager))
- **Multi-instance profiles** — each profile tracks its own mods/addons, launch config, and WoW directory
- **One-click game launch** — per-instance launch methods (Auto, Lutris, Wine, Custom)
- **Quick Add catalog** — curated mod presets with metadata and companion addon links
- **WoW.exe patching (Tweaks tab)** — binary patches for FoV, farclip, quickloot, camera fixes, etc.
- **In-game radio** — streams the Everlook Broadcasting Co. radio with volume control, auto-connect, auto-play, and configurable buffer
- **DXVK Configurator** — GUI for editing `dxvk.conf` with Turtle WoW-specific presets

## Frontends

Wuddle has **two frontends** sharing the same engine and database:

| | **Iced (v3)** | **Tauri (v2)** |
|---|---|---|
| Status | **Active development** | Maintenance only |
| Branch | `Wuddle-Iced-Dev` | `main` |
| UI framework | [iced](https://iced.rs) 0.14 (pure Rust) | Tauri v2 (Rust + HTML/CSS/JS) |
| Current version | `3.0.0-beta.7` | `2.5.8` |
| Crate | `wuddle-iced/` | `wuddle-gui/` |

**The Iced frontend is the primary target for new work.** The Tauri version is retained for reference and backward compatibility.

## Architecture

### Crate Structure

```
wuddle-engine/         Rust library crate — core logic, no UI dependency
  src/
    lib.rs             Engine struct, public API (add_repo, check_updates, install, etc.)
    db.rs              SQLite database (repos, installs, rate_limits tables)
    model.rs           Data types: Repo, InstallMode, AddonProbeResult, etc.
    install.rs         Asset download, extraction, DLL/addon installation
    util.rs            Shared helpers
    forge/             Per-forge API implementations
      mod.rs           detect_repo(), URL parsing, forge dispatch
      github.rs        GitHub Releases API
      gitlab.rs        GitLab Releases/Packages API
      gitea.rs         Gitea/Codeberg Releases API
      git_sync.rs      Git clone/pull for addon_git mode

wuddle-iced/           Iced 0.14 desktop app (ACTIVE)
  src/
    main.rs            App struct, Message enum, update(), view(), all state
    service.rs         Async wrappers around wuddle-engine (spawns blocking tasks)
    settings.rs        AppSettings struct, JSON persistence
    theme.rs           ThemeColors, button/container/tooltip style helpers
    radio.rs           Radio streaming (rodio + symphonia + reqwest::blocking)
    tweaks.rs          WoW.exe binary patching UI logic
    anchored_overlay.rs  Custom overlay widget for dropdown menus
    panels/            Tab content views (pure functions returning Element)
      home.rs          Home tab — radio card, update overview, launch button
      projects.rs      Mods & Addons tabs — repo list, detail dialogs
      options.rs       Options tab — settings checkboxes/dropdowns
      logs.rs          Logs tab — scrollable log viewer
      about.rs         About tab — version info, update button, links
      tweaks.rs        Tweaks tab — binary patch toggles
      dxvk_config.rs   DXVK Configurator panel
      radio.rs         Radio Settings dialog view
  icons/               SVG icons (cogwheel, chevrons, file, forge logos)
  fonts/               LifeCraft title font, Friz Quadrata body font

wuddle-gui/            Tauri v2 desktop app (MAINTENANCE)
  src-tauri/
    src/lib.rs         All #[tauri::command] handlers
    src/self_update.rs In-app self-update logic
    src/tweaks.rs      WoW.exe binary patching
  src/                 Frontend (vanilla JS, served directly, NO bundler)

wuddle-launcher/       Thin Windows launcher binary (version switcher for self-update)
```

### Key Architectural Decisions (Iced)

1. **Single `main.rs` monolith**: All app state lives in the `App` struct, all messages in the `Message` enum, all logic in `update()`. Panel views are delegated to `panels/*.rs` as pure functions.

2. **`service.rs` async bridge**: Engine operations run via `tokio::task::spawn_blocking()` because `Engine` (holding `rusqlite::Connection`) is `Send` but not `Sync`. Service functions return `Task<Message>`.

3. **Dialog pattern**: Modal dialogs use a `Dialog` enum variant stored in `App.dialog`. The scrim is a `mouse_area` that catches clicks outside. Panels render via `view_dialog()` matching on the variant. Close buttons use red ✕ styling; action buttons are right-aligned at the bottom.

4. **Theme system**: `ThemeColors` struct holds all colors for the current theme. Colors are passed by value to panel view functions (not via global state). Style closures capture copied `ThemeColors` or individual `Color` values.

5. **SVG icons**: Use `iced::widget::svg::Handle::from_memory(include_bytes!("../../icons/foo.svg"))` pattern. SVGs should use `stroke="currentColor"` or `fill="currentColor"` so the iced `svg::Style { color: Some(...) }` override works across themes.

6. **Desktop notifications**: Use `notify-rust` crate, gated on `opt_desktop_notify` setting.

7. **Settings persistence**: `settings.rs` defines `AppSettings` with `#[serde(default)]` on the struct — new fields automatically get defaults when loading old `settings.json` files.

8. **Radio streaming**: `radio.rs` uses `rodio` + `symphonia` for playback, with `reqwest::blocking` for the HTTP stream. The audio device (`rodio::OutputStream`) is `!Send` and must be created on its owning thread.

### Key Architectural Decisions (Tauri — for reference)

1. **No frontend bundler**: `tauri.conf.json` sets `"frontendDist": "../src"` — `src/` is served directly. Libraries must be pre-bundled into `src/vendor/`.
2. **`withGlobalTauri: true`**: Tauri core API available as `window.__TAURI__`.
3. **Shared HTTP client**: Single `reqwest::blocking::Client` via `OnceLock` across all Tauri commands.

### Shared (Both Frontends)

- **SQLite migrations**: `db.rs` uses `PRAGMA user_version` for versioned schema migrations (currently at v7)
- **Case-insensitive repo dedup**: The unique index uses `COLLATE NOCASE`; owner/name preserve original casing from the forge
- **GAM-compatible addon deployment**: Addons are cloned directly into `Interface/AddOns/{name}/` with `.git` inside the addon folder, matching GitAddonsManager and the TurtleWoW launcher layout
- **Mod cache**: Release asset downloads cached in `{wow_dir}/.wuddle/cache/`

## Conventions and Style

### Rust (General)
- Error handling: return `Result<T, String>` at UI boundaries — use `.map_err(|e| e.to_string())?`
- Forge API calls always include `User-Agent: Wuddle/{version}` header
- GitHub API calls attach bearer auth token when available (`wuddle_engine::github_token()`)

### Rust (Iced-specific)
- Panel view functions are pure: `fn view<'a>(..., colors: &ThemeColors) -> Element<'a, Message>`
- Button styles use `theme::tab_button_style`, `theme::tab_button_active_style`, `theme::tab_button_hovered_style` closures
- Tooltips: use `tip()` helper (wraps `tooltip` + `container` with consistent styling)
- Checkbox API (iced 0.14): `checkbox(bool_value).label("...").on_toggle(Message::Foo)` — takes 1 arg, not 2
- `rule::horizontal(1)` is a free function, not `Rule::horizontal`
- Color copying: `ThemeColors` is `Copy`, so `let c = *colors;` then use `c` in closures

### JavaScript (Tauri frontend only)
- No framework — plain vanilla JS with ES module imports
- DOM querying: `$("id")` helper (alias for `document.getElementById`)
- All Tauri calls go through `safeInvoke(command, args)` from `commands.js`

### Naming
- **User-facing**: Always "Wuddle" — never "wuddle-gui", "wuddle-iced", or "wuddle-engine"
- Cargo package names are internal only

## Common Pitfalls

1. **Don't use `canonicalize()` for path validation** — it resolves symlinks, breaking the path check when the WoW directory is a symlink. Use simple `..` string guards instead.

2. **Don't lowercase owner/name for display** — `detect_repo()` preserves original casing for `owner` and `name`. Only `canonical_url` and `project_path` should be lowercased (for dedup/matching).

3. **Rate limiting**: GitHub API has rate limits (60/hr unauthenticated). The engine tracks rate-limit resets in the `rate_limits` table. Always support and encourage GitHub token auth.

4. **`rodio::OutputStream` is `!Send`** — contains raw pointers from cpal. Must be created on the thread that will own it; cannot be moved between threads.

5. **iced `Frames` (iced_gif) is not `Clone`** — wrap in `Arc<Frames>` when storing in structs that derive `Clone`.

6. **Don't use npm imports at runtime** (Tauri only) — the Tauri frontend has no bundler. Pre-bundle into `src/vendor/`.

7. **Don't use the Tauri notification plugin** (Tauri only) — doesn't work with the no-bundler setup. Use the Browser Notification API.

## What to Prioritize

- **User experience**: Wuddle should feel fast, responsive, and self-explanatory. Toast messages for feedback, no silent failures
- **Correctness over cleverness**: Simple, readable code. Avoid over-abstraction
- **Cross-forge consistency**: Features should work across GitHub, GitLab, and Gitea/Codeberg
- **Linux-first but cross-platform**: Primary target is Linux (AppImage). Windows support is secondary but maintained and equally important
- **Minimal dependencies**: Don't add Rust crates unless clearly necessary
- **Backward compatibility**: DB migrations must handle upgrades from any previous version gracefully

## What to Avoid

- **Over-engineering**: Don't add features, abstractions, or error handling beyond what's needed
- **Breaking existing DB data**: Always add migrations, never assume a clean database
- **Exposing internal names to users**: Users see "Wuddle", not crate names
- **Force-pushing or destructive git operations** without explicit user approval

## Release Process

### Iced (v3) — active
1. Bump version in `wuddle-iced/Cargo.toml`
2. Update `CHANGELOG.md` with a new version section
3. Run `cargo build` to verify + regenerate `Cargo.lock`
4. Commit, tag (`vX.Y.Z`), push with tags to `Wuddle-Iced-Dev`
5. CI (`iced-release.yml`) builds Linux AppImage + tar.gz and Windows portable zip
6. CI creates a GitHub pre-release with artifacts and changelog extract

### Tauri (v2) — maintenance
1. Bump version in: `wuddle-gui/package.json`, `wuddle-gui/src-tauri/Cargo.toml`, `wuddle-gui/src-tauri/tauri.conf.json`
2. Tag, push to `main`
3. CI (`release-build.yml`) builds artifacts

## Current State

- **Active frontend**: Iced v3 (`wuddle-iced/`, version `3.0.0-beta.7`)
- **Engine**: `wuddle-engine/`, shared by both frontends
- **SQLite schema**: v7
- **Iced framework**: 0.14 with features: markdown, image, svg, tokio, canvas, advanced
- **Additional iced crates**: `iced_gif` 0.14 (GIF animation in README previews)
- **Supported forges**: GitHub, GitLab, Gitea/Codeberg
- **Install modes**: `release` (DLL mods via GitHub/GitLab/Gitea releases), `addon_git` (git clone/pull)
- **Themes**: 5 themes (Cata, Obsidian, Emerald, Ashen, WoWUI)
- **Radio**: Everlook Broadcasting Co. stream via rodio + symphonia
