//! Platform-specific self-update transactions and restart handoffs.
//!
//! Windows portable builds keep a stable launcher beside versioned runtimes.
//! Because that launcher remains open while the UI runs, the UI stages a new
//! launcher and asks the newly installed runtime to replace it only after the
//! old process tree has released the executable.

use std::path::{Path, PathBuf};

#[cfg(any(target_os = "windows", test))]
use pelite::{FileMap, PeFile};
#[cfg(any(target_os = "linux", target_os = "windows", test))]
use std::fs;
#[cfg(any(target_os = "linux", test))]
use std::io::Read;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
use std::io::Write;
#[cfg(any(target_os = "windows", test))]
use std::path::Component;
#[cfg(any(target_os = "windows", test))]
use std::process::Command;
#[cfg(any(target_os = "windows", test))]
use std::time::{Duration, Instant};

#[cfg(any(target_os = "windows", test))]
const FINISH_LAUNCHER_UPDATE_ENV: &str = "WUDDLE_FINISH_LAUNCHER_UPDATE";
#[cfg(any(target_os = "windows", test))]
const PENDING_LAUNCHER_NAME: &str = ".Wuddle-launcher-update.exe";
#[cfg(any(target_os = "windows", test))]
const PREVIOUS_LAUNCHER_NAME: &str = ".Wuddle-launcher-previous.exe";
#[cfg(any(target_os = "windows", test))]
const LAUNCHER_REPLACEMENT_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(any(target_os = "windows", test))]
const LAUNCHER_REPLACEMENT_RETRY: Duration = Duration::from_millis(100);

#[cfg(any(target_os = "linux", test))]
fn validate_appimage_header(header: &[u8]) -> Result<(), String> {
    if header.len() < 20
        || &header[..4] != b"\x7fELF"
        || header[4] != 2
        || header[5] != 1
        || &header[8..11] != b"AI\x02"
        || u16::from_le_bytes([header[18], header[19]]) != 62
    {
        return Err(
            "The staged update is not a 64-bit x86 AppImage produced in the expected format."
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn validate_appimage_path(path: &Path, description: &str) -> Result<(), String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("Failed to open the {description}: {error}"))?;
    let mut header = [0_u8; 20];
    file.read_exact(&mut header)
        .map_err(|error| format!("Failed to inspect the {description}: {error}"))?;
    validate_appimage_header(&header)
}

#[cfg(target_os = "linux")]
fn previous_appimage_path(live: &Path) -> Result<PathBuf, String> {
    let name = live
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "The running AppImage has an invalid filename.".to_string())?;
    Ok(live.with_file_name(format!(".{name}.wuddle-previous")))
}

#[cfg(target_os = "linux")]
fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("Failed to synchronize the AppImage directory: {error}"))
}

