//! Thin async wrappers around wuddle-engine.
//! Every function opens a fresh Engine (it's Send+!Sync due to rusqlite).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;
use wuddle_engine::{CheckMode, Engine, InstallMode, InstallOptions, Repo, UpdatePlan};
use reqwest::Client;
use serde::Deserialize;
use iced;

// ---------------------------------------------------------------------------
// Row types for the UI (Clone-friendly, owned data)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RepoRow {
    pub id: i64,
    pub forge: String,
    pub owner: String,
    pub name: String,
    pub url: String,
    pub mode: String,
    pub enabled: bool,
    pub last_version: Option<String>,
    pub git_branch: Option<String>,
    /// DLL files managed by this repo: (filename, is_enabled_in_dlls_txt).
    /// Empty for non-DLL repos. More than one entry means this is a multi-DLL mod.
    pub installed_dlls: Vec<(String, bool)>,
}

impl From<Repo> for RepoRow {
    fn from(r: Repo) -> Self {
        // Normalize legacy "gitea" label for well-known hosts with their own brand.
        let forge = if r.forge == "gitea" && r.host.eq_ignore_ascii_case("codeberg.org") {
            "codeberg".to_string()
        } else {
            r.forge
        };
        Self {
            id: r.id,
            forge,
            owner: r.owner,
            name: r.name,
            url: r.url,
            mode: r.mode.as_str().to_string(),
            enabled: r.enabled,
            last_version: r.last_version,
            git_branch: r.git_branch,
            installed_dlls: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlanRow {
    pub repo_id: i64,
    pub owner: String,
    pub name: String,
    pub current: Option<String>,
    pub latest: String,
    pub asset_name: String,
    pub has_update: bool,
    pub repair_needed: bool,
    pub error: Option<String>,
}

impl From<UpdatePlan> for PlanRow {
    fn from(p: UpdatePlan) -> Self {
        // Use the engine's authoritative signal: asset_url is non-empty iff something
        // needs to be downloaded. Exclude repair_needed (files missing but version
        // current) since that is not an "update". Mirrors Tauri's !p.asset_url.is_empty().
        let has_update = !p.asset_url.is_empty() && !p.repair_needed && p.error.is_none();
        Self {
            repo_id: p.repo_id,
            owner: p.owner,
            name: p.name,
            current: p.current,
            latest: p.latest,
            asset_name: p.asset_name,
            has_update,
            repair_needed: p.repair_needed,
            error: p.error,
        }
    }
}

// ---------------------------------------------------------------------------
// Engine helpers
// ---------------------------------------------------------------------------

fn open_engine(db_path: Option<&Path>) -> Result<Engine, String> {
    match db_path {
        Some(p) => Engine::open(p).map_err(|e| e.to_string()),
        None => Engine::open_default().map_err(|e| e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Repo queries
// ---------------------------------------------------------------------------

pub async fn list_repos(
    db_path: Option<PathBuf>,
    wow_dir: Option<String>,
) -> Result<Vec<RepoRow>, String> {
    tokio::task::spawn_blocking(move || {
        // No wow_dir means no WoW installation configured — return empty list
        let dir = match wow_dir.as_deref() {
            Some(d) if !d.trim().is_empty() => d,
            _ => return Ok(Vec::new()),
        };
        let eng = open_engine(db_path.as_deref())?;
        let wow_path = Path::new(dir);
        // Prune repos whose files no longer exist on disk (DB only, never deletes files)
        let _ = eng.prune_missing_repos(wow_path);
        // Auto-import newly discovered addon git repos
        let _ = eng.import_existing_addon_git_repos(wow_path);
        // Remove duplicate tracking entries
        let _ = eng.dedup_addon_repos_by_folder(wow_path);
        // One-time casing fix (v4 migration lowercased owner/name).
        if eng.db().needs_casing_fix() {
            let _ = eng.db().mark_casing_fixed();
        }
        let repos = eng.db().list_repos().map_err(|e| e.to_string())?;

        // Read dlls.txt once to determine per-DLL enabled state.
        let dlls_txt = std::fs::read_to_string(wow_path.join("dlls.txt")).unwrap_or_default();
        let enabled_dlls: std::collections::HashSet<String> = dlls_txt
            .lines()
            .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
            .map(|l| l.trim().to_lowercase())
            .collect();

        let mut rows: Vec<RepoRow> = Vec::with_capacity(repos.len());
        for repo in repos {
            let mut row = RepoRow::from(repo);
            let installs = eng.db().list_installs(row.id).unwrap_or_default();
            row.installed_dlls = installs
                .into_iter()
                .filter(|e| e.kind == "dll")
                .filter_map(|e| {
                    let fname = std::path::Path::new(&e.path)
                        .file_name()?.to_str()?.to_string();
                    let is_enabled = enabled_dlls.contains(&fname.to_lowercase());
                    Some((fname, is_enabled))
                })
                .collect();
            rows.push(row);
        }
        Ok(rows)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn check_updates(
    db_path: Option<PathBuf>,
    wow_dir: Option<String>,
    mode: CheckMode,
) -> Result<Vec<PlanRow>, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let plans = tokio::runtime::Handle::current()
            .block_on(async {
                eng.check_updates_with_wow(wow_dir.as_deref().map(Path::new), mode)
                    .await
            })
            .map_err(|e| e.to_string())?;
        Ok(plans.into_iter().map(PlanRow::from).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------

pub async fn add_repo(
    db_path: Option<PathBuf>,
    url: String,
    mode: String,
) -> Result<i64, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let install_mode = InstallMode::from_str(&mode).unwrap_or(InstallMode::Auto);
        eng.add_repo(&url, install_mode, None)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn remove_repo(
    db_path: Option<PathBuf>,
    id: i64,
    wow_dir: Option<String>,
    remove_local_files: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.remove_repo(id, wow_dir.as_deref().map(Path::new), remove_local_files)
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn set_repo_enabled(
    db_path: Option<PathBuf>,
    id: i64,
    enabled: bool,
    wow_dir: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_repo_enabled(id, enabled, None)
            .map_err(|e| e.to_string())?;
        // Also toggle all DLLs for this repo so dlls.txt stays in sync.
        if !wow_dir.is_empty() {
            let installs = eng.db().list_installs(id).unwrap_or_default();
            let wow_path = Path::new(&wow_dir);
            for entry in installs.iter().filter(|e| e.kind == "dll") {
                if let Some(fname) = Path::new(&entry.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                {
                    let _ = eng.set_dll_enabled(fname, enabled, wow_path);
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Returns all installed files for a repo as (path_relative_to_wow_root, kind) pairs.
pub async fn list_repo_installs(
    db_path: Option<PathBuf>,
    repo_id: i64,
) -> Result<Vec<(String, String)>, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let entries = eng.db().list_installs(repo_id).map_err(|e| e.to_string())?;
        Ok(entries.into_iter().map(|e| (e.path, e.kind)).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn set_dll_enabled(
    db_path: Option<PathBuf>,
    wow_dir: String,
    dll_name: String,
    enabled: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_dll_enabled(&dll_name, enabled, Path::new(&wow_dir))
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Result for a single repo updated as part of update-all.
#[derive(Debug, Clone)]
pub struct UpdateOneResult {
    pub owner: String,
    pub name: String,
    /// The updated plan, or None if already up to date.
    pub plan: Option<PlanRow>,
    /// Verbose log lines for this repo.
    pub log_lines: Vec<String>,
    /// Error message if the update failed.
    pub error: Option<String>,
}

/// Update only the repos in `ids_to_update` (already filtered: has_update && !ignored && enabled).
/// Repos are updated in parallel. Returns one result per repo.
pub async fn update_all(
    db_path: Option<PathBuf>,
    wow_dir: String,
    ids_to_update: Vec<i64>,
    opts: InstallOptions,
) -> Result<Vec<UpdateOneResult>, String> {
    if ids_to_update.is_empty() {
        return Ok(Vec::new());
    }

    let mut set = tokio::task::JoinSet::new();

    for id in ids_to_update {
        let db = db_path.clone();
        let wow = wow_dir.clone();
        let opts = opts.clone();

        set.spawn_blocking(move || -> Result<UpdateOneResult, String> {
            let eng = open_engine(db.as_deref())?;
            let repo = eng.db().get_repo(id).map_err(|e| e.to_string())?;
            let owner = repo.owner.clone();
            let name = repo.name.clone();
            let mut log: Vec<String> = Vec::new();

            if repo.mode.as_str() == "addon_git" {
                let branch = repo.git_branch.as_deref().unwrap_or("master");
                log.push(format!("{}/{}: syncing branch '{}'.", owner, name, branch));
            } else {
                log.push(format!("{}/{}: checking release assets.", owner, name));
            }

            let result = tokio::runtime::Handle::current().block_on(async {
                eng.update_repo(id, Path::new(&wow), None, opts).await
            });

            match result {
                Err(e) => {
                    let err = e.to_string();
                    log.push(format!("{}/{}: error — {}", owner, name, err));
                    Ok(UpdateOneResult { owner, name, plan: None, log_lines: log, error: Some(err) })
                }
                Ok(None) => {
                    log.push(format!("{}/{}: already up to date.", owner, name));
                    Ok(UpdateOneResult { owner, name, plan: None, log_lines: log, error: None })
                }
                Ok(Some(plan)) => {
                    if plan.mode.as_str() == "addon_git" {
                        log.push(format!("{}/{}: repository synced.", plan.owner, plan.name));
                    } else if !plan.asset_name.is_empty() {
                        log.push(format!("{}/{}: installed '{}'.", plan.owner, plan.name, plan.asset_name));
                    }
                    log.push(format!("{}/{}: update complete.", plan.owner, plan.name));
                    Ok(UpdateOneResult {
                        owner: plan.owner.clone(),
                        name: plan.name.clone(),
                        plan: Some(PlanRow::from(plan)),
                        log_lines: log,
                        error: None,
                    })
                }
            }
        });
    }

    let mut results = Vec::new();
    while let Some(task) = set.join_next().await {
        match task {
            Err(e) => return Err(format!("Update task panicked: {}", e)),
            Ok(Err(e)) => return Err(e),
            Ok(Ok(r)) => results.push(r),
        }
    }
    Ok(results)
}

/// Install a freshly-added repo, mirroring Tauri's add flow:
/// try `update_repo` first; if it returns None (engine says nothing to do),
/// fall back to `reinstall_repo` to force a fresh clone/download.
pub async fn install_new_repo(
    db_path: Option<PathBuf>,
    id: i64,
    wow_dir: String,
    opts: InstallOptions,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let wow_path = Path::new(&wow_dir);

        let update_result = tokio::runtime::Handle::current()
            .block_on(async { eng.update_repo(id, wow_path, None, opts.clone()).await })
            .map_err(|e| e.to_string())?;

        if let Some(plan) = update_result {
            // update_repo returned a plan — installation happened
            Ok(format!("Installed {}/{}.", plan.owner, plan.name))
        } else {
            // update_repo returned None (engine says up-to-date or nothing to fetch).
            // Force a fresh install via reinstall_repo so the files actually land on disk.
            let plan = tokio::runtime::Handle::current()
                .block_on(async { eng.reinstall_repo(id, wow_path, None, opts).await })
                .map_err(|e| e.to_string())?;
            Ok(format!("Installed {}/{}.", plan.owner, plan.name))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn update_repo(
    db_path: Option<PathBuf>,
    id: i64,
    wow_dir: String,
    opts: InstallOptions,
) -> Result<Option<PlanRow>, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let plan = tokio::runtime::Handle::current()
            .block_on(async {
                eng.update_repo(id, Path::new(&wow_dir), None, opts).await
            })
            .map_err(|e| e.to_string())?;
        Ok(plan.map(PlanRow::from))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn reinstall_repo(
    db_path: Option<PathBuf>,
    id: i64,
    wow_dir: String,
    opts: InstallOptions,
) -> Result<PlanRow, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let plan = tokio::runtime::Handle::current()
            .block_on(async {
                eng.reinstall_repo(id, Path::new(&wow_dir), None, opts)
                    .await
            })
            .map_err(|e| e.to_string())?;
        Ok(PlanRow::from(plan))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Branch management
// ---------------------------------------------------------------------------

pub async fn list_repo_branches(
    db_path: Option<PathBuf>,
    repo_id: i64,
) -> (i64, Result<Vec<String>, String>) {
    let result: Result<Vec<String>, String> = tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.list_repo_branches(repo_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    (repo_id, result)
}

pub async fn set_repo_branch(
    db_path: Option<PathBuf>,
    repo_id: i64,
    branch: String,
) -> Result<i64, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let branch_opt = if branch.is_empty() { None } else { Some(branch) };
        eng.set_repo_git_branch(repo_id, branch_opt).map_err(|e| e.to_string())?;
        Ok(repo_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Game launch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LaunchConfig {
    pub method: String,        // "auto", "lutris", "wine", "custom"
    pub lutris_target: String, // e.g. "lutris:rungameid/2"
    pub wine_command: String,  // e.g. "wine"
    pub wine_args: String,
    pub custom_command: String,
    pub custom_args: String,
    pub clear_wdb: bool,
}

fn first_existing_file(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn resolve_launch_target(wow_path: &Path) -> Result<PathBuf, String> {
    first_existing_file(wow_path, &["VanillaFixes.exe", "vanillafixes.exe"])
        .or_else(|| first_existing_file(wow_path, &["Wow.exe", "wow.exe", "WoW.exe"]))
        .ok_or_else(|| {
            format!(
                "No launcher found in {} (expected VanillaFixes.exe or Wow.exe).",
                wow_path.display()
            )
        })
}

fn parse_arg_string(raw: &str) -> Vec<String> {
    raw.split_whitespace().map(|s| s.to_string()).collect()
}

fn spawn_launch_command(program: &str, args: &[String], cwd: &Path) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(cwd);
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        clean_env_for_child(&mut cmd);
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch '{}': {}", program, e))
}

/// Strip AppImage-injected env vars so child processes see a normal environment.
#[cfg(all(unix, not(target_os = "macos")))]
fn clean_env_for_child(cmd: &mut Command) {
    const BLOCKLIST: &[&str] = &[
        "APPDIR", "APPIMAGE", "ARGV0", "OWD",
        "LD_LIBRARY_PATH", "LD_PRELOAD",
        "GIO_MODULE_DIR", "GST_PLUGIN_PATH", "GST_PLUGIN_SYSTEM_PATH",
        "QT_PLUGIN_PATH", "PYTHONPATH", "PYTHONHOME", "GDK_BACKEND",
    ];
    for key in BLOCKLIST {
        cmd.env_remove(key);
    }
    let clean_path = std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|p| !p.contains("/tmp/.mount_"))
        .collect::<Vec<_>>()
        .join(":");
    if !clean_path.is_empty() {
        cmd.env("PATH", clean_path);
    }
    if let Ok(dirs) = std::env::var("XDG_DATA_DIRS") {
        let clean: Vec<&str> = dirs.split(':').filter(|p| !p.contains("/tmp/.mount_")).collect();
        if !clean.is_empty() {
            cmd.env("XDG_DATA_DIRS", clean.join(":"));
        } else {
            cmd.env_remove("XDG_DATA_DIRS");
        }
    }
}

pub async fn launch_game(wow_dir: String, cfg: LaunchConfig) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let wow_path = PathBuf::from(wow_dir.trim());
        if !wow_path.is_dir() {
            return Err(format!("WoW path is not a directory: {}", wow_path.display()));
        }

        // Optionally clear WDB cache before launch
        if cfg.clear_wdb {
            let wdb = wow_path.join("WDB");
            if wdb.is_dir() {
                let _ = std::fs::remove_dir_all(&wdb);
            }
        }

        let target = resolve_launch_target(&wow_path)?;
        let target_str = target.to_string_lossy().to_string();
        let target_name = target.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "game".to_string());

        let method = cfg.method.trim().to_ascii_lowercase();

        if method == "lutris" {
            let command = if cfg.custom_command.trim().is_empty() { "lutris" } else { cfg.custom_command.trim() };
            let target_arg = cfg.lutris_target.trim();
            if target_arg.is_empty() {
                return Err("Lutris launch target is empty (expected e.g. lutris:rungameid/2).".to_string());
            }
            let mut args = vec![target_arg.to_string()];
            args.extend(parse_arg_string(&cfg.custom_args));
            spawn_launch_command(command, &args, &wow_path)?;
            return Ok(format!("Launched {} via {}.", target_name, command));
        }

        if method == "wine" {
            let command = if cfg.wine_command.trim().is_empty() { "wine" } else { cfg.wine_command.trim() };
            let mut args = parse_arg_string(&cfg.wine_args);
            args.push(target_str);
            spawn_launch_command(command, &args, &wow_path)?;
            return Ok(format!("Launched {} via {}.", target_name, command));
        }

        if method == "custom" {
            let command = cfg.custom_command.trim();
            if command.is_empty() {
                return Err("Custom launch command is empty.".to_string());
            }
            let mut args = parse_arg_string(&cfg.custom_args);
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
            spawn_launch_command(command, &args, &wow_path)?;
            return Ok(format!("Launched {} via custom command.", target_name));
        }

        // "auto" or fallback: launch executable directly
        let mut cmd = Command::new(&target);
        cmd.current_dir(&wow_path);
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            clean_env_for_child(&mut cmd);
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        cmd.spawn()
            .map(|_| format!("Launched {}.", target_name))
            .map_err(|e| format!("Failed to launch {}: {}", target_name, e))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// GitHub token management
// ---------------------------------------------------------------------------

const KEYCHAIN_SERVICE: &str = "wuddle";
const KEYCHAIN_ACCOUNT: &str = "github_token";
const KEYCHAIN_TIMEOUT_MS: u64 = 2500;

fn keychain_call_with_timeout<T, F>(label: &'static str, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || { let _ = tx.send(f()); });
    match rx.recv_timeout(Duration::from_millis(KEYCHAIN_TIMEOUT_MS)) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
            "System keychain timed out while {}. Ensure keychain is running, or use WUDDLE_GITHUB_TOKEN env.",
            label
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("System keychain worker failed unexpectedly.".to_string())
        }
    }
}

fn token_file_path() -> Result<PathBuf, String> {
    Ok(crate::settings::app_dir()?.join(".github_token"))
}

fn read_file_token() -> Result<Option<String>, String> {
    let path = token_file_path()?;
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            let t = s.trim().to_string();
            Ok(if t.is_empty() { None } else { Some(t) })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn is_portable() -> bool {
    std::env::var("WUDDLE_PORTABLE")
        .ok()
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn read_stored_token() -> Result<Option<String>, String> {
    if is_portable() {
        return read_file_token();
    }
    keychain_call_with_timeout("reading token", || {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
            .map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(token) => {
                let token = token.trim().to_string();
                Ok(if token.is_empty() { None } else { Some(token) })
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    })
}

pub fn sync_github_token() {
    // Try keychain/file first, then env
    if let Ok(Some(token)) = read_stored_token() {
        wuddle_engine::set_github_token(Some(token));
        return;
    }
    // Check env variables
    if let Some(token) = std::env::var("WUDDLE_GITHUB_TOKEN")
        .ok()
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        wuddle_engine::set_github_token(Some(token));
    }
}

pub async fn save_github_token(token: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let token = token.trim().to_string();
        if token.is_empty() {
            return Err("Token is empty.".to_string());
        }
        if is_portable() {
            let path = token_file_path()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, &token).map_err(|e| e.to_string())?;
        } else {
            keychain_call_with_timeout("saving token", move || {
                let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
                    .map_err(|e| e.to_string())?;
                entry.set_password(&token).map_err(|e| e.to_string())
            })?;
        }
        // Update engine's in-memory token
        sync_github_token();
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn clear_github_token() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        if is_portable() {
            let path = token_file_path()?;
            if path.exists() {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
        } else {
            keychain_call_with_timeout("clearing token", || {
                let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
                    .map_err(|e| e.to_string())?;
                if let Err(e) = entry.delete_credential() {
                    if !matches!(e, keyring::Error::NoEntry) {
                        return Err(e.to_string());
                    }
                }
                Ok(())
            })?;
        }
        wuddle_engine::set_github_token(None);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Repo preview (for Add dialog)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RepoFileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub struct RepoPreviewInfo {
    pub name: String,
    pub description: String,
    pub stars: u64,
    pub forks: u64,
    pub language: String,
    pub license: String,
    pub readme_text: String,
    pub readme_items: Vec<iced::widget::markdown::Item>,
    /// Fetched image bytes keyed by the URL as it appears in the markdown (may be absolute or relative).
    pub image_cache: std::collections::HashMap<String, Vec<u8>>,
    pub files: Vec<RepoFileEntry>,
    /// Base URL for resolving relative image paths (e.g. "https://raw.githubusercontent.com/owner/repo/HEAD/")
    pub raw_base_url: String,
    pub forge: String,
    pub owner: String,
    pub repo_name: String,
    pub forge_url: String,
}

// ---------------------------------------------------------------------------
// Parse forge from URL
// ---------------------------------------------------------------------------

pub struct ForgeInfo {
    pub owner: String,
    pub repo: String,
    pub forge: &'static str,
    pub host: String,
    pub scheme: String,
}

pub fn parse_forge_url(url: &str) -> Option<ForgeInfo> {
    let trimmed = url.trim().trim_end_matches('/');
    let without_scheme = trimmed
        .strip_prefix("https://")
        .map(|s| ("https", s))
        .or_else(|| trimmed.strip_prefix("http://").map(|s| ("http", s)))
        .unwrap_or(("https", trimmed));
    let (scheme, rest) = without_scheme;

    if let Some(r) = rest.strip_prefix("github.com/") {
        let parts: Vec<&str> = r.splitn(3, '/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            let repo = parts[1].trim_end_matches(".git").to_string();
            return Some(ForgeInfo { owner: parts[0].to_string(), repo, forge: "github", host: "github.com".into(), scheme: scheme.into() });
        }
    } else if let Some(r) = rest.strip_prefix("gitlab.com/") {
        let parts: Vec<&str> = r.splitn(3, '/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            let repo = parts[1].trim_end_matches(".git").to_string();
            return Some(ForgeInfo { owner: parts[0].to_string(), repo, forge: "gitlab", host: "gitlab.com".into(), scheme: scheme.into() });
        }
    } else {
        let parts: Vec<&str> = rest.splitn(4, '/').collect();
        if parts.len() >= 3 && !parts[1].is_empty() && !parts[2].is_empty() {
            let host = parts[0];
            if host.contains("gitea") || host.contains("forgejo") || host.contains("codeberg") || host.contains("gitea") {
                let repo = parts[2].trim_end_matches(".git").to_string();
                return Some(ForgeInfo { owner: parts[1].to_string(), repo, forge: "gitea", host: host.into(), scheme: scheme.into() });
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Image helpers
// ---------------------------------------------------------------------------

/// Collect image URLs from raw HTML `<img src="...">` tags in markdown text.
/// Markdown-syntax images (`![alt](url)`) are handled by `Content::parse().images()` instead,
/// which uses the same pulldown-cmark parser as iced's renderer (guaranteed URL match).
fn collect_html_img_urls_from_text(markdown: &str) -> Vec<String> {
    let mut urls = Vec::new();

    // --- HTML syntax: <img src="url"> or <img src='url'> ---
    let mut hpos = 0;
    while hpos < markdown.len() {
        match markdown[hpos..].find("<img") {
            None => break,
            Some(tag_offset) => {
                let tag_start = hpos + tag_offset;
                // Find the end of this tag
                let tag_end = markdown[tag_start..].find('>')
                    .map(|e| tag_start + e + 1)
                    .unwrap_or(markdown.len());
                let tag_slice = &markdown[tag_start..tag_end];
                // Find src= attribute inside tag
                if let Some(src_pos) = tag_slice.find("src=") {
                    let after_src = &tag_slice[src_pos + 4..];
                    let quote = after_src.chars().next();
                    if let Some(q @ ('"' | '\'')) = quote {
                        let inner = &after_src[1..];
                        if let Some(end_q) = inner.find(q) {
                            let url = inner[..end_q].trim();
                            if !url.is_empty() {
                                urls.push(url.to_string());
                            }
                        }
                    }
                }
                hpos = tag_start + 4; // skip past "<img"
            }
        }
    }

    urls
}

/// Resolve a potentially-relative image URL against a raw base URL.
pub fn resolve_image_url(url: &str, raw_base_url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        let clean = url.trim_start_matches("./").trim_start_matches('/');
        format!("{}{}", raw_base_url, clean)
    }
}

/// Fetch image bytes for URLs found in the README. Limits: max 12 images, 5 MB each, 20 MB total.
async fn fetch_images(
    client: &Client,
    image_urls: &[String],
    raw_base_url: &str,
) -> std::collections::HashMap<String, Vec<u8>> {
    let mut cache = std::collections::HashMap::new();
    let mut total_bytes = 0usize;

    // Pre-resolve github.com/user-attachments/assets/UUID → signed CDN URLs.
    // GitHub renders these as private-user-images.githubusercontent.com/?jwt=... in its HTML.
    let attachment_resolves =
        resolve_github_user_attachments(client, raw_base_url, image_urls).await;

    for url in image_urls.iter().take(12) {
        if total_bytes > 20_000_000 { break; }

        let abs_url = resolve_image_url(url, raw_base_url);

        // For user-attachments URLs, use the signed CDN URL extracted from GitHub HTML.
        let fetch_url: String = attachment_resolves
            .get(url.as_str())
            .cloned()
            .unwrap_or_else(|| abs_url.clone());

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            async {
                let mut req = client.get(&fetch_url);
                // Non-signed private-user-images URLs may need a GitHub token.
                if fetch_url.contains("private-user-images.githubusercontent.com")
                    && !fetch_url.contains("?jwt=")
                {
                    if let Some(token) = wuddle_engine::github_token() {
                        req = req.bearer_auth(token);
                    }
                }
                let resp = req.send().await?;
                let ct = resp.headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("(none)")
                    .to_string();
                if !resp.status().is_success() {
                    return Err(reqwest::Error::from(resp.error_for_status().unwrap_err()));
                }
                if !ct.starts_with("image/") {
                    return Ok(Default::default());
                }
                resp.bytes().await
            },
        ).await;

        if let Ok(Ok(bytes)) = result {
            if !bytes.is_empty() && bytes.len() <= 5_000_000 {
                total_bytes += bytes.len();
                let data = bytes.to_vec();
                // Store by original URL (as seen in markdown) AND absolute URL
                cache.insert(url.clone(), data.clone());
                if abs_url != *url {
                    cache.insert(abs_url, data);
                }
            }
        }
    }
    cache
}

/// Resolve `github.com/user-attachments/assets/UUID` URLs to time-limited signed CDN URLs.
///
/// GitHub's HTML page for the repo contains `<img src="https://private-user-images.githubusercontent.com/…?jwt=…">`
/// entries for any user-attachments referenced in the README.  We fetch the page once, then
/// extract the signed URL for each UUID we care about.
async fn resolve_github_user_attachments(
    client: &Client,
    raw_base_url: &str,
    image_urls: &[String],
) -> std::collections::HashMap<String, String> {
    let attachment_pairs: Vec<(String, String)> = image_urls
        .iter()
        .filter_map(|u| {
            u.strip_prefix("https://github.com/user-attachments/assets/")
                .map(|uuid| (u.clone(), uuid.to_string()))
        })
        .collect();

    let mut result: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if attachment_pairs.is_empty() { return result; }

    // Derive owner/repo from raw_base_url:
    //   "https://raw.githubusercontent.com/{owner}/{repo}/..."
    let after = raw_base_url
        .strip_prefix("https://raw.githubusercontent.com/")
        .unwrap_or("");
    let parts: Vec<&str> = after.splitn(3, '/').collect();
    if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
        return result;
    }
    let html_url = format!("https://github.com/{}/{}", parts[0], parts[1]);

    let resp = match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        client.get(&html_url).send(),
    )
    .await
    {
        Ok(Ok(r)) => r,
        Ok(Err(_)) => return result,
        Err(_) => return result,
    };
    if !resp.status().is_success() { return result; }
    let html = resp.text().await.unwrap_or_default();

    // Scan all private-user-images URLs in the HTML and match each one by UUID.
    // We scan rather than searching for the UUID first because the UUID may appear
    // earlier in the HTML inside JSON blobs where the signed URL isn't present.
    let signed_prefix = "https://private-user-images.githubusercontent.com/";
    let mut signed_urls: Vec<String> = Vec::new();
    let mut scan_pos = 0;
    while let Some(p) = html[scan_pos..].find(signed_prefix) {
        let start = scan_pos + p;
        let rest = &html[start..];
        // URL ends at the first `"`, `'`, `\` (JSON-escaped quote context), or whitespace
        let end = rest
            .find(|c: char| c == '"' || c == '\'' || c == '\\' || c.is_ascii_whitespace())
            .unwrap_or_else(|| rest.len().min(3000));
        let candidate = rest[..end].to_string();
        if !candidate.is_empty() && !signed_urls.contains(&candidate) {
            signed_urls.push(candidate);
        }
        scan_pos = start + signed_prefix.len();
    }
    for (orig_url, uuid) in &attachment_pairs {
        // Find the signed URL whose path contains this UUID
        if let Some(signed) = signed_urls.iter().find(|u| u.contains(uuid.as_str())) {
            result.insert(orig_url.clone(), signed.clone());
        }
    }
    result
}

/// Fetch raw text content of a file from a repo's raw base URL.
/// Returns (filename/path, content).
pub async fn fetch_raw_file(raw_base_url: String, path: String) -> Result<(String, String), String> {
    let base = raw_base_url.trim_end_matches('/');
    let url = format!("{}/{}", base, path.trim_start_matches('/'));
    let client = Client::builder()
        .user_agent("wuddle-iced")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let content = resp.text().await.map_err(|e| e.to_string())?;
    Ok((path, content))
}

// ---------------------------------------------------------------------------
// Files tree helper
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ContentEntry { name: String, #[serde(rename = "type")] kind: String }

async fn fetch_files(client: &Client, forge: &str, host: &str, owner: &str, repo: &str, scheme: &str) -> Vec<RepoFileEntry> {
    match forge {
        "github" => {
            let url = format!("https://api.github.com/repos/{}/{}/contents/", owner, repo);
            let mut req = client.get(&url).header("Accept", "application/vnd.github+json");
            if let Some(token) = wuddle_engine::github_token() { req = req.bearer_auth(token); }
            match req.send().await {
                Ok(r) if r.status().is_success() => {
                    r.json::<Vec<ContentEntry>>().await.unwrap_or_default()
                        .into_iter()
                        .map(|e| RepoFileEntry { is_dir: e.kind == "dir", path: e.name.clone(), name: e.name })
                        .collect()
                }
                _ => Vec::new(),
            }
        }
        "gitlab" => {
            let encoded = format!("{}/{}", owner, repo).replace('/', "%2F");
            let url = format!("https://gitlab.com/api/v4/projects/{}/repository/tree?per_page=50", encoded);
            match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => {
                    r.json::<Vec<ContentEntry>>().await.unwrap_or_default()
                        .into_iter()
                        .map(|e| RepoFileEntry { is_dir: e.kind == "tree", path: e.name.clone(), name: e.name })
                        .collect()
                }
                _ => Vec::new(),
            }
        }
        _ => {
            let url = format!("{}://{}/api/v1/repos/{}/{}/contents/", scheme, host, owner, repo);
            match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => {
                    r.json::<Vec<ContentEntry>>().await.unwrap_or_default()
                        .into_iter()
                        .map(|e| RepoFileEntry { is_dir: e.kind == "dir", path: e.name.clone(), name: e.name })
                        .collect()
                }
                _ => Vec::new(),
            }
        }
    }
}

/// Fetch contents of a subdirectory within a repo tree.
/// Returns (dir_path, entries) where each entry's `path` is the full path from repo root.
pub async fn fetch_dir_contents(
    forge_url: String,
    dir_path: String,
) -> Result<(String, Vec<RepoFileEntry>), String> {
    let fi = parse_forge_url(&forge_url)
        .ok_or_else(|| "Could not parse repo URL".to_string())?;
    let client = Client::builder()
        .user_agent("wuddle-iced")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let entries: Vec<RepoFileEntry> = match fi.forge {
        "github" => {
            let url = format!(
                "https://api.github.com/repos/{}/{}/contents/{}",
                fi.owner, fi.repo, dir_path
            );
            let mut req = client.get(&url).header("Accept", "application/vnd.github+json");
            if let Some(token) = wuddle_engine::github_token() { req = req.bearer_auth(token); }
            match req.send().await {
                Ok(r) if r.status().is_success() => {
                    r.json::<Vec<ContentEntry>>().await.unwrap_or_default()
                        .into_iter()
                        .map(|e| RepoFileEntry {
                            is_dir: e.kind == "dir",
                            path: format!("{}/{}", dir_path, e.name),
                            name: e.name,
                        })
                        .collect()
                }
                _ => Vec::new(),
            }
        }
        "gitlab" => {
            let encoded = format!("{}/{}", fi.owner, fi.repo).replace('/', "%2F");
            let url = format!(
                "https://gitlab.com/api/v4/projects/{}/repository/tree?path={}&per_page=50",
                encoded, dir_path
            );
            match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => {
                    r.json::<Vec<ContentEntry>>().await.unwrap_or_default()
                        .into_iter()
                        .map(|e| RepoFileEntry {
                            is_dir: e.kind == "tree",
                            path: format!("{}/{}", dir_path, e.name),
                            name: e.name,
                        })
                        .collect()
                }
                _ => Vec::new(),
            }
        }
        _ => {
            let url = format!(
                "{}://{}/api/v1/repos/{}/{}/contents/{}",
                fi.scheme, fi.host, fi.owner, fi.repo, dir_path
            );
            match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => {
                    r.json::<Vec<ContentEntry>>().await.unwrap_or_default()
                        .into_iter()
                        .map(|e| RepoFileEntry {
                            is_dir: e.kind == "dir",
                            path: format!("{}/{}", dir_path, e.name),
                            name: e.name,
                        })
                        .collect()
                }
                _ => Vec::new(),
            }
        }
    };
    Ok((dir_path, entries))
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub async fn fetch_repo_preview(url: String) -> Result<RepoPreviewInfo, String> {
    let fi = parse_forge_url(&url)
        .ok_or_else(|| "Could not parse repo URL".to_string())?;

    let client = Client::builder()
        .user_agent("wuddle-iced")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    match fi.forge {
        "github" => fetch_github_preview(&client, &fi.owner, &fi.repo).await,
        "gitlab" => fetch_gitlab_preview(&client, &fi.owner, &fi.repo).await,
        _ => fetch_gitea_preview(&client, &fi.host, &fi.scheme, &fi.owner, &fi.repo).await,
    }
}

// ---------------------------------------------------------------------------
// GitHub
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GhRepoInfo {
    name: Option<String>,
    description: Option<String>,
    stargazers_count: Option<u64>,
    forks_count: Option<u64>,
    language: Option<String>,
    license: Option<GhLicense>,
}
#[derive(Debug, Deserialize)]
struct GhLicense { spdx_id: Option<String> }

async fn fetch_github_preview(client: &Client, owner: &str, repo: &str) -> Result<RepoPreviewInfo, String> {
    let info_url = format!("https://api.github.com/repos/{}/{}", owner, repo);
    let mut req = client.get(&info_url).header("Accept", "application/vnd.github+json");
    if let Some(token) = wuddle_engine::github_token() { req = req.bearer_auth(token); }
    let info: GhRepoInfo = req.send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let readme_url = format!("https://api.github.com/repos/{}/{}/readme", owner, repo);
    let mut readme_req = client.get(&readme_url).header("Accept", "application/vnd.github.raw+json");
    if let Some(token) = wuddle_engine::github_token() { readme_req = readme_req.bearer_auth(token); }
    let readme_text = match readme_req.send().await {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        _ => String::new(),
    };

    let raw_base = format!("https://raw.githubusercontent.com/{}/{}/HEAD/", owner, repo);
    let md_content = iced::widget::markdown::Content::parse(&readme_text);
    let readme_items: Vec<iced::widget::markdown::Item> = md_content.items().to_vec();
    // Use Content::images() for exact URL match with iced's renderer; add HTML <img> tags too
    let mut image_urls: Vec<String> = md_content.images().iter().cloned().collect();
    for url in collect_html_img_urls_from_text(&readme_text) {
        if !image_urls.contains(&url) { image_urls.push(url); }
    }
    let image_cache = fetch_images(client, &image_urls, &raw_base).await;

    let files = fetch_files(client, "github", "github.com", owner, repo, "https").await;

    let license = info.license.and_then(|l| l.spdx_id).unwrap_or_default();
    let license = if license == "NOASSERTION" || license.is_empty() { String::new() } else { license };

    Ok(RepoPreviewInfo {
        name: info.name.unwrap_or_else(|| repo.to_string()),
        description: info.description.unwrap_or_default(),
        stars: info.stargazers_count.unwrap_or(0),
        forks: info.forks_count.unwrap_or(0),
        language: info.language.unwrap_or_default(),
        license,
        readme_items,
        readme_text,
        image_cache,
        files,
        raw_base_url: raw_base,
        forge: "github".into(),
        owner: owner.into(),
        repo_name: repo.into(),
        forge_url: format!("https://github.com/{}/{}", owner, repo),
    })
}

// ---------------------------------------------------------------------------
// GitLab
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GlProject {
    name: Option<String>,
    description: Option<String>,
    star_count: Option<u64>,
    forks_count: Option<u64>,
}

async fn fetch_gitlab_preview(client: &Client, owner: &str, repo: &str) -> Result<RepoPreviewInfo, String> {
    let encoded = format!("{}/{}", owner, repo).replace('/', "%2F");
    let url = format!("https://gitlab.com/api/v4/projects/{}", encoded);
    let info: GlProject = client.get(&url).send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let readme_url = format!("https://gitlab.com/{}/{}/raw/HEAD/README.md", owner, repo);
    let readme_text = match client.get(&readme_url).send().await {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        _ => String::new(),
    };

    let raw_base = format!("https://gitlab.com/{}/{}/raw/HEAD/", owner, repo);
    let md_content = iced::widget::markdown::Content::parse(&readme_text);
    let readme_items: Vec<iced::widget::markdown::Item> = md_content.items().to_vec();
    let mut image_urls: Vec<String> = md_content.images().iter().cloned().collect();
    for url in collect_html_img_urls_from_text(&readme_text) {
        if !image_urls.contains(&url) { image_urls.push(url); }
    }
    let image_cache = fetch_images(client, &image_urls, &raw_base).await;
    let files = fetch_files(client, "gitlab", "gitlab.com", owner, repo, "https").await;

    Ok(RepoPreviewInfo {
        name: info.name.unwrap_or_else(|| repo.to_string()),
        description: info.description.unwrap_or_default(),
        stars: info.star_count.unwrap_or(0),
        forks: info.forks_count.unwrap_or(0),
        language: String::new(),
        license: String::new(),
        readme_items,
        readme_text,
        image_cache,
        files,
        raw_base_url: raw_base,
        forge: "gitlab".into(),
        owner: owner.into(),
        repo_name: repo.into(),
        forge_url: format!("https://gitlab.com/{}/{}", owner, repo),
    })
}

// ---------------------------------------------------------------------------
// Gitea / Codeberg / Forgejo
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GiteaRepo {
    name: Option<String>,
    description: Option<String>,
    stars_count: Option<u64>,
    forks_count: Option<u64>,
    language: Option<String>,
    default_branch: Option<String>,
}

async fn fetch_gitea_preview(client: &Client, host: &str, scheme: &str, owner: &str, repo: &str) -> Result<RepoPreviewInfo, String> {
    let api_url = format!("{}://{}/api/v1/repos/{}/{}", scheme, host, owner, repo);
    let info: GiteaRepo = client.get(&api_url).send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let branch = info.default_branch.as_deref().unwrap_or("master");
    let readme_url = format!("{}://{}/{}/{}/raw/branch/{}/README.md", scheme, host, owner, repo, branch);
    let readme_text = match client.get(&readme_url).send().await {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        _ => String::new(),
    };

    let raw_base = format!("{}://{}/{}/{}/raw/branch/{}/", scheme, host, owner, repo, branch);
    let md_content = iced::widget::markdown::Content::parse(&readme_text);
    let readme_items: Vec<iced::widget::markdown::Item> = md_content.items().to_vec();
    let mut image_urls: Vec<String> = md_content.images().iter().cloned().collect();
    for url in collect_html_img_urls_from_text(&readme_text) {
        if !image_urls.contains(&url) { image_urls.push(url); }
    }
    let image_cache = fetch_images(client, &image_urls, &raw_base).await;
    let files = fetch_files(client, "gitea", host, owner, repo, scheme).await;

    Ok(RepoPreviewInfo {
        name: info.name.unwrap_or_else(|| repo.to_string()),
        description: info.description.unwrap_or_default(),
        stars: info.stars_count.unwrap_or(0),
        forks: info.forks_count.unwrap_or(0),
        language: info.language.unwrap_or_default(),
        license: String::new(),
        readme_items,
        readme_text,
        image_cache,
        files,
        raw_base_url: raw_base,
        forge: "gitea".into(),
        owner: owner.into(),
        repo_name: repo.into(),
        forge_url: format!("{}://{}/{}/{}", scheme, host, owner, repo),
    })
}

// ---------------------------------------------------------------------------
// Tweak wrappers (delegates to crate::tweaks which ports vanilla-tweaks)
// ---------------------------------------------------------------------------

pub async fn read_tweaks(wow_dir: String) -> Result<crate::tweaks::ReadTweakValues, String> {
    tokio::task::spawn_blocking(move || {
        crate::tweaks::read_tweaks(std::path::Path::new(&wow_dir))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn apply_tweaks(wow_dir: String, opts: crate::tweaks::TweakOptions) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        crate::tweaks::apply_tweaks(std::path::Path::new(&wow_dir), &opts)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn restore_tweaks(wow_dir: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        crate::tweaks::restore_backup(std::path::Path::new(&wow_dir))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Releases (for in-app Release Notes)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ReleaseItem {
    pub tag_name: String,
    pub name: String,
    pub published_at: String,
    pub body: String,
    pub items: Vec<iced::widget::markdown::Item>,
    pub prerelease: bool,
}

pub async fn fetch_releases(forge_url: String) -> Result<Vec<ReleaseItem>, String> {
    let fi = parse_forge_url(&forge_url)
        .ok_or_else(|| "Could not parse forge URL".to_string())?;

    let client = Client::builder()
        .user_agent("wuddle-iced")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    match fi.forge {
        "github" => {
            #[derive(Deserialize)]
            struct GhRelease {
                tag_name: String,
                name: Option<String>,
                published_at: Option<String>,
                body: Option<String>,
                prerelease: bool,
            }
            let url = format!(
                "https://api.github.com/repos/{}/{}/releases?per_page=20",
                fi.owner, fi.repo
            );
            let mut req = client.get(&url).header("Accept", "application/vnd.github+json");
            if let Some(token) = wuddle_engine::github_token() { req = req.bearer_auth(token); }
            let releases: Vec<GhRelease> = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                req.send(),
            ).await
            .map_err(|_| "Timed out fetching releases".to_string())?
            .map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
            Ok(releases.into_iter().map(|r| {
                let body = r.body.unwrap_or_default();
                let items = iced::widget::markdown::Content::parse(&body).items().to_vec();
                ReleaseItem {
                    tag_name: r.tag_name.clone(),
                    name: r.name.filter(|s| !s.is_empty()).unwrap_or_else(|| r.tag_name),
                    published_at: r.published_at.unwrap_or_default(),
                    body,
                    items,
                    prerelease: r.prerelease,
                }
            }).collect())
        }
        "gitlab" => {
            #[derive(Deserialize)]
            struct GlRelease {
                tag_name: String,
                name: Option<String>,
                released_at: Option<String>,
                description: Option<String>,
            }
            let encoded = format!("{}/{}", fi.owner, fi.repo).replace('/', "%2F");
            let url = format!("https://gitlab.com/api/v4/projects/{}/releases", encoded);
            let releases: Vec<GlRelease> = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                client.get(&url).send(),
            ).await
            .map_err(|_| "Timed out fetching releases".to_string())?
            .map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
            Ok(releases.into_iter().map(|r| {
                let body = r.description.unwrap_or_default();
                let items = iced::widget::markdown::Content::parse(&body).items().to_vec();
                ReleaseItem {
                    tag_name: r.tag_name.clone(),
                    name: r.name.filter(|s| !s.is_empty()).unwrap_or_else(|| r.tag_name),
                    published_at: r.released_at.unwrap_or_default(),
                    body,
                    items,
                    prerelease: false,
                }
            }).collect())
        }
        _ => {
            // Gitea / Forgejo / Codeberg
            #[derive(Deserialize)]
            struct GiteaRelease {
                tag_name: String,
                name: Option<String>,
                published_at: Option<String>,
                body: Option<String>,
                prerelease: bool,
            }
            let url = format!(
                "{}://{}/api/v1/repos/{}/{}/releases?limit=20",
                fi.scheme, fi.host, fi.owner, fi.repo
            );
            let releases: Vec<GiteaRelease> = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                client.get(&url).send(),
            ).await
            .map_err(|_| "Timed out fetching releases".to_string())?
            .map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
            Ok(releases.into_iter().map(|r| {
                let body = r.body.unwrap_or_default();
                let items = iced::widget::markdown::Content::parse(&body).items().to_vec();
                ReleaseItem {
                    tag_name: r.tag_name.clone(),
                    name: r.name.filter(|s| !s.is_empty()).unwrap_or_else(|| r.tag_name),
                    published_at: r.published_at.unwrap_or_default(),
                    body,
                    items,
                    prerelease: r.prerelease,
                }
            }).collect())
        }
    }
}

// ---------------------------------------------------------------------------
// Self-update: fetch latest GitHub release tag
// ---------------------------------------------------------------------------

const WUDDLE_RELEASE_API_LATEST: &str = "https://api.github.com/repos/ZythDr/Wuddle/releases/latest";
const WUDDLE_RELEASE_API_ALL: &str = "https://api.github.com/repos/ZythDr/Wuddle/releases?per_page=5";
const CHANGELOG_URL: &str = "https://raw.githubusercontent.com/ZythDr/Wuddle/main/CHANGELOG.md";
const CHANGELOG_EMBEDDED: &str = include_str!("../../CHANGELOG.md");

pub async fn fetch_changelog() -> Result<String, String> {
    let client = Client::builder()
        .user_agent(concat!("wuddle/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    match client.get(CHANGELOG_URL).send().await {
        Ok(resp) if resp.status().is_success() => {
            resp.text().await.map_err(|e| e.to_string())
        }
        _ => Ok(CHANGELOG_EMBEDDED.to_string()),
    }
}

/// Check for the latest Wuddle release.
/// `beta_channel`: when true, fetches all releases (including pre-releases) and
/// returns the newest tag; when false, fetches `/releases/latest` which GitHub
/// guarantees is the most recent non-pre-release.
pub async fn check_self_update(beta_channel: bool) -> Result<String, String> {
    #[derive(Deserialize)]
    struct GhRelease { tag_name: String }

    let client = Client::builder()
        .user_agent(concat!("wuddle/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let url = if beta_channel { WUDDLE_RELEASE_API_ALL } else { WUDDLE_RELEASE_API_LATEST };

    let resp = tokio::time::timeout(
        Duration::from_secs(12),
        client.get(url).header("Accept", "application/vnd.github+json").send(),
    )
    .await
    .map_err(|_| "Timed out checking for updates".to_string())?
    .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API error: HTTP {}", resp.status()));
    }

    if beta_channel {
        // Returns an array; GitHub sorts by created_at desc so the first entry is newest.
        let releases: Vec<GhRelease> = resp.json().await.map_err(|e| e.to_string())?;
        releases.into_iter().next()
            .map(|r| r.tag_name)
            .ok_or_else(|| "No releases found".to_string())
    } else {
        let release: GhRelease = resp.json().await.map_err(|e| e.to_string())?;
        Ok(release.tag_name)
    }
}

/// Write the generated dxvk.conf content to the given path.
pub async fn save_dxvk_conf(path: std::path::PathBuf, content: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        std::fs::write(&path, content.as_bytes()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
