use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub static AUTO_UI_SCALE: OnceLock<f32> = OnceLock::new();

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct WindowGeometry {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub x: Option<i32>,
    pub y: Option<i32>,
}

impl WindowGeometry {
    const MIN_WIDTH: u32 = 640;
    const MIN_HEIGHT: u32 = 480;
    const MAX_DIMENSION: u32 = 16_384;
    const MAX_ABSOLUTE_POSITION: i32 = 100_000;

    pub fn initial_size(self) -> Option<(f32, f32)> {
        let width = self.width?;
        let height = self.height?;
        if (Self::MIN_WIDTH..=Self::MAX_DIMENSION).contains(&width)
            && (Self::MIN_HEIGHT..=Self::MAX_DIMENSION).contains(&height)
        {
            Some((width as f32, height as f32))
        } else {
            None
        }
    }

    pub fn initial_position(self) -> Option<(f32, f32)> {
        let x = self.x?;
        let y = self.y?;
        if x.unsigned_abs() <= Self::MAX_ABSOLUTE_POSITION as u32
            && y.unsigned_abs() <= Self::MAX_ABSOLUTE_POSITION as u32
        {
            Some((x as f32, y as f32))
        } else {
            None
        }
    }

    pub fn remember_size(&mut self, width: f32, height: f32) {
        if width.is_finite() && height.is_finite() {
            let width = width.round().clamp(0.0, u32::MAX as f32) as u32;
            let height = height.round().clamp(0.0, u32::MAX as f32) as u32;
            if (Self::MIN_WIDTH..=Self::MAX_DIMENSION).contains(&width)
                && (Self::MIN_HEIGHT..=Self::MAX_DIMENSION).contains(&height)
            {
                self.width = Some(width);
                self.height = Some(height);
            }
        }
    }

    pub fn remember_position(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            let x = x.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32;
            let y = y.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32;
            if x.unsigned_abs() <= Self::MAX_ABSOLUTE_POSITION as u32
                && y.unsigned_abs() <= Self::MAX_ABSOLUTE_POSITION as u32
            {
                self.x = Some(x);
                self.y = Some(y);
            }
        }
    }
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
    pub auto_launch_exe: Option<String>,
    pub launch_method: String,
    #[serde(default = "default_true")]
    pub show_mods_tab: bool,
    #[serde(default = "default_true")]
    pub show_addons_tab: bool,
    #[serde(default = "default_true")]
    pub show_patches_tab: bool,
    #[serde(default = "default_true")]
    pub show_tweaks_tab: bool,
    pub clear_wdb: bool,
    pub auto_login_enabled: bool,
    pub lutris_target: String,
    pub wine_command: String,
    pub wine_args: String,
    pub custom_command: String,
    pub custom_args: String,
    pub working_dir: String,
    pub env_text: String,
    #[cfg(feature = "auto-login")]
    pub auto_login_accounts: Vec<wuddle_engine::auto_login::AccountRef>,
    #[cfg(not(feature = "auto-login"))]
    pub auto_login_accounts: Vec<serde_json::Value>,
    #[cfg(feature = "auto-login")]
    pub selected_auto_login_account_id: Option<wuddle_engine::auto_login::AccountId>,
    #[cfg(not(feature = "auto-login"))]
    pub selected_auto_login_account_id: Option<serde_json::Value>,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            id: String::from("default"),
            name: String::from("Default"),
            wow_dir: String::new(),
            auto_launch_exe: None,
            launch_method: String::from("auto"),
            show_mods_tab: true,
            show_addons_tab: true,
            show_patches_tab: true,
            show_tweaks_tab: true,
            clear_wdb: false,
            auto_login_enabled: false,
            lutris_target: String::new(),
            wine_command: String::from("wine"),
            wine_args: String::new(),
            custom_command: String::new(),
            custom_args: String::new(),
            working_dir: String::new(),
            env_text: String::new(),
            #[cfg(feature = "auto-login")]
            auto_login_accounts: Vec::new(),
            #[cfg(not(feature = "auto-login"))]
            auto_login_accounts: Vec::new(),
            #[cfg(feature = "auto-login")]
            selected_auto_login_account_id: None,
            #[cfg(not(feature = "auto-login"))]
            selected_auto_login_account_id: None,
        }
    }
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub wow_dir: String,
    pub theme: String,
    pub active_profile_id: String,
    pub opt_auto_check: bool,
    pub opt_conserve_github_api: bool,
    pub opt_desktop_notify: bool,
    pub opt_symlinks: bool,
    pub opt_xattr: bool,
    pub opt_clock12: bool,
    pub opt_friz_font: bool,
    pub remember_window_geometry: bool,
    pub log_wrap: bool,
    pub log_autoscroll: bool,
    pub verbose_diagnostics: bool,
    pub auto_check_minutes: u32,
    pub profiles: Vec<ProfileConfig>,
    pub ignored_update_ids: Vec<i64>,
    pub ignored_update_ids_by_profile: HashMap<String, Vec<i64>>,
    pub mods_warning_dismissed_profile_ids: Vec<String>,
    pub patches_warning_dismissed_profile_ids: Vec<String>,
    pub update_channel: UpdateChannel,
    pub ui_scale_mode: UiScaleMode,
    pub migrated_from_tauri: bool,
    pub auto_login_warning_acknowledged: bool,
    pub window_geometry: WindowGeometry,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            wow_dir: String::new(),
            theme: String::from("cata"),
            active_profile_id: String::from("default"),
            opt_auto_check: false,
            opt_conserve_github_api: true,
            opt_desktop_notify: false,
            opt_symlinks: false,
            opt_xattr: true,
            opt_clock12: false,
            opt_friz_font: false,
            remember_window_geometry: true,
            log_wrap: false,
            log_autoscroll: true,
            verbose_diagnostics: false,
            auto_check_minutes: 15,
            profiles: vec![ProfileConfig::default()],
            ignored_update_ids: Vec::new(),
            ignored_update_ids_by_profile: HashMap::new(),
            mods_warning_dismissed_profile_ids: Vec::new(),
            patches_warning_dismissed_profile_ids: Vec::new(),
            update_channel: UpdateChannel::Beta,
            ui_scale_mode: UiScaleMode::Auto,
            migrated_from_tauri: false,
            auto_login_warning_acknowledged: false,
            window_geometry: WindowGeometry::default(),
        }
    }
}

