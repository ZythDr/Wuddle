use serde::{Deserialize, Serialize};

#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "windows")]
use std::{
    io::{Cursor, Read, Write},
    path::Path,
};
#[cfg(target_os = "windows")]
use zip::ZipArchive;

#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;

use crate::OperationResult;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfUpdateInfo {
    pub supported: bool,
    pub launcher_layout: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub assets_pending: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubReleaseAsset>,
}

const WUDDLE_RELEASE_API_LATEST: &str =
    "https://api.github.com/repos/ZythDr/Wuddle/releases/latest";
const WUDDLE_RELEASE_API_ALL: &str =
    "https://api.github.com/repos/ZythDr/Wuddle/releases?per_page=5";

pub fn update_info(current_version: &str, beta_channel: bool) -> Result<SelfUpdateInfo, String> {
    #[cfg(target_os = "linux")]
    {
        let appimage = is_appimage();
        let supported = appimage.is_some();

        let release = match fetch_release_meta(beta_channel) {
            Ok(v) => v,
            Err(err) => {
                return Ok(SelfUpdateInfo {
                    supported,
                    launcher_layout: false,
                    current_version: current_version.to_string(),
                    latest_version: None,
                    update_available: false,
                    assets_pending: false,
                    message: format!("Latest version check failed: {}", err),
                });
            }
        };

        let latest_version = normalize_release_tag(&release.tag_name);
        let latest_version = if latest_version.is_empty() {
            None
        } else {
            Some(latest_version)
        };
        let mut update_available = supported
            && latest_version
                .as_deref()
                .map(|latest| is_version_newer(latest, current_version))
                .unwrap_or(false);

        let mut assets_pending = false;
        if update_available && select_linux_appimage_asset(&release).is_none() {
            assets_pending = true;
            update_available = false;
        }

        let message = if !supported {
            "Self-update is available only for AppImage builds on Linux.".to_string()
        } else if assets_pending {
            "Update available but release assets are still being built. Try again in a few minutes.".to_string()
        } else if update_available {
            "A newer version is available.".to_string()
        } else {
            "No newer version detected.".to_string()
        };

        return Ok(SelfUpdateInfo {
            supported,
            launcher_layout: false,
            current_version: current_version.to_string(),
            latest_version,
            update_available,
            assets_pending,
            message,
        });
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let latest_version = fetch_release_meta(beta_channel)
            .ok()
            .map(|r| normalize_release_tag(&r.tag_name))
            .filter(|v| !v.is_empty());
        return Ok(SelfUpdateInfo {
            supported: false,
            launcher_layout: false,
            current_version: current_version.to_string(),
            latest_version,
            update_available: false,
            assets_pending: false,
            message: "In-app update is not available on this platform.".to_string(),
        });
    }

    #[cfg(target_os = "windows")]
    {
        let current_version = current_version.to_string();
        let root = launcher_root_dir()?;
        let exe_path = current_exe_path()?;
        let launcher = launcher_exe_path(&root);
        let launcher_layout = launcher.is_file() && is_versioned_runtime_layout(&root, &exe_path);

        let release = match fetch_release_meta(beta_channel) {
            Ok(v) => v,
            Err(err) => {
                return Ok(SelfUpdateInfo {
                    supported: launcher_layout,
                    launcher_layout,
                    current_version,
                    latest_version: None,
                    update_available: false,
                    assets_pending: false,
                    message: format!("Latest version check failed: {}", err),
                });
            }
        };

        let latest_version = normalize_release_tag(&release.tag_name);
        let latest_version = if latest_version.is_empty() {
            None
        } else {
            Some(latest_version)
        };
        let mut update_available = latest_version
            .as_deref()
            .map(|latest| launcher_layout && is_version_newer(latest, &current_version))
            .unwrap_or(false);

        let mut assets_pending = false;
        if update_available && select_windows_portable_asset(&release).is_none() {
            assets_pending = true;
            update_available = false;
        }

        let message = if !launcher_layout {
            "Current install is legacy layout. Install latest portable package once to enable in-app updates."
                .to_string()
        } else if assets_pending {
            "Update available but release assets are still being built. Try again in a few minutes.".to_string()
        } else if update_available {
            "A newer version is available.".to_string()
        } else {
            "No newer version detected.".to_string()
        };

        Ok(SelfUpdateInfo {
            supported: launcher_layout,
            launcher_layout,
            current_version,
            latest_version,
            update_available,
            assets_pending,
            message,
        })
    }
}

