//! Thin async wrappers around wuddle-engine.
//! Every function opens a fresh Engine (it's Send+!Sync due to rusqlite).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;
use wuddle_engine::{CheckMode, Engine, InstallMode, InstallOptions, Repo, UpdatePlan};

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
}

impl From<Repo> for RepoRow {
    fn from(r: Repo) -> Self {
        Self {
            id: r.id,
            forge: r.forge,
            owner: r.owner,
            name: r.name,
            url: r.url,
            mode: r.mode.as_str().to_string(),
            enabled: r.enabled,
            last_version: r.last_version,
            git_branch: r.git_branch,
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
        let has_update = p.current.as_deref() != Some(&p.latest) && p.error.is_none();
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
        let eng = open_engine(db_path.as_deref())?;
        if let Some(ref dir) = wow_dir {
            let wow_path = Path::new(dir);
            // Prune repos whose files no longer exist on disk (DB only, never deletes files)
            let _ = eng.prune_missing_repos(wow_path);
            // Auto-import newly discovered addon git repos
            let _ = eng.import_existing_addon_git_repos(wow_path);
            // Remove duplicate tracking entries
            let _ = eng.dedup_addon_repos_by_folder(wow_path);
        }
        // One-time casing fix (v4 migration lowercased owner/name).
        if eng.db().needs_casing_fix() {
            let _ = eng.db().mark_casing_fixed();
        }
        let repos = eng.db().list_repos().map_err(|e| e.to_string())?;
        Ok(repos.into_iter().map(RepoRow::from).collect())
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
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_repo_enabled(id, enabled, None)
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn update_all(
    db_path: Option<PathBuf>,
    wow_dir: String,
    opts: InstallOptions,
) -> Result<Vec<PlanRow>, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let plans = tokio::runtime::Handle::current()
            .block_on(async { eng.apply_updates(Path::new(&wow_dir), None, opts).await })
            .map_err(|e| e.to_string())?;
        Ok(plans.into_iter().map(PlanRow::from).collect())
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
) -> Result<(i64, Vec<String>), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let branches = eng.list_repo_branches(repo_id).map_err(|e| e.to_string())?;
        Ok((repo_id, branches))
    })
    .await
    .map_err(|e| e.to_string())?
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

pub async fn launch_game(wow_dir: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let wow_path = PathBuf::from(wow_dir.trim());
        if !wow_path.is_dir() {
            return Err(format!("WoW path is not a directory: {}", wow_path.display()));
        }
        let target = resolve_launch_target(&wow_path)?;
        let target_name = target.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "game".to_string());

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
