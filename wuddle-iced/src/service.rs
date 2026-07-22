//! Thin async wrappers around wuddle-engine.
//! Every function opens a fresh Engine (it's Send+!Sync due to rusqlite).

use crate::types::LogLevel;
use iced;
use pelite::{FileMap, PeFile};
use reqwest::Client;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use wuddle_engine::{CheckMode, Engine, InstallMode, InstallOptions, Repo, UpdatePlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RescanProgress {
    pub stage: String,
    pub detail: String,
}

fn rescan_progress_slot() -> &'static Mutex<Option<RescanProgress>> {
    static SLOT: OnceLock<Mutex<Option<RescanProgress>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

pub fn set_rescan_progress(stage: impl Into<String>, detail: impl Into<String>) {
    let stage = stage.into();
    let detail = detail.into();
    crate::diagnostics::trace("rescan", format!("progress: stage={stage}; detail omitted"));
    if let Ok(mut guard) = rescan_progress_slot().lock() {
        *guard = Some(RescanProgress { stage, detail });
    }
}

pub fn clear_rescan_progress() {
    if let Ok(mut guard) = rescan_progress_slot().lock() {
        *guard = None;
    }
}

pub fn latest_rescan_progress() -> Option<RescanProgress> {
    rescan_progress_slot()
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

#[derive(Debug, Clone)]
pub struct CollectionConflictOwnerGroup {
    pub repo_id: i64,
    pub repo_label: String,
    pub conflicting_addons: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum CollectionSelectionError {
    Conflict {
        repo_id: i64,
        repo_name: String,
        repo_url: String,
        selected_addons: Vec<String>,
        conflicts: Vec<wuddle_engine::AddonProbeConflict>,
        existing_repos: Vec<CollectionConflictOwnerGroup>,
    },
    Other(String),
}

#[derive(Debug, Clone)]
pub struct ReleaseAssetOption {
    pub name: String,
    pub tag: String,
    pub size: Option<u64>,
}

pub fn is_release_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("/releases")
}

pub fn is_direct_archive_url(url: &str) -> bool {
    wuddle_engine::is_direct_archive_url(url)
}

pub fn is_local_archive_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            let lower = name.to_ascii_lowercase();
            lower.ends_with(".zip") || lower.ends_with(".7z")
        })
        .unwrap_or(false)
}

fn is_archive_asset_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".7z")
}

fn release_tag_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/');
    let marker = "/releases/tag/";
    let idx = trimmed.to_ascii_lowercase().find(marker)?;
    let tag = &trimmed[idx + marker.len()..];
    tag.split('/')
        .next()
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(|tag| tag.to_string())
}

pub fn exact_asset_regex(asset_name: &str) -> String {
    let mut out = String::from("^");
    for ch in asset_name.chars() {
        match ch {
            '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out.push('$');
    out
}

pub fn root_probe_addon_names(probe: &wuddle_engine::AddonProbeResult) -> Vec<String> {
    let mut names = probe
        .addon_entries
        .iter()
        .filter(|entry| {
            let source = entry.source_path.trim().trim_matches('/');
            source.is_empty() || source == "."
        })
        .map(|entry| entry.addon_name.clone())
        .collect::<Vec<_>>();
    names.sort_by_key(|name| name.to_ascii_lowercase());
    names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    names
}

pub fn suggested_addon_for_expansion(
    options: &[String],
    expansion_hint: Option<&str>,
) -> Option<String> {
    let hint = expansion_hint?;
    let aliases: &[&str] = match hint {
        "vanilla" => &["vanilla", "classic", "era", "112", "1.12"],
        "tbc" => &["tbc", "bcc", "burning", "243", "2.4"],
        "wotlk" => &["wotlk", "wrath", "335", "3.3.5"],
        "cata" => &["cata", "cataclysm", "403", "4.0", "434", "4.3"],
        _ => &[],
    };

    options
        .iter()
        .find(|name| {
            let lower = name.to_ascii_lowercase();
            lower.contains(hint) || aliases.iter().any(|alias| lower.contains(alias))
        })
        .cloned()
}

#[cfg(test)]
mod primary_toc_tests {
    use super::{root_probe_addon_names, suggested_addon_for_expansion};
    use wuddle_engine::{AddonProbeEntry, AddonProbeResult};

    #[test]
    fn questie_root_tocs_are_choices_and_335_is_suggested_for_wotlk() {
        let probe = AddonProbeResult {
            addon_names: vec!["Questie".to_string(), "Questie-335".to_string()],
            addon_entries: vec![
                AddonProbeEntry {
                    addon_name: "Questie".to_string(),
                    source_path: String::new(),
                },
                AddonProbeEntry {
                    addon_name: "Questie-335".to_string(),
                    source_path: String::new(),
                },
            ],
            conflicts: Vec::new(),
            resolved_branch: "main".to_string(),
        };

        let options = root_probe_addon_names(&probe);
        assert_eq!(
            options,
            vec!["Questie".to_string(), "Questie-335".to_string()]
        );
        assert_eq!(
            suggested_addon_for_expansion(&options, Some("wotlk")),
            Some("Questie-335".to_string())
        );
    }
}

fn build_collection_conflict_owner_groups(
    conflicts: &[wuddle_engine::AddonProbeConflict],
) -> Vec<CollectionConflictOwnerGroup> {
    let mut groups = std::collections::BTreeMap::<i64, CollectionConflictOwnerGroup>::new();
    let mut untracked_locals = Vec::<String>::new();

    for conflict in conflicts {
        if conflict.owners.is_empty() {
            untracked_locals.push(conflict.addon_name.clone());
            continue;
        }

        for owner in &conflict.owners {
            let group =
                groups
                    .entry(owner.repo_id)
                    .or_insert_with(|| CollectionConflictOwnerGroup {
                        repo_id: owner.repo_id,
                        repo_label: format!("{}/{}", owner.owner, owner.name),
                        conflicting_addons: Vec::new(),
                    });

            if !group
                .conflicting_addons
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&conflict.addon_name))
            {
                group.conflicting_addons.push(conflict.addon_name.clone());
            }
        }
    }

    let mut out = groups.into_values().collect::<Vec<_>>();
    for group in &mut out {
        group
            .conflicting_addons
            .sort_by_key(|name| name.to_ascii_lowercase());
        group
            .conflicting_addons
            .dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    }

    if !untracked_locals.is_empty() {
        untracked_locals.sort_by_key(|name| name.to_ascii_lowercase());
        untracked_locals.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        out.push(CollectionConflictOwnerGroup {
            repo_id: 0,
            repo_label: "Untracked local folders".to_string(),
            conflicting_addons: untracked_locals,
        });
    }

    out
}

// ---------------------------------------------------------------------------
// Row types for the UI (Clone-friendly, owned data)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RepoRow {
    pub id: i64,
    pub forge: String,
    pub owner: String,
    pub name: String,
    pub url: String,
    pub mode: String,
    pub enabled: bool,
    pub last_version: Option<String>,
    pub git_branch: Option<String>,
    pub installed_branch: Option<String>,
    /// DLL files managed by this repo: (filename, is_enabled_in_dlls_txt, installed_version).
    /// Empty for non-DLL repos. More than one entry means this is a multi-DLL mod.
    pub installed_dlls: Vec<(String, bool, Option<String>)>,
    pub installed_addons: Vec<String>,
    pub installed_mpqs: Vec<wuddle_engine::mpq::MpqInstalledFile>,
    pub dependencies: Vec<(i64, String)>,
    pub selected_addons: Vec<String>,
    pub is_collection: bool,
    pub merge_installs: bool,
    pub pinned_version: Option<String>,
    pub installed_at_unix: Option<i64>,
    pub published_at_unix: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct RepoDetailEntry {
    pub path: String,
    pub kind: String,
    pub is_directory: bool,
}

#[derive(Debug, Clone)]
pub struct RepoDetailChild {
    pub name: String,
    pub is_directory: bool,
}

fn parse_selected_addons(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return Vec::new();
    };

    let mut parsed = serde_json::from_str::<Vec<String>>(raw).unwrap_or_default();
    parsed.retain(|name| !name.trim().is_empty());
    parsed.sort_by_key(|name| name.to_ascii_lowercase());
    parsed.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    parsed
}

impl From<Repo> for RepoRow {
    fn from(r: Repo) -> Self {
        // Normalize legacy "gitea" label for well-known hosts with their own brand.
        let forge = if r.forge == "gitea" && r.host.eq_ignore_ascii_case("codeberg.org") {
            "codeberg".to_string()
        } else {
            r.forge
        };
        Self {
            id: r.id,
            forge,
            owner: r.owner,
            name: r.name,
            url: r.url,
            mode: r.mode.as_str().to_string(),
            enabled: r.enabled,
            last_version: r.last_version,
            git_branch: r.git_branch,
            installed_branch: r
                .installed_asset_name
                .as_deref()
                .and_then(|url| url.strip_prefix("git:"))
                .map(|branch| branch.to_string()),
            installed_dlls: Vec::new(),
            installed_addons: Vec::new(),
            installed_mpqs: Vec::new(),
            dependencies: Vec::new(),
            selected_addons: parse_selected_addons(r.selected_addons_json.as_deref()),
            is_collection: r
                .selected_addons_json
                .as_deref()
                .map(str::trim)
                .map_or(false, |raw| !raw.is_empty()),
            merge_installs: r.merge_installs,
            pinned_version: r.pinned_version,
            installed_at_unix: r.installed_at_unix,
            published_at_unix: r.published_at_unix,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlanRow {
    pub repo_id: i64,
    pub owner: String,
    pub name: String,
    pub current: Option<String>,
    pub latest: String,
    pub asset_name: String,
    pub has_update: bool,
    pub repair_needed: bool,
    pub externally_modified: bool,
    pub not_modified: bool,
    pub mode: String,
    pub host: String,
    pub error: Option<String>,
    pub previous_dll_count: usize,
    pub new_dll_count: usize,
}

#[derive(Debug, Clone)]
pub struct RepoLoadLog {
    pub level: LogLevel,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct RepoLoadResult {
    pub rows: Vec<RepoRow>,
    pub untracked_mpqs: Vec<wuddle_engine::mpq::MpqProtectionEntry>,
    pub logs: Vec<RepoLoadLog>,
}

#[derive(Debug, Clone, Default)]
pub struct ClientVersionInfo {
    pub executable_path: String,
    pub executable_name: String,
    pub file_description: Option<String>,
    pub file_version: Option<String>,
    pub product_version: Option<String>,
    pub supports_legacy_1121_tweaks: bool,
    pub is_wotlk_335a_12340: bool,
    pub quick_add_family: ClientFamily,
}

/// Legacy WoW client families Wuddle can safely target with curated Quick Add mods.
/// Modern WoW Classic clients deliberately do not match their superficially similar
/// major version numbers because their engine and API are not compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClientFamily {
    Vanilla,
    Tbc,
    Wotlk,
    Unsupported,
    #[default]
    Unknown,
}

impl ClientFamily {
    pub fn label(self) -> &'static str {
        match self {
            Self::Vanilla => "Vanilla 1.12.1 or earlier",
            Self::Tbc => "The Burning Crusade 2.0–2.4.3",
            Self::Wotlk => "Wrath of the Lich King 3.0–3.3.5",
            Self::Unsupported => "an unsupported or modern WoW client",
            Self::Unknown => "an unknown WoW client",
        }
    }
}

fn classify_legacy_client(version: Option<(u16, u16, u16, u16)>) -> ClientFamily {
    let Some((major, minor, patch, _build)) = version else {
        return ClientFamily::Unknown;
    };

    match major {
        1 if minor < 12 || (minor == 12 && patch <= 1) => ClientFamily::Vanilla,
        2 if minor < 4 || (minor == 4 && patch <= 3) => ClientFamily::Tbc,
        3 if minor < 3 || (minor == 3 && patch <= 5) => ClientFamily::Wotlk,
        _ => ClientFamily::Unsupported,
    }
}

#[cfg(test)]
mod client_family_tests {
    use super::{classify_legacy_client, ClientFamily};

    #[test]
    fn classifies_only_supported_legacy_client_ranges() {
        assert_eq!(
            classify_legacy_client(Some((1, 12, 1, 5875))),
            ClientFamily::Vanilla
        );
        assert_eq!(
            classify_legacy_client(Some((2, 4, 3, 8606))),
            ClientFamily::Tbc
        );
        assert_eq!(
            classify_legacy_client(Some((3, 3, 5, 12340))),
            ClientFamily::Wotlk
        );
    }

    #[test]
    fn excludes_classic_and_newer_clients_with_similar_major_versions() {
        assert_eq!(
            classify_legacy_client(Some((1, 13, 0, 0))),
            ClientFamily::Unsupported
        );
        assert_eq!(
            classify_legacy_client(Some((2, 4, 4, 0))),
            ClientFamily::Unsupported
        );
        assert_eq!(
            classify_legacy_client(Some((3, 4, 0, 0))),
            ClientFamily::Unsupported
        );
        assert_eq!(classify_legacy_client(None), ClientFamily::Unknown);
    }
}

#[derive(Debug, Clone)]
pub enum CheckUpdatesStreamEvent {
    Progress(wuddle_engine::UpdateCheckProgress),
    Finished(Result<Vec<PlanRow>, String>),
}

#[derive(Debug, Clone)]
pub struct WdmReleaseAsset {
    pub name: String,
    pub download_url: String,
    pub size: Option<u64>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WdmReleaseSet {
    pub version: String,
    pub assets: Vec<WdmReleaseAsset>,
}

#[derive(Debug, Clone)]
pub struct WdmCatalog {
    pub locale: wuddle_engine::mpq::LocaleDetection,
    pub stable: WdmReleaseSet,
    pub caverns: Option<WdmReleaseSet>,
    pub addon: WdmReleaseSet,
}

pub const WDM_PATCH_URL: &str = "https://github.com/Trimitor/WDM-patch";
pub const EPOCH_WATER_URL: &str = "https://github.com/ZythDr/EpochWater";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CuratedMpqKind {
    Wdm,
    EpochWater,
}

impl CuratedMpqKind {
    pub fn readme_label(self) -> &'static str {
        match self {
            Self::Wdm => "WDM",
            Self::EpochWater => "Epoch Water",
        }
    }

    pub fn latest_label(self) -> &'static str {
        match self {
            Self::Wdm => "Latest WDM release",
            Self::EpochWater => "Latest Epoch Water source revision",
        }
    }
}

fn curated_mpq_kind_for_url(url: &str) -> Option<CuratedMpqKind> {
    let url = url.trim_end_matches('/');
    if url.eq_ignore_ascii_case(WDM_PATCH_URL) {
        Some(CuratedMpqKind::Wdm)
    } else if url.eq_ignore_ascii_case(EPOCH_WATER_URL) {
        Some(CuratedMpqKind::EpochWater)
    } else {
        None
    }
}

pub fn curated_mpq_kind(repo: &RepoRow) -> Option<CuratedMpqKind> {
    (repo.mode == "mpq")
        .then(|| curated_mpq_kind_for_url(&repo.url))
        .flatten()
}

fn curated_mpq_kind_for_repo(repo: &Repo) -> Option<CuratedMpqKind> {
    (repo.mode == InstallMode::Mpq)
        .then(|| curated_mpq_kind_for_url(&repo.url))
        .flatten()
}

pub fn is_wdm_repo(repo: &RepoRow) -> bool {
    curated_mpq_kind(repo) == Some(CuratedMpqKind::Wdm)
}

pub fn is_epoch_water_repo(repo: &RepoRow) -> bool {
    curated_mpq_kind(repo) == Some(CuratedMpqKind::EpochWater)
}

pub fn is_curated_mpq_repo(repo: &RepoRow) -> bool {
    curated_mpq_kind(repo).is_some()
}

impl WdmReleaseSet {
    pub fn locale_asset(&self, locale: &str, suffix: char) -> Option<&WdmReleaseAsset> {
        let expected = format!("patch-{locale}-{suffix}.MPQ");
        self.assets
            .iter()
            .find(|asset| asset.name.eq_ignore_ascii_case(&expected))
    }
}

static UPDATE_CHECK_PROGRESS: OnceLock<Mutex<Option<wuddle_engine::UpdateCheckProgress>>> =
    OnceLock::new();

fn update_check_progress_slot() -> &'static Mutex<Option<wuddle_engine::UpdateCheckProgress>> {
    UPDATE_CHECK_PROGRESS.get_or_init(|| Mutex::new(None))
}

fn set_update_check_progress(progress: Option<wuddle_engine::UpdateCheckProgress>) {
    if let Some(progress) = &progress {
        crate::diagnostics::trace(
            "update_check",
            format!(
                "progress: stage={:?}; mode={}; repository identity omitted",
                progress.stage, progress.mode
            ),
        );
    }
    if let Ok(mut slot) = update_check_progress_slot().lock() {
        *slot = progress;
    }
}

