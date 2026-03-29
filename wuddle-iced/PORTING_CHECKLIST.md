# Wuddle Iced — Porting Checklist

Cross-reference of every Tauri/Svelte feature vs. the Iced 0.14 implementation.

**Legend:** ✅ Done | 🔶 Partial | ❌ Missing | 🚫 N/A (platform/infra difference)

---

## Core Shell

| Feature | Tauri | Iced | Notes |
|---------|-------|------|-------|
| Window (1100×850) | ✅ | ✅ | |
| 5 themes (Cata, Obsidian, Emerald, Ashen, WoWUI) | ✅ | ✅ | |
| LifeCraft title font | ✅ | ✅ | |
| Friz Quadrata body font toggle | ✅ | ✅ | requires restart; logs message |
| 12-hour clock timestamps | ✅ | 🔶 | applied to all new log entries; initial startup entries always 24h (settings not yet loaded at that point) |
| Spinner/busy indicator | ✅ | ✅ | canvas-based, 80 ms tick |
| Footer PLAY button | ✅ | ✅ | |
| Footer status hint | ✅ | ✅ | |
| Escape closes dialogs | ✅ | ✅ | subscription-gated |
| Click outside dialog closes it | ✅ | ✅ | scrim mouse_area |
| Click-through prevention | ✅ | ✅ | `iced::widget::opaque()` |
| Desktop notifications | ✅ | ✅ | `notify-rust`, gated on `opt_desktop_notify` |
| Settings persistence (JSON) | ✅ | ✅ | `settings.rs` |
| Open URL in system browser | ✅ | ✅ | `open::that` |
| Open directory in file manager | ✅ | ✅ | `open::that` |
| Copy to clipboard | ✅ | ✅ | `arboard` |

---

## Topbar & Navigation

| Feature | Tauri | Iced | Notes |
|---------|-------|------|-------|
| Home / Mods / Addons / Tweaks tabs | ✅ | ✅ | |
| Options / Logs / About icon tabs | ✅ | ✅ | |
| Update counts on Mods/Addons tabs | ✅ | ✅ | `tab_label()` with live counts |
| Multi-profile picker in topbar | ✅ | ✅ | `pick_list` when >1 profile |

---

## Home Tab

| Feature | Tauri | Iced | Notes |
|---------|-------|------|-------|
| Quick-update summary (repos with updates) | ✅ | 🔶 | Iced shows repo table; Tauri has a compact summary widget on Home |
| 12 Turtle WoW resource links | ✅ | ✅ | all URLs wired |
| Update count displays | ✅ | ✅ | |

---

## Mods / Addons Tabs (Repository Table)

| Feature | Tauri | Iced | Notes |
|---------|-------|------|-------|
| List repos with name, version, status | ✅ | ✅ | |
| Enable/disable checkbox per repo | ✅ | ✅ | |
| Search / filter input with clear (✕) | ✅ | ✅ | |
| Filter: All / Updates / Errors | ✅ | ✅ | |
| Filter: Ignored | ✅ | ✅ | Tauri allows marking updates as ignored; Iced has no Ignored state |
| Sort by name / status (ascending/descending) | ✅ | ✅ | |
| "Check for updates" button | ✅ | ✅ | |
| "Update all" button | ✅ | ✅ | |
| "Refresh / Rescan" button | ✅ | ✅ | `RefreshRepos` |
| Last-checked timestamp | ✅ | ✅ | stored in `last_checked` |
| Per-repo context menu | ✅ | ✅ | |
| Context menu → Update | ✅ | ✅ | |
| Context menu → Reinstall/Repair | ✅ | ✅ | |
| Context menu → Branch selector | ✅ | ✅ | `pick_list` in context menu |
| Context menu → Enable/Disable | ✅ | ✅ | |
| Context menu → Remove | ✅ | ✅ | |
| Context menu → View Details (full dialog) | ✅ | ❌ | Tauri has a separate detail dialog per repo (README, releases, file tree); Iced only shows this in the Add dialog |
| Addon conflict detection (probe before add) | ✅ | ❌ | `wuddle_probe_addon_repo` not ported |
| SuperWoW AV risk acknowledgment dialog | ✅ | ❌ | |
| Mark update as ignored per-repo | ✅ | ✅ | context menu "Ignore Updates" / "Unignore Updates" |
| Scroll fade at table edges | ✅ | 🔶 | iced scrollable has no built-in fade; omitted |

