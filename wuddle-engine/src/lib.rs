use anyhow::Result;
use reqwest::Client;
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

mod db;
mod forge;
mod github;
mod install;
mod model;
mod util;

pub use db::Db;
pub use install::InstallOptions;
pub use model::{InstallMode, Repo};

use crate::forge::detect_repo;
use crate::forge::ForgeKind;
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

        let repo = Repo {
            id: 0,
            url: det.canonical_url.clone(),
            forge: det.forge_str.to_string(),
            host: det.host.clone(),
            owner: det.owner.clone(),
            name: det.name.clone(),
            mode,
            enabled: true,
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

    async fn build_update_plan_for_repo(
        &self,
        r: &Repo,
        use_cached_etag: bool,
        wow_dir: Option<&Path>,
    ) -> Result<UpdatePlan> {
        if !r.enabled {
            return Ok(Self::blank_plan(r));
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

    pub async fn check_updates_with_wow(&self, wow_dir: Option<&Path>) -> Result<Vec<UpdatePlan>> {
        let repos = self.db.list_repos()?;
        let mut plans = Vec::with_capacity(repos.len());

        // Keep checks concurrent in small bounded batches to avoid overloading forge APIs.
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
