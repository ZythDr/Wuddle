#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

use semver::Version;

#[derive(Debug)]
struct Candidate {
    version_name: String,
    exe_path: PathBuf,
    parsed: Option<Version>,
}

fn main() {
    if let Err(err) = run() {
        report_error(&err);
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let launcher_exe = env::current_exe().map_err(|e| format!("resolve launcher path: {e}"))?;
    let launcher_dir = launcher_exe
        .parent()
        .ok_or_else(|| "resolve launcher directory".to_string())?
        .to_path_buf();

    let target = resolve_target_binary(&launcher_dir, &launcher_exe).ok_or_else(|| {
        "No runnable Wuddle binary found. Expected versions/<version>/Wuddle-bin.exe".to_string()
    })?;

    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let status = Command::new(&target)
        .args(args)
        .current_dir(&launcher_dir)
        .status()
        .map_err(|e| format!("start {:?}: {e}", target.file_name().unwrap_or_default()))?;

    // A successful, normally completed run confirms that the selected runtime
    // was usable. Only then is it safe to retain that runtime plus one older
    // rollback candidate. If an update changed current.json while this process
    // was waiting, pruning is deliberately skipped.
    if status.success() {
        let _ = prune_confirmed_old_versions(&launcher_dir, &target);
    }

    process::exit(status.code().unwrap_or(0));
}

fn resolve_target_binary(launcher_dir: &Path, launcher_exe: &Path) -> Option<PathBuf> {
    let candidates = collect_candidates(launcher_dir);

    if let Some(preferred) = preferred_from_current_pointer(launcher_dir, &candidates) {
        if !is_same_file(&preferred, launcher_exe) {
            return Some(preferred);
        }
    }

    let mut sorted = candidates;
    sorted.sort_by(compare_candidates_desc);
    if let Some(best) = sorted
        .into_iter()
        .find(|c| !is_same_file(&c.exe_path, launcher_exe))
    {
        return Some(best.exe_path);
    }

    fallback_legacy_binary(launcher_dir, launcher_exe)
}

fn collect_candidates(launcher_dir: &Path) -> Vec<Candidate> {
    let versions_dir = launcher_dir.join("versions");
    let mut out = Vec::new();
    let entries = match fs::read_dir(&versions_dir) {
        Ok(v) => v,
        Err(_) => return out,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let bin = app_binary_names()
            .iter()
            .map(|n| path.join(n))
            .find(|p| p.is_file());
        let Some(bin) = bin else { continue };

        out.push(Candidate {
            version_name: name.clone(),
            exe_path: bin,
            parsed: parse_version(&name),
        });
    }

    out
}

fn preferred_from_current_pointer(
    launcher_dir: &Path,
    candidates: &[Candidate],
) -> Option<PathBuf> {
    let pointer_path = launcher_dir.join("current.json");
    let text = fs::read_to_string(pointer_path).ok()?;
    let wanted = extract_current_value(&text)?;
    if wanted.is_empty() {
        return None;
    }

    candidates
        .iter()
        .find(|c| {
            c.version_name.eq_ignore_ascii_case(wanted)
                || canonical_version_name(&c.version_name)
                    .eq_ignore_ascii_case(&canonical_version_name(wanted))
        })
        .map(|c| c.exe_path.clone())
}

fn extract_current_value(raw: &str) -> Option<&str> {
    let needle = "\"current\"";
    let key_pos = raw.find(needle)?;
    let after_key = &raw[key_pos + needle.len()..];
    let colon_pos = after_key.find(':')?;
    let after_colon = after_key[colon_pos + 1..].trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    let body = &after_colon[1..];
    let end_quote = body.find('"')?;
    Some(body[..end_quote].trim())
}

fn compare_candidates_desc(a: &Candidate, b: &Candidate) -> std::cmp::Ordering {
    match (&a.parsed, &b.parsed) {
        (Some(a_version), Some(b_version)) => b_version.cmp(a_version).then_with(|| {
            b.version_name
                .to_lowercase()
                .cmp(&a.version_name.to_lowercase())
        }),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b
            .version_name
            .to_lowercase()
            .cmp(&a.version_name.to_lowercase()),
    }
}

fn canonical_version_name(raw: &str) -> String {
    raw.trim().trim_start_matches(['v', 'V']).trim().to_string()
}

fn parse_version(raw: &str) -> Option<Version> {
    Version::parse(&canonical_version_name(raw)).ok()
}

fn fallback_legacy_binary(launcher_dir: &Path, launcher_exe: &Path) -> Option<PathBuf> {
    let names = legacy_binary_names();
    for name in names {
        let path = launcher_dir.join(name);
        if path.is_file() && !is_same_file(&path, launcher_exe) {
            return Some(path);
        }
    }
    None
}

fn prune_confirmed_old_versions(
    launcher_dir: &Path,
    selected_target: &Path,
) -> Result<Vec<PathBuf>, String> {
    let candidates = collect_candidates(launcher_dir);
    let Some(pointer_target) = preferred_from_current_pointer(launcher_dir, &candidates) else {
        return Ok(Vec::new());
    };
    if !is_same_file(&pointer_target, selected_target) {
        return Ok(Vec::new());
    }

    let Some(selected) = candidates
        .iter()
        .find(|candidate| is_same_file(&candidate.exe_path, selected_target))
    else {
        return Ok(Vec::new());
    };
    let Some(selected_version) = selected.parsed.clone() else {
        return Ok(Vec::new());
    };

    // A future/higher runtime may be a staged update or a deliberately retained
    // failed candidate. Never delete it merely because the pointer was rolled
    // back. Of the lower versions, keep the newest one as the rollback runtime.
    let rollback = candidates
        .iter()
        .filter(|candidate| {
            candidate
                .parsed
                .as_ref()
                .is_some_and(|version| version < &selected_version)
        })
        .max_by(|a, b| a.parsed.cmp(&b.parsed))
        .map(|candidate| candidate.version_name.clone());

    let versions_dir = launcher_dir.join("versions");
    let mut removed = Vec::new();
    for candidate in candidates {
        let Some(version) = candidate.parsed.as_ref() else {
            continue;
        };
        if version >= &selected_version
            || rollback
                .as_deref()
                .is_some_and(|name| name == candidate.version_name)
        {
            continue;
        }

        let version_dir = versions_dir.join(&candidate.version_name);
        let metadata = fs::symlink_metadata(&version_dir)
            .map_err(|error| format!("inspect old runtime: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        fs::remove_dir_all(&version_dir).map_err(|error| format!("remove old runtime: {error}"))?;
        removed.push(version_dir);
    }
    Ok(removed)
}

fn is_same_file(a: &Path, b: &Path) -> bool {
    let ac = fs::canonicalize(a).ok();
    let bc = fs::canonicalize(b).ok();
    match (ac, bc) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// Candidate binary names inside a version folder, in preference order.
#[cfg(target_os = "windows")]
fn app_binary_names() -> &'static [&'static str] {
    &["Wuddle-bin.exe", "wuddle.exe"]
}

#[cfg(not(target_os = "windows"))]
fn app_binary_names() -> &'static [&'static str] {
    &["wuddle-bin", "wuddle"]
}

#[cfg(target_os = "windows")]
fn legacy_binary_names() -> &'static [&'static str] {
    &["wuddle-gui.exe", "Wuddle.exe"]
}

#[cfg(not(target_os = "windows"))]
fn legacy_binary_names() -> &'static [&'static str] {
    &["wuddle-gui", "wuddle"]
}