---

## Add Repository Dialog

| Feature | Tauri | Iced | Notes |
|---------|-------|------|-------|
| URL input with clear button (✕) | ✅ | ✅ | |
| Quick Add preset list (Mods only) | ✅ | ✅ | all 8 presets |
| Preset cards with tags (Recommended, AV, category) | ✅ | ✅ | |
| Companion addon links in preset cards | ✅ | ✅ | |
| "Add" / "Installed" action per preset | ✅ | ✅ | |
| Preview fetch on URL input | ✅ | ✅ | debounced, GitHub/GitLab/Gitea |
| Two-card layout (sidebar + form) when preview loaded | ✅ | ✅ | |
| Sidebar: repo name, description, stars, forks, language, license | ✅ | ✅ | |
| Sidebar: file tree with 📁/📄 icons | ✅ | ✅ | sorted dirs first |
| README rendering with markdown | ✅ | ✅ | `iced::widget::markdown` |
| README images (inline + block) | ✅ | ✅ | fixed: scans raw text for `![alt](url)` |
| Forge link button with per-forge SVG icon | ✅ | ✅ | GitHub/GitLab + generic code icon |
| Release Notes (in-app, not browser) | ✅ | ✅ | fetches API, renders release list |
| README ↔ Release Notes toggle | ✅ | ✅ | |
| Advanced mode toggle (pick install mode) | ✅ | ✅ | |
| GIF playback in README | ✅ | 🚫 | iced 0.14 does not support animated GIFs; first frame only |
| Addon conflict warning before adding | ✅ | ❌ | |

---

## Tweaks Tab

| Feature | Tauri | Iced | Notes |
|---------|-------|------|-------|
| FoV slider | ✅ | ✅ | |
| Farclip slider | ✅ | ✅ | |
| Frill distance slider | ✅ | ✅ | |
| Nameplate distance slider | ✅ | ✅ | |
| Max camera distance input | ✅ | ✅ | |
| Sound channels input | ✅ | ✅ | |
| Background sound toggle | ✅ | ✅ | |
| Quickloot toggle | ✅ | ✅ | |
| Large Address Aware toggle | ✅ | ✅ | |
| Camera skip fix toggle | ✅ | ✅ | |
| Read Current Values | ✅ | ✅ | populates sliders |
| Apply Tweaks | ✅ | ✅ | always starts from .bak |
| Restore Backup | ✅ | ✅ | |
| Reset to Defaults | ✅ | ✅ | |
| Backup status indicator (has backup?) | ✅ | ✅ | shown in hint line below header |
| Disable all controls when no WoW dir | ✅ | ✅ | |
| FOV degree display (radians → degrees label) | ✅ | ✅ | shows `{radians:.2} ({degrees:.0}°)` |

---

## Options Tab

