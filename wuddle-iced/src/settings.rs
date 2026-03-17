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
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => AppSettings::default(),
    }
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    let data = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())
}
