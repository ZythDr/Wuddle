use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{diagnostics, install};

struct BackupArea {
    parent: PathBuf,
    directory: tempfile::TempDir,
}

struct Backup {
    original: PathBuf,
    stored: PathBuf,
}

struct Deployment {
    target: PathBuf,
    restore_source: Option<PathBuf>,
}

/// A best-effort filesystem transaction for replacing live install targets.
///
/// Every existing target is first renamed into a hidden directory on the same
/// filesystem. New targets can then be deployed and SQLite committed. Dropping
/// an armed transaction removes the new targets and restores every displaced
/// entry in reverse order.
pub(crate) struct ReplacementTransaction {
    operation: &'static str,
    repo_id: i64,
    backup_areas: Vec<BackupArea>,
    backups: Vec<Backup>,
    deployments: Vec<Deployment>,
    armed: bool,
}

impl ReplacementTransaction {
    pub(crate) fn new(operation: &'static str, repo_id: i64) -> Self {
        Self {
            operation,
            repo_id,
            backup_areas: Vec::new(),
            backups: Vec::new(),
            deployments: Vec::new(),
            armed: true,
        }
    }

    fn same_path(left: &Path, right: &Path) -> bool {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }

    fn actual_case(path: &Path) -> Option<PathBuf> {
        if path.exists() || path.is_symlink() {
            return Some(path.to_path_buf());
        }
        let parent = path.parent()?;
        let target = path.file_name()?.to_string_lossy();
        fs::read_dir(parent)
            .ok()?
            .flatten()
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&target)
            })
            .map(|entry| entry.path())
    }

    fn backup_area(&mut self, parent: &Path) -> Result<&Path> {
        if let Some(index) = self
            .backup_areas
            .iter()
            .position(|area| Self::same_path(&area.parent, parent))
        {
            return Ok(self.backup_areas[index].directory.path());
        }
        fs::create_dir_all(parent).context("create replacement target parent")?;
        let directory = tempfile::Builder::new()
            .prefix(".wuddle-rollback-")
            .tempdir_in(parent)
            .context("create replacement rollback directory")?;
        self.backup_areas.push(BackupArea {
            parent: parent.to_path_buf(),
            directory,
        });
        Ok(self
            .backup_areas
            .last()
            .expect("backup area was just inserted")
            .directory
            .path())
    }

    pub(crate) fn backup_target(&mut self, path: &Path) -> Result<bool> {
        let actual = Self::actual_case(path).unwrap_or_else(|| path.to_path_buf());
        if self
            .backups
            .iter()
            .any(|backup| Self::same_path(&backup.original, &actual))
        {
            return Ok(true);
        }
        let metadata = match fs::symlink_metadata(&actual) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if !(metadata.file_type().is_symlink() || metadata.is_file() || metadata.is_dir()) {
            anyhow::bail!("Refusing to replace an unsupported filesystem entry");
        }
        let index = self.backups.len();
        let parent = actual
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Replacement target has no parent directory"))?;
        let stored = self.backup_area(parent)?.join(format!("target-{index}"));
        fs::rename(&actual, &stored).context("stage existing target for rollback")?;
        self.backups.push(Backup {
            original: actual,
            stored,
        });
        Ok(true)
    }

    pub(crate) fn deploy(&mut self, staged: &Path, target: &Path) -> Result<()> {
        self.backup_target(target)?;
        let parent = target
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Replacement target has no parent directory"))?;
        fs::create_dir_all(parent).context("create replacement target parent")?;
        fs::rename(staged, target).context("commit staged replacement target")?;
        self.deployments.push(Deployment {
            target: target.to_path_buf(),
            restore_source: None,
        });
        Ok(())
    }

    /// Move a persistent displaced-file backup back into its live location.
    /// If a later step fails, rollback returns it to `source` before restoring
    /// the replacement that occupied `target`.
    pub(crate) fn deploy_returnable(&mut self, source: &Path, target: &Path) -> Result<()> {
        self.backup_target(target)?;
        let parent = target
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Replacement target has no parent directory"))?;
        fs::create_dir_all(parent).context("create restored target parent")?;
        fs::rename(source, target).context("restore displaced install target")?;
        self.deployments.push(Deployment {
            target: target.to_path_buf(),
            restore_source: Some(source.to_path_buf()),
        });
        Ok(())
    }

    /// Retain one displaced target after commit so removing the replacement can
    /// restore it later. Rollback still treats the promoted path as the source
    /// of the original live target.
    pub(crate) fn promote_backup(&mut self, original: &Path, persistent: &Path) -> Result<()> {
        let backup = self
            .backups
            .iter_mut()
            .find(|backup| Self::same_path(&backup.original, original))
            .ok_or_else(|| anyhow::anyhow!("No rollback backup exists for this target"))?;
        if persistent.exists() || persistent.is_symlink() {
            anyhow::bail!("A displaced-file backup already exists");
        }
        let parent = persistent
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Persistent backup has no parent directory"))?;
        fs::create_dir_all(parent).context("create displaced-file backup directory")?;
        fs::rename(&backup.stored, persistent).context("retain displaced-file backup")?;
        backup.stored = persistent.to_path_buf();
        Ok(())
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }

    pub(crate) fn rollback(&mut self) -> Result<()> {
        let mut failures = Vec::new();
        for deployment in self.deployments.iter().rev() {
            let result = if let Some(source) = deployment.restore_source.as_deref() {
                if source.exists() || source.is_symlink() {
                    install::remove_any_target(source)
                } else if deployment.target.exists() || deployment.target.is_symlink() {
                    fs::rename(&deployment.target, source)
                        .context("return restored target to its backup")
                } else {
                    Ok(())
                }
            } else {
                install::remove_any_target(&deployment.target)
            };
            if let Err(error) = result {
                failures.push(error.to_string());
            }
        }
        for backup in self.backups.iter().rev() {
            if !(backup.stored.exists() || backup.stored.is_symlink()) {
                failures.push("a rollback backup was missing".to_string());
                continue;
            }
            if backup.original.exists() || backup.original.is_symlink() {
                if let Err(error) = install::remove_any_target(&backup.original) {
                    failures.push(error.to_string());
                    continue;
                }
            }
            if let Some(parent) = backup.original.parent() {
                if let Err(error) = fs::create_dir_all(parent) {
                    failures.push(error.to_string());
                    continue;
                }
            }
            if let Err(error) = fs::rename(&backup.stored, &backup.original) {
                failures.push(error.to_string());
            }
        }
        if failures.is_empty() {
            self.armed = false;
            Ok(())
        } else {
            anyhow::bail!(
                "Filesystem replacement rollback was incomplete ({} failed step(s))",
                failures.len()
            )
        }
    }
}

