//! Persistent settings (JSON file in app data dir).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Stable,
    #[default]
    Beta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UiScaleMode {
    #[default]
    Auto,
    Smaller,
    Small,
    Medium,
    Large,
    Larger,
}

impl UiScaleMode {
    pub const ALL: &[UiScaleMode] = &[
        UiScaleMode::Auto,
        UiScaleMode::Smaller,
        UiScaleMode::Small,
        UiScaleMode::Medium,
        UiScaleMode::Large,
        UiScaleMode::Larger,
    ];

    pub fn label(self) -> &'static str {
        match self {
            UiScaleMode::Auto => "Auto",
            UiScaleMode::Smaller => "Smaller",
            UiScaleMode::Small => "Small",
            UiScaleMode::Medium => "Medium",
            UiScaleMode::Large => "Large",
            UiScaleMode::Larger => "Larger",
        }
    }

    pub fn factor(self) -> f32 {
        match self {
            UiScaleMode::Auto => 0.0, // sentinel — resolved at runtime
            UiScaleMode::Smaller => 0.75,
            UiScaleMode::Small => 0.85,
            UiScaleMode::Medium => 1.0,
            UiScaleMode::Large => 1.10,
            UiScaleMode::Larger => 1.20,
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            UiScaleMode::Auto => "Automatic — scales based on monitor resolution",
            UiScaleMode::Smaller => "Scale: 75%",
            UiScaleMode::Small => "Scale: 85%",
            UiScaleMode::Medium => "Scale: 100%",
            UiScaleMode::Large => "Scale: 110%",
            UiScaleMode::Larger => "Scale: 120%",
        }
    }
}

impl std::fmt::Display for UpdateChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateChannel::Stable => write!(f, "Stable"),
            UpdateChannel::Beta => write!(f, "Beta"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileConfig {
    pub id: String,
    pub name: String,
    pub wow_dir: String,
    pub launch_method: String,
    pub like_turtles: bool,
    pub clear_wdb: bool,
    pub lutris_target: String,
    pub wine_command: String,
    pub wine_args: String,
    pub custom_command: String,
    pub custom_args: String,
    pub working_dir: String,
    pub env_text: String,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            id: String::from("default"),
            name: String::from("Default"),
            wow_dir: String::new(),
            launch_method: String::from("auto"),
            like_turtles: true,
            clear_wdb: false,
            lutris_target: String::new(),
            wine_command: String::from("wine"),
            wine_args: String::new(),
            custom_command: String::new(),
            custom_args: String::new(),
            working_dir: String::new(),
            env_text: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub wow_dir: String,
    pub theme: String,
    pub active_profile_id: String,
    pub opt_auto_check: bool,
    pub opt_desktop_notify: bool,
    pub opt_symlinks: bool,
    pub opt_xattr: bool,
    pub radio_auto_connect: bool,
    pub radio_volume: f32,
    pub radio_auto_play: bool,
    pub radio_buffer_size: usize,
    pub radio_persist_volume: bool,
    pub opt_clock12: bool,
    pub opt_friz_font: bool,
    pub log_wrap: bool,
    pub log_autoscroll: bool,
    pub auto_check_minutes: u32,
    pub profiles: Vec<ProfileConfig>,
    pub ignored_update_ids: Vec<i64>,
    pub update_channel: UpdateChannel,
    pub ui_scale_mode: UiScaleMode,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            wow_dir: String::new(),
            theme: String::from("cata"),
            active_profile_id: String::from("default"),
            opt_auto_check: false,
            opt_desktop_notify: false,
            opt_symlinks: false,
            opt_xattr: true,
            radio_auto_connect: false,
            radio_volume: 0.25,
            radio_auto_play: false,
            radio_buffer_size: 4096,
            radio_persist_volume: true,
            opt_clock12: false,
            opt_friz_font: false,
            log_wrap: false,
            log_autoscroll: true,
            auto_check_minutes: 15,
            profiles: vec![ProfileConfig::default()],
            ignored_update_ids: Vec::new(),
            update_channel: UpdateChannel::Beta,
            ui_scale_mode: UiScaleMode::Auto,
        }
    }
}

