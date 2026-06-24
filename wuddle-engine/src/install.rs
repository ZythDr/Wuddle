use anyhow::{Context, Result};
use git2::{Repository, Tree};
use std::{
    collections::HashMap,
    fs, io,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

#[cfg(windows)]
fn is_reparse_dir(meta: &fs::Metadata) -> bool {
    meta.is_dir() && (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0
}

#[cfg(windows)]
fn remove_windows_dir_link(path: &Path) -> Result<()> {
    if junction::delete(path).is_ok() {
        return Ok(());
    }
    if fs::remove_dir(path).is_ok() {
        return Ok(());
    }
    let status = Command::new("cmd")
        .args(["/C", "rmdir"])
        .arg(path)
        .status()
        .with_context(|| format!("spawn rmdir {:?}", path))?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("remove dir link {:?}: rmdir exited with {}", path, status)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct InstallOptions {
    pub use_symlinks: bool,
    pub set_xattr_comment: bool,
    pub replace_addon_conflicts: bool,
    /// Number of cached release versions to retain per repo (0 = only current).
    pub cache_keep_versions: usize,
}

#[derive(Debug, Clone)]
pub struct InstallRecord {
    pub path: PathBuf,
    pub kind: &'static str, // "dll" | "addon" | "raw"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveKind {
    Zip,
    SevenZip,
}

/// Install from a downloaded archive into the WoW directory.
///
/// Modes:
/// - addon: copy addon folders into Interface/AddOns
/// - dll: copy *.dll into WoW root
/// - mixed: both
/// - raw: currently unused for archives
pub fn install_from_archive(
    archive_path: &Path,
    extract_dir: &Path,
    wow_dir: &Path,
    mode: &str,
    opts: InstallOptions,
    comment: &str,
) -> Result<Vec<InstallRecord>> {
    let want_addon = mode == "addon" || mode == "mixed" || mode == "auto";
    let want_dll = mode == "dll" || mode == "mixed" || mode == "auto";

    let wow_root = wow_dir;
    fs::create_dir_all(wow_dir.join("Interface").join("AddOns"))
        .context("create Interface/AddOns")?;

    extract_archive(archive_path, extract_dir).context("extract archive")?;

    let mut records = Vec::new();

    if want_dll {
        let mut installed_dlls: Vec<String> = Vec::new();
        let mut handled_vfpatcher = false;
        if let (Some(vf_exe_src), Some(vf_patcher_src)) = (
            find_first_file_by_name(extract_dir, "VanillaFixes.exe"),
            find_first_file_by_name(extract_dir, "VfPatcher.dll"),
        ) {
            let vf_exe_dst = wow_root.join("VanillaFixes.exe");
            install_file_or_symlink(&vf_exe_src, &vf_exe_dst, opts.use_symlinks)?;
            maybe_set_comment(&vf_exe_dst, comment, opts.set_xattr_comment);
            records.push(InstallRecord {
                path: vf_exe_dst,
                kind: "raw",
            });

            let vf_patcher_dst = wow_root.join("VfPatcher.dll");
            install_file_or_symlink(&vf_patcher_src, &vf_patcher_dst, opts.use_symlinks)?;
            maybe_set_comment(&vf_patcher_dst, comment, opts.set_xattr_comment);
            installed_dlls.push("VfPatcher.dll".to_string());
            records.push(InstallRecord {
                path: vf_patcher_dst,
                kind: "dll",
            });
            handled_vfpatcher = true;

            let dlls_txt_dst = wow_root.join("dlls.txt");
            if !dlls_txt_dst.exists() {
                if let Some(dlls_txt_src) = find_first_file_by_name(extract_dir, "dlls.txt") {
                    install_file_or_symlink(&dlls_txt_src, &dlls_txt_dst, opts.use_symlinks)?;
                    maybe_set_comment(&dlls_txt_dst, comment, opts.set_xattr_comment);
                    records.push(InstallRecord {
                        path: dlls_txt_dst,
                        kind: "raw",
                    });
                }
            }
        }

        for dll in select_dlls_for_install(extract_dir, detect_dlls(extract_dir)) {
            if let Some(fname) = dll.file_name().and_then(|s| s.to_str()) {
                if handled_vfpatcher && fname.eq_ignore_ascii_case("VfPatcher.dll") {
                    continue;
                }
                let dst = wow_root.join(fname);
                install_file_or_symlink(&dll, &dst, opts.use_symlinks)?;
                maybe_set_comment(&dst, comment, opts.set_xattr_comment);
                installed_dlls.push(fname.to_string());
                records.push(InstallRecord {
                    path: dst,
                    kind: "dll",
                });
            }
        }
        update_dlls_txt(wow_root, comment, &installed_dlls)?;
    }

    if want_addon {
        for (src_dir, addon_folder_name) in detect_addons(extract_dir) {
            let rec = install_addon_folder(&src_dir, wow_dir, &addon_folder_name, opts, comment)?;
            records.push(rec);
        }
    }

    Ok(records)
}

/// Backwards-compatible wrapper for callers that still name the ZIP-specific API.
#[allow(dead_code)]
pub fn install_from_zip(
    zip_path: &Path,
    extract_dir: &Path,
    wow_dir: &Path,
    mode: &str,
    opts: InstallOptions,
    comment: &str,
) -> Result<Vec<InstallRecord>> {
    install_from_archive(zip_path, extract_dir, wow_dir, mode, opts, comment)
}

fn find_first_file_by_name(root: &Path, want: &str) -> Option<PathBuf> {
    let mut matches = Vec::<PathBuf>::new();
    walk_dir(root, &mut |p| {
        if !p.is_file() {
            return;
        }
        if p.file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case(want))
            .unwrap_or(false)
        {
            matches.push(p.to_path_buf());
        }
    });

    matches.sort_by_key(|p| p.components().count());
    matches.into_iter().next()
}

fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir).with_context(|| format!("cleanup {:?}", dest_dir))?;
    }
    fs::create_dir_all(dest_dir).with_context(|| format!("mkdir {:?}", dest_dir))?;

    match detect_archive_kind(archive_path)? {
        ArchiveKind::Zip => unzip(archive_path, dest_dir),
        ArchiveKind::SevenZip => unseven(archive_path, dest_dir),
    }
}

