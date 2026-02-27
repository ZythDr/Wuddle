use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use url::Url;

use crate::model::LatestRelease;

pub mod git_sync;
pub mod gitea;
pub mod github;
pub mod gitlab;

const RELEASE_CACHE_TTL: Duration = Duration::from_secs(45);

#[derive(Clone)]
struct CachedRelease {
    fetched_at: Instant,
    etag: Option<String>,
    release: LatestRelease,
}

static RELEASE_CACHE: OnceLock<Mutex<HashMap<String, CachedRelease>>> = OnceLock::new();

fn release_cache() -> &'static Mutex<HashMap<String, CachedRelease>> {
    RELEASE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(repo: &DetectedRepo) -> String {
    let forge = match repo.kind {
        ForgeKind::GitHub => "github",
        ForgeKind::GitLab => "gitlab",
        ForgeKind::Gitea => "gitea",
    };
    format!(
        "{}|{}|{}",
        forge,
        repo.host.to_lowercase(),
        repo.project_path.to_lowercase()
    )
}

fn cache_read(
    repo: &DetectedRepo,
    etag: Option<&str>,
) -> Option<(Option<String>, Option<LatestRelease>, bool)> {
    let key = cache_key(repo);
    let mut guard = release_cache().lock().ok()?;
    let entry = guard.get(&key)?;
    if entry.fetched_at.elapsed() > RELEASE_CACHE_TTL {
        guard.remove(&key);
        return None;
    }

    if etag.is_some() && entry.etag.as_deref() == etag {
        return Some((entry.etag.clone(), None, true));
    }
    Some((entry.etag.clone(), Some(entry.release.clone()), false))
}

