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

use wuddle_engine::{AddonProbeResult, Engine, InstallMode, InstallOptions};

mod self_update;
mod tweaks;

/// Shared blocking HTTP client — reused across all fetch commands to avoid
/// repeated TLS/connection-pool setup.  15 s timeout covers most forge APIs.
fn shared_http_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("failed to build shared http client")
    })
}

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
    externally_modified: bool,
    not_modified: bool,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddonProbeOwnerRow {
    repo_id: i64,
    owner: String,
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddonProbeConflictRow {
    addon_name: String,
    target_path: String,
    owners: Vec<AddonProbeOwnerRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddonProbeResultRow {
    addon_names: Vec<String>,
    conflicts: Vec<AddonProbeConflictRow>,
    resolved_branch: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GithubAuthStatus {
    keychain_available: bool,
    token_stored: bool,
    env_token_present: bool,
    portable_mode: bool,
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
    clear_wdb: Option<bool>,
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

fn token_file_path() -> Result<PathBuf, String> {
    Ok(app_dir()?.join(".github_token"))
}

fn read_file_token() -> Result<Option<String>, String> {
    let path = token_file_path()?;
    match fs::read_to_string(&path) {
        Ok(s) => {
            let t = s.trim().to_string();
            Ok(if t.is_empty() { None } else { Some(t) })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn write_file_token(token: &str) -> Result<(), String> {
    let path = token_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, token.trim()).map_err(|e| e.to_string())
}

fn clear_file_token() -> Result<(), String> {
    let path = token_file_path()?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn read_keychain_token() -> Result<Option<String>, String> {
    if portable_mode_enabled() {
        return read_file_token();
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
        return Ok(()); // file-based storage is always available
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
        return write_file_token(&token);
    }
    keychain_call_with_timeout("saving token", move || {
        let entry = keychain_entry(KEYCHAIN_ACCOUNT_GITHUB_TOKEN)?;
        entry.set_password(&token).map_err(|e| e.to_string())
    })
}

fn clear_keychain_token() -> Result<(), String> {
    if portable_mode_enabled() {
        return clear_file_token();
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
    cache_keep_versions: Option<u32>,
) -> InstallOptions {
    InstallOptions {
        use_symlinks: use_symlinks.unwrap_or(false),
        set_xattr_comment: set_xattr_comment.unwrap_or(false),
        replace_addon_conflicts: replace_addon_conflicts.unwrap_or(false),
        cache_keep_versions: cache_keep_versions.unwrap_or(3) as usize,
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

/// One-time fix: the v4 migration lowercased all owner/name values.
/// This fetches the proper casing from each forge API and updates the DB.
fn fix_repo_casing_from_forges(eng: &Engine) {
    if !eng.db().needs_casing_fix() {
        return;
    }

    let repos = match eng.db().list_repos() {
        Ok(r) => r,
        Err(_) => {
            // Can't read repos — skip fix, will retry next startup
            return;
        }
    };

    let client = shared_http_client();
    let ua = format!("Wuddle/{}", env!("CARGO_PKG_VERSION"));
    let gh_token = wuddle_engine::github_token();

    for repo in &repos {
        let (new_owner, new_name) = match repo.forge.as_str() {
            "github" => {
                let api_url =
                    format!("https://api.github.com/repos/{}/{}", repo.owner, repo.name);
                let mut req = client
                    .get(&api_url)
                    .header("User-Agent", &ua)
                    .header("Accept", "application/vnd.github+json");
                if let Some(ref token) = gh_token {
                    req = req.bearer_auth(token);
                }
                match req.send() {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(json) = resp.json::<serde_json::Value>() {
                            let owner = json["owner"]["login"]
                                .as_str()
                                .unwrap_or(&repo.owner)
                                .to_string();
                            let name = json["name"]
                                .as_str()
                                .unwrap_or(&repo.name)
                                .to_string();
                            (owner, name)
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                }
            }
            "gitea" => {
                let api_url = format!(
                    "https://{}/api/v1/repos/{}/{}",
                    repo.host, repo.owner, repo.name
                );
                let req = client.get(&api_url).header("User-Agent", &ua);
                match req.send() {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(json) = resp.json::<serde_json::Value>() {
                            let owner = json["owner"]["login"]
                                .as_str()
                                .unwrap_or(&repo.owner)
                                .to_string();
                            let name = json["name"]
                                .as_str()
                                .unwrap_or(&repo.name)
                                .to_string();
                            (owner, name)
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                }
            }
            "gitlab" => {
                let encoded =
                    format!("{}/{}", repo.owner, repo.name).replace('/', "%2F");
                let api_url =
                    format!("https://{}/api/v4/projects/{}", repo.host, encoded);
                let req = client.get(&api_url).header("User-Agent", &ua);
                match req.send() {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(json) = resp.json::<serde_json::Value>() {
                            // path_with_namespace = "owner/name" or "group/sub/name"
                            if let Some(full_path) = json["path_with_namespace"].as_str()
                            {
                                let parts: Vec<&str> = full_path.rsplitn(2, '/').collect();
                                if parts.len() == 2 {
                                    (parts[1].to_string(), parts[0].to_string())
                                } else {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                }
            }
            _ => continue,
        };

        if new_owner != repo.owner || new_name != repo.name {
            let _ = eng.db().update_repo_casing(repo.id, &new_owner, &new_name);
        }
    }

    // Mark fix as complete regardless — best effort, don't retry on failures
    let _ = eng.db().mark_casing_fixed();
}

#[tauri::command]
#[allow(non_snake_case)]
async fn wuddle_list_repos(wowDir: Option<String>) -> Result<Vec<RepoRow>, String> {
    let wow_dir = normalize_optional_wow_dir(wowDir);

    run_blocking(move || {
        let eng = engine()?;
        if let Some(ref wow_dir) = wow_dir {
            let wow_path = Path::new(wow_dir);
            let _ = eng.import_existing_addon_git_repos(wow_path);
            let _ = eng.dedup_addon_repos_by_folder(wow_path);
        }
        fix_repo_casing_from_forges(&eng);
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
async fn wuddle_probe_addon_repo(
    url: String,
    wowDir: String,
    branch: Option<String>,
) -> Result<AddonProbeResultRow, String> {
    let wow_dir = normalize_wow_dir(wowDir)?;
    let branch = branch.map(|b| b.trim().to_string()).filter(|b| !b.is_empty());

    // Engine (rusqlite::Connection) is Send but !Sync due to internal RefCell usage, so we
    // can't hold &Engine across an .await point in a Send future. Instead we run the entire
    // operation — both the blocking engine open and the async probe — inside spawn_blocking
    // with its own block_on. This avoids the Sync constraint while keeping correct behaviour.
    run_blocking(move || {
        let eng = engine()?;
        let probed: AddonProbeResult = tauri::async_runtime::block_on(async {
            eng.probe_addon_repo_conflicts(&url, Path::new(&wow_dir), branch.as_deref())
                .await
                .map_err(|e| e.to_string())
        })?;

        Ok(AddonProbeResultRow {
            addon_names: probed.addon_names,
            conflicts: probed
                .conflicts
                .into_iter()
                .map(|c| AddonProbeConflictRow {
                    addon_name: c.addon_name,
                    target_path: c.target_path,
                    owners: c
                        .owners
                        .into_iter()
                        .map(|o| AddonProbeOwnerRow {
                            repo_id: o.repo_id,
                            owner: o.owner,
                            name: o.name,
                        })
                        .collect(),
                })
                .collect(),
            resolved_branch: probed.resolved_branch,
        })
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
async fn wuddle_check_updates(
    wowDir: Option<String>,
    checkMode: Option<String>,
) -> Result<Vec<PlanRow>, String> {
    let wow_dir = normalize_optional_wow_dir(wowDir);
    let mode = wuddle_engine::CheckMode::from_str(checkMode.as_deref().unwrap_or("force"));

    run_blocking(move || {
        let plans = tauri::async_runtime::block_on(async {
            let eng = engine()?;
            let wow_path = wow_dir.as_deref().map(Path::new);
            if let Some(wow_dir) = wow_path {
                let _ = eng.import_existing_addon_git_repos(wow_dir);
            }
            eng.check_updates_with_wow(wow_path, mode)
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
                externally_modified: p.externally_modified,
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
    cacheKeepVersions: Option<u32>,
) -> Result<String, String> {
    let wowDir = normalize_wow_dir(wowDir)?;
    let opts = install_options(useSymlinks, setXattrComment, replaceAddonConflicts, cacheKeepVersions);

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
    cacheKeepVersions: Option<u32>,
) -> Result<OperationResult, String> {
    let wowDir = normalize_wow_dir(wowDir)?;
    let opts = install_options(useSymlinks, setXattrComment, replaceAddonConflicts, cacheKeepVersions);

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
    cacheKeepVersions: Option<u32>,
) -> Result<OperationResult, String> {
    let wowDir = normalize_wow_dir(wowDir)?;
    let opts = install_options(useSymlinks, setXattrComment, replaceAddonConflicts, cacheKeepVersions);

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
            // In portable mode, use file-based token storage
            let file_token = read_file_token().unwrap_or(None);
            if let Some(ref token) = file_token {
                wuddle_engine::set_github_token(Some(token.clone()));
            } else {
                wuddle_engine::set_github_token(None);
            }
            return Ok(GithubAuthStatus {
                keychain_available: true, // file storage is always available
                token_stored: file_token.is_some(),
                env_token_present,
                portable_mode: true,
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
            portable_mode: false,
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
        package_name: "Wuddle".to_string(),
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

    if launch_cfg.clear_wdb.unwrap_or(false) {
        let wdb_path = wow_path.join("WDB");
        if wdb_path.is_dir() {
            match fs::remove_dir_all(&wdb_path) {
                Ok(_) => eprintln!("Cleared WDB cache: {}", wdb_path.display()),
                Err(e) => eprintln!("Warning: failed to clear WDB cache: {}", e),
            }
        }
    }

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

/// Build an `xdg-open` Command with AppImage-injected env vars stripped so
/// the system browser / file manager launches correctly. Uses a blacklist
/// rather than a whitelist because xdg-open and the programs it delegates to
/// (kfmclient, gio, etc.) need many DE-specific vars that are hard to enumerate.
#[cfg(all(unix, not(target_os = "macos")))]
fn clean_xdg_open(arg: &str) -> Command {
    let mut cmd = Command::new("xdg-open");
    cmd.arg(arg);
    // Remove all env vars that the AppImage runtime injects.
    for key in &[
        // AppImage runtime vars
        "APPDIR",
        "APPIMAGE",
        "ARGV0",
        "OWD",
        // Library paths pointing into the AppImage mount
        "LD_LIBRARY_PATH",
        "LD_PRELOAD",
        // GIO/GStreamer/Qt module paths bundled in the AppImage
        "GIO_MODULE_DIR",
        "GST_PLUGIN_PATH",
        "GST_PLUGIN_SYSTEM_PATH",
        "QT_PLUGIN_PATH",
        // Python paths (some AppImages bundle Python)
        "PYTHONPATH",
        "PYTHONHOME",
        // We set this ourselves for the Wayland/X11 fallback
        "GDK_BACKEND",
    ] {
        cmd.env_remove(key);
    }
    // Clean PATH so AppImage internal dirs don't shadow system binaries.
    let clean_path = std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|p| !p.contains("/tmp/.mount_"))
        .collect::<Vec<_>>()
        .join(":");
    if !clean_path.is_empty() {
        cmd.env("PATH", clean_path);
    }
    // Restore XDG_DATA_DIRS to system default if it was modified by AppImage.
    if let Ok(dirs) = std::env::var("XDG_DATA_DIRS") {
        let clean: Vec<&str> = dirs.split(':').filter(|p| !p.contains("/tmp/.mount_")).collect();
        if !clean.is_empty() {
            cmd.env("XDG_DATA_DIRS", clean.join(":"));
        } else {
            cmd.env_remove("XDG_DATA_DIRS");
        }
    }
    cmd
}

#[tauri::command]
fn wuddle_open_url(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("URL is empty.".to_string());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        clean_xdg_open(trimmed)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open URL: {}", e))
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(trimmed)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open URL: {}", e))
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", trimmed])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open URL: {}", e))
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
    let mut cmd = clean_xdg_open(trimmed);

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open directory {}: {}", path_buf.display(), e))
}

#[tauri::command]
fn wuddle_apply_tweaks(wow_dir: String, opts: tweaks::TweakOptions) -> Result<String, String> {
    let dir = PathBuf::from(wow_dir.trim());
    if !dir.is_dir() {
        return Err("Invalid WoW directory.".into());
    }
    tweaks::apply_tweaks(&dir, &opts)
}

#[tauri::command]
fn wuddle_restore_tweaks_backup(wow_dir: String) -> Result<String, String> {
    let dir = PathBuf::from(wow_dir.trim());
    if !dir.is_dir() {
        return Err("Invalid WoW directory.".into());
    }
    tweaks::restore_backup(&dir)
}

#[tauri::command]
fn wuddle_has_tweaks_backup(wow_dir: String) -> Result<bool, String> {
    let dir = PathBuf::from(wow_dir.trim());
    if !dir.is_dir() {
        return Err("Invalid WoW directory.".into());
    }
    Ok(tweaks::has_backup(&dir))
}

#[tauri::command]
fn wuddle_read_tweaks(wow_dir: String) -> Result<tweaks::ReadTweakValues, String> {
    let dir = PathBuf::from(wow_dir.trim());
    if !dir.is_dir() {
        return Err("Invalid WoW directory.".into());
    }
    tweaks::read_tweaks(&dir)
}

const CHANGELOG_EMBEDDED: &str = include_str!("../../../CHANGELOG.md");

#[tauri::command]
async fn wuddle_fetch_changelog() -> Result<String, String> {
    run_blocking(|| {
        let client = shared_http_client();

        let resp = client
            .get("https://raw.githubusercontent.com/ZythDr/Wuddle/main/CHANGELOG.md")
            .header(
                "User-Agent",
                format!("Wuddle/{}", env!("CARGO_PKG_VERSION")),
            )
            .send()
            .map_err(|e| format!("fetch changelog: {e}"))?;

        if resp.status().is_success() {
            resp.text()
                .map_err(|e| format!("read changelog body: {e}"))
        } else {
            // Fallback to embedded copy on any HTTP error.
            Ok(CHANGELOG_EMBEDDED.to_string())
        }
    })
    .await
}

#[tauri::command]
async fn wuddle_fetch_repo_readme(url: String) -> Result<String, String> {
    run_blocking(move || {
        let parsed = reqwest::Url::parse(url.trim())
            .map_err(|e| format!("invalid URL: {e}"))?;
        let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
        let segs: Vec<&str> = parsed
            .path_segments()
            .map(|s| s.collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|s: &&str| !s.is_empty())
            .collect();
        if segs.len() < 2 {
            return Err("URL must include owner and repo name".into());
        }
        let owner = segs[0];
        let name = segs[1].trim_end_matches(".git");

        let client = shared_http_client();
        let ua = format!("Wuddle/{}", env!("CARGO_PKG_VERSION"));

        if host == "github.com" {
            let api_url = format!("https://api.github.com/repos/{owner}/{name}/readme");
            let mut req = client
                .get(&api_url)
                .header("User-Agent", &ua)
                .header("Accept", "application/vnd.github.html+json");
            if let Some(token) = wuddle_engine::github_token() {
                req = req.bearer_auth(token);
            }
            let resp = req.send().map_err(|e| format!("github readme: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("README not found (HTTP {})", resp.status()));
            }
            resp.text().map_err(|e| format!("read body: {e}"))
        } else if host == "gitlab.com" {
            let encoded_path = format!("{owner}/{name}").replace('/', "%2F");
            let api_url = format!(
                "https://{host}/api/v4/projects/{encoded_path}/repository/files/README.md/raw?ref=HEAD"
            );
            let resp = client
                .get(&api_url)
                .header("User-Agent", &ua)
                .send()
                .map_err(|e| format!("gitlab readme: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("README not found (HTTP {})", resp.status()));
            }
            let md = resp.text().map_err(|e| format!("read body: {e}"))?;
            Ok(format!("<!--md-->{md}"))
        } else {
            // Gitea / Codeberg / other Gitea-compatible hosts
            let api_url =
                format!("https://{host}/api/v1/repos/{owner}/{name}/raw/README.md");
            let resp = client
                .get(&api_url)
                .header("User-Agent", &ua)
                .send()
                .map_err(|e| format!("gitea readme: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("README not found (HTTP {})", resp.status()));
            }
            let md = resp.text().map_err(|e| format!("read body: {e}"))?;
            Ok(format!("<!--md-->{md}"))
        }
    })
    .await
}

#[tauri::command]
async fn wuddle_fetch_repo_info(url: String) -> Result<String, String> {
    run_blocking(move || {
        let parsed = reqwest::Url::parse(url.trim())
            .map_err(|e| format!("invalid URL: {e}"))?;
        let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
        let segs: Vec<&str> = parsed
            .path_segments()
            .map(|s| s.collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|s: &&str| !s.is_empty())
            .collect();
        if segs.len() < 2 {
            return Err("URL must include owner and repo name".into());
        }
        let owner = segs[0];
        let name = segs[1].trim_end_matches(".git");

        let client = shared_http_client();
        let ua = format!("Wuddle/{}", env!("CARGO_PKG_VERSION"));

        if host == "github.com" {
            let api_url = format!("https://api.github.com/repos/{owner}/{name}");
            let mut req = client
                .get(&api_url)
                .header("User-Agent", &ua)
                .header("Accept", "application/vnd.github+json");
            if let Some(token) = wuddle_engine::github_token() {
                req = req.bearer_auth(token);
            }
            let resp = req.send().map_err(|e| format!("github repo info: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("repo info not found (HTTP {})", resp.status()));
            }
            let data: serde_json::Value =
                resp.json().map_err(|e| format!("parse json: {e}"))?;
            let result = serde_json::json!({
                "description": data["description"].as_str().unwrap_or(""),
                "stars": data["stargazers_count"].as_u64().unwrap_or(0),
                "forks": data["forks_count"].as_u64().unwrap_or(0),
                "language": data["language"].as_str().unwrap_or(""),
                "license": data["license"]["spdx_id"].as_str().unwrap_or(""),
                "forksUrl": format!("https://github.com/{owner}/{name}/forks"),
            });
            Ok(result.to_string())
        } else if host == "gitlab.com" {
            let encoded_path = format!("{owner}/{name}").replace('/', "%2F");
            let api_url =
                format!("https://{host}/api/v4/projects/{encoded_path}");
            let resp = client
                .get(&api_url)
                .header("User-Agent", &ua)
                .send()
                .map_err(|e| format!("gitlab repo info: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("repo info not found (HTTP {})", resp.status()));
            }
            let data: serde_json::Value =
                resp.json().map_err(|e| format!("parse json: {e}"))?;
            let result = serde_json::json!({
                "description": data["description"].as_str().unwrap_or(""),
                "stars": data["star_count"].as_u64().unwrap_or(0),
                "forks": data["forks_count"].as_u64().unwrap_or(0),
                "language": "",
                "license": "",
                "forksUrl": format!("https://{host}/{owner}/{name}/-/forks"),
            });
            Ok(result.to_string())
        } else {
            // Gitea / Codeberg
            let api_url =
                format!("https://{host}/api/v1/repos/{owner}/{name}");
            let resp = client
                .get(&api_url)
                .header("User-Agent", &ua)
                .send()
                .map_err(|e| format!("gitea repo info: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("repo info not found (HTTP {})", resp.status()));
            }
            let data: serde_json::Value =
                resp.json().map_err(|e| format!("parse json: {e}"))?;
            let result = serde_json::json!({
                "description": data["description"].as_str().unwrap_or(""),
                "stars": data["stars_count"].as_u64().unwrap_or(0),
                "forks": data["forks_count"].as_u64().unwrap_or(0),
                "language": data["language"].as_str().unwrap_or(""),
                "license": "",
                "forksUrl": format!("https://{host}/{owner}/{name}/forks"),
            });
            Ok(result.to_string())
        }
    })
    .await
}

#[tauri::command]
async fn wuddle_fetch_repo_tree(url: String) -> Result<String, String> {
    run_blocking(move || {
        let parsed = reqwest::Url::parse(url.trim())
            .map_err(|e| format!("invalid URL: {e}"))?;
        let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
        let segs: Vec<&str> = parsed
            .path_segments()
            .map(|s| s.collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|s: &&str| !s.is_empty())
            .collect();
        if segs.len() < 2 {
            return Err("URL must include owner and repo name".into());
        }
        let owner = segs[0];
        let name = segs[1].trim_end_matches(".git");

        let client = shared_http_client();
        let ua = format!("Wuddle/{}", env!("CARGO_PKG_VERSION"));

        // Fetch top-level contents only (children loaded on demand)
        let entries = fetch_contents_list(&client, &ua, &host, owner, name, "")?;
        let result: Vec<serde_json::Value> = entries
            .iter()
            .filter_map(|e| {
                let ename = e["name"].as_str().unwrap_or("");
                if ename.is_empty() { return None; }
                Some(serde_json::json!({
                    "name": ename,
                    "type": normalize_entry_type(e["type"].as_str().unwrap_or("file")),
                }))
            })
            .collect();
        serde_json::to_string(&result).map_err(|e| format!("serialize: {e}"))
    })
    .await
}

#[tauri::command]
async fn wuddle_fetch_repo_contents(url: String, path: String) -> Result<String, String> {
    run_blocking(move || {
        let parsed = reqwest::Url::parse(url.trim())
            .map_err(|e| format!("invalid URL: {e}"))?;
        let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
        let segs: Vec<&str> = parsed
            .path_segments()
            .map(|s| s.collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|s: &&str| !s.is_empty())
            .collect();
        if segs.len() < 2 {
            return Err("URL must include owner and repo name".into());
        }
        let owner = segs[0];
        let name = segs[1].trim_end_matches(".git");

        let client = shared_http_client();
        let ua = format!("Wuddle/{}", env!("CARGO_PKG_VERSION"));

        let entries = fetch_contents_list(&client, &ua, &host, owner, name, &path)?;
        let result: Vec<serde_json::Value> = entries
            .iter()
            .filter_map(|e| {
                let ename = e["name"].as_str().unwrap_or("");
                if ename.is_empty() { return None; }
                Some(serde_json::json!({
                    "name": ename,
                    "type": normalize_entry_type(e["type"].as_str().unwrap_or("file")),
                }))
            })
            .collect();
        serde_json::to_string(&result).map_err(|e| format!("serialize: {e}"))
    })
    .await
}

/// Normalize entry type across forges (GitHub uses "dir"/"file", GitLab uses "tree"/"blob").
fn normalize_entry_type(t: &str) -> &str {
    match t {
        "tree" | "dir" => "dir",
        _ => "file",
    }
}

/// Fetch directory contents from the appropriate forge API.
fn fetch_contents_list(
    client: &reqwest::blocking::Client,
    ua: &str,
    host: &str,
    owner: &str,
    name: &str,
    path: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let path_suffix = if path.is_empty() {
        String::new()
    } else {
        format!("/{path}")
    };

    if host == "github.com" {
        let api_url =
            format!("https://api.github.com/repos/{owner}/{name}/contents{path_suffix}");
        let mut req = client
            .get(&api_url)
            .header("User-Agent", ua)
            .header("Accept", "application/vnd.github+json");
        if let Some(token) = wuddle_engine::github_token() {
            req = req.bearer_auth(token);
        }
        let resp = req.send().map_err(|e| format!("github contents: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("contents not found (HTTP {})", resp.status()));
        }
        let entries: Vec<serde_json::Value> =
            resp.json().map_err(|e| format!("parse json: {e}"))?;
        Ok(entries)
    } else if host == "gitlab.com" {
        let encoded_path = format!("{owner}/{name}").replace('/', "%2F");
        let api_url = format!(
            "https://{host}/api/v4/projects/{encoded_path}/repository/tree?per_page=50&path={path}"
        );
        let resp = client
            .get(&api_url)
            .header("User-Agent", ua)
            .send()
            .map_err(|e| format!("gitlab tree: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("tree not found (HTTP {})", resp.status()));
        }
        let entries: Vec<serde_json::Value> =
            resp.json().map_err(|e| format!("parse json: {e}"))?;
        Ok(entries)
    } else {
        // Gitea / Codeberg
        let api_url =
            format!("https://{host}/api/v1/repos/{owner}/{name}/contents{path_suffix}");
        let resp = client
            .get(&api_url)
            .header("User-Agent", ua)
            .send()
            .map_err(|e| format!("gitea contents: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("contents not found (HTTP {})", resp.status()));
        }
        let entries: Vec<serde_json::Value> =
            resp.json().map_err(|e| format!("parse json: {e}"))?;
        Ok(entries)
    }
}

#[tauri::command]
async fn wuddle_fetch_repo_releases(url: String) -> Result<String, String> {
    run_blocking(move || {
        let parsed = reqwest::Url::parse(url.trim())
            .map_err(|e| format!("invalid URL: {e}"))?;
        let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
        let segs: Vec<&str> = parsed
            .path_segments()
            .map(|s| s.collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|s: &&str| !s.is_empty())
            .collect();
        if segs.len() < 2 {
            return Err("URL must include owner and repo name".into());
        }
        let owner = segs[0];
        let name = segs[1].trim_end_matches(".git");

        let client = shared_http_client();
        let ua = format!("Wuddle/{}", env!("CARGO_PKG_VERSION"));

        let releases: Vec<serde_json::Value> = if host == "github.com" {
            let api_url =
                format!("https://api.github.com/repos/{owner}/{name}/releases?per_page=20");
            let mut req = client
                .get(&api_url)
                .header("User-Agent", &ua)
                .header("Accept", "application/vnd.github+json");
            if let Some(token) = wuddle_engine::github_token() {
                req = req.bearer_auth(token);
            }
            let resp = req.send().map_err(|e| format!("github releases: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("releases not found (HTTP {})", resp.status()));
            }
            let items: Vec<serde_json::Value> =
                resp.json().map_err(|e| format!("parse json: {e}"))?;
            items
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "tag": r["tag_name"].as_str().unwrap_or(""),
                        "name": r["name"].as_str().unwrap_or(""),
                        "body": r["body"].as_str().unwrap_or(""),
                        "publishedAt": r["published_at"].as_str().unwrap_or(""),
                        "prerelease": r["prerelease"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        } else if host == "gitlab.com" {
            let encoded_path = format!("{owner}/{name}").replace('/', "%2F");
            let api_url = format!(
                "https://{host}/api/v4/projects/{encoded_path}/releases?per_page=20"
            );
            let resp = client
                .get(&api_url)
                .header("User-Agent", &ua)
                .send()
                .map_err(|e| format!("gitlab releases: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("releases not found (HTTP {})", resp.status()));
            }
            let items: Vec<serde_json::Value> =
                resp.json().map_err(|e| format!("parse json: {e}"))?;
            items
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "tag": r["tag_name"].as_str().unwrap_or(""),
                        "name": r["name"].as_str().unwrap_or(""),
                        "body": r["description"].as_str().unwrap_or(""),
                        "publishedAt": r["released_at"].as_str().unwrap_or(""),
                        "prerelease": false,
                    })
                })
                .collect()
        } else {
            // Gitea / Codeberg
            let api_url =
                format!("https://{host}/api/v1/repos/{owner}/{name}/releases?limit=20");
            let resp = client
                .get(&api_url)
                .header("User-Agent", &ua)
                .send()
                .map_err(|e| format!("gitea releases: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("releases not found (HTTP {})", resp.status()));
            }
            let items: Vec<serde_json::Value> =
                resp.json().map_err(|e| format!("parse json: {e}"))?;
            items
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "tag": r["tag_name"].as_str().unwrap_or(""),
                        "name": r["name"].as_str().unwrap_or(""),
                        "body": r["body"].as_str().unwrap_or(""),
                        "publishedAt": r["published_at"].as_str().unwrap_or(""),
                        "prerelease": r["prerelease"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        };

        serde_json::to_string(&releases).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn wuddle_fetch_repo_file(url: String, path: String) -> Result<String, String> {
    run_blocking(move || {
        if path.contains("..") {
            return Err("invalid path".into());
        }
        let parsed = reqwest::Url::parse(url.trim())
            .map_err(|e| format!("invalid URL: {e}"))?;
        let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
        let segs: Vec<&str> = parsed
            .path_segments()
            .map(|s| s.collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|s: &&str| !s.is_empty())
            .collect();
        if segs.len() < 2 {
            return Err("URL must include owner and repo name".into());
        }
        let owner = segs[0];
        let name = segs[1].trim_end_matches(".git");

        let client = shared_http_client();
        let ua = format!("Wuddle/{}", env!("CARGO_PKG_VERSION"));

        let resp = if host == "github.com" {
            let api_url = format!(
                "https://api.github.com/repos/{owner}/{name}/contents/{path}"
            );
            let mut req = client
                .get(&api_url)
                .header("User-Agent", &ua)
                .header("Accept", "application/vnd.github.raw+json");
            if let Some(token) = wuddle_engine::github_token() {
                req = req.bearer_auth(token);
            }
            req.send().map_err(|e| format!("github file: {e}"))?
        } else if host == "gitlab.com" {
            let encoded_path = format!("{owner}/{name}").replace('/', "%2F");
            let encoded_file = path.replace('/', "%2F");
            let api_url = format!(
                "https://{host}/api/v4/projects/{encoded_path}/repository/files/{encoded_file}/raw?ref=HEAD"
            );
            client
                .get(&api_url)
                .header("User-Agent", &ua)
                .send()
                .map_err(|e| format!("gitlab file: {e}"))?
        } else {
            let api_url = format!(
                "https://{host}/api/v1/repos/{owner}/{name}/raw/{path}"
            );
            client
                .get(&api_url)
                .header("User-Agent", &ua)
                .send()
                .map_err(|e| format!("gitea file: {e}"))?
        };

        if !resp.status().is_success() {
            return Err(format!("file not found (HTTP {})", resp.status()));
        }
        let len = resp.content_length().unwrap_or(0);
        if len > 512 * 1024 {
            return Err("File too large to preview".into());
        }
        let text = resp.text().map_err(|e| format!("read body: {e}"))?;
        if text.len() > 512 * 1024 {
            return Err("File too large to preview".into());
        }
        Ok(text)
    })
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
async fn wuddle_read_local_file(
    wowDir: String,
    path: String,
) -> Result<String, String> {
    run_blocking(move || {
        if path.contains("..") {
            return Err("invalid path".into());
        }
        let full = Path::new(&wowDir).join(&path);
        let meta = fs::metadata(&full).map_err(|e| format!("stat: {e}"))?;
        if meta.len() > 512 * 1024 {
            return Err("File too large to preview".into());
        }
        let bytes = fs::read(&full).map_err(|e| format!("read: {e}"))?;
        // Binary guard: check first 8KB for null bytes
        let check_len = bytes.len().min(8192);
        if bytes[..check_len].contains(&0u8) {
            return Err("Binary file \u{2014} cannot preview".into());
        }
        String::from_utf8(bytes).map_err(|_| "File is not valid UTF-8 text".into())
    })
    .await
}

#[tauri::command]
async fn wuddle_list_repo_installs(id: i64) -> Result<String, String> {
    run_blocking(move || {
        let eng = engine()?;
        let installs = eng.db().list_installs(id).map_err(|e| e.to_string())?;
        let entries: Vec<serde_json::Value> = installs
            .into_iter()
            .map(|e| {
                serde_json::json!({
                    "path": e.path,
                    "kind": e.kind,
                })
            })
            .collect();
        serde_json::to_string(&entries).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
async fn wuddle_list_local_files(
    wowDir: String,
    basePath: String,
) -> Result<String, String> {
    run_blocking(move || {
        if basePath.contains("..") {
            return Err("invalid path".into());
        }
        let full = Path::new(&wowDir).join(&basePath);
        if !full.is_dir() {
            return Err("not a directory".into());
        }
        let mut entries: Vec<serde_json::Value> = Vec::new();
        let mut count = 0u32;
        for entry in fs::read_dir(&full).map_err(|e| e.to_string())? {
            if count >= 100 {
                break;
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().to_string();
            let ft = entry.file_type().map_err(|e| e.to_string())?;
            let kind = if ft.is_dir() { "dir" } else { "file" };
            entries.push(serde_json::json!({ "name": name, "type": kind }));
            count += 1;
        }
        // Sort: dirs first, then files, alphabetical within each group
        entries.sort_by(|a, b| {
            let a_type = a["type"].as_str().unwrap_or("");
            let b_type = b["type"].as_str().unwrap_or("");
            let a_dir = a_type == "dir";
            let b_dir = b_type == "dir";
            if a_dir != b_dir {
                return if a_dir {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }
            let a_name = a["name"].as_str().unwrap_or("");
            let b_name = b["name"].as_str().unwrap_or("");
            a_name.to_ascii_lowercase().cmp(&b_name.to_ascii_lowercase())
        });
        serde_json::to_string(&entries).map_err(|e| e.to_string())
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
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            wuddle_list_repos,
            wuddle_add_repo,
            wuddle_probe_addon_repo,
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
            wuddle_open_url,
            wuddle_open_directory,
            wuddle_apply_tweaks,
            wuddle_restore_tweaks_backup,
            wuddle_has_tweaks_backup,
            wuddle_read_tweaks,
            wuddle_fetch_changelog,
            wuddle_fetch_repo_readme,
            wuddle_fetch_repo_info,
            wuddle_fetch_repo_tree,
            wuddle_fetch_repo_contents,
            wuddle_fetch_repo_releases,
            wuddle_fetch_repo_file,
            wuddle_read_local_file,
            wuddle_list_repo_installs,
            wuddle_list_local_files
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