/// Returns the app data directory, creating it if needed.
pub fn app_dir() -> Result<PathBuf, String> {
    let dir = if portable_mode_enabled() {
        portable_app_dir()?
    } else {
        standard_app_dir()?
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn standard_app_dir() -> Result<PathBuf, String> {
    Ok(dirs::data_dir()
        .ok_or_else(|| "no data_dir".to_string())?
        .join("wuddle"))
}

fn portable_mode_enabled() -> bool {
    let env_enabled = std::env::var("WUDDLE_PORTABLE")
        .ok()
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);
    if env_enabled {
        return true;
    }
    portable_mode_flag_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

fn portable_mode_flag_path() -> Result<PathBuf, String> {
    Ok(portable_root_dir()?.join("wuddle-portable.flag"))
}

fn portable_app_dir() -> Result<PathBuf, String> {
    Ok(portable_root_dir()?.join("wuddle-data"))
}

fn portable_root_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| "no exe parent".to_string())?;
    // AppImage: exe is inside a version dir, go up one more
    if exe_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with("wuddle"))
        .unwrap_or(false)
    {
        exe_dir
            .parent()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "no parent".to_string())
    } else {
        Ok(exe_dir.to_path_buf())
    }
}

/// DB path for a profile. "default" uses `wuddle.sqlite`, others use `wuddle-{id}.sqlite`.
pub fn profile_db_path(profile_id: &str) -> Result<PathBuf, String> {
    let dir = app_dir()?;
    if profile_id == "default" {
        Ok(dir.join("wuddle.sqlite"))
    } else {
        Ok(dir.join(format!("wuddle-{}.sqlite", profile_id)))
    }
}

/// Like `profile_db_path`, but falls back to `wuddle.sqlite` when the
/// profile-specific DB doesn't exist or has zero repos.  This ensures mods
/// installed under the "default" profile remain visible when the active
/// profile switches to a Tauri-originated ID like "wow1".
pub fn resolve_profile_db_path(profile_id: &str) -> Result<PathBuf, String> {
    let path = profile_db_path(profile_id)?;
    if profile_id == "default" {
        return Ok(path);
    }
    let should_fallback = if path.exists() {
        // Check whether this DB has any repos
        match rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            Ok(conn) => conn
                .query_row("SELECT COUNT(*) FROM repos", [], |row| row.get::<_, i64>(0))
                .unwrap_or(0)
                == 0,
            Err(_) => true,
        }
    } else {
        true
    };
    if should_fallback {
        let dir = app_dir()?;
        let default_path = dir.join("wuddle.sqlite");
        if default_path.exists() {
            return Ok(default_path);
        }
    }
    Ok(path)
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(app_dir()?.join("settings.json"))
}

pub fn load_settings() -> AppSettings {
    let path = match settings_path() {
        Ok(p) => p,
        Err(_) => return AppSettings::default(),
    };
    let settings_existed = path.exists();
    let mut settings: AppSettings = match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => AppSettings::default(),
    };

    // On first launch (settings.json didn't exist yet), import everything
    // from Tauri v2's WebKit localStorage so options carry over seamlessly.
    if !settings_existed {
        import_tauri_options(&mut settings);
    }

    // Discover orphaned profile databases and import from Tauri localStorage
    if let Ok(dir) = app_dir() {
        let before = settings.profiles.len();
        discover_orphan_profiles(&mut settings, &dir);
        let discovered = settings.profiles.len() > before;

        // Import active profile ID from Tauri localStorage
        if discovered || settings.active_profile_id == "default" {
            import_tauri_active_profile(&mut settings);
        }

        // Remove the Iced-only "default" placeholder profile when real Tauri
        // profiles exist. Two cases:
        //   (a) The "default" profile has an empty wow_dir (it was never configured
        //       in Iced) but at least one other profile has a real wow_dir.
        //   (b) The "default" profile's wow_dir duplicates another profile's wow_dir
        //       (the Tauri profile is the canonical one for that installation).
        // In both cases the "default" placeholder is redundant and causes confusion.
        {
            let default_wow = settings.profiles.iter()
                .find(|p| p.id == "default")
                .map(|p| p.wow_dir.clone());
            if let Some(ref dw) = default_wow {
                let has_other_real = settings.profiles.iter()
                    .any(|p| p.id != "default" && !p.wow_dir.is_empty());
                let is_placeholder = dw.is_empty() && has_other_real;
                let is_duplicate = !dw.is_empty() && settings.profiles.iter()
                    .any(|p| p.id != "default" && p.wow_dir == *dw);
                if is_placeholder || is_duplicate {
                    settings.profiles.retain(|p| p.id != "default");
                    // Switch active profile away from the removed placeholder
                    if settings.active_profile_id == "default" {
                        if let Some(first) = settings.profiles.first() {
                            settings.active_profile_id = first.id.clone();
                            settings.wow_dir = first.wow_dir.clone();
                        }
                    }
                }
            }
        }

        if discovered || settings.profiles.len() != before {
            let _ = save_settings(&settings);
        }
    }

    settings
}

