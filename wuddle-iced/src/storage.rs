use std::path::PathBuf;
use std::sync::OnceLock;

static APP_DIR: OnceLock<Result<PathBuf, String>> = OnceLock::new();

/// Select and initialize Wuddle's one authoritative data directory.
///
/// Windows uses a self-contained directory beside the stable launcher. Other
/// platforms deliberately retain their existing standard/portable behavior.
#[cfg(target_os = "windows")]
pub fn initialize() -> Result<(), String> {
    app_dir().map(|_| ())
}

pub fn app_dir() -> Result<PathBuf, String> {
    APP_DIR.get_or_init(resolve_and_initialize).clone()
}

/// Resolve the directory that contains Wuddle's stable launcher or standalone
/// executable. User-visible recovery files belong here rather than inside the
/// application-data directory that a reset removes.
pub fn installation_root() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let executable = std::env::current_exe()
            .map_err(|error| format!("Could not locate the running Wuddle executable: {error}"))?;
        return windows::resolve_install_root(&executable);
    }
    #[cfg(not(target_os = "windows"))]
    {
        crate::settings::portable_root_dir()
    }
}

#[cfg(target_os = "windows")]
fn resolve_and_initialize() -> Result<PathBuf, String> {
    windows::resolve_and_initialize()
}

#[cfg(not(target_os = "windows"))]
fn resolve_and_initialize() -> Result<PathBuf, String> {
    let dir = if crate::settings::portable_mode_enabled() {
        crate::settings::portable_app_dir()?
    } else {
        crate::settings::standard_app_dir()?
    };
    std::fs::create_dir_all(&dir).map_err(|e| {
        format!(
            "Could not create Wuddle data directory {}: {e}",
            dir.display()
        )
    })?;
    Ok(dir)
}

/// Windows' initialization marker prevents deleted local files from causing a
/// later launch to resurrect settings from AppData. Linux keeps its existing
/// Tauri migration behavior unchanged.
pub fn allow_legacy_tauri_import() -> bool {
    !cfg!(target_os = "windows")
}

#[cfg(target_os = "windows")]
pub fn legacy_plaintext_token_paths() -> Result<Vec<PathBuf>, String> {
    windows::legacy_plaintext_token_paths()
}

#[cfg(not(target_os = "windows"))]
pub fn legacy_plaintext_token_paths() -> Result<Vec<PathBuf>, String> {
    let path = app_dir()?.join(".github_token");
    Ok(if path.is_file() {
        vec![path]
    } else {
        Vec::new()
    })
}

#[cfg(any(target_os = "windows", test))]
mod windows {
    use super::*;
    use rusqlite::{backup::Backup, Connection, OpenFlags};
    #[cfg(any(target_os = "windows", test))]
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const DATA_DIR_NAME: &str = "wuddle-data";
    const MARKER_SUFFIX: &str = "-storage-initialized";
    const STAGING_PREFIX: &str = ".wuddle-data-migration-";

    #[cfg(any(target_os = "windows", test))]
    #[cfg_attr(test, allow(dead_code))]
    pub(super) fn resolve_and_initialize() -> Result<PathBuf, String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Could not locate the running Wuddle executable: {e}"))?;
        let install_root = resolve_install_root(&exe)?;
        let current_dir = std::env::current_dir()
            .map_err(|e| format!("Could not resolve the current directory: {e}"))?;
        let data_dir = select_data_dir(
            &install_root,
            std::env::var_os("WUDDLE_DATA_DIR").map(PathBuf::from),
            &current_dir,
        );
        let marker = marker_path(&data_dir)?;
        let appdata = dirs::data_dir().map(|path| path.join("wuddle"));
        let portable_sources = legacy_portable_sources(&install_root, &data_dir)?;

