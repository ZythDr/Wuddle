use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use crate::model::{LatestRelease, ReleaseAsset};

fn rate_limit_reset(response: &reqwest::Response) -> Option<i64> {
    response
        .headers()
        .get("x-ratelimit-reset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
}

fn rate_limit_error(has_token: bool, reset_epoch: Option<i64>) -> String {
    let reset = reset_epoch
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let message = if has_token {
        "GitHub's API limit has been reached. The saved token may be expired, invalid, or shared with other applications; re-save it in Options."
    } else {
        "GitHub's anonymous API limit of 60 requests per hour has been reached. Add a GitHub token in Options to raise the limit to 5,000 requests per hour."
    };
    format!("GITHUB_RATE_LIMIT:{reset}:{message}")
}

async fn checked_response(response: reqwest::Response, context: &str) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let remaining_is_zero = response
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim() == "0");
    let reset_epoch = rate_limit_reset(&response);
    let body = response
        .text()
        .await
        .unwrap_or_default()
        .to_ascii_lowercase();
    if status == StatusCode::TOO_MANY_REQUESTS
        || remaining_is_zero
        || (status == StatusCode::FORBIDDEN && body.contains("rate limit"))
    {
        anyhow::bail!(rate_limit_error(
            crate::github_token().is_some(),
            reset_epoch
        ));
    }
    if status == StatusCode::UNAUTHORIZED
        || body.contains("bad credentials")
        || body.contains("requires authentication")
    {
        anyhow::bail!(if crate::github_token().is_some() {
            "GitHub authentication failed. Re-save or replace the saved token in Options."
        } else {
            "GitHub requires authentication for this request. Add a GitHub token in Options."
        });
    }
    anyhow::bail!("{context}: GitHub returned HTTP {}", status.as_u16());
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    name: Option<String>,
    #[serde(default)]
    prerelease: bool,
    published_at: Option<String>,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    id: Option<u64>,
    name: String,
    browser_download_url: String,
    size: Option<u64>,
    content_type: Option<String>,
    digest: Option<String>,
}

pub struct GitHub;

fn parse_sha256_digest(raw: Option<&str>) -> Option<String> {
    let digest = raw?.trim();
    if digest.is_empty() {
        return None;
    }
    let hex = digest
        .strip_prefix("sha256:")
        .or_else(|| digest.strip_prefix("SHA256:"))
        .unwrap_or(digest)
        .trim()
        .to_ascii_lowercase();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(hex)
}

impl GitHub {
    pub async fn latest_release(
        client: &Client,
        owner: &str,
        repo: &str,
        etag: Option<&str>,
    ) -> Result<(Option<String>, Option<LatestRelease>, bool)> {
        // returns (new_etag, release_or_none, not_modified)
        let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");

        let mut req = client
            .get(url)
            .header("User-Agent", "wuddle-engine")
            .header("Accept", "application/vnd.github+json");

        let token = crate::github_token();
        if let Some(token) = token {
            req = req.bearer_auth(token);
        }

        if let Some(et) = etag {
            req = req.header("If-None-Match", et);
        }

        let resp = req.send().await.context("github request failed")?;
        let status = resp.status();

        if status == StatusCode::NOT_MODIFIED {
            // 304 - no changes
            return Ok((etag.map(|s| s.to_string()), None, true));
        }

        let new_etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        if status == StatusCode::NOT_FOUND {
            anyhow::bail!("GitHub repo/release not found (no latest release?)");
        }

        let resp = checked_response(resp, "GitHub release request failed").await?;

        let gh: GhRelease = resp.json().await.context("invalid github json")?;

        let assets = gh
            .assets
            .into_iter()
            .map(|a| ReleaseAsset {
                id: a.id.map(|v| v.to_string()),
                name: a.name,
                download_url: a.browser_download_url,
                size: a.size,
                content_type: a.content_type,
                sha256: parse_sha256_digest(a.digest.as_deref()),
            })
            .collect();

        Ok((
            new_etag,
            Some(LatestRelease {
                tag: gh.tag_name,
                name: gh.name,
                prerelease: gh.prerelease,
                assets,
                published_at: gh
                    .published_at
                    .as_deref()
                    .and_then(super::parse_rfc3339_unix),
            }),
            false,
        ))
    }
}

use super::DetectedRepo;

pub async fn latest_release(
    client: &Client,
    repo: &DetectedRepo,
    etag: Option<&str>,
) -> Result<(Option<String>, Option<LatestRelease>, bool)> {
    GitHub::latest_release(client, &repo.owner, &repo.name, etag).await
}

