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

/// Collect image URLs by scanning raw markdown text for:
///   - ![alt](url) markdown syntax
///   - <img src="url"> / <img src='url'> HTML syntax
fn collect_image_urls_from_text(markdown: &str) -> Vec<String> {
    let mut urls = Vec::new();

    // --- Markdown syntax: ![alt](url) ---
    let mut pos = 0;
    while pos < markdown.len() {
        match markdown[pos..].find("![") {
            None => break,
            Some(bang_offset) => {
                let abs = pos + bang_offset;
                match markdown[abs + 2..].find("](") {
                    None => { pos = abs + 2; continue; }
                    Some(close_offset) => {
                        let url_start = abs + 2 + close_offset + 2;
                        match markdown[url_start..].find(')') {
                            None => { pos = abs + 2; continue; }
                            Some(end_offset) => {
                                let raw = markdown[url_start..url_start + end_offset].trim();
                                // Strip optional title: url "title" or url 'title'
                                let url = raw
                                    .find(|c: char| c == ' ' || c == '"' || c == '\'')
                                    .map(|i| raw[..i].trim())
                                    .unwrap_or(raw);
                                if !url.is_empty() {
                                    urls.push(url.to_string());
                                }
                                pos = url_start + end_offset + 1;
                            }
                        }
                    }
                }
            }
        }
    }

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

    for url in image_urls.iter().take(12) {
        if total_bytes > 20_000_000 { break; }

        let abs_url = resolve_image_url(url, raw_base_url);

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            async {
                let resp = client.get(&abs_url).send().await?;
                if !resp.status().is_success() {
                    return Err(reqwest::Error::from(resp.error_for_status().unwrap_err()));
                }
                resp.bytes().await
            },
        ).await;

        if let Ok(Ok(bytes)) = result {
            if bytes.len() <= 5_000_000 {
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
    let readme_items: Vec<iced::widget::markdown::Item> = iced::widget::markdown::parse(&readme_text).collect();
    let image_urls = collect_image_urls_from_text(&readme_text);
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
    let readme_items: Vec<iced::widget::markdown::Item> = iced::widget::markdown::parse(&readme_text).collect();
    let image_urls = collect_image_urls_from_text(&readme_text);
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
}

async fn fetch_gitea_preview(client: &Client, host: &str, scheme: &str, owner: &str, repo: &str) -> Result<RepoPreviewInfo, String> {
    let api_url = format!("{}://{}/api/v1/repos/{}/{}", scheme, host, owner, repo);
    let info: GiteaRepo = client.get(&api_url).send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let readme_url = format!("{}://{}/{}/{}/raw/branch/master/README.md", scheme, host, owner, repo);
    let readme_text = match client.get(&readme_url).send().await {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        _ => String::new(),
    };

    let raw_base = format!("{}://{}/{}/{}/raw/branch/master/", scheme, host, owner, repo);
    let readme_items: Vec<iced::widget::markdown::Item> = iced::widget::markdown::parse(&readme_text).collect();
    let image_urls = collect_image_urls_from_text(&readme_text);
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
            Ok(releases.into_iter().map(|r| ReleaseItem {
                tag_name: r.tag_name.clone(),
                name: r.name.filter(|s| !s.is_empty()).unwrap_or_else(|| r.tag_name),
                published_at: r.published_at.unwrap_or_default(),
                body: r.body.unwrap_or_default(),
                prerelease: r.prerelease,
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
            Ok(releases.into_iter().map(|r| ReleaseItem {
                tag_name: r.tag_name.clone(),
                name: r.name.filter(|s| !s.is_empty()).unwrap_or_else(|| r.tag_name),
                published_at: r.released_at.unwrap_or_default(),
                body: r.description.unwrap_or_default(),
                prerelease: false,
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
            Ok(releases.into_iter().map(|r| ReleaseItem {
                tag_name: r.tag_name.clone(),
                name: r.name.filter(|s| !s.is_empty()).unwrap_or_else(|| r.tag_name),
                published_at: r.published_at.unwrap_or_default(),
                body: r.body.unwrap_or_default(),
                prerelease: r.prerelease,
            }).collect())
        }
    }
}

// ---------------------------------------------------------------------------
// Self-update: fetch latest GitHub release tag
// ---------------------------------------------------------------------------

const WUDDLE_RELEASE_API: &str = "https://api.github.com/repos/ZythDr/Wuddle/releases/latest";
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

pub async fn check_self_update() -> Result<String, String> {
    #[derive(Deserialize)]
    struct GhRelease { tag_name: String }

    let client = Client::builder()
        .user_agent(concat!("wuddle/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = tokio::time::timeout(
        Duration::from_secs(12),
        client.get(WUDDLE_RELEASE_API)
            .header("Accept", "application/vnd.github+json")
            .send(),
    )
    .await
    .map_err(|_| "Timed out checking for updates".to_string())?
    .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API error: HTTP {}", resp.status()));
    }

    let release: GhRelease = resp.json().await.map_err(|e| e.to_string())?;
    Ok(release.tag_name)
}