fn detect_archive_kind(archive_path: &Path) -> Result<ArchiveKind> {
    let mut file =
        fs::File::open(archive_path).with_context(|| format!("open archive {:?}", archive_path))?;
    let mut head = [0u8; 6];
    let n = file.read(&mut head)?;
    let slice = &head[..n];

    if slice.starts_with(b"PK\x03\x04")
        || slice.starts_with(b"PK\x05\x06")
        || slice.starts_with(b"PK\x07\x08")
    {
        return Ok(ArchiveKind::Zip);
    }
    if slice.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Ok(ArchiveKind::SevenZip);
    }

    anyhow::bail!("unsupported archive signature: {:?}", archive_path)
}

/// Unzip ZIP file into destination directory.
fn unzip(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = fs::File::open(zip_path).with_context(|| format!("open zip {:?}", zip_path))?;
    let mut archive = zip::ZipArchive::new(file).context("read zip")?;

    for i in 0..archive.len() {
        let mut f = archive.by_index(i).context("zip entry")?;
        let Some(entry_path) = f.enclosed_name() else { continue; };
        let outpath = dest_dir.join(entry_path);

        if f.is_dir() {
            fs::create_dir_all(&outpath).with_context(|| format!("mkdir {:?}", outpath))?;
            continue;
        }

        if let Some(parent) = outpath.parent() {
            fs::create_dir_all(parent).with_context(|| format!("mkdir {:?}", parent))?;
        }

        let mut outfile =
            fs::File::create(&outpath).with_context(|| format!("create {:?}", outpath))?;
        io::copy(&mut f, &mut outfile).context("extract file")?;
    }

    Ok(())
}

fn unseven(seven_path: &Path, dest_dir: &Path) -> Result<()> {
    let mut rejected = Vec::<String>::new();

    sevenz_rust::decompress_file_with_extract_fn(seven_path, dest_dir, |entry, reader, dest| {
        if entry.name().trim().is_empty() && entry.is_directory() {
            return Ok(false);
        }
        if safe_archive_path(entry.name()).is_none() {
            rejected.push(entry.name().to_string());
            return Ok(false);
        }
        sevenz_rust::default_entry_extract_fn(entry, reader, dest)
    })
    .with_context(|| format!("read 7z {:?}", seven_path))?;

    if !rejected.is_empty() {
        anyhow::bail!(
            "7z archive contains unsafe path(s): {}",
            rejected.join(", ")
        );
    }

    Ok(())
}

fn safe_archive_path(name: &str) -> Option<PathBuf> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.contains('\\') || is_windows_drive_path(trimmed) {
        return None;
    }

    let path = Path::new(trimmed);
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    if safe.as_os_str().is_empty() {
        None
    } else {
        Some(safe)
    }
}

fn is_windows_drive_path(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {:?}", parent))?;
    }
    fs::copy(src, dst).with_context(|| format!("copy {:?} -> {:?}", src, dst))?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("mkdir {:?}", dst))?;
    for entry in fs::read_dir(src).with_context(|| format!("read_dir {:?}", src))? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            copy_file(&path, &target)?;
        }
    }
    Ok(())
}