pub fn latest_update_check_progress() -> Option<wuddle_engine::UpdateCheckProgress> {
    update_check_progress_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

pub fn clear_update_check_progress() {
    set_update_check_progress(None);
}

fn first_existing_game_executable(dir: &Path) -> Option<PathBuf> {
    ["WoW.exe", "wow.exe", "Wow.exe", "WOW.EXE"]
        .iter()
        .map(|name| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn resolve_tweak_target_executable(
    wow_dir: &Path,
    auto_launch_exe: Option<&str>,
) -> Result<PathBuf, String> {
    if let Some(exe_name) = auto_launch_exe
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        let explicit = wow_dir.join(exe_name);
        if explicit.is_file() {
            return Ok(explicit);
        }
        return Err(format!(
            "{} not found in the specified directory.",
            exe_name
        ));
    }

    first_existing_game_executable(wow_dir)
        .ok_or_else(|| "WoW.exe not found in the specified directory.".to_string())
}

fn parse_version_tuple(raw: &str) -> Option<(u16, u16, u16, u16)> {
    let parts: Vec<u16> = raw
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u16>().ok())
        .collect();

    if parts.len() < 3 {
        return None;
    }

    Some((parts[0], parts[1], parts[2], *parts.get(3).unwrap_or(&0)))
}

pub async fn detect_tweak_client(
    wow_dir: String,
    auto_launch_exe: Option<String>,
) -> Result<ClientVersionInfo, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("detect_tweak_client");
    tokio::task::spawn_blocking(move || {
        let wow_path = Path::new(&wow_dir);
        let exe_path = resolve_tweak_target_executable(wow_path, auto_launch_exe.as_deref())?;
        let file_map = FileMap::open(&exe_path)
            .map_err(|e| format!("Failed to open {}: {e}", exe_path.display()))?;
        let pe = PeFile::from_bytes(&file_map).map_err(|e| {
            format!(
                "Failed to parse {} as a Windows executable: {e}",
                exe_path.display()
            )
        })?;

        let mut file_description = None;
        let mut file_version = None;
        let mut product_version = None;
        let mut version_tuple = None;

        if let Ok(resources) = pe.resources() {
            if let Ok(version_info) = resources.version_info() {
                if let Some(fixed) = version_info.fixed() {
                    version_tuple = Some((
                        fixed.dwFileVersion.Major,
                        fixed.dwFileVersion.Minor,
                        fixed.dwFileVersion.Patch,
                        fixed.dwFileVersion.Build,
                    ));
                    file_version = Some(format!(
                        "{}.{}.{}.{}",
                        fixed.dwFileVersion.Major,
                        fixed.dwFileVersion.Minor,
                        fixed.dwFileVersion.Patch,
                        fixed.dwFileVersion.Build,
                    ));
                    product_version = Some(format!(
                        "{}.{}.{}.{}",
                        fixed.dwProductVersion.Major,
                        fixed.dwProductVersion.Minor,
                        fixed.dwProductVersion.Patch,
                        fixed.dwProductVersion.Build,
                    ));
                }

                let file_info = version_info.file_info();
                if let Some(strings) = file_info.strings.values().next() {
                    file_description = strings.get("FileDescription").cloned();
                    if file_version.is_none() {
                        file_version = strings.get("FileVersion").cloned();
                    }
                    if product_version.is_none() {
                        product_version = strings.get("ProductVersion").cloned();
                    }
                }
            }
        }

        if version_tuple.is_none() {
            version_tuple = file_version
                .as_deref()
                .and_then(parse_version_tuple)
                .or_else(|| product_version.as_deref().and_then(parse_version_tuple));
        }

        let supports_legacy_1121_tweaks = version_tuple
            .map(|(major, minor, patch, _)| (major, minor, patch) == (1, 12, 1))
            .unwrap_or(false);
        let is_wotlk_335a_12340 = version_tuple == Some((3, 3, 5, 12340));

        Ok(ClientVersionInfo {
            executable_path: exe_path.to_string_lossy().to_string(),
            executable_name: exe_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "WoW.exe".to_string()),
            file_description,
            file_version,
            product_version,
            supports_legacy_1121_tweaks,
            is_wotlk_335a_12340,
            quick_add_family: classify_legacy_client(version_tuple),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

impl From<UpdatePlan> for PlanRow {
    fn from(p: UpdatePlan) -> Self {
        // Use the engine's authoritative signal: asset_url is non-empty iff something
        // needs to be downloaded. Exclude repair_needed (files missing but version
        // current) since that is not an "update". Mirrors Tauri's !p.asset_url.is_empty().
        let has_update = !p.asset_url.is_empty() && !p.repair_needed && p.error.is_none();
        Self {
            repo_id: p.repo_id,
            owner: p.owner,
            name: p.name,
            current: p.current,
            latest: p.latest,
            asset_name: p.asset_name,
            has_update,
            repair_needed: p.repair_needed,
            externally_modified: p.externally_modified,
            not_modified: p.not_modified,
            mode: p.mode.as_str().to_string(),
            host: p.host,
            error: p.error,
            previous_dll_count: p.previous_dll_count,
            new_dll_count: p.new_dll_count,
        }
    }
}

// ---------------------------------------------------------------------------
// Engine helpers
// ---------------------------------------------------------------------------

pub fn is_mod(repo: &RepoRow) -> bool {
    !matches!(repo.mode.as_str(), "addon" | "addon_git" | "manual")
}

fn open_engine(db_path: Option<&Path>) -> Result<Engine, String> {
    match db_path {
        Some(p) => Engine::open(p).map_err(|e| e.to_string()),
        None => Engine::open_default().map_err(|e| e.to_string()),
    }
}

pub async fn initialize_profile_database(
    db_path: PathBuf,
    wow_dir: String,
) -> Result<usize, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("initialize_profile_database");
    tokio::task::spawn_blocking(move || {
        let eng = Engine::open(&db_path).map_err(|e| e.to_string())?;
        let wow_dir = wow_dir.trim();
        if wow_dir.is_empty() {
            return Ok(0);
        }
        eng.import_existing_addons(Path::new(wow_dir))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn inspect_local_mpq(
    db_path: Option<PathBuf>,
    wow_dir: String,
    source: PathBuf,
) -> Result<wuddle_engine::mpq::MpqInspection, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("inspect_local_mpq");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.inspect_local_mpq_source(Path::new(&wow_dir), &source)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn install_local_mpq(
    db_path: Option<PathBuf>,
    wow_dir: String,
    source: PathBuf,
    selections: Vec<wuddle_engine::mpq::MpqInstallSelection>,
    set_xattr_comment: bool,
) -> Result<i64, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("install_local_mpq");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.install_local_mpq_package(Path::new(&wow_dir), &source, &selections, set_xattr_comment)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn preview_local_mpq_targets(
    db_path: Option<PathBuf>,
    wow_dir: String,
    source: PathBuf,
    selections: Vec<wuddle_engine::mpq::MpqInstallSelection>,
) -> Result<Vec<wuddle_engine::mpq::MpqTargetPreview>, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.preview_local_mpq_targets(Path::new(&wow_dir), &source, &selections)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn load_mpq_protection(
    db_path: Option<PathBuf>,
    wow_dir: String,
) -> Result<Vec<wuddle_engine::mpq::MpqProtectionEntry>, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.list_mpq_protection(Path::new(&wow_dir))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn detect_mpq_locale(
    db_path: Option<PathBuf>,
    wow_dir: String,
) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        let engine = open_engine(db_path.as_deref())?;
        let detection = engine.detect_wow_locale(Path::new(&wow_dir));
        if detection.ambiguous {
            return Ok(None);
        }
        Ok(detection
            .recommended
            .or_else(|| (detection.candidates.len() == 1).then(|| detection.candidates[0].clone())))
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn rescan_mpqs(db_path: Option<PathBuf>, wow_dir: String) -> Result<usize, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.list_mpq_protection(Path::new(&wow_dir))
            .map(|entries| entries.len())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn change_mpq_protection(
    db_path: Option<PathBuf>,
    wow_dir: String,
    path: String,
    protected: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_mpq_protected(Path::new(&wow_dir), &path, protected)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn set_untracked_mpq_editor_unlocked(
    db_path: Option<PathBuf>,
    wow_dir: String,
    path: String,
    editor_unlocked: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_untracked_mpq_editor_unlocked(Path::new(&wow_dir), &path, editor_unlocked)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn set_tracked_mpq_editor_unlocked(
    db_path: Option<PathBuf>,
    wow_dir: String,
    repo_id: i64,
    path: String,
    editor_unlocked: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_tracked_mpq_editor_unlocked(repo_id, Path::new(&wow_dir), &path, editor_unlocked)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn change_mpq_classification(
    db_path: Option<PathBuf>,
    wow_dir: String,
    path: String,
    core: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_mpq_core_classification(Path::new(&wow_dir), &path, core)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn set_untracked_mpq_enabled(
    db_path: Option<PathBuf>,
    wow_dir: String,
    path: String,
    enabled: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_untracked_mpq_enabled(Path::new(&wow_dir), &path, enabled)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn rename_untracked_mpq(
    db_path: Option<PathBuf>,
    wow_dir: String,
    path: String,
    display_name: String,
    set_xattr_comment: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.rename_untracked_mpq_display_name(
            Path::new(&wow_dir),
            &path,
            &display_name,
            set_xattr_comment,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn edit_untracked_mpq(
    db_path: Option<PathBuf>,
    wow_dir: String,
    path: String,
    display_name: String,
    file_name: String,
    destination: wuddle_engine::mpq::MpqDestination,
    core: bool,
    set_xattr_comment: bool,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.edit_untracked_mpq(
            Path::new(&wow_dir),
            &path,
            &display_name,
            &file_name,
            &destination,
            core,
            set_xattr_comment,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn rename_untracked_mpq_file(
    db_path: Option<PathBuf>,
    wow_dir: String,
    path: String,
    file_name: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.rename_untracked_mpq_file(Path::new(&wow_dir), &path, &file_name)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn rename_mpq_component(
    db_path: Option<PathBuf>,
    wow_dir: String,
    repo_id: i64,
    path: String,
    display_name: String,
    file_name: String,
    destination: wuddle_engine::mpq::MpqDestination,
    set_xattr_comment: bool,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.edit_tracked_mpq(
            repo_id,
            Path::new(&wow_dir),
            &path,
            &display_name,
            &file_name,
            &destination,
            set_xattr_comment,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn remove_mpq_component(
    db_path: Option<PathBuf>,
    wow_dir: String,
    repo_id: i64,
    path: String,
    force_modified: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.remove_mpq_component(repo_id, &path, Path::new(&wow_dir), force_modified)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn set_mpq_enabled(
    db_path: Option<PathBuf>,
    wow_dir: String,
    repo_id: i64,
    path: Option<String>,
    enabled: bool,
) -> Result<bool, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("set_mpq_enabled");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_mpq_enabled(repo_id, path.as_deref(), enabled, Path::new(&wow_dir))
            .map(|_| enabled)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn protect_modified_mpq(
    db_path: Option<PathBuf>,
    wow_dir: String,
    repo_id: i64,
    path: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.protect_modified_mpq(repo_id, &path, Path::new(&wow_dir))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn wdm_release_set(release: &wuddle_engine::LatestRelease, suffix: char) -> Option<WdmReleaseSet> {
    let assets = release
        .assets
        .iter()
        .filter(|asset| {
            let lower = asset.name.to_ascii_lowercase();
            lower.starts_with("patch-")
                && lower.ends_with(&format!("-{suffix}.mpq").to_ascii_lowercase())
        })
        .map(|asset| WdmReleaseAsset {
            name: asset.name.clone(),
            download_url: asset.download_url.clone(),
            size: asset.size,
            sha256: asset.sha256.clone(),
        })
        .collect::<Vec<_>>();
    (!assets.is_empty()).then(|| WdmReleaseSet {
        version: release.tag.clone(),
        assets,
    })
}

async fn resolve_wdm_stable(
    eng: &Engine,
) -> Result<(WdmReleaseSet, Vec<wuddle_engine::LatestRelease>), String> {
    let patch_releases = eng
        .list_releases(WDM_PATCH_URL)
        .await
        .map_err(|error| error.to_string())?;
    let stable = patch_releases
        .iter()
        .filter(|release| !release.prerelease)
        .find_map(|release| wdm_release_set(release, 'M'))
        .ok_or_else(|| "WDM has no stable locale-specific M patch release.".to_string())?;
    Ok((stable, patch_releases))
}

#[cfg(test)]
mod wdm_recipe_tests {
    use super::*;

    fn asset(name: &str) -> wuddle_engine::ReleaseAsset {
        wuddle_engine::ReleaseAsset {
            id: None,
            name: name.to_string(),
            download_url: format!("https://example.invalid/{name}"),
            size: None,
            content_type: None,
            sha256: None,
        }
    }

    #[test]
    fn selects_only_exact_locale_letter_assets() {
        let release = wuddle_engine::LatestRelease {
            tag: "current".to_string(),
            name: None,
            prerelease: false,
            assets: vec![
                asset("patch-enUS-M.MPQ"),
                asset("patch-deDE-M.MPQ"),
                asset("patch-enUS-N.MPQ"),
                asset("notes.zip"),
            ],
            published_at: None,
        };
        let main = wdm_release_set(&release, 'M').unwrap();
        assert_eq!(main.assets.len(), 2);
        assert_eq!(
            main.locale_asset("ENus", 'M')
                .map(|asset| asset.name.as_str()),
            Some("patch-enUS-M.MPQ")
        );
        assert!(main.locale_asset("frFR", 'M').is_none());
    }

    #[test]
    fn update_checks_use_the_main_wdm_patch_version_even_when_disabled() {
        let installs = [
            (
                "Data/enUS/patch-enUS-N.MPQ",
                Some("WDM Caverns & Mines"),
                Some("caverns-preview"),
            ),
            (
                "Data/enUS/renamed-main.MPQ.disabled",
                Some("WDM Dungeon Maps"),
                Some("v1.4.0"),
            ),
        ];
        assert_eq!(
            installed_wdm_main_version(installs).as_deref(),
            Some("v1.4.0")
        );
    }

    #[test]
    fn curated_updates_preserve_a_user_selected_target_name_and_destination() {
        let temp = tempfile::tempdir().unwrap();
        let engine = Engine::open(&temp.path().join("wuddle.sqlite")).unwrap();
        let repo_id = engine
            .add_repo(WDM_PATCH_URL, InstallMode::Mpq, None, None)
            .unwrap();
        engine
            .db()
            .add_install_with_hash(
                repo_id,
                "Data/enUS/patch-enUS-X.MPQ",
                "mpq",
                None,
                Some("v1.4.0"),
            )
            .unwrap();
        engine
            .db()
            .set_install_display_name(repo_id, "Data/enUS/patch-enUS-X.MPQ", "WDM Dungeon Maps")
            .unwrap();

        assert_eq!(
            saved_curated_mpq_target(&engine, WDM_PATCH_URL, "WDM Dungeon Maps"),
            Some((
                "patch-enUS-X.MPQ".to_string(),
                wuddle_engine::mpq::MpqDestination::Locale("enUS".to_string()),
            ))
        );
    }

    #[test]
    fn recognizes_the_supported_curated_mpq_sources() {
        assert_eq!(
            curated_mpq_kind_for_url("https://github.com/Trimitor/WDM-patch/"),
            Some(CuratedMpqKind::Wdm)
        );
        assert_eq!(
            curated_mpq_kind_for_url("https://github.com/ZythDr/EpochWater"),
            Some(CuratedMpqKind::EpochWater)
        );
        assert_eq!(
            curated_mpq_kind_for_url("https://github.com/example/other-patch"),
            None
        );
    }
}

pub async fn resolve_wdm(db_path: Option<PathBuf>, wow_dir: String) -> Result<WdmCatalog, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("resolve_wdm");
    let eng = open_engine(db_path.as_deref())?;
    let locale = eng.detect_wow_locale(Path::new(&wow_dir));
    let (stable, patch_releases) = resolve_wdm_stable(&eng).await?;
    let caverns = patch_releases
        .iter()
        .filter(|release| release.prerelease)
        .find_map(|release| wdm_release_set(release, 'N'));

    let addon_releases = eng
        .list_releases("https://github.com/Trimitor/WDM-addons")
        .await
        .map_err(|error| error.to_string())?;
    let addon_release = addon_releases
        .iter()
        .find(|release| !release.prerelease)
        .or_else(|| addon_releases.first())
        .ok_or_else(|| "WDM-addons has no release.".to_string())?;
    let addon_asset = addon_release
        .assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case("WDM.zip"))
        .ok_or_else(|| "The latest WDM-addons release has no WDM.zip asset.".to_string())?;
    let addon = WdmReleaseSet {
        version: addon_release.tag.clone(),
        assets: vec![WdmReleaseAsset {
            name: addon_asset.name.clone(),
            download_url: addon_asset.download_url.clone(),
            size: addon_asset.size,
            sha256: addon_asset.sha256.clone(),
        }],
    };
    Ok(WdmCatalog {
        locale,
        stable,
        caverns,
        addon,
    })
}

fn saved_curated_mpq_target(
    engine: &Engine,
    repository_url: &str,
    display_name: &str,
) -> Option<(String, wuddle_engine::mpq::MpqDestination)> {
    let repo = engine.db().list_repos().ok()?.into_iter().find(|repo| {
        repo.mode == InstallMode::Mpq
            && repo
                .url
                .trim_end_matches('/')
                .eq_ignore_ascii_case(repository_url.trim_end_matches('/'))
    })?;
    let install = engine
        .db()
        .list_installs(repo.id)
        .ok()?
        .into_iter()
        .find(|entry| {
            entry.kind == "mpq"
                && entry
                    .display_name
                    .as_deref()
                    .map(|name| name.eq_ignore_ascii_case(display_name))
                    .unwrap_or(false)
        })?;
    let enabled_path = install
        .path
        .strip_suffix(".disabled")
        .unwrap_or(&install.path);
    let path = Path::new(enabled_path);
    let file_name = path.file_name()?.to_str()?.to_string();
    let parts = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    let destination = if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("Data") {
        wuddle_engine::mpq::normalize_locale(parts[1])
            .map(wuddle_engine::mpq::MpqDestination::Locale)
            .unwrap_or(wuddle_engine::mpq::MpqDestination::DataRoot)
    } else {
        wuddle_engine::mpq::MpqDestination::DataRoot
    };
    Some((file_name, destination))
}

pub async fn install_wdm(
    db_path: Option<PathBuf>,
    wow_dir: String,
    catalog: WdmCatalog,
    locale: String,
    include_caverns: bool,
    install_addon: bool,
    options: InstallOptions,
) -> Result<i64, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("install_wdm");
    let eng = open_engine(db_path.as_deref())?;
    let wow_path = Path::new(&wow_dir);
    let locale = wuddle_engine::mpq::normalize_locale(&locale)
        .ok_or_else(|| "Choose a supported WoW locale.".to_string())?;
    let main = catalog
        .stable
        .locale_asset(&locale, 'M')
        .cloned()
        .ok_or_else(|| {
            format!(
                "WDM {} has no patch-{locale}-M.MPQ asset.",
                catalog.stable.version
            )
        })?;
    let caverns = if include_caverns {
        Some(
            catalog
                .caverns
                .as_ref()
                .and_then(|release| {
                    release
                        .locale_asset(&locale, 'N')
                        .map(|asset| (release, asset))
                })
                .ok_or_else(|| format!("No Caverns & Mines patch is available for {locale}."))?,
        )
    } else {
        None
    };
    if include_caverns && !install_addon {
        return Err("The WDM addon is required by Caverns & Mines.".to_string());
    }

    let addon_url = "https://github.com/Trimitor/WDM-addons";
    let existing_addon = eng
        .db()
        .list_repos()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|repo| {
            repo.url
                .trim_end_matches('/')
                .eq_ignore_ascii_case(addon_url)
        });
    let mut newly_installed_addon = None;
    if install_addon && existing_addon.is_none() {
        let addon_id = eng
            .add_repo(
                addon_url,
                InstallMode::Addon,
                Some(r"(?i)^WDM\.zip$".to_string()),
                None,
            )
            .map_err(|error| error.to_string())?;
        if let Err(error) = eng.reinstall_repo(addon_id, wow_path, None, options).await {
            let _ = eng.remove_repo(addon_id, Some(wow_path), true);
            return Err(format!(
                "The WDM companion addon could not be installed: {error}"
            ));
        }
        newly_installed_addon = Some(addon_id);
    }

    let destination = wuddle_engine::mpq::MpqDestination::Locale(locale.clone());
    let main_target = saved_curated_mpq_target(&eng, WDM_PATCH_URL, "WDM Dungeon Maps");
    let caverns_target = saved_curated_mpq_target(&eng, WDM_PATCH_URL, "WDM Caverns & Mines");
    let mut assets = vec![wuddle_engine::mpq::MpqRemoteAsset {
        asset_name: main.name.clone(),
        target_file_name: main_target.as_ref().map(|(name, _)| name.clone()),
        download_url: main.download_url.clone(),
        size: main.size,
        sha256: main.sha256.clone(),
        display_name: "WDM Dungeon Maps".to_string(),
        destination: main_target
            .as_ref()
            .map(|(_, destination)| destination.clone())
            .unwrap_or_else(|| destination.clone()),
        replace_unprotected: true,
        version: Some(catalog.stable.version.clone()),
    }];
    if let Some((release, asset)) = caverns {
        assets.push(wuddle_engine::mpq::MpqRemoteAsset {
            asset_name: asset.name.clone(),
            target_file_name: caverns_target.as_ref().map(|(name, _)| name.clone()),
            download_url: asset.download_url.clone(),
            size: asset.size,
            sha256: asset.sha256.clone(),
            display_name: "WDM Caverns & Mines".to_string(),
            destination: caverns_target
                .as_ref()
                .map(|(_, destination)| destination.clone())
                .unwrap_or(destination),
            replace_unprotected: true,
            version: Some(release.version.clone()),
        });
    }
    let package = wuddle_engine::mpq::MpqRemotePackage {
        url: WDM_PATCH_URL.to_string(),
        forge: "github".to_string(),
        host: "github.com".to_string(),
        owner: "Trimitor".to_string(),
        name: "WDM".to_string(),
    };
    let mpq_repo_id = match eng
        .install_remote_mpq_package(wow_path, package, &assets, options.set_xattr_comment)
        .await
    {
        Ok(repo_id) => repo_id,
        Err(error) => {
            if let Some(addon_id) = newly_installed_addon {
                let _ = eng.remove_repo(addon_id, Some(wow_path), true);
            }
            return Err(error.to_string());
        }
    };

    if !include_caverns {
        let stale = eng
            .list_installed_mpqs(mpq_repo_id, wow_path)
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|entry| entry.path.to_ascii_lowercase().ends_with("-n.mpq"));
        if let Some(stale) = stale {
            eng.remove_mpq_component(mpq_repo_id, &stale.path, wow_path, false)
                .map_err(|error| error.to_string())?;
        }
    }
    if let Some(addon_id) = newly_installed_addon {
        eng.record_repo_dependency(mpq_repo_id, addon_id, "wdm-companion")
            .map_err(|error| error.to_string())?;
    }
    Ok(mpq_repo_id)
}

#[derive(Debug, Deserialize)]
struct GithubSourceFile {
    #[serde(rename = "type")]
    kind: String,
    name: String,
    sha: String,
    size: u64,
    download_url: Option<String>,
}

#[derive(Debug, Clone)]
struct EpochWaterSource {
    version: String,
    download_url: String,
    size: u64,
}

async fn resolve_epoch_water_source() -> Result<EpochWaterSource, String> {
    const API_URL: &str =
        "https://api.github.com/repos/ZythDr/EpochWater/contents/patch-W.mpq?ref=main";
    const FALLBACK_DOWNLOAD_URL: &str =
        "https://raw.githubusercontent.com/ZythDr/EpochWater/main/patch-W.mpq";

    let mut request = Client::new()
        .get(API_URL)
        .header(
            "User-Agent",
            format!("Wuddle/{}", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = wuddle_engine::github_token() {
        request = request.bearer_auth(token);
    }
    let source = request
        .send()
        .await
        .map_err(|error| format!("Could not look up Epoch Water: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Could not look up Epoch Water: {error}"))?
        .json::<GithubSourceFile>()
        .await
        .map_err(|error| format!("Could not read Epoch Water source metadata: {error}"))?;
    if source.kind != "file" || !source.name.eq_ignore_ascii_case("patch-W.mpq") {
        return Err("Epoch Water's patch-W.mpq source file was not found.".to_string());
    }
    if source.sha.trim().is_empty() || source.size == 0 {
        return Err("Epoch Water's patch-W.mpq source file is invalid.".to_string());
    }
    Ok(EpochWaterSource {
        version: source.sha,
        download_url: source
            .download_url
            .unwrap_or_else(|| FALLBACK_DOWNLOAD_URL.to_string()),
        size: source.size,
    })
}

pub async fn install_epoch_water(
    db_path: Option<PathBuf>,
    wow_dir: String,
    options: InstallOptions,
) -> Result<i64, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("install_epoch_water");
    let source = resolve_epoch_water_source().await?;
    let eng = open_engine(db_path.as_deref())?;
    let saved_target = saved_curated_mpq_target(&eng, EPOCH_WATER_URL, "Epoch Water");
    let assets = [wuddle_engine::mpq::MpqRemoteAsset {
        asset_name: "patch-W.mpq".to_string(),
        target_file_name: saved_target.as_ref().map(|(name, _)| name.clone()),
        download_url: source.download_url,
        size: Some(source.size),
        // GitHub's source blob ID is not a SHA-256 checksum of the download.
        // Keep it as the installed version and let the engine validate the MPQ.
        sha256: None,
        display_name: "Epoch Water".to_string(),
        destination: saved_target
            .map(|(_, destination)| destination)
            .unwrap_or(wuddle_engine::mpq::MpqDestination::DataRoot),
        replace_unprotected: true,
        version: Some(source.version),
    }];
    let package = wuddle_engine::mpq::MpqRemotePackage {
        url: EPOCH_WATER_URL.to_string(),
        forge: "github".to_string(),
        host: "github.com".to_string(),
        owner: "ZythDr".to_string(),
        name: "Epoch Water".to_string(),
    };
    eng.install_remote_mpq_package(
        Path::new(&wow_dir),
        package,
        &assets,
        options.set_xattr_comment,
    )
    .await
    .map_err(|error| error.to_string())
}

pub async fn remove_wdm(
    db_path: Option<PathBuf>,
    wow_dir: String,
    mpq_repo_id: i64,
    addon_repo_id: i64,
    remove_addon: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let wow = Path::new(&wow_dir);
        eng.remove_mpq_package(mpq_repo_id, wow, false)
            .map_err(|error| error.to_string())?;
        let addon_exists = { eng.db().get_repo(addon_repo_id).is_ok() };
        if remove_addon && addon_exists {
            eng.remove_repo(addon_repo_id, Some(wow), true)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}

// ---------------------------------------------------------------------------
// Repo queries
// ---------------------------------------------------------------------------

/// Best-effort fix: re-fetch correct owner/name casing from each forge API.
/// Called during rescan so repos lowercased by the v4 migration get corrected.
/// Only queries the API for repos whose owner or name are entirely lowercase
/// (indicating they were likely lowercased by the v4 migration).
fn fix_repo_casing_from_forges(eng: &Engine) {
    let repos = match eng.db().list_repos() {
        Ok(r) => r,
        Err(_) => return,
    };

    // Only fix repos that look like they were lowercased by the migration.
    let needs_fix: Vec<&Repo> = repos
        .iter()
        .filter(|r| {
            let owner_lower = r.owner == r.owner.to_ascii_lowercase()
                && r.owner.chars().any(|c| c.is_ascii_alphabetic());
            let name_lower = r.name == r.name.to_ascii_lowercase()
                && r.name.chars().any(|c| c.is_ascii_alphabetic());
            owner_lower || name_lower
        })
        .collect();

    if needs_fix.is_empty() {
        return;
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    let ua = format!("Wuddle/{}", env!("CARGO_PKG_VERSION"));
    let gh_token = wuddle_engine::github_token();

    for repo in &needs_fix {
        let (new_owner, new_name) = match repo.forge.as_str() {
            "github" => {
                let api_url = format!("https://api.github.com/repos/{}/{}", repo.owner, repo.name);
                let mut req = client
                    .get(&api_url)
                    .header("User-Agent", &ua)
                    .header("Accept", "application/vnd.github+json");
                if let Some(ref token) = gh_token {
                    req = req.bearer_auth(token);
                }
                match req.send() {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(json) = resp.json::<serde_json::Value>() {
                            let owner = json["owner"]["login"]
                                .as_str()
                                .unwrap_or(&repo.owner)
                                .to_string();
                            let name = json["name"].as_str().unwrap_or(&repo.name).to_string();
                            (owner, name)
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                }
            }
            "gitea" => {
                let api_url = format!(
                    "https://{}/api/v1/repos/{}/{}",
                    repo.host, repo.owner, repo.name
                );
                let req = client.get(&api_url).header("User-Agent", &ua);
                match req.send() {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(json) = resp.json::<serde_json::Value>() {
                            let owner = json["owner"]["login"]
                                .as_str()
                                .unwrap_or(&repo.owner)
                                .to_string();
                            let name = json["name"].as_str().unwrap_or(&repo.name).to_string();
                            (owner, name)
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                }
            }
            "gitlab" => {
                let encoded = format!("{}/{}", repo.owner, repo.name).replace('/', "%2F");
                let api_url = format!("https://{}/api/v4/projects/{}", repo.host, encoded);
                let req = client.get(&api_url).header("User-Agent", &ua);
                match req.send() {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(json) = resp.json::<serde_json::Value>() {
                            if let Some(full_path) = json["path_with_namespace"].as_str() {
                                let parts: Vec<&str> = full_path.rsplitn(2, '/').collect();
                                if parts.len() == 2 {
                                    (parts[1].to_string(), parts[0].to_string())
                                } else {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                }
            }
            _ => continue,
        };

        if new_owner != repo.owner || new_name != repo.name {
            let _ = eng.db().update_repo_casing(repo.id, &new_owner, &new_name);
        }

        // Rate limit: 1.5s delay between requests to avoid hammering APIs
        std::thread::sleep(Duration::from_millis(1500));
    }
}

pub async fn list_repos(
    db_path: Option<PathBuf>,
    wow_dir: Option<String>,
    fix_casing: bool,
) -> Result<RepoLoadResult, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("list_repos");
    clear_rescan_progress();
    set_rescan_progress("Repository load", "Resolving WoW directory...");

    // No wow_dir means no WoW installation configured — return empty list
    let dir = match wow_dir.as_deref() {
        Some(d) if !d.trim().is_empty() => d,
        _ => {
            clear_rescan_progress();
            return Ok(RepoLoadResult {
                rows: Vec::new(),
                untracked_mpqs: Vec::new(),
                logs: Vec::new(),
            });
        }
    };
    let wow_path_buf = PathBuf::from(dir);
    let db_existed_before_open = db_path.as_ref().map(|p| p.exists()).unwrap_or(true);
    set_rescan_progress("Repository load", "Opening profile database...");
    let eng = open_engine(db_path.as_deref())?;
    let mut logs = Vec::new();

    set_rescan_progress("Repository load", "Checking tracked repositories...");
    let repo_count = eng.db().list_repos().map_err(|e| e.to_string())?.len();
    if !db_existed_before_open || repo_count == 0 {
        set_rescan_progress("Repository load", "Importing existing addon folders...");
        let imported = eng
            .import_existing_addons_with_progress(&wow_path_buf, |detail| {
                set_rescan_progress("Repository import", detail);
            })
            .map_err(|e| e.to_string())?;
        if !db_existed_before_open || imported > 0 {
            let text = if !db_existed_before_open {
                format!(
                    "Initialized profile database and imported {} existing addon repo(s).",
                    imported
                )
            } else {
                format!("Imported {} existing addon repo(s).", imported)
            };
            logs.push(RepoLoadLog {
                level: LogLevel::Info,
                text,
            });
        }
    }

    // Cheap tracked-link verification runs on normal refresh/load.
    // Full repair/reconciliation stays behind explicit Rescan only.
    if !fix_casing {
        set_rescan_progress("Repository load", "Verifying tracked addon links...");
        let eng_clone = eng.clone();
        let verify_path = wow_path_buf.clone();
        let repaired = tokio::task::spawn_blocking(move || {
            eng_clone.verify_and_repair_tracked_addon_links(&verify_path)
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
        if repaired > 0 {
            logs.push(RepoLoadLog {
                level: LogLevel::Info,
                text: format!(
                    "Verified tracked addon links and repaired {} broken entry(s).",
                    repaired
                ),
            });
        }
    }

    // Perform authoritative repairs only if explicitly requested via Rescan.
    // This handles casing, symlinks, and missing files/repos.
    if fix_casing {
        set_rescan_progress("Rescan", "Repairing broken installations...");
        logs.push(RepoLoadLog {
            level: LogLevel::Info,
            text: "Rescan: repairing broken installations...".to_string(),
        });
        let started = Instant::now();
        match eng.repair_broken_installations(&wow_path_buf).await {
            Ok(fixed) => logs.push(RepoLoadLog {
                level: LogLevel::Info,
                text: format!(
                    "Rescan: repair phase finished in {}ms ({} change(s)).",
                    started.elapsed().as_millis(),
                    fixed
                ),
            }),
            Err(err) => logs.push(RepoLoadLog {
                level: LogLevel::Error,
                text: format!(
                    "Rescan: repair phase failed after {}ms: {}",
                    started.elapsed().as_millis(),
                    err
                ),
            }),
        }
    }

    let mut background_logs = tokio::task::spawn_blocking(move || {
        let wow_path = wow_path_buf.as_path();
        let mut logs = Vec::new();

        set_rescan_progress("Repository load", "Cleaning casing collisions...");
        let started = Instant::now();
        let cleaned = eng.cleanup_casing_collisions(wow_path).unwrap_or(0);
        logs.push(RepoLoadLog {
            level: LogLevel::Info,
            text: format!(
                "Refresh: casing cleanup finished in {}ms ({} change(s)).",
                started.elapsed().as_millis(),
                cleaned
            ),
        });

        // Restore correct capitalization from disk (.toc files/folders for addons).
        // This is fast and runs on every refresh to satisfy the requirement that
        // the list matches disk casing.
        // Heavy maintenance tasks: only run during a full rescan or the one-time v4 migration.
        // This keeps the standard launch and refresh cycles fast and prevents
        // deleted repos from being automatically re-imported.
        if fix_casing || eng.db().needs_casing_fix() {
            set_rescan_progress("Rescan", "Pruning missing repositories...");
            let started = Instant::now();
            // Prune repos whose files no longer exist on disk
            let pruned = eng.prune_missing_repos(wow_path).unwrap_or(0);
            logs.push(RepoLoadLog {
                level: LogLevel::Info,
                text: format!(
                    "Rescan: prune phase finished in {}ms ({} repo(s) removed).",
                    started.elapsed().as_millis(),
                    pruned
                ),
            });

            set_rescan_progress("Rescan", "Importing newly discovered addon repos...");
            let started = Instant::now();
            // Auto-import newly discovered addon git repos
            let imported = eng
                .import_existing_addons_with_progress(wow_path, |detail| {
                    set_rescan_progress("Rescan import", detail);
                })
                .unwrap_or(0);
            logs.push(RepoLoadLog {
                level: LogLevel::Info,
                text: format!(
                    "Rescan: import phase finished in {}ms ({} repo(s) added).",
                    started.elapsed().as_millis(),
                    imported
                ),
            });

            set_rescan_progress("Rescan", "Removing duplicate addon repo records...");
            let started = Instant::now();
            // Remove duplicate tracking entries
            let deduped = eng.dedup_addon_repos_by_folder(wow_path).unwrap_or(0);
            logs.push(RepoLoadLog {
                level: LogLevel::Info,
                text: format!(
                    "Rescan: dedup phase finished in {}ms ({} duplicate repo(s) removed).",
                    started.elapsed().as_millis(),
                    deduped
                ),
            });
        }

        // Fix repo owner/name casing from forge APIs (best-effort).
        // On first launch after the v4 migration (needs_casing_fix), always run.
        // Otherwise only run when explicitly requested (manual rescan).
        // Spawning in a background thread to avoid blocking the main rescan loop.
        if fix_casing || eng.db().needs_casing_fix() {
            let db_clone = db_path.clone();
            std::thread::spawn(move || {
                if let Ok(e) = open_engine(db_clone.as_deref()) {
                    fix_repo_casing_from_forges(&e);
                    let _ = e.db().mark_casing_fixed();
                }
            });
        }
        set_rescan_progress("Repository load", "Reading repository records...");
        let repos = eng.db().list_repos().map_err(|e| e.to_string())?;

        // Legacy Vanilla launchers use dlls.txt. Later clients load proxy DLLs
        // directly, so their disabled state is represented by a .disabled suffix.
        set_rescan_progress("Repository load", "Reading installed DLL state...");
        let dlls_txt_path = wow_path.join("dlls.txt");
        let uses_dlls_txt = dlls_txt_path.is_file();
        let dlls_txt = std::fs::read_to_string(&dlls_txt_path).unwrap_or_default();
        let enabled_dlls: std::collections::HashSet<String> = dlls_txt
            .lines()
            .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
            .map(|l| l.trim().to_lowercase())
            .collect();

        set_rescan_progress("Repository load", "Building repository rows...");
        let mut rows: Vec<RepoRow> = Vec::with_capacity(repos.len());
        for repo in repos {
            let mut row = RepoRow::from(repo);
            let installs = eng.db().list_installs(row.id).unwrap_or_default();
            row.installed_dlls = installs
                .iter()
                .filter(|e| e.kind == "dll")
                .filter_map(|e| {
                    let fname = std::path::Path::new(&e.path)
                        .file_name()?
                        .to_str()?
                        .to_string();
                    let is_enabled = if wow_path.join(format!("{fname}.disabled")).is_file() {
                        false
                    } else if uses_dlls_txt {
                        enabled_dlls.contains(&fname.to_lowercase())
                    } else {
                        wow_path.join(&fname).is_file()
                    };
                    Some((fname, is_enabled, e.version.clone()))
                })
                .collect();
            row.installed_addons = installs
                .into_iter()
                .filter(|e| e.kind == "addon")
                .filter_map(|e| {
                    std::path::Path::new(&e.path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.to_string())
                })
                .collect();
            row.installed_addons
                .sort_by_key(|name| name.to_ascii_lowercase());
            row.installed_addons
                .dedup_by(|left, right| left.eq_ignore_ascii_case(right));
            if row.mode == "mpq" {
                row.installed_mpqs = eng
                    .list_installed_mpqs(row.id, wow_path)
                    .unwrap_or_default();
                row.dependencies = eng.repo_dependencies(row.id).unwrap_or_default();
            }
            rows.push(row);
        }
        set_rescan_progress("Repository load", "Scanning custom MPQs...");
        let untracked_mpqs = eng
            .list_mpq_protection(wow_path)
            .map_err(|error| error.to_string())?;
        Ok::<RepoLoadResult, String>(RepoLoadResult {
            rows,
            untracked_mpqs,
            logs,
        })
    })
    .await
    .map_err(|e| e.to_string())??;

    logs.append(&mut background_logs.logs);
    set_rescan_progress("Repository load", "Finished loading repositories.");
    Ok(RepoLoadResult {
        rows: background_logs.rows,
        untracked_mpqs: background_logs.untracked_mpqs,
        logs,
    })
}

pub async fn check_updates(
    db_path: Option<PathBuf>,
    wow_dir: Option<String>,
    mode: CheckMode,
) -> Result<Vec<PlanRow>, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("check_updates");
    check_updates_skip(db_path, wow_dir, mode, std::collections::HashSet::new()).await
}

async fn build_wdm_update_plan(eng: &Engine, repo: Repo) -> PlanRow {
    let installs = eng.db().list_installs(repo.id).unwrap_or_default();
    let current =
        installed_wdm_main_version(installs.iter().filter(|entry| entry.kind == "mpq").map(
            |entry| {
                (
                    entry.path.as_str(),
                    entry.display_name.as_deref(),
                    entry.version.as_deref(),
                )
            },
        ));
    match resolve_wdm_stable(eng).await {
        Ok((stable, _)) => {
            let has_update = current.as_deref() != Some(stable.version.as_str());
            PlanRow {
                repo_id: repo.id,
                owner: repo.owner,
                name: repo.name,
                current,
                latest: stable.version,
                asset_name: "Locale-specific WDM patch".to_string(),
                has_update,
                repair_needed: false,
                externally_modified: false,
                not_modified: !has_update,
                mode: "mpq".to_string(),
                host: repo.host,
                error: None,
                previous_dll_count: 0,
                new_dll_count: 0,
            }
        }
        Err(error) => PlanRow {
            repo_id: repo.id,
            owner: repo.owner,
            name: repo.name,
            current,
            latest: String::new(),
            asset_name: String::new(),
            has_update: false,
            repair_needed: false,
            externally_modified: false,
            not_modified: false,
            mode: "mpq".to_string(),
            host: repo.host,
            error: Some(error),
            previous_dll_count: 0,
            new_dll_count: 0,
        },
    }
}

async fn build_epoch_water_update_plan(repo: Repo) -> PlanRow {
    let current = repo.last_version.clone();
    match resolve_epoch_water_source().await {
        Ok(source) => {
            let has_update = current.as_deref() != Some(source.version.as_str());
            PlanRow {
                repo_id: repo.id,
                owner: repo.owner,
                name: repo.name,
                current,
                latest: source.version,
                asset_name: "patch-W.mpq".to_string(),
                has_update,
                repair_needed: false,
                externally_modified: false,
                not_modified: !has_update,
                mode: "mpq".to_string(),
                host: repo.host,
                error: None,
                previous_dll_count: 0,
                new_dll_count: 0,
            }
        }
        Err(error) => PlanRow {
            repo_id: repo.id,
            owner: repo.owner,
            name: repo.name,
            current,
            latest: String::new(),
            asset_name: String::new(),
            has_update: false,
            repair_needed: false,
            externally_modified: false,
            not_modified: false,
            mode: "mpq".to_string(),
            host: repo.host,
            error: Some(error),
            previous_dll_count: 0,
            new_dll_count: 0,
        },
    }
}

fn installed_wdm_main_version<'a>(
    installs: impl IntoIterator<Item = (&'a str, Option<&'a str>, Option<&'a str>)>,
) -> Option<String> {
    let mut legacy_match = None;
    for (path, display_name, version) in installs {
        if display_name
            .map(|name| name.eq_ignore_ascii_case("WDM Dungeon Maps"))
            .unwrap_or(false)
        {
            return version.map(str::to_string);
        }
        if legacy_match.is_none() {
            let path = path.to_ascii_lowercase();
            let enabled_path = path.strip_suffix(".disabled").unwrap_or(&path);
            if enabled_path.ends_with("-m.mpq") {
                legacy_match = version.map(str::to_string);
            }
        }
    }
    legacy_match
}

pub async fn check_updates_skip(
    db_path: Option<PathBuf>,
    wow_dir: Option<String>,
    mode: CheckMode,
    skip_repo_ids: std::collections::HashSet<i64>,
) -> Result<Vec<PlanRow>, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("check_updates_skip");
    clear_update_check_progress();
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let curated_repos = eng
            .db()
            .list_repos()
            .map_err(|error| error.to_string())?
            .into_iter()
            .filter_map(|repo| curated_mpq_kind_for_repo(&repo).map(|kind| (repo, kind)))
            .collect::<Vec<_>>();
        // Curated MPQs use their own source/release selection, so keep them out
        // of the generic forge updater and append purpose-built plans below.
        let mut engine_skip_repo_ids = skip_repo_ids.clone();
        for (repo, _) in &curated_repos {
            engine_skip_repo_ids.insert(repo.id);
        }
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
        let progress_forwarder = std::thread::spawn(move || {
            while let Some(progress) = progress_rx.blocking_recv() {
                set_update_check_progress(Some(progress));
            }
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let plans = runtime
            .block_on(async {
                eng.check_updates_with_wow_skip_progress(
                    wow_dir.as_deref().map(Path::new),
                    mode,
                    &engine_skip_repo_ids,
                    progress_tx,
                )
                .await
            })
            .map_err(|e| e.to_string())?;
        let mut rows = plans.into_iter().map(PlanRow::from).collect::<Vec<_>>();
        for (repo, kind) in curated_repos
            .into_iter()
            .filter(|(repo, _)| !skip_repo_ids.contains(&repo.id))
        {
            rows.retain(|plan| plan.repo_id != repo.id);
            rows.push(match kind {
                CuratedMpqKind::Wdm => runtime.block_on(build_wdm_update_plan(&eng, repo)),
                CuratedMpqKind::EpochWater => runtime.block_on(build_epoch_water_update_plan(repo)),
            });
        }
        let _ = progress_forwarder.join();
        clear_update_check_progress();
        Ok(rows)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------

pub async fn add_repo(
    db_path: Option<PathBuf>,
    url: String,
    mode: String,
    asset_regex: Option<String>,
    selected_addons: Option<Vec<String>>,
) -> Result<i64, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("add_repo");
    crate::diagnostics::trace(
        "service",
        format!(
            "add_repo: mode={mode}; selected_addons={}; asset_filter={}",
            selected_addons.as_ref().map(Vec::len).unwrap_or(0),
            asset_regex.is_some()
        ),
    );
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let install_mode = InstallMode::from_str(&mode).unwrap_or(InstallMode::Auto);
        if wuddle_engine::is_direct_archive_url(&url) {
            return eng.add_direct_archive_url(&url).map_err(|e| e.to_string());
        }
        eng.add_repo(&url, install_mode, asset_regex, selected_addons)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn add_local_archive_file(
    db_path: Option<PathBuf>,
    path: PathBuf,
) -> Result<i64, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("add_local_archive_file");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.add_local_archive_file(&path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn update_collection_selection(
    db_path: Option<PathBuf>,
    repo_id: i64,
    wow_dir: String,
    selected_addons: Vec<String>,
    opts: InstallOptions,
) -> Result<String, CollectionSelectionError> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("update_collection_selection");
    crate::diagnostics::trace(
        "service",
        format!(
            "update_collection_selection: repo_id={repo_id}; selected_count={}; replace_conflicts={}",
            selected_addons.len(),
            opts.replace_addon_conflicts
        ),
    );
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref()).map_err(CollectionSelectionError::Other)?;
        let repo = eng
            .db()
            .get_repo(repo_id)
            .map_err(|e| CollectionSelectionError::Other(e.to_string()))?;
        let previous_selected = parse_selected_addons(repo.selected_addons_json.as_deref());

        if !opts.replace_addon_conflicts {
            let conflicts = eng
                .addon_selection_conflicts(repo_id, Path::new(&wow_dir), &selected_addons)
                .map_err(|e| CollectionSelectionError::Other(e.to_string()))?;
            if !conflicts.is_empty() {
                let existing_repos = build_collection_conflict_owner_groups(&conflicts);
                return Err(CollectionSelectionError::Conflict {
                    repo_id,
                    repo_name: format!("{}/{}", repo.owner, repo.name),
                    repo_url: repo.url.clone(),
                    selected_addons,
                    conflicts,
                    existing_repos,
                });
            }
        }

        eng.set_repo_selected_addons(repo_id, Some(selected_addons.clone()))
            .map_err(|e| CollectionSelectionError::Other(e.to_string()))?;

        if selected_addons.is_empty() {
            eng.remove_repo(repo_id, Some(Path::new(&wow_dir)), true)
                .map_err(|e| CollectionSelectionError::Other(e.to_string()))?;
            return Ok(format!("Removed collection {}/{}.", repo.owner, repo.name));
        }

        let reinstall_result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| CollectionSelectionError::Other(e.to_string()))?
            .block_on(async {
                eng.reinstall_repo(repo_id, Path::new(&wow_dir), None, opts)
                    .await
            });

        let plan = match reinstall_result {
            Ok(plan) => plan,
            Err(e) => {
                let _ = eng.set_repo_selected_addons(
                    repo_id,
                    if previous_selected.is_empty() {
                        None
                    } else {
                        Some(previous_selected)
                    },
                );
                return Err(CollectionSelectionError::Other(e.to_string()));
            }
        };

        Ok(format!(
            "Updated collection selection for {}/{}.",
            plan.owner, plan.name
        ))
    })
    .await
    .map_err(|e| CollectionSelectionError::Other(e.to_string()))?
}

pub async fn probe_conflicts(
    db_path: Option<PathBuf>,
    url: String,
    wow_dir: String,
) -> Result<wuddle_engine::AddonProbeResult, String> {
    probe_conflicts_on_branch(db_path, url, wow_dir, None).await
}

pub async fn probe_conflicts_on_branch(
    db_path: Option<PathBuf>,
    url: String,
    wow_dir: String,
    preferred_branch: Option<String>,
) -> Result<wuddle_engine::AddonProbeResult, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("probe_conflicts");
    // NOTE: probe_addon_repo_conflicts is async, so we can't simply call it inside
    // spawn_blocking. Using Handle::current().block_on() inside spawn_blocking would
    // deadlock because both sides wait on the same Tokio runtime. Instead we build a
    // fresh, isolated current-thread runtime inside the blocking task.
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let normalized_url = normalize_repo_input_url(&url);
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?
            .block_on(async {
                eng.probe_addon_repo_conflicts(
                    &normalized_url,
                    Path::new(&wow_dir),
                    preferred_branch.as_deref(),
                )
                .await
            })
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r)
}

/// Result of the lightweight pre-install conflict check.
#[derive(Debug, Clone)]
pub struct PreInstallConflictInfo {
    pub conflicts: Vec<wuddle_engine::AddonProbeConflict>,
    pub existing_repos: Vec<CollectionConflictOwnerGroup>,
    pub new_repo_label: String,
    pub addon_names: Vec<String>,
}

/// Lightweight pre-install conflict check that runs after `add_repo` but before
/// `install_new_repo`. Uses the engine's DB + filesystem queries (no network call)
/// to detect whether the repo's target files already exist or are tracked by
/// another repository.
pub async fn check_pre_install_conflicts(
    db_path: Option<PathBuf>,
    repo_id: i64,
    wow_dir: String,
    addon_names: Vec<String>,
) -> Result<PreInstallConflictInfo, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("check_pre_install_conflicts");
    crate::diagnostics::trace(
        "service",
        format!(
            "check_pre_install_conflicts: repo_id={repo_id}; addon_count={}",
            addon_names.len()
        ),
    );
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let repo = eng.db().get_repo(repo_id).map_err(|e| e.to_string())?;

        let names_to_check = if addon_names.is_empty() {
            vec![repo.name.clone()]
        } else {
            addon_names
        };

        let conflicts = eng
            .addon_selection_conflicts(repo_id, Path::new(&wow_dir), &names_to_check)
            .map_err(|e| e.to_string())?;

        let existing_repos = if conflicts.is_empty() {
            Vec::new()
        } else {
            build_collection_conflict_owner_groups(&conflicts)
        };

        Ok(PreInstallConflictInfo {
            conflicts,
            existing_repos,
            new_repo_label: format!("{}/{}", repo.owner, repo.name),
            addon_names: names_to_check,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn remove_repo(
    db_path: Option<PathBuf>,
    id: i64,
    wow_dir: Option<String>,
    remove_local_files: bool,
) -> Result<(), String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("remove_repo");
    crate::diagnostics::trace(
        "service",
        format!("remove_repo: repo_id={id}; remove_local_files={remove_local_files}"),
    );
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.remove_repo(id, wow_dir.as_deref().map(Path::new), remove_local_files)
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn set_repo_enabled(
    db_path: Option<PathBuf>,
    id: i64,
    enabled: bool,
    wow_dir: String,
    use_dlls_txt: bool,
) -> Result<(), String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("set_repo_enabled");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let wow_path = (!wow_dir.trim().is_empty()).then(|| Path::new(&wow_dir));
        eng.set_repo_enabled(id, enabled, wow_path, use_dlls_txt)
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn is_awesome_wotlk_repo(db_path: Option<PathBuf>, repo_id: i64) -> bool {
    tokio::task::spawn_blocking(move || {
        let Ok(engine) = open_engine(db_path.as_deref()) else {
            return false;
        };
        let Ok(repo) = engine.db().get_repo(repo_id) else {
            return false;
        };
        repo.url
            .eq_ignore_ascii_case("https://github.com/noname08662/awesome_wotlk")
            || repo.name.eq_ignore_ascii_case("awesome_wotlk")
    })
    .await
    .unwrap_or(false)
}

/// Returns all installed files for a repo as (path_relative_to_wow_root, kind) pairs.
pub async fn list_repo_installs(
    db_path: Option<PathBuf>,
    repo_id: i64,
) -> Result<Vec<(String, String)>, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("list_repo_installs");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let entries = eng.db().list_installs(repo_id).map_err(|e| e.to_string())?;
        Ok(entries.into_iter().map(|e| (e.path, e.kind)).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn load_repo_details(
    db_path: Option<PathBuf>,
    repo_id: i64,
    wow_dir: PathBuf,
) -> Result<Vec<RepoDetailEntry>, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("load_repo_details");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let mut entries = eng
            .db()
            .list_installs(repo_id)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|entry| {
                let is_directory = wow_dir.join(&entry.path).is_dir();
                RepoDetailEntry {
                    path: entry.path,
                    kind: entry.kind,
                    is_directory,
                }
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.path
                .to_ascii_lowercase()
                .cmp(&right.path.to_ascii_lowercase())
        });
        Ok(entries)
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn load_game_directory_children(
    wow_dir: PathBuf,
    relative_path: String,
) -> Result<Vec<RepoDetailChild>, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("load_game_directory_children");
    tokio::task::spawn_blocking(move || {
        let relative = Path::new(&relative_path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err("The selected game path is invalid.".to_string());
        }
        let directory = wow_dir.join(relative);
        if !directory.is_dir() {
            return Err("The tracked directory could not be found.".to_string());
        }
        let mut children = std::fs::read_dir(directory)
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_str()?.to_string();
                let is_directory = entry.file_type().ok()?.is_dir();
                Some(RepoDetailChild { name, is_directory })
            })
            .collect::<Vec<_>>();
        children.sort_by(|left, right| {
            right.is_directory.cmp(&left.is_directory).then_with(|| {
                left.name
                    .to_ascii_lowercase()
                    .cmp(&right.name.to_ascii_lowercase())
            })
        });
        Ok(children)
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn set_dll_enabled(
    db_path: Option<PathBuf>,
    wow_dir: String,
    dll_name: String,
    enabled: bool,
    use_dlls_txt: bool,
) -> Result<(), String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("set_dll_enabled");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_dll_enabled(&dll_name, enabled, Path::new(&wow_dir), use_dlls_txt)
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Result for a single repo updated as part of update-all.
#[derive(Debug, Clone)]
pub struct UpdateOneResult {
    pub repo_id: i64,
    pub owner: String,
    pub name: String,
    /// The updated plan, or None if already up to date.
    pub plan: Option<PlanRow>,
    /// Verbose log lines for this repo.
    pub log_lines: Vec<String>,
    /// Error message if the update failed.
    pub error: Option<String>,
}

/// Update only the repos in `ids_to_update` (already filtered: has_update && !ignored && enabled).
/// Repos are updated in parallel. Returns one result per repo.
pub async fn update_all(
    db_path: Option<PathBuf>,
    wow_dir: String,
    ids_to_update: Vec<i64>,
    opts: InstallOptions,
) -> Result<Vec<UpdateOneResult>, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("update_all");
    crate::diagnostics::trace(
        "service",
        format!("update_all: requested_count={}", ids_to_update.len()),
    );
    if ids_to_update.is_empty() {
        return Ok(Vec::new());
    }

    let mut set = tokio::task::JoinSet::new();

    for id in ids_to_update {
        let db = db_path.clone();
        let wow = wow_dir.clone();
        let opts = opts.clone();

        set.spawn_blocking(move || -> Result<UpdateOneResult, String> {
            let eng = open_engine(db.as_deref())?;
            let repo = eng.db().get_repo(id).map_err(|e| e.to_string())?;
            let owner = repo.owner.clone();
            let name = repo.name.clone();
            let mut log: Vec<String> = Vec::new();

            if repo.mode.as_str() == "addon_git" {
                let branch = repo.git_branch.as_deref().unwrap_or("default branch");
                log.push(format!("{}/{}: syncing branch '{}'.", owner, name, branch));
            } else {
                log.push(format!("{}/{}: checking release assets.", owner, name));
            }

            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?
                .block_on(async { eng.update_repo(id, Path::new(&wow), None, opts).await });

            match result {
                Err(e) => {
                    let err = e.to_string();
                    log.push(format!("{}/{}: error — {}", owner, name, err));
                    Ok(UpdateOneResult {
                        repo_id: id,
                        owner,
                        name,
                        plan: None,
                        log_lines: log,
                        error: Some(err),
                    })
                }
                Ok(None) => {
                    log.push(format!("{}/{}: already up to date.", owner, name));
                    Ok(UpdateOneResult {
                        repo_id: id,
                        owner,
                        name,
                        plan: None,
                        log_lines: log,
                        error: None,
                    })
                }
                Ok(Some(plan)) => {
                    if plan.mode.as_str() == "addon_git" {
                        log.push(format!("{}/{}: repository synced.", plan.owner, plan.name));
                    } else if !plan.asset_name.is_empty() {
                        log.push(format!(
                            "{}/{}: installed '{}'.",
                            plan.owner, plan.name, plan.asset_name
                        ));
                    }
                    log.push(format!("{}/{}: update complete.", plan.owner, plan.name));
                    Ok(UpdateOneResult {
                        repo_id: plan.repo_id,
                        owner: plan.owner.clone(),
                        name: plan.name.clone(),
                        plan: Some(PlanRow::from(plan)),
                        log_lines: log,
                        error: None,
                    })
                }
            }
        });
    }

    let mut results = Vec::new();
    while let Some(task) = set.join_next().await {
        match task {
            Err(e) => return Err(format!("Update task panicked: {}", e)),
            Ok(Err(e)) => return Err(e),
            Ok(Ok(r)) => results.push(r),
        }
    }
    Ok(results)
}

/// Install a freshly-added repo, mirroring Tauri's add flow:
/// try `update_repo` first; if it returns None (engine says nothing to do),
/// fall back to `reinstall_repo` to force a fresh clone/download.
pub async fn install_new_repo(
    db_path: Option<PathBuf>,
    id: i64,
    wow_dir: String,
    opts: InstallOptions,
) -> Result<String, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("install_new_repo");
    crate::diagnostics::trace("service", format!("install_new_repo: repo_id={id}"));
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let wow_path = Path::new(&wow_dir);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;

        let update_result = runtime
            .block_on(async { eng.update_repo(id, wow_path, None, opts.clone()).await })
            .map_err(|e| e.to_string())?;

        if let Some(plan) = update_result {
            Ok(format!("Installed {}/{}.", plan.owner, plan.name))
        } else {
            let plan = runtime
                .block_on(async { eng.reinstall_repo(id, wow_path, None, opts).await })
                .map_err(|e| e.to_string())?;
            Ok(format!("Installed {}/{}.", plan.owner, plan.name))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn update_repo(
    db_path: Option<PathBuf>,
    id: i64,
    wow_dir: String,
    opts: InstallOptions,
) -> Result<Option<PlanRow>, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("update_repo");
    crate::diagnostics::trace("service", format!("update_repo: repo_id={id}"));
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let plan = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?
            .block_on(async { eng.update_repo(id, Path::new(&wow_dir), None, opts).await })
            .map_err(|e| e.to_string())?;
        Ok(plan.map(PlanRow::from))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn reinstall_repo(
    db_path: Option<PathBuf>,
    id: i64,
    wow_dir: String,
    opts: InstallOptions,
) -> Result<PlanRow, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("reinstall_repo");
    crate::diagnostics::trace("service", format!("reinstall_repo: repo_id={id}"));
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let plan = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?
            .block_on(async {
                eng.reinstall_repo(id, Path::new(&wow_dir), None, opts)
                    .await
            })
            .map_err(|e| e.to_string())?;
        Ok(PlanRow::from(plan))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn reinstall_repo_with_selection(
    db_path: Option<PathBuf>,
    id: i64,
    wow_dir: String,
    opts: InstallOptions,
    selected_addon: String,
) -> Result<PlanRow, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("reinstall_repo_with_selection");
    crate::diagnostics::trace(
        "service",
        format!("reinstall_repo_with_selection: repo_id={id}"),
    );
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let repo = eng.db().get_repo(id).map_err(|e| e.to_string())?;
        let previous_selected = parse_selected_addons(repo.selected_addons_json.as_deref());
        eng.set_repo_selected_addons(id, Some(vec![selected_addon]))
            .map_err(|e| e.to_string())?;

        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?
            .block_on(async {
                eng.reinstall_repo(id, Path::new(&wow_dir), None, opts)
                    .await
            });

        match result {
            Ok(plan) => Ok(PlanRow::from(plan)),
            Err(error) => {
                let _ = eng.set_repo_selected_addons(
                    id,
                    if previous_selected.is_empty() {
                        None
                    } else {
                        Some(previous_selected)
                    },
                );
                Err(error.to_string())
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Branch management
// ---------------------------------------------------------------------------

pub async fn list_repo_branches(
    db_path: Option<PathBuf>,
    repo_id: i64,
) -> (i64, Result<Vec<String>, String>) {
    let _diagnostic = crate::diagnostics::OperationGuard::new("list_repo_branches");
    let result: Result<Vec<String>, String> = tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.list_repo_branches(repo_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    (repo_id, result)
}

pub async fn set_repo_branch(
    db_path: Option<PathBuf>,
    repo_id: i64,
    branch: String,
) -> Result<i64, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("set_repo_branch");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let branch_opt = if branch.is_empty() {
            None
        } else {
            Some(branch)
        };
        eng.set_repo_git_branch(repo_id, branch_opt)
            .map_err(|e| e.to_string())?;
        Ok(repo_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn set_merge_installs(
    db_path: Option<PathBuf>,
    repo_id: i64,
    merge: bool,
) -> Result<i64, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("set_merge_installs");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_repo_merge_installs(repo_id, merge)
            .map_err(|e| e.to_string())?;
        Ok(repo_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn set_pinned_version(
    db_path: Option<PathBuf>,
    repo_id: i64,
    version: Option<String>,
) -> Result<i64, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("set_pinned_version");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        eng.set_repo_pinned_version(repo_id, version)
            .map_err(|e| e.to_string())?;
        Ok(repo_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Release tag + name for the version picker dropdown.
#[derive(Debug, Clone)]
pub struct VersionItem {
    pub tag: String,
    pub name: Option<String>,
}

/// Fetch all release versions for a repo URL using the engine's forge API.
pub async fn list_repo_versions(
    db_path: Option<PathBuf>,
    repo_url: String,
) -> Result<Vec<VersionItem>, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("list_repo_versions");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let releases = tokio::runtime::Handle::current()
            .block_on(eng.list_releases(&repo_url))
            .map_err(|e| e.to_string())?;
        Ok(releases
            .into_iter()
            .map(|r| VersionItem {
                tag: r.tag,
                name: r.name,
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn fetch_latest_release_archive_options(
    db_path: Option<PathBuf>,
    repo_url: String,
) -> Result<Vec<ReleaseAssetOption>, String> {
    let _diagnostic =
        crate::diagnostics::OperationGuard::new("fetch_latest_release_archive_options");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let releases = tokio::runtime::Handle::current()
            .block_on(eng.list_releases(&repo_url))
            .map_err(|e| e.to_string())?;
        let tag_hint = release_tag_from_url(&repo_url);
        let release = if let Some(tag) = tag_hint {
            releases
                .into_iter()
                .find(|release| release.tag == tag)
                .ok_or_else(|| format!("Release tag not found: {}", tag))?
        } else {
            let Some(latest) = releases.into_iter().next() else {
                return Ok(Vec::new());
            };
            latest
        };
        let mut options = release
            .assets
            .into_iter()
            .filter(|asset| is_archive_asset_name(&asset.name))
            .map(|asset| ReleaseAssetOption {
                name: asset.name,
                tag: release.tag.clone(),
                size: asset.size,
            })
            .collect::<Vec<_>>();
        options.sort_by_key(|asset| asset.name.to_ascii_lowercase());
        Ok(options)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn open_repo_folder(
    db_path: Option<PathBuf>,
    repo_id: i64,
    wow_dir: PathBuf,
) -> Result<(), String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("open_repo_folder");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let repo = eng.db().get_repo(repo_id).map_err(|e| e.to_string())?;

        let installs = eng.db().list_installs(repo_id).map_err(|e| e.to_string())?;

        // 1. For addon_git repos, prefer the worktree root over individual addon symlinks.
        if matches!(repo.mode, InstallMode::AddonGit) {
            let addons_dir = wow_dir.join("Interface").join("AddOns");
            // Try standard clone location first, then .repo suffix (GAM collision rename)
            let candidates = [
                addons_dir.join(&repo.name),
                addons_dir.join(format!("{}.repo", repo.name)),
            ];
            for candidate in &candidates {
                if candidate.is_dir() {
                    let _ = open::that(candidate);
                    return Ok(());
                }
            }
        }

        // 2. Try first valid install path (for release/manual mods)
        let preferred_install = installs
            .iter()
            .min_by_key(|entry| match entry.kind.as_str() {
                "dll" => 0,
                "raw" => 1,
                "addon" => 2,
                _ => 1,
            });
        if let Some(first) = preferred_install {
            let full_path = wow_dir.join(&first.path);
            let target = if full_path.is_dir() {
                Some(full_path.as_path())
            } else {
                full_path.parent().filter(|parent| parent.is_dir())
            };
            if let Some(target) = target {
                open::that(target).map_err(|error| error.to_string())?;
                return Ok(());
            }
        }

        // 3. Fallback for Manual: construct path from repo name in AddOns
        if matches!(repo.mode, InstallMode::Manual) {
            let addons_dir = wow_dir.join("Interface").join("AddOns");
            let repo_path = addons_dir.join(&repo.name);
            if repo_path.exists() {
                let _ = open::that(repo_path);
                return Ok(());
            }
        }

        // 3. Last resort: open AddOns folder
        let addons_dir = wow_dir.join("Interface").join("AddOns");
        if addons_dir.exists() {
            let _ = open::that(addons_dir);
        } else if wow_dir.exists() {
            let _ = open::that(wow_dir);
        }

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn open_game_path_folder(wow_dir: PathBuf, relative_path: String) -> Result<(), String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("open_game_path_folder");
    tokio::task::spawn_blocking(move || {
        let relative = std::path::Path::new(&relative_path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err("The selected game path is invalid.".to_string());
        }

        let full_path = wow_dir.join(relative);
        let target = if full_path.is_dir() {
            full_path
        } else {
            full_path
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or(wow_dir)
        };
        if !target.is_dir() {
            return Err("The containing game directory could not be found.".to_string());
        }
        open::that(target).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn open_addon_folder(
    db_path: Option<PathBuf>,
    repo_id: i64,
    wow_dir: PathBuf,
    addon_name: String,
) -> Result<(), String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("open_addon_folder");
    tokio::task::spawn_blocking(move || {
        let eng = open_engine(db_path.as_deref())?;
        let installs = eng.db().list_installs(repo_id).map_err(|e| e.to_string())?;

        if let Some(entry) = installs.into_iter().find(|entry| {
            entry.kind == "addon"
                && std::path::Path::new(&entry.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.eq_ignore_ascii_case(&addon_name))
                    .unwrap_or(false)
        }) {
            let full_path = wow_dir.join(entry.path);
            if full_path.exists() {
                let _ = open::that(full_path);
                return Ok(());
            }
        }

        let fallback = wow_dir.join("Interface").join("AddOns").join(addon_name);
        if fallback.exists() {
            let _ = open::that(fallback);
        }

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Game launch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LaunchConfig {
    pub method: String, // "auto", "lutris", "wine", "custom"
    pub auto_launch_exe: Option<String>,
    pub lutris_target: String, // e.g. "lutris:rungameid/2"
    pub wine_command: String,  // e.g. "wine"
    pub wine_args: String,
    pub custom_command: String,
    pub custom_args: String,
    pub clear_wdb: bool,
    #[cfg(feature = "auto-login")]
    pub profile_id: String,
    #[cfg(feature = "auto-login")]
    pub auto_login_account_id: Option<wuddle_engine::auto_login::AccountId>,
}

fn first_existing_file(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    // Preserve an exact spelling when present, then tolerate filesystem/case
    // differences in filenames selected by older settings or file pickers.
    if let Some(candidate) = names
        .iter()
        .map(|name| dir.join(name))
        .find(|candidate| candidate.is_file())
    {
        return Some(candidate);
    }

    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .find_map(|entry| {
            let filename = entry.file_name();
            let filename = filename.to_string_lossy();
            names
                .iter()
                .any(|name| filename.eq_ignore_ascii_case(name))
                .then_some(entry.path())
        })
}

fn resolve_launch_target(
    wow_path: &Path,
    auto_launch_exe: Option<&str>,
) -> Result<PathBuf, String> {
    let override_name = auto_launch_exe
        .map(str::trim)
        .filter(|name| !name.is_empty());

    if let Some(exe_name) = override_name {
        if let Some(target) = first_existing_file(wow_path, &[exe_name]) {
            return Ok(target);
        }
    }

    first_existing_file(wow_path, &["VanillaFixes.exe", "vanillafixes.exe"])
        .or_else(|| first_existing_file(wow_path, &["Wow.exe", "wow.exe", "WoW.exe"]))
        .ok_or_else(|| match override_name {
            Some(exe_name) => format!(
                "No launcher found in {} (checked {}, VanillaFixes.exe, and Wow.exe).",
                wow_path.display(),
                exe_name
            ),
            None => format!(
                "No launcher found in {} (expected VanillaFixes.exe or Wow.exe).",
                wow_path.display()
            ),
        })
}

fn parse_arg_string(raw: &str) -> Vec<String> {
    raw.split_whitespace().map(|s| s.to_string()).collect()
}

fn spawn_launch_command(program: &str, args: &[String], cwd: &Path) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    spawn_command(cmd, program, cwd)
}

fn spawn_command(mut cmd: Command, program: &str, cwd: &Path) -> Result<(), String> {
    cmd.current_dir(cwd);
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        clean_env_for_child(&mut cmd);
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch '{}': {}", program, e))
}

/// Strip AppImage-injected env vars so child processes see a normal environment.
#[cfg(all(unix, not(target_os = "macos")))]
fn clean_env_for_child(cmd: &mut Command) {
    const BLOCKLIST: &[&str] = &[
        "APPDIR",
        "APPIMAGE",
        "ARGV0",
        "OWD",
        "LD_LIBRARY_PATH",
        "LD_PRELOAD",
        "GIO_MODULE_DIR",
        "GST_PLUGIN_PATH",
        "GST_PLUGIN_SYSTEM_PATH",
        "QT_PLUGIN_PATH",
        "PYTHONPATH",
        "PYTHONHOME",
        "GDK_BACKEND",
    ];
    for key in BLOCKLIST {
        cmd.env_remove(key);
    }
    let clean_path = std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|p| !p.contains("/tmp/.mount_"))
        .collect::<Vec<_>>()
        .join(":");
    if !clean_path.is_empty() {
        cmd.env("PATH", clean_path);
    }
    if let Ok(dirs) = std::env::var("XDG_DATA_DIRS") {
        let clean: Vec<&str> = dirs
            .split(':')
            .filter(|p| !p.contains("/tmp/.mount_"))
            .collect();
        if !clean.is_empty() {
            cmd.env("XDG_DATA_DIRS", clean.join(":"));
        } else {
            cmd.env_remove("XDG_DATA_DIRS");
        }
    }
}

pub async fn launch_game(wow_dir: String, cfg: LaunchConfig) -> Result<String, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("launch_game");
    #[cfg(feature = "auto-login")]
    let auto_login_requested = cfg.auto_login_account_id.is_some();
    #[cfg(not(feature = "auto-login"))]
    let auto_login_requested = false;
    crate::diagnostics::trace(
        "launch",
        format!(
            "launch_game: method={}; clear_wdb={}; auto_login_requested={}",
            cfg.method, cfg.clear_wdb, auto_login_requested
        ),
    );
    tokio::task::spawn_blocking(move || {
        let wow_path = PathBuf::from(wow_dir.trim());
        if !wow_path.is_dir() {
            return Err(format!("WoW path is not a directory: {}", wow_path.display()));
        }

        // Optionally clear WDB cache before launch
        if cfg.clear_wdb {
            let wdb = wow_path.join("WDB");
            if wdb.is_dir() {
                let _ = std::fs::remove_dir_all(&wdb);
            }
        }

        let target = resolve_launch_target(&wow_path, cfg.auto_launch_exe.as_deref())?;
        let target_str = target.to_string_lossy().to_string();
        let target_name = target
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "game".to_string());

        let method = cfg.method.trim().to_ascii_lowercase();

        #[cfg(feature = "auto-login")]
        let prepared_auto_login = if let Some(account_id) = cfg.auto_login_account_id.as_ref() {
            if method == "lutris" {
                return Err(
                    "Secure auto-login is not supported by Lutris launches because Lutris has no transient argument override. Choose Manual Login or use Wuddle's Wine launch method."
                        .to_string(),
                );
            }
            Some(
                wuddle_engine::auto_login::AutoLoginService::system()
                    .prepare_arguments(&cfg.profile_id, account_id)
                    .map_err(|error| format!("Could not prepare secure auto-login: {error}"))?,
            )
        } else {
            None
        };

        if method == "lutris" {
            let command = if cfg.custom_command.trim().is_empty() {
                "lutris"
            } else {
                cfg.custom_command.trim()
            };
            let target_arg = cfg.lutris_target.trim();
            if target_arg.is_empty() {
                return Err(
                    "Lutris launch target is empty (expected e.g. lutris:rungameid/2).".to_string(),
                );
            }
            let mut args = vec![target_arg.to_string()];
            args.extend(parse_arg_string(&cfg.custom_args));
            spawn_launch_command(command, &args, &wow_path)?;
            return Ok(format!("Launched {} via {}.", target_name, command));
        }

        if method == "wine" {
            let command = if cfg.wine_command.trim().is_empty() {
                "wine"
            } else {
                cfg.wine_command.trim()
            };
            let args = parse_arg_string(&cfg.wine_args);
            let mut launch = Command::new(command);
            launch.args(&args).arg(&target_str);
            #[cfg(feature = "auto-login")]
            if let Some(prepared) = prepared_auto_login.as_ref() {
                prepared.append_to_command(&mut launch);
            }
            spawn_command(launch, command, &wow_path)?;
            return Ok(format!("Launched {} via {}.", target_name, command));
        }

        if method == "custom" {
            let command = cfg.custom_command.trim();
            if command.is_empty() {
                return Err("Custom launch command is empty.".to_string());
            }
            let mut args = parse_arg_string(&cfg.custom_args);
            let mut inserted_exe = false;
            for arg in &mut args {
                if arg.contains("{exe}") {
                    *arg = arg.replace("{exe}", &target_str);
                    inserted_exe = true;
                }
                if arg.contains("{wow_dir}") {
                    *arg = arg.replace("{wow_dir}", wow_path.to_string_lossy().as_ref());
                }
            }
            if !inserted_exe {
                args.push(target_str);
            }
            let mut launch = Command::new(command);
            #[cfg(feature = "auto-login")]
            match prepared_auto_login.as_ref() {
                Some(prepared) => prepared
                    .append_custom_command(&mut launch, &args)
                    .map_err(|error| error.to_string())?,
                None => wuddle_engine::auto_login::append_manual_custom_arguments(
                    &mut launch,
                    &args,
                ),
            }
            #[cfg(not(feature = "auto-login"))]
            launch.args(&args);
            spawn_command(launch, command, &wow_path)?;
            return Ok(format!("Launched {} via custom command.", target_name));
        }

        // "auto" or fallback: launch executable directly
        let mut cmd = Command::new(&target);
        #[cfg(feature = "auto-login")]
        if let Some(prepared) = prepared_auto_login.as_ref() {
            prepared.append_to_command(&mut cmd);
        }
        cmd.current_dir(&wow_path);
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            clean_env_for_child(&mut cmd);
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        cmd.spawn()
            .map(|_| format!("Launched {}.", target_name))
            .map_err(|e| format!("Failed to launch {}: {}", target_name, e))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Launch a companion executable installed in the WoW root. On Linux this uses
/// the profile's Wine command because these tools are Windows executables.
pub async fn launch_wow_root_tool(
    wow_dir: String,
    cfg: LaunchConfig,
    candidates: Vec<String>,
) -> Result<String, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("launch_wow_root_tool");
    crate::diagnostics::trace(
        "launch",
        format!("launch_wow_root_tool: candidate_count={}", candidates.len()),
    );
    tokio::task::spawn_blocking(move || {
        let wow_path = PathBuf::from(wow_dir.trim());
        if !wow_path.is_dir() {
            return Err(format!(
                "WoW path is not a directory: {}",
                wow_path.display()
            ));
        }
        let candidate_refs: Vec<&str> = candidates.iter().map(String::as_str).collect();
        let tool = first_existing_file(&wow_path, &candidate_refs).ok_or_else(|| {
            format!(
                "Required tool was not found in {} (checked {}). Reinstall the mod first.",
                wow_path.display(),
                candidates.join(", ")
            )
        })?;
        let tool_name = tool
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("tool")
            .to_string();

        #[cfg(windows)]
        {
            let mut command = Command::new(&tool);
            command.current_dir(&wow_path);
            command
                .spawn()
                .map_err(|e| format!("Failed to launch {}: {}", tool_name, e))?;
        }
        #[cfg(not(windows))]
        {
            let command = if cfg.wine_command.trim().is_empty() {
                "wine"
            } else {
                cfg.wine_command.trim()
            };
            let mut args = parse_arg_string(&cfg.wine_args);
            args.push(tool.to_string_lossy().to_string());
            spawn_launch_command(command, &args, &wow_path)?;
        }

        Ok(format!("Launched {}.", tool_name))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Back up the selected WoW.exe once, then run Awesome WotLK's patcher with that
/// executable as its target (the equivalent of dragging WoW.exe onto the tool).
pub async fn patch_wow_with_awesome_wotlk(
    wow_dir: String,
    cfg: LaunchConfig,
) -> Result<String, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("patch_wow_with_awesome_wotlk");
    tokio::task::spawn_blocking(move || {
        let wow_path = PathBuf::from(wow_dir.trim());
        if !wow_path.is_dir() {
            return Err(format!(
                "WoW path is not a directory: {}",
                wow_path.display()
            ));
        }

        let wow_exe = first_existing_game_executable(&wow_path)
            .ok_or_else(|| format!("WoW.exe was not found in {}.", wow_path.display()))?;
        let backup = wow_path.join("original_wow.exe");
        let backup_created = if backup.exists() || backup.is_symlink() {
            false
        } else {
            std::fs::copy(&wow_exe, &backup).map_err(|e| {
                format!(
                    "Failed to create {} from {}: {}",
                    backup.display(),
                    wow_exe.display(),
                    e
                )
            })?;
            true
        };

        let patcher = first_existing_file(
            &wow_path,
            &["AwesomeWotlkPatch.exe", "AwesomeWotLKPatcher.exe"],
        )
        .ok_or_else(|| {
            "Awesome WotLK patch tool was not found in the WoW directory. Reinstall the mod first."
                .to_string()
        })?;

        #[cfg(windows)]
        let status = Command::new(&patcher)
            .arg(&wow_exe)
            .current_dir(&wow_path)
            .status()
            .map_err(|e| format!("Failed to run {}: {}", patcher.display(), e))?;

        #[cfg(not(windows))]
        let status = {
            let command = if cfg.wine_command.trim().is_empty() {
                "wine"
            } else {
                cfg.wine_command.trim()
            };
            let mut command_process = Command::new(command);
            command_process
                .args(parse_arg_string(&cfg.wine_args))
                .arg(&patcher)
                .arg(&wow_exe)
                .current_dir(&wow_path);
            #[cfg(all(unix, not(target_os = "macos")))]
            clean_env_for_child(&mut command_process);
            command_process
                .status()
                .map_err(|e| format!("Failed to run {}: {}", patcher.display(), e))?
        };

        if !status.success() {
            return Err(format!(
                "Awesome WotLK patch tool exited with status {}. Your {} backup is retained.",
                status,
                backup.display()
            ));
        }

        Ok(if backup_created {
            format!(
                "Created {} and patched {} with Awesome WotLK.",
                backup.display(),
                wow_exe.display()
            )
        } else {
            format!(
                "Patched {} with Awesome WotLK; existing {} was preserved.",
                wow_exe.display(),
                backup.display()
            )
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// GitHub token management
// ---------------------------------------------------------------------------

const KEYCHAIN_SERVICE: &str = "wuddle";
const KEYCHAIN_ACCOUNT: &str = "github_token";
const KEYCHAIN_TIMEOUT_MS: u64 = 2500;

fn keychain_call_with_timeout<T, F>(label: &'static str, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(Duration::from_millis(KEYCHAIN_TIMEOUT_MS)) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
            "System keychain timed out while {}. Ensure keychain is running, or use WUDDLE_GITHUB_TOKEN env.",
            label
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("System keychain worker failed unexpectedly.".to_string())
        }
    }
}

fn token_file_path() -> Result<PathBuf, String> {
    Ok(crate::settings::app_dir()?.join(".github_token"))
}

fn read_file_token() -> Result<Option<String>, String> {
    let path = token_file_path()?;
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            let t = s.trim().to_string();
            Ok(if t.is_empty() { None } else { Some(t) })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn read_keychain_token() -> Result<Option<String>, String> {
    keychain_call_with_timeout("reading token", || {
        let entry =
            keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(token) => {
                let token = token.trim().to_string();
                Ok(if token.is_empty() { None } else { Some(token) })
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    })
}

fn write_keychain_token(token: &str) -> Result<(), String> {
    let token = token.to_string();
    keychain_call_with_timeout("saving token", move || {
        let entry =
            keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT).map_err(|e| e.to_string())?;
        entry.set_password(&token).map_err(|e| e.to_string())
    })
}

fn delete_keychain_token() -> Result<(), String> {
    keychain_call_with_timeout("clearing token", || {
        let entry =
            keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT).map_err(|e| e.to_string())?;
        if let Err(e) = entry.delete_credential() {
            if !matches!(e, keyring::Error::NoEntry) {
                return Err(e.to_string());
            }
        }
        Ok(())
    })
}

#[cfg(any(target_os = "windows", test))]
#[cfg_attr(test, allow(dead_code))]
fn import_legacy_plaintext_token() -> Result<(), String> {
    let paths = crate::storage::legacy_plaintext_token_paths()?;
    if paths.is_empty() {
        return Ok(());
    }

    let mut tokens = Vec::<String>::new();
    for path in &paths {
        let token = std::fs::read_to_string(path)
            .map_err(|e| format!("Could not read legacy GitHub token {}: {e}", path.display()))?
            .trim()
            .to_string();
        if !token.is_empty() && !tokens.iter().any(|known| known == &token) {
            tokens.push(token);
        }
    }
    if tokens.len() > 1 {
        return Err(format!(
            "Several legacy portable GitHub token files contain different credentials. They were left untouched: {}",
            paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let Some(token) = tokens.pop() else {
        return Ok(());
    };

    match read_keychain_token()? {
        Some(stored) if stored != token => {
            return Err(
                "A legacy portable GitHub token differs from the token already stored in Windows Credential Manager. The plaintext file was left untouched."
                    .to_string(),
            );
        }
        Some(_) => {}
        None => write_keychain_token(&token)?,
    }
    verify_stored_token(&token, read_keychain_token()?)?;

    for path in paths {
        std::fs::remove_file(&path).map_err(|e| {
            format!(
                "The GitHub token was imported into Windows Credential Manager, but the legacy plaintext copy could not be removed from {}: {e}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn read_stored_token() -> Result<Option<String>, String> {
    #[cfg(target_os = "windows")]
    {
        return read_keychain_token();
    }

    #[cfg(not(target_os = "windows"))]
    if crate::settings::portable_mode_enabled() {
        return read_file_token();
    }

    #[cfg(not(target_os = "windows"))]
    read_keychain_token()
}

fn environment_github_token() -> Option<String> {
    std::env::var("WUDDLE_GITHUB_TOKEN")
        .ok()
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Load a persisted token, falling back to an environment variable. Returning
/// an error is important: callers must not silently report anonymous API
/// access as an authenticated saved-token session when the system credential
/// store could not be read.
pub fn sync_github_token() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    if let Err(error) = import_legacy_plaintext_token() {
        wuddle_engine::set_github_token(environment_github_token());
        return Err(format!("Could not migrate the saved GitHub token: {error}"));
    }

    match read_stored_token() {
        Ok(Some(token)) => {
            wuddle_engine::set_github_token(Some(token));
            Ok(())
        }
        Ok(None) => {
            wuddle_engine::set_github_token(environment_github_token());
            Ok(())
        }
        Err(error) => {
            // An environment token still permits authenticated use, but retain
            // the read failure so Options can explain why the saved credential
            // was not available.
            wuddle_engine::set_github_token(environment_github_token());
            Err(format!("Could not read the saved GitHub token: {error}"))
        }
    }
}

fn verify_stored_token(expected: &str, stored: Option<String>) -> Result<(), String> {
    match stored {
        Some(token) if token == expected => Ok(()),
        Some(_) => Err(
            "The saved GitHub token did not match when read back. It was not activated."
                .to_string(),
        ),
        None => Err(
            "GitHub token storage returned no token after saving. It was not activated."
                .to_string(),
        ),
    }
}

#[cfg(test)]
mod github_token_tests {
    use super::verify_stored_token;

    #[test]
    fn token_readback_requires_an_exact_match() {
        assert!(verify_stored_token("secret", Some("secret".to_string())).is_ok());
        assert!(verify_stored_token("secret", Some("different".to_string())).is_err());
        assert!(verify_stored_token("secret", None).is_err());
    }
}

#[cfg(test)]
mod launch_target_tests {
    use super::resolve_launch_target;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn explicit_launch_target_matching_is_case_insensitive() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "wuddle-launch-target-case-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let expected = dir.join("wow.exe");
        fs::write(&expected, []).unwrap();

        let target = resolve_launch_target(&dir, Some("WoW.ExE")).unwrap();
        assert_eq!(target, expected);

        fs::remove_dir_all(&dir).unwrap();
    }
}

pub async fn save_github_token(token: String) -> Result<(), String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("save_github_token");
    tokio::task::spawn_blocking(move || {
        let token = token.trim().to_string();
        if token.is_empty() {
            return Err("Token is empty.".to_string());
        }
        #[cfg(not(target_os = "windows"))]
        if crate::settings::portable_mode_enabled() {
            let path = token_file_path()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, &token).map_err(|e| e.to_string())?;
        } else {
            write_keychain_token(&token)?;
        }

        #[cfg(target_os = "windows")]
        write_keychain_token(&token)?;

        // A successful write alone is not enough: some credential-store
        // backends can accept a write yet fail on the next process/read. Only
        // report success once Wuddle can read the exact credential back.
        let stored = read_stored_token()?;
        verify_stored_token(&token, stored)?;

        #[cfg(target_os = "windows")]
        for path in crate::storage::legacy_plaintext_token_paths()? {
            std::fs::remove_file(&path).map_err(|e| {
                format!(
                    "The new GitHub token was saved securely, but an old plaintext copy could not be removed from {}: {e}",
                    path.display()
                )
            })?;
        }

        wuddle_engine::set_github_token(Some(token));
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn clear_github_token() -> Result<(), String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("clear_github_token");
    tokio::task::spawn_blocking(|| {
        #[cfg(not(target_os = "windows"))]
        if crate::settings::portable_mode_enabled() {
            let path = token_file_path()?;
            if path.exists() {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
        } else {
            delete_keychain_token()?;
        }

        #[cfg(target_os = "windows")]
        {
            delete_keychain_token()?;
            for path in crate::storage::legacy_plaintext_token_paths()? {
                std::fs::remove_file(&path).map_err(|e| {
                    format!(
                        "The secure GitHub token was cleared, but an old plaintext copy could not be removed from {}: {e}",
                        path.display()
                    )
                })?;
            }
        }
        sync_github_token()
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Repo preview (for Add dialog)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RepoFileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub struct RepoPreviewInfo {
    pub name: String,
    pub description: String,
    pub stars: u64,
    pub forks: u64,
    pub language: String,
    pub license: String,
    pub readme_text: String,
    pub readme_items: Vec<iced::widget::markdown::Item>,
    /// Decoded image handles keyed by URL. Handle IDs are stable so iced can cache decoded images
    /// across renders without re-decoding on every frame.
    pub image_cache: std::collections::HashMap<String, iced::widget::image::Handle>,
    /// Decoded GIF frames keyed by URL (for animated images in READMEs).
    pub gif_cache: std::collections::HashMap<String, std::sync::Arc<iced_gif::Frames>>,
    pub files: Vec<RepoFileEntry>,
    /// Base URL for resolving relative image paths (e.g. "https://raw.githubusercontent.com/owner/repo/HEAD/")
    pub raw_base_url: String,
    pub forge: String,
    pub owner: String,
    pub repo_name: String,
    pub forge_url: String,
}

// ---------------------------------------------------------------------------
// Parse forge from URL
// ---------------------------------------------------------------------------

pub struct ForgeInfo {
    pub owner: String,
    pub repo: String,
    pub forge: &'static str,
    pub host: String,
    pub scheme: String,
}

pub fn parse_forge_url(url: &str) -> Option<ForgeInfo> {
    let trimmed = url.trim().trim_end_matches('/');
    let without_scheme = trimmed
        .strip_prefix("https://")
        .map(|s| ("https", s))
        .or_else(|| trimmed.strip_prefix("http://").map(|s| ("http", s)))
        .unwrap_or(("https", trimmed));
    let (scheme, rest) = without_scheme;

    if let Some(r) = rest.strip_prefix("github.com/") {
        let parts: Vec<&str> = r.splitn(3, '/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            let repo = parts[1].trim_end_matches(".git").to_string();
            return Some(ForgeInfo {
                owner: parts[0].to_string(),
                repo,
                forge: "github",
                host: "github.com".into(),
                scheme: scheme.into(),
            });
        }
    } else if let Some(r) = rest.strip_prefix("gitlab.com/") {
        let mut parts: Vec<&str> = r.split('/').filter(|part| !part.is_empty()).collect();
        if let Some(metadata) = parts.iter().position(|part| *part == "-") {
            parts.truncate(metadata);
        }
        if parts.len() >= 2 {
            let repo = parts.pop().unwrap().trim_end_matches(".git").to_string();
            return Some(ForgeInfo {
                owner: parts.join("/"),
                repo,
                forge: "gitlab",
                host: "gitlab.com".into(),
                scheme: scheme.into(),
            });
        }
    } else {
        let parts: Vec<&str> = rest.splitn(4, '/').collect();
        if parts.len() >= 3 && !parts[1].is_empty() && !parts[2].is_empty() {
            let host = parts[0];
            if host.contains("gitea")
                || host.contains("forgejo")
                || host.contains("codeberg")
                || host.contains("gitea")
            {
                let repo = parts[2].trim_end_matches(".git").to_string();
                return Some(ForgeInfo {
                    owner: parts[1].to_string(),
                    repo,
                    forge: "gitea",
                    host: host.into(),
                    scheme: scheme.into(),
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod forge_url_tests {
    use super::parse_forge_url;

    #[test]
    fn gitlab_nested_namespace_is_not_truncated() {
        let parsed =
            parse_forge_url("https://gitlab.com/group/subgroup/addons/project.git").unwrap();
        assert_eq!(parsed.forge, "gitlab");
        assert_eq!(parsed.owner, "group/subgroup/addons");
        assert_eq!(parsed.repo, "project");
    }

    #[test]
    fn gitlab_browse_suffix_is_not_part_of_identity() {
        let parsed =
            parse_forge_url("https://gitlab.com/group/subgroup/project/-/tree/main/Addon").unwrap();
        assert_eq!(parsed.owner, "group/subgroup");
        assert_eq!(parsed.repo, "project");
    }
}

pub fn normalize_repo_input_url(url: &str) -> String {
    parse_forge_url(url)
        .map(|fi| format!("{}://{}/{}/{}", fi.scheme, fi.host, fi.owner, fi.repo))
        .unwrap_or_else(|| url.trim().trim_end_matches('/').to_string())
}

pub fn selected_addon_hint_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/');
    let without_scheme = trimmed
        .strip_prefix("https://")
        .map(|s| ("https", s))
        .or_else(|| trimmed.strip_prefix("http://").map(|s| ("http", s)))
        .unwrap_or(("https", trimmed));
    let (_scheme, rest) = without_scheme;

    if let Some(r) = rest.strip_prefix("github.com/") {
        let parts: Vec<&str> = r.split('/').filter(|part| !part.is_empty()).collect();
        if parts.len() >= 5 && parts[2] == "tree" {
            return parts.last().map(|name| name.to_string());
        }
    }

    if let Some(r) = rest.strip_prefix("gitlab.com/") {
        let parts: Vec<&str> = r.split('/').filter(|part| !part.is_empty()).collect();
        if let Some(tree_index) = parts.iter().position(|part| *part == "tree") {
            if parts.get(tree_index.wrapping_add(2)).is_some() {
                return parts.last().map(|name| name.to_string());
            }
        }
    }

    let parts: Vec<&str> = rest.split('/').filter(|part| !part.is_empty()).collect();
    if let Some(src_index) = parts.iter().position(|part| *part == "src") {
        if parts.get(src_index.wrapping_add(2)).is_some() {
            return parts.last().map(|name| name.to_string());
        }
    }

    None
}

pub fn normalize_collection_entry_key(name: &str) -> String {
    let mut key = name.trim().to_ascii_lowercase();

    for suffix in ["-master", "_master", "-main", "_main"] {
        if let Some(stripped) = key.strip_suffix(suffix) {
            key = stripped.to_string();
            break;
        }
    }

    key
}

// ---------------------------------------------------------------------------
// Image helpers
// ---------------------------------------------------------------------------

/// Convert `<img src="..." alt="...">` HTML tags in markdown text to standard
/// `![alt](url)` syntax so iced's pulldown-cmark parser creates `Item::Image` entries.
/// Also strips `<p>`, `</p>`, and `<br>` tags that GitHub injects around images.
pub fn convert_html_images_to_markdown(markdown: &str) -> String {
    let mut result = String::with_capacity(markdown.len());
    let mut pos = 0;
    while pos < markdown.len() {
        match markdown[pos..].find("<img") {
            None => {
                result.push_str(&markdown[pos..]);
                break;
            }
            Some(tag_offset) => {
                result.push_str(&markdown[pos..pos + tag_offset]);
                let tag_start = pos + tag_offset;
                let tag_end = markdown[tag_start..]
                    .find('>')
                    .map(|e| tag_start + e + 1)
                    .unwrap_or(markdown.len());
                let tag_slice = &markdown[tag_start..tag_end];
                // Extract src= attribute
                let src = extract_attr(tag_slice, "src");
                let alt = extract_attr(tag_slice, "alt").unwrap_or_default();
                if let Some(url) = src {
                    result.push_str(&format!("![{}]({})", alt, url));
                } else {
                    result.push_str(tag_slice);
                }
                pos = tag_end;
            }
        }
    }
    // Strip <p>, </p>, <br>, <br/>, <br /> tags that GitHub wraps around images
    let result = result
        .replace("<p>", "")
        .replace("</p>", "")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");
    result
}

fn extract_attr<'a>(tag: &'a str, attr_name: &str) -> Option<String> {
    let needle = format!("{}=", attr_name);
    let attr_pos = tag.find(&needle)?;
    let after = &tag[attr_pos + needle.len()..];
    let q = after.chars().next()?;
    if q == '"' || q == '\'' {
        let inner = &after[1..];
        let end = inner.find(q)?;
        Some(inner[..end].trim().to_string())
    } else {
        // Unquoted attribute value — take until space or >
        let end = after
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(after.len());
        Some(after[..end].trim().to_string())
    }
}

/// Resolve a potentially-relative image URL against a raw base URL.
pub fn resolve_image_url(url: &str, raw_base_url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        let clean = url.trim_start_matches("./").trim_start_matches('/');
        format!("{}{}", raw_base_url, clean)
    }
}

/// Fetch images for URLs found in the README.
/// Returns two caches: static image handles and animated GIF frames.
/// Handles are created once here so their IDs are fixed — iced can then cache the decoded
/// pixels across renders without re-decoding on every frame.
/// Limits: max 12 images, 5 MB each, 20 MB total.
async fn fetch_images(
    client: &Client,
    image_urls: &[String],
    raw_base_url: &str,
) -> (
    std::collections::HashMap<String, iced::widget::image::Handle>,
    std::collections::HashMap<String, std::sync::Arc<iced_gif::Frames>>,
) {
    let mut image_cache = std::collections::HashMap::new();
    let mut gif_cache = std::collections::HashMap::new();
    let mut total_bytes = 0usize;

    // Pre-resolve github.com/user-attachments/assets/UUID → signed CDN URLs.
    // GitHub renders these as private-user-images.githubusercontent.com/?jwt=... in its HTML.
    let attachment_resolves =
        resolve_github_user_attachments(client, raw_base_url, image_urls).await;

    for url in image_urls.iter().take(12) {
        if total_bytes > 20_000_000 {
            break;
        }

        let abs_url = resolve_image_url(url, raw_base_url);

        // For user-attachments URLs, use the signed CDN URL extracted from GitHub HTML.
        let fetch_url: String = attachment_resolves
            .get(url.as_str())
            .cloned()
            .unwrap_or_else(|| abs_url.clone());

        let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let mut req = client.get(&fetch_url);
            // Non-signed private-user-images URLs may need a GitHub token.
            if fetch_url.contains("private-user-images.githubusercontent.com")
                && !fetch_url.contains("?jwt=")
            {
                if let Some(token) = wuddle_engine::github_token() {
                    req = req.bearer_auth(token);
                }
            }
            let resp = req.send().await?;
            let ct = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(none)")
                .to_string();
            if !resp.status().is_success() {
                return Err(reqwest::Error::from(resp.error_for_status().unwrap_err()));
            }
            if !ct.starts_with("image/") {
                return Ok((Default::default(), false));
            }
            let is_gif =
                ct == "image/gif" || fetch_url.split('?').next().unwrap_or("").ends_with(".gif");
            resp.bytes().await.map(|b| (b, is_gif))
        })
        .await;

        if let Ok(Ok((bytes, is_gif))) = result {
            if !bytes.is_empty() && bytes.len() <= 5_000_000 {
                total_bytes += bytes.len();
                if is_gif {
                    // Decode animated GIF frames for iced_gif widget.
                    if let Ok(frames) = iced_gif::Frames::from_bytes(bytes.to_vec()) {
                        let frames = std::sync::Arc::new(frames);
                        gif_cache.insert(url.clone(), frames.clone());
                        if abs_url != *url {
                            gif_cache.insert(abs_url, frames);
                        }
                    } else {
                        // Fall back to static handle if decoding fails.
                        let handle = iced::widget::image::Handle::from_bytes(bytes);
                        image_cache.insert(url.clone(), handle.clone());
                        if abs_url != *url {
                            image_cache.insert(abs_url, handle);
                        }
                    }
                } else {
                    // Create the handle once — its Id is fixed for the lifetime of this preview,
                    // so iced can cache the decoded image across renders.
                    let handle = iced::widget::image::Handle::from_bytes(bytes);
                    // Store by original URL (as seen in markdown) AND absolute URL
                    image_cache.insert(url.clone(), handle.clone());
                    if abs_url != *url {
                        image_cache.insert(abs_url, handle);
                    }
                }
            }
        }
    }
    (image_cache, gif_cache)
}

/// Resolve `github.com/user-attachments/assets/UUID` URLs to time-limited signed CDN URLs.
///
/// GitHub's HTML page for the repo contains `<img src="https://private-user-images.githubusercontent.com/…?jwt=…">`
/// entries for any user-attachments referenced in the README.  We fetch the page once, then
/// extract the signed URL for each UUID we care about.
async fn resolve_github_user_attachments(
    client: &Client,
    raw_base_url: &str,
    image_urls: &[String],
) -> std::collections::HashMap<String, String> {
    let attachment_pairs: Vec<(String, String)> = image_urls
        .iter()
        .filter_map(|u| {
            u.strip_prefix("https://github.com/user-attachments/assets/")
                .map(|uuid| (u.clone(), uuid.to_string()))
        })
        .collect();

    let mut result: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if attachment_pairs.is_empty() {
        return result;
    }

    // Derive owner/repo from raw_base_url:
    //   "https://raw.githubusercontent.com/{owner}/{repo}/..."
    let after = raw_base_url
        .strip_prefix("https://raw.githubusercontent.com/")
        .unwrap_or("");
    let parts: Vec<&str> = after.splitn(3, '/').collect();
    if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
        return result;
    }
    let html_url = format!("https://github.com/{}/{}", parts[0], parts[1]);

    let resp = match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        client.get(&html_url).send(),
    )
    .await
    {
        Ok(Ok(r)) => r,
        Ok(Err(_)) => return result,
        Err(_) => return result,
    };
    if !resp.status().is_success() {
        return result;
    }
    let html = resp.text().await.unwrap_or_default();

    // Scan all private-user-images URLs in the HTML and match each one by UUID.
    // We scan rather than searching for the UUID first because the UUID may appear
    // earlier in the HTML inside JSON blobs where the signed URL isn't present.
    let signed_prefix = "https://private-user-images.githubusercontent.com/";
    let mut signed_urls: Vec<String> = Vec::new();
    let mut scan_pos = 0;
    while let Some(p) = html[scan_pos..].find(signed_prefix) {
        let start = scan_pos + p;
        let rest = &html[start..];
        // URL ends at the first `"`, `'`, `\` (JSON-escaped quote context), or whitespace
        let end = rest
            .find(|c: char| c == '"' || c == '\'' || c == '\\' || c.is_ascii_whitespace())
            .unwrap_or_else(|| rest.len().min(3000));
        let candidate = rest[..end].to_string();
        if !candidate.is_empty() && !signed_urls.contains(&candidate) {
            signed_urls.push(candidate);
        }
        scan_pos = start + signed_prefix.len();
    }
    for (orig_url, uuid) in &attachment_pairs {
        // Find the signed URL whose path contains this UUID
        if let Some(signed) = signed_urls.iter().find(|u| u.contains(uuid.as_str())) {
            result.insert(orig_url.clone(), signed.clone());
        }
    }
    result
}

/// Fetch raw text content of a file from a repo's raw base URL.
/// Returns (filename/path, content).
pub async fn fetch_raw_file(
    raw_base_url: String,
    path: String,
) -> Result<(String, String), String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("fetch_raw_file");
    let base = raw_base_url.trim_end_matches('/');
    let url = format!("{}/{}", base, path.trim_start_matches('/'));
    let client = Client::builder()
        .user_agent("wuddle-iced")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let content = resp.text().await.map_err(|e| e.to_string())?;
    Ok((path, content))
}

// ---------------------------------------------------------------------------
// Files tree helper
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ContentEntry {
    name: String,
    #[serde(rename = "type")]
    kind: String,
}

async fn fetch_files(
    client: &Client,
    forge: &str,
    host: &str,
    owner: &str,
    repo: &str,
    scheme: &str,
) -> Vec<RepoFileEntry> {
    match forge {
        "github" => {
            let url = format!("https://api.github.com/repos/{}/{}/contents/", owner, repo);
            let mut req = client
                .get(&url)
                .header("Accept", "application/vnd.github+json");
            if let Some(token) = wuddle_engine::github_token() {
                req = req.bearer_auth(token);
            }
            match req.send().await {
                Ok(r) if r.status().is_success() => r
                    .json::<Vec<ContentEntry>>()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| RepoFileEntry {
                        is_dir: e.kind == "dir",
                        path: e.name.clone(),
                        name: e.name,
                    })
                    .collect(),
                _ => Vec::new(),
            }
        }
        "gitlab" => {
            let encoded = format!("{}/{}", owner, repo).replace('/', "%2F");
            let url = format!(
                "https://gitlab.com/api/v4/projects/{}/repository/tree?per_page=50",
                encoded
            );
            match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => r
                    .json::<Vec<ContentEntry>>()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| RepoFileEntry {
                        is_dir: e.kind == "tree",
                        path: e.name.clone(),
                        name: e.name,
                    })
                    .collect(),
                _ => Vec::new(),
            }
        }
        _ => {
            let url = format!(
                "{}://{}/api/v1/repos/{}/{}/contents/",
                scheme, host, owner, repo
            );
            match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => r
                    .json::<Vec<ContentEntry>>()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| RepoFileEntry {
                        is_dir: e.kind == "dir",
                        path: e.name.clone(),
                        name: e.name,
                    })
                    .collect(),
                _ => Vec::new(),
            }
        }
    }
}

/// Fetch contents of a subdirectory within a repo tree.
/// Returns (dir_path, entries) where each entry's `path` is the full path from repo root.
pub async fn fetch_dir_contents(
    forge_url: String,
    dir_path: String,
) -> Result<(String, Vec<RepoFileEntry>), String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("fetch_dir_contents");
    let fi = parse_forge_url(&forge_url).ok_or_else(|| "Could not parse repo URL".to_string())?;
    let client = Client::builder()
        .user_agent("wuddle-iced")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let entries: Vec<RepoFileEntry> = match fi.forge {
        "github" => {
            let url = format!(
                "https://api.github.com/repos/{}/{}/contents/{}",
                fi.owner, fi.repo, dir_path
            );
            let mut req = client
                .get(&url)
                .header("Accept", "application/vnd.github+json");
            if let Some(token) = wuddle_engine::github_token() {
                req = req.bearer_auth(token);
            }
            match req.send().await {
                Ok(r) if r.status().is_success() => r
                    .json::<Vec<ContentEntry>>()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| RepoFileEntry {
                        is_dir: e.kind == "dir",
                        path: format!("{}/{}", dir_path, e.name),
                        name: e.name,
                    })
                    .collect(),
                _ => Vec::new(),
            }
        }
        "gitlab" => {
            let encoded = format!("{}/{}", fi.owner, fi.repo).replace('/', "%2F");
            let url = format!(
                "https://gitlab.com/api/v4/projects/{}/repository/tree?path={}&per_page=50",
                encoded, dir_path
            );
            match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => r
                    .json::<Vec<ContentEntry>>()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| RepoFileEntry {
                        is_dir: e.kind == "tree",
                        path: format!("{}/{}", dir_path, e.name),
                        name: e.name,
                    })
                    .collect(),
                _ => Vec::new(),
            }
        }
        _ => {
            let url = format!(
                "{}://{}/api/v1/repos/{}/{}/contents/{}",
                fi.scheme, fi.host, fi.owner, fi.repo, dir_path
            );
            match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => r
                    .json::<Vec<ContentEntry>>()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| RepoFileEntry {
                        is_dir: e.kind == "dir",
                        path: format!("{}/{}", dir_path, e.name),
                        name: e.name,
                    })
                    .collect(),
                _ => Vec::new(),
            }
        }
    };
    Ok((dir_path, entries))
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub async fn fetch_repo_preview(url: String) -> Result<RepoPreviewInfo, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("fetch_repo_preview");
    let fi = parse_forge_url(&url).ok_or_else(|| "Could not parse repo URL".to_string())?;

    let client = Client::builder()
        .user_agent("wuddle-iced")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    match fi.forge {
        "github" => fetch_github_preview(&client, &fi.owner, &fi.repo).await,
        "gitlab" => fetch_gitlab_preview(&client, &fi.owner, &fi.repo).await,
        _ => fetch_gitea_preview(&client, &fi.host, &fi.scheme, &fi.owner, &fi.repo).await,
    }
}

// ---------------------------------------------------------------------------
// GitHub
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GhRepoInfo {
    name: Option<String>,
    description: Option<String>,
    stargazers_count: Option<u64>,
    forks_count: Option<u64>,
    language: Option<String>,
    license: Option<GhLicense>,
}
#[derive(Debug, Deserialize)]
struct GhLicense {
    spdx_id: Option<String>,
}

async fn fetch_github_preview(
    client: &Client,
    owner: &str,
    repo: &str,
) -> Result<RepoPreviewInfo, String> {
    let info_url = format!("https://api.github.com/repos/{}/{}", owner, repo);
    let mut req = client
        .get(&info_url)
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = wuddle_engine::github_token() {
        req = req.bearer_auth(token);
    }
    let info: GhRepoInfo = req
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let readme_url = format!("https://api.github.com/repos/{}/{}/readme", owner, repo);
    let mut readme_req = client
        .get(&readme_url)
        .header("Accept", "application/vnd.github.raw+json");
    if let Some(token) = wuddle_engine::github_token() {
        readme_req = readme_req.bearer_auth(token);
    }
    let readme_text = match readme_req.send().await {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        _ => String::new(),
    };

    let raw_base = format!("https://raw.githubusercontent.com/{}/{}/HEAD/", owner, repo);
    // Convert HTML <img> tags to markdown syntax so iced's parser creates Image items
    let readme_md = convert_html_images_to_markdown(&readme_text);
    let md_content = iced::widget::markdown::Content::parse(&readme_md);
    let readme_items: Vec<iced::widget::markdown::Item> = md_content.items().to_vec();
    let image_urls: Vec<String> = md_content.images().iter().cloned().collect();
    let (image_cache, gif_cache) = fetch_images(client, &image_urls, &raw_base).await;

    let files = fetch_files(client, "github", "github.com", owner, repo, "https").await;

    let license = info.license.and_then(|l| l.spdx_id).unwrap_or_default();
    let license = if license == "NOASSERTION" || license.is_empty() {
        String::new()
    } else {
        license
    };

    Ok(RepoPreviewInfo {
        name: info.name.unwrap_or_else(|| repo.to_string()),
        description: info.description.unwrap_or_default(),
        stars: info.stargazers_count.unwrap_or(0),
        forks: info.forks_count.unwrap_or(0),
        language: info.language.unwrap_or_default(),
        license,
        readme_items,
        readme_text,
        image_cache,
        gif_cache,
        files,
        raw_base_url: raw_base,
        forge: "github".into(),
        owner: owner.into(),
        repo_name: repo.into(),
        forge_url: format!("https://github.com/{}/{}", owner, repo),
    })
}

// ---------------------------------------------------------------------------
// GitLab
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GlProject {
    name: Option<String>,
    description: Option<String>,
    star_count: Option<u64>,
    forks_count: Option<u64>,
}

async fn fetch_gitlab_preview(
    client: &Client,
    owner: &str,
    repo: &str,
) -> Result<RepoPreviewInfo, String> {
    let encoded = format!("{}/{}", owner, repo).replace('/', "%2F");
    let url = format!("https://gitlab.com/api/v4/projects/{}", encoded);
    let info: GlProject = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let readme_url = format!("https://gitlab.com/{}/{}/raw/HEAD/README.md", owner, repo);
    let readme_text = match client.get(&readme_url).send().await {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        _ => String::new(),
    };

    let raw_base = format!("https://gitlab.com/{}/{}/raw/HEAD/", owner, repo);
    let readme_md = convert_html_images_to_markdown(&readme_text);
    let md_content = iced::widget::markdown::Content::parse(&readme_md);
    let readme_items: Vec<iced::widget::markdown::Item> = md_content.items().to_vec();
    let image_urls: Vec<String> = md_content.images().iter().cloned().collect();
    let (image_cache, gif_cache) = fetch_images(client, &image_urls, &raw_base).await;
    let files = fetch_files(client, "gitlab", "gitlab.com", owner, repo, "https").await;

    Ok(RepoPreviewInfo {
        name: info.name.unwrap_or_else(|| repo.to_string()),
        description: info.description.unwrap_or_default(),
        stars: info.star_count.unwrap_or(0),
        forks: info.forks_count.unwrap_or(0),
        language: String::new(),
        license: String::new(),
        readme_items,
        readme_text,
        image_cache,
        gif_cache,
        files,
        raw_base_url: raw_base,
        forge: "gitlab".into(),
        owner: owner.into(),
        repo_name: repo.into(),
        forge_url: format!("https://gitlab.com/{}/{}", owner, repo),
    })
}

// ---------------------------------------------------------------------------
// Gitea / Codeberg / Forgejo
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GiteaRepo {
    name: Option<String>,
    description: Option<String>,
    stars_count: Option<u64>,
    forks_count: Option<u64>,
    language: Option<String>,
    default_branch: Option<String>,
}

async fn fetch_gitea_preview(
    client: &Client,
    host: &str,
    scheme: &str,
    owner: &str,
    repo: &str,
) -> Result<RepoPreviewInfo, String> {
    let api_url = format!("{}://{}/api/v1/repos/{}/{}", scheme, host, owner, repo);
    let info: GiteaRepo = client
        .get(&api_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let branch = info.default_branch.as_deref().unwrap_or("master");
    let readme_url = format!(
        "{}://{}/{}/{}/raw/branch/{}/README.md",
        scheme, host, owner, repo, branch
    );
    let readme_text = match client.get(&readme_url).send().await {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        _ => String::new(),
    };

    let raw_base = format!(
        "{}://{}/{}/{}/raw/branch/{}/",
        scheme, host, owner, repo, branch
    );
    let readme_md = convert_html_images_to_markdown(&readme_text);
    let md_content = iced::widget::markdown::Content::parse(&readme_md);
    let readme_items: Vec<iced::widget::markdown::Item> = md_content.items().to_vec();
    let image_urls: Vec<String> = md_content.images().iter().cloned().collect();
    let (image_cache, gif_cache) = fetch_images(client, &image_urls, &raw_base).await;
    let files = fetch_files(client, "gitea", host, owner, repo, scheme).await;

    Ok(RepoPreviewInfo {
        name: info.name.unwrap_or_else(|| repo.to_string()),
        description: info.description.unwrap_or_default(),
        stars: info.stars_count.unwrap_or(0),
        forks: info.forks_count.unwrap_or(0),
        language: info.language.unwrap_or_default(),
        license: String::new(),
        readme_items,
        readme_text,
        image_cache,
        gif_cache,
        files,
        raw_base_url: raw_base,
        forge: "gitea".into(),
        owner: owner.into(),
        repo_name: repo.into(),
        forge_url: format!("{}://{}/{}/{}", scheme, host, owner, repo),
    })
}

// ---------------------------------------------------------------------------
// WeirdUtils Dynamic Info
// ---------------------------------------------------------------------------

/// Fetch and parse the WeirdUtils README to find a live description for a specific DLL.
pub async fn fetch_dll_description(dll_name: String) -> Result<(String, String), String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("fetch_dll_description");
    let url = "https://codeberg.org/MarcelineVQ/WeirdUtils/raw/branch/main/README.md";
    let client = Client::builder()
        .user_agent("wuddle-iced")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let readme = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Read error: {}", e))?;

    if let Some(desc) = extract_dll_info_from_readme(&readme, &dll_name) {
        Ok((dll_name, desc))
    } else {
        Err(format!(
            "No documentation found for '{}' in WeirdUtils README.",
            dll_name
        ))
    }
}

fn extract_dll_info_from_readme(readme: &str, target_dll: &str) -> Option<String> {
    // WeirdUtils README uses --- to separate feature blocks.
    let segments: Vec<&str> = readme.split("---").collect();
    let target_base = target_dll.to_lowercase().replace(".dll", "");

    for segment in segments {
        let lower = segment.to_lowercase();
        if lower.contains(&format!("**dll:** `{}`", target_dll.to_lowercase()))
            || lower.contains(&format!("**dll:** `{}`", target_base))
        {
            let lines: Vec<&str> = segment.lines().collect();
            let mut start_idx = 0;
            let mut end_idx = lines.len();

            for (i, line) in lines.iter().enumerate() {
                if line.trim().starts_with("### ") && start_idx == 0 {
                    start_idx = i;
                }
                if line.to_lowercase().contains("**dll:**") {
                    end_idx = i + 1;
                    break;
                }
            }

            let extracted = lines[start_idx..end_idx].join("\n").trim().to_string();
            if !extracted.is_empty() {
                return Some(extracted);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tweak wrappers (delegates to crate::tweaks which ports vanilla-tweaks)
// ---------------------------------------------------------------------------

pub async fn read_tweaks(
    wow_dir: String,
    auto_launch_exe: Option<String>,
) -> Result<crate::tweaks::ReadTweakValues, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("read_tweaks");
    tokio::task::spawn_blocking(move || {
        crate::tweaks::read_tweaks(std::path::Path::new(&wow_dir), auto_launch_exe.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn apply_tweaks(
    wow_dir: String,
    auto_launch_exe: Option<String>,
    opts: crate::tweaks::TweakOptions,
) -> Result<String, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("apply_tweaks");
    tokio::task::spawn_blocking(move || {
        crate::tweaks::apply_tweaks(
            std::path::Path::new(&wow_dir),
            auto_launch_exe.as_deref(),
            &opts,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn restore_tweaks(
    wow_dir: String,
    auto_launch_exe: Option<String>,
) -> Result<String, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("restore_tweaks");
    tokio::task::spawn_blocking(move || {
        crate::tweaks::restore_backup(std::path::Path::new(&wow_dir), auto_launch_exe.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Releases (for in-app Release Notes)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ReleaseItem {
    pub tag_name: String,
    pub name: String,
    pub published_at: String,
    pub body: String,
    pub items: Vec<iced::widget::markdown::Item>,
    pub prerelease: bool,
}

pub async fn fetch_releases(forge_url: String) -> Result<Vec<ReleaseItem>, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("fetch_releases");
    let fi = parse_forge_url(&forge_url).ok_or_else(|| "Could not parse forge URL".to_string())?;

    let client = Client::builder()
        .user_agent("wuddle-iced")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    match fi.forge {
        "github" => {
            #[derive(Deserialize)]
            struct GhRelease {
                tag_name: String,
                name: Option<String>,
                published_at: Option<String>,
                body: Option<String>,
                prerelease: bool,
            }
            let url = format!(
                "https://api.github.com/repos/{}/{}/releases?per_page=20",
                fi.owner, fi.repo
            );
            let mut req = client
                .get(&url)
                .header("Accept", "application/vnd.github+json");
            if let Some(token) = wuddle_engine::github_token() {
                req = req.bearer_auth(token);
            }
            let releases: Vec<GhRelease> =
                tokio::time::timeout(std::time::Duration::from_secs(15), req.send())
                    .await
                    .map_err(|_| "Timed out fetching releases".to_string())?
                    .map_err(|e| e.to_string())?
                    .json()
                    .await
                    .map_err(|e| e.to_string())?;
            Ok(releases
                .into_iter()
                .map(|r| {
                    let body = r.body.unwrap_or_default();
                    let items = iced::widget::markdown::Content::parse(&body)
                        .items()
                        .to_vec();
                    ReleaseItem {
                        tag_name: r.tag_name.clone(),
                        name: r
                            .name
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| r.tag_name),
                        published_at: r.published_at.unwrap_or_default(),
                        body,
                        items,
                        prerelease: r.prerelease,
                    }
                })
                .collect())
        }
        "gitlab" => {
            #[derive(Deserialize)]
            struct GlRelease {
                tag_name: String,
                name: Option<String>,
                released_at: Option<String>,
                description: Option<String>,
            }
            let encoded = format!("{}/{}", fi.owner, fi.repo).replace('/', "%2F");
            let url = format!("https://gitlab.com/api/v4/projects/{}/releases", encoded);
            let releases: Vec<GlRelease> =
                tokio::time::timeout(std::time::Duration::from_secs(15), client.get(&url).send())
                    .await
                    .map_err(|_| "Timed out fetching releases".to_string())?
                    .map_err(|e| e.to_string())?
                    .json()
                    .await
                    .map_err(|e| e.to_string())?;
            Ok(releases
                .into_iter()
                .map(|r| {
                    let body = r.description.unwrap_or_default();
                    let items = iced::widget::markdown::Content::parse(&body)
                        .items()
                        .to_vec();
                    ReleaseItem {
                        tag_name: r.tag_name.clone(),
                        name: r
                            .name
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| r.tag_name),
                        published_at: r.released_at.unwrap_or_default(),
                        body,
                        items,
                        prerelease: false,
                    }
                })
                .collect())
        }
        _ => {
            // Gitea / Forgejo / Codeberg
            #[derive(Deserialize)]
            struct GiteaRelease {
                tag_name: String,
                name: Option<String>,
                published_at: Option<String>,
                body: Option<String>,
                prerelease: bool,
            }
            let url = format!(
                "{}://{}/api/v1/repos/{}/{}/releases?limit=20",
                fi.scheme, fi.host, fi.owner, fi.repo
            );
            let releases: Vec<GiteaRelease> =
                tokio::time::timeout(std::time::Duration::from_secs(15), client.get(&url).send())
                    .await
                    .map_err(|_| "Timed out fetching releases".to_string())?
                    .map_err(|e| e.to_string())?
                    .json()
                    .await
                    .map_err(|e| e.to_string())?;
            Ok(releases
                .into_iter()
                .map(|r| {
                    let body = r.body.unwrap_or_default();
                    let items = iced::widget::markdown::Content::parse(&body)
                        .items()
                        .to_vec();
                    ReleaseItem {
                        tag_name: r.tag_name.clone(),
                        name: r
                            .name
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| r.tag_name),
                        published_at: r.published_at.unwrap_or_default(),
                        body,
                        items,
                        prerelease: r.prerelease,
                    }
                })
                .collect())
        }
    }
}

// ---------------------------------------------------------------------------
// Self-update: fetch latest GitHub release tag
// ---------------------------------------------------------------------------

const WUDDLE_RELEASE_API_LATEST: &str =
    "https://api.github.com/repos/ZythDr/Wuddle/releases/latest";
const WUDDLE_RELEASE_API_ALL: &str =
    "https://api.github.com/repos/ZythDr/Wuddle/releases?per_page=5";
const WUDDLE_BETA_CHANGELOG_API: &str =
    "https://api.github.com/repos/ZythDr/Wuddle/releases?per_page=20";
const CHANGELOG_URL: &str = "https://raw.githubusercontent.com/ZythDr/Wuddle/main/CHANGELOG.md";
const BETA_CHANGELOG_URL: &str =
    "https://raw.githubusercontent.com/ZythDr/Wuddle/beta/CHANGELOG.md";
const CHANGELOG_EMBEDDED: &str = include_str!("../../CHANGELOG.md");

#[derive(Debug, Deserialize)]
struct GhChangelogRelease {
    tag_name: String,
    body: Option<String>,
    prerelease: bool,
    draft: bool,
}

fn strip_changelog_preamble(markdown: &str) -> String {
    let lines = markdown.lines().collect::<Vec<_>>();
    let first_release = lines.iter().position(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("## ") && !trimmed.starts_with("### ")
    });
    first_release
        .map(|index| lines[index..].join("\n"))
        .unwrap_or_else(|| markdown.trim().to_string())
}

fn strip_matching_release_heading<'a>(body: &'a str, tag: &str) -> &'a str {
    let trimmed = body.trim();
    let (first_line, remainder) = trimmed.split_once('\n').unwrap_or((trimmed, ""));
    let heading = first_line.trim().trim_start_matches('#').trim();
    if first_line.trim_start().starts_with('#')
        && heading
            .to_ascii_lowercase()
            .contains(&tag.trim().to_ascii_lowercase())
    {
        remainder.trim_start()
    } else {
        trimmed
    }
}

fn format_beta_changelog(releases: Vec<GhChangelogRelease>) -> Option<String> {
    let sections = releases
        .into_iter()
        .filter(|release| release.prerelease && !release.draft)
        .map(|release| {
            let tag = release.tag_name.trim();
            let body = release
                .body
                .as_deref()
                .map(str::trim)
                .filter(|body| !body.is_empty())
                .unwrap_or("No release notes were provided for this beta.");
            let body = strip_matching_release_heading(body, tag);
            format!("## {tag}\n\n{body}")
        })
        .collect::<Vec<_>>();

    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n\n---\n\n"))
    }
}

async fn fetch_beta_changelog(client: &Client) -> Result<String, String> {
    let mut request = client
        .get(WUDDLE_BETA_CHANGELOG_API)
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = wuddle_engine::github_token() {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("GitHub API error: HTTP {}", response.status()));
    }
    let releases = response
        .json::<Vec<GhChangelogRelease>>()
        .await
        .map_err(|error| error.to_string())?;
    format_beta_changelog(releases).ok_or_else(|| "No beta release notes found".to_string())
}

pub async fn fetch_changelog(beta_channel: bool) -> Result<String, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("fetch_changelog");
    let client = Client::builder()
        .user_agent(concat!("wuddle/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    if beta_channel {
        if let Ok(changelog) = fetch_beta_changelog(&client).await {
            return Ok(changelog);
        }
    }

    let fallback_url = if beta_channel {
        BETA_CHANGELOG_URL
    } else {
        CHANGELOG_URL
    };
    match client.get(fallback_url).send().await {
        Ok(resp) if resp.status().is_success() => resp
            .text()
            .await
            .map(|text| strip_changelog_preamble(&text))
            .map_err(|e| e.to_string()),
        _ => Ok(strip_changelog_preamble(CHANGELOG_EMBEDDED)),
    }
}

/// Write the generated dxvk.conf content to the given path.
pub async fn save_dxvk_conf(path: std::path::PathBuf, content: String) -> Result<(), String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("save_dxvk_conf");
    tokio::task::spawn_blocking(move || {
        std::fs::write(&path, content.as_bytes()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Self-update: download, apply, restart
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SelfUpdateStatus {
    pub supported: bool,
    pub update_available: bool,
    pub assets_pending: bool,
    pub latest_version: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct GhReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GhReleaseFull {
    tag_name: String,
    assets: Vec<GhReleaseAsset>,
}

fn normalize_tag(raw: &str) -> String {
    raw.trim().trim_start_matches(['v', 'V']).trim().to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreReleaseIdentifier {
    Numeric(u64),
    Text(String),
}

/// Parse the SemVer parts needed by the updater. Build metadata is deliberately
/// ignored, while the individual pre-release identifiers remain significant:
/// `3.6.0-beta.3` must sort after `3.6.0-beta.2`.
fn parse_version_parts(raw: &str) -> (Vec<u64>, Option<Vec<PreReleaseIdentifier>>) {
    let tag = normalize_tag(raw);
    let without_build = tag
        .split_once('+')
        .map_or(tag.as_str(), |(version, _)| version);
    let (core_raw, pre_raw) = without_build
        .split_once('-')
        .map_or((without_build, None), |(core, pre)| (core, Some(pre)));
    let core = core_raw
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect();
    let prerelease = pre_raw.map(|pre| {
        pre.split('.')
            .filter(|part| !part.is_empty())
            .map(|part| match part.parse::<u64>() {
                Ok(number) => PreReleaseIdentifier::Numeric(number),
                Err(_) => PreReleaseIdentifier::Text(part.to_ascii_lowercase()),
            })
            .collect()
    });
    (core, prerelease)
}

fn compare_pre_release_identifier(
    left: &PreReleaseIdentifier,
    right: &PreReleaseIdentifier,
) -> std::cmp::Ordering {
    use PreReleaseIdentifier::{Numeric, Text};

    match (left, right) {
        (Numeric(a), Numeric(b)) => a.cmp(b),
        // SemVer specifies that numeric identifiers have lower precedence than
        // non-numeric identifiers.
        (Numeric(_), Text(_)) => std::cmp::Ordering::Less,
        (Text(_), Numeric(_)) => std::cmp::Ordering::Greater,
        (Text(a), Text(b)) => a.cmp(b),
    }
}

fn is_version_newer(latest: &str, current: &str) -> bool {
    let (latest_core, latest_pre) = parse_version_parts(latest);
    let (current_core, current_pre) = parse_version_parts(current);
    let max = latest_core.len().max(current_core.len());
    for i in 0..max {
        let latest_part = *latest_core.get(i).unwrap_or(&0);
        let current_part = *current_core.get(i).unwrap_or(&0);
        match latest_part.cmp(&current_part) {
            std::cmp::Ordering::Greater => return true,
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => {}
        }
    }

    match (latest_pre, current_pre) {
        // A final release is newer than a pre-release with the same core.
        (None, Some(_)) => true,
        (Some(_), None) | (None, None) => false,
        (Some(latest_pre), Some(current_pre)) => {
            let max = latest_pre.len().max(current_pre.len());
            for i in 0..max {
                match (latest_pre.get(i), current_pre.get(i)) {
                    // A longer matching pre-release identifier list has higher
                    // precedence, e.g. `beta.1.1` > `beta.1`.
                    (Some(_), None) => return true,
                    (None, Some(_)) => return false,
                    (Some(latest_identifier), Some(current_identifier)) => {
                        match compare_pre_release_identifier(latest_identifier, current_identifier)
                        {
                            std::cmp::Ordering::Greater => return true,
                            std::cmp::Ordering::Less => return false,
                            std::cmp::Ordering::Equal => {}
                        }
                    }
                    (None, None) => break,
                }
            }
            false
        }
    }
}

#[cfg(test)]
mod self_update_version_tests {
    use super::{
        format_beta_changelog, is_version_newer, strip_changelog_preamble, GhChangelogRelease,
    };

    #[test]
    fn beta_sequence_is_compared() {
        assert!(is_version_newer("3.6.0-beta.3", "3.6.0-beta.2"));
        assert!(!is_version_newer("3.6.0-beta.2", "3.6.0-beta.3"));
    }

    #[test]
    fn prerelease_and_stable_precedence_is_respected() {
        assert!(is_version_newer("3.6.0", "3.6.0-beta.3"));
        assert!(!is_version_newer("3.6.0-beta.3", "3.6.0"));
        assert!(is_version_newer("3.6.0-rc.1", "3.6.0-beta.3"));
    }

    #[test]
    fn beta_changelog_contains_only_published_prereleases() {
        let changelog = format_beta_changelog(vec![
            GhChangelogRelease {
                tag_name: "v3.7.0-beta.2".to_string(),
                body: Some("## v3.7.0-beta.2\n\nBeta two changes".to_string()),
                prerelease: true,
                draft: false,
            },
            GhChangelogRelease {
                tag_name: "v3.7.0".to_string(),
                body: Some("Stable changes".to_string()),
                prerelease: false,
                draft: false,
            },
            GhChangelogRelease {
                tag_name: "v3.8.0-beta.1".to_string(),
                body: Some("Unpublished changes".to_string()),
                prerelease: true,
                draft: true,
            },
        ])
        .unwrap();

        assert!(changelog.contains("## v3.7.0-beta.2"));
        assert!(changelog.contains("Beta two changes"));
        assert_eq!(changelog.matches("v3.7.0-beta.2").count(), 1);
        assert!(!changelog.contains("Stable changes"));
        assert!(!changelog.contains("Unpublished changes"));
    }

    #[test]
    fn changelog_document_preamble_is_not_rendered_inside_the_dialog() {
        let markdown = "# Changelog\n\nAll notable changes are documented here.\n\n## v3.7.0\n\n### New Features\n\n- A feature";
        let visible = strip_changelog_preamble(markdown);

        assert!(visible.starts_with("## v3.7.0"));
        assert!(!visible.contains("# Changelog\n"));
        assert!(!visible.contains("All notable changes"));
    }

    #[test]
    fn build_metadata_does_not_affect_precedence() {
        assert!(!is_version_newer(
            "3.6.0-beta.3+linux.x86_64",
            "3.6.0-beta.3"
        ));
        assert!(is_version_newer("3.6.1-beta.1", "3.6.0-beta.2"));
    }
}

async fn fetch_release_full(beta_channel: bool) -> Result<GhReleaseFull, String> {
    let url = if beta_channel {
        WUDDLE_RELEASE_API_ALL
    } else {
        WUDDLE_RELEASE_API_LATEST
    };
    let client = Client::builder()
        .user_agent(concat!("wuddle/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = tokio::time::timeout(
        Duration::from_secs(25),
        client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .send(),
    )
    .await
    .map_err(|_| "Timed out fetching release".to_string())?
    .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API error: HTTP {}", resp.status()));
    }

    if beta_channel {
        let releases: Vec<GhReleaseFull> = resp.json().await.map_err(|e| e.to_string())?;
        releases
            .into_iter()
            .next()
            .ok_or_else(|| "No releases found".to_string())
    } else {
        resp.json().await.map_err(|e| e.to_string())
    }
}

async fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    let client = Client::builder()
        .user_agent(concat!("wuddle/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .send()
        .await
        .map_err(|e| format!("download: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("download HTTP {}", resp.status()));
    }
    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| e.to_string())
}

/// Check whether self-update is supported and whether an update is available.
pub async fn check_self_update_full(beta_channel: bool) -> Result<SelfUpdateStatus, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("check_self_update_full");
    crate::diagnostics::trace(
        "self_update",
        format!("check_self_update_full: beta_channel={beta_channel}"),
    );
    let current = env!("CARGO_PKG_VERSION");
    let supported = is_self_update_supported();

    let release = match fetch_release_full(beta_channel).await {
        Ok(r) => r,
        Err(e) => {
            return Ok(SelfUpdateStatus {
                supported,
                update_available: false,
                assets_pending: false,
                latest_version: None,
                message: format!("Version check failed: {}", e),
            })
        }
    };

    let latest = normalize_tag(&release.tag_name);
    let newer = !latest.is_empty() && is_version_newer(&latest, current);
    let has_asset = newer && pick_platform_asset(&release).is_some();

    let message = if !supported {
        format!(
            "v{} — self-update not supported for this install type",
            latest
        )
    } else if newer && !has_asset {
        format!(
            "v{} available but assets still building — try again shortly",
            latest
        )
    } else if newer {
        format!("Update available: v{}", latest)
    } else {
        "Up to date".to_string()
    };

    let assets_pending = newer && !has_asset;

    Ok(SelfUpdateStatus {
        supported,
        update_available: has_asset && supported,
        assets_pending,
        latest_version: if latest.is_empty() {
            None
        } else {
            Some(latest)
        },
        message,
    })
}

fn is_self_update_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        return is_appimage().is_some();
    }
    #[cfg(target_os = "windows")]
    {
        return detect_launcher_root().map(|r| r.1).unwrap_or(false);
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        return false;
    }
}

fn pick_platform_asset(release: &GhReleaseFull) -> Option<&GhReleaseAsset> {
    #[cfg(target_os = "linux")]
    {
        release.assets.iter().find(|a| {
            let lower = a.name.to_ascii_lowercase();
            lower.ends_with(".appimage")
        })
    }
    #[cfg(target_os = "windows")]
    {
        release.assets.iter().find(|a| {
            let lower = a.name.to_ascii_lowercase();
            lower.contains("windows") && lower.ends_with(".zip")
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Download and apply the latest release. Returns a status message.
pub async fn apply_self_update(beta_channel: bool) -> Result<String, String> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("apply_self_update");
    let current = env!("CARGO_PKG_VERSION");
    let release = fetch_release_full(beta_channel).await?;
    let latest = normalize_tag(&release.tag_name);

    if latest.is_empty() {
        return Err("Latest release tag is empty".to_string());
    }
    if !is_version_newer(&latest, current) {
        return Ok(format!("Already up to date (v{}).", current));
    }

    let asset = pick_platform_asset(&release)
        .ok_or_else(|| "No compatible asset found in release".to_string())?;
    let url = asset.browser_download_url.clone();
    let asset_name = asset.name.clone();

    let bytes = download_bytes(&url).await?;

    // Apply in a blocking task (filesystem I/O)
    let latest_clone = latest.clone();
    tokio::task::spawn_blocking(move || apply_downloaded_update(&bytes, &asset_name, &latest_clone))
        .await
        .map_err(|e| e.to_string())?
}

fn apply_downloaded_update(
    bytes: &[u8],
    _asset_name: &str,
    latest: &str,
) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let appimage_path = is_appimage()
            .ok_or_else(|| "Not running as AppImage; self-update unavailable.".to_string())?;

        // Clean up stale temp files
        if let Some(parent) = appimage_path.parent() {
            if let Some(stem) = appimage_path.file_stem().and_then(|s| s.to_str()) {
                if let Ok(entries) = std::fs::read_dir(parent) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let name = name.to_string_lossy();
                        if name.starts_with(stem) && name.contains(".tmp-") {
                            let _ = std::fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let tmp_path = appimage_path.with_extension(format!("tmp-{}", stamp));

        std::fs::write(&tmp_path, bytes).map_err(|e| format!("Failed to write temp file: {e}"))?;

        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to chmod: {e}"))?;

        std::fs::rename(&tmp_path, &appimage_path)
            .map_err(|e| format!("Failed to replace AppImage: {e}"))?;

        Ok(format!("Updated to v{}. Restart to apply.", latest))
    }

    #[cfg(target_os = "windows")]
    {
        let (root, launcher_layout) =
            detect_launcher_root().map_err(|e| format!("Cannot detect install layout: {e}"))?;
        if !launcher_layout {
            return Err("Launcher layout not detected. Install the latest portable package once to enable in-app updates.".to_string());
        }

        // Extract Wuddle-bin.exe from the zip into versions/<tag>/
        let cursor = std::io::Cursor::new(bytes);
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| format!("Failed to open zip: {e}"))?;

        let sanitized = sanitize_version_name(latest);
        let version_dir = root.join("versions").join(&sanitized);
        std::fs::create_dir_all(&version_dir).map_err(|e| e.to_string())?;

        let mut found_runtime = false;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
            if file.is_dir() {
                continue;
            }
            let name = file.name().replace('\\', "/");
            let lower = name.to_ascii_lowercase();
            if lower.ends_with("/wuddle-bin.exe") || lower == "wuddle-bin.exe" {
                let target = version_dir.join("Wuddle-bin.exe");
                let mut out = std::fs::File::create(&target).map_err(|e| e.to_string())?;
                std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
                found_runtime = true;
                break;
            }
        }
        if !found_runtime {
            return Err("Wuddle-bin.exe not found in update zip".to_string());
        }

        // Update current.json
        let current_json = serde_json::json!({ "current": format!("v{}", sanitized) });
        std::fs::write(
            root.join("current.json"),
            current_json.to_string().as_bytes(),
        )
        .map_err(|e| format!("Failed to write current.json: {e}"))?;

        Ok(format!("Staged v{}. Restart to apply.", latest))
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (bytes, _asset_name, latest);
        Err("Self-update not supported on this platform".to_string())
    }
}

/// Restart the application after a successful update.
pub fn restart_app() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let appimage_path =
            is_appimage().ok_or_else(|| "Not running as AppImage; cannot restart.".to_string())?;
        Command::new(&appimage_path)
            .spawn()
            .map_err(|e| format!("Failed to relaunch: {e}"))?;
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(200));
            std::process::exit(0);
        });
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let (root, _) =
            detect_launcher_root().map_err(|e| format!("Cannot detect launcher: {e}"))?;
        let launcher = root.join("Wuddle.exe");
        if !launcher.is_file() {
            return Err(format!("Launcher not found at {}", launcher.display()));
        }
        Command::new(&launcher)
            .current_dir(&root)
            .spawn()
            .map_err(|e| format!("Failed to relaunch: {e}"))?;
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(200));
            std::process::exit(0);
        });
        Ok(())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Err("Restart not supported on this platform".to_string())
    }
}

#[cfg(target_os = "linux")]
fn is_appimage() -> Option<PathBuf> {
    let path = std::env::var("APPIMAGE").ok()?;
    let p = PathBuf::from(path);
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn detect_launcher_root() -> Result<(PathBuf, bool), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // Walk up to find the root that contains Wuddle.exe (launcher) and versions/
    let mut dir = exe.parent().map(|p| p.to_path_buf());
    for _ in 0..4 {
        if let Some(ref d) = dir {
            let launcher = d.join("Wuddle.exe");
            let versions = d.join("versions");
            if launcher.is_file() && versions.is_dir() {
                return Ok((d.clone(), true));
            }
            dir = d.parent().map(|p| p.to_path_buf());
        } else {
            break;
        }
    }
    // No launcher layout found
    let root = exe.parent().unwrap_or(Path::new(".")).to_path_buf();
    Ok((root, false))
}

// ---------------------------------------------------------------------------
// GitHub rate limit info
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GitHubRateInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset_epoch: i64,
}

pub async fn fetch_github_rate_limit() -> Option<GitHubRateInfo> {
    let _diagnostic = crate::diagnostics::OperationGuard::new("fetch_github_rate_limit");
    #[derive(Deserialize)]
    struct RateLimitResponse {
        rate: RateCore,
    }
    #[derive(Deserialize)]
    struct RateCore {
        limit: u32,
        remaining: u32,
        reset: i64,
    }

    let mut req = reqwest::Client::new()
        .get("https://api.github.com/rate_limit")
        .header("User-Agent", concat!("Wuddle/", env!("CARGO_PKG_VERSION")));

    if let Some(token) = wuddle_engine::github_token() {
        req = req.bearer_auth(token);
    }

    let resp = req.send().await.ok()?;
    let data: RateLimitResponse = resp.json().await.ok()?;
    Some(GitHubRateInfo {
        limit: data.rate.limit,
        remaining: data.rate.remaining,
        reset_epoch: data.rate.reset,
    })
}

#[cfg(target_os = "windows")]
fn sanitize_version_name(raw: &str) -> String {
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