fn report_error(msg: &str) {
    eprintln!("wuddle-launcher error: {msg}");
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let path = dir.join("WuddleLauncher-error.txt");
            let _ = fs::write(path, msg.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "wuddle-launcher-{label}-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn install_candidate(root: &Path, version: &str) -> PathBuf {
        let directory = root.join("versions").join(version);
        fs::create_dir_all(&directory).unwrap();
        let binary = directory.join(app_binary_names()[0]);
        fs::write(&binary, b"test runtime").unwrap();
        binary
    }

    fn candidate(name: &str) -> Candidate {
        Candidate {
            version_name: name.to_string(),
            exe_path: PathBuf::from(name).join("Wuddle-bin.exe"),
            parsed: parse_version(name),
        }
    }

    #[test]
    fn stable_release_sorts_after_same_core_beta() {
        let mut candidates = [candidate("v3.7.0-beta.6"), candidate("v3.7.0")];
        candidates.sort_by(compare_candidates_desc);

        assert_eq!(candidates[0].version_name, "v3.7.0");
    }

    #[test]
    fn beta_identifiers_follow_semver_precedence() {
        let mut candidates = [
            candidate("v3.7.0-beta.2"),
            candidate("v3.7.0-beta.10"),
            candidate("v3.7.0-beta.3"),
        ];
        candidates.sort_by(compare_candidates_desc);

        assert_eq!(candidates[0].version_name, "v3.7.0-beta.10");
        assert_eq!(candidates[1].version_name, "v3.7.0-beta.3");
    }

    #[test]
    fn current_pointer_accepts_legacy_v_prefix_mismatch() {
        let temp = temp_root("pointer-prefix");
        fs::write(temp.join("current.json"), r#"{"current":"v3.7.0"}"#).unwrap();
        let candidates = vec![candidate("3.7.0")];

        let selected = preferred_from_current_pointer(&temp, &candidates).unwrap();

        assert_eq!(selected, PathBuf::from("3.7.0").join("Wuddle-bin.exe"));
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn invalid_directory_names_never_outrank_versions() {
        let mut candidates = [candidate("future"), candidate("v3.7.0-beta.1")];
        candidates.sort_by(compare_candidates_desc);

        assert_eq!(candidates[0].version_name, "v3.7.0-beta.1");
    }

    #[test]
    fn valid_current_pointer_wins_over_a_newer_fallback_candidate() {
        let temp = temp_root("pointer-wins");
        let selected = install_candidate(&temp, "v3.6.0");
        install_candidate(&temp, "v3.7.0");
        fs::write(temp.join("current.json"), r#"{"current":"v3.6.0"}"#).unwrap();
        let launcher = temp.join("Wuddle.exe");

        assert_eq!(resolve_target_binary(&temp, &launcher).unwrap(), selected);
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn malformed_pointer_falls_back_to_highest_semver_runtime() {
        let temp = temp_root("malformed-pointer");
        install_candidate(&temp, "v3.7.0-beta.9");
        let stable = install_candidate(&temp, "v3.7.0");
        fs::write(temp.join("current.json"), r#"{"current":42}"#).unwrap();
        let launcher = temp.join("Wuddle.exe");

        assert_eq!(resolve_target_binary(&temp, &launcher).unwrap(), stable);
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn directories_without_a_runtime_are_ignored() {
        let temp = temp_root("missing-runtime");
        fs::create_dir_all(temp.join("versions").join("v9.0.0")).unwrap();
        let usable = install_candidate(&temp, "v3.7.0");
        let launcher = temp.join("Wuddle.exe");

        assert_eq!(resolve_target_binary(&temp, &launcher).unwrap(), usable);
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn legacy_binary_is_used_only_when_no_versioned_runtime_exists() {
        let temp = temp_root("legacy-fallback");
        let legacy = temp.join(legacy_binary_names()[0]);
        fs::write(&legacy, b"legacy runtime").unwrap();
        let launcher = temp.join("launcher-under-test.exe");

        assert_eq!(resolve_target_binary(&temp, &launcher).unwrap(), legacy);
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn launcher_never_recursively_selects_itself_as_a_legacy_runtime() {
        let temp = temp_root("no-recursion");
        let launcher = temp.join(legacy_binary_names()[0]);
        fs::write(&launcher, b"launcher").unwrap();

        assert!(resolve_target_binary(&temp, &launcher).is_none());
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn successful_current_runtime_pruning_keeps_one_rollback() {
        let temp = temp_root("prune");
        let selected = install_candidate(&temp, "v3.7.0");
        install_candidate(&temp, "v3.6.0");
        install_candidate(&temp, "v3.5.0");
        install_candidate(&temp, "local-build");
        fs::write(temp.join("current.json"), r#"{"current":"v3.7.0"}"#).unwrap();

        let removed = prune_confirmed_old_versions(&temp, &selected).unwrap();

        assert_eq!(removed.len(), 1);
        assert!(!temp.join("versions/v3.5.0").exists());
        assert!(temp.join("versions/v3.6.0").exists());
        assert!(temp.join("versions/v3.7.0").exists());
        assert!(temp.join("versions/local-build").exists());
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn pruning_is_skipped_when_the_pointer_changed_during_the_run() {
        let temp = temp_root("changed-pointer");
        let old = install_candidate(&temp, "v3.6.0");
        install_candidate(&temp, "v3.5.0");
        install_candidate(&temp, "v3.7.0");
        fs::write(temp.join("current.json"), r#"{"current":"v3.7.0"}"#).unwrap();

        let removed = prune_confirmed_old_versions(&temp, &old).unwrap();

        assert!(removed.is_empty());
        assert!(temp.join("versions/v3.5.0").exists());
        fs::remove_dir_all(temp).unwrap();
    }
}
