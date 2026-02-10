use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use super::{apply_if_none_match, etag_from_headers, handle_304, DetectedRepo};
use crate::model::{LatestRelease, ReleaseAsset};

#[derive(Debug, Deserialize)]
struct GitLabRelease {
    tag_name: String,
    name: Option<String>,
    assets: GitLabAssets,
}

#[derive(Debug, Deserialize)]
struct GitLabAssets {
    #[serde(default)]
    links: Vec<GitLabLink>,
    // sources exist too, but we intentionally ignore them
}

#[derive(Debug, Deserialize)]
struct GitLabLink {
    name: String,
    url: String,
    // direct_asset_url exists in newer GitLab; url should work for public assets
    #[allow(dead_code)]
    direct_asset_url: Option<String>,
}

pub async fn latest_release(
    client: &Client,
    repo: &DetectedRepo,
    etag: Option<&str>,
) -> Result<(Option<String>, Option<LatestRelease>, bool)> {
    let encoded = urlencoding::encode(&repo.project_path);
    let url = format!(
        "https://{}/api/v4/projects/{}/releases/permalink/latest",
        repo.host, encoded
    );

    let mut req = client
        .get(url)
        .header("User-Agent", "wuddle-engine")
        .header("Accept", "application/json");
    req = apply_if_none_match(req, etag);

    let resp = req.send().await.context("gitlab request failed")?;

    if let Some(x) = handle_304(resp.status(), etag) {
        return Ok(x);
    }

    let new_etag = etag_from_headers(&resp);

    if resp.status() == StatusCode::NOT_FOUND {
        anyhow::bail!("GitLab project/release not found (no latest release?)");
    }

    let resp = resp.error_for_status().context("gitlab error status")?;
    let rel: GitLabRelease = resp.json().await.context("invalid gitlab json")?;

    let assets = rel
        .assets
        .links
        .into_iter()
        .map(|l| {
            let url = l.direct_asset_url.unwrap_or(l.url);
            ReleaseAsset {
                id: None,
                name: l.name,
                download_url: url,
                size: None,
                content_type: None,
            }
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
