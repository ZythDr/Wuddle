use keyring::Entry;
use serde::Serialize;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{mpsc, Mutex, OnceLock},
    time::Duration,
};
use tauri::Manager;

use wuddle_engine::{Engine, InstallMode, InstallOptions};

#[derive(Serialize)]
struct RepoRow {
    id: i64,
    forge: String,
    owner: String,
    name: String,
    mode: String,
    enabled: bool,
    url: String,
}

#[derive(Serialize)]
struct PlanRow {
    repo_id: i64,
    owner: String,
    name: String,
    current: Option<String>,
    latest: String,
    asset_name: String,
    has_update: bool,
    repair_needed: bool,
    not_modified: bool,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GithubAuthStatus {
    keychain_available: bool,
    token_stored: bool,
    env_token_present: bool,
}

const KEYCHAIN_SERVICE: &str = "wuddle";
const KEYCHAIN_ACCOUNT_GITHUB_TOKEN: &str = "github_token";
const KEYCHAIN_ACCOUNT_PROBE: &str = "github_token_probe";
const DEFAULT_PROFILE_ID: &str = "default";
const KEYCHAIN_TIMEOUT_MS: u64 = 2500;

static ACTIVE_PROFILE_ID: OnceLock<Mutex<String>> = OnceLock::new();
static KEYCHAIN_SYNC_ATTEMPTED: OnceLock<Mutex<bool>> = OnceLock::new();

fn active_profile_state() -> &'static Mutex<String> {
    ACTIVE_PROFILE_ID.get_or_init(|| Mutex::new(DEFAULT_PROFILE_ID.to_string()))
}

fn keychain_sync_attempted_state() -> &'static Mutex<bool> {
    KEYCHAIN_SYNC_ATTEMPTED.get_or_init(|| Mutex::new(false))
}

fn normalize_profile_id(value: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in value.trim().chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        DEFAULT_PROFILE_ID.to_string()
    } else {
        out
    }
}

fn active_profile_id() -> String {
    match active_profile_state().lock() {
        Ok(guard) => guard.clone(),
        Err(_) => DEFAULT_PROFILE_ID.to_string(),
    }
}