pub fn profile_id_from_name(name: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;

    for ch in name.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }

    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "profile".to_string()
    } else {
        out
    }
}

pub fn unique_profile_id(name: &str, profiles: &[ProfileConfig]) -> String {
    let base = profile_id_from_name(name);
    if !profiles.iter().any(|p| p.id == base) {
        return base;
    }

    for n in 2.. {
        let candidate = format!("{}-{}", base, n);
        if !profiles.iter().any(|p| p.id == candidate) {
            return candidate;
        }
    }

    unreachable!()
}

pub fn normalize_wow_path_input(raw: &str) -> (String, Option<String>) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }

    let cleaned = trimmed.trim_end_matches(['/', '\\']).to_string();
    if cleaned.to_ascii_lowercase().ends_with(".exe") {
        let exe_path = Path::new(&cleaned);
        let parent = exe_path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let exe_name = exe_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|name| name.to_string());

        if !parent.is_empty() && exe_name.is_some() {
            return (parent, exe_name);
        }
    }

    (cleaned, None)
}

pub fn wow_path_display(wow_dir: &str, auto_launch_exe: Option<&str>) -> String {
    if let Some(exe_name) = auto_launch_exe.filter(|name| !name.trim().is_empty()) {
        return Path::new(wow_dir)
            .join(exe_name)
            .to_string_lossy()
            .to_string();
    }
    wow_dir.to_string()
}

pub fn auto_launch_description(auto_launch_exe: Option<&str>) -> String {
    match auto_launch_exe.filter(|name| !name.trim().is_empty()) {
        Some(exe_name) => format!(
            "Auto: launches {} if present, otherwise VanillaFixes.exe, then Wow.exe",
            exe_name
        ),
        None => "Auto: launches VanillaFixes.exe if present, otherwise Wow.exe".to_string(),
    }
}

/// Returns the app data directory, creating it if needed.
pub fn app_dir() -> Result<PathBuf, String> {
    crate::storage::app_dir()
}

pub(crate) fn standard_app_dir() -> Result<PathBuf, String> {
    Ok(dirs::data_dir()
        .ok_or_else(|| "no data_dir".to_string())?
        .join("wuddle"))
}

