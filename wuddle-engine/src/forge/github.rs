use anyhow::Result;
use reqwest::Client;

use super::DetectedRepo;
use crate::model::LatestRelease;

/// GitHub implementation delegates to existing crate::github::GitHub
pub async fn latest_release(
    client: &Client,
    repo: &DetectedRepo,
    etag: Option<&str>,
) -> Result<(Option<String>, Option<LatestRelease>, bool)> {
    crate::github::GitHub::latest_release(client, &repo.owner, &repo.name, etag).await
}
