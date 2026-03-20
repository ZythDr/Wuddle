//! Persistent settings (JSON file in app data dir).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub opt_clock12: bool,
    pub opt_friz_font: bool,
    pub log_wrap: bool,
    pub log_autoscroll: bool,
    pub auto_check_minutes: u32,
    pub profiles: Vec<ProfileConfig>,
    pub ignored_update_ids: Vec<i64>,
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
            opt_clock12: false,
            opt_friz_font: false,
            log_wrap: false,
            log_autoscroll: true,
            auto_check_minutes: 15,
            profiles: vec![ProfileConfig::default()],
            ignored_update_ids: Vec::new(),
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

fn settings_path() -> Result<PathBuf, String> {
    Ok(app_dir()?.join("settings.json"))
}

pub fn load_settings() -> AppSettings {
    let path = match settings_path() {
        Ok(p) => p,
        Err(_) => return AppSettings::default(),
    };
    let mut settings: AppSettings = match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => AppSettings::default(),
    };

    // Discover orphaned profile databases and import from Tauri localStorage
    if let Ok(dir) = app_dir() {
        let before = settings.profiles.len();
        discover_orphan_profiles(&mut settings, &dir);
        let discovered = settings.profiles.len() > before;

        // Import active profile ID from Tauri localStorage
        if discovered || settings.active_profile_id == "default" {
            import_tauri_active_profile(&mut settings);
        }

        // Remove the Iced-only "default" profile if:
        // - It uses the default wuddle.sqlite (no profile-specific DB)
        // - Another profile from Tauri has the same wow_dir (it's the real profile)
        // - The active profile is not "default"
        if settings.active_profile_id != "default" {
            let default_wow = settings.profiles.iter()
                .find(|p| p.id == "default")
                .map(|p| p.wow_dir.clone());
            if let Some(ref dw) = default_wow {
                if !dw.is_empty() {
                    let has_duplicate = settings.profiles.iter().any(|p| {
                        p.id != "default" && p.wow_dir == *dw
                    });
                    if has_duplicate {
                        settings.profiles.retain(|p| p.id != "default");
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

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    let data = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())
}