impl Drop for ReplacementTransaction {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.rollback",
            format!(
                "{} rollback started: repo_id={}; target_count={}",
                self.operation,
                self.repo_id,
                self.deployments.len()
            ),
        );
        if let Err(error) = self.rollback() {
            let backup_count = self.backups.len();
            for area in self.backup_areas.drain(..) {
                let _ = area.directory.keep();
            }
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Debug,
                "engine.rollback",
                format!(
                    "{} rollback failed: repo_id={}; preserved_backup_count={backup_count}; error={error}",
                    self.operation, self.repo_id
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ReplacementTransaction;
    use std::fs;

    #[test]
    fn dropping_armed_transaction_restores_every_replaced_target() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("shared.dll");
        let staged = temp.path().join("new.dll");
        fs::write(&target, b"old").unwrap();
        fs::write(&staged, b"new").unwrap();

        {
            let mut transaction = ReplacementTransaction::new("test", 1);
            transaction.deploy(&staged, &target).unwrap();
            assert_eq!(fs::read(&target).unwrap(), b"new");
        }

        assert_eq!(fs::read(&target).unwrap(), b"old");
        assert!(!staged.exists());
    }

    #[test]
    fn rollback_returns_restored_persistent_backup_to_its_storage_path() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("shared.dll");
        let persistent = temp.path().join("displaced.backup");
        fs::write(&target, b"replacement").unwrap();
        fs::write(&persistent, b"original").unwrap();

        {
            let mut transaction = ReplacementTransaction::new("test", 2);
            transaction.deploy_returnable(&persistent, &target).unwrap();
            assert_eq!(fs::read(&target).unwrap(), b"original");
        }

        assert_eq!(fs::read(&target).unwrap(), b"replacement");
        assert_eq!(fs::read(&persistent).unwrap(), b"original");
    }

    #[test]
    fn disarmed_transaction_keeps_replacement_and_displaced_backup() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("shared.dll");
        let staged = temp.path().join("new.dll");
        let persistent = temp.path().join("backups").join("shared.dll");
        fs::write(&target, b"old").unwrap();
        fs::write(&staged, b"new").unwrap();

        let mut transaction = ReplacementTransaction::new("test", 3);
        transaction.backup_target(&target).unwrap();
        transaction.promote_backup(&target, &persistent).unwrap();
        transaction.deploy(&staged, &target).unwrap();
        transaction.disarm();
        drop(transaction);

        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert_eq!(fs::read(&persistent).unwrap(), b"old");
    }
}
