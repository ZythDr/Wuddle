use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct InstallOptions {
    pub use_symlinks: bool,
    pub set_xattr_comment: bool,
    pub replace_addon_conflicts: bool,
}

#[derive(Debug, Clone)]
pub struct InstallRecord {
    pub path: PathBuf,
    pub kind: &'static str, // "dll" | "addon" | "raw"
}

/// Install from a downloaded ZIP into the WoW directory.
///
/// Modes:
/// - addon: copy addon folders into Interface/AddOns
/// - dll: copy *.dll into WoW root
/// - mixed: both
/// - raw: currently unused for zip
pub fn install_from_zip(
    zip_path: &Path,
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

    unzip(zip_path, extract_dir).context("unzip")?;

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
        update_dlls_txt(wow_root, &installed_dlls)?;
    }

    if want_addon {
        for (src_dir, addon_folder_name) in detect_addons(extract_dir) {
            let rec = install_addon_folder(&src_dir, wow_dir, &addon_folder_name, opts, comment)?;
            records.push(rec);
        }
    }

    Ok(records)
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

/// Unzip ZIP file into destination directory.
fn unzip(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir).with_context(|| format!("cleanup {:?}", dest_dir))?;
    }
    fs::create_dir_all(dest_dir).with_context(|| format!("mkdir {:?}", dest_dir))?;

    let file = fs::File::open(zip_path).with_context(|| format!("open zip {:?}", zip_path))?;
    let mut archive = zip::ZipArchive::new(file).context("read zip")?;

    for i in 0..archive.len() {
        let mut f = archive.by_index(i).context("zip entry")?;
        let outpath = dest_dir.join(f.mangled_name());

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
            fs::remove_file(path).with_context(|| format!("remove symlink {:?}", path))?;
            return Ok(());
        }
        if ft.is_dir() {
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

fn update_dlls_txt(wow_dir: &Path, dll_names: &[String]) -> Result<()> {
    if dll_names.is_empty() {
        return Ok(());
    }

    let path = wow_dir.join("dlls.txt");
    let existing = fs::read_to_string(&path).unwrap_or_default();

    let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();

    for dll in dll_names {
        let mut found = false;

        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let (_commented, rest) = if let Some(stripped) = trimmed.strip_prefix('#') {
                (true, stripped.trim())
            } else {
                (false, trimmed)
            };

            if rest.eq_ignore_ascii_case(dll) {
                *line = dll.clone();
                found = true;
                break;
            }
        }

        if !found {
            lines.push(dll.clone());
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

fn detect_addons(root: &Path) -> Vec<(PathBuf, String)> {
    let mut candidates: Vec<(PathBuf, String)> = Vec::new();

    let ia = root.join("Interface").join("AddOns");
    if ia.exists() {
        if let Ok(rd) = fs::read_dir(&ia) {
            for entry in rd.flatten() {
                let dir = entry.path();
                if dir.is_dir() {
                    if let Some(folder_name) = addon_folder_name_from_toc(&dir, &ia) {
                        candidates.push((dir, folder_name));
                    }
                }
            }
        }
        if !candidates.is_empty() {
            return candidates;
        }
    }

    walk_dir(root, &mut |p| {
        if p.is_file() {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("toc") {
                    if let Some(parent) = p.parent() {
                        if let Some(folder_name) = addon_folder_name_from_toc(parent, root) {
                            candidates.push((parent.to_path_buf(), folder_name));
                        }
                    }
                }
            }
        }
    });

    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    candidates.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    candidates
}

pub fn detect_addons_in_tree(root: &Path) -> Vec<(PathBuf, String)> {
    detect_addons(root)
}

fn addon_folder_name_from_toc(dir: &Path, scan_root: &Path) -> Option<String> {
    let is_root = dir == scan_root;
    let dir_name = dir.file_name().and_then(|s| s.to_str()).unwrap_or_default();
    let mut stems: Vec<String> = Vec::new();

    let rd = fs::read_dir(dir).ok()?;
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
    if stems.is_empty() {
        return None;
    }

    let normalized: Vec<String> = stems.iter().map(|s| normalize_toc_stem(s)).collect();

    // Credit: inspired by GitAddonsManager's subfolder scan behavior.
    // For non-root addon directories, if any TOC matches this folder directly
    // (or after suffix normalization), keep the folder name as-is.
    if !is_root && !dir_name.is_empty() {
        if stems.iter().any(|name| name.eq_ignore_ascii_case(dir_name))
            || normalized
                .iter()
                .any(|name| name.eq_ignore_ascii_case(dir_name))
        {
            return Some(dir_name.to_string());
        }
    }

    let first_norm = normalized[0].to_ascii_lowercase();
    let all_same_norm = normalized
        .iter()
        .all(|name| name.to_ascii_lowercase() == first_norm);

    // If all TOCs normalize to the same base, use that base (GAM-style).
    if all_same_norm {
        return normalized.into_iter().next();
    }

    // Root disagreement means this is likely a repo root with multiple modules;
    // skip root here and let nested addon folders be detected.
    if is_root {
        return None;
    }

    // Non-root fallback: choose the most common normalized name.
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

    best.map(|(name, _)| name)
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
    update_dlls_txt(wow_dir, &[filename.to_string()])?;
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
    use super::normalize_toc_stem;

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
    }
}
