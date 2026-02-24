use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use crate::model::{LatestRelease, ReleaseAsset};

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    name: Option<String>,
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

fn compact_body(body: &str) -> String {
    body.replace('\n', " ").trim().chars().take(220).collect()
}

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

        if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
            let remaining = resp
                .headers()
                .get("x-ratelimit-remaining")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("?")
                .to_string();
            let reset = resp
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("?")
                .to_string();
            let body = compact_body(&resp.text().await.unwrap_or_default());
            anyhow::bail!(
                "GitHub API rate-limited or forbidden (HTTP {}, remaining {}, reset {}). {} Add a GitHub token in Wuddle settings to raise limits.",
                status,
                remaining,
                reset,
                body
            );
        }

        if !status.is_success() {
            let body = compact_body(&resp.text().await.unwrap_or_default());
            anyhow::bail!("GitHub API error HTTP {}: {}", status, body);
        }

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
                assets,
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