/// Scan for `wuddle-*.sqlite` files that aren't tracked in settings.profiles and add them.
/// Also tries to import profile metadata from Tauri's WebKit localStorage.
fn discover_orphan_profiles(settings: &mut AppSettings, dir: &std::path::Path) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let known_ids: std::collections::HashSet<String> =
        settings.profiles.iter().map(|p| p.id.clone()).collect();

    // Try to load profile metadata from Tauri's localStorage (WebKit SQLite)
    let tauri_profiles = read_tauri_localstorage_profiles();

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // Match wuddle-{id}.sqlite but not wuddle.sqlite (that's "default")
        if let Some(id) = name
            .strip_prefix("wuddle-")
            .and_then(|s| s.strip_suffix(".sqlite"))
        {
            if !id.is_empty() && !known_ids.contains(id) {
                // Check if Tauri has metadata for this profile
                let tauri_match = tauri_profiles.iter().find(|p| p.id == id);
                settings.profiles.push(if let Some(tp) = tauri_match {
                    tp.clone()
                } else {
                    ProfileConfig {
                        id: id.to_string(),
                        name: id.to_string(),
                        ..ProfileConfig::default()
                    }
                });
            }
        }
    }

    // Also merge metadata from Tauri for profiles that exist but have empty wow_dir
    for profile in &mut settings.profiles {
        if profile.wow_dir.is_empty() {
            if let Some(tp) = tauri_profiles.iter().find(|p| p.id == profile.id) {
                if !tp.wow_dir.is_empty() {
                    profile.wow_dir = tp.wow_dir.clone();
                    profile.name = tp.name.clone();
                    profile.launch_method = tp.launch_method.clone();
                    profile.like_turtles = tp.like_turtles;
                    profile.clear_wdb = tp.clear_wdb;
                    profile.lutris_target = tp.lutris_target.clone();
                    profile.wine_command = tp.wine_command.clone();
                    profile.wine_args = tp.wine_args.clone();
                    profile.custom_command = tp.custom_command.clone();
                    profile.custom_args = tp.custom_args.clone();
                    profile.working_dir = tp.working_dir.clone();
                    profile.env_text = tp.env_text.clone();
                }
            }
        }
    }
}