fn cache_write(repo: &DetectedRepo, etag: Option<String>, release: LatestRelease) {
    let key = cache_key(repo);
    if let Ok(mut guard) = release_cache().lock() {
        guard.insert(
            key,
            CachedRelease {
                fetched_at: Instant::now(),
                etag,
                release,
            },
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeKind {
    GitHub,
    GitLab,
    Gitea, // includes Codeberg (Gitea)
}

#[derive(Debug, Clone)]
pub struct DetectedRepo {
    pub kind: ForgeKind,
    pub forge_str: &'static str, // "github" | "gitlab" | "gitea"
    pub host: String,
    pub owner: String, // GitHub/Gitea: owner. GitLab: namespace path (group/subgroup)
    pub name: String,  // repo/project name
    pub canonical_url: String, // scheme://host/<project_path>
    pub project_path: String, // GitHub/Gitea: owner/name. GitLab: full path group/sub/project
}

/// Accepts repo URLs with or without /releases and normalizes them.
pub fn detect_repo(input: &str) -> Result<DetectedRepo> {
    let input = input.trim();

    let url = Url::parse(input).context("invalid URL")?;
    let host = url.host_str().context("URL missing host")?.to_string();
    let scheme = url.scheme();

    // path segments without empty pieces
    let mut segs: Vec<String> = url
        .path_segments()
        .map(|it| {
            it.filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    if segs.is_empty() {
        anyhow::bail!("URL path is empty");
    }

    // normalize common suffixes
    // GitHub/Gitea: /owner/repo/releases[/...]
    if segs.len() >= 3 && segs[2].eq_ignore_ascii_case("releases") {
        segs.truncate(2);
    }
    // GitLab: /group/sub/project/-/releases
    if segs.len() >= 3 {
        // remove trailing "latest" or similar after /releases
        while segs
            .last()
            .map(|s| s.eq_ignore_ascii_case("latest"))
            .unwrap_or(false)
        {
            segs.pop();
        }
        // if ends with ... /-/releases
        if segs.len() >= 2
            && segs[segs.len() - 2] == "-"
            && segs[segs.len() - 1].eq_ignore_ascii_case("releases")
        {
            segs.truncate(segs.len() - 2);
        }
        // if ends with ... /-/tags or /tags
        if segs.len() >= 2
            && segs[segs.len() - 2] == "-"
            && segs[segs.len() - 1].eq_ignore_ascii_case("tags")
        {
            segs.truncate(segs.len() - 2);
        }
        if segs
            .last()
            .map(|s| s.eq_ignore_ascii_case("tags"))
            .unwrap_or(false)
        {
            segs.pop();
        }
    }

    // determine forge kind
    let kind = if host.eq_ignore_ascii_case("github.com") {
        ForgeKind::GitHub
    } else if host.eq_ignore_ascii_case("gitlab.com") {
        ForgeKind::GitLab
    } else if host.eq_ignore_ascii_case("codeberg.org") {
        ForgeKind::Gitea
    } else {
        // heuristic: if the URL contains "/-/" anywhere, treat as GitLab-ish
        if url.path().contains("/-/") {
            ForgeKind::GitLab
        } else {
            ForgeKind::Gitea
        }
    };

    match kind {
        ForgeKind::GitHub | ForgeKind::Gitea => {
            if segs.len() < 2 {
                anyhow::bail!(
                    "Expected URL like https://host/owner/repo (got path {})",
                    url.path()
                );
            }
            let owner = segs[0].clone();
            let mut name = segs[1].clone();
            if name.ends_with(".git") {
                name.truncate(name.len() - 4);
            }
            let project_path = format!("{}/{}", owner, name);
            let canonical_url = format!("{scheme}://{host}/{project_path}");
            Ok(DetectedRepo {
                kind,
                forge_str: if kind == ForgeKind::GitHub {
                    "github"
                } else {
                    "gitea"
                },
                host,
                owner,
                name,
                canonical_url,
                project_path,
            })
        }
        ForgeKind::GitLab => {
            if segs.len() < 2 {
                anyhow::bail!(
                    "Expected URL like https://host/group/project (got path {})",
                    url.path()
                );
            }
            // GitLab allows subgroups: group/sub/project
            let mut project_segs = segs.clone();
            // strip trailing .git
            if let Some(last) = project_segs.last_mut() {
                if last.ends_with(".git") {
                    last.truncate(last.len() - 4);
                }
            }
            let name = project_segs
                .last()
                .cloned()
                .unwrap_or_else(|| "project".into());
            let owner = project_segs[..project_segs.len().saturating_sub(1)].join("/");
            let project_path = project_segs.join("/");
            let canonical_url = format!("{scheme}://{host}/{project_path}");
            Ok(DetectedRepo {
                kind,
                forge_str: "gitlab",
                host,
                owner,
                name,
                canonical_url,
                project_path,
            })
        }
    }
}

/// Unified "latest release" fetch with optional ETag.
/// Returns: (new_etag, release_or_none, not_modified)
pub async fn latest_release(
    client: &Client,
    repo: &DetectedRepo,
    etag: Option<&str>,
) -> Result<(Option<String>, Option<LatestRelease>, bool)> {
    if let Some(hit) = cache_read(repo, etag) {
        return Ok(hit);
    }

    let out = match repo.kind {
        ForgeKind::GitHub => github::latest_release(client, repo, etag).await,
        ForgeKind::GitLab => gitlab::latest_release(client, repo, etag).await,
        ForgeKind::Gitea => gitea::latest_release(client, repo, etag).await,
    }?;

    if let Some(rel) = out.1.clone() {
        cache_write(repo, out.0.clone(), rel);
    }

    Ok(out)
}

/// Helper for forges that support 304 Not Modified.
pub(crate) fn etag_from_headers(resp: &reqwest::Response) -> Option<String> {
    resp.headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Convenience for building conditional GET requests.
fn apply_if_none_match(
    mut req: reqwest::RequestBuilder,
    etag: Option<&str>,
) -> reqwest::RequestBuilder {
    if let Some(et) = etag {
        req = req.header("If-None-Match", et);
    }
    req
}

/// Common handler for 304.
fn handle_304(
    status: StatusCode,
    etag: Option<&str>,
) -> Option<(Option<String>, Option<LatestRelease>, bool)> {
    if status == StatusCode::NOT_MODIFIED {
        return Some((etag.map(|s| s.to_string()), None, true));
    }
    None
}