| Feature | Tauri | Iced | Notes |
|---------|-------|------|-------|
| Instance list + add/edit/remove | ✅ | ✅ | |
| Instance Settings dialog (name, WoW dir, launch method) | ✅ | ✅ | |
| Browse WoW dir (native file dialog) | ✅ | ✅ | `rfd` |
| Open WoW dir in file manager | ✅ | ✅ | |
| Launch method: Auto / Lutris / Wine / Custom | ✅ | ✅ | |
| Like Turtles checkbox | ✅ | ✅ | |
| Auto-clear WDB cache checkbox | ✅ | ✅ | |
| GitHub token entry + Save/Forget | ✅ | ✅ | OS keychain with file fallback |
| GitHub Tokens web link | ✅ | ✅ | |
| GitHub auth health status display | ✅ | ✅ | Tauri shows rate-limit/keychain health badges; Iced does not |
| Auto-check toggle | ✅ | ✅ | |
| Auto-check interval (minutes) | ✅ | ✅ | stored in settings |
| Desktop notifications toggle | ✅ | ✅ | |
| Use symlinks toggle | ✅ | ✅ | |
| 12-hour clock toggle | ✅ | 🔶 | applied to new log entries; initial startup entries use 24h |
| Theme picker | ✅ | ✅ | |
| Font picker (Friz Quadrata) | ✅ | ✅ | |
| DMA-BUF toggle (Linux) | ✅ | ❌ | Linux-only GPU passthrough option |

---

## Logs Tab

| Feature | Tauri | Iced | Notes |
|---------|-------|------|-------|
| Log lines with level + timestamp | ✅ | ✅ | |
| Filter: All / Info / Errors | ✅ | ✅ | |
| Text search | ✅ | ✅ | |
| Line wrap toggle | ✅ | ✅ | |
| Auto-scroll toggle | ✅ | ✅ | |
| Copy log to clipboard | ✅ | ✅ | |
| Clear log | ✅ | ✅ | |

---

## About Tab

| Feature | Tauri | Iced | Notes |
|---------|-------|------|-------|
| App version display | ✅ | ✅ | |
| Check for Wuddle updates | ✅ | ✅ | GitHub API |
| Latest version display | ✅ | ✅ | |
| Open Wuddle on GitHub link | ✅ | ✅ | |
| Changelog link | ✅ | ✅ | |
| Wuddle changelog viewer (in-app markdown) | ✅ | ✅ | Dialog::Changelog — fetches remote, falls back to embedded; renders markdown |
| Self-update apply (download + stage) | ✅ | ✅ | Linux AppImage + Windows portable launcher layout |
| Self-update restart | ✅ | ✅ | re-exec on Linux, launcher restart on Windows |

---

## Auto-Check & Background Operations

| Feature | Tauri | Iced | Notes |
|---------|-------|------|-------|
| Auto-check on launch (if enabled) | ✅ | ✅ | fires after repos loaded |
| Periodic auto-check timer | ✅ | ✅ | `iced::time::every` subscription |
| Notification on updates found | ✅ | ✅ | |
| Wuddle self-update poll (60 min) | ✅ | ✅ | hourly `iced::time::every(3600s)` subscription fires `CheckSelfUpdate` |

---

## Profile Management

| Feature | Tauri | Iced | Notes |
|---------|-------|------|-------|
| Multiple profiles (WoW installs) | ✅ | ✅ | |
| Per-profile SQLite database | ✅ | ✅ | |
| Switch profile without restart | ✅ | ✅ | |
| Remove profile (deletes DB) | ✅ | ✅ | |
| Per-profile launch config | ✅ | ✅ | stored in ProfileConfig |
| Per-profile cached update plans | ✅ | ✅ | `cached_plans` HashMap |
| Profile picker in topbar | ✅ | ✅ | |

---

## Priority Summary

### High Priority (visible feature gaps)
1. **GitHub auth health status** — rate-limit info and keychain health feedback in Options (Tauri shows detailed badges).
2. **Per-repo "View Details" dialog** — open a full detail dialog (same 2-card layout) for already-tracked repos via the context menu.

### Medium Priority
3. **Addon conflict warning** — probe before adding, show conflicting repos.
4. **12-hour clock for startup entries** — the two initial log lines at startup always use 24h format because settings haven't loaded yet; cosmetic only.

### Low Priority / Future
5. **SuperWoW AV risk dialog** — one-time acknowledgment before adding SuperWoW.
6. ~~**GIF animation**~~ — ✅ Done via `iced_gif` crate; animated GIFs play in README previews.