fn remove_any_target(path: &Path) -> Result<()> {
    if !path.exists() {
        if !path.is_symlink() {
            return Ok(());
        }
    }
    if let Ok(meta) = fs::symlink_metadata(path) {
        let ft = meta.file_type();
        if ft.is_symlink() {
            #[cfg(windows)]
            {
                if path.is_dir() {
                    remove_windows_dir_link(path)
                        .with_context(|| format!("remove dir symlink {:?}", path))?;
                } else {
                    fs::remove_file(path).with_context(|| format!("remove symlink {:?}", path))?;
                }
            }
            #[cfg(not(windows))]
            {
                fs::remove_file(path).with_context(|| format!("remove symlink {:?}", path))?;
            }
            return Ok(());
        }
        if ft.is_dir() {
            #[cfg(windows)]
            {
                if is_reparse_dir(&meta) {
                    remove_windows_dir_link(path)
                        .with_context(|| format!("remove junction {:?}", path))?;
                    return Ok(());
                }
                if fs::remove_dir(path).is_ok() {
                    return Ok(());
                }
            }
            fs::remove_dir_all(path).with_context(|| format!("remove dir {:?}", path))?;
            return Ok(());
        }
        fs::remove_file(path).with_context(|| format!("remove file {:?}", path))?;
    }
    Ok(())
}

#[cfg(unix)]
fn symlink_path(src: &Path, dst: &Path) -> Result<()> {
    std::os::unix::fs::symlink(src, dst)
        .with_context(|| format!("symlink {:?} -> {:?}", src, dst))?;
    Ok(())
}

#[cfg(windows)]
fn symlink_path(src: &Path, dst: &Path) -> Result<()> {
    if src.is_dir() {
        std::os::windows::fs::symlink_dir(src, dst)
            .with_context(|| format!("symlink dir {:?} -> {:?}", src, dst))?;
    } else {
        std::os::windows::fs::symlink_file(src, dst)
            .with_context(|| format!("symlink file {:?} -> {:?}", src, dst))?;
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn symlink_path(_src: &Path, _dst: &Path) -> Result<()> {
    anyhow::bail!("symlinks are not supported on this platform")
}

fn install_file_or_symlink(src: &Path, dst: &Path, use_symlink: bool) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {:?}", parent))?;
    }
    remove_any_target(dst)?;

    if use_symlink {
        if symlink_path(src, dst).is_ok() {
            return Ok(());
        }
    }

    copy_file(src, dst)
}

fn install_dir_or_symlink(src_dir: &Path, dst_dir: &Path, use_symlink: bool) -> Result<()> {
    if let Some(parent) = dst_dir.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {:?}", parent))?;
    }
    remove_any_target(dst_dir)?;

    if use_symlink {
        if symlink_path(src_dir, dst_dir).is_ok() {
            return Ok(());
        }
    }

    copy_dir_recursive(src_dir, dst_dir)
}

