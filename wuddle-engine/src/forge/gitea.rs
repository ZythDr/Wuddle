use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use super::{apply_if_none_match, etag_from_headers, handle_304, DetectedRepo};
use crate::model::{LatestRelease, ReleaseAsset};

#[derive(Debug, Deserialize)]
struct GiteaRelease {
    tag_name: String,
    name: Option<String>,
    assets: Vec<GiteaAsset>,
}

#[derive(Debug, Deserialize)]
struct GiteaAsset {
    id: Option<u64>,
    name: String,
    browser_download_url: String,
    size: Option<u64>,
}

pub async fn latest_release(
    client: &Client,
    repo: &DetectedRepo,
    etag: Option<&str>,
) -> Result<(Option<String>, Option<LatestRelease>, bool)> {
    // Gitea API: /api/v1/repos/{owner}/{repo}/releases/latest
    let url = format!(
        "https://{}/api/v1/repos/{}/releases/latest",
        repo.host, repo.project_path
    );

    let mut req = client
        .get(url)
        .header("User-Agent", "wuddle-engine")
        .header("Accept", "application/json");
    req = apply_if_none_match(req, etag);

    let resp = req.send().await.context("gitea request failed")?;

    if let Some(x) = handle_304(resp.status(), etag) {
        return Ok(x);
    }

    let new_etag = etag_from_headers(&resp);

    if resp.status() == StatusCode::NOT_FOUND {
        anyhow::bail!("Gitea repo/release not found (no latest release?)");
    }

    let resp = resp.error_for_status().context("gitea error status")?;
    let rel: GiteaRelease = resp.json().await.context("invalid gitea json")?;

    let assets = rel
        .assets
        .into_iter()
        .map(|a| ReleaseAsset {
            id: a.id.map(|v| v.to_string()),
            name: a.name,
            download_url: a.browser_download_url,
            size: a.size,
            content_type: None,
            sha256: None,
        })
        .collect();

    Ok((
        new_etag,
        Some(LatestRelease {
            tag: rel.tag_name,
            name: rel.name,
            assets,
        }),
        false,
    ))
}