fn app_dir() -> Result<PathBuf, String> {
    let dir = dirs::data_dir()
        .ok_or_else(|| "no data_dir".to_string())?
        .join("wuddle");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn profile_db_path(profile_id: &str) -> Result<PathBuf, String> {
    Ok(app_dir()?.join(format!("wuddle-{}.sqlite", profile_id)))
}

fn default_db_path() -> Result<PathBuf, String> {
    Ok(app_dir()?.join("wuddle.sqlite"))
}

fn profile_db_main_path(profile_id: &str) -> Result<PathBuf, String> {
    if profile_id == DEFAULT_PROFILE_ID {
        default_db_path()
    } else {
        profile_db_path(profile_id)
    }
}

fn remove_db_with_sidecars(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    for suffix in ["-wal", "-shm"] {
        let mut sidecar_os = path.as_os_str().to_os_string();
        sidecar_os.push(suffix);
        let sidecar = PathBuf::from(sidecar_os);
        if sidecar.exists() {
            fs::remove_file(sidecar).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn keychain_entry(account: &str) -> Result<Entry, String> {
    Entry::new(KEYCHAIN_SERVICE, account).map_err(|e| e.to_string())
}

fn keychain_call_with_timeout<T, F>(label: &'static str, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(Duration::from_millis(KEYCHAIN_TIMEOUT_MS)) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
            "System keychain timed out while {}. Ensure KWallet is running, or use WUDDLE_GITHUB_TOKEN.",
            label
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("System keychain worker failed unexpectedly.".to_string())
        }
    }
}

fn env_token_present() -> bool {
    std::env::var("WUDDLE_GITHUB_TOKEN")
        .ok()
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .is_some()
}

fn read_keychain_token() -> Result<Option<String>, String> {
    keychain_call_with_timeout("reading token", || {
        let entry = keychain_entry(KEYCHAIN_ACCOUNT_GITHUB_TOKEN)?;
        match entry.get_password() {
            Ok(token) => {
                let token = token.trim().to_string();
                if token.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(token))
                }
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    })
}

fn keychain_probe_available() -> Result<(), String> {
    keychain_call_with_timeout("probing keychain", || {
        let probe = format!("wuddle-probe-{}", std::process::id());
        let entry = keychain_entry(KEYCHAIN_ACCOUNT_PROBE)?;
        entry.set_password(&probe).map_err(|e| e.to_string())?;
        let read_back = entry.get_password().map_err(|e| e.to_string())?;
        if read_back != probe {
            return Err("keychain probe mismatch".to_string());
        }
        let _ = entry.delete_credential();
        Ok(())
    })
}

fn set_keychain_token(token: String) -> Result<(), String> {
    keychain_call_with_timeout("saving token", move || {
        let entry = keychain_entry(KEYCHAIN_ACCOUNT_GITHUB_TOKEN)?;
        entry.set_password(&token).map_err(|e| e.to_string())
    })
}

fn clear_keychain_token() -> Result<(), String> {
    keychain_call_with_timeout("clearing token", || {
        let entry = keychain_entry(KEYCHAIN_ACCOUNT_GITHUB_TOKEN)?;
        if let Err(e) = entry.delete_credential() {
            if !matches!(e, keyring::Error::NoEntry) {
                return Err(e.to_string());
            }
        }
        Ok(())
    })
}

fn sync_github_token_env_from_keychain() {
    if std::env::var("WUDDLE_GITHUB_TOKEN")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .is_some()
    {
        return;
    }
    let already_attempted = match keychain_sync_attempted_state().lock() {
        Ok(guard) => *guard,
        Err(_) => true,
    };
    if already_attempted {
        return;
    }
    if let Ok(mut guard) = keychain_sync_attempted_state().lock() {
        *guard = true;
    }

    if let Ok(Some(token)) = read_keychain_token() {
        std::env::set_var("WUDDLE_GITHUB_TOKEN", &token);
    }
}

fn clear_cached_github_rate_limits(eng: &Engine) {
    let repos = match eng.db().list_repos() {
        Ok(v) => v,
        Err(_) => return,
    };

    let mut seen = HashSet::new();
    for repo in repos {
        if !repo.forge.eq_ignore_ascii_case("github") {
            continue;
        }
        if seen.insert(repo.host.clone()) {
            let _ = eng.db().clear_rate_limit(&repo.host);
        }
    }
}

fn engine_for_profile(profile_id: &str) -> Result<Engine, String> {
    sync_github_token_env_from_keychain();
    if profile_id == DEFAULT_PROFILE_ID {
        return Engine::open_default().map_err(|e| e.to_string());
    }
    let db_path = profile_db_path(profile_id)?;
    Engine::open(&db_path).map_err(|e| e.to_string())
}

fn engine() -> Result<Engine, String> {
    engine_for_profile(&active_profile_id())
}

fn normalize_wow_dir(wow_dir: String) -> Result<String, String> {
    let wow_dir = wow_dir.trim().to_string();
    if wow_dir.is_empty() {
        return Err("wowDir is empty".into());
    }
    Ok(wow_dir)
}

fn normalize_optional_wow_dir(wow_dir: Option<String>) -> Option<String> {
    wow_dir
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn install_options(use_symlinks: Option<bool>, set_xattr_comment: Option<bool>) -> InstallOptions {
    InstallOptions {
        use_symlinks: use_symlinks.unwrap_or(false),
        set_xattr_comment: set_xattr_comment.unwrap_or(false),
    }
}

#[cfg(target_os = "linux")]
fn apply_linux_runtime_env_defaults() {
    let is_appimage = std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some();
    if !is_appimage {
        return;
    }

    let defaults = [
        // Work around WebKitGTK rendering issues seen in some AppImage environments.
        ("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
        ("WEBKIT_DISABLE_COMPOSITING_MODE", "1"),
        // On some hosts WebKit sandboxing breaks inside AppImage and yields a blank view.
        ("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS", "1"),
    ];

    for (key, value) in defaults {
        let has_value = std::env::var_os(key)
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if !has_value {
            std::env::set_var(key, value);
        }
    }
}

async fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn wuddle_list_repos() -> Result<Vec<RepoRow>, String> {
    run_blocking(|| {
        let eng = engine()?;
        let repos = eng.db().list_repos().map_err(|e| e.to_string())?;

        Ok(repos
            .into_iter()
            .map(|r| RepoRow {
                id: r.id,
                forge: r.forge,
                owner: r.owner,
                name: r.name,
                mode: r.mode.as_str().to_string(),
                enabled: r.enabled,
                url: r.url,
            })
            .collect())
    })
    .await
}

#[tauri::command]
async fn wuddle_add_repo(url: String, mode: String) -> Result<i64, String> {
    run_blocking(move || {
        let eng = engine()?;
        let mode = InstallMode::from_str(&mode).ok_or("Invalid mode")?;
        eng.add_repo(&url, mode, None).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
async fn wuddle_remove_repo(
    id: i64,
    removeLocalFiles: Option<bool>,
    wowDir: Option<String>,
) -> Result<String, String> {
    let remove_local_files = removeLocalFiles.unwrap_or(false);
    let wow_dir = normalize_optional_wow_dir(wowDir);

    run_blocking(move || {
        let eng = engine()?;
        let removed = eng
            .remove_repo(id, wow_dir.as_deref().map(Path::new), remove_local_files)
            .map_err(|e| e.to_string())?;
        if remove_local_files {
            Ok(format!(
                "Removed from Wuddle and deleted {} local path(s).",
                removed
            ))
        } else {
            Ok("Removed from Wuddle.".to_string())
        }
    })
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
async fn wuddle_set_repo_enabled(
    id: i64,
    enabled: bool,
    wowDir: Option<String>,
) -> Result<String, String> {
    let wow_dir = normalize_optional_wow_dir(wowDir);

    run_blocking(move || {
        let eng = engine()?;
        let touched = eng
            .set_repo_enabled(id, enabled, wow_dir.as_deref().map(Path::new))
            .map_err(|e| e.to_string())?;
        if touched > 0 {
            Ok(format!(
                "{} project and updated {} dlls.txt entr{}.",
                if enabled { "Enabled" } else { "Disabled" },
                touched,
                if touched == 1 { "y" } else { "ies" }
            ))
        } else {
            Ok(format!(
                "{} project.",
                if enabled { "Enabled" } else { "Disabled" }
            ))
        }
    })
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
async fn wuddle_check_updates(wowDir: Option<String>) -> Result<Vec<PlanRow>, String> {
    let wow_dir = normalize_optional_wow_dir(wowDir);

    run_blocking(move || {
        let plans = tauri::async_runtime::block_on(async {
            let eng = engine()?;
            let wow_path = wow_dir.as_deref().map(Path::new);
            eng.check_updates_with_wow(wow_path)
                .await
                .map_err(|e| e.to_string())
        })?;

        Ok(plans
            .into_iter()
            .map(|p| PlanRow {
                repo_id: p.repo_id,
                owner: p.owner,
                name: p.name,
                current: p.current,
                latest: p.latest,
                asset_name: p.asset_name,
                has_update: !p.asset_url.is_empty(),
                repair_needed: p.repair_needed,
                not_modified: p.not_modified,
                error: p.error,
            })
            .collect())
    })
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
async fn wuddle_update_all(
    wowDir: String,
    useSymlinks: Option<bool>,
    setXattrComment: Option<bool>,
) -> Result<String, String> {
    let wowDir = normalize_wow_dir(wowDir)?;
    let opts = install_options(useSymlinks, setXattrComment);

    run_blocking(move || {
        let plans = tauri::async_runtime::block_on(async {
            let eng = engine()?;
            eng.apply_updates(Path::new(&wowDir), None, opts)
                .await
                .map_err(|e| e.to_string())
        })?;

        let updated = plans.iter().filter(|p| p.applied).count();
        let failed = plans.iter().filter(|p| p.error.is_some()).count();
        if failed > 0 {
            Ok(format!(
                "Done. Updated {} repo(s); {} failed.",
                updated, failed
            ))
        } else {
            Ok(format!("Done. Updated {} repo(s).", updated))
        }
    })
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
async fn wuddle_update_repo(
    id: i64,
    wowDir: String,
    useSymlinks: Option<bool>,
    setXattrComment: Option<bool>,
) -> Result<String, String> {
    let wowDir = normalize_wow_dir(wowDir)?;
    let opts = install_options(useSymlinks, setXattrComment);

    run_blocking(move || {
        let updated = tauri::async_runtime::block_on(async {
            let eng = engine()?;
            eng.update_repo(id, Path::new(&wowDir), None, opts)
                .await
                .map_err(|e| e.to_string())
        })?;

        match updated {
            Some(p) => Ok(format!("Updated {}/{} to {}.", p.owner, p.name, p.latest)),
            None => Ok("No update available.".to_string()),
        }
    })
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
async fn wuddle_reinstall_repo(
    id: i64,
    wowDir: String,
    useSymlinks: Option<bool>,
    setXattrComment: Option<bool>,
) -> Result<String, String> {
    let wowDir = normalize_wow_dir(wowDir)?;
    let opts = install_options(useSymlinks, setXattrComment);

    run_blocking(move || {
        let plan = tauri::async_runtime::block_on(async {
            let eng = engine()?;
            eng.reinstall_repo(id, Path::new(&wowDir), None, opts)
                .await
                .map_err(|e| e.to_string())
        })?;

        Ok(format!(
            "Reinstalled {}/{} from {}.",
            plan.owner, plan.name, plan.latest
        ))
    })
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
async fn wuddle_set_active_profile(profileId: String) -> Result<String, String> {
    run_blocking(move || {
        let profile_id = normalize_profile_id(&profileId);
        let mut guard = active_profile_state()
            .lock()
            .map_err(|_| "profile state lock poisoned".to_string())?;
        *guard = profile_id.clone();
        Ok(profile_id)
    })
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
async fn wuddle_delete_profile(
    profileId: String,
    removeLocalFiles: Option<bool>,
    wowDir: Option<String>,
) -> Result<String, String> {
    let profile_id = normalize_profile_id(&profileId);
    let remove_local_files = removeLocalFiles.unwrap_or(false);
    let wow_dir = normalize_optional_wow_dir(wowDir);

    run_blocking(move || {
        if remove_local_files && wow_dir.is_none() {
            return Err("wowDir is required when removeLocalFiles is true".to_string());
        }

        let mut removed_paths = 0usize;
        if remove_local_files {
            let eng = engine_for_profile(&profile_id)?;
            let repos = eng.db().list_repos().map_err(|e| e.to_string())?;
            let wow_path = wow_dir.as_deref().map(Path::new);
            for repo in repos {
                removed_paths += eng
                    .remove_repo(repo.id, wow_path, true)
                    .map_err(|e| e.to_string())?;
            }
        }

        let db_path = profile_db_main_path(&profile_id)?;
        remove_db_with_sidecars(&db_path)?;

        if let Ok(mut guard) = active_profile_state().lock() {
            if *guard == profile_id {
                *guard = DEFAULT_PROFILE_ID.to_string();
            }
        }

        if remove_local_files {
            Ok(format!(
                "Removed instance and deleted {} local path(s).",
                removed_paths
            ))
        } else {
            Ok("Removed instance.".to_string())
        }
    })
    .await
}

#[tauri::command]
async fn wuddle_github_auth_status() -> Result<GithubAuthStatus, String> {
    run_blocking(|| {
        let (keychain_available, token_stored) = match read_keychain_token() {
            Ok(Some(token)) => {
                std::env::set_var("WUDDLE_GITHUB_TOKEN", token);
                (true, true)
            }
            Ok(None) => (true, false),
            Err(_) => (false, false),
        };

        Ok(GithubAuthStatus {
            keychain_available,
            token_stored,
            env_token_present: env_token_present(),
        })
    })
    .await
}

#[tauri::command]
async fn wuddle_github_auth_set_token(token: String) -> Result<(), String> {
    run_blocking(move || {
        let token = token.trim().to_string();
        if token.is_empty() {
            return Err("GitHub token is empty".to_string());
        }

        std::env::set_var("WUDDLE_GITHUB_TOKEN", &token);
        if let Err(err) = keychain_probe_available().and_then(|_| set_keychain_token(token.clone())) {
            eprintln!("wuddle: keychain save unavailable, using env token only: {}", err);
        }

        if let Ok(eng) = engine() {
            clear_cached_github_rate_limits(&eng);
        }

        Ok(())
    })
    .await
}

#[tauri::command]
async fn wuddle_github_auth_clear_token() -> Result<(), String> {
    run_blocking(|| {
        std::env::remove_var("WUDDLE_GITHUB_TOKEN");
        if let Err(err) = clear_keychain_token() {
            eprintln!("wuddle: keychain clear unavailable, env token removed: {}", err);
        }
        Ok(())
    })
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    apply_linux_runtime_env_defaults();

    tauri::Builder::default()
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let runtime_icon =
                    tauri::image::Image::from_bytes(include_bytes!("../icons/app-icon.png")).ok();

                if let Some(icon) = runtime_icon.or_else(|| app.default_window_icon().cloned()) {
                    let _ = window.set_icon(icon);
                }
            }
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            wuddle_list_repos,
            wuddle_add_repo,
            wuddle_remove_repo,
            wuddle_set_repo_enabled,
            wuddle_check_updates,
            wuddle_update_all,
            wuddle_update_repo,
            wuddle_reinstall_repo,
            wuddle_set_active_profile,
            wuddle_delete_profile,
            wuddle_github_auth_status,
            wuddle_github_auth_set_token,
            wuddle_github_auth_clear_token
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
