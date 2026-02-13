use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

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

pub fn sha256_file_hex(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path).with_context(|| format!("open {:?}", path))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f
            .read(&mut buf)
            .with_context(|| format!("read {:?}", path))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}
