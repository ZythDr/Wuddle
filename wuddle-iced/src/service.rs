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
    pub merge_installs: bool,
    pub pinned_version: Option<String>,
    pub published_at_unix: Option<i64>,
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
            merge_installs: r.merge_installs,
            pinned_version: r.pinned_version,
            published_at_unix: r.published_at_unix,
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
    pub externally_modified: bool,
    pub error: Option<String>,
    pub previous_dll_count: usize,
    pub new_dll_count: usize,
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
            externally_modified: p.externally_modified,
            error: p.error,
            previous_dll_count: p.previous_dll_count,
            new_dll_count: p.new_dll_count,
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

/// Best-effort fix: re-fetch correct owner/name casing from each forge API.
/// Called during rescan so repos lowercased by the v4 migration get corrected.
/// Only queries the API for repos whose owner or name are entirely lowercase
/// (indicating they were likely lowercased by the v4 migration).
fn fix_repo_casing_from_forges(eng: &Engine) {
    let repos = match eng.db().list_repos() {
        Ok(r) => r,
        Err(_) => return,
    };

    // Only fix repos that look like they were lowercased by the migration.
    let needs_fix: Vec<&Repo> = repos
        .iter()
        .filter(|r| {
            let owner_lower = r.owner == r.owner.to_ascii_lowercase()
                && r.owner.chars().any(|c| c.is_ascii_alphabetic());
            let name_lower = r.name == r.name.to_ascii_lowercase()
                && r.name.chars().any(|c| c.is_ascii_alphabetic());
            owner_lower || name_lower
        })
        .collect();

    if needs_fix.is_empty() {
        return;
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    let ua = format!("Wuddle/{}", env!("CARGO_PKG_VERSION"));
    let gh_token = wuddle_engine::github_token();

    for repo in &needs_fix {
        let (new_owner, new_name) = match repo.forge.as_str() {
            "github" => {
                let api_url = format!(
                    "https://api.github.com/repos/{}/{}",
                    repo.owner, repo.name
                );
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
}

pub async fn list_repos(
    db_path: Option<PathBuf>,
    wow_dir: Option<String>,
    fix_casing: bool,
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
        // Fix repo owner/name casing from forge APIs (best-effort).
        // On first launch after the v4 migration (needs_casing_fix), always run.
        // Otherwise only run when explicitly requested (manual rescan).
        if fix_casing || eng.db().needs_casing_fix() {
            fix_repo_casing_from_forges(&eng);
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
    check_updates_skip(db_path, wow_dir, mode, std::collections::HashSet::new()).await
}

pub async fn check_updates_skip(
    db_path: Option<PathBuf>,
    wow_dir: Option<String>,
    mode: CheckMode,
    skip_repo_ids: std::collections::HashSet<i64>,
) -> Result<Vec<PlanRow>, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let plans = tokio::runtime::Handle::current()
            .block_on(async {
                eng.check_updates_with_wow_skip(wow_dir.as_deref().map(Path::new), mode, &skip_repo_ids)
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

pub async fn set_merge_installs(
    db_path: Option<PathBuf>,
    repo_id: i64,
    merge: bool,
) -> Result<i64, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_repo_merge_installs(repo_id, merge)
            .map_err(|e| e.to_string())?;
        Ok(repo_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn set_pinned_version(
    db_path: Option<PathBuf>,
    repo_id: i64,
    version: Option<String>,
) -> Result<i64, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_repo_pinned_version(repo_id, version)
            .map_err(|e| e.to_string())?;
        Ok(repo_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Release tag + name for the version picker dropdown.
#[derive(Debug, Clone)]
pub struct VersionItem {
    pub tag: String,
    pub name: Option<String>,
}

/// Fetch all release versions for a repo URL using the engine's forge API.
pub async fn list_repo_versions(
    db_path: Option<PathBuf>,
    repo_url: String,
) -> Result<Vec<VersionItem>, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let releases = tokio::runtime::Handle::current()
            .block_on(eng.list_releases(&repo_url))
            .map_err(|e| e.to_string())?;
        Ok(releases
            .into_iter()
            .map(|r| VersionItem {
                tag: r.tag,
                name: r.name,
            })
            .collect())
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
    /// Decoded image handles keyed by URL. Handle IDs are stable so iced can cache decoded images
    /// across renders without re-decoding on every frame.
    pub image_cache: std::collections::HashMap<String, iced::widget::image::Handle>,
    /// Decoded GIF frames keyed by URL (for animated images in READMEs).
    pub gif_cache: std::collections::HashMap<String, std::sync::Arc<iced_gif::Frames>>,
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

/// Convert `<img src="..." alt="...">` HTML tags in markdown text to standard
/// `![alt](url)` syntax so iced's pulldown-cmark parser creates `Item::Image` entries.
/// Also strips `<p>`, `</p>`, and `<br>` tags that GitHub injects around images.
pub fn convert_html_images_to_markdown(markdown: &str) -> String {
    let mut result = String::with_capacity(markdown.len());
    let mut pos = 0;
    while pos < markdown.len() {
        match markdown[pos..].find("<img") {
            None => {
                result.push_str(&markdown[pos..]);
                break;
            }
            Some(tag_offset) => {
                result.push_str(&markdown[pos..pos + tag_offset]);
                let tag_start = pos + tag_offset;
                let tag_end = markdown[tag_start..].find('>')
                    .map(|e| tag_start + e + 1)
                    .unwrap_or(markdown.len());
                let tag_slice = &markdown[tag_start..tag_end];
                // Extract src= attribute
                let src = extract_attr(tag_slice, "src");
                let alt = extract_attr(tag_slice, "alt").unwrap_or_default();
                if let Some(url) = src {
                    result.push_str(&format!("![{}]({})", alt, url));
                } else {
                    result.push_str(tag_slice);
                }
                pos = tag_end;
            }
        }
    }
    // Strip <p>, </p>, <br>, <br/>, <br /> tags that GitHub wraps around images
    let result = result.replace("<p>", "").replace("</p>", "").replace("<br>", "\n")
        .replace("<br/>", "\n").replace("<br />", "\n");
    result
}

fn extract_attr<'a>(tag: &'a str, attr_name: &str) -> Option<String> {
    let needle = format!("{}=", attr_name);
    let attr_pos = tag.find(&needle)?;
    let after = &tag[attr_pos + needle.len()..];
    let q = after.chars().next()?;
    if q == '"' || q == '\'' {
        let inner = &after[1..];
        let end = inner.find(q)?;
        Some(inner[..end].trim().to_string())
    } else {
        // Unquoted attribute value — take until space or >
        let end = after.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(after.len());
        Some(after[..end].trim().to_string())
    }
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

/// Fetch images for URLs found in the README.
/// Returns two caches: static image handles and animated GIF frames.
/// Handles are created once here so their IDs are fixed — iced can then cache the decoded
/// pixels across renders without re-decoding on every frame.
/// Limits: max 12 images, 5 MB each, 20 MB total.
async fn fetch_images(
    client: &Client,
    image_urls: &[String],
    raw_base_url: &str,
) -> (
    std::collections::HashMap<String, iced::widget::image::Handle>,
    std::collections::HashMap<String, std::sync::Arc<iced_gif::Frames>>,
) {
    let mut image_cache = std::collections::HashMap::new();
    let mut gif_cache = std::collections::HashMap::new();
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
                    return Ok((Default::default(), false));
                }
                let is_gif = ct == "image/gif"
                    || fetch_url.split('?').next().unwrap_or("").ends_with(".gif");
                resp.bytes().await.map(|b| (b, is_gif))
            },
        ).await;

        if let Ok(Ok((bytes, is_gif))) = result {
            if !bytes.is_empty() && bytes.len() <= 5_000_000 {
                total_bytes += bytes.len();
                if is_gif {
                    // Decode animated GIF frames for iced_gif widget.
                    if let Ok(frames) = iced_gif::Frames::from_bytes(bytes.to_vec()) {
                        let frames = std::sync::Arc::new(frames);
                        gif_cache.insert(url.clone(), frames.clone());
                        if abs_url != *url {
                            gif_cache.insert(abs_url, frames);
                        }
                    } else {
                        // Fall back to static handle if decoding fails.
                        let handle = iced::widget::image::Handle::from_bytes(bytes);
                        image_cache.insert(url.clone(), handle.clone());
                        if abs_url != *url {
                            image_cache.insert(abs_url, handle);
                        }
                    }
                } else {
                    // Create the handle once — its Id is fixed for the lifetime of this preview,
                    // so iced can cache the decoded image across renders.
                    let handle = iced::widget::image::Handle::from_bytes(bytes);
                    // Store by original URL (as seen in markdown) AND absolute URL
                    image_cache.insert(url.clone(), handle.clone());
                    if abs_url != *url {
                        image_cache.insert(abs_url, handle);
                    }
                }
            }
        }
    }
    (image_cache, gif_cache)
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
    // Convert HTML <img> tags to markdown syntax so iced's parser creates Image items
    let readme_md = convert_html_images_to_markdown(&readme_text);
    let md_content = iced::widget::markdown::Content::parse(&readme_md);
    let readme_items: Vec<iced::widget::markdown::Item> = md_content.items().to_vec();
    let image_urls: Vec<String> = md_content.images().iter().cloned().collect();
    let (image_cache, gif_cache) = fetch_images(client, &image_urls, &raw_base).await;

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
        gif_cache,
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
    let readme_md = convert_html_images_to_markdown(&readme_text);
    let md_content = iced::widget::markdown::Content::parse(&readme_md);
    let readme_items: Vec<iced::widget::markdown::Item> = md_content.items().to_vec();
    let image_urls: Vec<String> = md_content.images().iter().cloned().collect();
    let (image_cache, gif_cache) = fetch_images(client, &image_urls, &raw_base).await;
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
        gif_cache,
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
    let readme_md = convert_html_images_to_markdown(&readme_text);
    let md_content = iced::widget::markdown::Content::parse(&readme_md);
    let readme_items: Vec<iced::widget::markdown::Item> = md_content.items().to_vec();
    let image_urls: Vec<String> = md_content.images().iter().cloned().collect();
    let (image_cache, gif_cache) = fetch_images(client, &image_urls, &raw_base).await;
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
        gif_cache,
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

/// Write the generated dxvk.conf content to the given path.
pub async fn save_dxvk_conf(path: std::path::PathBuf, content: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        std::fs::write(&path, content.as_bytes()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Self-update: download, apply, restart
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SelfUpdateStatus {
    pub supported: bool,
    pub update_available: bool,
    pub assets_pending: bool,
    pub latest_version: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct GhReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GhReleaseFull {
    tag_name: String,
    assets: Vec<GhReleaseAsset>,
}

fn normalize_tag(raw: &str) -> String {
    raw.trim().trim_start_matches(['v', 'V']).trim().to_string()
}

/// Split a version string into its numeric core and whether it has a
/// pre-release suffix (alpha, beta, rc, etc.).
fn parse_version_parts(raw: &str) -> (Vec<u64>, bool) {
    let tag = normalize_tag(raw);
    let is_prerelease = tag.contains("alpha")
        || tag.contains("beta")
        || tag.contains("rc")
        || tag.contains("dev");
    let nums: Vec<u64> = tag
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u64>().ok())
        .collect();
    // For pre-release tags, only keep the first 3 segments (major.minor.patch)
    // so that e.g. "3.0.0-beta.8" compares as [3,0,0] pre-release, not [3,0,0,8].
    let core = if is_prerelease { nums.into_iter().take(3).collect() } else { nums };
    (core, is_prerelease)
}

fn is_version_newer(latest: &str, current: &str) -> bool {
    let (a, a_pre) = parse_version_parts(latest);
    let (b, b_pre) = parse_version_parts(current);
    let max = a.len().max(b.len());
    for i in 0..max {
        let av = *a.get(i).unwrap_or(&0);
        let bv = *b.get(i).unwrap_or(&0);
        if av > bv { return true; }
        if av < bv { return false; }
    }
    // Same numeric core: a stable release is newer than a pre-release.
    // e.g. 3.0.0 is newer than 3.0.0-beta.8
    if !a_pre && b_pre { return true; }
    false
}

async fn fetch_release_full(beta_channel: bool) -> Result<GhReleaseFull, String> {
    let url = if beta_channel { WUDDLE_RELEASE_API_ALL } else { WUDDLE_RELEASE_API_LATEST };
    let client = Client::builder()
        .user_agent(concat!("wuddle/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = tokio::time::timeout(
        Duration::from_secs(25),
        client.get(url).header("Accept", "application/vnd.github+json").send(),
    )
    .await
    .map_err(|_| "Timed out fetching release".to_string())?
    .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API error: HTTP {}", resp.status()));
    }

    if beta_channel {
        let releases: Vec<GhReleaseFull> = resp.json().await.map_err(|e| e.to_string())?;
        releases.into_iter().next().ok_or_else(|| "No releases found".to_string())
    } else {
        resp.json().await.map_err(|e| e.to_string())
    }
}

async fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    let client = Client::builder()
        .user_agent(concat!("wuddle/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .send()
        .await
        .map_err(|e| format!("download: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("download HTTP {}", resp.status()));
    }
    resp.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
}

/// Check whether self-update is supported and whether an update is available.
pub async fn check_self_update_full(beta_channel: bool) -> Result<SelfUpdateStatus, String> {
    let current = env!("CARGO_PKG_VERSION");
    let supported = is_self_update_supported();

    let release = match fetch_release_full(beta_channel).await {
        Ok(r) => r,
        Err(e) => return Ok(SelfUpdateStatus {
            supported,
            update_available: false,
            assets_pending: false,
            latest_version: None,
            message: format!("Version check failed: {}", e),
        }),
    };

    let latest = normalize_tag(&release.tag_name);
    let newer = !latest.is_empty() && is_version_newer(&latest, current);
    let has_asset = newer && pick_platform_asset(&release).is_some();

    let message = if !supported {
        format!("v{} — self-update not supported for this install type", latest)
    } else if newer && !has_asset {
        format!("v{} available but assets still building — try again shortly", latest)
    } else if newer {
        format!("Update available: v{}", latest)
    } else {
        "Up to date".to_string()
    };

    let assets_pending = newer && !has_asset;

    Ok(SelfUpdateStatus {
        supported,
        update_available: has_asset && supported,
        assets_pending,
        latest_version: if latest.is_empty() { None } else { Some(latest) },
        message,
    })
}

fn is_self_update_supported() -> bool {
    #[cfg(target_os = "linux")]
    { return is_appimage().is_some(); }
    #[cfg(target_os = "windows")]
    { return detect_launcher_root().map(|r| r.1).unwrap_or(false); }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    { return false; }
}

fn pick_platform_asset(release: &GhReleaseFull) -> Option<&GhReleaseAsset> {
    #[cfg(target_os = "linux")]
    {
        release.assets.iter().find(|a| {
            let lower = a.name.to_ascii_lowercase();
            lower.ends_with(".appimage")
        })
    }
    #[cfg(target_os = "windows")]
    {
        release.assets.iter().find(|a| {
            let lower = a.name.to_ascii_lowercase();
            lower.contains("windows") && lower.ends_with(".zip")
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    { None }
}

/// Download and apply the latest release. Returns a status message.
pub async fn apply_self_update(beta_channel: bool) -> Result<String, String> {
    let current = env!("CARGO_PKG_VERSION");
    let release = fetch_release_full(beta_channel).await?;
    let latest = normalize_tag(&release.tag_name);

    if latest.is_empty() {
        return Err("Latest release tag is empty".to_string());
    }
    if !is_version_newer(&latest, current) {
        return Ok(format!("Already up to date (v{}).", current));
    }

    let asset = pick_platform_asset(&release)
        .ok_or_else(|| "No compatible asset found in release".to_string())?;
    let url = asset.browser_download_url.clone();
    let asset_name = asset.name.clone();

    let bytes = download_bytes(&url).await?;

    // Apply in a blocking task (filesystem I/O)
    let latest_clone = latest.clone();
    tokio::task::spawn_blocking(move || {
        apply_downloaded_update(&bytes, &asset_name, &latest_clone)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn apply_downloaded_update(bytes: &[u8], _asset_name: &str, latest: &str) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let appimage_path = is_appimage()
            .ok_or_else(|| "Not running as AppImage; self-update unavailable.".to_string())?;

        // Clean up stale temp files
        if let Some(parent) = appimage_path.parent() {
            if let Some(stem) = appimage_path.file_stem().and_then(|s| s.to_str()) {
                if let Ok(entries) = std::fs::read_dir(parent) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let name = name.to_string_lossy();
                        if name.starts_with(stem) && name.contains(".tmp-") {
                            let _ = std::fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let tmp_path = appimage_path.with_extension(format!("tmp-{}", stamp));

        std::fs::write(&tmp_path, bytes)
            .map_err(|e| format!("Failed to write temp file: {e}"))?;

        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to chmod: {e}"))?;

        std::fs::rename(&tmp_path, &appimage_path)
            .map_err(|e| format!("Failed to replace AppImage: {e}"))?;

        Ok(format!("Updated to v{}. Restart to apply.", latest))
    }

    #[cfg(target_os = "windows")]
    {
        let (root, launcher_layout) = detect_launcher_root()
            .map_err(|e| format!("Cannot detect install layout: {e}"))?;
        if !launcher_layout {
            return Err("Launcher layout not detected. Install the latest portable package once to enable in-app updates.".to_string());
        }

        // Extract Wuddle-bin.exe from the zip into versions/<tag>/
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| format!("Failed to open zip: {e}"))?;

        let sanitized = sanitize_version_name(latest);
        let version_dir = root.join("versions").join(&sanitized);
        std::fs::create_dir_all(&version_dir).map_err(|e| e.to_string())?;

        let mut found_runtime = false;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
            if file.is_dir() { continue; }
            let name = file.name().replace('\\', "/");
            let lower = name.to_ascii_lowercase();
            if lower.ends_with("/wuddle-bin.exe") || lower == "wuddle-bin.exe" {
                let target = version_dir.join("Wuddle-bin.exe");
                let mut out = std::fs::File::create(&target).map_err(|e| e.to_string())?;
                std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
                found_runtime = true;
                break;
            }
        }
        if !found_runtime {
            return Err("Wuddle-bin.exe not found in update zip".to_string());
        }

        // Update current.json
        let current_json = serde_json::json!({ "current": format!("v{}", sanitized) });
        std::fs::write(root.join("current.json"), current_json.to_string().as_bytes())
            .map_err(|e| format!("Failed to write current.json: {e}"))?;

        Ok(format!("Staged v{}. Restart to apply.", latest))
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (bytes, _asset_name, latest);
        Err("Self-update not supported on this platform".to_string())
    }
}

/// Restart the application after a successful update.
pub fn restart_app() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let appimage_path = is_appimage()
            .ok_or_else(|| "Not running as AppImage; cannot restart.".to_string())?;
        Command::new(&appimage_path)
            .spawn()
            .map_err(|e| format!("Failed to relaunch: {e}"))?;
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(200));
            std::process::exit(0);
        });
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let (root, _) = detect_launcher_root()
            .map_err(|e| format!("Cannot detect launcher: {e}"))?;
        let launcher = root.join("Wuddle.exe");
        if !launcher.is_file() {
            return Err(format!("Launcher not found at {}", launcher.display()));
        }
        Command::new(&launcher)
            .current_dir(&root)
            .spawn()
            .map_err(|e| format!("Failed to relaunch: {e}"))?;
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(200));
            std::process::exit(0);
        });
        Ok(())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Err("Restart not supported on this platform".to_string())
    }
}

#[cfg(target_os = "linux")]
fn is_appimage() -> Option<PathBuf> {
    let path = std::env::var("APPIMAGE").ok()?;
    let p = PathBuf::from(path);
    if p.is_file() { Some(p) } else { None }
}

#[cfg(target_os = "windows")]
fn detect_launcher_root() -> Result<(PathBuf, bool), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // Walk up to find the root that contains Wuddle.exe (launcher) and versions/
    let mut dir = exe.parent().map(|p| p.to_path_buf());
    for _ in 0..4 {
        if let Some(ref d) = dir {
            let launcher = d.join("Wuddle.exe");
            let versions = d.join("versions");
            if launcher.is_file() && versions.is_dir() {
                return Ok((d.clone(), true));
            }
            dir = d.parent().map(|p| p.to_path_buf());
        } else {
            break;
        }
    }
    // No launcher layout found
    let root = exe.parent().unwrap_or(Path::new(".")).to_path_buf();
    Ok((root, false))
}

// ---------------------------------------------------------------------------
// GitHub rate limit info
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GitHubRateInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset_epoch: i64,
}

pub async fn fetch_github_rate_limit() -> Option<GitHubRateInfo> {
    #[derive(Deserialize)]
    struct RateLimitResponse { rate: RateCore }
    #[derive(Deserialize)]
    struct RateCore { limit: u32, remaining: u32, reset: i64 }

    let mut req = reqwest::Client::new()
        .get("https://api.github.com/rate_limit")
        .header("User-Agent", concat!("Wuddle/", env!("CARGO_PKG_VERSION")));

    if let Some(token) = wuddle_engine::github_token() {
        req = req.bearer_auth(token);
    }

    let resp = req.send().await.ok()?;
    let data: RateLimitResponse = resp.json().await.ok()?;
    Some(GitHubRateInfo {
        limit: data.rate.limit,
        remaining: data.rate.remaining,
        reset_epoch: data.rate.reset,
    })
}

#[cfg(target_os = "windows")]
fn sanitize_version_name(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            out.push(ch);
        }
    }
    if out.is_empty() { "latest".to_string() } else { out }
}