#[cfg(target_os = "linux")]
fn require_regular_file(path: &Path, description: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Failed to inspect the {description}: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("The {description} is not a regular file."));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn install_linux_appimage_update(live: &Path, downloaded: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    require_regular_file(live, "running AppImage")?;
    validate_appimage_path(live, "running AppImage")?;
    require_regular_file(downloaded, "downloaded update")?;
    validate_appimage_path(downloaded, "downloaded update")?;

    let parent = live
        .parent()
        .ok_or_else(|| "The running AppImage has no parent directory.".to_string())?;
    let previous = previous_appimage_path(live)?;
    let displaced_previous = parent.join(format!(
        ".wuddle-appimage-previous-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));

    let mut staged = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("Failed to stage the AppImage update: {error}"))?;
    let mut source = fs::File::open(downloaded)
        .map_err(|error| format!("Failed to open the downloaded update: {error}"))?;
    std::io::copy(&mut source, staged.as_file_mut())
        .map_err(|error| format!("Failed to stage the AppImage update: {error}"))?;
    fs::set_permissions(staged.path(), fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("Failed to mark the staged AppImage executable: {error}"))?;
    staged
        .as_file_mut()
        .flush()
        .map_err(|error| format!("Failed to flush the staged AppImage: {error}"))?;
    staged
        .as_file()
        .sync_all()
        .map_err(|error| format!("Failed to synchronize the staged AppImage: {error}"))?;
    validate_appimage_path(staged.path(), "staged AppImage")?;

    let had_previous = match fs::symlink_metadata(&previous) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("The previous AppImage backup is not a regular file.".to_string());
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(format!(
                "Failed to inspect the previous AppImage backup: {error}"
            ));
        }
    };
    if had_previous {
        fs::rename(&previous, &displaced_previous)
            .map_err(|error| format!("Failed to rotate the previous AppImage backup: {error}"))?;
    }

    if let Err(error) = fs::rename(live, &previous) {
        if had_previous {
            let _ = fs::rename(&displaced_previous, &previous);
        }
        return Err(format!(
            "Failed to preserve the running AppImage before updating: {error}"
        ));
    }

    if let Err(error) = staged.persist(live) {
        let _ = fs::rename(&previous, live);
        if had_previous {
            let _ = fs::rename(&displaced_previous, &previous);
        }
        return Err(format!(
            "Failed to commit the staged AppImage: {}",
            error.error
        ));
    }

    if let Err(error) = sync_directory(parent) {
        let _ = fs::remove_file(live);
        let _ = fs::rename(&previous, live);
        if had_previous {
            let _ = fs::rename(&displaced_previous, &previous);
        }
        let _ = sync_directory(parent);
        return Err(format!(
            "{error}; the previous AppImage was restored before reporting the failure."
        ));
    }

    if had_previous {
        let _ = fs::remove_file(displaced_previous);
        let _ = sync_directory(parent);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn rollback_linux_appimage_update(live: &Path) -> Result<(), String> {
    let previous = previous_appimage_path(live)?;
    require_regular_file(&previous, "previous AppImage backup")?;
    validate_appimage_path(&previous, "previous AppImage backup")?;
    require_regular_file(live, "new AppImage")?;

    let parent = live
        .parent()
        .ok_or_else(|| "The running AppImage has no parent directory.".to_string())?;
    let failed = parent.join(format!(
        ".wuddle-appimage-failed-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    fs::rename(live, &failed)
        .map_err(|error| format!("Failed to quarantine the new AppImage: {error}"))?;
    if let Err(error) = fs::rename(&previous, live) {
        let _ = fs::rename(&failed, live);
        return Err(format!(
            "Failed to restore the previous AppImage; the new file was put back: {error}"
        ));
    }
    sync_directory(parent)?;
    let _ = fs::remove_file(failed);
    let _ = sync_directory(parent);
    Ok(())
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn canonical_windows_version_name(raw: &str) -> String {
    let normalized = raw.trim().trim_start_matches(['v', 'V']);
    let mut out = String::new();
    for ch in normalized.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            out.push(ch);
        }
    }
    if out.is_empty() {
        "vlatest".to_string()
    } else {
        format!("v{out}")
    }
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn detect_windows_launcher_root() -> Result<(PathBuf, bool), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut directory = executable.parent().map(Path::to_path_buf);
    for _ in 0..4 {
        let Some(candidate) = directory else {
            break;
        };
        if candidate.join("versions").is_dir()
            && (candidate.join("Wuddle.exe").is_file()
                || candidate.join(PENDING_LAUNCHER_NAME).is_file()
                || candidate.join(PREVIOUS_LAUNCHER_NAME).is_file())
        {
            let complete = candidate.join("Wuddle.exe").is_file();
            return Ok((candidate, complete));
        }
        directory = candidate.parent().map(Path::to_path_buf);
    }

    Ok((
        executable
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        false,
    ))
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn stage_windows_portable_update(
    root: &Path,
    downloaded_path: &Path,
    latest: &str,
) -> Result<String, String> {
    let version_name = canonical_windows_version_name(latest);
    let version_directory = root.join("versions").join(&version_name);
    fs::create_dir_all(&version_directory)
        .map_err(|error| format!("Failed to create the version directory: {error}"))?;

    let downloaded = fs::File::open(downloaded_path)
        .map_err(|error| format!("Failed to open the update package: {error}"))?;
    let mut archive = zip::ZipArchive::new(downloaded)
        .map_err(|error| format!("Failed to open the update package: {error}"))?;
    let mut runtime = tempfile::NamedTempFile::new_in(&version_directory)
        .map_err(|error| format!("Failed to stage the Wuddle runtime: {error}"))?;
    let mut launcher = tempfile::NamedTempFile::new_in(root)
        .map_err(|error| format!("Failed to stage the Wuddle launcher: {error}"))?;
    let mut runtime_count = 0usize;
    let mut launcher_count = 0usize;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Failed to inspect the update package: {error}"))?;
        if entry.is_dir() {
            continue;
        }

        let Some(enclosed) = entry.enclosed_name() else {
            continue;
        };
        if is_packaged_runtime(&enclosed, &version_name) {
            runtime_count += 1;
            if runtime_count > 1 {
                return Err("The update package contains multiple Wuddle runtimes.".to_string());
            }
            std::io::copy(&mut entry, runtime.as_file_mut())
                .map_err(|error| format!("Failed to stage the Wuddle runtime: {error}"))?;
        } else if is_packaged_launcher(&enclosed) {
            launcher_count += 1;
            if launcher_count > 1 {
                return Err("The update package contains multiple Wuddle launchers.".to_string());
            }
            std::io::copy(&mut entry, launcher.as_file_mut())
                .map_err(|error| format!("Failed to stage the Wuddle launcher: {error}"))?;
        }
    }

    if runtime_count != 1 {
        return Err("Wuddle-bin.exe was not found at the expected package location.".to_string());
    }
    if launcher_count != 1 {
        return Err("Wuddle.exe was not found at the expected package location.".to_string());
    }

    sync_and_validate_pe(&mut runtime, "runtime")?;
    sync_and_validate_pe(&mut launcher, "launcher")?;

    let pending_launcher = root.join(PENDING_LAUNCHER_NAME);
    persist_replacing(launcher, &pending_launcher, "launcher update")?;
    persist_replacing(
        runtime,
        &version_directory.join("Wuddle-bin.exe"),
        "Wuddle runtime",
    )?;
    write_current_pointer(root, &version_name)?;

    Ok(version_name)
}

#[cfg(any(target_os = "windows", test))]
fn is_packaged_runtime(path: &Path, version_name: &str) -> bool {
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    components.len() == 3
        && components[0].eq_ignore_ascii_case("versions")
        && components[1].eq_ignore_ascii_case(version_name)
        && components[2].eq_ignore_ascii_case("Wuddle-bin.exe")
}

#[cfg(any(target_os = "windows", test))]
fn is_packaged_launcher(path: &Path) -> bool {
    path.components().count() == 1
        && path
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("Wuddle.exe"))
}

#[cfg(any(target_os = "windows", test))]
fn sync_and_validate_pe(
    temporary: &mut tempfile::NamedTempFile,
    description: &str,
) -> Result<(), String> {
    temporary
        .as_file_mut()
        .flush()
        .map_err(|error| format!("Failed to flush the staged {description}: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("Failed to synchronize the staged {description}: {error}"))?;
    let map = FileMap::open(temporary.path())
        .map_err(|error| format!("Failed to inspect the staged {description}: {error}"))?;
    PeFile::from_bytes(&map)
        .map_err(|error| format!("The staged {description} is not a valid PE file: {error}"))?;
    Ok(())
}

#[cfg(any(target_os = "windows", test))]
fn persist_replacing(
    temporary: tempfile::NamedTempFile,
    target: &Path,
    description: &str,
) -> Result<(), String> {
    let rollback = target.with_file_name(format!(
        ".wuddle-replace-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let had_target = target.is_file();
    if had_target {
        fs::rename(target, &rollback)
            .map_err(|error| format!("Failed to prepare the existing {description}: {error}"))?;
    }

    match temporary.persist(target) {
        Ok(_) => {
            if had_target {
                let _ = fs::remove_file(rollback);
            }
            Ok(())
        }
        Err(error) => {
            if had_target {
                let _ = fs::rename(&rollback, target);
            }
            Err(format!(
                "Failed to commit the staged {description}: {}",
                error.error
            ))
        }
    }
}

#[cfg(any(target_os = "windows", test))]
fn write_current_pointer(root: &Path, version_name: &str) -> Result<(), String> {
    let mut temporary = tempfile::NamedTempFile::new_in(root)
        .map_err(|error| format!("Failed to stage current.json: {error}"))?;
    let value = serde_json::json!({ "current": version_name }).to_string();
    temporary
        .as_file_mut()
        .write_all(value.as_bytes())
        .map_err(|error| format!("Failed to write current.json: {error}"))?;
    temporary
        .as_file_mut()
        .flush()
        .map_err(|error| format!("Failed to flush current.json: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("Failed to synchronize current.json: {error}"))?;
    persist_replacing(temporary, &root.join("current.json"), "version pointer")
}

#[cfg(any(target_os = "windows", test))]
fn safe_current_version(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    let mut components = Path::new(trimmed).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) if trimmed != "." && trimmed != ".." => Some(trimmed),
        _ => None,
    }
}

#[cfg(any(target_os = "windows", test))]
fn current_runtime(root: &Path) -> Result<PathBuf, String> {
    let raw = fs::read_to_string(root.join("current.json"))
        .map_err(|error| format!("Failed to read current.json: {error}"))?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|error| format!("Failed to parse current.json: {error}"))?;
    let current = value
        .get("current")
        .and_then(serde_json::Value::as_str)
        .and_then(safe_current_version)
        .ok_or_else(|| "current.json does not contain a safe version directory.".to_string())?;
    let runtime = root.join("versions").join(current).join("Wuddle-bin.exe");
    if runtime.is_file() {
        Ok(runtime)
    } else {
        Err("The selected Wuddle runtime is missing.".to_string())
    }
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn restart_windows_portable(root: &Path) -> Result<(), String> {
    let launcher = root.join("Wuddle.exe");
    if !launcher.is_file() {
        return Err("Wuddle.exe is missing from the portable installation.".to_string());
    }

    let mut command = if root.join(PENDING_LAUNCHER_NAME).is_file() {
        let runtime = current_runtime(root)?;
        let mut command = Command::new(runtime);
        command.env(FINISH_LAUNCHER_UPDATE_ENV, "1");
        command
    } else {
        Command::new(&launcher)
    };
    command
        .current_dir(root)
        .env(
            crate::single_instance::RESTART_PARENT_PID_ENV,
            std::process::id().to_string(),
        )
        .spawn()
        .map_err(|error| format!("Failed to relaunch Wuddle: {error}"))?;
    Ok(())
}

/// Returns `None` during ordinary startup and a completed helper result when
/// the staged runtime was launched solely to replace `Wuddle.exe`.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn finish_launcher_update_if_requested() -> Option<Result<(), String>> {
    if std::env::var(FINISH_LAUNCHER_UPDATE_ENV).ok().as_deref() != Some("1") {
        return None;
    }
    std::env::remove_var(FINISH_LAUNCHER_UPDATE_ENV);
    crate::single_instance::wait_for_restart_parent();
    let result = finish_launcher_update();
    if let Err(error) = &result {
        if let Ok((root, _)) = detect_windows_launcher_root() {
            let _ = fs::write(root.join("WuddleLauncher-error.txt"), error.as_bytes());
        }
    }
    Some(result)
}

#[cfg(any(target_os = "windows", test))]
fn finish_launcher_update() -> Result<(), String> {
    let (root, _) = detect_windows_launcher_root()?;
    let live = root.join("Wuddle.exe");
    let pending = root.join(PENDING_LAUNCHER_NAME);
    let previous = root.join(PREVIOUS_LAUNCHER_NAME);
    if !live.is_file() && previous.is_file() {
        fs::rename(&previous, &live).map_err(|error| {
            format!("The previous launcher could not be restored before updating: {error}")
        })?;
    }
    if !live.is_file() {
        return Err("The launcher replacement helper could not find Wuddle.exe.".to_string());
    }
    validate_pe_path(&pending, "pending launcher")?;

    let deadline = Instant::now() + LAUNCHER_REPLACEMENT_TIMEOUT;
    let mut last_error = "the existing launcher is still in use".to_string();
    let replaced = loop {
        match try_replace_launcher(&live, &pending, &previous) {
            Ok(()) => break true,
            Err(error) => last_error = error,
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(LAUNCHER_REPLACEMENT_RETRY);
    };

    if !replaced {
        spawn_launcher(&live, &root)?;
        return Err(format!(
            "The launcher could not be updated safely; the existing launcher was restarted ({last_error})."
        ));
    }

    if let Err(error) = spawn_launcher(&live, &root) {
        let _ = fs::rename(&live, &pending);
        let _ = fs::rename(&previous, &live);
        let _ = spawn_launcher(&live, &root);
        return Err(format!(
            "The new launcher could not start and the previous launcher was restored: {error}"
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "windows", test))]
fn try_replace_launcher(live: &Path, pending: &Path, previous: &Path) -> Result<(), String> {
    if !live.is_file() || !pending.is_file() {
        return Err("a required launcher file is missing".to_string());
    }
    if previous.exists() {
        fs::remove_file(previous)
            .map_err(|error| format!("remove the previous launcher backup: {error}"))?;
    }
    fs::rename(live, previous)
        .map_err(|error| format!("release the existing launcher: {error}"))?;
    if let Err(error) = fs::rename(pending, live) {
        let _ = fs::rename(previous, live);
        return Err(format!("commit the pending launcher: {error}"));
    }
    Ok(())
}

#[cfg(any(target_os = "windows", test))]
fn spawn_launcher(launcher: &Path, root: &Path) -> Result<(), String> {
    Command::new(launcher)
        .current_dir(root)
        .env(
            crate::single_instance::RESTART_PARENT_PID_ENV,
            std::process::id().to_string(),
        )
        .spawn()
        .map_err(|error| format!("start Wuddle.exe: {error}"))?;
    Ok(())
}

#[cfg(any(target_os = "windows", test))]
fn validate_pe_path(path: &Path, description: &str) -> Result<(), String> {
    let map = FileMap::open(path)
        .map_err(|error| format!("Failed to inspect the {description}: {error}"))?;
    PeFile::from_bytes(&map)
        .map_err(|error| format!("The {description} is not a valid PE file: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_windows_version_name, is_packaged_launcher, is_packaged_runtime,
        safe_current_version, stage_windows_portable_update, validate_appimage_header,
    };
    #[cfg(target_os = "linux")]
    use super::{
        install_linux_appimage_update, previous_appimage_path, rollback_linux_appimage_update,
    };
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    fn fake_appimage(marker: u8) -> Vec<u8> {
        let mut bytes = vec![0_u8; 64];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[8..11].copy_from_slice(b"AI\x02");
        bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
        bytes[63] = marker;
        bytes
    }

    #[test]
    fn appimage_header_requires_type_two_x86_64_elf_identity() {
        let valid = fake_appimage(1);
        assert!(validate_appimage_header(&valid).is_ok());

        assert!(validate_appimage_header(&valid[..19]).is_err());
        let mut wrong_class = valid.clone();
        wrong_class[4] = 1;
        assert!(validate_appimage_header(&wrong_class).is_err());
        let mut wrong_endian = valid.clone();
        wrong_endian[5] = 2;
        assert!(validate_appimage_header(&wrong_endian).is_err());
        let mut wrong_magic = valid.clone();
        wrong_magic[8..11].copy_from_slice(b"BAD");
        assert!(validate_appimage_header(&wrong_magic).is_err());
        let mut wrong_architecture = valid;
        wrong_architecture[18..20].copy_from_slice(&183_u16.to_le_bytes());
        assert!(validate_appimage_header(&wrong_architecture).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_appimage_update_is_atomic_and_rotates_one_rollback() {
        let temp = tempfile::tempdir().unwrap();
        let live = temp.path().join("Wuddle.AppImage");
        let first_update = temp.path().join("first.AppImage");
        let second_update = temp.path().join("second.AppImage");
        let original = fake_appimage(1);
        let first = fake_appimage(2);
        let second = fake_appimage(3);
        fs::write(&live, &original).unwrap();
        fs::write(&first_update, &first).unwrap();
        fs::write(&second_update, &second).unwrap();

        install_linux_appimage_update(&live, &first_update).unwrap();
        let previous = previous_appimage_path(&live).unwrap();
        assert_eq!(fs::read(&live).unwrap(), first);
        assert_eq!(fs::read(&previous).unwrap(), original);

        install_linux_appimage_update(&live, &second_update).unwrap();
        assert_eq!(fs::read(&live).unwrap(), second);
        assert_eq!(fs::read(&previous).unwrap(), fake_appimage(2));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn invalid_appimage_never_changes_the_live_or_rollback_files() {
        let temp = tempfile::tempdir().unwrap();
        let live = temp.path().join("Wuddle.AppImage");
        let update = temp.path().join("update.AppImage");
        let original = fake_appimage(1);
        fs::write(&live, &original).unwrap();
        fs::write(&update, b"not an AppImage").unwrap();

        assert!(install_linux_appimage_update(&live, &update).is_err());
        assert_eq!(fs::read(&live).unwrap(), original);
        assert!(!previous_appimage_path(&live).unwrap().exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_appimage_update_never_replaces_a_linked_rollback_path() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let live = temp.path().join("Wuddle.AppImage");
        let update = temp.path().join("update.AppImage");
        let external = temp.path().join("external-backup-target");
        let original = fake_appimage(1);
        fs::write(&live, &original).unwrap();
        fs::write(&update, fake_appimage(2)).unwrap();
        fs::write(&external, b"do not replace").unwrap();
        let previous = previous_appimage_path(&live).unwrap();
        symlink(&external, &previous).unwrap();

        assert!(install_linux_appimage_update(&live, &update).is_err());
        assert_eq!(fs::read(&live).unwrap(), original);
        assert_eq!(fs::read(&external).unwrap(), b"do not replace");
        assert!(fs::symlink_metadata(previous)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_appimage_rollback_restores_the_valid_previous_version() {
        let temp = tempfile::tempdir().unwrap();
        let live = temp.path().join("Wuddle.AppImage");
        let update = temp.path().join("update.AppImage");
        let original = fake_appimage(1);
        fs::write(&live, &original).unwrap();
        fs::write(&update, fake_appimage(2)).unwrap();
        install_linux_appimage_update(&live, &update).unwrap();

        rollback_linux_appimage_update(&live).unwrap();

        assert_eq!(fs::read(&live).unwrap(), original);
        assert!(!previous_appimage_path(&live).unwrap().exists());
    }

    #[test]
    fn windows_version_names_use_one_release_tag_form() {
        assert_eq!(
            canonical_windows_version_name("3.7.0-beta.6"),
            "v3.7.0-beta.6"
        );
        assert_eq!(canonical_windows_version_name("v3.7.0"), "v3.7.0");
    }

    #[test]
    fn current_pointer_component_cannot_escape_versions() {
        assert_eq!(safe_current_version("v3.7.0"), Some("v3.7.0"));
        assert_eq!(safe_current_version("../v3.7.0"), None);
        assert_eq!(safe_current_version("nested/v3.7.0"), None);
        assert_eq!(safe_current_version("."), None);
        assert_eq!(safe_current_version(""), None);
    }

    #[test]
    fn portable_package_entries_require_the_exact_release_layout() {
        assert!(is_packaged_launcher(Path::new("Wuddle.exe")));
        assert!(!is_packaged_launcher(Path::new("nested/Wuddle.exe")));
        assert!(is_packaged_runtime(
            Path::new("versions/v3.7.0-beta.7/Wuddle-bin.exe"),
            "v3.7.0-beta.7"
        ));
        assert!(!is_packaged_runtime(
            Path::new("versions/other/Wuddle-bin.exe"),
            "v3.7.0-beta.7"
        ));
        assert!(!is_packaged_runtime(
            Path::new("nested/versions/v3.7.0-beta.7/Wuddle-bin.exe"),
            "v3.7.0-beta.7"
        ));
    }

    #[test]
    fn invalid_executables_never_advance_the_version_pointer() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("versions")).unwrap();
        fs::write(temp.path().join("Wuddle.exe"), b"existing launcher").unwrap();
        let package = temp.path().join("update.zip");
        let file = fs::File::create(&package).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        archive.start_file("Wuddle.exe", options).unwrap();
        archive.write_all(b"not a PE launcher").unwrap();
        archive
            .start_file("versions/v3.7.0-beta.7/Wuddle-bin.exe", options)
            .unwrap();
        archive.write_all(b"not a PE runtime").unwrap();
        archive.finish().unwrap();

        let error =
            stage_windows_portable_update(temp.path(), &package, "3.7.0-beta.7").unwrap_err();

        assert!(error.contains("not a valid PE file"));
        assert!(!temp.path().join("current.json").exists());
        assert!(!temp
            .path()
            .join("versions/v3.7.0-beta.7/Wuddle-bin.exe")
            .exists());
        assert!(!temp.path().join(".Wuddle-launcher-update.exe").exists());
        assert_eq!(
            fs::read(temp.path().join("Wuddle.exe")).unwrap(),
            b"existing launcher"
        );
    }
}
