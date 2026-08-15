use anyhow::{Context, Result};
use reqwest::{
    header::{CONTENT_LENGTH, LOCATION},
    Client, StatusCode,
};
use std::{io::Write, path::Path};
use url::Url;

pub(crate) const MAX_REMOTE_ASSET_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_REDIRECTS: usize = 8;

fn redirect_target(response_url: &Url, location: &str) -> Result<Url> {
    response_url
        .join(location)
        .context("Download redirect contained an invalid destination")
}

fn response_content_length(response: &reqwest::Response) -> Result<Option<u64>> {
    let Some(value) = response.headers().get(CONTENT_LENGTH) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .context("Download returned an invalid Content-Length header")?;
    let length = value
        .parse::<u64>()
        .context("Download returned an invalid Content-Length header")?;
    Ok(Some(length))
}

fn persist_staged_file(staged: tempfile::NamedTempFile, destination: &Path) -> Result<()> {
    staged
        .persist(destination)
        .map(|_| ())
        .map_err(|error| error.error)
        .with_context(|| {
            format!(
                "Failed to commit the completed download as {}",
                destination
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("the requested asset")
            )
        })
}

/// Download an HTTPS resource without delegating redirect decisions to
/// reqwest. Every hop is passed through `validate_url`, the response is
/// streamed into a same-directory temporary file, and only a fully received
/// and validated file is persisted.
pub(crate) async fn download_to_file<U, F>(
    client: &Client,
    initial_url: &str,
    destination: &Path,
    max_bytes: u64,
    validate_url: U,
    validate_file: F,
) -> Result<()>
where
    U: Fn(&str) -> Result<()>,
    F: Fn(&Path) -> Result<()>,
{
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Download destination has no parent directory"))?;
    std::fs::create_dir_all(parent)?;

    let mut current = Url::parse(initial_url).context("Download URL is invalid")?;
    for redirect_count in 0..=MAX_REDIRECTS {
        validate_url(current.as_str())?;

        let mut response = client.get(current.clone()).send().await?;
        if response.status().is_redirection() {
            if redirect_count == MAX_REDIRECTS {
                anyhow::bail!("Download exceeded the redirect limit");
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| anyhow::anyhow!("Download redirect omitted its destination"))?;
            current = redirect_target(response.url(), location)?;
            continue;
        }

        if response.status() != StatusCode::OK && !response.status().is_success() {
            response.error_for_status_ref()?;
        }
        if response_content_length(&response)?.is_some_and(|length| length > max_bytes) {
            anyhow::bail!("Download exceeds Wuddle's maximum supported asset size");
        }

        let mut staged = tempfile::Builder::new()
            .prefix(".wuddle-download-")
            .tempfile_in(parent)?;
        let mut received = 0u64;
        while let Some(chunk) = response.chunk().await? {
            received = received
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| anyhow::anyhow!("Download size overflowed"))?;
            if received > max_bytes {
                anyhow::bail!("Download exceeds Wuddle's maximum supported asset size");
            }
            staged.write_all(&chunk)?;
        }
        staged.flush()?;
        staged.as_file().sync_all()?;
        validate_file(staged.path())?;
        return persist_staged_file(staged, destination);
    }

    unreachable!("redirect loop always returns or errors")
}

#[cfg(test)]
mod tests {
    use super::redirect_target;
    use url::Url;

    #[test]
    fn resolves_relative_redirects_against_the_response_url() {
        let source = Url::parse("https://downloads.example/releases/file").unwrap();
        let target = redirect_target(&source, "../asset.zip").unwrap();
        assert_eq!(target.as_str(), "https://downloads.example/asset.zip");
    }
}