/// Fetch all releases for a GitHub repo (paginated, newest first).
pub async fn list_releases(client: &Client, repo: &DetectedRepo) -> Result<Vec<LatestRelease>> {
    let mut page = 1u32;
    let mut all = Vec::new();
    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases?per_page=100&page={}",
            repo.owner, repo.name, page
        );
        let mut req = client
            .get(&url)
            .header("User-Agent", "wuddle-engine")
            .header("Accept", "application/vnd.github+json");
        if let Some(token) = crate::github_token() {
            req = req.bearer_auth(token);
        }
        let resp = req
            .send()
            .await
            .context("github list_releases request failed")?;
        if resp.status() == StatusCode::NOT_FOUND {
            break;
        }
        let resp = checked_response(resp, "GitHub release-list request failed").await?;
        let rels: Vec<GhRelease> = resp.json().await.context("invalid github json")?;
        if rels.is_empty() {
            break;
        }
        for gh in &rels {
            let assets = gh
                .assets
                .iter()
                .map(|a| ReleaseAsset {
                    id: a.id.map(|v| v.to_string()),
                    name: a.name.clone(),
                    download_url: a.browser_download_url.clone(),
                    size: a.size,
                    content_type: a.content_type.clone(),
                    sha256: parse_sha256_digest(a.digest.as_deref()),
                })
                .collect();
            all.push(LatestRelease {
                tag: gh.tag_name.clone(),
                name: gh.name.clone(),
                prerelease: gh.prerelease,
                assets,
                published_at: gh
                    .published_at
                    .as_deref()
                    .and_then(super::parse_rfc3339_unix),
            });
        }
        if rels.len() < 100 {
            break;
        }
        page += 1;
    }
    Ok(all)
}

#[derive(Debug, Deserialize)]
struct GhTreeResponse {
    tree: Vec<GhTreeEntry>,
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct GhTreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String, // "blob" or "tree"
}

pub struct RepoFile {
    pub path: String,
    pub is_dir: bool,
}

fn complete_tree_files(tree: GhTreeResponse) -> Result<Vec<RepoFile>> {
    if tree.truncated {
        anyhow::bail!(
            "GitHub returned an incomplete repository tree; a staged Git probe is required"
        );
    }
    Ok(tree
        .tree
        .into_iter()
        .map(|entry| RepoFile {
            path: entry.path,
            is_dir: entry.kind == "tree",
        })
        .collect())
}

/// Fetch all files in a repo recursively using the Tree API.
pub async fn list_files_recursive(
    client: &Client,
    owner: &str,
    repo: &str,
    branch: Option<&str>,
) -> Result<Vec<RepoFile>> {
    let branch = branch.unwrap_or("HEAD");
    let url = format!(
        "https://api.github.com/repos/{}/{}/git/trees/{}?recursive=1",
        owner, repo, branch
    );

    let mut req = client
        .get(&url)
        .header("User-Agent", "wuddle-engine")
        .header("Accept", "application/vnd.github+json");

    if let Some(token) = crate::github_token() {
        req = req.bearer_auth(token);
    }

    let resp = req.send().await.context("github tree request failed")?;
    if resp.status() == StatusCode::NOT_FOUND && branch != "HEAD" {
        // Try fallback to HEAD if branch failed
        return Box::pin(list_files_recursive(client, owner, repo, None)).await;
    }

    let resp = checked_response(resp, "GitHub tree request failed").await?;

    let tree: GhTreeResponse = resp.json().await.context("invalid github tree json")?;

    complete_tree_files(tree)
}

#[cfg(test)]
mod tests {
    use super::{complete_tree_files, GhTreeEntry, GhTreeResponse};

    #[test]
    fn truncated_recursive_trees_are_never_treated_as_authoritative() {
        let truncated = GhTreeResponse {
            tree: vec![GhTreeEntry {
                path: "Partial/Partial.toc".to_string(),
                kind: "blob".to_string(),
            }],
            truncated: true,
        };
        assert!(complete_tree_files(truncated).is_err());

        let complete = GhTreeResponse {
            tree: vec![GhTreeEntry {
                path: "Complete/Complete.toc".to_string(),
                kind: "blob".to_string(),
            }],
            truncated: false,
        };
        let files = complete_tree_files(complete).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "Complete/Complete.toc");
        assert!(!files[0].is_dir);
    }
}
