use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{mpsc, Mutex, OnceLock},
    time::Duration,
};
use tauri::Manager;

use wuddle_engine::{Engine, InstallMode, InstallOptions};

mod self_update;

#[derive(Serialize)]
struct RepoRow {
    id: i64,
    forge: String,
    owner: String,
    name: String,
    mode: String,
    enabled: bool,
    url: String,
    #[serde(rename = "gitBranch")]
    git_branch: Option<String>,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AboutInfo {
    app_version: String,
    package_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchDiagnostics {
    ready: bool,
    message: String,
    hint: Option<String>,
    target_executable: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationResult {
    message: String,
    steps: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LaunchConfig {
    method: Option<String>,
    executable_path: Option<String>,
    lutris_target: Option<String>,
    wine_command: Option<String>,
    wine_args: Option<String>,
    custom_command: Option<String>,
    custom_args: Option<String>,
    working_dir: Option<String>,
    env: Option<HashMap<String, String>>,
}

const KEYCHAIN_SERVICE: &str = "wuddle";
const KEYCHAIN_ACCOUNT_GITHUB_TOKEN: &str = "github_token";
const KEYCHAIN_ACCOUNT_PROBE: &str = "github_token_probe";
const DEFAULT_PROFILE_ID: &str = "default";
const KEYCHAIN_TIMEOUT_MS: u64 = 2500;

static ACTIVE_PROFILE_ID: OnceLock<Mutex<String>> = OnceLock::new();
static KEYCHAIN_SYNC_ATTEMPTED: OnceLock<Mutex<bool>> = OnceLock::new();
static LEGACY_PORTABLE_MIGRATION_ATTEMPTED: OnceLock<Mutex<bool>> = OnceLock::new();

fn active_profile_state() -> &'static Mutex<String> {
    ACTIVE_PROFILE_ID.get_or_init(|| Mutex::new(DEFAULT_PROFILE_ID.to_string()))
}

fn keychain_sync_attempted_state() -> &'static Mutex<bool> {
    KEYCHAIN_SYNC_ATTEMPTED.get_or_init(|| Mutex::new(false))
}

fn legacy_portable_migration_attempted_state() -> &'static Mutex<bool> {
    LEGACY_PORTABLE_MIGRATION_ATTEMPTED.get_or_init(|| Mutex::new(false))
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
    let dir = if portable_mode_enabled() {
        portable_app_dir()?
    } else {
        let dir = standard_app_dir()?;
        if let Err(err) = migrate_legacy_portable_dbs_once(&dir) {
            eprintln!("wuddle: portable data migration skipped: {}", err);
        }
        dir
    };
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
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
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            v == "1" || v == "true" || v == "yes" || v == "on"
        })
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
        .ok_or_else(|| "no executable parent dir".to_string())?
        .to_path_buf();

    let Some(version_dir) = exe_dir.parent() else {
        return Ok(exe_dir);
    };
    let Some(maybe_versions) = version_dir.file_name().and_then(|s| s.to_str()) else {
        return Ok(exe_dir);
    };
    if !maybe_versions.eq_ignore_ascii_case("versions") {
        return Ok(exe_dir);
    }

    let root = version_dir
        .parent()
        .ok_or_else(|| "no launcher root dir".to_string())?;
    Ok(root.to_path_buf())
}

fn is_sqlite_payload(name: &str) -> bool {
    name == "wuddle.sqlite"
        || (name.starts_with("wuddle-") && name.ends_with(".sqlite"))
        || name == "wuddle.sqlite-wal"
        || name == "wuddle.sqlite-shm"
        || (name.starts_with("wuddle-") && name.ends_with(".sqlite-wal"))
        || (name.starts_with("wuddle-") && name.ends_with(".sqlite-shm"))
}

fn migrate_legacy_portable_dbs_once(target_dir: &Path) -> Result<(), String> {
    let already_attempted = match legacy_portable_migration_attempted_state().lock() {
        Ok(guard) => *guard,
        Err(_) => true,
    };
    if already_attempted {
        return Ok(());
    }
    if let Ok(mut guard) = legacy_portable_migration_attempted_state().lock() {
        *guard = true;
    }
    migrate_legacy_portable_dbs(target_dir)
}