fn maybe_set_comment(path: &Path, comment: &str, enabled: bool) {
    if !enabled {
        return;
    }
    #[cfg(unix)]
    {
        let setfattr_ok = Command::new("setfattr")
            .args(["-n", "user.xdg.comment", "-v", comment])
            .arg(path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if setfattr_ok {
            return;
        }
        let _ = Command::new("xattr")
            .args(["-w", "user.xdg.comment", comment])
            .arg(path)
            .status();
    }
}

/// Write `dll_names` into dlls.txt, grouped under `# == repo_name ==` / `# == /repo_name ==`
/// block markers when more than one DLL is being registered.
///
/// Rules:
/// - If the DLL already exists in dlls.txt (commented or not), its enabled/disabled state
///   (`#` prefix) is preserved — we only update the normalised filename casing.
/// - If the DLL is new, it is appended inside the block (or as a bare line for single-DLL mods).
/// - Block markers are only written when `dll_names.len() > 1`.
pub(crate) fn update_dlls_txt(wow_dir: &Path, repo_name: &str, dll_names: &[String]) -> Result<()> {
    if dll_names.is_empty() {
        return Ok(());
    }

    let path = wow_dir.join("dlls.txt");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();

    let multi = dll_names.len() > 1;
    let block_start = format!("# == {} ==", repo_name);
    let block_end   = format!("# == /{} ==", repo_name);

    // Track which DLLs still need inserting after scanning existing lines.
    let mut needs_insert: Vec<&String> = dll_names.iter().collect();

    // Pass 1 — update any existing lines for these DLLs, preserving their # state.
    for line in lines.iter_mut() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (commented, rest) = if let Some(s) = trimmed.strip_prefix('#') {
            (true, s.trim())
        } else {
            (false, trimmed)
        };
        // Skip marker lines.
        if rest.starts_with("== ") {
            continue;
        }
        if let Some(pos) = needs_insert.iter().position(|d| d.eq_ignore_ascii_case(rest)) {
            let dll = needs_insert.remove(pos);
            // Preserve enabled/disabled state; only normalise casing.
            *line = if commented {
                format!("# {}", dll)
            } else {
                dll.clone()
            };
        }
    }

    // Pass 2 — insert any DLLs not yet present.
    if !needs_insert.is_empty() {
        if multi {
            // Find existing block end marker and insert before it, or append a new block.
            if let Some(end_pos) = lines.iter().position(|l| {
                l.trim().eq_ignore_ascii_case(block_end.trim())
            }) {
                for dll in needs_insert.iter().rev() {
                    lines.insert(end_pos, (*dll).clone());
                }
            } else {
                // No block yet — append start marker, DLLs, end marker.
                if !lines.last().map(|l| l.trim().is_empty()).unwrap_or(true) {
                    lines.push(String::new());
                }
                lines.push(block_start.clone());
                for dll in needs_insert.iter() {
                    lines.push((*dll).clone());
                }
                lines.push(block_end.clone());
            }
        } else {
            for dll in needs_insert.iter() {
                lines.push((*dll).clone());
            }
        }
    }

    // Ensure block markers exist for multi-DLL repos even if all DLLs were already tracked.
    if multi && !existing.contains(&block_start) {
        // Find the first of our DLL lines and wrap the group.
        // (They may be scattered; just prepend/append markers around the last known position.)
        if let Some(first_pos) = lines.iter().position(|l| {
            let trimmed = l.trim();
            let rest = trimmed.strip_prefix('#').map(|s| s.trim()).unwrap_or(trimmed);
            dll_names.iter().any(|d| d.eq_ignore_ascii_case(rest))
        }) {
            lines.insert(first_pos, block_start);
            // Find new last position after insertion.
            let last_pos = lines.iter().rposition(|l| {
                let trimmed = l.trim();
                let rest = trimmed.strip_prefix('#').map(|s| s.trim()).unwrap_or(trimmed);
                dll_names.iter().any(|d| d.eq_ignore_ascii_case(rest))
            }).unwrap_or(first_pos);
            lines.insert(last_pos + 1, block_end);
        }
    }

    let mut out = lines.join("\n");
    out.push('\n');
    fs::write(&path, out).with_context(|| format!("write {:?}", path))?;
    Ok(())
}

fn has_filename(path: &Path, name: &str) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case(name))
        .unwrap_or(false)
}

