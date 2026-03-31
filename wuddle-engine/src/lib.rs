use anyhow::{Context, Result};
use git2::Repository;
use reqwest::Client;
use std::{
    collections::{HashMap, HashSet},
    fs,
    future::Future,
    io::Read,
    path::{Component, Path, PathBuf},
    pin::Pin,
    sync::{LazyLock, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

mod db;
mod forge;
mod install;
mod model;
mod util;

pub use db::Db;
pub use install::InstallOptions;
pub use model::{InstallMode, LatestRelease, ReleaseAsset, Repo};

use crate::forge::detect_repo;
use crate::forge::git_sync;
use crate::forge::ForgeKind;
// LatestRelease and ReleaseAsset re-exported via `pub use model::` above.

#[derive(Debug, Clone)]
pub struct UpdatePlan {
    pub repo_id: i64,
    pub forge: String,
    pub host: String,
    pub owner: String,
    pub name: String,
    pub url: String,

    pub mode: InstallMode,

    pub current: Option<String>,
    pub latest: String,

    pub asset_id: String,
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: Option<u64>,
    pub asset_sha256: Option<String>,

    pub repair_needed: bool,
    pub externally_modified: bool,
    pub not_modified: bool,
    pub applied: bool,
    pub error: Option<String>,

    /// Additional assets to install alongside the primary one.
    /// Only populated for Dll-mode repos that publish multiple individual .dll files.
    pub extra_assets: Vec<ReleaseAsset>,

    /// Number of DLL install entries currently tracked for this repo.
    pub previous_dll_count: usize,
    /// Number of DLL files in the new release (primary + extras).
    pub new_dll_count: usize,
}

/// Controls how aggressively the engine checks for updates.
/// When no GitHub token is configured, adaptive frequency skips repos
/// whose latest release is old (stable/dormant) to conserve API quota.
#[derive(Debug, Clone, Copy)]
pub enum CheckMode {
    /// User clicked "Check for updates". Skips stable/dormant repos (no token).
    Manual,
    /// Auto-check timer fired. Cycle-based modulo skipping (no token).
    Auto { cycle: u32 },
    /// Always check everything (startup, post-install, token save, etc.).
    Force,
}

impl CheckMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "manual" => CheckMode::Manual,
            "force" => CheckMode::Force,
            other => {
                if let Some(n) = other.strip_prefix("auto:") {
                    if let Ok(cycle) = n.parse::<u32>() {
                        return CheckMode::Auto { cycle };
                    }
                }
                CheckMode::Force
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum RepoActivity {
    Active,
    Stable,
    Dormant,
}

fn classify_repo_activity(published_at_unix: i64, now_unix: i64) -> RepoActivity {
    let age_days = now_unix.saturating_sub(published_at_unix) / 86400;
    if age_days < 30 {
        RepoActivity::Active
    } else if age_days < 90 {
        RepoActivity::Stable
    } else {
        RepoActivity::Dormant
    }
}

/// Returns true if this repo should be skipped to conserve API quota.
fn should_skip_adaptive(
    check_mode: CheckMode,
    published_at_unix: Option<i64>,
    now_unix: i64,
    has_token: bool,
) -> bool {
    if has_token {
        return false;
    }
    if matches!(check_mode, CheckMode::Force) {
        return false;
    }
    let pub_at = match published_at_unix {
        Some(v) => v,
        None => return false, // unknown = always check
    };
    let activity = classify_repo_activity(pub_at, now_unix);
    match check_mode {
        CheckMode::Manual => matches!(activity, RepoActivity::Stable | RepoActivity::Dormant),
        CheckMode::Auto { cycle } => match activity {
            RepoActivity::Active => false,
            RepoActivity::Stable => cycle % 2 != 0,
            RepoActivity::Dormant => cycle % 4 != 0,
        },
        CheckMode::Force => false,
    }
}

pub struct Engine {
    db: Db,
    client: Client,
}

#[derive(Debug, Clone)]
pub struct AddonProbeOwner {
    pub repo_id: i64,
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct AddonProbeConflict {
    pub addon_name: String,
    pub target_path: String,
    pub owners: Vec<AddonProbeOwner>,
}

#[derive(Debug, Clone)]
pub struct AddonProbeResult {
    pub addon_names: Vec<String>,
    pub conflicts: Vec<AddonProbeConflict>,
    pub resolved_branch: String,
}

#[derive(Debug, Clone)]
struct AddonInstallConflict {
    addon_name: String,
    target_path: PathBuf,
    owners: Vec<db::AddonInstallOwner>,
}

static GITHUB_TOKEN: OnceLock<Mutex<Option<String>>> = OnceLock::new();

static RE_GITHUB_RESET: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"reset (\d+)").unwrap());

static RE_VERSION_FROM_ASSET: LazyLock<regex::Regex> = LazyLock::new(|| {
    // Suffix character class deliberately excludes '.' to avoid consuming file
    // extensions (e.g. "2.1-1.tar.gz" should match "2.1-1", not "2.1-1.tar.gz").
    regex::Regex::new(r"(?i)\bv?\d+(?:[._]\d+){1,3}(?:[-+][0-9A-Za-z-]+)?\b").unwrap()
});

fn github_token_state() -> &'static Mutex<Option<String>> {
    GITHUB_TOKEN.get_or_init(|| Mutex::new(None))
}

pub fn set_github_token(token: Option<String>) {
    let normalized = token
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    if let Ok(mut guard) = github_token_state().lock() {
        *guard = normalized;
    }
}

pub fn github_token() -> Option<String> {
    if let Ok(guard) = github_token_state().lock() {
        if let Some(token) = guard.clone() {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return Some(token);
            }
        }
    }
    std::env::var("WUDDLE_GITHUB_TOKEN")
        .ok()
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

impl Engine {
    pub fn open(db_path: &Path) -> Result<Self> {
        Ok(Self {
            db: Db::open(db_path)?,
            client: Client::builder().user_agent("wuddle-engine").build()?,
        })
    }

    pub fn open_default() -> Result<Self> {
        let db_path = util::db_path()?;
        Self::open(&db_path)
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub fn add_repo(
        &self,
        url: &str,
        mode: InstallMode,
        asset_regex: Option<String>,
    ) -> Result<i64> {
        let det = detect_repo(url)?;
        let is_addon_git = matches!(&mode, InstallMode::AddonGit);

        let repo = Repo {
            id: 0,
            url: det.canonical_url.clone(),
            forge: det.forge_str.to_string(),
            host: det.host.clone(),
            owner: det.owner.clone(),
            name: det.name.clone(),
            mode,
            enabled: true,
            git_branch: if is_addon_git {
                Some("master".to_string())
            } else {
                None
            },
            asset_regex,
            last_version: None,
            etag: None,
            installed_asset_id: None,
            installed_asset_name: None,
            installed_asset_size: None,
            installed_asset_url: None,
            published_at_unix: None,
            merge_installs: false,
            pinned_version: None,
        };

        self.db.add_repo(&repo)
    }

    pub async fn probe_addon_repo_conflicts(
        &self,
        url: &str,
        wow_dir: &Path,
        preferred_branch: Option<&str>,
    ) -> Result<AddonProbeResult> {
        let preferred_branch = preferred_branch
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .unwrap_or("master");

        let probe_dir = tempfile::tempdir().context("create addon probe dir")?;
        let synced = git_sync::sync_repo(url, probe_dir.path(), Some(preferred_branch))
            .with_context(|| format!("git sync {}", url))?;

        let mut detected = install::detect_addons_in_tree(probe_dir.path());
        detected.sort_by_key(|(src, name)| (src.components().count(), name.clone()));

        let mut addon_names = Vec::<String>::new();
        let mut seen_names = HashSet::<String>::new();
        for (_, addon_name) in detected {
            let key = addon_name.to_lowercase();
            if seen_names.insert(key) {
                addon_names.push(addon_name);
            }
        }
        if addon_names.is_empty() {
            anyhow::bail!(
                "No addon .toc files found in synced repo. Expected at least one addon folder."
            );
        }

        let mut conflicts = Vec::<AddonProbeConflict>::new();
        for addon_name in &addon_names {
            let target_path = wow_dir.join("Interface").join("AddOns").join(addon_name);
            let manifest_path = Self::to_manifest_path(&target_path, wow_dir);
            let owners = self.db.find_addon_install_owners(&manifest_path, None)?;
            let has_local_conflict = Self::path_has_conflicting_content(&target_path);
            if owners.is_empty() && !has_local_conflict {
                continue;
            }

            conflicts.push(AddonProbeConflict {
                addon_name: addon_name.clone(),
                target_path: target_path.display().to_string(),
                owners: owners
                    .into_iter()
                    .map(|o| AddonProbeOwner {
                        repo_id: o.repo_id,
                        owner: o.owner,
                        name: o.name,
                    })
                    .collect(),
            });
        }

        Ok(AddonProbeResult {
            addon_names,
            conflicts,
            resolved_branch: synced.branch,
        })
    }

    fn blank_plan(r: &Repo) -> UpdatePlan {
        let current = Self::normalized_current_version(r);
        UpdatePlan {
            repo_id: r.id,
            forge: r.forge.clone(),
            host: r.host.clone(),
            owner: r.owner.clone(),
            name: r.name.clone(),
            url: r.url.clone(),
            mode: r.mode.clone(),
            current: current.clone(),
            latest: current.unwrap_or_else(|| "unknown".to_string()),
            asset_id: "".to_string(),
            asset_name: "".to_string(),
            asset_url: "".to_string(),
            asset_size: None,
            asset_sha256: None,
            repair_needed: false,
            externally_modified: false,
            not_modified: false,
            applied: false,
            error: None,
            extra_assets: Vec::new(),
            previous_dll_count: 0,
            new_dll_count: 0,
        }
    }

    fn now_unix() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    fn parse_github_reset_epoch(msg: &str) -> Option<i64> {
        let caps = RE_GITHUB_RESET.captures(msg)?;
        caps.get(1)?.as_str().parse::<i64>().ok()
    }

    fn has_github_token() -> bool {
        github_token().is_some()
    }

    fn rate_limited_plan(r: &Repo, reset_epoch: i64) -> UpdatePlan {
        let mut p = Self::blank_plan(r);
        p.error = Some(format!(
            "GitHub API rate-limited for {} until unix {}. Add a GitHub token in Wuddle settings to raise limits.",
            r.host, reset_epoch
        ));
        p
    }

    fn effective_asset_id(asset: &ReleaseAsset) -> String {
        asset
            .id
            .clone()
            .unwrap_or_else(|| util::sha256_hex(&asset.download_url))
    }

    fn size_u64_to_i64(v: Option<u64>) -> Option<i64> {
        v.and_then(|n| i64::try_from(n).ok())
    }

    fn installed_matches(
        r: &Repo,
        latest_tag: &str,
        latest_asset_id: &str,
        latest_asset_name: &str,
        latest_asset_size: Option<i64>,
    ) -> bool {
        if let Some(stored_id) = r.installed_asset_id.as_deref() {
            let name_match = r.installed_asset_name.as_deref() == Some(latest_asset_name);
            let size_match = r.installed_asset_size == latest_asset_size;
            return stored_id == latest_asset_id && name_match && size_match;
        }

        // Backward compatibility with old DBs that only had last_version.
        matches!(
            Self::normalized_current_version(r).as_deref(),
            Some(cur) if cur == latest_tag
        )
    }

    fn is_generic_release_label(label: &str) -> bool {
        let l = label.trim().to_ascii_lowercase();
        if l.is_empty() {
            return true;
        }
        matches!(
            l.as_str(),
            "release" | "latest" | "stable" | "current" | "download"
        ) || l.starts_with("release ")
            || l.starts_with("latest ")
            || l.starts_with("stable ")
    }

    fn version_from_asset_name(asset_name: &str) -> Option<String> {
        // Extract semver-like fragments, e.g. "SuperWoW 1.5.1.zip" -> "1.5.1"
        let m = RE_VERSION_FROM_ASSET.find(asset_name)?;
        let mut v = m.as_str().trim().to_string();
        if v.is_empty() {
            return None;
        }
        v = v.replace('_', ".");
        Some(v)
    }

    fn effective_latest_label(tag: &str, asset_name: &str) -> String {
        let trimmed = tag.trim();
        if !Self::is_generic_release_label(trimmed) {
            return trimmed.to_string();
        }
        if let Some(v) = Self::version_from_asset_name(asset_name) {
            return v;
        }
        trimmed.to_string()
    }

    fn normalized_current_version(r: &Repo) -> Option<String> {
        let cur = r.last_version.clone()?;
        if !Self::is_generic_release_label(&cur) {
            return Some(cur);
        }
        if let Some(asset_name) = r.installed_asset_name.as_deref() {
            if let Some(v) = Self::version_from_asset_name(asset_name) {
                return Some(v);
            }
        }
        Some(cur)
    }

    fn normalize_rel_path(path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }

    fn to_manifest_path(path: &Path, wow_dir: &Path) -> String {
        if let Ok(rel) = path.strip_prefix(wow_dir) {
            return Self::normalize_rel_path(rel);
        }
        Self::normalize_rel_path(path)
    }

    fn has_missing_targets(&self, repo_id: i64, wow_dir: Option<&Path>) -> Result<bool> {
        let wow_dir = match wow_dir {
            Some(p) => p,
            None => return Ok(false),
        };

        let entries = self.db.list_installs(repo_id)?;
        if entries.is_empty() {
            return Ok(false);
        }

        for e in entries {
            let p = Path::new(&e.path);
            let full = if p.is_absolute() {
                p.to_path_buf()
            } else {
                wow_dir.join(p)
            };
            if !full.exists() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn is_mod_mode(mode: &InstallMode) -> bool {
        matches!(
            mode,
            InstallMode::Dll | InstallMode::Mixed | InstallMode::Raw | InstallMode::Auto
        )
    }

    /// Check whether any tracked file for this repo was modified externally.
    /// Returns `true` on the first hash mismatch. Skips addon dirs and entries
    /// without a stored hash (pre-migration installs).
    fn check_files_modified(&self, repo_id: i64, wow_dir: Option<&Path>) -> bool {
        let wow_dir = match wow_dir {
            Some(p) => p,
            None => return false,
        };
        let entries = match self.db.list_installs(repo_id) {
            Ok(v) => v,
            Err(_) => return false,
        };
        for e in entries {
            let stored = match e.sha256.as_deref() {
                Some(h) if !h.is_empty() => h,
                _ => continue,
            };
            if e.kind == "addon" {
                continue;
            }
            let full = match Self::resolve_install_path(&e.path, Some(wow_dir)) {
                Some(p) => p,
                None => continue,
            };
            if !full.is_file() {
                continue;
            }
            match util::sha256_file_hex(&full) {
                Ok(ref actual) if actual != stored => return true,
                _ => {}
            }
        }
        false
    }

    fn addon_git_worktree_dir(&self, repo_id: i64, wow_dir: &Path, repo: &Repo) -> PathBuf {
        // Check DB install entries for an existing valid git repo on disk.
        // This covers both the new direct location (Interface/AddOns/{name})
        // and any legacy staging paths from older Wuddle installs.
        if let Ok(entries) = self.db.list_installs(repo_id) {
            for entry in entries {
                let Some(full) = Self::resolve_install_path(&entry.path, Some(wow_dir)) else {
                    continue;
                };
                if !full.is_dir() || !Self::has_local_git_marker(&full) {
                    continue;
                }
                if Repository::open(&full).is_ok() {
                    return full;
                }
            }
        }
        // Default: clone directly into Interface/AddOns/{name} (GAM-compatible).
        git_sync::addon_direct_dir(wow_dir, &repo.name)
    }

    fn repo_key(host: &str, owner: &str, name: &str) -> String {
        format!(
            "{}|{}|{}",
            host.trim().to_ascii_lowercase(),
            owner.trim().to_ascii_lowercase(),
            name.trim().to_ascii_lowercase()
        )
    }

    fn normalize_git_remote_url(raw: &str) -> Option<String> {
        let url = raw.trim();
        if url.is_empty() {
            return None;
        }

        if url.starts_with("https://") || url.starts_with("http://") {
            return Some(url.to_string());
        }

        if url.starts_with("ssh://") || url.starts_with("git://") {
            let parsed = Url::parse(url).ok()?;
            let host = parsed.host_str()?.trim();
            if host.is_empty() {
                return None;
            }
            let path = parsed.path().trim().trim_start_matches('/');
            if path.is_empty() {
                return None;
            }
            return Some(format!("https://{}/{}", host, path));
        }

        // SCP-like SSH form, e.g. git@github.com:owner/repo.git
        if let Some(at_pos) = url.find('@') {
            let rest = &url[at_pos + 1..];
            if let Some(colon_pos) = rest.find(':') {
                let host = rest[..colon_pos].trim();
                let path = rest[colon_pos + 1..].trim();
                if !host.is_empty() && !path.is_empty() {
                    return Some(format!("https://{}/{}", host, path.trim_start_matches('/')));
                }
            }
        }

        None
    }

    fn local_repo_remote_url(repo: &Repository) -> Option<String> {
        if let Ok(origin) = repo.find_remote("origin") {
            if let Some(url) = origin.url() {
                let trimmed = url.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }

        let remotes = repo.remotes().ok()?;
        for name in remotes.iter().flatten() {
            let remote = match repo.find_remote(name) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let url = remote.url()?;
            let trimmed = url.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    fn local_repo_branch(repo: &Repository) -> Option<String> {
        let head = repo.head().ok()?;
        let branch = head.shorthand()?.trim();
        if branch.is_empty() || branch.eq_ignore_ascii_case("HEAD") {
            return None;
        }
        Some(branch.to_string())
    }

    fn local_repo_oid(repo: &Repository) -> Option<String> {
        repo.head()
            .ok()
            .and_then(|h| h.target())
            .map(|oid| oid.to_string())
    }

    fn local_repo_short_oid(repo: &Repository) -> Option<String> {
        Self::local_repo_oid(repo).map(|oid| oid.chars().take(10).collect())
    }

    fn has_local_git_marker(path: &Path) -> bool {
        path.join(".git").exists()
    }

    pub fn import_existing_addon_git_repos(&self, wow_dir: &Path) -> Result<usize> {
        let addons_root = wow_dir.join("Interface").join("AddOns");
        if !addons_root.is_dir() {
            return Ok(0);
        }
        // Credit: behavior inspired by GitAddonsManager UX — detect already-cloned
        // addon repos in AddOns and import them without forcing reinstallation.

        let existing = self.db.list_repos()?;
        let mut known = existing
            .iter()
            .map(|r| Self::repo_key(&r.host, &r.owner, &r.name))
            .collect::<HashSet<_>>();

        // Track addon folder paths already claimed by any repo, so we skip
        // importing a different fork that deploys to the same folders.
        let mut claimed_addon_paths = self.db.all_addon_install_paths()?;

        let mut imported = 0usize;

        let read_dir = match fs::read_dir(&addons_root) {
            Ok(v) => v,
            Err(_) => return Ok(0),
        };

        for entry in read_dir.flatten() {
            let root = entry.path();
            if !root.is_dir() {
                continue;
            }
            let folder_name = root
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            if folder_name.starts_with('.') {
                continue;
            }
            if !Self::has_local_git_marker(&root) {
                continue;
            }

            let repo = match Repository::open(&root) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let remote_raw = match Self::local_repo_remote_url(&repo) {
                Some(v) => v,
                None => continue,
            };
            let remote_url = match Self::normalize_git_remote_url(&remote_raw) {
                Some(v) => v,
                None => continue,
            };
            let det = match detect_repo(&remote_url) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let key = Self::repo_key(&det.host, &det.owner, &det.name);
            if known.contains(&key) {
                continue;
            }

            let detected_addons = install::detect_addons_in_tree(&root);
            if detected_addons.is_empty() {
                continue;
            }

            // Check if ALL addon folders this repo would deploy are already tracked
            // by another repo (e.g. a different fork). If so, skip the import to
            // prevent duplicate entries for the same on-disk addon.
            let candidate_paths: Vec<String> = detected_addons
                .iter()
                .map(|(_src, name)| {
                    let p = wow_dir.join("Interface").join("AddOns").join(name);
                    Self::to_manifest_path(&p, wow_dir).to_ascii_lowercase()
                })
                .collect();
            let all_claimed = candidate_paths
                .iter()
                .all(|p| claimed_addon_paths.contains(p));
            if all_claimed {
                continue;
            }

            let branch = Self::local_repo_branch(&repo).unwrap_or_else(|| "master".to_string());
            let short_oid = Self::local_repo_short_oid(&repo);
            let full_oid = Self::local_repo_oid(&repo);

            let tracked = Repo {
                id: 0,
                url: det.canonical_url.clone(),
                forge: det.forge_str.to_string(),
                host: det.host.clone(),
                owner: det.owner.clone(),
                name: det.name.clone(),
                mode: InstallMode::AddonGit,
                enabled: true,
                git_branch: Some(branch.clone()),
                asset_regex: None,
                last_version: short_oid.clone(),
                etag: None,
                installed_asset_id: full_oid.clone(),
                installed_asset_name: Some(format!("git:{}", branch)),
                installed_asset_size: None,
                installed_asset_url: Some(det.canonical_url.clone()),
                published_at_unix: None,
                merge_installs: false,
                pinned_version: None,
            };
            let repo_id = self.db.add_repo(&tracked)?;

            let mut addon_names = HashSet::<String>::new();
            for (_src_dir, addon_name) in detected_addons {
                if !addon_names.insert(addon_name.to_ascii_lowercase()) {
                    continue;
                }
                let install_path = wow_dir.join("Interface").join("AddOns").join(&addon_name);
                let manifest = Self::to_manifest_path(&install_path, wow_dir);
                self.db.add_install(repo_id, &manifest, "addon")?;
                claimed_addon_paths.insert(manifest.to_ascii_lowercase());
            }

            known.insert(key);
            imported += 1;
        }

        Ok(imported)
    }

    /// Remove duplicate addon_git repos that share the same on-disk addon
    /// folders. Keeps the repo whose git remote matches what's actually
    /// cloned on disk; removes the other(s).
    pub fn dedup_addon_repos_by_folder(&self, wow_dir: &Path) -> Result<usize> {
        let repos = self.db.list_repos()?;
        let addon_repos: Vec<&Repo> = repos
            .iter()
            .filter(|r| matches!(r.mode, InstallMode::AddonGit))
            .collect();
        if addon_repos.len() < 2 {
            return Ok(0);
        }

        // Map each addon install path → list of repo ids that claim it.
        let mut path_to_repos: HashMap<String, Vec<i64>> = HashMap::new();
        for r in &addon_repos {
            let installs = self.db.list_installs(r.id)?;
            for entry in installs {
                if entry.kind != "addon" {
                    continue;
                }
                path_to_repos
                    .entry(entry.path.to_ascii_lowercase())
                    .or_default()
                    .push(r.id);
            }
        }

        // Find repo ids that share at least one addon path with another repo.
        let mut contested_ids = HashSet::<i64>::new();
        for (_path, ids) in &path_to_repos {
            if ids.len() > 1 {
                for id in ids {
                    contested_ids.insert(*id);
                }
            }
        }
        if contested_ids.is_empty() {
            return Ok(0);
        }

        // For each contested repo, check if its worktree dir exists on disk
        // and if the actual git remote matches this repo's URL.
        let mut to_remove = Vec::<i64>::new();
        let repo_map: HashMap<i64, &Repo> = addon_repos.iter().map(|r| (r.id, *r)).collect();

        for &repo_id in &contested_ids {
            let r = match repo_map.get(&repo_id) {
                Some(r) => r,
                None => continue,
            };
            let worktree = self.addon_git_worktree_dir(repo_id, wow_dir, r);
            if !worktree.is_dir() || !Self::has_local_git_marker(&worktree) {
                // No local clone → this entry is stale, mark for removal.
                to_remove.push(repo_id);
                continue;
            }
            let git_repo = match Repository::open(&worktree) {
                Ok(v) => v,
                Err(_) => {
                    to_remove.push(repo_id);
                    continue;
                }
            };
            let remote_raw = match Self::local_repo_remote_url(&git_repo) {
                Some(v) => v,
                None => {
                    to_remove.push(repo_id);
                    continue;
                }
            };
            let remote_url = Self::normalize_git_remote_url(&remote_raw);
            let det = remote_url.as_deref().and_then(|u| detect_repo(u).ok());

            let matches = det
                .as_ref()
                .map(|d| {
                    d.host.eq_ignore_ascii_case(&r.host)
                        && d.owner.eq_ignore_ascii_case(&r.owner)
                        && d.name.eq_ignore_ascii_case(&r.name)
                })
                .unwrap_or(false);

            if !matches {
                // On-disk remote doesn't match this DB entry → stale.
                to_remove.push(repo_id);
            }
        }

        let mut removed = 0usize;
        for repo_id in to_remove {
            // Only remove tracking, don't delete files (the real repo still owns them).
            self.db.remove_repo(repo_id)?;
            removed += 1;
        }
        Ok(removed)
    }

    /// Remove repos from the database whose installed files no longer exist
    /// on disk at the given `wow_dir`.  This only untracks them — it never
    /// deletes any user files.
    ///
    /// A repo is pruned when it has **zero** install entries that resolve to
    /// an existing path, OR when it has no install entries at all and is not
    /// a manually-added (non-addon_git) repo that was never installed.
    pub fn prune_missing_repos(&self, wow_dir: &Path) -> Result<usize> {
        let repos = self.db.list_repos()?;
        let mut pruned = 0usize;

        for repo in &repos {
            let entries = match self.db.list_installs(repo.id) {
                Ok(e) => e,
                Err(_) => continue,
            };

            // Repos with no install entries were never installed.
            // For addon_git repos this means the clone is gone → prune.
            // For other modes (manually added by URL) that were never
            // installed, keep them — the user explicitly added them.
            if entries.is_empty() {
                if matches!(repo.mode, InstallMode::AddonGit) {
                    // Check if the git worktree still exists
                    let worktree = self.addon_git_worktree_dir(repo.id, wow_dir, repo);
                    if !worktree.is_dir() {
                        eprintln!("[prune] removing addon_git '{}' (no worktree at {:?})", repo.name, worktree);
                        self.db.remove_repo(repo.id)?;
                        pruned += 1;
                    }
                }
                continue;
            }

            // Check if ANY installed path still exists on disk.
            let any_present = entries.iter().any(|entry| {
                let resolved = Self::resolve_install_path(&entry.path, Some(wow_dir));
                let exists = resolved.as_ref().map(|full| full.exists()).unwrap_or(false);
                if !exists {
                    eprintln!("[prune] '{}' install entry '{}' -> {:?} exists={}", repo.name, entry.path, resolved, exists);
                }
                exists
            });

            if !any_present {
                eprintln!("[prune] removing '{}' ({}) — no install entries found on disk", repo.name, repo.mode.as_str());
                self.db.remove_repo(repo.id)?;
                pruned += 1;
            }
        }

        if pruned > 0 {
            eprintln!("[prune] pruned {} repos from database", pruned);
        }

        Ok(pruned)
    }

    fn build_git_addon_plan_for_repo(
        &self,
        r: &Repo,
        wow_dir: Option<&Path>,
    ) -> Result<UpdatePlan> {
        let wow_dir = match wow_dir {
            Some(p) => p,
            None => {
                let mut p = Self::blank_plan(r);
                p.error = Some("WoW path is required for addon git-sync mode.".to_string());
                return Ok(p);
            }
        };

        let worktree_dir = self.addon_git_worktree_dir(r.id, wow_dir, r);
        let local = match git_sync::local_head(&worktree_dir) {
            Ok(v) => v,
            Err(e) => {
                let mut p = Self::blank_plan(r);
                p.error = Some(e.to_string());
                return Ok(p);
            }
        };
        let preferred_branch = r
            .git_branch
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .unwrap_or("master");
        let remote = match git_sync::remote_head_for_branch(&r.url, Some(preferred_branch)) {
            Ok(v) => v,
            Err(e) => {
                let mut p = Self::blank_plan(r);
                p.current = local
                    .as_ref()
                    .map(|h| h.short_oid.clone())
                    .or_else(|| Self::normalized_current_version(r));
                p.error = Some(format!("Git sync check failed: {}", e));
                return Ok(p);
            }
        };

        let current = local
            .as_ref()
            .map(|h| h.short_oid.clone())
            .or_else(|| Self::normalized_current_version(r));
        let missing_targets = self.has_missing_targets(r.id, Some(wow_dir))?;
        let installed_matches = local.as_ref().map(|h| h.oid == remote.oid).unwrap_or(false);
        let needs_sync = !installed_matches || missing_targets;
        let repair_needed = missing_targets && current.is_some();

        Ok(UpdatePlan {
            repo_id: r.id,
            forge: r.forge.clone(),
            host: r.host.clone(),
            owner: r.owner.clone(),
            name: r.name.clone(),
            url: r.url.clone(),
            mode: r.mode.clone(),
            current,
            latest: remote.short_oid.clone(),
            asset_id: remote.oid.clone(),
            asset_name: format!("git:{}", remote.branch),
            asset_url: if needs_sync {
                r.url.clone()
            } else {
                "".to_string()
            },
            asset_size: None,
            asset_sha256: None,
            repair_needed,
            externally_modified: false,
            not_modified: false,
            applied: false,
            error: None,
            extra_assets: Vec::new(),
            previous_dll_count: 0,
            new_dll_count: 0,
        })
    }

    async fn build_git_addon_plan_for_repo_async(
        &self,
        r: &Repo,
        wow_dir: Option<&Path>,
    ) -> Result<UpdatePlan> {
        let wow_dir = match wow_dir {
            Some(p) => p,
            None => {
                let mut p = Self::blank_plan(r);
                p.error = Some("WoW path is required for addon git-sync mode.".to_string());
                return Ok(p);
            }
        };

        let worktree_dir = self.addon_git_worktree_dir(r.id, wow_dir, r);
        let local = match git_sync::local_head(&worktree_dir) {
            Ok(v) => v,
            Err(e) => {
                let mut p = Self::blank_plan(r);
                p.error = Some(e.to_string());
                return Ok(p);
            }
        };
        let preferred_branch = r
            .git_branch
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .unwrap_or("master")
            .to_string();

        let url = r.url.clone();
        let preferred_for_task = preferred_branch.clone();
        let remote = tokio::task::spawn_blocking(move || {
            git_sync::remote_head_for_branch(&url, Some(preferred_for_task.as_str()))
        })
        .await
        .map_err(|e| anyhow::anyhow!("Git sync worker failed: {}", e));
        let remote = match remote {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                let mut p = Self::blank_plan(r);
                p.current = local
                    .as_ref()
                    .map(|h| h.short_oid.clone())
                    .or_else(|| Self::normalized_current_version(r));
                p.error = Some(format!("Git sync check failed: {}", e));
                return Ok(p);
            }
            Err(e) => {
                let mut p = Self::blank_plan(r);
                p.current = local
                    .as_ref()
                    .map(|h| h.short_oid.clone())
                    .or_else(|| Self::normalized_current_version(r));
                p.error = Some(e.to_string());
                return Ok(p);
            }
        };

        let current = local
            .as_ref()
            .map(|h| h.short_oid.clone())
            .or_else(|| Self::normalized_current_version(r));
        let missing_targets = self.has_missing_targets(r.id, Some(wow_dir))?;
        let installed_matches = local.as_ref().map(|h| h.oid == remote.oid).unwrap_or(false);
        let needs_sync = !installed_matches || missing_targets;
        let repair_needed = missing_targets && current.is_some();

        Ok(UpdatePlan {
            repo_id: r.id,
            forge: r.forge.clone(),
            host: r.host.clone(),
            owner: r.owner.clone(),
            name: r.name.clone(),
            url: r.url.clone(),
            mode: r.mode.clone(),
            current,
            latest: remote.short_oid.clone(),
            asset_id: remote.oid.clone(),
            asset_name: format!("git:{}", remote.branch),
            asset_url: if needs_sync {
                r.url.clone()
            } else {
                "".to_string()
            },
            asset_size: None,
            asset_sha256: None,
            repair_needed,
            externally_modified: false,
            not_modified: false,
            applied: false,
            error: None,
            extra_assets: Vec::new(),
            previous_dll_count: 0,
            new_dll_count: 0,
        })
    }

    async fn build_update_plan_for_repo(
        &self,
        r: &Repo,
        use_cached_etag: bool,
        wow_dir: Option<&Path>,
        check_mode: CheckMode,
    ) -> Result<UpdatePlan> {
        if !r.enabled {
            return Ok(Self::blank_plan(r));
        }

        // Adaptive update frequency: skip repos with old releases to conserve API quota.
        if should_skip_adaptive(check_mode, r.published_at_unix, Self::now_unix(), Self::has_github_token()) {
            let mut p = Self::blank_plan(r);
            p.not_modified = true;
            return Ok(p);
        }

        if matches!(r.mode, InstallMode::AddonGit) {
            return self.build_git_addon_plan_for_repo_async(r, wow_dir).await;
        }

        let missing_targets = self.has_missing_targets(r.id, wow_dir)?;
        let det = detect_repo(&r.url)?;
        let now = Self::now_unix();

        if det.kind == ForgeKind::GitHub {
            if Self::has_github_token() {
                let _ = self.db.clear_rate_limit(&r.host);
            } else if let Some(reset_epoch) = self.db.get_rate_limit(&r.host)? {
                if now < reset_epoch {
                    return Ok(Self::rate_limited_plan(r, reset_epoch));
                }
                let _ = self.db.clear_rate_limit(&r.host);
            }
        }

        let mut etag = if use_cached_etag {
            r.etag.as_deref()
        } else {
            None
        };
        let mut attempted_uncached = !use_cached_etag;

        let rel = loop {
            let (new_etag, rel_opt, not_modified) =
                match forge::latest_release(&self.client, &det, etag).await {
                    Ok(v) => v,
                    Err(e) => {
                        let msg = e.to_string();
                        if det.kind == ForgeKind::GitHub {
                            if let Some(reset_epoch) = Self::parse_github_reset_epoch(&msg) {
                                let _ = self.db.set_rate_limit(&r.host, reset_epoch);
                                return Ok(Self::rate_limited_plan(r, reset_epoch));
                            }
                        }
                        let mut p = Self::blank_plan(r);
                        p.error = Some(msg);
                        return Ok(p);
                    }
                };

            if let Some(ref et) = new_etag {
                let _ = self.db.update_etag(r.id, Some(et.as_str()));
            }
            if det.kind == ForgeKind::GitHub {
                let _ = self.db.clear_rate_limit(&r.host);
            }

            if not_modified {
                let has_known_install = r.installed_asset_id.is_some() || r.last_version.is_some();
                let needs_uncached_refresh = !attempted_uncached
                    && (!has_known_install
                        || (missing_targets
                            && r.installed_asset_url.as_deref().unwrap_or("").is_empty()));

                if needs_uncached_refresh {
                    etag = None;
                    attempted_uncached = true;
                    continue;
                }

                let can_repair = missing_targets
                    && r.installed_asset_url.is_some()
                    && r.installed_asset_name.is_some()
                    && !r.installed_asset_url.as_deref().unwrap_or("").is_empty();

                let mut p = Self::blank_plan(r);
                p.not_modified = true;
                p.repair_needed = can_repair;
                p.externally_modified = if Self::is_mod_mode(&r.mode) {
                    self.check_files_modified(r.id, wow_dir)
                } else {
                    false
                };
                p.asset_id = r.installed_asset_id.clone().unwrap_or_default();
                p.asset_name = r.installed_asset_name.clone().unwrap_or_default();
                p.asset_size = r.installed_asset_size.and_then(|n| u64::try_from(n).ok());
                p.asset_sha256 = None;
                p.error = None;
                if can_repair {
                    p.asset_url = r.installed_asset_url.clone().unwrap_or_default();
                }
                return Ok(p);
            }

            match rel_opt {
                Some(x) => {
                    if let Some(pub_at) = x.published_at {
                        let _ = self.db.set_published_at(r.id, Some(pub_at));
                    }
                    break x;
                }
                None => {
                    let mut p = Self::blank_plan(r);
                    p.latest = "none".to_string();
                    return Ok(p);
                }
            }
        };

        let mode = r.mode.clone();

        // If repo is pinned to a specific version and the latest release doesn't
        // match the pin, fetch the pinned release from the full release list.
        let (target_rel, latest_tag_for_display) = if let Some(ref pin) = r.pinned_version {
            if rel.tag != *pin {
                // Fetch the pinned release from the full list.
                let pinned = match forge::list_releases(&self.client, &det).await {
                    Ok(all) => all.into_iter().find(|r| r.tag == *pin),
                    Err(_) => None,
                };
                match pinned {
                    Some(pinned_rel) => {
                        // latest_tag_for_display = actual latest so UI shows "update available"
                        (pinned_rel, Some(rel.tag.clone()))
                    }
                    None => {
                        // Pinned version not found — fall through to latest.
                        (rel, None)
                    }
                }
            } else {
                // Pinned version IS the latest — no extra fetch needed.
                (rel, None)
            }
        } else {
            (rel, None)
        };

        // Collect ALL .dll assets for repos that publish individual DLL files (e.g. WeirdUtils).
        // Applies to Dll mode always, and to Auto/Mixed when no zip asset is present (a zip
        // would bundle all the DLLs itself, so we only need the zip in that case).
        let has_zip_asset = target_rel.assets.iter().any(|a| {
            a.name.to_lowercase().ends_with(".zip") && Self::is_asset_allowed(a, &mode)
        });
        let collect_all_dlls = matches!(mode, InstallMode::Dll)
            || (!has_zip_asset && matches!(mode, InstallMode::Auto | InstallMode::Mixed));
        let all_dll_assets: Vec<ReleaseAsset> = if collect_all_dlls {
            target_rel
                .assets
                .iter()
                .filter(|a| {
                    a.name.to_lowercase().ends_with(".dll")
                        && Self::is_asset_allowed(a, &mode)
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        let asset = match Self::pick_asset(&target_rel, mode.clone(), r.asset_regex.as_deref()) {
            Ok(asset) => asset,
            Err(e) => {
                let mut p = Self::blank_plan(r);
                p.error = Some(e.to_string());
                return Ok(p);
            }
        };

        // Extra assets = everything except the primary one (skip duplicates by name).
        let extra_assets: Vec<ReleaseAsset> = all_dll_assets
            .into_iter()
            .filter(|a| !a.name.eq_ignore_ascii_case(&asset.name))
            .collect();

        // For version tracking of multi-asset repos, use the release tag so that the "version"
        // represents the whole release rather than a specific DLL filename.
        let target_tag = if extra_assets.is_empty() {
            Self::effective_latest_label(&target_rel.tag, &asset.name)
        } else {
            // Use tag directly — individual asset names carry no shared version info.
            target_rel.tag.clone()
        };
        let asset_id = Self::effective_asset_id(&asset);
        let asset_size_i64 = Self::size_u64_to_i64(asset.size);

        let installed_matches =
            Self::installed_matches(r, &target_tag, &asset_id, &asset.name, asset_size_i64);
        let needs_download = !installed_matches || missing_targets;
        let repair_needed = missing_targets && installed_matches;

        // Clear the cached ETag when an update or repair is pending so that the
        // next check re-fetches the release instead of getting a 304 (which would
        // incorrectly report "up to date" while the update remains uninstalled).
        if needs_download {
            let _ = self.db.update_etag(r.id, None);
        }

        // DLL count detection — count currently installed DLLs vs new release DLLs.
        let previous_dll_count = self.db.list_installs(r.id)
            .map(|entries| entries.iter().filter(|e| e.kind == "dll").count())
            .unwrap_or(0);
        // New count = primary asset (if DLL) + extra DLL assets.
        let new_dll_count = if asset.name.to_lowercase().ends_with(".dll") {
            1 + extra_assets.len()
        } else {
            extra_assets.len()
        };

        // When pinned, latest_tag_for_display holds the real latest tag so the UI
        // can show "update available" even though we're downloading the pinned version.
        let display_latest = latest_tag_for_display.unwrap_or_else(|| target_tag.clone());

        Ok(UpdatePlan {
            repo_id: r.id,
            forge: r.forge.clone(),
            host: r.host.clone(),
            owner: r.owner.clone(),
            name: r.name.clone(),
            url: r.url.clone(),
            mode,
            current: Self::normalized_current_version(r),
            latest: display_latest,
            asset_id,
            asset_name: asset.name.clone(),
            asset_url: if needs_download {
                asset.download_url.clone()
            } else {
                "".to_string()
            },
            asset_size: asset.size,
            asset_sha256: asset.sha256.clone(),
            repair_needed,
            externally_modified: if Self::is_mod_mode(&r.mode) {
                self.check_files_modified(r.id, wow_dir)
            } else {
                false
            },
            not_modified: false,
            applied: false,
            error: None,
            extra_assets,
            previous_dll_count,
            new_dll_count,
        })
    }

    pub async fn check_updates(&self) -> Result<Vec<UpdatePlan>> {
        self.check_updates_with_wow(None, CheckMode::Force).await
    }

    fn check_updates_parallel<'a>(
        &'a self,
        repos: &'a [Repo],
        wow_dir: Option<&'a Path>,
        check_mode: CheckMode,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<UpdatePlan>>> + 'a>> {
        Box::pin(async move {
            match repos {
                [] => Ok(Vec::new()),
                [repo] => Ok(vec![
                    self.build_update_plan_for_repo(repo, true, wow_dir, check_mode).await?,
                ]),
                _ => {
                    let mid = repos.len() / 2;
                    let (left, right) = repos.split_at(mid);
                    let (lres, rres) = tokio::join!(
                        self.check_updates_parallel(left, wow_dir, check_mode),
                        self.check_updates_parallel(right, wow_dir, check_mode)
                    );
                    let mut plans = lres?;
                    plans.extend(rres?);
                    Ok(plans)
                }
            }
        })
    }

    async fn check_updates_batched(
        &self,
        repos: &[Repo],
        wow_dir: Option<&Path>,
        check_mode: CheckMode,
    ) -> Result<Vec<UpdatePlan>> {
        let mut plans = Vec::with_capacity(repos.len());

        // Keep release API checks bounded to avoid bursty rate-limit pressure.
        for chunk in repos.chunks(4) {
            match chunk {
                [r1] => {
                    plans.push(self.build_update_plan_for_repo(r1, true, wow_dir, check_mode).await?);
                }
                [r1, r2] => {
                    let (p1, p2) = tokio::join!(
                        self.build_update_plan_for_repo(r1, true, wow_dir, check_mode),
                        self.build_update_plan_for_repo(r2, true, wow_dir, check_mode)
                    );
                    plans.push(p1?);
                    plans.push(p2?);
                }
                [r1, r2, r3] => {
                    let (p1, p2, p3) = tokio::join!(
                        self.build_update_plan_for_repo(r1, true, wow_dir, check_mode),
                        self.build_update_plan_for_repo(r2, true, wow_dir, check_mode),
                        self.build_update_plan_for_repo(r3, true, wow_dir, check_mode)
                    );
                    plans.push(p1?);
                    plans.push(p2?);
                    plans.push(p3?);
                }
                [r1, r2, r3, r4] => {
                    let (p1, p2, p3, p4) = tokio::join!(
                        self.build_update_plan_for_repo(r1, true, wow_dir, check_mode),
                        self.build_update_plan_for_repo(r2, true, wow_dir, check_mode),
                        self.build_update_plan_for_repo(r3, true, wow_dir, check_mode),
                        self.build_update_plan_for_repo(r4, true, wow_dir, check_mode)
                    );
                    plans.push(p1?);
                    plans.push(p2?);
                    plans.push(p3?);
                    plans.push(p4?);
                }
                _ => unreachable!("chunk size is bounded to 4"),
            }
        }

        Ok(plans)
    }

    pub async fn check_updates_with_wow(&self, wow_dir: Option<&Path>, check_mode: CheckMode) -> Result<Vec<UpdatePlan>> {
        self.check_updates_with_wow_skip(wow_dir, check_mode, &HashSet::new()).await
    }

    pub async fn check_updates_with_wow_skip(&self, wow_dir: Option<&Path>, check_mode: CheckMode, skip_repo_ids: &HashSet<i64>) -> Result<Vec<UpdatePlan>> {
        if let Some(wow_dir) = wow_dir {
            let _ = self.import_existing_addon_git_repos(wow_dir);
            let _ = self.dedup_addon_repos_by_folder(wow_dir);
        }

        let repos = self.db.list_repos()?;
        let mut git_repos = Vec::new();
        let mut release_repos = Vec::new();
        for repo in repos {
            if !repo.enabled || skip_repo_ids.contains(&repo.id) {
                continue; // skip disabled or explicitly skipped repos
            }
            if matches!(repo.mode, InstallMode::AddonGit) {
                git_repos.push(repo);
            } else {
                release_repos.push(repo);
            }
        }

        let (git_plans, release_plans) = tokio::join!(
            self.check_updates_parallel(&git_repos, wow_dir, check_mode),
            self.check_updates_batched(&release_repos, wow_dir, check_mode)
        );

        let mut plans = Vec::with_capacity(git_repos.len() + release_repos.len());
        plans.extend(git_plans?);
        plans.extend(release_plans?);
        Ok(plans)
    }

    fn pick_asset(
        rel: &LatestRelease,
        mode: InstallMode,
        asset_regex: Option<&str>,
    ) -> Result<ReleaseAsset> {
        let assets = &rel.assets;
        if assets.is_empty() {
            anyhow::bail!("No assets found in latest release {}", rel.tag);
        }

        let is_allowed = |a: &ReleaseAsset| Self::is_asset_allowed(a, &mode);

        if let Some(rx) = asset_regex {
            let re = regex::Regex::new(rx)?;
            if let Some(a) = assets
                .iter()
                .find(|a| re.is_match(&a.name) && is_allowed(a))
            {
                return Ok(a.clone());
            }
        }

        let prefer_zip = matches!(
            mode,
            InstallMode::Addon | InstallMode::Mixed | InstallMode::Auto
        );

        if prefer_zip {
            let has_vanillafixes_assets = assets
                .iter()
                .any(|a| a.name.to_ascii_lowercase().starts_with("vanillafixes"));

            if has_vanillafixes_assets {
                if let Some(a) = assets.iter().find(|a| {
                    let lower = a.name.to_ascii_lowercase();
                    lower.ends_with(".zip") && !lower.contains("-dxvk") && is_allowed(a)
                }) {
                    return Ok(a.clone());
                }
            }

            if let Some(a) = assets
                .iter()
                .find(|a| a.name.to_lowercase().ends_with(".zip") && is_allowed(a))
            {
                return Ok(a.clone());
            }
        }

        if matches!(mode, InstallMode::Dll) {
            if let Some(a) = assets
                .iter()
                .find(|a| a.name.to_lowercase().ends_with(".dll") && is_allowed(a))
            {
                return Ok(a.clone());
            }
        }

        if let Some(a) = assets.iter().find(|a| is_allowed(a)) {
            return Ok(a.clone());
        }

        anyhow::bail!(
            "No safe/compatible release asset found for mode {} in {}.",
            mode.as_str(),
            rel.tag
        )
    }

    fn asset_extension(name: &str) -> Option<String> {
        Path::new(name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.trim().to_ascii_lowercase())
            .filter(|ext| !ext.is_empty())
    }

    fn is_blocked_extension(ext: &str) -> bool {
        matches!(
            ext,
            "exe"
                | "msi"
                | "msix"
                | "appx"
                | "bat"
                | "cmd"
                | "ps1"
                | "vbs"
                | "js"
                | "jse"
                | "wsf"
                | "wsh"
                | "scr"
                | "com"
                | "sh"
                | "run"
                | "apk"
                | "jar"
                | "py"
                | "pl"
                | "rb"
                | "dmg"
                | "pkg"
        )
    }

    fn is_asset_allowed(asset: &ReleaseAsset, mode: &InstallMode) -> bool {
        let name = asset.name.trim();
        if name.is_empty() {
            return false;
        }
        let ext = match Self::asset_extension(name) {
            Some(ext) => ext,
            None => return matches!(mode, InstallMode::Raw),
        };
        if Self::is_blocked_extension(&ext) {
            return false;
        }
        match mode {
            InstallMode::Addon | InstallMode::Mixed => ext == "zip",
            InstallMode::AddonGit => false,
            InstallMode::Dll => ext == "dll" || ext == "zip",
            InstallMode::Auto => ext == "dll" || ext == "zip",
            InstallMode::Raw => true,
        }
    }

    fn host_matches_or_subdomain(host: &str, trusted: &str) -> bool {
        host.eq_ignore_ascii_case(trusted)
            || host
                .to_ascii_lowercase()
                .ends_with(&format!(".{}", trusted.to_ascii_lowercase()))
    }

    fn validate_asset_url(plan: &UpdatePlan) -> Result<()> {
        let parsed = Url::parse(&plan.asset_url)?;
        if parsed.scheme() != "https" {
            anyhow::bail!("Blocked non-HTTPS asset URL: {}", plan.asset_url);
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("Asset URL missing host"))?;

        let mut trusted_hosts = vec![plan.host.as_str()];
        if plan.forge.eq_ignore_ascii_case("github") {
            trusted_hosts.extend([
                "github.com",
                "objects.githubusercontent.com",
                "release-assets.githubusercontent.com",
                "codeload.github.com",
            ]);
        }

        if trusted_hosts
            .iter()
            .any(|h| Self::host_matches_or_subdomain(host, h))
        {
            return Ok(());
        }

        anyhow::bail!(
            "Blocked asset host '{}' (not trusted for {}/{})",
            host,
            plan.owner,
            plan.name
        )
    }

    fn looks_like_zip_bytes(head: &[u8]) -> bool {
        head.starts_with(b"PK\x03\x04")
            || head.starts_with(b"PK\x05\x06")
            || head.starts_with(b"PK\x07\x08")
    }

    fn looks_like_dll_bytes(head: &[u8]) -> bool {
        head.starts_with(b"MZ")
    }

    fn validate_downloaded_asset(path: &Path, plan: &UpdatePlan) -> Result<()> {
        if !path.exists() {
            anyhow::bail!("Downloaded asset not found: {:?}", path);
        }

        let file_len = fs::metadata(path)?.len();
        if let Some(expected) = plan.asset_size {
            if file_len != expected {
                anyhow::bail!(
                    "Downloaded asset size mismatch for {}: expected {}, got {}",
                    plan.asset_name,
                    expected,
                    file_len
                );
            }
        }

        let lower = plan.asset_name.to_ascii_lowercase();
        if !(lower.ends_with(".zip") || lower.ends_with(".dll")) {
            return Ok(());
        }

        let mut f = fs::File::open(path)?;
        let mut head = [0u8; 4];
        let n = f.read(&mut head)?;
        let slice = &head[..n];

        if lower.ends_with(".zip") && !Self::looks_like_zip_bytes(slice) {
            anyhow::bail!(
                "Downloaded ZIP asset failed signature check: {}",
                plan.asset_name
            );
        }
        if lower.ends_with(".dll") && !Self::looks_like_dll_bytes(slice) {
            anyhow::bail!(
                "Downloaded DLL asset failed signature check: {}",
                plan.asset_name
            );
        }
        Ok(())
    }

    fn verify_asset_digest(path: &Path, expected_sha256: Option<&str>) -> Result<()> {
        let expected = match expected_sha256 {
            Some(v) if !v.trim().is_empty() => v.trim().to_ascii_lowercase(),
            _ => return Ok(()),
        };
        let actual = util::sha256_file_hex(path)?;
        if actual != expected {
            anyhow::bail!(
                "SHA-256 mismatch for {:?} (expected {}, got {})",
                path.file_name().unwrap_or_default(),
                expected,
                actual
            );
        }
        Ok(())
    }

    fn sanitize_for_fs(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                out.push(c);
            } else {
                out.push('_');
            }
        }
        if out.is_empty() {
            "unknown".to_string()
        } else {
            out
        }
    }

    fn release_cache_dir(plan: &UpdatePlan, wow_dir: Option<&Path>) -> Result<PathBuf> {
        let dir = util::cache_dir(wow_dir)?
            .join("releases")
            .join(Self::sanitize_for_fs(&plan.forge))
            .join(Self::sanitize_for_fs(&plan.host))
            .join(Self::sanitize_for_fs(&plan.owner))
            .join(Self::sanitize_for_fs(&plan.name))
            .join(Self::sanitize_for_fs(&plan.latest))
            .join(Self::sanitize_for_fs(&plan.asset_id));
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    async fn download_asset_to(&self, plan: &UpdatePlan, dest: &Path) -> Result<()> {
        Self::validate_asset_url(plan)?;
        self.download_url_to(&plan.asset_url, dest).await
    }

    async fn download_url_to(&self, url: &str, dest: &Path) -> Result<()> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        std::fs::write(dest, &bytes)?;
        Ok(())
    }

    fn looks_like_zip(path: &Path, name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.ends_with(".zip") || path.extension().map(|e| e == "zip").unwrap_or(false)
    }

    fn persist_installs(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        records: &[install::InstallRecord],
    ) -> Result<()> {
        self.db.clear_installs(repo_id)?;
        for rec in records {
            let manifest_path = Self::to_manifest_path(&rec.path, wow_dir);
            self.db.add_install(repo_id, &manifest_path, rec.kind)?;
        }
        Ok(())
    }

    /// Like `persist_installs` but merges new records with existing ones instead
    /// of replacing. Existing install entries not present in `records` are kept.
    fn persist_installs_merge(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        records: &[install::InstallRecord],
    ) -> Result<()> {
        for rec in records {
            let manifest_path = Self::to_manifest_path(&rec.path, wow_dir);
            self.db.add_install(repo_id, &manifest_path, rec.kind)?;
        }
        Ok(())
    }

    /// Hash each installed file and store the digest in the DB for integrity checking.
    /// Only hashes regular files (not addon directories). Failures are non-fatal.
    fn hash_and_store_installs(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        records: &[install::InstallRecord],
    ) {
        for rec in records {
            if rec.kind == "addon" {
                continue;
            }
            if !rec.path.is_file() {
                continue;
            }
            let manifest_path = Self::to_manifest_path(&rec.path, wow_dir);
            match util::sha256_file_hex(&rec.path) {
                Ok(digest) => {
                    let _ = self.db.set_install_sha256(repo_id, &manifest_path, Some(&digest));
                }
                Err(_) => {}
            }
        }
    }

    fn cleanup_stale_addon_installs(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        records: &[install::InstallRecord],
    ) -> Result<()> {
        let keep: HashSet<PathBuf> = records
            .iter()
            .filter(|rec| rec.kind == "addon")
            .map(|rec| rec.path.clone())
            .collect();

        for entry in self.db.list_installs(repo_id)? {
            if entry.kind != "addon" {
                continue;
            }
            let Some(full) = Self::resolve_install_path(&entry.path, Some(wow_dir)) else {
                continue;
            };
            if keep.contains(&full) {
                continue;
            }
            let _ = Self::remove_any_target(&full);
        }
        Ok(())
    }

    fn remove_any_target(path: &Path) -> Result<bool> {
        let meta = match fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(_) => return Ok(false),
        };
        let ft = meta.file_type();
        if ft.is_symlink() {
            fs::remove_file(path)?;
            return Ok(true);
        }
        if ft.is_dir() {
            fs::remove_dir_all(path)?;
            return Ok(true);
        }
        fs::remove_file(path)?;
        Ok(true)
    }

    fn path_has_conflicting_content(path: &Path) -> bool {
        let meta = match fs::symlink_metadata(path) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            return fs::read_dir(path)
                .ok()
                .and_then(|mut rd| rd.next())
                .is_some();
        }
        // File or symlink present at target path is always a conflict for addon folder installs.
        true
    }

    fn addon_install_conflicts(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        addon_folder_names: &[String],
    ) -> Result<Vec<AddonInstallConflict>> {
        let tracked_paths: HashSet<PathBuf> = self
            .db
            .list_installs(repo_id)?
            .into_iter()
            .filter(|entry| entry.kind == "addon")
            .filter_map(|entry| Self::resolve_install_path(&entry.path, Some(wow_dir)))
            .collect();

        let mut out = Vec::<AddonInstallConflict>::new();
        for addon_name in addon_folder_names {
            let dst = wow_dir.join("Interface").join("AddOns").join(addon_name);
            let manifest_path = Self::to_manifest_path(&dst, wow_dir);
            let owners = self
                .db
                .find_addon_install_owners(&manifest_path, Some(repo_id))?;

            if tracked_paths.contains(&dst) {
                continue;
            }
            // GAM-style safety:
            // 1) if another tracked repo already owns this addon folder target, conflict
            // 2) if destination exists locally and is non-empty/present, conflict
            if owners.is_empty() && !Self::path_has_conflicting_content(&dst) {
                continue;
            }
            out.push(AddonInstallConflict {
                addon_name: addon_name.clone(),
                target_path: dst,
                owners,
            });
        }
        Ok(out)
    }

    fn format_addon_conflict_message(conflicts: &[AddonInstallConflict]) -> String {
        let details = conflicts
            .iter()
            .map(|conflict| {
                let owner_text = if conflict.owners.is_empty() {
                    "local files already exist".to_string()
                } else {
                    let labels = conflict
                        .owners
                        .iter()
                        .map(|o| format!("{}/{}", o.owner, o.name))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("already tracked by {}", labels)
                };
                format!(
                    "{} ({}) [{}]",
                    conflict.addon_name,
                    conflict.target_path.display(),
                    owner_text
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        format!("ADDON_CONFLICT: Existing addon files were found for: {}. Confirm replacement to delete those folders and continue.", details)
    }

    fn clear_conflicting_addon_tracking(
        &self,
        current_repo_id: i64,
        wow_dir: &Path,
        conflicts: &[AddonInstallConflict],
    ) -> Result<()> {
        let mut by_repo = HashMap::<i64, HashSet<String>>::new();
        for conflict in conflicts {
            for owner in &conflict.owners {
                if owner.repo_id == current_repo_id {
                    continue;
                }
                by_repo
                    .entry(owner.repo_id)
                    .or_default()
                    .insert(owner.manifest_path.clone());
            }
        }

        for (repo_id, manifest_paths) in by_repo {
            for path in manifest_paths {
                self.db.remove_install(repo_id, &path)?;
            }

            let remaining_installs = self.db.list_installs(repo_id)?;
            let has_addon_installs = remaining_installs.iter().any(|entry| entry.kind == "addon");
            if !has_addon_installs {
                // If this was an addon_git repo and no addon installs remain after conflict
                // replacement, remove it from tracking entirely so duplicate forks cannot
                // coexist in the tracked addons list.
                let should_remove_repo = self
                    .db
                    .get_repo(repo_id)
                    .ok()
                    .map(|r| matches!(r.mode, InstallMode::AddonGit))
                    .unwrap_or(false);
                if should_remove_repo {
                    let _ = self.remove_repo(repo_id, Some(wow_dir), true)?;
                } else {
                    self.db
                        .set_installed_asset_state(repo_id, None, None, None, None, None)?;
                }
            }
        }

        Ok(())
    }

    fn resolve_install_path(path: &str, wow_dir: Option<&Path>) -> Option<PathBuf> {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            return Some(p);
        }
        let base = wow_dir?;
        if p.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return None;
        }
        Some(base.join(p))
    }

    fn remove_dlls_txt_entries(wow_dir: &Path, dll_names: &[String]) -> Result<()> {
        if dll_names.is_empty() {
            return Ok(());
        }
        let path = wow_dir.join("dlls.txt");
        if !path.exists() {
            return Ok(());
        }

        let remove_set: HashSet<String> = dll_names.iter().map(|n| n.to_lowercase()).collect();
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let mut kept = Vec::new();

        for line in existing.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                kept.push(line.to_string());
                continue;
            }
            let rest = if let Some(stripped) = trimmed.strip_prefix('#') {
                stripped.trim()
            } else {
                trimmed
            };
            if remove_set.contains(&rest.to_lowercase()) {
                continue;
            }
            kept.push(line.to_string());
        }

        let mut out = kept.join("\n");
        out.push('\n');
        fs::write(path, out)?;
        Ok(())
    }

    fn set_dlls_txt_entries_commented(
        wow_dir: &Path,
        dll_names: &[String],
        commented: bool,
    ) -> Result<usize> {
        if dll_names.is_empty() {
            return Ok(0);
        }
        let path = wow_dir.join("dlls.txt");
        if !path.exists() {
            return Ok(0);
        }

        let wanted: HashSet<String> = dll_names.iter().map(|n| n.to_lowercase()).collect();
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();
        let mut changed = 0usize;
        let mut seen = HashSet::<String>::new();

        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let rest = if let Some(stripped) = trimmed.strip_prefix('#') {
                stripped.trim()
            } else {
                trimmed
            };
            let key = rest.to_lowercase();
            if !wanted.contains(&key) {
                continue;
            }

            let desired = if commented {
                format!("# {}", rest)
            } else {
                rest.to_string()
            };
            if line.trim() != desired {
                *line = desired;
                changed += 1;
            }
            seen.insert(key);
        }

        if !commented {
            for dll in dll_names {
                let key = dll.to_lowercase();
                if !seen.contains(&key) {
                    lines.push(dll.to_string());
                    changed += 1;
                }
            }
        }

        if changed > 0 {
            let mut out = lines.join("\n");
            out.push('\n');
            fs::write(path, out)?;
        }
        Ok(changed)
    }

    pub fn set_repo_enabled(
        &self,
        repo_id: i64,
        enabled: bool,
        wow_dir: Option<&Path>,
    ) -> Result<usize> {
        let mut dll_names = Vec::<String>::new();
        for entry in self.db.list_installs(repo_id)? {
            if entry.kind != "dll" {
                continue;
            }
            if let Some(name) = Path::new(&entry.path).file_name().and_then(|s| s.to_str()) {
                dll_names.push(name.to_string());
            }
        }

        let mut touched = 0usize;
        if let Some(base) = wow_dir {
            touched = Self::set_dlls_txt_entries_commented(base, &dll_names, !enabled)?;
        }

        self.db.set_repo_enabled(repo_id, enabled)?;
        Ok(touched)
    }

    /// Toggle a single DLL's enabled state in dlls.txt without touching the whole repo.
    /// Returns `true` if dlls.txt was modified.
    pub fn set_dll_enabled(
        &self,
        dll_name: &str,
        enabled: bool,
        wow_dir: &Path,
    ) -> Result<bool> {
        let names = vec![dll_name.to_string()];
        let touched = Self::set_dlls_txt_entries_commented(wow_dir, &names, !enabled)?;
        Ok(touched > 0)
    }

    pub fn set_repo_git_branch(&self, repo_id: i64, git_branch: Option<String>) -> Result<()> {
        let repo = self.db.get_repo(repo_id)?;
        if !matches!(repo.mode, InstallMode::AddonGit) {
            anyhow::bail!("Branch selection is only supported for addon_git repos.");
        }
        let normalized = git_branch
            .map(|b| b.trim().to_string())
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| "master".to_string());
        self.db
            .set_repo_git_branch(repo_id, Some(normalized.as_str()))?;
        Ok(())
    }

    pub fn set_repo_merge_installs(&self, repo_id: i64, merge: bool) -> Result<()> {
        self.db.set_merge_installs(repo_id, merge)?;
        Ok(())
    }

    pub fn set_repo_pinned_version(
        &self,
        repo_id: i64,
        version: Option<String>,
    ) -> Result<()> {
        let normalized = version
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        self.db
            .set_pinned_version(repo_id, normalized.as_deref())?;
        Ok(())
    }

    /// Fetch the full list of releases for a repo (newest first).
    pub async fn list_releases(&self, repo_url: &str) -> Result<Vec<LatestRelease>> {
        let det = detect_repo(repo_url)?;
        forge::list_releases(&self.client, &det).await
    }

    pub fn list_repo_branches(&self, repo_id: i64) -> Result<Vec<String>> {
        let repo = self.db.get_repo(repo_id)?;
        if !matches!(repo.mode, InstallMode::AddonGit) {
            return Ok(Vec::new());
        }
        let mut branches = git_sync::remote_branches(&repo.url)?;
        if let Some(selected) = repo.git_branch {
            if !branches.iter().any(|b| b.eq_ignore_ascii_case(&selected)) {
                branches.insert(0, selected);
            }
        }
        Ok(branches)
    }

    pub fn remove_repo(
        &self,
        repo_id: i64,
        wow_dir: Option<&Path>,
        remove_local_files: bool,
    ) -> Result<usize> {
        let mut removed_paths = 0usize;
        let mut removed_dlls = Vec::<String>::new();

        if remove_local_files {
            for entry in self.db.list_installs(repo_id)? {
                if let Some(full) = Self::resolve_install_path(&entry.path, wow_dir) {
                    if Self::remove_any_target(&full)? {
                        removed_paths += 1;
                    }
                }
                if entry.kind == "dll" {
                    if let Some(name) = Path::new(&entry.path).file_name().and_then(|s| s.to_str())
                    {
                        removed_dlls.push(name.to_string());
                    }
                }
            }
            if let Some(base) = wow_dir {
                let _ = Self::remove_dlls_txt_entries(base, &removed_dlls);
            }
        }

        self.db.remove_repo(repo_id)?;
        Ok(removed_paths)
    }

    pub async fn apply_updates(
        &self,
        wow_dir: &Path,
        raw_dest: Option<&Path>,
        opts: InstallOptions,
    ) -> Result<Vec<UpdatePlan>> {
        let repos = self.db.list_repos()?;
        let mut plans = Vec::new();

        for r in repos {
            let mut plan = self
                .build_update_plan_for_repo(&r, true, Some(wow_dir), CheckMode::Force)
                .await?;
            if r.enabled && !plan.asset_url.is_empty() && !plan.externally_modified {
                match self.apply_one(&plan, wow_dir, raw_dest, opts).await {
                    Ok(()) => {
                        plan.applied = true;
                    }
                    Err(e) => {
                        plan.error = Some(format!("Install failed: {}", e));
                    }
                }
            }
            plans.push(plan);
        }

        Ok(plans)
    }

    pub async fn update_repo(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        raw_dest: Option<&Path>,
        opts: InstallOptions,
    ) -> Result<Option<UpdatePlan>> {
        let repo = self.db.get_repo(repo_id)?;
        let mut plan = self
            .build_update_plan_for_repo(&repo, true, Some(wow_dir), CheckMode::Force)
            .await?;

        if let Some(err) = plan.error.clone() {
            anyhow::bail!(err);
        }

        if plan.asset_url.is_empty() {
            return Ok(None);
        }

        self.apply_one(&plan, wow_dir, raw_dest, opts).await?;
        plan.applied = true;
        Ok(Some(plan))
    }

    /// One-time migration: if a repo was previously cloned into the legacy
    /// `.wuddle/addon_git/…` staging area, move it to the new direct location
    /// (`Interface/AddOns/{name}`) so it becomes cross-compatible with GAM and
    /// the TurtleWoW launcher.  Updates the DB install entries in-place.
    /// Safe to call repeatedly — does nothing when the legacy path doesn't exist
    /// or the target already exists.
    fn migrate_staging_clone_if_needed(&self, wow_dir: &Path, repo: &Repo) -> Result<()> {
        let legacy = git_sync::addon_repo_legacy_staging_dir(
            wow_dir, &repo.host, &repo.owner, &repo.name,
        );
        if !legacy.is_dir() || !Self::has_local_git_marker(&legacy) {
            return Ok(());
        }
        let direct = git_sync::addon_direct_dir(wow_dir, &repo.name);
        if direct.is_dir() {
            // Target already exists — nothing to move.
            return Ok(());
        }
        if let Some(parent) = direct.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&legacy, &direct).with_context(|| {
            format!("migrate addon clone {:?} -> {:?}", legacy, direct)
        })?;

        // Update any DB install entries that pointed at the old path.
        let legacy_manifest = Self::to_manifest_path(&legacy, wow_dir);
        let direct_manifest = Self::to_manifest_path(&direct, wow_dir);
        if let Ok(entries) = self.db.list_installs(repo.id) {
            for entry in entries {
                if entry.path == legacy_manifest {
                    let _ = self.db.update_install_path(repo.id, &entry.path, &direct_manifest);
                } else if entry.path.starts_with(&legacy_manifest) {
                    let new_path = direct_manifest.clone() + &entry.path[legacy_manifest.len()..];
                    let _ = self.db.update_install_path(repo.id, &entry.path, &new_path);
                }
            }
        }
        Ok(())
    }

    async fn apply_one(
        &self,
        plan: &UpdatePlan,
        wow_dir: &Path,
        raw_dest: Option<&Path>,
        opts: InstallOptions,
    ) -> Result<()> {
        if matches!(plan.mode, InstallMode::AddonGit) {
            let repo = self.db.get_repo(plan.repo_id)?;

            // Migrate legacy staging clones to the direct AddOns location on first encounter.
            self.migrate_staging_clone_if_needed(wow_dir, &repo)?;

            let worktree_dir = self.addon_git_worktree_dir(plan.repo_id, wow_dir, &repo);
            let preferred_branch = repo
                .git_branch
                .as_deref()
                .map(str::trim)
                .filter(|b| !b.is_empty())
                .unwrap_or("master");
            let synced = git_sync::sync_repo(&plan.url, &worktree_dir, Some(preferred_branch))
                .with_context(|| format!("git sync {}", plan.url))?;

            // Detect addon folders inside the cloned repo.
            // detect_addons_in_tree returns (src_path, toc_name) pairs.
            let mut detected = install::detect_addons_in_tree(&worktree_dir);
            detected.sort_by_key(|(src, name)| (src.components().count(), name.clone()));

            let mut chosen = Vec::<(PathBuf, String)>::new();
            let mut seen_names = HashSet::<String>::new();
            for (src, addon_name) in detected {
                let key = addon_name.to_lowercase();
                if seen_names.insert(key) {
                    chosen.push((src, addon_name));
                }
            }

            if chosen.is_empty() {
                anyhow::bail!(
                    "No addon .toc files found in synced repo. Expected at least one addon folder."
                );
            }

            // GAM post-clone rename: if the repo directory name differs from the
            // detected .toc name (single-addon case), rename the directory to match
            // the .toc name — exactly as GAM's Control::clone() does after cloning.
            // This ensures cross-compatibility when the repo slug ≠ addon name.
            let worktree_dir = if chosen.len() == 1 {
                let (ref src, ref toc_name) = chosen[0];
                if src == &worktree_dir {
                    // Single-addon repo: src is the repo root. Check if names match.
                    let current_name = worktree_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    if !current_name.eq_ignore_ascii_case(toc_name) {
                        let new_dir = worktree_dir.with_file_name(toc_name);
                        if !new_dir.exists() {
                            fs::rename(&worktree_dir, &new_dir).with_context(|| {
                                format!("rename addon dir {:?} -> {:?}", worktree_dir, new_dir)
                            })?;
                            // Update DB install paths that pointed at the old location.
                            let old_manifest = Self::to_manifest_path(&worktree_dir, wow_dir);
                            let new_manifest = Self::to_manifest_path(&new_dir, wow_dir);
                            if let Ok(entries) = self.db.list_installs(repo.id) {
                                for entry in entries {
                                    if entry.path == old_manifest || entry.path.starts_with(&(old_manifest.clone() + "/")) {
                                        let updated = new_manifest.clone() + &entry.path[old_manifest.len()..];
                                        let _ = self.db.update_install_path(repo.id, &entry.path, &updated);
                                    }
                                }
                            }
                            // Update chosen to reflect the new path.
                            chosen[0].0 = new_dir.clone();
                            new_dir
                        } else {
                            worktree_dir
                        }
                    } else {
                        worktree_dir
                    }
                } else {
                    worktree_dir
                }
            } else {
                worktree_dir
            };

            // Remove previously created sub-addon symlinks/copies for this repo
            // (but never the worktree dir itself or anything inside it).
            for entry in self.db.list_installs(plan.repo_id)? {
                if entry.kind != "addon" {
                    continue;
                }
                if let Some(full) = Self::resolve_install_path(&entry.path, Some(wow_dir)) {
                    if full == worktree_dir || full.starts_with(&worktree_dir) {
                        continue;
                    }
                    let _ = Self::remove_any_target(&full);
                }
            }

            // GAM subfolder collision: if a subfolder has the same name as the repo
            // directory, rename the repo dir to "{name}.repo" first — exactly as
            // GAM's Addon::unpackSubfolders() does — so the symlink can be created.
            let repo_dir_name = worktree_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let has_collision = chosen.iter().any(|(src, name)| {
                src != &worktree_dir && name.eq_ignore_ascii_case(&repo_dir_name)
            });
            let worktree_dir = if has_collision {
                let repo_dir = worktree_dir.with_file_name(format!("{}.repo", repo_dir_name));
                if !repo_dir.exists() {
                    fs::rename(&worktree_dir, &repo_dir).with_context(|| {
                        format!("rename repo dir to .repo suffix {:?} -> {:?}", worktree_dir, repo_dir)
                    })?;
                    // Update src paths in chosen that pointed at the old worktree dir.
                    for (src, _) in &mut chosen {
                        if src.starts_with(&worktree_dir) {
                            let rel = src.strip_prefix(&worktree_dir).unwrap_or(src.as_path());
                            *src = repo_dir.join(rel);
                        }
                    }
                }
                repo_dir
            } else {
                worktree_dir
            };

            // Conflict check for sub-addon symlink targets only (not the repo dir itself).
            let sub_addon_names: Vec<String> = chosen
                .iter()
                .filter(|(src, _)| *src != worktree_dir)
                .map(|(_, name)| name.clone())
                .collect();
            if !sub_addon_names.is_empty() {
                let conflicts = self.addon_install_conflicts(plan.repo_id, wow_dir, &sub_addon_names)?;
                if !conflicts.is_empty() {
                    if !opts.replace_addon_conflicts {
                        anyhow::bail!(Self::format_addon_conflict_message(&conflicts));
                    }
                    for conflict in &conflicts {
                        let _ = Self::remove_any_target(&conflict.target_path)?;
                    }
                    self.clear_conflicting_addon_tracking(plan.repo_id, wow_dir, &conflicts)?;
                }
            }

            let mut records = Vec::<install::InstallRecord>::new();

            for (src_dir, addon_folder_name) in chosen {
                let dst_dir = wow_dir
                    .join("Interface")
                    .join("AddOns")
                    .join(&addon_folder_name);

                if src_dir == dst_dir {
                    // Single-addon repo: the clone root is the addon folder — no symlink needed.
                    records.push(install::InstallRecord {
                        path: dst_dir,
                        kind: "addon",
                    });
                } else {
                    // Multi-addon repo subfolder: create a symlink from AddOns/{name}
                    // into the repo directory — matches GAM's unpackSubfolders().
                    let rec = install::link_addon_subfolder(&src_dir, &dst_dir)?;
                    records.push(rec);
                }
            }

            // No kind='raw' worktree entry — GAM doesn't track anything beyond the
            // addon folders themselves. The .git dir inside the addon folder is the
            // ground truth; import_existing_addon_git_repos() will re-discover it.
            self.persist_installs(plan.repo_id, wow_dir, &records)?;
            self.db.set_installed_asset_state(
                plan.repo_id,
                Some(&synced.short_oid),
                Some(&synced.oid),
                Some(&format!("git:{}", synced.branch)),
                None,
                Some(&plan.url),
            )?;
            return Ok(());
        }

        if plan.asset_url.is_empty() {
            anyhow::bail!("No downloadable asset in update plan");
        }

        let release_dir = Self::release_cache_dir(plan, Some(wow_dir))?;
        let asset_name_fs = Path::new(&plan.asset_name)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("asset.bin")
            .to_string();
        let asset_path = release_dir.join(asset_name_fs);

        Self::validate_asset_url(plan)?;

        let mut should_download = match (asset_path.metadata().ok(), plan.asset_size) {
            (Some(meta), Some(expected)) => meta.len() != expected,
            (Some(_), None) => false,
            (None, _) => true,
        };
        if !should_download && plan.asset_sha256.is_some() {
            should_download =
                Self::verify_asset_digest(&asset_path, plan.asset_sha256.as_deref()).is_err();
        }
        if should_download {
            self.download_asset_to(plan, &asset_path).await?;
        }
        Self::validate_downloaded_asset(&asset_path, plan)?;
        Self::verify_asset_digest(&asset_path, plan.asset_sha256.as_deref())?;

        let comment = format!(
            "{}/{} {} - managed by Wuddle",
            plan.owner, plan.name, plan.latest
        );

        let mut records = if Self::looks_like_zip(&asset_path, &plan.asset_name) {
            let extract_dir = release_dir.join("unzip");
            install::install_from_zip(
                &asset_path,
                &extract_dir,
                wow_dir,
                plan.mode.as_str(),
                opts,
                &comment,
            )?
        } else {
            let lower = plan.asset_name.to_lowercase();
            if lower.ends_with(".dll") {
                vec![install::install_dll(
                    &asset_path,
                    wow_dir,
                    &plan.asset_name,
                    opts,
                    &comment,
                )?]
            } else if matches!(plan.mode, InstallMode::Raw | InstallMode::Auto) {
                let dest = raw_dest.ok_or_else(|| {
                    anyhow::anyhow!("raw_dest is required for raw/auto non-zip assets")
                })?;
                vec![install::install_raw_file(
                    &asset_path,
                    dest,
                    &plan.asset_name,
                    opts,
                    &comment,
                )?]
            } else {
                anyhow::bail!("Asset is not zip/dll; use raw mode (or auto with raw_dest).")
            }
        };

        // Download and install any additional .dll assets (multi-DLL repos like WeirdUtils).
        for extra in &plan.extra_assets {
            let extra_name_fs = Path::new(&extra.name)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("extra.dll")
                .to_string();
            let extra_path = release_dir.join(&extra_name_fs);
            let needs_dl = match (extra_path.metadata().ok(), extra.size) {
                (Some(meta), Some(expected)) => meta.len() != expected,
                (Some(_), None) => false,
                (None, _) => true,
            };
            if needs_dl {
                self.download_url_to(&extra.download_url, &extra_path).await?;
            }
            if extra_path.exists() {
                records.push(install::install_dll(
                    &extra_path,
                    wow_dir,
                    &extra.name,
                    opts,
                    &comment,
                )?);
            }
        }

        // For multi-DLL repos, do a consolidated update_dlls_txt call with ALL dll names so that
        // block markers (# == RepoName == / # == /RepoName ==) get written around the group.
        if !plan.extra_assets.is_empty() {
            let all_dll_names: Vec<String> = records
                .iter()
                .filter(|r| r.kind == "dll")
                .filter_map(|r| {
                    std::path::Path::new(&r.path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                })
                .collect();
            if !all_dll_names.is_empty() {
                install::update_dlls_txt(wow_dir, &plan.name, &all_dll_names)?;
            }
        }

        // Check if this repo uses merge-mode (keep existing files on update).
        let merge_mode = self.db.get_repo(plan.repo_id)
            .map(|r| r.merge_installs)
            .unwrap_or(false);

        if merge_mode {
            // Merge: keep existing install entries, only add/overwrite new ones.
            self.persist_installs_merge(plan.repo_id, wow_dir, &records)?;
        } else {
            // Clean: remove previously tracked addon targets that are no longer
            // part of this release (e.g. suffix variants collapsing).
            self.cleanup_stale_addon_installs(plan.repo_id, wow_dir, &records)?;
            self.persist_installs(plan.repo_id, wow_dir, &records)?;
        }
        self.hash_and_store_installs(plan.repo_id, wow_dir, &records);
        self.db.set_installed_asset_state(
            plan.repo_id,
            Some(&plan.latest),
            Some(&plan.asset_id),
            Some(&plan.asset_name),
            Self::size_u64_to_i64(plan.asset_size),
            Some(&plan.asset_url),
        )?;

        self.prune_release_cache(plan, opts.cache_keep_versions, Some(wow_dir));

        Ok(())
    }

    /// Remove old cached release versions for a repo, keeping the `keep_versions`
    /// most recent plus the currently-installed version. Non-fatal on any error.
    fn prune_release_cache(&self, plan: &UpdatePlan, keep_versions: usize, wow_dir: Option<&Path>) {
        let repo = match self.db.get_repo(plan.repo_id) {
            Ok(r) => r,
            Err(_) => return,
        };

        let repo_cache = match util::cache_dir(wow_dir) {
            Ok(c) => c
                .join("releases")
                .join(Self::sanitize_for_fs(&plan.forge))
                .join(Self::sanitize_for_fs(&plan.host))
                .join(Self::sanitize_for_fs(&plan.owner))
                .join(Self::sanitize_for_fs(&plan.name)),
            Err(_) => return,
        };

        if !repo_cache.is_dir() {
            return;
        }

        let current_version = repo
            .last_version
            .as_deref()
            .map(|v| Self::sanitize_for_fs(v));

        // Collect version subdirectories with modification time for sorting.
        let mut versions: Vec<(String, std::time::SystemTime)> = Vec::new();
        let entries = match fs::read_dir(&repo_cache) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let mtime = path
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                versions.push((name.to_string(), mtime));
            }
        }

        // Sort newest first by modification time.
        versions.sort_by(|a, b| b.1.cmp(&a.1));

        let mut kept = 0usize;
        for (name, _) in &versions {
            let is_current = current_version.as_deref() == Some(name.as_str());
            if is_current || kept < keep_versions {
                if !is_current {
                    kept += 1;
                }
                continue;
            }
            let dir = repo_cache.join(name);
            let _ = fs::remove_dir_all(&dir);
        }
    }

    /// Force reinstall a repo even if already "up to date".
    pub async fn reinstall_repo(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        raw_dest: Option<&Path>,
        opts: InstallOptions,
    ) -> Result<UpdatePlan> {
        let r = self.db.get_repo(repo_id)?;

        if matches!(r.mode, InstallMode::AddonGit) {
            let mut plan = self.build_git_addon_plan_for_repo(&r, Some(wow_dir))?;
            if let Some(err) = plan.error.clone() {
                anyhow::bail!(err);
            }
            // Force sync even if already up to date.
            plan.asset_url = r.url.clone();
            self.apply_one(&plan, wow_dir, raw_dest, opts).await?;
            plan.applied = true;
            return Ok(plan);
        }

        let det = detect_repo(&r.url)?;

        // force fetch (no ETag) so we always get asset URLs
        let (etag, rel_opt, _not_modified) =
            forge::latest_release(&self.client, &det, None).await?;

        if let Some(ref et) = etag {
            let _ = self.db.update_etag(r.id, Some(et.as_str()));
        }

        let rel = rel_opt.ok_or_else(|| anyhow::anyhow!("No releases found for {}", r.url))?;
        let mode = r.mode.clone();
        let asset = Self::pick_asset(&rel, mode.clone(), r.asset_regex.as_deref())?;
        let latest = Self::effective_latest_label(&rel.tag, &asset.name);

        let mut plan = UpdatePlan {
            repo_id: r.id,
            forge: r.forge.clone(),
            host: r.host.clone(),
            owner: r.owner.clone(),
            name: r.name.clone(),
            url: r.url.clone(),
            mode,
            current: Self::normalized_current_version(&r),
            latest,
            asset_id: Self::effective_asset_id(&asset),
            asset_name: asset.name.clone(),
            asset_url: asset.download_url.clone(),
            asset_size: asset.size,
            asset_sha256: asset.sha256.clone(),
            repair_needed: false,
            externally_modified: false,
            not_modified: false,
            applied: false,
            error: None,
            extra_assets: Vec::new(),
            previous_dll_count: 0,
            new_dll_count: 0,
        };

        self.apply_one(&plan, wow_dir, raw_dest, opts).await?;
        plan.applied = true;
        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;

    // ── version_from_asset_name ──────────────────────────────────────────────

    #[test]
    fn version_plain_semver() {
        assert_eq!(
            Engine::version_from_asset_name("SuperWoW 1.5.1.zip"),
            Some("1.5.1".into())
        );
    }

    #[test]
    fn version_hyphen_prefix() {
        assert_eq!(
            Engine::version_from_asset_name("nampower-0.9.7.zip"),
            Some("0.9.7".into())
        );
    }

    #[test]
    fn version_hyphen_prefix_with_v() {
        assert_eq!(
            Engine::version_from_asset_name("VanillaFixes-v2.1.4.zip"),
            Some("v2.1.4".into())
        );
    }

    #[test]
    fn version_underscores_converted_to_dots() {
        // Underscores between digit groups are normalised to dots when the
        // version is preceded by a word boundary (e.g. a dash or space).
        assert_eq!(
            Engine::version_from_asset_name("UnitXP_SP3-1_0_3.zip"),
            Some("1.0.3".into())
        );
    }

    #[test]
    fn version_four_part() {
        assert_eq!(
            Engine::version_from_asset_name("mod-1.2.3.4.zip"),
            Some("1.2.3.4".into())
        );
    }

    #[test]
    fn version_with_build_tag() {
        // The optional [-+tag] suffix is captured but dots in it are excluded
        // from the character class, so file extensions aren't consumed.
        assert_eq!(
            Engine::version_from_asset_name("dxvk-gplasync-2.1-1.tar.gz"),
            Some("2.1-1".into())
        );
    }

    #[test]
    fn version_no_version_in_name() {
        assert_eq!(Engine::version_from_asset_name("README.md"), None);
        assert_eq!(Engine::version_from_asset_name("install.sh"), None);
    }

    #[test]
    fn version_empty_string() {
        assert_eq!(Engine::version_from_asset_name(""), None);
    }

    // ── parse_github_reset_epoch ─────────────────────────────────────────────

    #[test]
    fn reset_epoch_extracted() {
        assert_eq!(
            Engine::parse_github_reset_epoch("rate limit: reset 1234567890"),
            Some(1234567890)
        );
    }

    #[test]
    fn reset_epoch_large_value() {
        assert_eq!(
            Engine::parse_github_reset_epoch("reset 9876543210"),
            Some(9876543210)
        );
    }

    #[test]
    fn reset_epoch_no_match() {
        assert_eq!(Engine::parse_github_reset_epoch("no epoch here"), None);
        assert_eq!(Engine::parse_github_reset_epoch(""), None);
    }
}