pub fn apply_update(current_version: &str, beta_channel: bool) -> Result<OperationResult, String> {
    #[cfg(target_os = "linux")]
    {
        let mut steps = Vec::new();
        let current_version = current_version.to_string();

        let appimage_path = is_appimage()
            .ok_or_else(|| "Not running as an AppImage. Self-update is unavailable.".to_string())?;
        steps.push(format!("AppImage path: {}", appimage_path.display()));

        cleanup_stale_appimage_temps(&appimage_path);

        steps.push("Checking latest release metadata…".to_string());
        let release = fetch_release_meta(beta_channel)?;
        let latest_version = normalize_release_tag(&release.tag_name);
        if latest_version.is_empty() {
            return Err("Latest release tag is empty.".to_string());
        }
        if !is_version_newer(&latest_version, &current_version) {
            return Ok(OperationResult {
                message: format!("Already up to date ({current_version})."),
                steps,
            });
        }

        let asset = select_linux_appimage_asset(&release)
            .ok_or_else(|| "No Linux AppImage asset found in latest release.".to_string())?;
        steps.push(format!("Selected asset: {}", asset.name));
        steps.push(format!("Downloading {}", asset.browser_download_url));

        let bytes = download_bytes(&asset.browser_download_url)?;
        steps.push(format!("Downloaded {} bytes.", bytes.len()));

        // Write to a temp file next to the current AppImage (same filesystem for atomic rename)
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let tmp_path = appimage_path.with_extension(format!("tmp-{}", stamp));

        fs::write(&tmp_path, &bytes)
            .map_err(|e| format!("Failed to write temp file: {e}"))?;

        // chmod +x
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&tmp_path, perms)
            .map_err(|e| format!("Failed to set executable permission: {e}"))?;
        steps.push("Set executable permission on temp file.".to_string());

        // Atomic rename over the original
        fs::rename(&tmp_path, &appimage_path)
            .map_err(|e| format!("Failed to replace AppImage: {e}"))?;
        steps.push(format!("Replaced {}", appimage_path.display()));

        return Ok(OperationResult {
            message: format!(
                "Updated Wuddle to {} successfully. Restart to apply.",
                latest_version
            ),
            steps,
        });
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = current_version;
        return Err("In-app update is not available on this platform.".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let mut steps = Vec::new();
        let current_version = current_version.to_string();

        let root = launcher_root_dir()?;
        let exe_path = current_exe_path()?;
        let launcher = launcher_exe_path(&root);
        let launcher_layout = launcher.is_file() && is_versioned_runtime_layout(&root, &exe_path);
        if !launcher_layout {
            return Err(
                "Legacy install layout detected. Install latest portable package manually once, then retry in-app updates."
                    .to_string(),
            );
        }

        cleanup_stale_update_files(&root);
        steps.push(format!("Detected launcher root: {}", root.display()));
        steps.push("Checking latest release metadata…".to_string());
        let release = fetch_release_meta(beta_channel)?;
        let latest_version = normalize_release_tag(&release.tag_name);
        if latest_version.is_empty() {
            return Err("Latest release tag is empty.".to_string());
        }
        if !is_version_newer(&latest_version, &current_version) {
            return Ok(OperationResult {
                message: format!("Already up to date ({current_version})."),
                steps,
            });
        }

        let asset = select_windows_portable_asset(&release)
            .ok_or_else(|| "No Windows portable ZIP asset found in latest release.".to_string())?;
        steps.push(format!("Selected asset: {}", asset.name));
        steps.push(format!("Downloading {}", asset.browser_download_url));
        let zip_bytes = download_bytes(&asset.browser_download_url)?;
        steps.push(format!("Downloaded {} bytes.", zip_bytes.len()));

        let payload = extract_windows_payload_from_zip(&zip_bytes, &latest_version)?;
        let target_version = sanitize_version_folder_name(&payload.version_name);
        let target_runtime = root
            .join("versions")
            .join(&target_version)
            .join(runtime_binary_name());
        write_atomic(&target_runtime, &payload.runtime_bytes)?;
        steps.push(format!("Staged runtime: {}", target_runtime.display()));

        if let Some(launcher_bytes) = payload.launcher_bytes {
            let launcher_target = launcher_exe_path(&root);
            write_atomic(&launcher_target, &launcher_bytes)?;
            steps.push(format!("Updated launcher: {}", launcher_target.display()));
        }

        write_current_pointer(&root, &target_version)?;
        steps.push(format!("Switched current.json to {}", target_version));

        match prune_old_versions(&root, 2) {
            Ok(removed) => {
                for name in &removed {
                    steps.push(format!("Removed old version: {}", name));
                }
            }
            Err(e) => {
                steps.push(format!("Version cleanup skipped: {}", e));
            }
        }

        Ok(OperationResult {
            message: format!(
                "Staged Wuddle {} successfully. Restarting will apply the update.",
                target_version
            ),
            steps,
        })
    }
}