fn rel_has_component(root: &Path, path: &Path, want: &str) -> bool {
    path.strip_prefix(root)
        .ok()
        .map(|rel| {
            rel.components().any(|c| {
                c.as_os_str()
                    .to_str()
                    .map(|s| s.eq_ignore_ascii_case(want))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn select_dlls_for_install(root: &Path, dlls: Vec<PathBuf>) -> Vec<PathBuf> {
    if dlls.is_empty() {
        return dlls;
    }

    // DXVK archives bundle many x32/x64 DLLs, but for vanilla WoW we only want x32/d3d9.dll.
    let has_dxgi_x32 = dlls
        .iter()
        .any(|p| has_filename(p, "dxgi.dll") && rel_has_component(root, p, "x32"));
    if has_dxgi_x32 {
        if let Some(d3d9_x32) = dlls
            .iter()
            .find(|p| has_filename(p, "d3d9.dll") && rel_has_component(root, p, "x32"))
            .cloned()
        {
            return vec![d3d9_x32];
        }
    }

    dlls
}

fn detect_dlls(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_dir(root, &mut |p| {
        if p.is_file() {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("dll") {
                    out.push(p.to_path_buf());
                }
            }
        }
    });
    out
}

const MAX_ADDON_SCAN_DEPTH: usize = 5;

fn detect_addons(root: &Path) -> Vec<(PathBuf, String)> {
    detect_addons_with_deadline(root, None).0
}

fn detect_addons_with_deadline(
    root: &Path,
    deadline: Option<Instant>,
) -> (Vec<(PathBuf, String)>, bool) {
    let mut candidates: Vec<(PathBuf, String)> = Vec::new();

    // 1. Check if the root itself is an addon
    for folder_name in addon_folder_names_from_toc(root, root) {
        candidates.push((root.to_path_buf(), folder_name));
    }
    if !candidates.is_empty() {
        return (candidates, false);
    }

    let timed_out = scan_addon_dirs(root, root, 1, &mut candidates, deadline);

    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    candidates.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    (candidates, timed_out)
}

fn scan_addon_dirs(
    dir: &Path,
    scan_root: &Path,
    depth: usize,
    candidates: &mut Vec<(PathBuf, String)>,
    deadline: Option<Instant>,
) -> bool {
    if depth > MAX_ADDON_SCAN_DEPTH {
        return false;
    }
    if deadline.map(|d| Instant::now() >= d).unwrap_or(false) {
        return true;
    }

    let Ok(rd) = fs::read_dir(dir) else {
        return false;
    };

    for entry in rd.flatten() {
        if deadline.map(|d| Instant::now() >= d).unwrap_or(false) {
            return true;
        }
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false)
        {
            continue;
        }

        let names = addon_folder_names_from_toc(&path, scan_root);
        if !names.is_empty() {
            for folder_name in names {
                candidates.push((path.clone(), folder_name));
            }
            continue;
        }

        if scan_addon_dirs(&path, scan_root, depth + 1, candidates, deadline) {
            return true;
        }
    }
    false
}

pub fn detect_addons_in_tree(root: &Path) -> Vec<(PathBuf, String)> {
    if let Ok(list) = detect_addons_in_git_tree(root) {
        if !list.is_empty() {
            return list;
        }
    }
    detect_addons(root)
}

pub fn detect_addons_in_tree_with_time_limit(
    root: &Path,
    limit: Duration,
) -> (Vec<(PathBuf, String)>, bool) {
    let deadline = Some(Instant::now() + limit);
    match detect_addons_in_git_tree_with_deadline(root, deadline) {
        Ok((list, timed_out)) if !list.is_empty() || timed_out => (list, timed_out),
        _ => detect_addons_with_deadline(root, deadline),
    }
}

pub fn detect_single_addon_folder(dir: &Path) -> Option<String> {
    addon_folder_names_from_toc(dir, dir).into_iter().next()
}

pub fn detect_addon_folders_in_dir(dir: &Path) -> Vec<String> {
    addon_folder_names_from_toc(dir, dir)
}

pub fn detect_addons_in_git_tree(root: &Path) -> Result<Vec<(PathBuf, String)>> {
    detect_addons_in_git_tree_with_deadline(root, None).map(|(list, _)| list)
}

fn detect_addons_in_git_tree_with_deadline(
    root: &Path,
    deadline: Option<Instant>,
) -> Result<(Vec<(PathBuf, String)>, bool)> {
    let repo = Repository::open(root).context("open git repo for tree scan")?;
    let head = repo.head()?.peel_to_tree()?;
    let mut candidates = Vec::new();

    // 1. Check root
    for name in addon_folder_names_from_git_tree(&repo, &head, "", true) {
        candidates.push((root.to_path_buf(), name));
    }
    if !candidates.is_empty() {
        candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        candidates.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        return Ok((candidates, false));
    }

    let timed_out = scan_git_addon_dirs(&repo, root, &head, "", 1, &mut candidates, deadline)?;

    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    candidates.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    Ok((candidates, timed_out))
}

fn scan_git_addon_dirs(
    repo: &Repository,
    root: &Path,
    tree: &Tree,
    rel_path: &str,
    depth: usize,
    candidates: &mut Vec<(PathBuf, String)>,
    deadline: Option<Instant>,
) -> Result<bool> {
    if depth > MAX_ADDON_SCAN_DEPTH {
        return Ok(false);
    }
    if deadline.map(|d| Instant::now() >= d).unwrap_or(false) {
        return Ok(true);
    }

    for entry in tree.iter() {
        if deadline.map(|d| Instant::now() >= d).unwrap_or(false) {
            return Ok(true);
        }
        if entry.kind() != Some(git2::ObjectType::Tree) {
            continue;
        }

        let name = entry.name().unwrap_or("");
        if name.starts_with('.') {
            continue;
        }

        let subtree = entry.to_object(repo)?.peel_to_tree()?;
        let child_rel = if rel_path.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", rel_path, name)
        };

        let names = addon_folder_names_from_git_tree(repo, &subtree, name, false);
        if !names.is_empty() {
            for addon_name in names {
                candidates.push((root.join(&child_rel), addon_name));
            }
            continue;
        }

        if scan_git_addon_dirs(repo, root, &subtree, &child_rel, depth + 1, candidates, deadline)? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn addon_folder_names_from_git_tree(
    _repo: &Repository,
    tree: &Tree,
    dir_name: &str,
    is_root: bool,
) -> Vec<String> {
    let mut stems = Vec::new();
    for entry in tree.iter() {
        if entry.kind() == Some(git2::ObjectType::Blob) {
            let name = entry.name().unwrap_or("");
            if name.to_ascii_lowercase().ends_with(".toc") {
                let stem = Path::new(name).file_stem().and_then(|s| s.to_str());
                if let Some(s) = stem {
                    stems.push(s.to_string());
                }
            }
        }
    }

    if stems.is_empty() {
        return vec![];
    }

    resolve_addon_names_from_stems(stems, dir_name, is_root)
}

fn resolve_addon_names_from_stems(
    stems: Vec<String>,
    dir_name: &str,
    is_root: bool,
) -> Vec<String> {
    if is_root {
        let mut out = stems;
        out.sort();
        out.dedup();
        return out;
    }

    let normalized: Vec<String> = stems.iter().map(|s| normalize_toc_stem(s)).collect();

    // GAM strict rule: for subfolders, matches folder name (or normalized folder name).
    if !is_root && !dir_name.is_empty() {
        // Prefer the exact TOC stem if it matches the folder name case-insensitively.
        if let Some(matching_stem) = stems.iter().find(|name| name.eq_ignore_ascii_case(dir_name)) {
            return vec![matching_stem.clone()];
        }
        // Also check normalized stems (e.g. Atlas-classic matches Atlas)
        if let Some(idx) = normalized.iter().position(|name| name.eq_ignore_ascii_case(dir_name)) {
            return vec![stems[idx].clone()];
        }
    }

    let first_norm = normalized[0].to_ascii_lowercase();
    let all_same_norm = normalized
        .iter()
        .all(|name| name.to_ascii_lowercase() == first_norm);

    if all_same_norm {
        return vec![normalized.into_iter().next().unwrap()];
    }

    // Fallback for subfolders if no direct match but multiple TOCs agree on a name.
    let mut counts: HashMap<String, usize> = HashMap::new();
    for name in normalized {
        *counts.entry(name.to_ascii_lowercase()).or_insert(0) += 1;
    }
    let mut best: Option<(String, usize)> = None;
    for (key, count) in counts {
        match &best {
            None => best = Some((key, count)),
            Some((prev, prev_count)) => {
                if count > *prev_count || (count == *prev_count && key < *prev) {
                    best = Some((key, count));
                }
            }
        }
    }
    best.map(|(name, _)| vec![name]).unwrap_or_default()
}

fn addon_folder_names_from_toc(dir: &Path, scan_root: &Path) -> Vec<String> {
    let is_root = dir == scan_root;
    let dir_name = dir.file_name().and_then(|s| s.to_str()).unwrap_or_default();
    let mut stems: Vec<String> = Vec::new();

    let rd = fs::read_dir(dir).ok();
    if let Some(rd) = rd {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_file() {
                if p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("toc"))
                    .unwrap_or(false)
                {
                    let stem = p
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    if let Some(name) = stem {
                        stems.push(name);
                    }
                }
            }
        }
    }

    if stems.is_empty() {
        return vec![];
    }

    resolve_addon_names_from_stems(stems, dir_name, is_root)
}

fn normalize_toc_stem(stem: &str) -> String {
    let mut out = stem.trim().to_string();
    if out.is_empty() {
        return out;
    }

    // Common expansion/channel suffixes used in WoW addon TOCs.
    // Credit: inspired by GitAddonsManager's TOC suffix handling
    // (Control::removeTocSuffixes in control.cpp), expanded here to cover both
    // dash/underscore forms and additional channel tags seen in the wild.
    const SUFFIXES: &[&str] = &[
        "-classic",
        "_classic",
        "-bcc",
        "_bcc",
        "-vanilla",
        "_vanilla",
        "-tbc",
        "_tbc",
        "-mainline",
        "_mainline",
        "-wrath",
        "_wrath",
        "-wotlk",
        "_wotlk",
        "-wotlkc",
        "_wotlkc",
        "-era",
        "_era",
        "-classicera",
        "_classicera",
        "-retail",
        "_retail",
        "-beta",
        "_beta",
        "-ptr",
        "_ptr",
        "-test",
        "_test",
        "-development",
        "_development",
        "-dev",
        "_dev",
        "-cata",
        "_cata",
        "-sod",
        "_sod",
    ];

    loop {
        let lower = out.to_ascii_lowercase();
        let mut changed = false;
        for suffix in SUFFIXES {
            if lower.ends_with(suffix) && out.len() > suffix.len() {
                let new_len = out.len() - suffix.len();
                out.truncate(new_len);
                out = out.trim_end_matches(['-', '_']).trim().to_string();
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }

    if out.is_empty() {
        stem.trim().to_string()
    } else {
        out
    }
}

fn walk_dir(root: &Path, cb: &mut dyn FnMut(&Path)) {
    let rd = match fs::read_dir(root) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in rd.flatten() {
        let p = entry.path();
        cb(&p);
        if p.is_dir() {
            if p.file_name()
                .and_then(|s| s.to_str())
                .map(|name| {
                    name.eq_ignore_ascii_case(".git") || name.eq_ignore_ascii_case(".wuddle")
                })
                .unwrap_or(false)
            {
                continue;
            }
            walk_dir(&p, cb);
        }
    }
}

/// Expose a sub-addon folder from a multi-addon git repository at `dst`.
///
/// This follows GAM's `unpackSubfolders()` behaviour by default, while still
/// allowing Wuddle's explicit symlink option to override the install primitive.
/// - When `use_symlink` is false, move the folder out of the repo worktree.
/// - When `use_symlink` is true, try a relative symlink on Unix or a junction
///   on Windows before falling back to moving the folder.
/// - If the link path is unavailable, rename (move) the folder out of the repo.
pub fn link_addon_subfolder(
    repo_dir: &Path,
    sub_path: &str,
    dst: &Path,
    use_symlink: bool,
) -> Result<InstallRecord> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {:?}", parent))?;
    }
    remove_any_target(dst)?;

    let src = repo_dir.join(sub_path);
    let repo_dir_name = repo_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let rel_sub_path = sub_path.replace('\\', "/");
    let rel_src = format!("./{}/{}", repo_dir_name, rel_sub_path);

    let mut linked = false;

    if use_symlink {
        #[cfg(unix)]
        {
            if std::os::unix::fs::symlink(&rel_src, dst).is_ok() {
                linked = true;
            }
        }

        #[cfg(windows)]
        {
            if junction::create(&src, dst).is_ok() {
                linked = true;
            }
        }
    }

    if !linked {
        // Fallback to rename (move) exactly as GAM does.
        // If the source folder DOES NOT exist, check if it was already moved to the destination.
        if !src.exists() {
            if dst.exists() && dst.is_dir() {
                // Already moved in a previous session or by GAM.
                return Ok(InstallRecord {
                    path: dst.to_path_buf(),
                    kind: "addon",
                });
            }
            anyhow::bail!("Source subfolder {:?} does not exist in repo and is not at destination {:?}", src, dst);
        }

        fs::rename(&src, dst).with_context(|| {
            format!("Failed both link and move for {:?} -> {:?}", src, dst)
        })?;
    }

    Ok(InstallRecord {
        path: dst.to_path_buf(),
        kind: "addon",
    })
}

pub fn install_addon_folder(
    src_dir: &Path,
    wow_dir: &Path,
    addon_folder_name: &str,
    opts: InstallOptions,
    comment: &str,
) -> Result<InstallRecord> {
    let dst_dir = wow_dir
        .join("Interface")
        .join("AddOns")
        .join(addon_folder_name);
    install_dir_or_symlink(src_dir, &dst_dir, opts.use_symlinks)?;
    maybe_set_comment(&dst_dir, comment, opts.set_xattr_comment);
    Ok(InstallRecord {
        path: dst_dir,
        kind: "addon",
    })
}

pub fn install_dll(
    downloaded: &Path,
    wow_dir: &Path,
    filename: &str,
    opts: InstallOptions,
    comment: &str,
) -> Result<InstallRecord> {
    let dst = wow_dir.join(filename);
    install_file_or_symlink(downloaded, &dst, opts.use_symlinks)?;
    update_dlls_txt(wow_dir, comment, &[filename.to_string()][..])?;
    maybe_set_comment(&dst, comment, opts.set_xattr_comment);
    Ok(InstallRecord {
        path: dst,
        kind: "dll",
    })
}

pub fn install_raw_file(
    downloaded: &Path,
    dest_dir: &Path,
    filename: &str,
    opts: InstallOptions,
    comment: &str,
) -> Result<InstallRecord> {
    fs::create_dir_all(dest_dir).context("create raw destination dir")?;
    let dst = dest_dir.join(filename);
    install_file_or_symlink(downloaded, &dst, opts.use_symlinks)?;
    maybe_set_comment(&dst, comment, opts.set_xattr_comment);
    Ok(InstallRecord {
        path: dst,
        kind: "raw",
    })
}

#[cfg(test)]
mod tests {
    use super::{
        detect_addons_in_tree, install_from_archive, normalize_toc_stem, safe_archive_path,
        InstallOptions,
    };
    use git2::Repository;
    use std::{fs, io::Write};

    #[test]
    fn normalize_toc_suffixes_common_cases() {
        assert_eq!(normalize_toc_stem("pfQuest-tbc"), "pfQuest");
        assert_eq!(normalize_toc_stem("pfQuest-wotlk"), "pfQuest");
        assert_eq!(normalize_toc_stem("pfQuest_Wrath"), "pfQuest");
        assert_eq!(normalize_toc_stem("pfUI-Classic"), "pfUI");
        assert_eq!(normalize_toc_stem("MyAddon-WOTLKC"), "MyAddon");
    }

    #[test]
    fn normalize_toc_suffixes_preserves_non_suffix_names() {
        assert_eq!(normalize_toc_stem("nampower"), "nampower");
        assert_eq!(normalize_toc_stem("VanillaHelpers"), "VanillaHelpers");
        assert_eq!(normalize_toc_stem("Addon-Tooling"), "Addon-Tooling");
        assert_eq!(normalize_toc_stem("Questie-335"), "Questie-335");
    }

    #[test]
    fn git_tree_detection_prefers_root_tocs_over_embedded_libraries() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("Questie.toc"), b"## Interface: 30300\n").unwrap();
        fs::write(tmp.path().join("Questie-335.toc"), b"## Interface: 30300\n").unwrap();
        let lib = tmp.path().join("Libs").join("HereBeDragons");
        fs::create_dir_all(&lib).unwrap();
        fs::write(lib.join("HereBeDragons.toc"), b"## Interface: 30300\n").unwrap();

        let repo = Repository::init(tmp.path()).unwrap();
        let mut index = repo.index().unwrap();
        index.add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Wuddle Test", "test@example.invalid").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[]).unwrap();

        let detected = detect_addons_in_tree(tmp.path());
        assert_eq!(
            detected
                .iter()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["Questie", "Questie-335"]
        );
        assert!(detected.iter().all(|(src, _)| src == tmp.path()));
    }

    #[test]
    fn safe_archive_path_rejects_traversal_and_absolute_paths() {
        assert!(safe_archive_path("folder/file.dll").is_some());
        assert!(safe_archive_path("../evil.dll").is_none());
        assert!(safe_archive_path("/tmp/evil.dll").is_none());
        assert!(safe_archive_path("C:/evil.dll").is_none());
        assert!(safe_archive_path("C:\\evil.dll").is_none());
        assert!(safe_archive_path("").is_none());
    }

    #[test]
    fn install_from_7z_installs_dll_and_updates_dlls_txt() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let wow = tmp.path().join("wow");
        let extract = tmp.path().join("extract");
        let archive = tmp.path().join("dll_mod.7z");

        fs::create_dir_all(src.join("nested")).unwrap();
        fs::write(src.join("nested").join("Example.dll"), b"MZ fake dll").unwrap();
        sevenz_rust::compress_to_path(&src, &archive).unwrap();

        let records = install_from_archive(
            &archive,
            &extract,
            &wow,
            "dll",
            InstallOptions::default(),
            "example/mod v1 - managed by Wuddle",
        )
        .unwrap();

        assert!(wow.join("Example.dll").exists());
        assert!(fs::read_to_string(wow.join("dlls.txt"))
            .unwrap()
            .contains("Example.dll"));
        assert_eq!(records.iter().filter(|r| r.kind == "dll").count(), 1);
    }

    #[test]
    fn install_from_7z_installs_addon_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let addon = src.join("MyAddon");
        let wow = tmp.path().join("wow");
        let extract = tmp.path().join("extract");
        let archive = tmp.path().join("addon_mod.7z");

        fs::create_dir_all(&addon).unwrap();
        fs::write(addon.join("MyAddon.toc"), b"## Interface: 11200\n").unwrap();
        fs::write(addon.join("core.lua"), b"print('ok')\n").unwrap();
        sevenz_rust::compress_to_path(&src, &archive).unwrap();

        let records = install_from_archive(
            &archive,
            &extract,
            &wow,
            "addon",
            InstallOptions::default(),
            "example/addon v1 - managed by Wuddle",
        )
        .unwrap();

        assert!(wow
            .join("Interface")
            .join("AddOns")
            .join("MyAddon")
            .join("MyAddon.toc")
            .exists());
        assert_eq!(records.iter().filter(|r| r.kind == "addon").count(), 1);
    }

    #[test]
    fn install_from_mislabeled_7z_zip_bytes_installs_dll() {
        let tmp = tempfile::tempdir().unwrap();
        let wow = tmp.path().join("wow");
        let extract = tmp.path().join("extract");
        let archive = tmp.path().join("Release.7z");

        let file = fs::File::create(&archive).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(
            "nested/Example.dll",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"MZ fake dll").unwrap();
        zip.finish().unwrap();

        let records = install_from_archive(
            &archive,
            &extract,
            &wow,
            "dll",
            InstallOptions::default(),
            "example/mod v1 - managed by Wuddle",
        )
        .unwrap();

        assert!(wow.join("Example.dll").exists());
        assert_eq!(records.iter().filter(|r| r.kind == "dll").count(), 1);
    }
}
