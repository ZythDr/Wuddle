use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

pub fn app_dir() -> Result<PathBuf> {
    let dir = dirs::data_dir().context("no data_dir")?.join("wuddle");
    fs::create_dir_all(&dir).context("create app dir")?;
    Ok(dir)
}

pub fn db_path() -> Result<PathBuf> {
    Ok(app_dir()?.join("wuddle.sqlite"))
}

pub fn cache_dir() -> Result<PathBuf> {
    let d = app_dir()?.join("cache");
    fs::create_dir_all(&d)?;
    Ok(d)
}

pub fn sha256_hex(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
}
