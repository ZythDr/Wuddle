#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use std::cmp::Ordering;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

#[derive(Debug)]
struct Candidate {
    version_name: String,
    exe_path: PathBuf,
    parsed: Vec<u64>,
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

    let target = resolve_target_binary(&launcher_dir, &launcher_exe)
        .ok_or_else(|| "No runnable Wuddle binary found. Expected versions/<version>/Wuddle-bin.exe".to_string())?;

    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let status = Command::new(&target)
        .args(args)
        .current_dir(&launcher_dir)
        .status()
        .map_err(|e| format!("start {:?}: {e}", target.file_name().unwrap_or_default()))?;

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
    if let Some(best) = sorted.into_iter().find(|c| !is_same_file(&c.exe_path, launcher_exe)) {
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
        let bin = path.join(app_binary_name());
        if !bin.is_file() {
            continue;
        }

        out.push(Candidate {
            version_name: name.clone(),
            exe_path: bin,
            parsed: parse_version(&name),
        });
    }

    out
}

fn preferred_from_current_pointer(launcher_dir: &Path, candidates: &[Candidate]) -> Option<PathBuf> {
    let pointer_path = launcher_dir.join("current.json");
    let text = fs::read_to_string(pointer_path).ok()?;
    let wanted = extract_current_value(&text)?;
    if wanted.is_empty() {
        return None;
    }

    candidates
        .iter()
        .find(|c| c.version_name.eq_ignore_ascii_case(wanted))
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

fn compare_candidates_desc(a: &Candidate, b: &Candidate) -> Ordering {
    let ver_order = compare_versions(&a.parsed, &b.parsed).reverse();
    if ver_order != Ordering::Equal {
        return ver_order;
    }
    b.version_name.to_lowercase().cmp(&a.version_name.to_lowercase())
}

fn compare_versions(a: &[u64], b: &[u64]) -> Ordering {
    let max = a.len().max(b.len());
    for i in 0..max {
        let av = *a.get(i).unwrap_or(&0);
        let bv = *b.get(i).unwrap_or(&0);
        match av.cmp(&bv) {
            Ordering::Equal => continue,
            non_eq => return non_eq,
        }
    }
    Ordering::Equal
}

fn parse_version(raw: &str) -> Vec<u64> {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    trimmed
        .split(|c: char| !(c.is_ascii_alphanumeric()))
        .filter(|segment| !segment.is_empty())
        .filter_map(|segment| segment.parse::<u64>().ok())
        .collect()
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

fn is_same_file(a: &Path, b: &Path) -> bool {
    let ac = fs::canonicalize(a).ok();
    let bc = fs::canonicalize(b).ok();
    match (ac, bc) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

#[cfg(target_os = "windows")]
fn app_binary_name() -> &'static str {
    "Wuddle-bin.exe"
}

#[cfg(not(target_os = "windows"))]
fn app_binary_name() -> &'static str {
    "wuddle-bin"
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
