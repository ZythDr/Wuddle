use anyhow::{Context, Result};
use git2::Repository;
use reqwest::Client;
use std::{
    collections::HashSet,
    future::Future,
    fs,
    io::Read,
    pin::Pin,
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
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
pub use model::{InstallMode, Repo};

use crate::forge::detect_repo;
use crate::forge::ForgeKind;
use crate::forge::git_sync;
use crate::model::{LatestRelease, ReleaseAsset};

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
    pub not_modified: bool,
    pub applied: bool,
    pub error: Option<String>,
}

pub struct Engine {
    db: Db,
    client: Client,
}

static GITHUB_TOKEN: OnceLock<Mutex<Option<String>>> = OnceLock::new();

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
        };

        self.db.add_repo(&repo)
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
            not_modified: false,
            applied: false,
            error: None,
        }
    }

    fn now_unix() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    fn parse_github_reset_epoch(msg: &str) -> Option<i64> {
        let re = regex::Regex::new(r"reset (\d+)").ok()?;
        let caps = re.captures(msg)?;
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
        let re = regex::Regex::new(r"(?i)\bv?\d+(?:[._]\d+){1,3}(?:[-+][0-9A-Za-z.-]+)?\b").ok()?;
        let m = re.find(asset_name)?;
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

    fn addon_git_worktree_dir(&self, repo_id: i64, wow_dir: &Path, repo: &Repo) -> PathBuf {
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
        git_sync::addon_repo_staging_dir(wow_dir, &repo.host, &repo.owner, &repo.name)
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
            };
            let repo_id = self.db.add_repo(&tracked)?;

            let raw_manifest = Self::to_manifest_path(&root, wow_dir);
            self.db.add_install(repo_id, &raw_manifest, "raw")?;

            let mut addon_names = HashSet::<String>::new();
            for (_src_dir, addon_name) in detected_addons {
                if !addon_names.insert(addon_name.to_ascii_lowercase()) {
                    continue;
                }
                let install_path = wow_dir.join("Interface").join("AddOns").join(&addon_name);
                let manifest = Self::to_manifest_path(&install_path, wow_dir);
                self.db.add_install(repo_id, &manifest, "addon")?;
            }

            known.insert(key);
            imported += 1;
        }

        Ok(imported)
    }

    fn build_git_addon_plan_for_repo(&self, r: &Repo, wow_dir: Option<&Path>) -> Result<UpdatePlan> {
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
        let installed_matches = local
            .as_ref()
            .map(|h| h.oid == remote.oid)
            .unwrap_or(false);
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
            not_modified: false,
            applied: false,
            error: None,
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
        let installed_matches = local
            .as_ref()
            .map(|h| h.oid == remote.oid)
            .unwrap_or(false);
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
            not_modified: false,
            applied: false,
            error: None,
        })
    }

    async fn build_update_plan_for_repo(
        &self,
        r: &Repo,
        use_cached_etag: bool,
        wow_dir: Option<&Path>,
    ) -> Result<UpdatePlan> {
        if !r.enabled {
            return Ok(Self::blank_plan(r));
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
                Some(x) => break x,
                None => {
                    let mut p = Self::blank_plan(r);
                    p.latest = "none".to_string();
                    return Ok(p);
                }
            }
        };

        let mode = r.mode.clone();
        let asset = match Self::pick_asset(&rel, mode.clone(), r.asset_regex.as_deref()) {
            Ok(asset) => asset,
            Err(e) => {
                let mut p = Self::blank_plan(r);
                p.error = Some(e.to_string());
                return Ok(p);
            }
        };
        let latest_tag = Self::effective_latest_label(&rel.tag, &asset.name);
        let asset_id = Self::effective_asset_id(&asset);
        let asset_size_i64 = Self::size_u64_to_i64(asset.size);

        let installed_matches =
            Self::installed_matches(r, &latest_tag, &asset_id, &asset.name, asset_size_i64);
        let needs_download = !installed_matches || missing_targets;
        let repair_needed = missing_targets && installed_matches;

        Ok(UpdatePlan {
            repo_id: r.id,
            forge: r.forge.clone(),
            host: r.host.clone(),
            owner: r.owner.clone(),
            name: r.name.clone(),
            url: r.url.clone(),
            mode,
            current: Self::normalized_current_version(r),
            latest: latest_tag,
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
            not_modified: false,
            applied: false,
            error: None,
        })
    }

    pub async fn check_updates(&self) -> Result<Vec<UpdatePlan>> {
        self.check_updates_with_wow(None).await
    }

    fn check_updates_parallel<'a>(
        &'a self,
        repos: &'a [Repo],
        wow_dir: Option<&'a Path>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<UpdatePlan>>> + 'a>> {
        Box::pin(async move {
            match repos {
                [] => Ok(Vec::new()),
                [repo] => Ok(vec![self.build_update_plan_for_repo(repo, true, wow_dir).await?]),
                _ => {
                    let mid = repos.len() / 2;
                    let (left, right) = repos.split_at(mid);
                    let (lres, rres) = tokio::join!(
                        self.check_updates_parallel(left, wow_dir),
                        self.check_updates_parallel(right, wow_dir)
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
    ) -> Result<Vec<UpdatePlan>> {
        let mut plans = Vec::with_capacity(repos.len());

        // Keep release API checks bounded to avoid bursty rate-limit pressure.
        for chunk in repos.chunks(4) {
            match chunk {
                [r1] => {
                    plans.push(self.build_update_plan_for_repo(r1, true, wow_dir).await?);
                }
                [r1, r2] => {
                    let (p1, p2) = tokio::join!(
                        self.build_update_plan_for_repo(r1, true, wow_dir),
                        self.build_update_plan_for_repo(r2, true, wow_dir)
                    );
                    plans.push(p1?);
                    plans.push(p2?);
                }
                [r1, r2, r3] => {
                    let (p1, p2, p3) = tokio::join!(
                        self.build_update_plan_for_repo(r1, true, wow_dir),
                        self.build_update_plan_for_repo(r2, true, wow_dir),
                        self.build_update_plan_for_repo(r3, true, wow_dir)
                    );
                    plans.push(p1?);
                    plans.push(p2?);
                    plans.push(p3?);
                }
                [r1, r2, r3, r4] => {
                    let (p1, p2, p3, p4) = tokio::join!(
                        self.build_update_plan_for_repo(r1, true, wow_dir),
                        self.build_update_plan_for_repo(r2, true, wow_dir),
                        self.build_update_plan_for_repo(r3, true, wow_dir),
                        self.build_update_plan_for_repo(r4, true, wow_dir)
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

    pub async fn check_updates_with_wow(&self, wow_dir: Option<&Path>) -> Result<Vec<UpdatePlan>> {
        if let Some(wow_dir) = wow_dir {
            let _ = self.import_existing_addon_git_repos(wow_dir);
        }

        let repos = self.db.list_repos()?;
        let mut git_repos = Vec::new();
        let mut release_repos = Vec::new();
        for repo in repos {
            if matches!(repo.mode, InstallMode::AddonGit) {
                git_repos.push(repo);
            } else {
                release_repos.push(repo);
            }
        }

        let (git_plans, release_plans) = tokio::join!(
            self.check_updates_parallel(&git_repos, wow_dir),
            self.check_updates_batched(&release_repos, wow_dir)
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

    fn release_cache_dir(plan: &UpdatePlan) -> Result<PathBuf> {
        let dir = util::cache_dir()?
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
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let bytes = self
            .client
            .get(&plan.asset_url)
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
    ) -> Result<Vec<(String, PathBuf)>> {
        let tracked_paths: HashSet<PathBuf> = self
            .db
            .list_installs(repo_id)?
            .into_iter()
            .filter(|entry| entry.kind == "addon")
            .filter_map(|entry| Self::resolve_install_path(&entry.path, Some(wow_dir)))
            .collect();

        let mut out = Vec::<(String, PathBuf)>::new();
        for addon_name in addon_folder_names {
            let dst = wow_dir
                .join("Interface")
                .join("AddOns")
                .join(addon_name);

            if tracked_paths.contains(&dst) {
                continue;
            }
            if !Self::path_has_conflicting_content(&dst) {
                continue;
            }
            out.push((addon_name.clone(), dst));
        }
        Ok(out)
    }

    fn format_addon_conflict_message(conflicts: &[(String, PathBuf)]) -> String {
        let details = conflicts
            .iter()
            .map(|(name, path)| format!("{} ({})", name, path.display()))
            .collect::<Vec<_>>()
            .join("; ");
        format!("ADDON_CONFLICT: Existing addon files were found for: {}. Confirm replacement to delete those folders and continue.", details)
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

    pub fn set_repo_git_branch(&self, repo_id: i64, git_branch: Option<String>) -> Result<()> {
        let repo = self.db.get_repo(repo_id)?;
        if !matches!(repo.mode, InstallMode::AddonGit) {
            anyhow::bail!("Branch selection is only supported for addon_git repos.");
        }
        let normalized = git_branch
            .map(|b| b.trim().to_string())
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| "master".to_string());
        self.db.set_repo_git_branch(repo_id, Some(normalized.as_str()))?;
        Ok(())
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
                .build_update_plan_for_repo(&r, true, Some(wow_dir))
                .await?;
            if r.enabled && !plan.asset_url.is_empty() {
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
            .build_update_plan_for_repo(&repo, true, Some(wow_dir))
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

    async fn apply_one(
        &self,
        plan: &UpdatePlan,
        wow_dir: &Path,
        raw_dest: Option<&Path>,
        opts: InstallOptions,
    ) -> Result<()> {
        if matches!(plan.mode, InstallMode::AddonGit) {
            let repo = self.db.get_repo(plan.repo_id)?;
            let worktree_dir = self.addon_git_worktree_dir(plan.repo_id, wow_dir, &repo);
            let preferred_branch = repo
                .git_branch
                .as_deref()
                .map(str::trim)
                .filter(|b| !b.is_empty())
                .unwrap_or("master");
            let synced = git_sync::sync_repo(&plan.url, &worktree_dir, Some(preferred_branch))
                .with_context(|| format!("git sync {}", plan.url))?;

            // Credit: deployment model inspired by GitAddonsManager's subfolder/.toc scan flow.
            // Keep repo metadata/worktree in hidden staging area, then deploy only real addon roots
            // (folders with .toc) directly into Interface/AddOns.
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

            let addon_names: Vec<String> = chosen.iter().map(|(_, name)| name.clone()).collect();
            let conflicts = self.addon_install_conflicts(plan.repo_id, wow_dir, &addon_names)?;
            if !conflicts.is_empty() {
                if !opts.replace_addon_conflicts {
                    anyhow::bail!(Self::format_addon_conflict_message(&conflicts));
                }
                for (_, path) in &conflicts {
                    let _ = Self::remove_any_target(path)?;
                }
            }

            // Remove previously deployed addon directories for this repo before redeploy.
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

            let comment = format!(
                "{}/{} {} - managed by Wuddle",
                plan.owner, plan.name, synced.short_oid
            );
            let mut records = Vec::<install::InstallRecord>::new();
            records.push(install::InstallRecord {
                path: worktree_dir.clone(),
                kind: "raw",
            });
            for (src_dir, addon_folder_name) in chosen {
                let dst_dir = wow_dir
                    .join("Interface")
                    .join("AddOns")
                    .join(&addon_folder_name);
                if src_dir == dst_dir {
                    records.push(install::InstallRecord {
                        path: dst_dir,
                        kind: "addon",
                    });
                    continue;
                }
                let rec = install::install_addon_folder(
                    &src_dir,
                    wow_dir,
                    &addon_folder_name,
                    opts,
                    &comment,
                )?;
                records.push(rec);
            }

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

        let release_dir = Self::release_cache_dir(plan)?;
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

        let records = if Self::looks_like_zip(&asset_path, &plan.asset_name) {
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

        // Remove previously tracked addon targets that are no longer part of this release install
        // (e.g. suffix variants like "-tbc"/"-wotlk" collapsing into one canonical addon folder).
        self.cleanup_stale_addon_installs(plan.repo_id, wow_dir, &records)?;
        self.persist_installs(plan.repo_id, wow_dir, &records)?;
        self.db.set_installed_asset_state(
            plan.repo_id,
            Some(&plan.latest),
            Some(&plan.asset_id),
            Some(&plan.asset_name),
            Self::size_u64_to_i64(plan.asset_size),
            Some(&plan.asset_url),
        )?;
        Ok(())
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
            not_modified: false,
            applied: false,
            error: None,
        };

        self.apply_one(&plan, wow_dir, raw_dest, opts).await?;
        plan.applied = true;
        Ok(plan)
    }
}
