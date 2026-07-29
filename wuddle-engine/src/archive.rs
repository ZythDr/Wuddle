use anyhow::Result;
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

pub(crate) const MAX_ARCHIVE_ENTRIES: usize = 20_000;
pub(crate) const MAX_ARCHIVE_DEPTH: usize = 32;
pub(crate) const MAX_ARCHIVE_ENTRY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub(crate) const MAX_ARCHIVE_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_COMPRESSION_RATIO: u64 = 500;
const COMPRESSION_RATIO_MIN_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Default)]
pub(crate) struct ArchiveBudget {
    entries: usize,
    unpacked_bytes: u64,
    paths: HashSet<String>,
}

fn is_windows_device_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(['.', ' '])
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || stem.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
}

pub(crate) fn safe_relative_path(name: &str) -> Result<PathBuf> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed != name
        || trimmed.contains('\\')
        || trimmed
            .chars()
            .any(|character| character.is_control() || character == ':')
    {
        anyhow::bail!("Archive contains an unsafe entry path");
    }

    let mut safe = PathBuf::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(part) => {
                let part = part
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Archive entry name is not valid UTF-8"))?;
                if part.is_empty() || part.ends_with(['.', ' ']) || is_windows_device_name(part) {
                    anyhow::bail!("Archive contains an unsafe entry path");
                }
                safe.push(part);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("Archive contains an unsafe entry path");
            }
        }
    }

    if safe.as_os_str().is_empty() {
        anyhow::bail!("Archive contains an empty entry path");
    }
    if safe.components().count() > MAX_ARCHIVE_DEPTH {
        anyhow::bail!("Archive entry exceeds Wuddle's directory-depth limit");
    }
    Ok(safe)
}

impl ArchiveBudget {
    pub(crate) fn register(
        &mut self,
        name: &str,
        is_directory: bool,
        unpacked_bytes: u64,
        compressed_bytes: Option<u64>,
    ) -> Result<PathBuf> {
        self.entries = self
            .entries
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Archive entry count overflowed"))?;
        if self.entries > MAX_ARCHIVE_ENTRIES {
            anyhow::bail!("Archive exceeds Wuddle's entry-count limit");
        }

        let path = safe_relative_path(name)?;
        let identity = path
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        if !self.paths.insert(identity) {
            anyhow::bail!("Archive contains duplicate or case-colliding entry paths");
        }

        if is_directory {
            if unpacked_bytes != 0 {
                anyhow::bail!("Archive directory entry declares file contents");
            }
            return Ok(path);
        }
        if unpacked_bytes > MAX_ARCHIVE_ENTRY_BYTES {
            anyhow::bail!("Archive entry exceeds Wuddle's per-file size limit");
        }
        self.unpacked_bytes = self
            .unpacked_bytes
            .checked_add(unpacked_bytes)
            .ok_or_else(|| anyhow::anyhow!("Archive expanded size overflowed"))?;
        if self.unpacked_bytes > MAX_ARCHIVE_TOTAL_BYTES {
            anyhow::bail!("Archive exceeds Wuddle's total expanded-size limit");
        }

        if unpacked_bytes >= COMPRESSION_RATIO_MIN_BYTES {
            if let Some(compressed_bytes) = compressed_bytes {
                if compressed_bytes == 0
                    || unpacked_bytes > compressed_bytes.saturating_mul(MAX_COMPRESSION_RATIO)
                {
                    anyhow::bail!("Archive entry exceeds Wuddle's compression-ratio limit");
                }
            }
        }
        Ok(path)
    }

    pub(crate) fn validate_archive_ratio(&self, archive_bytes: u64) -> Result<()> {
        if self.unpacked_bytes >= COMPRESSION_RATIO_MIN_BYTES
            && (archive_bytes == 0
                || self.unpacked_bytes > archive_bytes.saturating_mul(MAX_COMPRESSION_RATIO))
        {
            anyhow::bail!("Archive exceeds Wuddle's compression-ratio limit");
        }
        Ok(())
    }
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

pub(crate) fn validate_extracted_tree(root: &Path) -> Result<()> {
    fn visit(root: &Path, directory: &Path, budget: &mut ArchiveBudget) -> Result<()> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if is_link_or_reparse(&metadata) {
                anyhow::bail!("Extracted archive contains a link or reparse point");
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|_| anyhow::anyhow!("Extracted archive escaped its staging root"))?;
            let relative = relative.to_string_lossy().replace('\\', "/");
            if metadata.is_dir() {
                budget.register(&relative, true, 0, None)?;
                visit(root, &path, budget)?;
            } else if metadata.is_file() {
                budget.register(&relative, false, metadata.len(), None)?;
            } else {
                anyhow::bail!("Extracted archive contains an unsupported filesystem entry");
            }
        }
        Ok(())
    }

    let metadata = fs::symlink_metadata(root)?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        anyhow::bail!("Archive staging root is not a normal directory");
    }
    visit(root, root, &mut ArchiveBudget::default())
}

#[cfg(test)]
mod tests {
    use super::{safe_relative_path, ArchiveBudget, MAX_ARCHIVE_DEPTH};

    #[test]
    fn rejects_cross_platform_unsafe_archive_paths() {
        assert!(safe_relative_path("folder/file.dll").is_ok());
        for path in [
            "../evil.dll",
            "/tmp/evil.dll",
            "C:/evil.dll",
            "C:\\evil.dll",
            "folder/file.dll:stream",
            "folder/CON.dll",
            "folder/trailing.",
            "",
        ] {
            assert!(
                safe_relative_path(path).is_err(),
                "unexpectedly allowed {path}"
            );
        }
        let too_deep = std::iter::repeat_n("folder", MAX_ARCHIVE_DEPTH + 1)
            .collect::<Vec<_>>()
            .join("/");
        assert!(safe_relative_path(&too_deep).is_err());
    }

    #[test]
    fn rejects_duplicate_paths_and_extreme_compression_ratios() {
        let mut budget = ArchiveBudget::default();
        budget.register("File.txt", false, 4, Some(4)).unwrap();
        assert!(budget.register("file.TXT", false, 4, Some(4)).is_err());

        let mut budget = ArchiveBudget::default();
        assert!(budget
            .register("bomb.bin", false, 32 * 1024 * 1024, Some(1))
            .is_err());
    }
}