/// Whether non-Windows builds should keep data beside the executable instead
/// of in the platform application-data directory. Windows is self-contained by
/// default and keeps credentials separately in Windows Credential Manager.
pub fn portable_mode_enabled() -> bool {
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

pub(crate) fn portable_app_dir() -> Result<PathBuf, String> {
    Ok(portable_root_dir()?.join("wuddle-data"))
}

pub(crate) fn portable_root_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe.parent().ok_or_else(|| "no exe parent".to_string())?;
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

/// Resolve the database path for exactly one profile.
/// Profile data must not fall back to another profile's database.
pub fn resolve_profile_db_path(profile_id: &str) -> Result<PathBuf, String> {
    profile_db_path(profile_id)
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
    if !settings_existed && crate::storage::allow_legacy_tauri_import() {
        import_tauri_options(&mut settings);
    }

    // Discover orphaned profile databases and import from Tauri localStorage.
    // This is primarily for the first launch or migration from Tauri v2.
    // Once migrated, we stop auto-discovering so that deleted profiles stay deleted.
    if let Ok(dir) = app_dir() {
        if !settings.migrated_from_tauri && crate::storage::allow_legacy_tauri_import() {
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
                let default_wow = settings
                    .profiles
                    .iter()
                    .find(|p| p.id == "default")
                    .map(|p| p.wow_dir.clone());
                if let Some(ref dw) = default_wow {
                    let has_other_real = settings
                        .profiles
                        .iter()
                        .any(|p| p.id != "default" && !p.wow_dir.is_empty());
                    let is_placeholder = dw.is_empty() && has_other_real;
                    let is_duplicate = !dw.is_empty()
                        && settings
                            .profiles
                            .iter()
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

            settings.migrated_from_tauri = true;
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
                    profile.auto_launch_exe = tp.auto_launch_exe.clone();
                    profile.name = tp.name.clone();
                    profile.launch_method = tp.launch_method.clone();
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
            let raw_wow_dir = p.get("wowDir").and_then(|v| v.as_str()).unwrap_or("");
            let (wow_dir, auto_launch_exe) = normalize_wow_path_input(raw_wow_dir);
            Some(ProfileConfig {
                id,
                name: p
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("WoW")
                    .to_string(),
                wow_dir,
                auto_launch_exe,
                launch_method: launch
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("auto")
                    .to_string(),
                show_mods_tab: true,
                show_addons_tab: true,
                show_patches_tab: true,
                show_tweaks_tab: true,
                clear_wdb: launch
                    .get("clearWdb")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                auto_login_enabled: false,
                lutris_target: launch
                    .get("lutrisTarget")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                wine_command: launch
                    .get("wineCommand")
                    .and_then(|v| v.as_str())
                    .unwrap_or("wine")
                    .to_string(),
                wine_args: launch
                    .get("wineArgs")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                custom_command: launch
                    .get("customCommand")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                custom_args: launch
                    .get("customArgs")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                working_dir: launch
                    .get("workingDir")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                env_text: launch
                    .get("envText")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                #[cfg(feature = "auto-login")]
                auto_login_accounts: Vec::new(),
                #[cfg(not(feature = "auto-login"))]
                auto_login_accounts: Vec::new(),
                #[cfg(feature = "auto-login")]
                selected_auto_login_account_id: None,
                #[cfg(not(feature = "auto-login"))]
                selected_auto_login_account_id: None,
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
            &blob
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect::<Vec<u16>>(),
        ) {
            let id = text.trim().trim_matches('"').to_string();
            if settings.profiles.iter().any(|p| p.id == id) {
                settings.active_profile_id = id;
            }
        }
    }

    // Also sync wow_dir from active profile
    if let Some(p) = settings
        .profiles
        .iter()
        .find(|p| p.id == settings.active_profile_id)
    {
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
            .query_row("SELECT value FROM ItemTable WHERE key = ?1", [key], |row| {
                row.get(0)
            })
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

pub fn detect_auto_scale() -> f32 {
    if let Some((_w, h)) = crate::monitor::primary_monitor_size() {
        if h <= 1080 {
            return 0.85;
        }
    }
    1.0
}

pub fn resolve_ui_scale(mode: UiScaleMode) -> f32 {
    match mode {
        UiScaleMode::Auto => *AUTO_UI_SCALE.get().unwrap_or(&1.0),
        other => other.factor(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_default_to_manual_login() {
        let settings: AppSettings = serde_json::from_str("{}").unwrap();
        assert!(!settings.verbose_diagnostics);
        assert!(settings.opt_conserve_github_api);
        assert!(!settings.auto_login_warning_acknowledged);
        assert!(!settings.profiles[0].auto_login_enabled);
        assert!(settings.profiles[0].auto_login_accounts.is_empty());
        assert!(settings.profiles[0]
            .selected_auto_login_account_id
            .is_none());
        assert!(settings.mods_warning_dismissed_profile_ids.is_empty());
        assert!(settings.patches_warning_dismissed_profile_ids.is_empty());
        assert!(settings.profiles[0].show_mods_tab);
        assert!(settings.profiles[0].show_addons_tab);
        assert!(settings.profiles[0].show_patches_tab);
        assert!(settings.profiles[0].show_tweaks_tab);
        assert!(settings.remember_window_geometry);
        assert_eq!(settings.window_geometry, WindowGeometry::default());
    }

    #[test]
    fn window_geometry_round_trips_and_rejects_unreasonable_values() {
        let mut geometry = WindowGeometry::default();
        geometry.remember_size(1280.4, 719.6);
        geometry.remember_position(-1440.2, 120.7);
        assert_eq!(geometry.initial_size(), Some((1280.0, 720.0)));
        assert_eq!(geometry.initial_position(), Some((-1440.0, 121.0)));

        let encoded = serde_json::to_string(&geometry).unwrap();
        let decoded: WindowGeometry = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, geometry);

        let unreasonable = WindowGeometry {
            width: Some(20),
            height: Some(40_000),
            x: Some(200_000),
            y: Some(0),
        };
        assert_eq!(unreasonable.initial_size(), None);
        assert_eq!(unreasonable.initial_position(), None);
    }

    #[test]
    fn profile_tab_visibility_defaults_on_and_round_trips_overrides() {
        let legacy: ProfileConfig =
            serde_json::from_str(r#"{"id":"legacy","name":"Legacy"}"#).unwrap();
        assert!(legacy.show_mods_tab);
        assert!(legacy.show_addons_tab);
        assert!(legacy.show_patches_tab);
        assert!(legacy.show_tweaks_tab);

        let customized: ProfileConfig = serde_json::from_str(
            r#"{
                "id":"restricted",
                "name":"Restricted server",
                "show_mods_tab":false,
                "show_addons_tab":true,
                "show_patches_tab":false,
                "show_tweaks_tab":false
            }"#,
        )
        .unwrap();
        assert!(!customized.show_mods_tab);
        assert!(customized.show_addons_tab);
        assert!(!customized.show_patches_tab);
        assert!(!customized.show_tweaks_tab);
        let encoded = serde_json::to_string(&customized).unwrap();
        assert!(encoded.contains("\"show_mods_tab\":false"));
        assert!(encoded.contains("\"show_addons_tab\":true"));
    }

    #[test]
    fn auto_login_metadata_round_trips_without_credentials() {
        let raw = r#"{
            "auto_login_warning_acknowledged": true,
            "profiles": [{
                "id": "default",
                "name": "Default",
                "auto_login_enabled": true,
                "auto_login_accounts": [{
                    "id": "de305d54-75b4-431b-adb2-eb6b9e546014",
                    "label": "Main"
                }],
                "selected_auto_login_account_id": "de305d54-75b4-431b-adb2-eb6b9e546014"
            }]
        }"#;
        let settings: AppSettings = serde_json::from_str(raw).unwrap();
        assert!(settings.profiles[0].auto_login_enabled);
        let encoded = serde_json::to_string(&settings).unwrap();
        assert!(encoded.contains("\"auto_login_enabled\":true"));
        assert!(encoded.contains("Main"));
        assert!(encoded.contains("de305d54-75b4-431b-adb2-eb6b9e546014"));
        assert!(!encoded.contains("password"));
        assert!(!encoded.contains("realmlist"));
        assert!(!encoded.contains("realm_name"));
    }
}