        initialize_at(&data_dir, &marker, appdata.as_deref(), &portable_sources)?;
        wuddle_engine::set_default_app_dir(data_dir.clone()).map_err(|e| {
            format!(
                "Could not configure Wuddle's engine data directory as {}: {e}",
                data_dir.display()
            )
        })?;
        Ok(data_dir)
    }

    fn select_data_dir(
        install_root: &Path,
        override_dir: Option<PathBuf>,
        current_dir: &Path,
    ) -> PathBuf {
        let selected = override_dir
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| install_root.join(DATA_DIR_NAME));
        if selected.is_absolute() {
            selected
        } else {
            current_dir.join(selected)
        }
    }

    fn marker_path(data_dir: &Path) -> Result<PathBuf, String> {
        let parent = data_dir.parent().ok_or_else(|| {
            format!(
                "Wuddle data directory has no parent: {}",
                data_dir.display()
            )
        })?;
        let directory_name = data_dir
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(DATA_DIR_NAME);
        Ok(parent.join(format!(".{directory_name}{MARKER_SUFFIX}")))
    }

    pub(super) fn resolve_install_root(exe: &Path) -> Result<PathBuf, String> {
        let exe_dir = exe.parent().ok_or_else(|| {
            format!(
                "Wuddle executable has no parent directory: {}",
                exe.display()
            )
        })?;

        for candidate in exe_dir.ancestors() {
            if candidate.join("Wuddle.exe").is_file() && candidate.join("versions").is_dir() {
                return Ok(candidate.to_path_buf());
            }
        }

        // Standalone and development builds intentionally keep their data next
        // to the executable when no launcher layout exists.
        Ok(exe_dir.to_path_buf())
    }

    fn has_local_data(dir: &Path) -> Result<bool, String> {
        if !dir.exists() {
            return Ok(false);
        }
        let mut entries = fs::read_dir(dir)
            .map_err(|e| format!("Could not inspect local data {}: {e}", dir.display()))?;
        Ok(entries.next().is_some())
    }

    fn has_migratable_data(dir: &Path) -> Result<bool, String> {
        if !dir.is_dir() {
            return Ok(false);
        }
        let entries = fs::read_dir(dir)
            .map_err(|e| format!("Could not inspect migration source {}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| {
                format!("Could not inspect migration source {}: {e}", dir.display())
            })?;
            if entry.path().is_file() && is_migratable_name(&entry.file_name().to_string_lossy()) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn is_migratable_name(name: &str) -> bool {
        name.eq_ignore_ascii_case("settings.json")
            || (name.to_ascii_lowercase().starts_with("wuddle")
                && name.to_ascii_lowercase().ends_with(".sqlite"))
    }

    fn initialize_at(
        data_dir: &Path,
        marker: &Path,
        appdata_source: Option<&Path>,
        portable_sources: &[PathBuf],
    ) -> Result<(), String> {
        let parent = data_dir.parent().ok_or_else(|| {
            format!(
                "Wuddle data directory has no parent: {}",
                data_dir.display()
            )
        })?;
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Could not create the parent of Wuddle's data directory {}: {e}",
                parent.display()
            )
        })?;

        // Once initialized, AppData is never consulted again—even when a
        // settings file or database was deliberately removed.
        if marker.is_file() {
            fs::create_dir_all(data_dir).map_err(|e| {
                format!(
                    "Could not create Wuddle data directory {}: {e}",
                    data_dir.display()
                )
            })?;
            verify_writable(data_dir)?;
            return Ok(());
        }

        // A bundled/preconfigured local directory is authoritative as a whole.
        if has_local_data(data_dir)? {
            verify_writable(data_dir)?;
            write_marker(marker, data_dir)?;
            return Ok(());
        }

        let appdata = match appdata_source {
            Some(source) if has_migratable_data(source)? => Some(source.to_path_buf()),
            _ => None,
        };
        let source = if let Some(source) = appdata {
            Some(source)
        } else {
            let valid_sources = portable_sources
                .iter()
                .filter_map(|source| match has_migratable_data(source) {
                    Ok(true) => Some(Ok(source.clone())),
                    Ok(false) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            match valid_sources.as_slice() {
                [] => None,
                [only] => Some(only.clone()),
                many => {
                    let paths = many
                        .iter()
                        .map(|path| format!("  - {}", path.display()))
                        .collect::<Vec<_>>()
                        .join("\n");
                    return Err(format!(
                        "Several legacy portable data directories were found, so Wuddle cannot safely choose one:\n{paths}\nMove the data you want to keep into {} and try again.",
                        data_dir.display()
                    ));
                }
            }
        };

        if let Some(source) = source {
            migrate_directory(&source, data_dir)?;
        } else {
            fs::create_dir_all(data_dir).map_err(|e| {
                format!(
                    "Could not create Wuddle data directory {}: {e}",
                    data_dir.display()
                )
            })?;
        }

        verify_writable(data_dir)?;
        write_marker(marker, data_dir)
    }

    fn migrate_directory(source: &Path, data_dir: &Path) -> Result<(), String> {
        let parent = data_dir.parent().ok_or_else(|| {
            format!(
                "Wuddle data directory has no parent: {}",
                data_dir.display()
            )
        })?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let staging = parent.join(format!("{STAGING_PREFIX}{}-{nonce}", std::process::id()));
        fs::create_dir(&staging).map_err(|e| {
            format!(
                "Could not create migration staging directory {}: {e}",
                staging.display()
            )
        })?;

        let result = (|| {
            let entries = fs::read_dir(source).map_err(|e| {
                format!("Could not read migration source {}: {e}", source.display())
            })?;
            for entry in entries {
                let entry = entry.map_err(|e| {
                    format!("Could not read migration source {}: {e}", source.display())
                })?;
                let source_file = entry.path();
                if !source_file.is_file() {
                    continue;
                }
                let name = entry.file_name();
                let name_text = name.to_string_lossy();
                if !is_migratable_name(&name_text) {
                    continue;
                }
                let destination = staging.join(&name);
                if name_text.eq_ignore_ascii_case("settings.json") {
                    validate_and_copy_settings(&source_file, &destination)?;
                } else {
                    backup_sqlite(&source_file, &destination)?;
                }
            }

            // An empty destination directory may have been created by an
            // earlier attempt or by packaging. It cannot contain local data at
            // this point, so remove only that empty directory before rename.
            if data_dir.exists() {
                fs::remove_dir(data_dir).map_err(|e| {
                    format!(
                        "Could not replace empty Wuddle data directory {} during migration: {e}",
                        data_dir.display()
                    )
                })?;
            }
            fs::rename(&staging, data_dir).map_err(|e| {
                format!(
                    "Could not activate migrated Wuddle data at {}: {e}",
                    data_dir.display()
                )
            })?;
            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }

    fn validate_and_copy_settings(source: &Path, destination: &Path) -> Result<(), String> {
        let text = fs::read_to_string(source)
            .map_err(|e| format!("Could not read settings {}: {e}", source.display()))?;
        serde_json::from_str::<crate::settings::AppSettings>(&text).map_err(|e| {
            format!(
                "Settings file {} is not a valid Wuddle settings file: {e}",
                source.display()
            )
        })?;
        fs::write(destination, text)
            .map_err(|e| format!("Could not stage settings at {}: {e}", destination.display()))
    }

    fn backup_sqlite(source: &Path, destination: &Path) -> Result<(), String> {
        let source_db = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("Could not open database {}: {e}", source.display()))?;
        let mut destination_db = Connection::open(destination).map_err(|e| {
            format!(
                "Could not create migrated database {}: {e}",
                destination.display()
            )
        })?;
        {
            let backup = Backup::new(&source_db, &mut destination_db).map_err(|e| {
                format!(
                    "Could not start database backup for {}: {e}",
                    source.display()
                )
            })?;
            backup
                .run_to_completion(64, Duration::from_millis(5), None)
                .map_err(|e| format!("Could not back up database {}: {e}", source.display()))?;
        }
        let integrity: String = destination_db
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|e| format!("Could not validate database {}: {e}", source.display()))?;
        if !integrity.eq_ignore_ascii_case("ok") {
            return Err(format!(
                "Migrated database {} failed its integrity check: {integrity}",
                source.display()
            ));
        }
        Ok(())
    }

    fn verify_writable(data_dir: &Path) -> Result<(), String> {
        let probe = data_dir.join(format!(".wuddle-write-test-{}", std::process::id()));
        fs::write(&probe, b"writable").map_err(|e| {
            format!(
                "Wuddle's data directory is not writable: {} ({e})",
                data_dir.display()
            )
        })?;
        fs::remove_file(&probe).map_err(|e| {
            format!(
                "Could not finish the write test in {}: {e}",
                data_dir.display()
            )
        })
    }

    fn write_marker(marker: &Path, data_dir: &Path) -> Result<(), String> {
        let temporary = marker.with_extension(format!("tmp-{}", std::process::id()));
        let contents = format!(
            "version=1\ndata_dir={}\n",
            data_dir
                .canonicalize()
                .unwrap_or_else(|_| data_dir.to_path_buf())
                .display()
        );
        fs::write(&temporary, contents).map_err(|e| {
            format!(
                "Could not write storage marker {}: {e}",
                temporary.display()
            )
        })?;
        fs::rename(&temporary, marker).map_err(|e| {
            let _ = fs::remove_file(&temporary);
            format!(
                "Could not activate storage marker {}: {e}",
                marker.display()
            )
        })
    }

    fn legacy_portable_sources(
        install_root: &Path,
        authoritative_data_dir: &Path,
    ) -> Result<Vec<PathBuf>, String> {
        let versions = install_root.join("versions");
        if !versions.is_dir() {
            return Ok(Vec::new());
        }
        let entries = fs::read_dir(&versions).map_err(|e| {
            format!(
                "Could not inspect legacy Wuddle versions {}: {e}",
                versions.display()
            )
        })?;
        let mut sources = Vec::new();
        // Older portable-root detection treated version directory names that
        // started with "wuddle" as wrappers and placed data directly beneath
        // `versions/` rather than beneath one specific version.
        let shared_candidate = versions.join(DATA_DIR_NAME);
        if shared_candidate != authoritative_data_dir && shared_candidate.is_dir() {
            sources.push(shared_candidate.clone());
        }
        for entry in entries {
            let entry = entry.map_err(|e| {
                format!(
                    "Could not inspect legacy Wuddle versions {}: {e}",
                    versions.display()
                )
            })?;
            let entry_path = entry.path();
            if entry_path == shared_candidate {
                continue;
            }
            let candidate = entry_path.join(DATA_DIR_NAME);
            if candidate != authoritative_data_dir && candidate.is_dir() {
                sources.push(candidate);
            }
        }
        Ok(sources)
    }

    #[cfg(any(target_os = "windows", test))]
    #[cfg_attr(test, allow(dead_code))]
    pub(super) fn legacy_plaintext_token_paths() -> Result<Vec<PathBuf>, String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Could not locate the running Wuddle executable: {e}"))?;
        let install_root = resolve_install_root(&exe)?;
        let authoritative = super::app_dir()?;
        let mut dirs = vec![authoritative.clone()];
        dirs.extend(legacy_portable_sources(&install_root, &authoritative)?);

        let mut seen = HashSet::new();
        Ok(dirs
            .into_iter()
            .map(|dir| dir.join(".github_token"))
            .filter(|path| path.is_file())
            .filter(|path| seen.insert(path.clone()))
            .collect())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn temp_dir(name: &str) -> PathBuf {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("wuddle-storage-{name}-{nonce}"));
            fs::create_dir_all(&dir).unwrap();
            dir
        }

        #[test]
        fn launcher_root_is_found_above_version_binary() {
            let root = temp_dir("launcher-root");
            fs::write(root.join("Wuddle.exe"), b"").unwrap();
            fs::create_dir_all(root.join("versions/v3.6.0")).unwrap();
            let exe = root.join("versions/v3.6.0/Wuddle-bin.exe");
            fs::write(&exe, b"").unwrap();
            assert_eq!(resolve_install_root(&exe).unwrap(), root);
        }

        #[test]
        fn standalone_uses_executable_directory() {
            let root = temp_dir("standalone");
            let exe = root.join("Wuddle-bin.exe");
            fs::write(&exe, b"").unwrap();
            assert_eq!(resolve_install_root(&exe).unwrap(), root);
        }

        #[test]
        fn explicit_data_directory_overrides_install_root() {
            let root = temp_dir("override-root");
            let custom = root.join("writable/custom-data");
            assert_eq!(select_data_dir(&root, Some(custom.clone()), &root), custom);
            assert_eq!(
                select_data_dir(&root, Some(PathBuf::from("relative-data")), &root),
                root.join("relative-data")
            );
            assert_eq!(
                select_data_dir(&root, None, &root),
                root.join(DATA_DIR_NAME)
            );
        }

        #[test]
        fn launcher_versions_share_one_stable_data_directory() {
            let root = temp_dir("updates");
            fs::write(root.join("Wuddle.exe"), b"").unwrap();
            for version in ["v3.6.0", "v3.6.1"] {
                let version_dir = root.join("versions").join(version);
                fs::create_dir_all(&version_dir).unwrap();
                let exe = version_dir.join("Wuddle-bin.exe");
                fs::write(&exe, b"").unwrap();
                let resolved_root = resolve_install_root(&exe).unwrap();
                assert_eq!(
                    select_data_dir(&resolved_root, None, &root),
                    root.join(DATA_DIR_NAME)
                );
            }
        }

        #[test]
        fn legacy_portable_sources_are_found_beneath_versions() {
            let root = temp_dir("legacy-sources");
            let expected = root.join("versions/v3.5.0/wuddle-data");
            let shared = root.join("versions/wuddle-data");
            fs::create_dir_all(&expected).unwrap();
            fs::create_dir_all(&shared).unwrap();
            fs::create_dir_all(root.join("versions/v3.6.0")).unwrap();

            assert_eq!(
                legacy_portable_sources(&root, &root.join(DATA_DIR_NAME)).unwrap(),
                vec![shared, expected]
            );
        }

        #[test]
        fn appdata_settings_and_wal_database_migrate_without_touching_source() {
            let root = temp_dir("migration");
            let source = root.join("appdata");
            let data = root.join(DATA_DIR_NAME);
            let marker = marker_path(&data).unwrap();
            fs::create_dir_all(&source).unwrap();
            fs::write(source.join("settings.json"), r#"{"theme":"cata"}"#).unwrap();

            let db_path = source.join("wuddle-profile.sqlite");
            let db = Connection::open(&db_path).unwrap();
            db.pragma_update(None, "journal_mode", "WAL").unwrap();
            db.execute("CREATE TABLE values_table(value TEXT)", [])
                .unwrap();
            db.execute("INSERT INTO values_table VALUES ('from-wal')", [])
                .unwrap();

            initialize_at(&data, &marker, Some(&source), &[]).unwrap();
            assert!(marker.is_file());
            assert!(source.join("settings.json").is_file());
            assert!(source.join("wuddle-profile.sqlite").is_file());
            let migrated = Connection::open(data.join("wuddle-profile.sqlite")).unwrap();
            let value: String = migrated
                .query_row("SELECT value FROM values_table", [], |row| row.get(0))
                .unwrap();
            assert_eq!(value, "from-wal");
        }

        #[test]
        fn preconfigured_local_data_wins_without_appdata_merge() {
            let root = temp_dir("local-wins");
            let source = root.join("appdata");
            let data = root.join(DATA_DIR_NAME);
            let marker = marker_path(&data).unwrap();
            fs::create_dir_all(&source).unwrap();
            fs::create_dir_all(&data).unwrap();
            fs::write(source.join("settings.json"), r#"{"theme":"appdata"}"#).unwrap();
            fs::write(data.join("settings.json"), r#"{"theme":"local"}"#).unwrap();

            initialize_at(&data, &marker, Some(&source), &[]).unwrap();
            let local = fs::read_to_string(data.join("settings.json")).unwrap();
            assert!(local.contains("local"));
            assert!(!data.join("wuddle.sqlite").exists());
        }

        #[test]
        fn marker_prevents_appdata_from_returning_after_local_deletion() {
            let root = temp_dir("marker");
            let source = root.join("appdata");
            let data = root.join(DATA_DIR_NAME);
            let marker = marker_path(&data).unwrap();
            fs::create_dir_all(&source).unwrap();
            fs::write(source.join("settings.json"), r#"{"theme":"old"}"#).unwrap();
            initialize_at(&data, &marker, Some(&source), &[]).unwrap();
            fs::remove_file(data.join("settings.json")).unwrap();

            initialize_at(&data, &marker, Some(&source), &[]).unwrap();
            assert!(!data.join("settings.json").exists());
        }

        #[test]
        fn invalid_migration_never_activates_partial_data() {
            let root = temp_dir("invalid");
            let source = root.join("appdata");
            let data = root.join(DATA_DIR_NAME);
            let marker = marker_path(&data).unwrap();
            fs::create_dir_all(&source).unwrap();
            fs::write(source.join("settings.json"), b"not json").unwrap();
            assert!(initialize_at(&data, &marker, Some(&source), &[]).is_err());
            assert!(!data.exists());
            assert!(!marker.exists());
        }

        #[test]
        fn corrupt_database_never_activates_partial_data() {
            let root = temp_dir("corrupt-database");
            let source = root.join("appdata");
            let data = root.join(DATA_DIR_NAME);
            let marker = marker_path(&data).unwrap();
            fs::create_dir_all(&source).unwrap();
            fs::write(source.join("settings.json"), b"{}").unwrap();
            fs::write(source.join("wuddle.sqlite"), b"not a sqlite database").unwrap();

            assert!(initialize_at(&data, &marker, Some(&source), &[]).is_err());
            assert!(!data.exists());
            assert!(!marker.exists());
        }

        #[test]
        fn ambiguous_portable_sources_are_rejected() {
            let root = temp_dir("ambiguous");
            let data = root.join(DATA_DIR_NAME);
            let marker = marker_path(&data).unwrap();
            let first = root.join("versions/one/wuddle-data");
            let second = root.join("versions/two/wuddle-data");
            for source in [&first, &second] {
                fs::create_dir_all(source).unwrap();
                fs::write(source.join("settings.json"), b"{}").unwrap();
            }
            let error = initialize_at(&data, &marker, None, &[first, second]).unwrap_err();
            assert!(error.contains("Several legacy portable"));
            assert!(!data.exists());
        }
    }
}