pub fn restart_after_update() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let appimage_path = is_appimage()
            .ok_or_else(|| "Not running as an AppImage; cannot restart.".to_string())?;

        Command::new(&appimage_path)
            .spawn()
            .map_err(|e| format!("Failed to relaunch AppImage: {e}"))?;

        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(200));
            std::process::exit(0);
        });

        return Ok(());
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        return Err("In-app update restart is not available on this platform.".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let root = launcher_root_dir()?;
        let launcher = launcher_exe_path(&root);
        if !launcher.is_file() {
            return Err(format!("Launcher not found at {}", launcher.display()));
        }

        Command::new(&launcher)
            .current_dir(&root)
            .spawn()
            .map_err(|e| format!("Failed to relaunch launcher: {}", e))?;

        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(200));
            std::process::exit(0);
        });

        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn launcher_root_dir() -> Result<PathBuf, String> {
    crate::portable_root_dir()
}

#[cfg(target_os = "windows")]
fn current_exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn launcher_exe_path(root: &Path) -> PathBuf {
    root.join("Wuddle.exe")
}

#[cfg(target_os = "windows")]
fn runtime_binary_name() -> &'static str {
    "Wuddle-bin.exe"
}

#[cfg(target_os = "windows")]
fn is_versioned_runtime_layout(root: &Path, exe_path: &Path) -> bool {
    let versions = root.join("versions");
    if !versions.is_dir() {
        return false;
    }
    let Some(parent) = exe_path.parent() else {
        return false;
    };
    let Some(version_dir) = parent.parent() else {
        return false;
    };
    let Some(name) = version_dir.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    name.eq_ignore_ascii_case("versions")
}

fn normalize_release_tag(raw: &str) -> String {
    raw.trim().trim_start_matches(['v', 'V']).trim().to_string()
}

#[cfg(target_os = "windows")]
fn sanitize_version_folder_name(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            out.push(ch);
        }
    }
    if out.is_empty() {
        "latest".to_string()
    } else {
        out
    }
}

fn parse_version_key(raw: &str) -> Vec<u64> {
    let trimmed = normalize_release_tag(raw);
    trimmed
        .split(|c: char| !(c.is_ascii_alphanumeric()))
        .filter(|segment| !segment.is_empty())
        .filter_map(|segment| segment.parse::<u64>().ok())
        .collect()
}

fn is_version_newer(latest: &str, current: &str) -> bool {
    let a = parse_version_key(latest);
    let b = parse_version_key(current);
    let max = a.len().max(b.len());
    for i in 0..max {
        let av = *a.get(i).unwrap_or(&0);
        let bv = *b.get(i).unwrap_or(&0);
        if av > bv {
            return true;
        }
        if av < bv {
            return false;
        }
    }
    // Numeric parts equal — treat as newer if the full tag differs
    // (handles suffixed hotfix tags like "2.4.5-fix" vs "2.4.5")
    normalize_release_tag(latest) != normalize_release_tag(current)
}

fn github_api_token() -> Option<String> {
    if let Some(token) = crate::env_token() {
        return Some(token);
    }
    crate::read_keychain_token().ok().flatten()
}