fn migrate_legacy_portable_dbs(target_dir: &Path) -> Result<(), String> {
    let legacy_dir = portable_app_dir()?;
    if legacy_dir == target_dir {
        return Ok(());
    }
    if !legacy_dir.exists() || !legacy_dir.is_dir() {
        return Ok(());
    }

    fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;

    let mut copied = 0usize;
    let entries = fs::read_dir(&legacy_dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let src = entry.path();
        if !src.is_file() {
            continue;
        }
        let Some(file_name_os) = src.file_name() else {
            continue;
        };
        let file_name = file_name_os.to_string_lossy();
        if !is_sqlite_payload(&file_name) {
            continue;
        }

        let dst = target_dir.join(file_name_os);
        if dst.exists() {
            continue;
        }

        fs::copy(&src, &dst).map_err(|e| {
            format!(
                "failed to copy {} -> {} ({})",
                src.display(),
                dst.display(),
                e
            )
        })?;
        copied += 1;
    }

    if copied > 0 {
        eprintln!(
            "wuddle: migrated {} legacy portable data file(s) to {}",
            copied,
            target_dir.display()
        );
    }

    Ok(())
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

fn env_token() -> Option<String> {
    std::env::var("WUDDLE_GITHUB_TOKEN")
        .ok()
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn env_token_present() -> bool {
    env_token().is_some()
}

fn read_keychain_token() -> Result<Option<String>, String> {
    if portable_mode_enabled() {
        return Ok(None);
    }
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
    if portable_mode_enabled() {
        return Err("system keychain disabled in portable mode".to_string());
    }
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
    if portable_mode_enabled() {
        return Err("system keychain disabled in portable mode".to_string());
    }
    keychain_call_with_timeout("saving token", move || {
        let entry = keychain_entry(KEYCHAIN_ACCOUNT_GITHUB_TOKEN)?;
        entry.set_password(&token).map_err(|e| e.to_string())
    })
}

fn clear_keychain_token() -> Result<(), String> {
    if portable_mode_enabled() {
        return Ok(());
    }
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

fn sync_github_token_from_sources() {
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
        wuddle_engine::set_github_token(Some(token));
        return;
    }

    if env_token().is_some() {
        // Keep engine token unset so engine-side env fallback is used.
        wuddle_engine::set_github_token(None);
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
    sync_github_token_from_sources();
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

fn install_options(
    use_symlinks: Option<bool>,
    set_xattr_comment: Option<bool>,
    replace_addon_conflicts: Option<bool>,
) -> InstallOptions {
    InstallOptions {
        use_symlinks: use_symlinks.unwrap_or(false),
        set_xattr_comment: set_xattr_comment.unwrap_or(false),
        replace_addon_conflicts: replace_addon_conflicts.unwrap_or(false),
    }
}

fn expand_install_path(wow_dir: &str, path: &str) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        return p.display().to_string();
    }
    Path::new(wow_dir).join(p).display().to_string()
}

#[cfg(target_os = "linux")]
fn apply_linux_runtime_env_defaults() {
    let is_appimage =
        std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some();
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
#[allow(non_snake_case)]
async fn wuddle_list_repos(wowDir: Option<String>) -> Result<Vec<RepoRow>, String> {
    let wow_dir = normalize_optional_wow_dir(wowDir);

    run_blocking(move || {
        let eng = engine()?;
        if let Some(ref wow_dir) = wow_dir {
            let _ = eng.import_existing_addon_git_repos(Path::new(wow_dir));
        }
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
                git_branch: r.git_branch,
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
            if let Some(wow_dir) = wow_path {
                let _ = eng.import_existing_addon_git_repos(wow_dir);
            }
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
    replaceAddonConflicts: Option<bool>,
) -> Result<String, String> {
    let wowDir = normalize_wow_dir(wowDir)?;
    let opts = install_options(useSymlinks, setXattrComment, replaceAddonConflicts);

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
    replaceAddonConflicts: Option<bool>,
) -> Result<OperationResult, String> {
    let wowDir = normalize_wow_dir(wowDir)?;
    let opts = install_options(useSymlinks, setXattrComment, replaceAddonConflicts);

    run_blocking(move || {
        let eng = engine()?;
        let repo = eng.db().get_repo(id).map_err(|e| e.to_string())?;
        let mut steps: Vec<String> = Vec::new();
        steps.push(format!(
            "{}/{}: update requested (mode: {}).",
            repo.owner,
            repo.name,
            repo.mode.as_str()
        ));
        steps.push(format!(
            "{}/{}: source: {}",
            repo.owner, repo.name, repo.url
        ));
        if repo.mode.as_str() == "addon_git" {
            let branch = repo
                .git_branch
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("master");
            steps.push(format!(
                "{}/{}: syncing git branch '{}'.",
                repo.owner, repo.name, branch
            ));
        } else {
            steps.push(format!(
                "{}/{}: checking release assets.",
                repo.owner, repo.name
            ));
        }

        let updated = tauri::async_runtime::block_on(async {
            eng.update_repo(id, Path::new(&wowDir), None, opts)
                .await
                .map_err(|e| e.to_string())
        })?;

        match updated {
            Some(p) => {
                if p.mode.as_str() == "addon_git" {
                    steps.push(format!("{}/{}: repository sync complete.", p.owner, p.name));
                } else {
                    if !p.asset_name.is_empty() {
                        steps.push(format!(
                            "{}/{}: selected asset '{}'.",
                            p.owner, p.name, p.asset_name
                        ));
                    }
                    if !p.asset_url.is_empty() {
                        steps.push(format!(
                            "{}/{}: downloading from {}.",
                            p.owner, p.name, p.asset_url
                        ));
                    }
                    if p.asset_name.to_ascii_lowercase().ends_with(".zip") {
                        steps.push(format!(
                            "{}/{}: extracting archive '{}'.",
                            p.owner, p.name, p.asset_name
                        ));
                    }
                }

                let installs = eng.db().list_installs(id).map_err(|e| e.to_string())?;
                for entry in installs {
                    let full = expand_install_path(&wowDir, &entry.path);
                    steps.push(format!(
                        "{}/{}: target [{}] {}",
                        p.owner, p.name, entry.kind, full
                    ));
                }
                steps.push(format!("{}/{}: install complete.", p.owner, p.name));
                Ok(OperationResult {
                    message: format!("Updated {}/{} to {}.", p.owner, p.name, p.latest),
                    steps,
                })
            }
            None => Ok(OperationResult {
                message: "No update available.".to_string(),
                steps,
            }),
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
    replaceAddonConflicts: Option<bool>,
) -> Result<OperationResult, String> {
    let wowDir = normalize_wow_dir(wowDir)?;
    let opts = install_options(useSymlinks, setXattrComment, replaceAddonConflicts);

    run_blocking(move || {
        let eng = engine()?;
        let repo = eng.db().get_repo(id).map_err(|e| e.to_string())?;
        let mut steps: Vec<String> = Vec::new();
        steps.push(format!(
            "{}/{}: reinstall requested (mode: {}).",
            repo.owner,
            repo.name,
            repo.mode.as_str()
        ));
        steps.push(format!(
            "{}/{}: source: {}",
            repo.owner, repo.name, repo.url
        ));

        let plan = tauri::async_runtime::block_on(async {
            eng.reinstall_repo(id, Path::new(&wowDir), None, opts)
                .await
                .map_err(|e| e.to_string())
        })?;

        let installs = eng.db().list_installs(id).map_err(|e| e.to_string())?;
        for entry in installs {
            let full = expand_install_path(&wowDir, &entry.path);
            steps.push(format!(
                "{}/{}: target [{}] {}",
                plan.owner, plan.name, entry.kind, full
            ));
        }
        steps.push(format!("{}/{}: reinstall complete.", plan.owner, plan.name));

        Ok(OperationResult {
            message: format!(
                "Reinstalled {}/{} from {}.",
                plan.owner, plan.name, plan.latest
            ),
            steps,
        })
    })
    .await
}

#[tauri::command]
async fn wuddle_list_repo_branches(id: i64) -> Result<Vec<String>, String> {
    run_blocking(move || {
        let eng = engine()?;
        eng.list_repo_branches(id).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn wuddle_set_repo_branch(id: i64, branch: Option<String>) -> Result<String, String> {
    run_blocking(move || {
        let eng = engine()?;
        let normalized = branch
            .map(|b| b.trim().to_string())
            .filter(|b| !b.is_empty());
        eng.set_repo_git_branch(id, normalized.clone())
            .map_err(|e| e.to_string())?;
        Ok(match normalized {
            Some(b) => format!("Set branch to {}.", b),
            None => "Using default remote branch.".to_string(),
        })
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
        let env_token_present = env_token_present();
        if portable_mode_enabled() {
            wuddle_engine::set_github_token(None);
            return Ok(GithubAuthStatus {
                keychain_available: false,
                token_stored: false,
                env_token_present,
            });
        }

        let (keychain_available, token_stored) = match read_keychain_token() {
            Ok(Some(token)) => {
                wuddle_engine::set_github_token(Some(token));
                (true, true)
            }
            Ok(None) => {
                wuddle_engine::set_github_token(None);
                (true, false)
            }
            Err(_) => {
                wuddle_engine::set_github_token(None);
                (false, false)
            }
        };

        Ok(GithubAuthStatus {
            keychain_available,
            token_stored,
            env_token_present,
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

        wuddle_engine::set_github_token(Some(token.clone()));
        if let Err(err) = keychain_probe_available().and_then(|_| set_keychain_token(token.clone()))
        {
            eprintln!(
                "wuddle: keychain save unavailable, using in-memory token only: {}",
                err
            );
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
        wuddle_engine::set_github_token(None);
        if let Err(err) = clear_keychain_token() {
            eprintln!("wuddle: keychain clear unavailable: {}", err);
        }
        Ok(())
    })
    .await
}

#[tauri::command]
fn wuddle_about_info() -> AboutInfo {
    AboutInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        package_name: env!("CARGO_PKG_NAME").to_string(),
    }
}

#[tauri::command]
async fn wuddle_self_update_info() -> Result<self_update::SelfUpdateInfo, String> {
    run_blocking(|| self_update::update_info(env!("CARGO_PKG_VERSION"))).await
}

#[tauri::command]
async fn wuddle_self_update_apply() -> Result<OperationResult, String> {
    run_blocking(|| self_update::apply_update(env!("CARGO_PKG_VERSION"))).await
}

#[tauri::command]
fn wuddle_self_update_restart() -> Result<(), String> {
    self_update::restart_after_update()
}

fn first_existing_file(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn parse_arg_string(raw: &str) -> Vec<String> {
    raw.split_whitespace().map(|s| s.to_string()).collect()
}

fn normalize_working_dir(wow_path: &Path, override_dir: Option<&str>) -> PathBuf {
    let raw = override_dir.unwrap_or("").trim();
    if raw.is_empty() {
        return wow_path.to_path_buf();
    }
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        candidate
    } else {
        wow_path.join(candidate)
    }
}

fn spawn_launch_command(
    program: &str,
    args: &[String],
    cwd: &Path,
    env_map: Option<&HashMap<String, String>>,
) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(cwd);
    if let Some(env_map) = env_map {
        for (k, v) in env_map {
            let key = k.trim();
            if key.is_empty() {
                continue;
            }
            cmd.env(key, v);
        }
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch {}: {}", program, e))
}

fn resolve_launch_target(wow_path: &Path, launch_cfg: &LaunchConfig) -> Result<PathBuf, String> {
    let explicit_exe = launch_cfg
        .executable_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);

    if let Some(raw) = explicit_exe {
        let candidate = if raw.is_absolute() {
            raw
        } else {
            wow_path.join(raw)
        };
        if !candidate.exists() {
            return Err(format!(
                "Configured executable does not exist: {}",
                candidate.display()
            ));
        }
        if !candidate.is_file() {
            return Err(format!(
                "Configured executable path is not a file: {}",
                candidate.display()
            ));
        }
        return Ok(candidate);
    }

    first_existing_file(wow_path, &["VanillaFixes.exe", "vanillafixes.exe"])
        .or_else(|| first_existing_file(wow_path, &["Wow.exe", "wow.exe", "WoW.exe"]))
        .ok_or_else(|| {
            format!(
                "No launcher found in {} (expected VanillaFixes.exe or Wow.exe).",
                wow_path.display()
            )
        })
}

#[tauri::command]
#[allow(non_snake_case)]
fn wuddle_launch_diagnostics(wowDir: String, launch: Option<LaunchConfig>) -> LaunchDiagnostics {
    let trimmed = wowDir.trim();
    if trimmed.is_empty() {
        return LaunchDiagnostics {
            ready: false,
            message: "WoW path is empty.".to_string(),
            hint: Some("Open Settings and configure this instance path.".to_string()),
            target_executable: None,
        };
    }

    let wow_path = PathBuf::from(trimmed);
    if !wow_path.exists() {
        return LaunchDiagnostics {
            ready: false,
            message: format!("WoW path does not exist: {}", wow_path.display()),
            hint: Some("Open Settings and choose the correct game executable path.".to_string()),
            target_executable: None,
        };
    }
    if !wow_path.is_dir() {
        return LaunchDiagnostics {
            ready: false,
            message: format!("WoW path is not a directory: {}", wow_path.display()),
            hint: Some(
                "Set WoW path to the game executable (Wow.exe/VanillaFixes.exe) or the game folder."
                    .to_string(),
            ),
            target_executable: None,
        };
    }

    let launch_cfg = launch.unwrap_or_default();
    match resolve_launch_target(&wow_path, &launch_cfg) {
        Ok(target) => LaunchDiagnostics {
            ready: true,
            message: format!("Ready to launch: {}", target.display()),
            hint: None,
            target_executable: Some(target.display().to_string()),
        },
        Err(err) => LaunchDiagnostics {
            ready: false,
            message: err,
            hint: Some(
                "This usually means the selected path is not a valid WoW install folder. Check instance Settings."
                    .to_string(),
            ),
            target_executable: None,
        },
    }
}

#[tauri::command]
#[allow(non_snake_case)]
fn wuddle_launch_game(wowDir: String, launch: Option<LaunchConfig>) -> Result<String, String> {
    let trimmed = wowDir.trim();
    if trimmed.is_empty() {
        return Err("WoW directory is empty.".to_string());
    }

    let wow_path = PathBuf::from(trimmed);
    if !wow_path.exists() {
        return Err(format!(
            "WoW directory does not exist: {}",
            wow_path.display()
        ));
    }
    if !wow_path.is_dir() {
        return Err(format!(
            "WoW path is not a directory: {}",
            wow_path.display()
        ));
    }

    let launch_cfg = launch.unwrap_or_default();
    let target = resolve_launch_target(&wow_path, &launch_cfg)?;
    let target_name = target
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "game executable".to_string());
    let target_str = target.to_string_lossy().to_string();

    let method = launch_cfg
        .method
        .as_deref()
        .unwrap_or("auto")
        .trim()
        .to_ascii_lowercase();
    let cwd = normalize_working_dir(&wow_path, launch_cfg.working_dir.as_deref());
    let env_map = launch_cfg.env.as_ref();

    if method == "lutris" {
        let command = launch_cfg
            .custom_command
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("lutris");
        let target_arg = launch_cfg
            .lutris_target
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                "Lutris launch target is empty (expected e.g. lutris:rungameid/2).".to_string()
            })?;
        let mut args = vec![target_arg.to_string()];
        args.extend(parse_arg_string(
            launch_cfg.custom_args.as_deref().unwrap_or(""),
        ));
        spawn_launch_command(command, &args, &cwd, env_map)?;
        return Ok(format!("Launched {} via {}.", target_name, command));
    }

    if method == "wine" {
        let command = launch_cfg
            .wine_command
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("wine");
        let mut args = parse_arg_string(launch_cfg.wine_args.as_deref().unwrap_or(""));
        args.push(target_str);
        spawn_launch_command(command, &args, &cwd, env_map)?;
        return Ok(format!("Launched {} via {}.", target_name, command));
    }

    if method == "custom" {
        let command = launch_cfg
            .custom_command
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Custom launch command is empty.".to_string())?;
        let mut args = parse_arg_string(launch_cfg.custom_args.as_deref().unwrap_or(""));
        let mut inserted_exe = false;
        for arg in &mut args {
            if arg.contains("{exe}") {
                *arg = arg.replace("{exe}", &target_str);
                inserted_exe = true;
            }
            if arg.contains("{wow_dir}") {
                *arg = arg.replace("{wow_dir}", wow_path.to_string_lossy().as_ref());
            }
        }
        if !inserted_exe {
            args.push(target_str);
        }
        spawn_launch_command(command, &args, &cwd, env_map)?;
        return Ok(format!("Launched {} via custom command.", target_name));
    }

    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new(&target);
        cmd.current_dir(&cwd);
        if let Some(env_map) = env_map {
            for (k, v) in env_map {
                let key = k.trim();
                if key.is_empty() {
                    continue;
                }
                cmd.env(key, v);
            }
        }
        cmd.spawn()
            .map_err(|e| format!("Failed to launch {}: {}", target.display(), e))?;
        return Ok(format!("Launched {}.", target_name));
    }

    #[cfg(target_os = "macos")]
    {
        if spawn_launch_command(
            "wine",
            &vec![target.to_string_lossy().to_string()],
            &cwd,
            env_map,
        )
        .is_ok()
        {
            return Ok(format!("Launched {} via wine.", target_name));
        }
        spawn_launch_command(
            "open",
            &vec![target.to_string_lossy().to_string()],
            &cwd,
            env_map,
        )?;
        return Ok(format!("Launched {} via open.", target_name));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if spawn_launch_command(
            "wine",
            &vec![target.to_string_lossy().to_string()],
            &cwd,
            env_map,
        )
        .is_ok()
        {
            return Ok(format!("Launched {} via wine.", target_name));
        }
        if spawn_launch_command(
            "xdg-open",
            &vec![target.to_string_lossy().to_string()],
            &cwd,
            env_map,
        )
        .is_ok()
        {
            return Ok(format!("Launched {} via system handler.", target_name));
        }
        return Err(format!(
            "Failed to launch {}. Install wine or configure an .exe handler.",
            target.display()
        ));
    }
}

#[tauri::command]
fn wuddle_open_directory(path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Path is empty.".to_string());
    }

    let path_buf = PathBuf::from(trimmed);
    if !path_buf.exists() {
        return Err(format!("Path does not exist: {}", path_buf.display()));
    }
    if !path_buf.is_dir() {
        return Err(format!("Path is not a directory: {}", path_buf.display()));
    }

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("explorer");
        c.arg(&path_buf);
        c
    };

    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.arg(&path_buf);
        c
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(&path_buf);
        c
    };

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open directory {}: {}", path_buf.display(), e))
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
            wuddle_list_repo_branches,
            wuddle_set_repo_branch,
            wuddle_set_active_profile,
            wuddle_delete_profile,
            wuddle_github_auth_status,
            wuddle_github_auth_set_token,
            wuddle_github_auth_clear_token,
            wuddle_about_info,
            wuddle_self_update_info,
            wuddle_self_update_apply,
            wuddle_self_update_restart,
            wuddle_launch_diagnostics,
            wuddle_launch_game,
            wuddle_open_directory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