/// Try to read profile data from Tauri's WebKit localStorage SQLite file.
/// Returns an empty vec on any failure (missing file, wrong format, etc.).
fn read_tauri_localstorage_profiles() -> Vec<ProfileConfig> {
    let data_dir = match dirs::data_dir() {
        Some(d) => d,
        None => return Vec::new(),
    };

    // Tauri v2 stores localStorage in this path
    let ls_path = data_dir
        .join("io.github.zythdr.wuddle")
        .join("localstorage")
        .join("tauri_localhost_0.localstorage");

    if !ls_path.exists() {
        return Vec::new();
    }

    // Open the WebKit localStorage SQLite and read the profiles key
    let conn = match rusqlite::Connection::open_with_flags(
        &ls_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    // WebKit stores values as UTF-16LE blobs
    let blob: Vec<u8> = match conn.query_row(
        "SELECT value FROM ItemTable WHERE key = 'wuddle.profiles'",
        [],
        |row| row.get(0),
    ) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };

    // Decode UTF-16LE
    let text = match String::from_utf16(
        &blob
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect::<Vec<u16>>(),
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    // Parse JSON array of Tauri profile objects (camelCase)
    let arr: Vec<serde_json::Value> = match serde_json::from_str(&text) {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };

    arr.iter()
        .filter_map(|p| {
            let id = p.get("id")?.as_str()?.to_string();
            let launch = p.get("launch").cloned().unwrap_or(serde_json::json!({}));
            // Tauri stores wowDir which may point to an exe — extract directory
            let raw_wow_dir = p.get("wowDir").and_then(|v| v.as_str()).unwrap_or("");
            let wow_dir = if raw_wow_dir.to_lowercase().ends_with(".exe") {
                std::path::Path::new(raw_wow_dir)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default()
            } else {
                // Strip trailing slash
                raw_wow_dir.trim_end_matches('/').to_string()
            };
            Some(ProfileConfig {
                id,
                name: p.get("name").and_then(|v| v.as_str()).unwrap_or("WoW").to_string(),
                wow_dir,
                launch_method: launch.get("method").and_then(|v| v.as_str()).unwrap_or("auto").to_string(),
                like_turtles: p.get("likesTurtles").and_then(|v| v.as_bool()).unwrap_or(false),
                clear_wdb: launch.get("clearWdb").and_then(|v| v.as_bool()).unwrap_or(false),
                lutris_target: launch.get("lutrisTarget").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                wine_command: launch.get("wineCommand").and_then(|v| v.as_str()).unwrap_or("wine").to_string(),
                wine_args: launch.get("wineArgs").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                custom_command: launch.get("customCommand").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                custom_args: launch.get("customArgs").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                working_dir: launch.get("workingDir").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                env_text: launch.get("envText").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// Import active profile ID from Tauri's WebKit localStorage.
/// Only updates if the imported ID exists in the current profile list.
fn import_tauri_active_profile(settings: &mut AppSettings) {
    let data_dir = match dirs::data_dir() {
        Some(d) => d,
        None => return,
    };
    let ls_path = data_dir
        .join("io.github.zythdr.wuddle")
        .join("localstorage")
        .join("tauri_localhost_0.localstorage");
    if !ls_path.exists() {
        return;
    }
    let conn = match rusqlite::Connection::open_with_flags(
        &ls_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Read active profile ID
    if let Ok(blob) = conn.query_row::<Vec<u8>, _, _>(
        "SELECT value FROM ItemTable WHERE key = 'wuddle.profile.active'",
        [],
        |row| row.get(0),
    ) {
        if let Ok(text) = String::from_utf16(
            &blob.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<u16>>(),
        ) {
            let id = text.trim().trim_matches('"').to_string();
            if settings.profiles.iter().any(|p| p.id == id) {
                settings.active_profile_id = id;
            }
        }
    }

    // Also sync wow_dir from active profile
    if let Some(p) = settings.profiles.iter().find(|p| p.id == settings.active_profile_id) {
        if !p.wow_dir.is_empty() {
            settings.wow_dir = p.wow_dir.clone();
        }
    }
}

/// Import option flags from Tauri's WebKit localStorage into settings.
/// Called once on first launch (when settings.json didn't exist yet) so
/// that theme, symlinks, auto-check, friz-font, etc. carry over seamlessly.
fn import_tauri_options(settings: &mut AppSettings) {
    let data_dir = match dirs::data_dir() {
        Some(d) => d,
        None => return,
    };
    let ls_path = data_dir
        .join("io.github.zythdr.wuddle")
        .join("localstorage")
        .join("tauri_localhost_0.localstorage");
    if !ls_path.exists() {
        return;
    }
    let conn = match rusqlite::Connection::open_with_flags(
        &ls_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Helper: read a UTF-16LE WebKit localStorage value by key.
    let read_ls = |key: &str| -> Option<String> {
        let blob: Vec<u8> = conn
            .query_row(
                "SELECT value FROM ItemTable WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .ok()?;
        String::from_utf16(
            &blob
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect::<Vec<u16>>(),
        )
        .ok()
    };

    if let Some(v) = read_ls("wuddle.opt.theme") {
        let t = v.trim().trim_matches('"').to_string();
        if !t.is_empty() {
            settings.theme = t;
        }
    }
    if let Some(v) = read_ls("wuddle.opt.symlinks") {
        settings.opt_symlinks = v.trim().trim_matches('"') == "true";
    }
    if let Some(v) = read_ls("wuddle.opt.xattr") {
        settings.opt_xattr = v.trim().trim_matches('"') == "true";
    }
    if let Some(v) = read_ls("wuddle.opt.clock12") {
        settings.opt_clock12 = v.trim().trim_matches('"') == "true";
    }
    if let Some(v) = read_ls("wuddle.opt.frizfont") {
        settings.opt_friz_font = v.trim().trim_matches('"') == "true";
    }
    if let Some(v) = read_ls("wuddle.opt.autocheck") {
        settings.opt_auto_check = v.trim().trim_matches('"') == "true";
    }
    if let Some(v) = read_ls("wuddle.opt.autocheck.minutes") {
        if let Ok(n) = v.trim().trim_matches('"').parse::<u32>() {
            if n >= 1 && n <= 240 {
                settings.auto_check_minutes = n;
            }
        }
    }
    if let Some(v) = read_ls("wuddle.opt.desktop.notify") {
        settings.opt_desktop_notify = v.trim().trim_matches('"') == "true";
    }
    if let Some(v) = read_ls("wuddle.log.wrap") {
        settings.log_wrap = v.trim().trim_matches('"') == "true";
    }
    if let Some(v) = read_ls("wuddle.log.autoscroll") {
        settings.log_autoscroll = v.trim().trim_matches('"') == "true";
    }
    if let Some(v) = read_ls("wuddle.opt.update_channel") {
        let ch = v.trim().trim_matches('"').to_string();
        if ch == "beta" {
            settings.update_channel = UpdateChannel::Beta;
        } else if ch == "stable" {
            settings.update_channel = UpdateChannel::Stable;
        }
    }
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    let data = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())
}
