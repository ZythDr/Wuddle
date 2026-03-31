use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use super::{apply_if_none_match, etag_from_headers, handle_304, DetectedRepo};
use crate::model::{LatestRelease, ReleaseAsset};

#[derive(Debug, Deserialize)]
struct GiteaRelease {
    tag_name: String,
    name: Option<String>,
    published_at: Option<String>,
    assets: Vec<GiteaAsset>,
}

#[derive(Debug, Deserialize)]
struct GiteaAsset {
    id: Option<u64>,
    name: String,
    browser_download_url: String,
    size: Option<u64>,
}

/// Fetch all releases for a repo (paginated, newest first).
pub async fn list_releases(
    client: &Client,
    repo: &DetectedRepo,
) -> Result<Vec<crate::model::LatestRelease>> {
    let mut page = 1u32;
    let mut all = Vec::new();
    loop {
        let url = format!(
            "https://{}/api/v1/repos/{}/releases?limit=50&page={}",
            repo.host, repo.project_path, page
        );
        let resp = client
            .get(&url)
            .header("User-Agent", "wuddle-engine")
            .header("Accept", "application/json")
            .send()
            .await
            .context("gitea list_releases request failed")?;
        if resp.status() == StatusCode::NOT_FOUND {
            break;
        }
        let resp = resp
            .error_for_status()
            .context("gitea list_releases error")?;
        let rels: Vec<GiteaRelease> = resp.json().await.context("invalid gitea json")?;
        if rels.is_empty() {
            break;
        }
        for rel in &rels {
            let assets = rel
                .assets
                .iter()
                .map(|a| ReleaseAsset {
                    id: a.id.map(|v| v.to_string()),
                    name: a.name.clone(),
                    download_url: a.browser_download_url.clone(),
                    size: a.size,
                    content_type: None,
                    sha256: None,
                })
                .collect();
            all.push(crate::model::LatestRelease {
                tag: rel.tag_name.clone(),
                name: rel.name.clone(),
                assets,
                published_at: rel
                    .published_at
                    .as_deref()
                    .and_then(super::parse_rfc3339_unix),
            });
        }
        if rels.len() < 50 {
            break;
        }
        page += 1;
    }
    Ok(all)
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
            published_at: rel.published_at.as_deref().and_then(super::parse_rfc3339_unix),
        }),
        false,
    ))
}