fn fetch_release_meta(beta_channel: bool) -> Result<GithubRelease, String> {
    let url = if beta_channel {
        WUDDLE_RELEASE_API_ALL
    } else {
        WUDDLE_RELEASE_API_LATEST
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("build http client: {e}"))?;

    let mut req = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header(
            "User-Agent",
            format!("Wuddle/{}", env!("CARGO_PKG_VERSION")),
        );
    if let Some(token) = github_api_token() {
        req = req.header("Authorization", format!("Bearer {}", token));
    }

    let resp = req
        .send()
        .map_err(|e| format!("fetch release metadata: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let code = status.as_u16();
        let message = if code == 403 || code == 429 {
            let body = resp.text().unwrap_or_default().to_ascii_lowercase();
            if body.contains("rate limit") {
                "GitHub API rate limit exceeded. Add a GitHub token in Options to raise the limit."
            } else {
                "GitHub denied the request. Your token may be invalid or expired."
            }
        } else if code == 401 {
            "GitHub authentication failed. Your token may be invalid or expired."
        } else {
            "Could not fetch release information from GitHub."
        };
        return Err(format!("{} (HTTP {})", message, code));
    }

    if beta_channel {
        // /releases returns an array; take the first (most recent) entry
        let mut releases = resp
            .json::<Vec<GithubRelease>>()
            .map_err(|e| format!("parse releases list: {e}"))?;
        releases
            .into_iter()
            .next()
            .ok_or_else(|| "No releases found.".to_string())
    } else {
        resp.json::<GithubRelease>()
            .map_err(|e| format!("parse release metadata: {e}"))
    }
}

#[cfg(target_os = "windows")]
fn select_windows_portable_asset(release: &GithubRelease) -> Option<&GithubReleaseAsset> {
    // Tauri releases: wuddle-<tag>-windows-portable.zip
    // Iced releases:  wuddle-<tag>-windows-x86_64.zip
    release
        .assets
        .iter()
        .find(|a| a.name.ends_with("-windows-portable.zip"))
        .or_else(|| {
            release.assets.iter().find(|a| {
                let name = a.name.to_ascii_lowercase();
                name.contains("windows-portable") && name.ends_with(".zip")
            })
        })
        .or_else(|| {
            release.assets.iter().find(|a| {
                let name = a.name.to_ascii_lowercase();
                name.contains("windows") && name.ends_with(".zip")
            })
        })
}

#[cfg(target_os = "linux")]
fn is_appimage() -> Option<PathBuf> {
    let path = std::env::var("APPIMAGE").ok()?;
    let path = PathBuf::from(path);
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn select_linux_appimage_asset(release: &GithubRelease) -> Option<&GithubReleaseAsset> {
    // Primary: exact name "Wuddle.AppImage"
    release
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("Wuddle.AppImage"))
        .or_else(|| {
            // Fallback: any .AppImage file (handles older releases with versioned names)
            release
                .assets
                .iter()
                .find(|a| a.name.to_ascii_lowercase().ends_with(".appimage"))
        })
}

#[cfg(target_os = "linux")]
fn cleanup_stale_appimage_temps(appimage_path: &std::path::Path) {
    let Some(parent) = appimage_path.parent() else {
        return;
    };
    let Some(stem) = appimage_path.file_stem().and_then(|s| s.to_str()) else {
        return;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(stem) && name.contains(".tmp-") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("build http client: {e}"))?;

    let mut req = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .header(
            "User-Agent",
            format!("Wuddle/{}", env!("CARGO_PKG_VERSION")),
        );
    if let Some(token) = github_api_token() {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    let mut resp = req.send().map_err(|e| format!("download asset: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(format!("asset download HTTP {}: {}", status, body));
    }

    let mut out = Vec::new();
    resp.copy_to(&mut out)
        .map_err(|e| format!("read asset bytes: {e}"))?;
    Ok(out)
}

#[cfg(target_os = "windows")]
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("no parent directory for {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let tmp = path.with_extension(format!("tmp-{}", stamp));
    {
        let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())?;
    }
    if path.exists() {
        if fs::remove_file(path).is_err() {
            // File likely locked (running exe on Windows) — rename it out of the way.
            // Windows allows renaming a running exe, just not deleting it.
            let old = path.with_extension(format!("old-{}", stamp));
            fs::rename(path, &old)
                .map_err(|e| format!("failed to move locked file {}: {}", path.display(), e))?;
        }
    }
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Remove stale `.tmp-*` and `.old-*` files left by previous update attempts.
#[cfg(target_os = "windows")]
fn cleanup_stale_update_files(root: &Path) {
    let dirs_to_clean = [root.to_path_buf(), root.join("versions")];
    for dir in &dirs_to_clean {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let dominated = name.contains(".tmp-") || name.contains(".old-");
            let relevant = name.starts_with("Wuddle") || name.starts_with("wuddle");
            if dominated && relevant {
                let _ = fs::remove_file(entry.path());
            }
        }
        // Also clean inside version subdirectories
        if dir.ends_with("versions") {
            if let Ok(subdirs) = fs::read_dir(dir) {
                for subdir in subdirs.flatten() {
                    if !subdir.path().is_dir() {
                        continue;
                    }
                    if let Ok(files) = fs::read_dir(subdir.path()) {
                        for file in files.flatten() {
                            let n = file.file_name();
                            let n = n.to_string_lossy();
                            if (n.contains(".tmp-") || n.contains(".old-")) && n.starts_with("Wuddle") {
                                let _ = fs::remove_file(file.path());
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
struct ZipPayload {
    launcher_bytes: Option<Vec<u8>>,
    runtime_bytes: Vec<u8>,
    version_name: String,
}

#[cfg(target_os = "windows")]
fn extract_windows_payload_from_zip(
    zip_bytes: &[u8],
    fallback_version: &str,
) -> Result<ZipPayload, String> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| format!("open zip: {e}"))?;

    let fallback = normalize_release_tag(fallback_version);
    let mut launcher_bytes: Option<Vec<u8>> = None;
    let mut selected_runtime: Option<(String, Vec<u8>, bool)> = None;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("read zip entry: {e}"))?;
        if file.is_dir() {
            continue;
        }

        let raw_name = file.name().replace('\\', "/");
        let name = raw_name
            .trim_start_matches("./")
            .trim_matches('/')
            .to_string();
        let lower = name.to_ascii_lowercase();

        if lower == "wuddle.exe" {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .map_err(|e| format!("read launcher entry: {e}"))?;
            launcher_bytes = Some(bytes);
            continue;
        }

        let is_runtime = lower.ends_with("/wuddle-bin.exe") || lower == "wuddle-bin.exe";
        if !is_runtime {
            continue;
        }

        let parts: Vec<&str> = name.split('/').filter(|s| !s.is_empty()).collect();
        let mut version = fallback.clone();
        let mut from_versions_dir = false;
        if parts.len() >= 3
            && parts[0].eq_ignore_ascii_case("versions")
            && parts[parts.len() - 1].eq_ignore_ascii_case("Wuddle-bin.exe")
        {
            version = parts[1].to_string();
            from_versions_dir = true;
        }

        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|e| format!("read runtime entry: {e}"))?;

        match &selected_runtime {
            None => {
                selected_runtime = Some((version, bytes, from_versions_dir));
            }
            Some((_, _, had_from_versions)) if !had_from_versions && from_versions_dir => {
                selected_runtime = Some((version, bytes, from_versions_dir));
            }
            _ => {}
        }
    }

    let (version_name, runtime_bytes, _) =
        selected_runtime.ok_or_else(|| "no Wuddle-bin.exe found in update zip".to_string())?;

    let version_name = sanitize_version_folder_name(&version_name);
    let version_name = if version_name == "latest" {
        fallback
    } else {
        version_name
    };

    Ok(ZipPayload {
        launcher_bytes,
        runtime_bytes,
        version_name,
    })
}

#[cfg(target_os = "windows")]
fn write_current_pointer(root: &Path, version: &str) -> Result<(), String> {
    let content = serde_json::json!({ "current": version }).to_string();
    write_atomic(&root.join("current.json"), content.as_bytes())
}

/// Remove old version directories, keeping the `keep` highest versions.
/// Returns the names of removed version folders.
#[cfg(target_os = "windows")]
fn prune_old_versions(root: &Path, keep: usize) -> Result<Vec<String>, String> {
    let versions_dir = root.join("versions");
    if !versions_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<(Vec<u64>, String)> = Vec::new();
    let read_dir = fs::read_dir(&versions_dir).map_err(|e| e.to_string())?;
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let key = parse_version_key(name);
            if !key.is_empty() {
                entries.push((key, name.to_string()));
            }
        }
    }

    // Sort descending by version number.
    entries.sort_by(|a, b| b.0.cmp(&a.0));

    let mut removed = Vec::new();
    for (_key, name) in entries.into_iter().skip(keep) {
        let dir = versions_dir.join(&name);
        if fs::remove_dir_all(&dir).is_ok() {
            removed.push(name);
        }
    }
    Ok(removed)
}
