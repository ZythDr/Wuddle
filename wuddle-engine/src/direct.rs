use anyhow::{Context, Result};
use std::path::Path;
use url::Url;

use crate::util;

#[derive(Debug, Clone)]
pub struct DirectArchive {
    pub url: String,
    pub host: String,
    pub asset_name: String,
    pub display_name: String,
    pub url_hash: String,
}

pub fn is_direct_archive_url(url: &str) -> bool {
    parse_archive_url(url).is_ok()
}

pub fn parse_archive_url(url: &str) -> Result<DirectArchive> {
    let trimmed = url.trim();
    let parsed = Url::parse(trimmed).context("Direct archive URL must be an absolute URL")?;
    if parsed.scheme() == "file" {
        let path = parsed
            .to_file_path()
            .map_err(|_| anyhow::anyhow!("Local archive URL path is invalid"))?;
        return parse_local_archive_path(&path);
    }
    if parsed.scheme() != "https" {
        anyhow::bail!("Direct archive URLs must use HTTPS or file://");
    }

    let host = parsed
        .host_str()
        .map(|h| h.to_ascii_lowercase())
        .ok_or_else(|| anyhow::anyhow!("Direct archive URL is missing a host"))?;

    let raw_name = parsed
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Direct archive URL is missing a filename"))?;

    let decoded = urlencoding::decode(raw_name)
        .map(|name| name.into_owned())
        .unwrap_or_else(|_| raw_name.to_string());
    let asset_name = Path::new(&decoded)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Direct archive URL filename is invalid"))?
        .to_string();

    if !is_supported_archive_name(&asset_name) {
        anyhow::bail!("Direct archive URL must point to a .zip or .7z file");
    }

    let display_name = archive_stem(&asset_name).unwrap_or(&asset_name).to_string();
    let url_hash = util::sha256_hex(trimmed);

    Ok(DirectArchive {
        url: trimmed.to_string(),
        host,
        asset_name,
        display_name,
        url_hash,
    })
}

pub fn parse_local_archive_path(path: &Path) -> Result<DirectArchive> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("Local archive file not found: {:?}", path))?;
    if !canonical.is_file() {
        anyhow::bail!("Local archive path is not a file: {:?}", canonical);
    }

    let asset_name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Local archive filename is invalid"))?
        .to_string();

    if !is_supported_archive_name(&asset_name) {
        anyhow::bail!("Local archive must be a .zip or .7z file");
    }

    let url = Url::from_file_path(&canonical)
        .map_err(|_| anyhow::anyhow!("Could not convert local archive path to file URL"))?
        .to_string();
    let display_name = archive_stem(&asset_name).unwrap_or(&asset_name).to_string();
    let url_hash = util::sha256_hex(&url);

    Ok(DirectArchive {
        url,
        host: "local".to_string(),
        asset_name,
        display_name,
        url_hash,
    })
}

pub fn is_supported_archive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".7z")
}

fn archive_stem(name: &str) -> Option<&str> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".zip") {
        name.get(..name.len().saturating_sub(4))
    } else if lower.ends_with(".7z") {
        name.get(..name.len().saturating_sub(3))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{is_direct_archive_url, parse_archive_url};

    #[test]
    fn accepts_https_archive_download_urls() {
        let parsed = parse_archive_url(
            "https://github.com/noname08662/ElvUI_Extras/releases/download/1.11/ElvUI_Extras_ElvUI_6.09.zip",
        )
        .unwrap();

        assert_eq!(parsed.host, "github.com");
        assert_eq!(parsed.asset_name, "ElvUI_Extras_ElvUI_6.09.zip");
        assert_eq!(parsed.display_name, "ElvUI_Extras_ElvUI_6.09");
    }

    #[test]
    fn release_pages_are_not_direct_archives() {
        assert!(!is_direct_archive_url(
            "https://github.com/noname08662/ElvUI_Extras/releases/tag/1.11"
        ));
    }

    #[test]
    fn rejects_non_https_and_unsupported_files() {
        assert!(!is_direct_archive_url("http://example.com/addon.zip"));
        assert!(!is_direct_archive_url("https://example.com/addon.rar"));
    }
}
