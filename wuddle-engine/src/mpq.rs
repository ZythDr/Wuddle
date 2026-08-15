//! Generic MPQ patch inspection and filesystem safety helpers.
//!
//! This module deliberately knows nothing about WDM or any other curated mod.
//! It validates and stages local/remote MPQ payloads, detects WoW locales, and
//! classifies existing client archives so the engine can persist them safely.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use reqwest::Url;
use tempfile::{Builder, TempDir};

use crate::{db, diagnostics, install, util, InstallMode, Repo};

pub const KNOWN_LOCALES: &[&str] = &[
    "enGB", "enUS", "deDE", "esES", "frFR", "koKR", "zhCN", "zhTW", "enCN", "enTW", "esMX", "ruRU",
];

const MPQ_HEADER: &[u8; 4] = b"MPQ\x1A";
const MPQ_USER_DATA: &[u8; 4] = b"MPQ\x1B";
const MPQ_MIN_HEADER_SIZE: u32 = 0x20;
const MPQ_HEADER_ALIGNMENT: u64 = 0x200;
const MPQ_HEADER_SEARCH_LIMIT: u64 = 0x0800_0000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MpqError {
    UnsupportedSource,
    InvalidArchive,
    NoMpqFiles,
    InvalidMpq(String),
    InvalidSelection(String),
    ProtectedTarget(String),
    ManagedTarget(String),
    ReplacementNotApproved(String),
    ModifiedTarget(String),
    MissingTarget(String),
    ToggleCollision(String),
    RestoredTargetModified(String),
    Filesystem(&'static str),
}

impl fmt::Display for MpqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSource => {
                f.write_str("Choose a local .mpq, .zip, or .7z file")
            }
            Self::InvalidArchive => f.write_str("The selected archive could not be read safely"),
            Self::NoMpqFiles => f.write_str("No MPQ files were found in the selected source"),
            Self::InvalidMpq(name) => write!(f, "{name} is not a valid MPQ archive"),
            Self::InvalidSelection(message) => f.write_str(message),
            Self::ProtectedTarget(name) => {
                write!(f, "{name} is protected; choose a different MPQ filename")
            }
            Self::ManagedTarget(name) => write!(
                f,
                "{name} is managed by another Wuddle package; rename it or remove that package first"
            ),
            Self::ReplacementNotApproved(name) => write!(
                f,
                "{name} already exists; approve a backed-up replacement or choose another filename"
            ),
            Self::ModifiedTarget(name) => write!(
                f,
                "{name} changed after Wuddle installed it; keep and protect it or confirm forced removal"
            ),
            Self::MissingTarget(name) => {
                write!(f, "{name} is missing and cannot be enabled or disabled")
            }
            Self::ToggleCollision(name) => write!(
                f,
                "{name} cannot be enabled or disabled because the destination filename already exists"
            ),
            Self::RestoredTargetModified(name) => write!(
                f,
                "{name} changed while the Wuddle patch was disabled; resolve that file before enabling the patch"
            ),
            Self::Filesystem(action) => write!(f, "MPQ filesystem operation failed while {action}"),
        }
    }
}

impl std::error::Error for MpqError {}

pub type MpqResult<T> = Result<T, MpqError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleEvidence {
    pub locale: String,
    pub source: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocaleDetection {
    pub recommended: Option<String>,
    pub candidates: Vec<String>,
    pub evidence: Vec<LocaleEvidence>,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MpqDestination {
    DataRoot,
    Locale(String),
}

impl MpqDestination {
    pub fn label(&self) -> String {
        match self {
            Self::DataRoot => "Data/".to_string(),
            Self::Locale(locale) => format!("Data/{locale}/"),
        }
    }

    pub fn manifest_path(&self, file_name: &str) -> String {
        match self {
            Self::DataRoot => format!("Data/{file_name}"),
            Self::Locale(locale) => format!("Data/{locale}/{file_name}"),
        }
    }
}

impl fmt::Display for MpqDestination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpqCandidate {
    /// Stable slash-separated path inside the selected package.
    pub source_key: String,
    pub original_file_name: String,
    pub suggested_display_name: String,
    pub suggested_destination: MpqDestination,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct MpqInspection {
    pub source_path: PathBuf,
    pub package_name: String,
    pub locale: LocaleDetection,
    pub destinations: Vec<MpqDestination>,
    pub candidates: Vec<MpqCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpqInstallSelection {
    pub source_key: String,
    pub display_name: String,
    pub file_name: String,
    pub destination: MpqDestination,
    pub replace_unprotected: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpqInstalledFile {
    pub path: String,
    pub display_name: String,
    pub sha256: String,
    pub version: Option<String>,
    pub enabled: bool,
    pub protected: bool,
    pub editor_unlocked: bool,
    pub status: MpqFileStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpqPackageFileEdit {
    pub path: String,
    pub display_name: String,
    pub file_name: String,
    pub destination: MpqDestination,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpqFileStatus {
    Installed,
    Missing,
    Modified,
}

impl MpqFileStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Installed => "Installed",
            Self::Missing => "Missing",
            Self::Modified => "Modified",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MpqRemotePackage {
    pub url: String,
    pub forge: String,
    pub host: String,
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct MpqRemoteAsset {
    pub asset_name: String,
    /// Optional user-selected deployment name. The upstream asset keeps its
    /// canonical name in staging while the committed MPQ preserves this name.
    pub target_file_name: Option<String>,
    pub download_url: String,
    pub size: Option<u64>,
    pub sha256: Option<String>,
    pub display_name: String,
    pub destination: MpqDestination,
    pub replace_unprotected: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpqProtectionEntry {
    pub path: String,
    pub file_name: String,
    pub display_name: Option<String>,
    /// Size/timestamps/file identity only; no MPQ contents are read while
    /// opening or refreshing the protection dialog.
    pub fingerprint: String,
    pub protected: bool,
    pub core: bool,
    pub editor_unlocked: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpqTargetStatus {
    Available,
    SamePackage,
    ManagedByAnotherPackage,
    ProtectedCore,
    ProtectedUntracked,
    UnprotectedReplacement,
}

impl MpqTargetStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Available => "Available",
            Self::SamePackage => "Transactional reinstall",
            Self::ManagedByAnotherPackage => "Managed by another package",
            Self::ProtectedCore => "Locked core archive",
            Self::ProtectedUntracked => "Protected untracked archive",
            Self::UnprotectedReplacement => "Backed-up replacement available",
        }
    }

    pub fn blocks_install(self) -> bool {
        matches!(
            self,
            Self::ManagedByAnotherPackage | Self::ProtectedCore | Self::ProtectedUntracked
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpqTargetPreview {
    pub source_key: String,
    pub manifest_path: String,
    pub status: MpqTargetStatus,
}

#[derive(Debug)]
pub(crate) struct StagedMpqSource {
    _temp_dir: TempDir,
    files: BTreeMap<String, PathBuf>,
}

impl StagedMpqSource {
    pub(crate) fn file(&self, source_key: &str) -> Option<&Path> {
        self.files.get(source_key).map(PathBuf::as_path)
    }
}

pub(crate) fn stage_files(
    wow_dir: &Path,
    sources: &[(String, PathBuf)],
) -> MpqResult<StagedMpqSource> {
    let staging_parent = util::cache_dir(Some(wow_dir))
        .map_err(|_| MpqError::Filesystem("creating the MPQ cache"))?
        .join("mpq-staging");
    fs::create_dir_all(&staging_parent)
        .map_err(|_| MpqError::Filesystem("creating the MPQ staging directory"))?;
    let temp_dir = Builder::new()
        .prefix("payload-")
        .tempdir_in(&staging_parent)
        .map_err(|_| MpqError::Filesystem("creating an MPQ staging operation"))?;
    let mut files = BTreeMap::new();
    for (source_key, source) in sources {
        let file_name = Path::new(source_key)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(MpqError::InvalidArchive)?;
        if !is_mpq_name(file_name) {
            return Err(MpqError::InvalidMpq(file_name.to_string()));
        }
        let destination = temp_dir.path().join(file_name);
        fs::copy(source, &destination)
            .map_err(|_| MpqError::Filesystem("copying an MPQ into staging"))?;
        validate_mpq_file(&destination).map_err(|_| MpqError::InvalidMpq(file_name.to_string()))?;
        if files.insert(source_key.clone(), destination).is_some() {
            return Err(MpqError::InvalidArchive);
        }
    }
    if files.is_empty() {
        return Err(MpqError::NoMpqFiles);
    }
    Ok(StagedMpqSource {
        _temp_dir: temp_dir,
        files,
    })
}

pub fn is_supported_local_source(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            let lower = name.to_ascii_lowercase();
            lower.ends_with(".mpq") || lower.ends_with(".zip") || lower.ends_with(".7z")
        })
        .unwrap_or(false)
}

pub fn normalize_locale(value: &str) -> Option<String> {
    KNOWN_LOCALES
        .iter()
        .find(|locale| locale.eq_ignore_ascii_case(value.trim()))
        .map(|locale| (*locale).to_string())
}

pub fn detect_wow_locale(wow_dir: &Path) -> LocaleDetection {
    let mut evidence = Vec::new();
    let mut candidates = BTreeSet::new();
    let mut configured = None;

    let config_path = wow_dir.join("WTF").join("Config.wtf");
    if let Ok(config) = fs::read_to_string(config_path) {
        for line in config.lines() {
            let trimmed = line.trim();
            let mut command = trimmed.splitn(2, char::is_whitespace);
            if !command
                .next()
                .map(|token| token.eq_ignore_ascii_case("SET"))
                .unwrap_or(false)
            {
                continue;
            }
            let rest = command.next().unwrap_or_default();
            let mut fields = rest.splitn(2, char::is_whitespace);
            let key = fields.next().unwrap_or_default();
            if !matches!(
                key.to_ascii_lowercase().as_str(),
                "locale" | "textlocale" | "audiolocale"
            ) {
                continue;
            }
            let raw = fields.next().unwrap_or_default().trim().trim_matches('"');
            if let Some(locale) = normalize_locale(raw) {
                if configured.is_none() || key.eq_ignore_ascii_case("locale") {
                    configured = Some(locale.clone());
                }
                candidates.insert(locale.clone());
                evidence.push(LocaleEvidence {
                    locale,
                    source: format!("WTF/Config.wtf ({key})"),
                });
            }
        }
    }

    let data = wow_dir.join("Data");
    if let Ok(entries) = fs::read_dir(&data) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let meta = match entry.file_type() {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            if meta.is_dir() {
                if let Some(locale) = normalize_locale(&name) {
                    candidates.insert(locale.clone());
                    evidence.push(LocaleEvidence {
                        locale: locale.clone(),
                        source: "Data locale directory".to_string(),
                    });
                    let realmlist = path.join("realmlist.wtf");
                    if realmlist.is_file() {
                        evidence.push(LocaleEvidence {
                            locale,
                            source: "Data locale realmlist.wtf".to_string(),
                        });
                    }
                }
                continue;
            }
            if meta.is_file() && is_mpq_name(&name) {
                if let Some(locale) = locale_from_file_name(&name) {
                    candidates.insert(locale.clone());
                    evidence.push(LocaleEvidence {
                        locale,
                        source: "Data locale archive".to_string(),
                    });
                }
            }
            if meta.is_file() && name.eq_ignore_ascii_case("realmlist.wtf") {
                if let Ok(contents) = fs::read_to_string(path) {
                    for locale in KNOWN_LOCALES {
                        if contents
                            .to_ascii_lowercase()
                            .contains(&locale.to_ascii_lowercase())
                        {
                            candidates.insert((*locale).to_string());
                            evidence.push(LocaleEvidence {
                                locale: (*locale).to_string(),
                                source: "Data/realmlist.wtf locale token".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    let candidates = candidates.into_iter().collect::<Vec<_>>();
    let recommended = configured.or_else(|| {
        if candidates.len() == 1 {
            candidates.first().cloned()
        } else {
            None
        }
    });
    let ambiguous = candidates.len() > 1;

    LocaleDetection {
        recommended,
        candidates,
        evidence,
        ambiguous,
    }
}

pub fn available_destinations(wow_dir: &Path, detection: &LocaleDetection) -> Vec<MpqDestination> {
    let mut out = vec![MpqDestination::DataRoot];
    let data = wow_dir.join("Data");
    let mut locales = BTreeSet::new();
    for locale in &detection.candidates {
        if find_case_insensitive_child(&data, locale)
            .map(|path| path.is_dir())
            .unwrap_or(false)
        {
            locales.insert(locale.clone());
        }
    }
    if let Ok(entries) = fs::read_dir(&data) {
        for entry in entries.flatten() {
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                if let Some(locale) = normalize_locale(&entry.file_name().to_string_lossy()) {
                    locales.insert(locale);
                }
            }
        }
    }
    out.extend(locales.into_iter().map(MpqDestination::Locale));
    out
}

pub fn inspect_local_source(wow_dir: &Path, source: &Path) -> MpqResult<MpqInspection> {
    let staged = stage_source(wow_dir, source)?;
    let locale = detect_wow_locale(wow_dir);
    let destinations = available_destinations(wow_dir, &locale);
    let mut candidates = Vec::new();

    for (source_key, path) in &staged.files {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(MpqError::InvalidArchive)?
            .to_string();
        let size = path
            .metadata()
            .map_err(|_| MpqError::Filesystem("reading a staged MPQ"))?
            .len();
        candidates.push(MpqCandidate {
            source_key: source_key.clone(),
            original_file_name: file_name.clone(),
            suggested_display_name: friendly_stem(&file_name),
            suggested_destination: suggest_destination(&file_name),
            size,
        });
    }

    candidates.sort_by_key(|candidate| candidate.source_key.to_ascii_lowercase());
    let package_name = source
        .file_stem()
        .and_then(|name| name.to_str())
        .map(friendly_stem)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Local MPQ package".to_string());

    Ok(MpqInspection {
        source_path: source.to_path_buf(),
        package_name,
        locale,
        destinations,
        candidates,
    })
}

pub(crate) fn stage_source(wow_dir: &Path, source: &Path) -> MpqResult<StagedMpqSource> {
    if !is_supported_local_source(source) {
        return Err(MpqError::UnsupportedSource);
    }
    let metadata = fs::symlink_metadata(source).map_err(|_| MpqError::UnsupportedSource)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(MpqError::UnsupportedSource);
    }

    let staging_parent = util::cache_dir(Some(wow_dir))
        .map_err(|_| MpqError::Filesystem("creating the MPQ cache"))?
        .join("mpq-staging");
    fs::create_dir_all(&staging_parent)
        .map_err(|_| MpqError::Filesystem("creating the MPQ staging directory"))?;
    let temp_dir = Builder::new()
        .prefix("install-")
        .tempdir_in(&staging_parent)
        .map_err(|_| MpqError::Filesystem("creating an MPQ staging operation"))?;
    let payload = temp_dir.path().join("payload");
    fs::create_dir_all(&payload)
        .map_err(|_| MpqError::Filesystem("creating the MPQ staging payload"))?;

    let source_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(MpqError::UnsupportedSource)?;
    if is_mpq_name(source_name) {
        let target = payload.join(source_name);
        fs::copy(source, &target)
            .map_err(|_| MpqError::Filesystem("copying an MPQ into staging"))?;
    } else {
        install::extract_archive(source, &payload).map_err(|_| MpqError::InvalidArchive)?;
    }

    let mut files = BTreeMap::new();
    collect_staged_mpqs(&payload, &payload, &mut files)?;
    if files.is_empty() {
        return Err(MpqError::NoMpqFiles);
    }
    for (source_key, path) in &files {
        validate_mpq_file(path).map_err(|_| {
            let name = Path::new(source_key)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("MPQ")
                .to_string();
            MpqError::InvalidMpq(name)
        })?;
    }

    Ok(StagedMpqSource {
        _temp_dir: temp_dir,
        files,
    })
}

fn collect_staged_mpqs(
    root: &Path,
    directory: &Path,
    out: &mut BTreeMap<String, PathBuf>,
) -> MpqResult<()> {
    let entries =
        fs::read_dir(directory).map_err(|_| MpqError::Filesystem("reading staged MPQ contents"))?;
    for entry in entries {
        let entry = entry.map_err(|_| MpqError::Filesystem("reading staged MPQ contents"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| MpqError::Filesystem("reading staged MPQ metadata"))?;
        if metadata.file_type().is_symlink() {
            return Err(MpqError::InvalidArchive);
        }
        if metadata.is_dir() {
            collect_staged_mpqs(root, &path, out)?;
        } else if metadata.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .map(is_mpq_name)
                .unwrap_or(false)
        {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| MpqError::InvalidArchive)?;
            let key = normalize_relative_path(relative)?;
            if out.insert(key, path).is_some() {
                return Err(MpqError::InvalidArchive);
            }
        }
    }
    Ok(())
}

fn normalize_relative_path(path: &Path) -> MpqResult<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(MpqError::InvalidArchive)
            }
        }
    }
    if parts.is_empty() {
        Err(MpqError::InvalidArchive)
    } else {
        Ok(parts.join("/"))
    }
}

pub fn validate_mpq_file(path: &Path) -> MpqResult<()> {
    let mut file = fs::File::open(path).map_err(|_| MpqError::InvalidMpq("MPQ".into()))?;
    let file_size = file
        .metadata()
        .map_err(|_| MpqError::InvalidMpq("MPQ".into()))?
        .len();
    if file_size < MPQ_MIN_HEADER_SIZE as u64 {
        return Err(MpqError::InvalidMpq("MPQ".into()));
    }

    let search_end = file_size.min(MPQ_HEADER_SEARCH_LIMIT);
    let mut offset = 0u64;
    let mut buffer = vec![0u8; 1024 * 1024];
    while offset < search_end {
        file.seek(SeekFrom::Start(offset))
            .map_err(|_| MpqError::InvalidMpq("MPQ".into()))?;
        let to_read = (search_end - offset).min(buffer.len() as u64) as usize;
        let read = file
            .read(&mut buffer[..to_read])
            .map_err(|_| MpqError::InvalidMpq("MPQ".into()))?;
        if read < 8 {
            break;
        }
        let mut local = 0usize;
        while local + 8 <= read {
            let absolute = offset + local as u64;
            if absolute.is_multiple_of(MPQ_HEADER_ALIGNMENT) {
                let header = &buffer[local..];
                if header.starts_with(MPQ_HEADER) {
                    let size = u32::from_le_bytes(header[4..8].try_into().unwrap());
                    if size >= MPQ_MIN_HEADER_SIZE {
                        return Ok(());
                    }
                }
                if header.starts_with(MPQ_USER_DATA) && local + 12 <= read {
                    let header_offset =
                        u32::from_le_bytes(header[8..12].try_into().unwrap()) as u64;
                    if validate_header_at(&mut file, absolute.saturating_add(header_offset))? {
                        return Ok(());
                    }
                }
            }
            local += MPQ_HEADER_ALIGNMENT as usize;
        }
        // Preserve 0x200 alignment across chunks.
        offset = ((offset + read as u64) / MPQ_HEADER_ALIGNMENT) * MPQ_HEADER_ALIGNMENT;
        if offset == 0 {
            break;
        }
    }
    Err(MpqError::InvalidMpq("MPQ".into()))
}

fn validate_header_at(file: &mut fs::File, offset: u64) -> MpqResult<bool> {
    let mut header = [0u8; 8];
    file.seek(SeekFrom::Start(offset))
        .map_err(|_| MpqError::InvalidMpq("MPQ".into()))?;
    if file.read_exact(&mut header).is_err() {
        return Ok(false);
    }
    Ok(header.starts_with(MPQ_HEADER)
        && u32::from_le_bytes(header[4..8].try_into().unwrap()) >= MPQ_MIN_HEADER_SIZE)
}

pub fn validate_selection(selection: &MpqInstallSelection) -> MpqResult<()> {
    if selection.display_name.trim().is_empty() {
        return Err(MpqError::InvalidSelection(
            "Each MPQ needs a friendly name".to_string(),
        ));
    }
    validate_target_file_name_syntax(&selection.file_name)
}

pub fn validate_target_file_name(file_name: &str) -> MpqResult<()> {
    validate_target_file_name_syntax(file_name)?;
    if is_reserved_core_filename(file_name) {
        return Err(MpqError::InvalidSelection(format!(
            "{file_name} is reserved for core game data"
        )));
    }
    Ok(())
}

fn validate_target_file_name_syntax(file_name: &str) -> MpqResult<()> {
    let trimmed = file_name.trim();
    if trimmed.is_empty()
        || trimmed != file_name
        || Path::new(trimmed)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(trimmed)
        || trimmed.contains(['/', '\\'])
        || trimmed.contains(['<', '>', ':', '"', '|', '?', '*'])
        || trimmed.ends_with(['.', ' '])
        || trimmed.chars().any(char::is_control)
        || !is_mpq_name(trimmed)
    {
        return Err(MpqError::InvalidSelection(
            "MPQ filenames must be safe base filenames ending in .MPQ".to_string(),
        ));
    }
    Ok(())
}

pub fn is_reserved_core_filename(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    if !lower.ends_with(".mpq") {
        return false;
    }
    let stem = &lower[..lower.len() - 4];
    if matches!(
        stem,
        "art"
            | "base-osx"
            | "base-win"
            | "common"
            | "common-2"
            | "base"
            | "dbc"
            | "expansion"
            | "interface"
            | "itemtexture"
            | "lichking"
            | "misc"
            | "model"
            | "oldworld"
            | "sound"
            | "terrain"
            | "texture"
            | "fonts"
            | "wmo"
            | "world"
            | "world2"
    ) {
        return true;
    }
    if stem.starts_with("locale-")
        || stem.starts_with("speech-")
        || stem.starts_with("expansion-locale-")
        || stem.starts_with("expansion-speech-")
        || stem.starts_with("lichking-locale-")
        || stem.starts_with("lichking-speech-")
    {
        return true;
    }
    for locale in KNOWN_LOCALES {
        let locale = locale.to_ascii_lowercase();
        if stem == format!("base-{locale}") || stem == format!("backup-{locale}") {
            return true;
        }
    }
    if stem == "patch" {
        return true;
    }
    for prefix in ["common-", "base-", "expansion-", "lichking-"] {
        if let Some(number) = stem.strip_prefix(prefix) {
            if !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit()) {
                return true;
            }
        }
    }
    let Some(rest) = stem.strip_prefix("patch-") else {
        return false;
    };
    if rest.chars().all(|ch| ch.is_ascii_digit()) {
        return true;
    }
    for locale in KNOWN_LOCALES {
        if rest.eq_ignore_ascii_case(locale) {
            return true;
        }
        if let Some(number) = rest
            .strip_prefix(&format!("{}-", locale.to_ascii_lowercase()))
            .or_else(|| rest.strip_prefix(&format!("{}-", locale)))
        {
            if !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

pub(crate) fn target_path(
    wow_dir: &Path,
    destination: &MpqDestination,
    file_name: &str,
) -> MpqResult<PathBuf> {
    validate_target_file_name_syntax(file_name)?;
    let data = wow_dir.join("Data");
    match destination {
        MpqDestination::DataRoot => Ok(data.join(file_name)),
        MpqDestination::Locale(locale) => {
            let locale = normalize_locale(locale).ok_or_else(|| {
                MpqError::InvalidSelection("Choose a recognized WoW locale".to_string())
            })?;
            let directory =
                find_case_insensitive_child(&data, &locale).unwrap_or_else(|| data.join(&locale));
            Ok(directory.join(file_name))
        }
    }
}

pub(crate) fn copy_atomic(source: &Path, destination: &Path) -> MpqResult<()> {
    let parent = destination
        .parent()
        .ok_or(MpqError::Filesystem("resolving the MPQ destination"))?;
    fs::create_dir_all(parent).map_err(|_| MpqError::Filesystem("creating the MPQ destination"))?;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("patch.MPQ");
    let temporary = parent.join(format!(".{file_name}.wuddle-new-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_file(&temporary)
            .map_err(|_| MpqError::Filesystem("cleaning a temporary MPQ"))?;
    }
    fs::copy(source, &temporary)
        .map_err(|_| MpqError::Filesystem("copying an MPQ into the game"))?;
    fs::rename(&temporary, destination)
        .map_err(|_| MpqError::Filesystem("finalizing an MPQ installation"))?;
    Ok(())
}

pub(crate) fn backup_root(wow_dir: &Path, repo_id: i64) -> PathBuf {
    wow_dir
        .join(".wuddle")
        .join("backups")
        .join("mpq")
        .join(repo_id.to_string())
}

pub(crate) fn set_friendly_comment(path: &Path, display_name: &str, enabled: bool) {
    let comment = format!("{} - managed by Wuddle", display_name.trim());
    install::maybe_set_comment(path, &comment, enabled);
}

pub(crate) fn scan_existing_mpqs(wow_dir: &Path) -> MpqResult<Vec<MpqProtectionEntry>> {
    let data = wow_dir.join("Data");
    let mut paths = Vec::new();
    if !data.is_dir() {
        return Ok(Vec::new());
    }
    for entry in
        fs::read_dir(&data).map_err(|_| MpqError::Filesystem("scanning the Data directory"))?
    {
        let entry = entry.map_err(|_| MpqError::Filesystem("scanning the Data directory"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| MpqError::Filesystem("reading MPQ metadata"))?;
        if metadata.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .map(is_stored_mpq_name)
                .unwrap_or(false)
        {
            paths.push((path, metadata));
        } else if metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && normalize_locale(&entry.file_name().to_string_lossy()).is_some()
        {
            for child in fs::read_dir(&path)
                .map_err(|_| MpqError::Filesystem("scanning a locale directory"))?
            {
                let child =
                    child.map_err(|_| MpqError::Filesystem("scanning a locale directory"))?;
                let child_path = child.path();
                let child_meta = fs::symlink_metadata(&child_path)
                    .map_err(|_| MpqError::Filesystem("reading MPQ metadata"))?;
                if child_meta.is_file()
                    && child_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(is_stored_mpq_name)
                        .unwrap_or(false)
                {
                    paths.push((child_path, child_meta));
                }
            }
        }
    }

    let mut out = Vec::new();
    for (path, metadata) in paths {
        let relative = path
            .strip_prefix(wow_dir)
            .map_err(|_| MpqError::Filesystem("resolving an MPQ path"))?;
        let relative = normalize_relative_path(relative)?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("MPQ")
            .to_string();
        let enabled = !is_disabled_manifest_path(&file_name);
        let classified_name = enabled_manifest_path(&file_name);
        out.push(MpqProtectionEntry {
            path: relative,
            file_name,
            display_name: None,
            fingerprint: metadata_fingerprint(&metadata),
            protected: true,
            core: is_reserved_core_filename(&classified_name),
            editor_unlocked: false,
            enabled,
        });
    }
    out.sort_by_key(|entry| entry.path.to_ascii_lowercase());
    Ok(out)
}

/// Build a cheap identity from filesystem metadata only. This intentionally
/// does not inspect MPQ contents: changing any ordinary metadata component
/// causes an explicitly unprotected file to become protected again.
fn metadata_fingerprint(metadata: &fs::Metadata) -> String {
    fn time_key(value: std::io::Result<std::time::SystemTime>) -> u128 {
        value
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    }

    let modified = time_key(metadata.modified());
    let created = time_key(metadata.created());

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        format!(
            "v1:{}:{modified}:{created}:{}:{}",
            metadata.len(),
            metadata.dev(),
            metadata.ino()
        )
    }

    #[cfg(not(unix))]
    {
        format!("v1:{}:{modified}:{created}", metadata.len())
    }
}

pub(crate) fn find_case_insensitive_child(parent: &Path, name: &str) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
            {
                return Some(entry.path());
            }
        }
    }
    None
}

fn is_mpq_name(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".mpq")
}

fn is_stored_mpq_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".mpq") || lower.ends_with(".mpq.disabled")
}

const DISABLED_SUFFIX: &str = ".disabled";

fn is_disabled_manifest_path(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(DISABLED_SUFFIX)
}

fn enabled_manifest_path(path: &str) -> String {
    if is_disabled_manifest_path(path) {
        path[..path.len() - DISABLED_SUFFIX.len()].to_string()
    } else {
        path.to_string()
    }
}

fn disabled_manifest_path(path: &str) -> String {
    if is_disabled_manifest_path(path) {
        path.to_string()
    } else {
        format!("{path}{DISABLED_SUFFIX}")
    }
}

fn friendly_stem(name: &str) -> String {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(name);
    stem.replace(['_', '-'], " ")
        .split_whitespace()
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn locale_from_file_name(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    KNOWN_LOCALES.iter().find_map(|locale| {
        if lower.contains(&locale.to_ascii_lowercase()) {
            Some((*locale).to_string())
        } else {
            None
        }
    })
}

fn suggest_destination(file_name: &str) -> MpqDestination {
    // Locale archives normally identify their target directly in the MPQ
    // filename (for example patch-enUS-M.MPQ). Everything else belongs in
    // Data/ by default. Do not infer locale placement merely because the
    // client has one detected locale or an input archive happened to contain
    // a locale-shaped directory.
    if let Some(found) = locale_from_file_name(file_name) {
        return MpqDestination::Locale(found);
    }
    MpqDestination::DataRoot
}

impl crate::Engine {
    // ---------------------------------------------------------------------
    // MPQ patch management
    // ---------------------------------------------------------------------

    pub fn inspect_local_mpq_source(&self, wow_dir: &Path, source: &Path) -> Result<MpqInspection> {
        let _diagnostic = diagnostics::OperationGuard::new("inspect_local_mpq_source");
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Trace,
            "engine.mpq",
            "inspecting local MPQ source; path omitted",
        );
        inspect_local_source(wow_dir, source).map_err(Into::into)
    }

    pub fn detect_wow_locale(&self, wow_dir: &Path) -> LocaleDetection {
        detect_wow_locale(wow_dir)
    }

    pub fn list_mpq_protection(&self, wow_dir: &Path) -> Result<Vec<MpqProtectionEntry>> {
        let _diagnostic = diagnostics::OperationGuard::new("list_mpq_protection");
        let managed = self
            .db()
            .list_all_installs_full()?
            .into_iter()
            .filter(|(_, install)| install.kind == "mpq")
            .map(|(_, install)| install.path.to_ascii_lowercase())
            .collect::<HashSet<_>>();

        let discovered = scan_existing_mpqs(wow_dir)?;
        let mut out = Vec::new();
        for mut entry in discovered {
            if managed.contains(&entry.path.to_ascii_lowercase()) {
                continue;
            }
            let state =
                self.db()
                    .upsert_mpq_protection(&entry.path, &entry.fingerprint, entry.core)?;
            entry.protected = state.protected;
            entry.core = state.core;
            entry.editor_unlocked = state.editor_unlocked;
            entry.display_name = state.display_name;
            out.push(entry);
        }
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "scanned MPQ protection state: untracked_count={}",
                out.len()
            ),
        );
        Ok(out)
    }

    pub fn set_mpq_protected(
        &self,
        wow_dir: &Path,
        manifest_path: &str,
        protected: bool,
    ) -> Result<()> {
        let _diagnostic = diagnostics::OperationGuard::new("set_mpq_protected");
        let entries = self.list_mpq_protection(wow_dir)?;
        let entry = entries
            .into_iter()
            .find(|entry| entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("The selected MPQ is no longer present"))?;
        if entry.core && !protected {
            anyhow::bail!("Core game MPQs cannot be unprotected");
        }
        self.db()
            .set_mpq_protection(&entry.path, &entry.fingerprint, protected)?;
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!("changed MPQ protection: protected={protected}; path omitted"),
        );
        Ok(())
    }

    pub fn set_mpq_core_classification(
        &self,
        wow_dir: &Path,
        manifest_path: &str,
        core: bool,
    ) -> Result<()> {
        let _diagnostic = diagnostics::OperationGuard::new("set_mpq_core_classification");
        let entries = self.list_mpq_protection(wow_dir)?;
        let entry = entries
            .into_iter()
            .find(|entry| entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("The selected MPQ is no longer present"))?;
        self.db()
            .set_mpq_core_classification(&entry.path, &entry.fingerprint, core)?;
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!("changed MPQ classification: core={core}; path omitted"),
        );
        Ok(())
    }

    /// Explicitly unlock a detected core/custom MPQ for editing without
    /// changing how the archive is classified.
    pub fn unlock_untracked_mpq_for_editing(
        &self,
        wow_dir: &Path,
        manifest_path: &str,
    ) -> Result<()> {
        let entry = self
            .list_mpq_protection(wow_dir)?
            .into_iter()
            .find(|entry| entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("The selected MPQ is no longer present"))?;
        self.db()
            .set_mpq_editor_unlocked(&entry.path, &entry.fingerprint, true)?;
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            "unlocked an untracked MPQ for editing; path omitted",
        );
        Ok(())
    }

    pub fn set_untracked_mpq_editor_unlocked(
        &self,
        wow_dir: &Path,
        manifest_path: &str,
        editor_unlocked: bool,
    ) -> Result<()> {
        let _diagnostic = diagnostics::OperationGuard::new("set_untracked_mpq_editor_unlocked");
        let entry = self
            .list_mpq_protection(wow_dir)?
            .into_iter()
            .find(|entry| entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("The selected MPQ is no longer present"))?;
        self.db()
            .set_mpq_editor_unlocked(&entry.path, &entry.fingerprint, editor_unlocked)?;
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "untracked MPQ editor lock committed: editor_unlocked={editor_unlocked}; path omitted"
            ),
        );
        Ok(())
    }

    pub fn set_tracked_mpq_protected(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        manifest_path: &str,
        protected: bool,
    ) -> Result<()> {
        let entry = self
            .db()
            .list_installs(repo_id)?
            .into_iter()
            .find(|entry| entry.kind == "mpq" && entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("Tracked MPQ not found"))?;
        let target = Self::resolve_install_path(&entry.path, Some(wow_dir))
            .ok_or_else(|| anyhow::anyhow!("Could not resolve the tracked MPQ path"))?;
        let actual = Self::find_actual_case(&target).unwrap_or(target);
        let metadata = fs::symlink_metadata(&actual)
            .map_err(|_| MpqError::Filesystem("reading tracked MPQ metadata"))?;
        if !metadata.is_file() {
            anyhow::bail!("The selected MPQ is no longer present");
        }
        let fingerprint = metadata_fingerprint(&metadata);
        self.db()
            .upsert_mpq_protection(&entry.path, &fingerprint, false)?;
        self.db()
            .set_mpq_protection(&entry.path, &fingerprint, protected)?;
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!("changed tracked MPQ protection: protected={protected}; path omitted"),
        );
        Ok(())
    }

    pub fn set_tracked_mpq_editor_unlocked(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        manifest_path: &str,
        editor_unlocked: bool,
    ) -> Result<()> {
        let _diagnostic = diagnostics::OperationGuard::new("set_tracked_mpq_editor_unlocked");
        let entry = self
            .db()
            .list_installs(repo_id)?
            .into_iter()
            .find(|entry| entry.kind == "mpq" && entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("Tracked MPQ not found"))?;
        let target = Self::resolve_install_path(&entry.path, Some(wow_dir))
            .ok_or_else(|| anyhow::anyhow!("Could not resolve the tracked MPQ path"))?;
        let actual = Self::find_actual_case(&target).unwrap_or(target);
        let metadata = fs::symlink_metadata(&actual)
            .map_err(|_| MpqError::Filesystem("reading tracked MPQ metadata"))?;
        let fingerprint = metadata_fingerprint(&metadata);
        self.db()
            .upsert_mpq_protection(&entry.path, &fingerprint, false)?;
        self.db()
            .set_mpq_editor_unlocked(&entry.path, &fingerprint, editor_unlocked)?;
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "tracked MPQ editor lock committed: repo_id={repo_id}; editor_unlocked={editor_unlocked}; path omitted"
            ),
        );
        Ok(())
    }

    fn tracked_mpq_editor_unlocked(&self, wow_dir: &Path, manifest_path: &str) -> Result<bool> {
        let Some(target) = Self::resolve_install_path(manifest_path, Some(wow_dir)) else {
            return Ok(false);
        };
        let actual = Self::find_actual_case(&target).unwrap_or(target);
        let Ok(metadata) = fs::symlink_metadata(actual) else {
            return Ok(false);
        };
        let fingerprint = metadata_fingerprint(&metadata);
        Ok(self
            .db()
            .get_mpq_protection(manifest_path)?
            .filter(|row| row.fingerprint == fingerprint)
            .map(|row| row.editor_unlocked)
            .unwrap_or(false))
    }

    pub fn rename_untracked_mpq_display_name(
        &self,
        wow_dir: &Path,
        manifest_path: &str,
        display_name: &str,
        set_xattr_comment: bool,
    ) -> Result<()> {
        let display_name = display_name.trim();
        if display_name.is_empty()
            || display_name.chars().count() > 120
            || display_name.chars().any(char::is_control)
        {
            anyhow::bail!("MPQ friendly name must be 1–120 printable characters");
        }
        let entry = self
            .list_mpq_protection(wow_dir)?
            .into_iter()
            .find(|entry| entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("The selected MPQ is no longer present"))?;
        self.db()
            .set_mpq_protection_display_name(&entry.path, &entry.fingerprint, display_name)?;
        if let Some(path) = Self::resolve_install_path(&entry.path, Some(wow_dir)) {
            set_friendly_comment(&path, display_name, set_xattr_comment);
        }
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            "changed untracked MPQ friendly name; values omitted",
        );
        Ok(())
    }

    /// Enable or disable an MPQ that exists in the game directory but is not
    /// tracked as part of a Wuddle package. The editor padlock is an engine
    /// boundary, not merely a frontend affordance.
    pub fn set_untracked_mpq_enabled(
        &self,
        wow_dir: &Path,
        manifest_path: &str,
        enabled: bool,
    ) -> Result<()> {
        let _diagnostic = diagnostics::OperationGuard::new("set_untracked_mpq_enabled");
        let entry = self
            .list_mpq_protection(wow_dir)?
            .into_iter()
            .find(|entry| entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("The selected MPQ is no longer present"))?;
        if !entry.editor_unlocked {
            anyhow::bail!("Unlock this MPQ before changing its enabled state");
        }
        if entry.enabled == enabled {
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Debug,
                "engine.mpq",
                format!(
                    "untracked MPQ state already matched request: enabled={enabled}; no filesystem change"
                ),
            );
            return Ok(());
        }

        let new_manifest = if enabled {
            enabled_manifest_path(&entry.path)
        } else {
            disabled_manifest_path(&entry.path)
        };
        let current = Self::resolve_install_path(&entry.path, Some(wow_dir))
            .and_then(|path| Self::find_actual_case(&path).or(Some(path)))
            .ok_or_else(|| anyhow::anyhow!("Could not resolve the selected MPQ"))?;
        if !current.is_file() {
            anyhow::bail!("The selected MPQ is no longer present");
        }
        let desired = Self::resolve_install_path(&new_manifest, Some(wow_dir))
            .ok_or_else(|| anyhow::anyhow!("Could not resolve the MPQ toggle destination"))?;
        if Self::find_actual_case(&desired)
            .map(|path| path.exists())
            .unwrap_or(false)
        {
            anyhow::bail!(MpqError::ToggleCollision(
                desired
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("MPQ")
                    .to_string()
            ));
        }

        diagnostics::emit(
            diagnostics::DiagnosticLevel::Trace,
            "engine.mpq",
            format!("untracked MPQ filesystem rename started: enabled={enabled}; paths omitted"),
        );
        fs::rename(&current, &desired)
            .map_err(|_| MpqError::Filesystem("changing an MPQ enabled state"))?;
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Trace,
            "engine.mpq",
            "untracked MPQ filesystem rename completed; paths omitted",
        );
        let fingerprint = match fs::symlink_metadata(&desired) {
            Ok(metadata) => metadata_fingerprint(&metadata),
            Err(_) => {
                diagnostics::emit(
                    diagnostics::DiagnosticLevel::Debug,
                    "engine.mpq",
                    "untracked MPQ metadata read failed; rolling back filesystem rename",
                );
                let _ = fs::rename(&desired, &current);
                return Err(MpqError::Filesystem("reading MPQ metadata").into());
            }
        };
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Trace,
            "engine.mpq",
            "untracked MPQ metadata commit started",
        );
        if let Err(error) = self.db().move_mpq_protection(
            &entry.path,
            &new_manifest,
            &fingerprint,
            entry.protected,
            entry.core,
            entry.editor_unlocked,
            entry.display_name.as_deref(),
        ) {
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Debug,
                "engine.mpq",
                "untracked MPQ metadata commit failed; rolling back filesystem rename",
            );
            let _ = fs::rename(&desired, &current);
            return Err(error);
        }
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "untracked MPQ state committed: enabled={enabled}; filesystem_renamed=true; metadata_updated=true; path omitted"
            ),
        );
        Ok(())
    }

    /// Rename an unlocked, untracked custom MPQ without changing its Data/
    /// destination or enabled state. Managed and protected archives use their
    /// own package workflows and cannot pass through this path.
    pub fn rename_untracked_mpq_file(
        &self,
        wow_dir: &Path,
        manifest_path: &str,
        new_file_name: &str,
    ) -> Result<String> {
        let _diagnostic = diagnostics::OperationGuard::new("rename_untracked_mpq_file");
        let new_file_name = new_file_name.trim();
        validate_target_file_name(new_file_name)?;
        let entry = self
            .list_mpq_protection(wow_dir)?
            .into_iter()
            .find(|entry| entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("The selected MPQ is no longer present"))?;
        if entry.core {
            anyhow::bail!("Core game MPQs cannot be renamed");
        }
        if entry.protected {
            anyhow::bail!("Unlock this MPQ before renaming it on disk");
        }

        let stored_file_name = if entry.enabled {
            new_file_name.to_string()
        } else {
            format!("{new_file_name}{DISABLED_SUFFIX}")
        };
        let relative_parent = Path::new(&entry.path)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        let new_manifest = normalize_relative_path(&relative_parent.join(&stored_file_name))?;
        if entry.path == new_manifest {
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Debug,
                "engine.mpq",
                "untracked MPQ rename skipped because the requested filename already matches",
            );
            return Ok(entry.path);
        }

        let current = Self::resolve_install_path(&entry.path, Some(wow_dir))
            .and_then(|path| Self::find_actual_case(&path).or(Some(path)))
            .ok_or_else(|| anyhow::anyhow!("Could not resolve the selected MPQ"))?;
        if !current.is_file() {
            anyhow::bail!("The selected MPQ is no longer present");
        }
        let desired = current
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Could not resolve the MPQ destination"))?
            .join(&stored_file_name);
        if let Some(existing) = desired
            .parent()
            .and_then(|parent| find_case_insensitive_child(parent, &stored_file_name))
        {
            if existing != current {
                anyhow::bail!(MpqError::ToggleCollision(stored_file_name));
            }
        }

        diagnostics::emit(
            diagnostics::DiagnosticLevel::Trace,
            "engine.mpq",
            "untracked MPQ on-disk rename started; filenames and paths omitted",
        );
        fs::rename(&current, &desired)
            .map_err(|_| MpqError::Filesystem("renaming a custom MPQ"))?;
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Trace,
            "engine.mpq",
            "untracked MPQ on-disk rename completed; filenames and paths omitted",
        );
        let fingerprint = match fs::symlink_metadata(&desired) {
            Ok(metadata) => metadata_fingerprint(&metadata),
            Err(_) => {
                diagnostics::emit(
                    diagnostics::DiagnosticLevel::Debug,
                    "engine.mpq",
                    "renamed MPQ metadata read failed; rolling back on-disk rename",
                );
                let _ = fs::rename(&desired, &current);
                return Err(MpqError::Filesystem("reading renamed MPQ metadata").into());
            }
        };
        if let Err(error) = self.db().move_mpq_protection(
            &entry.path,
            &new_manifest,
            &fingerprint,
            entry.protected,
            entry.core,
            entry.editor_unlocked,
            entry.display_name.as_deref(),
        ) {
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Debug,
                "engine.mpq",
                "renamed MPQ metadata commit failed; rolling back on-disk rename",
            );
            let _ = fs::rename(&desired, &current);
            return Err(error);
        }
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            "untracked MPQ rename committed: filesystem_renamed=true; metadata_updated=true; paths omitted",
        );
        Ok(new_manifest)
    }

    /// Apply the editable properties of an unlocked, untracked MPQ as one
    /// filesystem/database operation. Classification and protection remain
    /// independent: saving never locks or unlocks the archive.
    // Editing is one filesystem/database transaction, so keep all requested
    // identity and placement fields together at this boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn edit_untracked_mpq(
        &self,
        wow_dir: &Path,
        manifest_path: &str,
        display_name: &str,
        new_file_name: &str,
        destination: &MpqDestination,
        core: bool,
        set_xattr_comment: bool,
    ) -> Result<String> {
        let _diagnostic = diagnostics::OperationGuard::new("edit_untracked_mpq");
        let display_name = display_name.trim();
        if display_name.is_empty()
            || display_name.chars().count() > 120
            || display_name.chars().any(char::is_control)
        {
            anyhow::bail!("MPQ friendly name must be 1–120 printable characters");
        }
        let new_file_name = new_file_name.trim();
        if core {
            validate_target_file_name_syntax(new_file_name)?;
        } else {
            validate_target_file_name(new_file_name)?;
        }

        let entry = self
            .list_mpq_protection(wow_dir)?
            .into_iter()
            .find(|entry| entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("The selected MPQ is no longer present"))?;
        if !entry.editor_unlocked {
            anyhow::bail!("Unlock this MPQ before editing it");
        }

        let stored_file_name = if entry.enabled {
            new_file_name.to_string()
        } else {
            format!("{new_file_name}{DISABLED_SUFFIX}")
        };
        let desired = target_path(wow_dir, destination, &stored_file_name)?;
        let new_manifest = normalize_relative_path(
            desired
                .strip_prefix(wow_dir)
                .map_err(|_| MpqError::Filesystem("resolving the MPQ destination"))?,
        )?;
        if self.db().find_mpq_install_owner(&new_manifest)?.is_some() {
            anyhow::bail!(MpqError::ManagedTarget(stored_file_name));
        }
        let current = Self::resolve_install_path(&entry.path, Some(wow_dir))
            .and_then(|path| Self::find_actual_case(&path).or(Some(path)))
            .ok_or_else(|| anyhow::anyhow!("Could not resolve the selected MPQ"))?;
        if !current.is_file() {
            anyhow::bail!("The selected MPQ is no longer present");
        }
        let path_changed = !entry.path.eq_ignore_ascii_case(&new_manifest)
            || current.file_name() != desired.file_name();
        if path_changed {
            if let Some(existing) = desired
                .parent()
                .and_then(|parent| find_case_insensitive_child(parent, &stored_file_name))
            {
                if existing != current {
                    anyhow::bail!(MpqError::ToggleCollision(stored_file_name));
                }
            }
            if let Some(parent) = desired.parent() {
                fs::create_dir_all(parent)?;
            }
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Trace,
                "engine.mpq",
                format!(
                    "untracked MPQ edit filesystem move started: destination={}; filenames and paths omitted",
                    destination.label()
                ),
            );
            fs::rename(&current, &desired)
                .map_err(|_| MpqError::Filesystem("renaming a custom MPQ"))?;
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Trace,
                "engine.mpq",
                "untracked MPQ edit filesystem move completed; filenames and paths omitted",
            );
        }

        let committed_path = if path_changed { &desired } else { &current };
        let fingerprint = match fs::symlink_metadata(committed_path) {
            Ok(metadata) => metadata_fingerprint(&metadata),
            Err(_) => {
                if path_changed {
                    diagnostics::emit(
                        diagnostics::DiagnosticLevel::Debug,
                        "engine.mpq",
                        "edited MPQ metadata read failed; rolling back filesystem move",
                    );
                    let _ = fs::rename(&desired, &current);
                }
                return Err(MpqError::Filesystem("reading edited MPQ metadata").into());
            }
        };
        if let Err(error) = self.db().edit_mpq_protection_entry(
            &entry.path,
            &new_manifest,
            &fingerprint,
            display_name,
            entry.protected,
            core,
            entry.editor_unlocked,
        ) {
            if path_changed {
                diagnostics::emit(
                    diagnostics::DiagnosticLevel::Debug,
                    "engine.mpq",
                    "untracked MPQ metadata commit failed; rolling back filesystem move",
                );
                let _ = fs::rename(&desired, &current);
            }
            return Err(error);
        }
        set_friendly_comment(committed_path, display_name, set_xattr_comment);
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "untracked MPQ edit committed: filesystem_moved={path_changed}; metadata_updated=true; core={core}; xattr_requested={set_xattr_comment}; values omitted"
            ),
        );
        Ok(new_manifest)
    }

    /// Rename or move an unlocked Wuddle-installed MPQ while retaining its
    /// package ownership, backup association, display metadata, and update
    /// tracking.
    // Editing is one filesystem/database transaction, so keep all requested
    // identity and placement fields together at this boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn edit_tracked_mpq(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        manifest_path: &str,
        display_name: &str,
        new_file_name: &str,
        destination: &MpqDestination,
        set_xattr_comment: bool,
    ) -> Result<String> {
        let _diagnostic = diagnostics::OperationGuard::new("edit_tracked_mpq");
        let display_name = display_name.trim();
        if display_name.is_empty()
            || display_name.chars().count() > 120
            || display_name.chars().any(char::is_control)
        {
            anyhow::bail!("MPQ friendly name must be 1–120 printable characters");
        }
        let new_file_name = new_file_name.trim();
        validate_target_file_name(new_file_name)?;
        let entry = self
            .db()
            .list_installs(repo_id)?
            .into_iter()
            .find(|entry| entry.kind == "mpq" && entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("Tracked MPQ not found"))?;
        if !self.tracked_mpq_editor_unlocked(wow_dir, &entry.path)? {
            anyhow::bail!("Unlock this MPQ before editing it");
        }

        let stored_file_name = if is_disabled_manifest_path(&entry.path) {
            format!("{new_file_name}{DISABLED_SUFFIX}")
        } else {
            new_file_name.to_string()
        };
        let desired_base = target_path(wow_dir, destination, new_file_name)?;
        let desired = desired_base.with_file_name(&stored_file_name);
        let new_manifest = normalize_relative_path(
            desired
                .strip_prefix(wow_dir)
                .map_err(|_| MpqError::Filesystem("resolving the MPQ destination"))?,
        )?;
        if self
            .db()
            .list_installs(repo_id)?
            .into_iter()
            .any(|install| {
                install.kind == "mpq"
                    && !install.path.eq_ignore_ascii_case(&entry.path)
                    && install.path.eq_ignore_ascii_case(&new_manifest)
            })
        {
            anyhow::bail!(MpqError::ManagedTarget(stored_file_name));
        }
        if self
            .db()
            .find_mpq_install_owner(&new_manifest)?
            .is_some_and(|owner| owner != repo_id)
        {
            anyhow::bail!(MpqError::ManagedTarget(stored_file_name));
        }
        let current = Self::resolve_install_path(&entry.path, Some(wow_dir))
            .and_then(|path| Self::find_actual_case(&path).or(Some(path)))
            .ok_or_else(|| anyhow::anyhow!("Could not resolve the tracked MPQ path"))?;
        if !current.is_file() {
            anyhow::bail!("The selected MPQ is no longer present");
        }
        let path_changed = !entry.path.eq_ignore_ascii_case(&new_manifest)
            || current.file_name() != desired.file_name();
        let displaced_backup = if path_changed {
            self.db()
                .get_mpq_backup(repo_id, &entry.path)?
                .and_then(|backup| Self::resolve_install_path(&backup.backup_path, Some(wow_dir)))
                .filter(|path| path.is_file())
        } else {
            None
        };
        if path_changed {
            if let Some(existing) = desired
                .parent()
                .and_then(|parent| find_case_insensitive_child(parent, &stored_file_name))
            {
                if existing != current {
                    anyhow::bail!(MpqError::ToggleCollision(stored_file_name));
                }
            }
            if let Some(parent) = desired.parent() {
                fs::create_dir_all(parent)?;
            }
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Trace,
                "engine.mpq",
                format!(
                    "tracked MPQ edit filesystem move started: repo_id={repo_id}; destination={}; filenames and paths omitted",
                    destination.label()
                ),
            );
            fs::rename(&current, &desired)
                .map_err(|_| MpqError::Filesystem("moving a tracked MPQ"))?;
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Trace,
                "engine.mpq",
                format!(
                    "tracked MPQ edit filesystem move completed: repo_id={repo_id}; filenames and paths omitted"
                ),
            );
        }

        let restored_backup = if let Some(backup_path) = displaced_backup.as_ref() {
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Trace,
                "engine.mpq",
                format!("tracked MPQ displaced backup restoration started: repo_id={repo_id}"),
            );
            if let Err(error) = fs::rename(backup_path, &current) {
                let _ = fs::rename(&desired, &current);
                return Err(error.into());
            }
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Trace,
                "engine.mpq",
                format!("tracked MPQ displaced backup restoration completed: repo_id={repo_id}"),
            );
            true
        } else {
            false
        };

        if let Err(error) = self.db().edit_tracked_mpq_install(
            repo_id,
            &entry.path,
            &new_manifest,
            display_name,
            path_changed,
        ) {
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Debug,
                "engine.mpq",
                format!(
                    "tracked MPQ metadata commit failed; rolling back filesystem changes: repo_id={repo_id}"
                ),
            );
            if restored_backup {
                if let Some(backup_path) = displaced_backup.as_ref() {
                    let _ = fs::rename(&current, backup_path);
                }
            }
            if path_changed {
                let _ = fs::rename(&desired, &current);
            }
            return Err(error);
        }
        set_friendly_comment(
            if path_changed { &desired } else { &current },
            display_name,
            set_xattr_comment,
        );
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "tracked MPQ edit committed: repo_id={repo_id}; filesystem_moved={path_changed}; backup_restored={restored_backup}; metadata_updated=true; xattr_requested={set_xattr_comment}; values omitted"
            ),
        );
        Ok(new_manifest)
    }

    fn validate_package_display_name(display_name: &str) -> Result<&str> {
        let display_name = display_name.trim();
        if display_name.is_empty()
            || display_name.chars().count() > 120
            || display_name.chars().any(char::is_control)
        {
            anyhow::bail!("MPQ package name must be 1–120 printable characters");
        }
        Ok(display_name)
    }

    fn local_mpq_display_name_from_identity(name: &str) -> String {
        let Some((base, suffix)) = name.rsplit_once('-') else {
            return name.to_string();
        };
        if suffix.len() == 8 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            base.to_string()
        } else {
            name.to_string()
        }
    }

    pub fn mpq_package_display_name(&self, repo_id: i64) -> Result<String> {
        if let Some(display_name) = self.db().mpq_package_display_name(repo_id)? {
            return Ok(display_name);
        }
        let repo = self.db().get_repo(repo_id)?;
        if repo.mode == InstallMode::Mpq
            && repo.host.eq_ignore_ascii_case("local-mpq")
            && repo.owner.eq_ignore_ascii_case("local")
        {
            Ok(Self::local_mpq_display_name_from_identity(&repo.name))
        } else {
            Ok(repo.name)
        }
    }

    /// Edit an MPQ package label and its tracked files as one staged
    /// filesystem/database transaction.
    pub fn edit_tracked_mpq_package(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        display_name: &str,
        edits: &[MpqPackageFileEdit],
        set_xattr_comment: bool,
    ) -> Result<()> {
        let _diagnostic = diagnostics::OperationGuard::new("edit_tracked_mpq_package");
        let display_name = Self::validate_package_display_name(display_name)?;
        if edits.is_empty() {
            anyhow::bail!("The MPQ package has no editable files");
        }

        let installs = self
            .db()
            .list_installs(repo_id)?
            .into_iter()
            .filter(|entry| entry.kind == "mpq")
            .collect::<Vec<_>>();
        if installs.len() != edits.len() {
            anyhow::bail!("The MPQ package changed while it was being edited; reopen it and retry");
        }

        struct Prepared {
            old_path: String,
            new_path: String,
            display_name: String,
            current: PathBuf,
            desired: PathBuf,
            path_changed: bool,
            backup: Option<PathBuf>,
        }

        let mut prepared = Vec::with_capacity(edits.len());
        let mut old_paths = HashSet::new();
        let mut new_paths = HashSet::new();
        let mut friendly_names = HashSet::new();

        for edit in edits {
            if !old_paths.insert(edit.path.to_ascii_lowercase()) {
                anyhow::bail!("The MPQ package contains a duplicate component");
            }
            let component_name = edit.display_name.trim();
            if component_name.is_empty()
                || component_name.chars().count() > 120
                || component_name.chars().any(char::is_control)
            {
                anyhow::bail!("MPQ friendly names must be 1–120 printable characters");
            }
            if !friendly_names.insert(component_name.to_ascii_lowercase()) {
                anyhow::bail!("Friendly MPQ names must be unique within a package");
            }
            let file_name = edit.file_name.trim();
            validate_target_file_name(file_name)?;
            let entry = installs
                .iter()
                .find(|entry| entry.path.eq_ignore_ascii_case(&edit.path))
                .ok_or_else(|| anyhow::anyhow!("An MPQ package component is no longer tracked"))?;
            let stored_file_name = if edit.enabled {
                file_name.to_string()
            } else {
                format!("{file_name}{DISABLED_SUFFIX}")
            };
            let desired_base = target_path(wow_dir, &edit.destination, file_name)?;
            let desired = desired_base.with_file_name(&stored_file_name);
            let new_path = normalize_relative_path(
                desired
                    .strip_prefix(wow_dir)
                    .map_err(|_| MpqError::Filesystem("resolving the MPQ destination"))?,
            )?;
            if !new_paths.insert(new_path.to_ascii_lowercase()) {
                anyhow::bail!("Two MPQs cannot use the same destination filename");
            }
            if self
                .db()
                .find_mpq_install_owner(&new_path)?
                .is_some_and(|owner| owner != repo_id)
            {
                anyhow::bail!(MpqError::ManagedTarget(stored_file_name));
            }
            let current = Self::resolve_install_path(&entry.path, Some(wow_dir))
                .and_then(|path| Self::find_actual_case(&path).or(Some(path)))
                .ok_or_else(|| anyhow::anyhow!("Could not resolve a tracked MPQ path"))?;
            if !current.is_file() {
                anyhow::bail!("A selected MPQ is no longer present");
            }
            let path_changed = !entry.path.eq_ignore_ascii_case(&new_path)
                || current.file_name() != desired.file_name();
            let friendly_name_changed = entry
                .display_name
                .as_deref()
                .map(str::trim)
                .is_none_or(|current| current != component_name);
            if (path_changed || friendly_name_changed)
                && !self.tracked_mpq_editor_unlocked(wow_dir, &entry.path)?
            {
                anyhow::bail!("Unlock every changed MPQ in Manage MPQs before editing the package");
            }
            let backup = if path_changed {
                self.db()
                    .get_mpq_backup(repo_id, &entry.path)?
                    .and_then(|backup| {
                        Self::resolve_install_path(&backup.backup_path, Some(wow_dir))
                    })
                    .filter(|path| path.is_file())
            } else {
                None
            };
            prepared.push(Prepared {
                old_path: entry.path.clone(),
                new_path,
                display_name: component_name.to_string(),
                current,
                desired,
                path_changed,
                backup,
            });
        }

        // Moving directly into another component's old path would require
        // ambiguous backup ownership. Keep that uncommon swap explicit instead
        // of risking the wrong file being restored.
        for edit in &prepared {
            if edit.path_changed
                && prepared.iter().any(|other| {
                    !other.old_path.eq_ignore_ascii_case(&edit.old_path)
                        && other.old_path.eq_ignore_ascii_case(&edit.new_path)
                })
            {
                anyhow::bail!(
                    "MPQ components cannot swap filenames in one edit; save an intermediate name first"
                );
            }
            if edit.path_changed {
                if let Some(existing) = edit.desired.parent().and_then(|parent| {
                    find_case_insensitive_child(
                        parent,
                        edit.desired
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or(""),
                    )
                }) {
                    if existing != edit.current {
                        anyhow::bail!(MpqError::ToggleCollision(
                            edit.desired
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("MPQ")
                                .to_string()
                        ));
                    }
                }
            }
        }

        struct MoveState {
            original: PathBuf,
            staged: PathBuf,
            desired: PathBuf,
            backup: Option<PathBuf>,
            staged_ok: bool,
            deployed: bool,
            backup_restored: bool,
        }

        fn rollback_moves(moves: &mut [MoveState]) {
            for moved in moves.iter_mut().rev() {
                if moved.backup_restored {
                    if let Some(backup) = moved.backup.as_ref() {
                        let _ = fs::rename(&moved.original, backup);
                    }
                    moved.backup_restored = false;
                }
            }
            for moved in moves.iter_mut().rev() {
                if moved.deployed {
                    let _ = fs::rename(&moved.desired, &moved.staged);
                    moved.deployed = false;
                }
            }
            for moved in moves.iter_mut().rev() {
                if moved.staged_ok {
                    let _ = fs::rename(&moved.staged, &moved.original);
                    moved.staged_ok = false;
                }
            }
        }

        let staging_parent = util::cache_dir(Some(wow_dir))?.join("mpq-staging");
        fs::create_dir_all(&staging_parent)?;
        let staging = tempfile::Builder::new()
            .prefix("package-edit-")
            .tempdir_in(&staging_parent)?;
        let mut moves = prepared
            .iter()
            .enumerate()
            .filter(|(_, edit)| edit.path_changed)
            .map(|(index, edit)| MoveState {
                original: edit.current.clone(),
                staged: staging.path().join(format!("component-{index}.mpq")),
                desired: edit.desired.clone(),
                backup: edit.backup.clone(),
                staged_ok: false,
                deployed: false,
                backup_restored: false,
            })
            .collect::<Vec<_>>();

        for moved in &mut moves {
            if let Err(error) = fs::rename(&moved.original, &moved.staged) {
                rollback_moves(&mut moves);
                return Err(error.into());
            }
            moved.staged_ok = true;
        }
        for moved in &mut moves {
            if let Some(parent) = moved.desired.parent() {
                if let Err(error) = fs::create_dir_all(parent) {
                    rollback_moves(&mut moves);
                    return Err(error.into());
                }
            }
            if let Err(error) = fs::rename(&moved.staged, &moved.desired) {
                rollback_moves(&mut moves);
                return Err(error.into());
            }
            moved.deployed = true;
        }
        for moved in &mut moves {
            if let Some(backup) = moved.backup.as_ref() {
                if let Err(error) = fs::rename(backup, &moved.original) {
                    rollback_moves(&mut moves);
                    return Err(error.into());
                }
                moved.backup_restored = true;
            }
        }

        let db_edits = prepared
            .iter()
            .map(|edit| db::MpqPackageInstallEdit {
                old_path: edit.old_path.clone(),
                new_path: edit.new_path.clone(),
                display_name: edit.display_name.clone(),
                path_changed: edit.path_changed,
            })
            .collect::<Vec<_>>();
        if let Err(error) = self
            .db()
            .edit_mpq_package_metadata(repo_id, display_name, &db_edits)
        {
            rollback_moves(&mut moves);
            return Err(error);
        }

        for edit in &prepared {
            set_friendly_comment(
                if edit.path_changed {
                    &edit.desired
                } else {
                    &edit.current
                },
                &edit.display_name,
                set_xattr_comment,
            );
        }
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "tracked MPQ package edit committed: repo_id={repo_id}; component_count={}; moved_count={}; values omitted",
                prepared.len(),
                moves.len()
            ),
        );
        Ok(())
    }

    fn local_mpq_base_name(source: &Path) -> String {
        source
            .file_stem()
            .and_then(|name| name.to_str())
            .map(Self::sanitize_for_fs)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Local MPQ package".to_string())
    }

    fn local_mpq_repo_name(source: &Path) -> Result<String> {
        let metadata = fs::symlink_metadata(source)
            .map_err(|_| MpqError::Filesystem("reading MPQ source metadata"))?;
        let source_hash = util::sha256_hex(&metadata_fingerprint(&metadata));
        let base_name = Self::local_mpq_base_name(source);
        let suffix = source_hash.get(..8).unwrap_or(&source_hash);
        Ok(format!("{base_name}-{suffix}"))
    }

    pub fn preview_local_mpq_targets(
        &self,
        wow_dir: &Path,
        source: &Path,
        selections: &[MpqInstallSelection],
    ) -> Result<Vec<MpqTargetPreview>> {
        let _ = stage_source(wow_dir, source)?;
        let repo_name = Self::local_mpq_repo_name(source)?;
        let expected_repo_id = self
            .db()
            .find_repo_by_identity("local-mpq", "local", &repo_name)?
            .map(|repo| repo.id);
        let _ = self.list_mpq_protection(wow_dir)?;
        let mut previews = Vec::with_capacity(selections.len());
        let mut targets = HashSet::new();
        for selection in selections {
            validate_selection(selection)?;
            let manifest = selection.destination.manifest_path(&selection.file_name);
            if !targets.insert(manifest.to_ascii_lowercase()) {
                anyhow::bail!(MpqError::InvalidSelection(
                    "Two MPQs cannot use the same destination filename".to_string()
                ));
            }
            let target = target_path(wow_dir, &selection.destination, &selection.file_name)?;
            let existing = target
                .parent()
                .and_then(|parent| find_case_insensitive_child(parent, &selection.file_name));
            let owner = self.db().find_mpq_install_owner(&manifest)?;
            let status = if let Some(owner) = owner {
                if Some(owner) == expected_repo_id {
                    MpqTargetStatus::SamePackage
                } else {
                    MpqTargetStatus::ManagedByAnotherPackage
                }
            } else if existing.is_none() {
                if is_reserved_core_filename(&selection.file_name) {
                    MpqTargetStatus::ProtectedCore
                } else {
                    MpqTargetStatus::Available
                }
            } else {
                let protection = self.db().get_mpq_protection(&manifest)?;
                match protection {
                    Some(row) if row.core && row.protected => MpqTargetStatus::ProtectedCore,
                    Some(row) if !row.protected => MpqTargetStatus::UnprotectedReplacement,
                    _ => MpqTargetStatus::ProtectedUntracked,
                }
            };
            previews.push(MpqTargetPreview {
                source_key: selection.source_key.clone(),
                manifest_path: manifest,
                status,
            });
        }
        Ok(previews)
    }

    pub fn install_local_mpq_package(
        &self,
        wow_dir: &Path,
        source: &Path,
        selections: &[MpqInstallSelection],
        set_xattr_comment: bool,
    ) -> Result<i64> {
        let _diagnostic = diagnostics::OperationGuard::new("install_local_mpq_package");
        let staged = stage_source(wow_dir, source)?;
        let source_metadata = fs::symlink_metadata(source)
            .map_err(|_| MpqError::Filesystem("reading MPQ source metadata"))?;
        let source_hash = util::sha256_hex(&metadata_fingerprint(&source_metadata));
        let repo_name = Self::local_mpq_repo_name(source)?;
        let repo_id = self.ensure_mpq_repo(MpqRemotePackage {
            url: String::new(),
            forge: "local".to_string(),
            host: "local-mpq".to_string(),
            owner: "local".to_string(),
            name: repo_name,
        })?;
        self.db()
            .ensure_mpq_package_display_name(repo_id, &Self::local_mpq_base_name(source))?;
        let installed_asset = db::InstalledAssetState {
            version: Some("Local".to_string()),
            asset_id: Some(source_hash),
            asset_size: source.metadata().ok().map(|meta| meta.len() as i64),
            installed_at_unix: Some(Self::now_unix()),
            ..db::InstalledAssetState::default()
        };

        if let Err(error) = self.commit_staged_mpq_package(
            repo_id,
            wow_dir,
            &staged,
            selections,
            set_xattr_comment,
            &installed_asset,
        ) {
            if self
                .db()
                .list_installs(repo_id)
                .unwrap_or_default()
                .is_empty()
            {
                let _ = self.db().remove_repo(repo_id);
            }
            return Err(error);
        }
        Ok(repo_id)
    }

    pub async fn install_remote_mpq_package(
        &self,
        wow_dir: &Path,
        package: MpqRemotePackage,
        assets: &[MpqRemoteAsset],
        set_xattr_comment: bool,
    ) -> Result<i64> {
        let _diagnostic = diagnostics::OperationGuard::new("install_remote_mpq_package");
        if assets.is_empty() {
            anyhow::bail!(MpqError::NoMpqFiles);
        }
        let staging_parent = util::cache_dir(Some(wow_dir))?.join("mpq-staging");
        fs::create_dir_all(&staging_parent)?;
        let downloads = tempfile::Builder::new()
            .prefix("download-")
            .tempdir_in(&staging_parent)?;
        let mut sources = Vec::new();
        let mut selections = Vec::new();

        for asset in assets {
            validate_target_file_name(&asset.asset_name)?;
            let target_file_name = asset
                .target_file_name
                .as_deref()
                .unwrap_or(&asset.asset_name);
            validate_target_file_name(target_file_name)?;
            let url = Url::parse(&asset.download_url)?;
            if url.scheme() != "https" {
                anyhow::bail!("Remote MPQ downloads must use HTTPS");
            }
            let destination = downloads.path().join(&asset.asset_name);
            super::network::download_to_file(
                &self.download_client,
                &asset.download_url,
                &destination,
                super::network::MAX_REMOTE_ASSET_BYTES,
                |url| {
                    Self::validate_asset_url_for_source(
                        &package.forge,
                        &package.host,
                        &package.owner,
                        &package.name,
                        url,
                    )
                },
                |path| {
                    if let Some(expected) = asset.size {
                        let actual = path.metadata()?.len();
                        if actual != expected {
                            anyhow::bail!("Downloaded MPQ size did not match the release metadata");
                        }
                    }
                    Self::verify_asset_digest(path, asset.sha256.as_deref())?;
                    validate_mpq_file(path).map_err(anyhow::Error::from)
                },
            )
            .await?;
            if let Some(expected) = asset.size {
                let actual = destination.metadata()?.len();
                if actual != expected {
                    anyhow::bail!("Downloaded MPQ size did not match the release metadata");
                }
            }
            Self::verify_asset_digest(&destination, asset.sha256.as_deref())?;
            validate_mpq_file(&destination)?;
            sources.push((asset.asset_name.clone(), destination));
            selections.push(MpqInstallSelection {
                source_key: asset.asset_name.clone(),
                display_name: asset.display_name.clone(),
                file_name: target_file_name.to_string(),
                destination: asset.destination.clone(),
                replace_unprotected: asset.replace_unprotected,
                version: asset.version.clone(),
            });
        }

        let versions = assets
            .iter()
            .filter_map(|asset| asset.version.clone())
            .collect::<Vec<_>>();
        let version = if versions.is_empty() {
            "Manual".to_string()
        } else {
            versions.join(" + ")
        };
        let installed_asset = db::InstalledAssetState {
            version: Some(version),
            installed_at_unix: Some(Self::now_unix()),
            ..db::InstalledAssetState::default()
        };
        let staged = stage_files(wow_dir, &sources)?;
        let package_display_name = package.name.clone();
        let repo_id = self.ensure_mpq_repo(package)?;
        self.db()
            .ensure_mpq_package_display_name(repo_id, &package_display_name)?;
        if let Err(error) = self.commit_staged_mpq_package(
            repo_id,
            wow_dir,
            &staged,
            &selections,
            set_xattr_comment,
            &installed_asset,
        ) {
            if self
                .db()
                .list_installs(repo_id)
                .unwrap_or_default()
                .is_empty()
            {
                let _ = self.db().remove_repo(repo_id);
            }
            return Err(error);
        }
        Ok(repo_id)
    }

    fn ensure_mpq_repo(&self, package: MpqRemotePackage) -> Result<i64> {
        let existing = {
            self.db()
                .find_repo_by_identity(&package.host, &package.owner, &package.name)?
        };
        if let Some(existing) = existing {
            self.db()
                .set_repo_release_source(existing.id, &InstallMode::Mpq, None, None, None)?;
            return Ok(existing.id);
        }
        self.db().add_repo(&Repo {
            id: 0,
            url: package.url,
            forge: package.forge,
            host: package.host,
            owner: package.owner,
            name: package.name,
            mode: InstallMode::Mpq,
            enabled: true,
            git_branch: None,
            asset_regex: None,
            last_version: None,
            etag: None,
            installed_asset_id: None,
            installed_asset_name: None,
            installed_asset_size: None,
            installed_asset_url: None,
            installed_at_unix: None,
            published_at_unix: None,
            merge_installs: false,
            pinned_version: None,
            selected_addons_json: None,
        })
    }

    fn commit_staged_mpq_package(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        staged: &StagedMpqSource,
        selections: &[MpqInstallSelection],
        set_xattr_comment: bool,
        installed_asset: &db::InstalledAssetState,
    ) -> Result<()> {
        if selections.is_empty() {
            anyhow::bail!(MpqError::NoMpqFiles);
        }
        let _ = self.list_mpq_protection(wow_dir)?;

        let mut target_keys = HashSet::new();
        let mut display_names = HashSet::new();
        struct Prepared<'a> {
            selection: &'a MpqInstallSelection,
            source: &'a Path,
            target: PathBuf,
            manifest: String,
            existing: Option<PathBuf>,
            owner: Option<i64>,
        }
        let mut prepared = Vec::new();

        for selection in selections {
            validate_selection(selection)?;
            if !display_names.insert(selection.display_name.trim().to_ascii_lowercase()) {
                anyhow::bail!(MpqError::InvalidSelection(
                    "Friendly MPQ names must be unique within a package".to_string()
                ));
            }
            let source = staged.file(&selection.source_key).ok_or_else(|| {
                MpqError::InvalidSelection(
                    "The selected MPQ is no longer present in staging".to_string(),
                )
            })?;
            let target = target_path(wow_dir, &selection.destination, &selection.file_name)?;
            let manifest = selection.destination.manifest_path(&selection.file_name);
            if !target_keys.insert(manifest.to_ascii_lowercase()) {
                anyhow::bail!(MpqError::InvalidSelection(
                    "Two MPQs cannot use the same destination filename".to_string()
                ));
            }
            let existing = target
                .parent()
                .and_then(|parent| find_case_insensitive_child(parent, &selection.file_name));
            let owner = self.db().find_mpq_install_owner(&manifest)?;
            if let Some(owner_id) = owner {
                if owner_id != repo_id {
                    anyhow::bail!(MpqError::ManagedTarget(selection.file_name.clone()));
                }
                if let Some(existing_path) = &existing {
                    let tracked = self.db().list_installs(repo_id)?.into_iter().find(|entry| {
                        entry.kind == "mpq" && entry.path.eq_ignore_ascii_case(&manifest)
                    });
                    if let Some(expected) = tracked.and_then(|entry| entry.file_fingerprint) {
                        let current = fs::symlink_metadata(existing_path)
                            .map(|metadata| metadata_fingerprint(&metadata))
                            .map_err(|_| MpqError::Filesystem("reading MPQ metadata"))?;
                        let explicitly_protected = self
                            .db()
                            .get_mpq_protection(&manifest)?
                            .filter(|row| row.fingerprint == current)
                            .map(|row| row.protected)
                            .unwrap_or(false);
                        if current != expected && explicitly_protected {
                            anyhow::bail!(MpqError::ProtectedTarget(selection.file_name.clone()));
                        }
                    }
                }
            } else if existing.is_some() {
                let protection = self.db().get_mpq_protection(&manifest)?;
                if protection.as_ref().map(|row| row.protected).unwrap_or(true) {
                    anyhow::bail!(MpqError::ProtectedTarget(selection.file_name.clone()));
                }
                if !selection.replace_unprotected {
                    anyhow::bail!(MpqError::ReplacementNotApproved(
                        selection.file_name.clone()
                    ));
                }
            } else if is_reserved_core_filename(&selection.file_name) {
                anyhow::bail!(MpqError::ProtectedTarget(selection.file_name.clone()));
            }
            prepared.push(Prepared {
                selection,
                source,
                target,
                manifest,
                existing,
                owner,
            });
        }

        let rollback_parent = util::cache_dir(Some(wow_dir))?.join("mpq-staging");
        fs::create_dir_all(&rollback_parent)?;
        let rollback_dir = tempfile::Builder::new()
            .prefix("rollback-")
            .tempdir_in(&rollback_parent)?;
        let mut rollback = Vec::<(PathBuf, Option<PathBuf>)>::new();
        struct StaleRollback {
            target: PathBuf,
            managed_copy: Option<PathBuf>,
            backup_path: Option<PathBuf>,
            restored_backup: bool,
        }
        let desired_paths = prepared
            .iter()
            .map(|item| item.manifest.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let stale_entries = self
            .db()
            .list_installs(repo_id)?
            .into_iter()
            .filter(|entry| {
                entry.kind == "mpq" && !desired_paths.contains(&entry.path.to_ascii_lowercase())
            })
            .collect::<Vec<_>>();
        let mut stale_rollback = Vec::<StaleRollback>::new();
        let mut installs = Vec::<db::InstallEntry>::new();
        let mut backups = Vec::<db::MpqBackupRow>::new();

        let operation = (|| -> Result<()> {
            for (index, item) in prepared.iter().enumerate() {
                let mut previous = None;
                if let Some(existing) = &item.existing {
                    if item.owner == Some(repo_id) {
                        if let Some(existing_backup) =
                            self.db().get_mpq_backup(repo_id, &item.manifest)?
                        {
                            backups.push(existing_backup);
                        }
                        let saved = rollback_dir.path().join(format!("existing-{index}.mpq"));
                        fs::rename(existing, &saved)?;
                        previous = Some(saved);
                    } else {
                        let metadata = fs::symlink_metadata(existing)
                            .map_err(|_| MpqError::Filesystem("reading MPQ metadata"))?;
                        let identity = util::sha256_hex(&metadata_fingerprint(&metadata));
                        let backup_root = backup_root(wow_dir, repo_id);
                        fs::create_dir_all(&backup_root)?;
                        let prefix = identity.get(..12).unwrap_or(&identity);
                        let backup_name = format!("{}-{}", prefix, item.selection.file_name);
                        let backup = backup_root.join(backup_name);
                        if backup.exists() {
                            fs::remove_file(&backup)?;
                        }
                        fs::rename(existing, &backup)?;
                        previous = Some(backup.clone());
                        backups.push(db::MpqBackupRow {
                            repo_id,
                            path: item.manifest.clone(),
                            backup_path: Self::to_manifest_path(&backup, wow_dir),
                            sha256: None,
                            fingerprint: Some(metadata_fingerprint(&metadata)),
                        });
                    }
                }

                if let Err(error) = copy_atomic(item.source, &item.target) {
                    if let Some(previous) = &previous {
                        let _ = fs::rename(previous, &item.target);
                    }
                    return Err(error.into());
                }
                set_friendly_comment(
                    &item.target,
                    &item.selection.display_name,
                    set_xattr_comment,
                );
                rollback.push((item.target.clone(), previous));
                let metadata = fs::symlink_metadata(&item.target)
                    .map_err(|_| MpqError::Filesystem("reading installed MPQ metadata"))?;
                installs.push(db::InstallEntry {
                    path: item.manifest.clone(),
                    kind: "mpq".to_string(),
                    sha256: None,
                    version: item.selection.version.clone(),
                    display_name: Some(item.selection.display_name.trim().to_string()),
                    file_fingerprint: Some(metadata_fingerprint(&metadata)),
                });
            }

            for (index, stale) in stale_entries.iter().enumerate() {
                let target = Self::resolve_install_path(&stale.path, Some(wow_dir))
                    .ok_or_else(|| anyhow::anyhow!("Could not resolve a stale MPQ path"))?;
                let actual = Self::find_actual_case(&target).unwrap_or(target.clone());
                let managed_copy = if actual.is_file() {
                    let saved = rollback_dir.path().join(format!("stale-{index}.mpq"));
                    fs::rename(&actual, &saved)?;
                    Some(saved)
                } else {
                    None
                };
                let backup_path =
                    self.db()
                        .get_mpq_backup(repo_id, &stale.path)?
                        .and_then(|backup| {
                            Self::resolve_install_path(&backup.backup_path, Some(wow_dir))
                        });
                let mut restored_backup = false;
                if let Some(backup) = backup_path.as_ref().filter(|path| path.is_file()) {
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::rename(backup, &target)?;
                    restored_backup = true;
                }
                stale_rollback.push(StaleRollback {
                    target,
                    managed_copy,
                    backup_path,
                    restored_backup,
                });
            }
            self.db()
                .commit_mpq_installs(repo_id, &installs, &backups, installed_asset)?;
            Ok(())
        })();

        if let Err(error) = operation {
            for stale in stale_rollback.iter().rev() {
                if stale.restored_backup {
                    if let Some(backup_path) = &stale.backup_path {
                        let _ = fs::rename(&stale.target, backup_path);
                    }
                }
                if let Some(managed_copy) = &stale.managed_copy {
                    let _ = fs::rename(managed_copy, &stale.target);
                }
            }
            for (target, previous) in rollback.iter().rev() {
                let _ = Self::remove_any_target(target);
                if let Some(previous) = previous {
                    if let Some(parent) = target.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::rename(previous, target);
                }
            }
            return Err(error);
        }

        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "installed MPQ package: repo_id={repo_id}; file_count={}",
                installs.len()
            ),
        );
        Ok(())
    }

    pub fn rename_mpq_display_name(
        &self,
        repo_id: i64,
        manifest_path: &str,
        display_name: &str,
        wow_dir: &Path,
        set_xattr_comment: bool,
    ) -> Result<()> {
        let display_name = display_name.trim();
        if display_name.is_empty() {
            anyhow::bail!("MPQ friendly name cannot be empty");
        }
        let entry = self
            .db()
            .list_installs(repo_id)?
            .into_iter()
            .find(|entry| entry.kind == "mpq" && entry.path == manifest_path)
            .ok_or_else(|| anyhow::anyhow!("Tracked MPQ not found"))?;
        if !self.tracked_mpq_editor_unlocked(wow_dir, &entry.path)? {
            anyhow::bail!("Unlock this MPQ before editing it");
        }
        self.db()
            .set_install_display_name(repo_id, &entry.path, display_name)?;
        if let Some(path) = Self::resolve_install_path(&entry.path, Some(wow_dir)) {
            set_friendly_comment(&path, display_name, set_xattr_comment);
        }
        Ok(())
    }

    pub fn list_installed_mpqs(
        &self,
        repo_id: i64,
        wow_dir: &Path,
    ) -> Result<Vec<MpqInstalledFile>> {
        let mut files = Vec::new();
        // Keep repository-row construction read-only. In particular, do not
        // try to backfill legacy fingerprints from inside this loop: the
        // temporary DB mutex guard used to live for the whole `for` statement,
        // so acquiring it again while handling a missing fingerprint
        // deadlocked startup indefinitely.
        let installs = self.db().list_installs(repo_id)?;
        for entry in installs.into_iter().filter(|entry| entry.kind == "mpq") {
            let enabled = !is_disabled_manifest_path(&entry.path);
            let status = match Self::resolve_install_path(&entry.path, Some(wow_dir)) {
                Some(path) => {
                    let actual = Self::find_actual_case(&path).unwrap_or(path);
                    if !actual.is_file() {
                        MpqFileStatus::Missing
                    } else {
                        let current = fs::symlink_metadata(&actual)
                            .map(|metadata| metadata_fingerprint(&metadata));
                        match (entry.file_fingerprint.as_deref(), current) {
                            (Some(expected), Ok(current)) if current != expected => {
                                MpqFileStatus::Modified
                            }
                            // MPQs tracked before metadata fingerprints were
                            // introduced have no reliable baseline. Treat them
                            // as installed without mutating the database while
                            // the UI is merely reading repository rows. A
                            // subsequent reinstall records the fingerprint.
                            (None, Ok(_)) => MpqFileStatus::Installed,
                            (_, Err(_)) => MpqFileStatus::Modified,
                            _ => MpqFileStatus::Installed,
                        }
                    }
                }
                None => MpqFileStatus::Missing,
            };
            let protection_state = Self::resolve_install_path(&entry.path, Some(wow_dir))
                .and_then(|path| Self::find_actual_case(&path).or(Some(path)))
                .and_then(|path| fs::symlink_metadata(path).ok())
                .map(|metadata| metadata_fingerprint(&metadata))
                .and_then(|fingerprint| {
                    self.db()
                        .get_mpq_protection(&entry.path)
                        .ok()
                        .flatten()
                        .filter(|row| row.fingerprint == fingerprint)
                });
            let protected = protection_state
                .as_ref()
                .map(|row| row.protected)
                .unwrap_or(false);
            let editor_unlocked = protection_state
                .as_ref()
                .map(|row| row.editor_unlocked)
                .unwrap_or(false);
            files.push(MpqInstalledFile {
                display_name: entry.display_name.clone().unwrap_or_else(|| {
                    Path::new(&entry.path)
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .unwrap_or("MPQ")
                        .to_string()
                }),
                path: entry.path,
                sha256: entry.sha256.unwrap_or_default(),
                version: entry.version,
                enabled,
                protected,
                editor_unlocked,
                status,
            });
        }
        files.sort_by(|left, right| {
            left.display_name
                .to_ascii_lowercase()
                .cmp(&right.display_name.to_ascii_lowercase())
        });
        Ok(files)
    }

    /// Enable or disable one tracked MPQ, or every MPQ in a package when
    /// `manifest_path` is `None`. Disabled files retain their complete name and
    /// receive a final `.disabled` suffix so the WoW client ignores them.
    pub fn set_mpq_enabled(
        &self,
        repo_id: i64,
        manifest_path: Option<&str>,
        enabled: bool,
        wow_dir: &Path,
    ) -> Result<usize> {
        let _diagnostic = diagnostics::OperationGuard::new("set_mpq_enabled");
        // Refresh untracked target identities first. This is metadata-only and
        // lets a user explicitly approve a restored file that changed while a
        // replacement patch was disabled.
        let _ = self.list_mpq_protection(wow_dir)?;
        let entries = self
            .db()
            .list_installs(repo_id)?
            .into_iter()
            .filter(|entry| entry.kind == "mpq")
            .collect::<Vec<_>>();
        let selected = entries
            .iter()
            .filter(|entry| {
                manifest_path
                    .map(|path| entry.path.eq_ignore_ascii_case(path))
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        if selected.is_empty() {
            anyhow::bail!("Tracked MPQ not found");
        }
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "tracked MPQ state plan started: repo_id={repo_id}; scope={}; selected_count={}; enabled={enabled}",
                if manifest_path.is_some() {
                    "component"
                } else {
                    "package"
                },
                selected.len()
            ),
        );
        for entry in &selected {
            if !self.tracked_mpq_editor_unlocked(wow_dir, &entry.path)? {
                anyhow::bail!("Unlock this MPQ before changing its enabled state");
            }
        }

        #[derive(Clone)]
        struct ToggleItem {
            old_manifest: String,
            new_manifest: String,
            current: PathBuf,
            desired: PathBuf,
            backup_path: Option<PathBuf>,
            restored_target: Option<PathBuf>,
        }

        let mut toggles = Vec::<ToggleItem>::new();
        for entry in selected {
            let currently_enabled = !is_disabled_manifest_path(&entry.path);
            if currently_enabled == enabled {
                continue;
            }
            let new_manifest = if enabled {
                enabled_manifest_path(&entry.path)
            } else {
                disabled_manifest_path(&entry.path)
            };
            let expected_target = Self::resolve_install_path(&entry.path, Some(wow_dir))
                .ok_or_else(|| anyhow::anyhow!("Could not resolve the tracked MPQ path"))?;
            let current = Self::find_actual_case(&expected_target).unwrap_or(expected_target);
            let file_name = Path::new(&entry.path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("MPQ")
                .to_string();
            if !current.is_file() {
                anyhow::bail!(MpqError::MissingTarget(file_name));
            }
            if let Some(expected) = entry.file_fingerprint.as_deref() {
                let actual = fs::symlink_metadata(&current)
                    .map(|metadata| metadata_fingerprint(&metadata))
                    .map_err(|_| MpqError::Filesystem("reading MPQ metadata"))?;
                if actual != expected {
                    anyhow::bail!(MpqError::ModifiedTarget(file_name));
                }
            }

            let desired = Self::resolve_install_path(&new_manifest, Some(wow_dir))
                .ok_or_else(|| anyhow::anyhow!("Could not resolve the MPQ toggle destination"))?;
            if self.db().find_mpq_install_owner(&new_manifest)?.is_some() {
                anyhow::bail!(MpqError::ToggleCollision(
                    Path::new(&new_manifest)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("MPQ")
                        .to_string()
                ));
            }
            let desired_actual = Self::find_actual_case(&desired);
            let backup = self.db().get_mpq_backup(repo_id, &entry.path)?;
            let backup_path = backup
                .as_ref()
                .and_then(|row| Self::resolve_install_path(&row.backup_path, Some(wow_dir)));
            let mut restored_target = None;

            if enabled {
                if let Some(existing) = desired_actual.filter(|path| path.exists()) {
                    let Some(backup) = backup.as_ref() else {
                        anyhow::bail!(MpqError::ToggleCollision(
                            Path::new(&new_manifest)
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("MPQ")
                                .to_string()
                        ));
                    };
                    let Some(storage) = backup_path.as_ref() else {
                        anyhow::bail!(MpqError::Filesystem("resolving an MPQ backup"));
                    };
                    if storage.exists() {
                        anyhow::bail!(MpqError::ToggleCollision(file_name));
                    }
                    if let Some(expected) = backup.fingerprint.as_deref() {
                        let current = fs::symlink_metadata(&existing)
                            .map(|metadata| metadata_fingerprint(&metadata))
                            .map_err(|_| MpqError::Filesystem("reading MPQ metadata"))?;
                        let explicitly_unprotected = self
                            .db()
                            .get_mpq_protection(&new_manifest)?
                            .filter(|row| row.fingerprint == current)
                            .map(|row| !row.protected && !row.core)
                            .unwrap_or(false);
                        if current != expected && !explicitly_unprotected {
                            anyhow::bail!(MpqError::RestoredTargetModified(file_name));
                        }
                    }
                    restored_target = Some(existing);
                }
            } else if desired_actual.map(|path| path.exists()).unwrap_or(false) {
                anyhow::bail!(MpqError::ToggleCollision(
                    Path::new(&new_manifest)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("MPQ")
                        .to_string()
                ));
            }

            toggles.push(ToggleItem {
                old_manifest: entry.path.clone(),
                new_manifest,
                current,
                desired,
                backup_path,
                restored_target,
            });
        }

        #[derive(Clone)]
        struct AppliedToggle {
            item: ToggleItem,
            backup_moved: bool,
        }
        let rollback = |applied: &[AppliedToggle]| {
            if !applied.is_empty() {
                diagnostics::emit(
                    diagnostics::DiagnosticLevel::Debug,
                    "engine.mpq",
                    format!(
                        "tracked MPQ filesystem rollback started: repo_id={repo_id}; component_count={}",
                        applied.len()
                    ),
                );
            }
            for applied in applied.iter().rev() {
                let item = &applied.item;
                if enabled {
                    if item.desired.exists() {
                        let _ = fs::rename(&item.desired, &item.current);
                    }
                    if applied.backup_moved {
                        if let (Some(storage), Some(original)) =
                            (&item.backup_path, &item.restored_target)
                        {
                            let _ = fs::rename(storage, original);
                        }
                    }
                } else {
                    if applied.backup_moved {
                        if let Some(storage) = &item.backup_path {
                            let _ = fs::rename(&item.current, storage);
                        }
                    }
                    if item.desired.exists() {
                        let _ = fs::rename(&item.desired, &item.current);
                    }
                }
            }
            if !applied.is_empty() {
                diagnostics::emit(
                    diagnostics::DiagnosticLevel::Debug,
                    "engine.mpq",
                    format!(
                        "tracked MPQ filesystem rollback finished: repo_id={repo_id}; component_count={}",
                        applied.len()
                    ),
                );
            }
        };

        let mut applied = Vec::<AppliedToggle>::new();
        for (index, item) in toggles.iter().enumerate() {
            let mut backup_moved = false;
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Trace,
                "engine.mpq",
                format!(
                    "tracked MPQ component transition started: repo_id={repo_id}; component={}/{}; enabled={enabled}; backup_or_restore_candidate={}; paths omitted",
                    index + 1,
                    toggles.len(),
                    item.restored_target.is_some() || item.backup_path.is_some()
                ),
            );
            let operation = (|| -> Result<()> {
                if enabled {
                    if let (Some(original), Some(storage)) =
                        (&item.restored_target, &item.backup_path)
                    {
                        if let Some(parent) = storage.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::rename(original, storage)?;
                        backup_moved = true;
                        diagnostics::emit(
                            diagnostics::DiagnosticLevel::Trace,
                            "engine.mpq",
                            format!(
                                "tracked MPQ conflicting target moved to backup: repo_id={repo_id}; component={}; paths omitted",
                                index + 1
                            ),
                        );
                    }
                    fs::rename(&item.current, &item.desired)?;
                } else {
                    fs::rename(&item.current, &item.desired)?;
                    if let Some(storage) = item.backup_path.as_ref().filter(|path| path.is_file()) {
                        fs::rename(storage, &item.current)?;
                        backup_moved = true;
                        diagnostics::emit(
                            diagnostics::DiagnosticLevel::Trace,
                            "engine.mpq",
                            format!(
                                "tracked MPQ displaced target restored from backup: repo_id={repo_id}; component={}; paths omitted",
                                index + 1
                            ),
                        );
                    }
                }
                Ok(())
            })();
            if let Err(error) = operation {
                let current = AppliedToggle {
                    item: item.clone(),
                    backup_moved,
                };
                rollback(std::slice::from_ref(&current));
                rollback(&applied);
                diagnostics::emit(
                    diagnostics::DiagnosticLevel::Debug,
                    "engine.mpq",
                    format!(
                        "tracked MPQ component transition failed: repo_id={repo_id}; component={}; rollback attempted",
                        index + 1
                    ),
                );
                return Err(error);
            }
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Trace,
                "engine.mpq",
                format!(
                    "tracked MPQ component filesystem rename completed: repo_id={repo_id}; component={}/{}; enabled={enabled}; paths omitted",
                    index + 1,
                    toggles.len()
                ),
            );
            applied.push(AppliedToggle {
                item: item.clone(),
                backup_moved,
            });
        }

        let changes = toggles
            .iter()
            .map(|item| (item.old_manifest.clone(), item.new_manifest.clone()))
            .collect::<Vec<_>>();
        let repo_enabled = entries.iter().all(|entry| {
            changes
                .iter()
                .find(|(old, _)| old.eq_ignore_ascii_case(&entry.path))
                .map(|(_, new)| !is_disabled_manifest_path(new))
                .unwrap_or_else(|| !is_disabled_manifest_path(&entry.path))
        });
        if let Err(error) = self
            .db()
            .update_mpq_enabled_paths(repo_id, &changes, repo_enabled)
        {
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Debug,
                "engine.mpq",
                format!(
                    "tracked MPQ metadata commit failed: repo_id={repo_id}; filesystem rollback starting"
                ),
            );
            rollback(&applied);
            return Err(error);
        }

        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "{} {} tracked MPQ component(s): repo_id={repo_id}; filesystem_renames={}; metadata_committed=true; repo_enabled={repo_enabled}",
                if enabled { "enabled" } else { "disabled" },
                changes.len(),
                changes.len()
            ),
        );
        Ok(changes.len())
    }

    pub fn remove_mpq_component(
        &self,
        repo_id: i64,
        manifest_path: &str,
        wow_dir: &Path,
        force_modified: bool,
    ) -> Result<bool> {
        let _diagnostic = diagnostics::OperationGuard::new("remove_mpq_component");
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "tracked MPQ component removal started: repo_id={repo_id}; force_modified={force_modified}; path omitted"
            ),
        );
        let entry = self
            .db()
            .list_installs(repo_id)?
            .into_iter()
            .find(|entry| entry.kind == "mpq" && entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("Tracked MPQ not found"))?;
        if !self.tracked_mpq_editor_unlocked(wow_dir, &entry.path)? {
            anyhow::bail!("Unlock this MPQ before removing it");
        }
        let target = Self::resolve_install_path(&entry.path, Some(wow_dir))
            .ok_or_else(|| anyhow::anyhow!("Could not resolve the tracked MPQ path"))?;
        let actual = Self::find_actual_case(&target).unwrap_or(target.clone());
        if actual.is_file() && !force_modified {
            if let Some(expected) = entry.file_fingerprint.as_deref() {
                let current = fs::symlink_metadata(&actual)
                    .map(|metadata| metadata_fingerprint(&metadata))
                    .map_err(|_| MpqError::Filesystem("reading MPQ metadata"))?;
                if current != expected {
                    let name = Path::new(&entry.path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("MPQ")
                        .to_string();
                    anyhow::bail!(MpqError::ModifiedTarget(name));
                }
            }
        }

        let rollback_parent = util::cache_dir(Some(wow_dir))?.join("mpq-staging");
        fs::create_dir_all(&rollback_parent)?;
        let rollback = tempfile::Builder::new()
            .prefix("remove-")
            .tempdir_in(&rollback_parent)?;
        let removed_copy = rollback.path().join("managed.mpq");
        let had_target = actual.is_file();
        if had_target {
            fs::rename(&actual, &removed_copy)?;
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Trace,
                "engine.mpq",
                format!(
                    "tracked MPQ moved into removal rollback staging: repo_id={repo_id}; path omitted"
                ),
            );
        }

        let backup = self.db().get_mpq_backup(repo_id, &entry.path)?;
        let mut restored_from = None;
        if let Some(backup) = &backup {
            let backup_path = Self::resolve_install_path(&backup.backup_path, Some(wow_dir))
                .ok_or_else(|| anyhow::anyhow!("Could not resolve the MPQ backup"))?;
            if backup_path.is_file() {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(&backup_path, &target)?;
                restored_from = Some(backup_path);
                diagnostics::emit(
                    diagnostics::DiagnosticLevel::Trace,
                    "engine.mpq",
                    format!(
                        "displaced MPQ restored from backup during component removal: repo_id={repo_id}; paths omitted"
                    ),
                );
            }
        }
        let restored_backup = restored_from.is_some();

        if let Err(error) = self
            .db()
            .remove_mpq_install_and_backup(repo_id, &entry.path)
        {
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Debug,
                "engine.mpq",
                format!(
                    "tracked MPQ removal metadata commit failed; filesystem rollback started: repo_id={repo_id}"
                ),
            );
            if let Some(backup_path) = restored_from {
                let _ = fs::rename(&target, backup_path);
            }
            if had_target {
                let _ = fs::rename(&removed_copy, &actual);
            }
            return Err(error);
        }

        if self
            .db()
            .list_installs(repo_id)?
            .into_iter()
            .all(|entry| entry.kind != "mpq")
        {
            self.db().remove_repo(repo_id)?;
        }
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "tracked MPQ component removal committed: repo_id={repo_id}; managed_file_removed={had_target}; displaced_backup_restored={restored_backup}; metadata_updated=true"
            ),
        );
        Ok(had_target || restored_backup)
    }

    pub fn protect_modified_mpq(
        &self,
        repo_id: i64,
        manifest_path: &str,
        wow_dir: &Path,
    ) -> Result<()> {
        let _diagnostic = diagnostics::OperationGuard::new("protect_modified_mpq");
        let entry = self
            .db()
            .list_installs(repo_id)?
            .into_iter()
            .find(|entry| entry.kind == "mpq" && entry.path.eq_ignore_ascii_case(manifest_path))
            .ok_or_else(|| anyhow::anyhow!("Tracked MPQ not found"))?;
        let path = Self::resolve_install_path(&entry.path, Some(wow_dir))
            .ok_or_else(|| anyhow::anyhow!("Could not resolve the tracked MPQ path"))?;
        let actual = Self::find_actual_case(&path).unwrap_or(path);
        if !actual.is_file() {
            anyhow::bail!("The modified MPQ is no longer present");
        }
        let metadata = fs::symlink_metadata(&actual)
            .map_err(|_| MpqError::Filesystem("reading MPQ metadata"))?;
        let fingerprint = metadata_fingerprint(&metadata);
        self.db()
            .upsert_mpq_protection(&entry.path, &fingerprint, false)?;
        self.db()
            .set_mpq_protection(&entry.path, &fingerprint, true)?;
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!("modified tracked MPQ protection committed: repo_id={repo_id}; path omitted"),
        );
        Ok(())
    }

    pub fn remove_mpq_package(
        &self,
        repo_id: i64,
        wow_dir: &Path,
        force_modified: bool,
    ) -> Result<usize> {
        let _diagnostic = diagnostics::OperationGuard::new("remove_mpq_package");
        let entries = self
            .db()
            .list_installs(repo_id)?
            .into_iter()
            .filter(|entry| entry.kind == "mpq")
            .collect::<Vec<_>>();
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "MPQ package removal started: repo_id={repo_id}; component_count={}; force_modified={force_modified}",
                entries.len()
            ),
        );
        // Preflight lightweight file identities before mutating any file.
        if !force_modified {
            for entry in &entries {
                if let (Some(expected), Some(full)) = (
                    entry.file_fingerprint.as_deref(),
                    Self::resolve_install_path(&entry.path, Some(wow_dir)),
                ) {
                    let actual = Self::find_actual_case(&full).unwrap_or(full);
                    let changed = actual.is_file()
                        && fs::symlink_metadata(&actual)
                            .map(|metadata| metadata_fingerprint(&metadata) != expected)
                            .unwrap_or(true);
                    if changed {
                        let name = Path::new(&entry.path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("MPQ")
                            .to_string();
                        anyhow::bail!(MpqError::ModifiedTarget(name));
                    }
                }
            }
        }

        let rollback_parent = util::cache_dir(Some(wow_dir))?.join("mpq-staging");
        fs::create_dir_all(&rollback_parent)?;
        let rollback_dir = Builder::new()
            .prefix("remove-package-")
            .tempdir_in(&rollback_parent)?;
        struct RemovalRollback {
            target: PathBuf,
            actual: PathBuf,
            managed_copy: Option<PathBuf>,
            backup_path: Option<PathBuf>,
            restored_backup: bool,
        }
        let mut rollback = Vec::<RemovalRollback>::new();
        let mut removed = 0usize;

        let filesystem_result = (|| -> Result<()> {
            for (index, entry) in entries.iter().enumerate() {
                let target = Self::resolve_install_path(&entry.path, Some(wow_dir))
                    .ok_or_else(|| anyhow::anyhow!("Could not resolve the tracked MPQ path"))?;
                let actual = Self::find_actual_case(&target).unwrap_or(target.clone());
                let managed_copy = if actual.is_file() {
                    let saved = rollback_dir.path().join(format!("managed-{index}.mpq"));
                    fs::rename(&actual, &saved)?;
                    removed += 1;
                    diagnostics::emit(
                        diagnostics::DiagnosticLevel::Trace,
                        "engine.mpq",
                        format!(
                            "MPQ package component moved into rollback staging: repo_id={repo_id}; component={}/{}; paths omitted",
                            index + 1,
                            entries.len()
                        ),
                    );
                    Some(saved)
                } else {
                    None
                };
                let backup_path =
                    self.db()
                        .get_mpq_backup(repo_id, &entry.path)?
                        .and_then(|backup| {
                            Self::resolve_install_path(&backup.backup_path, Some(wow_dir))
                        });
                rollback.push(RemovalRollback {
                    target: target.clone(),
                    actual,
                    managed_copy,
                    backup_path: backup_path.clone(),
                    restored_backup: false,
                });
                if let Some(backup) = backup_path.filter(|path| path.is_file()) {
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::rename(&backup, &target)?;
                    rollback.last_mut().expect("just pushed").restored_backup = true;
                    diagnostics::emit(
                        diagnostics::DiagnosticLevel::Trace,
                        "engine.mpq",
                        format!(
                            "MPQ package displaced backup restored: repo_id={repo_id}; component={}/{}; paths omitted",
                            index + 1,
                            entries.len()
                        ),
                    );
                }
            }
            Ok(())
        })();

        let rollback_files = |items: &[RemovalRollback]| {
            for item in items.iter().rev() {
                if item.restored_backup {
                    if let Some(backup_path) = &item.backup_path {
                        let _ = fs::rename(&item.target, backup_path);
                    }
                }
                if let Some(managed_copy) = &item.managed_copy {
                    let _ = fs::rename(managed_copy, &item.actual);
                }
            }
        };
        if let Err(error) = filesystem_result {
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Debug,
                "engine.mpq",
                format!(
                    "MPQ package filesystem removal failed; rollback started: repo_id={repo_id}"
                ),
            );
            rollback_files(&rollback);
            return Err(error);
        }
        if let Err(error) = self.db().remove_repo(repo_id) {
            diagnostics::emit(
                diagnostics::DiagnosticLevel::Debug,
                "engine.mpq",
                format!(
                    "MPQ package metadata removal failed; filesystem rollback started: repo_id={repo_id}"
                ),
            );
            rollback_files(&rollback);
            return Err(error);
        }
        diagnostics::emit(
            diagnostics::DiagnosticLevel::Debug,
            "engine.mpq",
            format!(
                "MPQ package removal committed: repo_id={repo_id}; managed_files_removed={removed}; backup_restorations={}; metadata_removed=true",
                rollback.iter().filter(|item| item.restored_backup).count()
            ),
        );
        Ok(removed)
    }

    pub fn record_repo_dependency(
        &self,
        parent_repo_id: i64,
        child_repo_id: i64,
        relationship: &str,
    ) -> Result<()> {
        self.db()
            .add_repo_dependency(parent_repo_id, child_repo_id, relationship)
    }

    pub fn repo_dependencies(&self, parent_repo_id: i64) -> Result<Vec<(i64, String)>> {
        self.db().list_repo_dependencies(parent_repo_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_valid_mpq(path: &Path) {
        let mut file = fs::File::create(path).unwrap();
        file.write_all(MPQ_HEADER).unwrap();
        file.write_all(&MPQ_MIN_HEADER_SIZE.to_le_bytes()).unwrap();
        file.write_all(&[0u8; 64]).unwrap();
    }

    fn write_valid_mpq_variant(path: &Path, marker: u8) {
        let mut file = fs::File::create(path).unwrap();
        file.write_all(MPQ_HEADER).unwrap();
        file.write_all(&MPQ_MIN_HEADER_SIZE.to_le_bytes()).unwrap();
        file.write_all(&[marker; 64]).unwrap();
    }

    #[test]
    fn detects_config_and_data_locale() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("WTF")).unwrap();
        fs::create_dir_all(temp.path().join("Data/enUS")).unwrap();
        fs::write(temp.path().join("WTF/Config.wtf"), "SET locale \"enUS\"\n").unwrap();

        let detection = detect_wow_locale(temp.path());
        assert_eq!(detection.recommended.as_deref(), Some("enUS"));
        assert!(detection.candidates.iter().any(|locale| locale == "enUS"));
    }

    #[test]
    fn reports_ambiguous_locales() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("Data/enUS")).unwrap();
        fs::create_dir_all(temp.path().join("Data/deDE")).unwrap();

        let detection = detect_wow_locale(temp.path());
        assert!(detection.ambiguous);
        assert_eq!(detection.recommended, None);
    }

    #[test]
    fn validates_standard_and_aligned_headers() {
        let temp = tempfile::tempdir().unwrap();
        let standard = temp.path().join("patch-X.MPQ");
        write_valid_mpq(&standard);
        assert!(validate_mpq_file(&standard).is_ok());

        let aligned = temp.path().join("patch-Y.MPQ");
        let mut file = fs::File::create(&aligned).unwrap();
        file.write_all(&vec![0u8; MPQ_HEADER_ALIGNMENT as usize])
            .unwrap();
        file.write_all(MPQ_HEADER).unwrap();
        file.write_all(&MPQ_MIN_HEADER_SIZE.to_le_bytes()).unwrap();
        file.write_all(&[0u8; 64]).unwrap();
        assert!(validate_mpq_file(&aligned).is_ok());

        let userdata = temp.path().join("patch-U.MPQ");
        let mut file = fs::File::create(&userdata).unwrap();
        file.write_all(MPQ_USER_DATA).unwrap();
        file.write_all(&64u32.to_le_bytes()).unwrap();
        file.write_all(&(MPQ_HEADER_ALIGNMENT as u32).to_le_bytes())
            .unwrap();
        file.write_all(&vec![0u8; MPQ_HEADER_ALIGNMENT as usize - 12])
            .unwrap();
        file.write_all(MPQ_HEADER).unwrap();
        file.write_all(&MPQ_MIN_HEADER_SIZE.to_le_bytes()).unwrap();
        file.write_all(&[0u8; 64]).unwrap();
        assert!(validate_mpq_file(&userdata).is_ok());
    }

    #[test]
    fn rejects_corrupt_mpqs() {
        let temp = tempfile::tempdir().unwrap();
        let corrupt = temp.path().join("patch-X.MPQ");
        fs::write(&corrupt, vec![0u8; 128]).unwrap();
        assert!(validate_mpq_file(&corrupt).is_err());
    }

    #[test]
    fn reserves_core_names_but_not_lettered_custom_patches() {
        assert!(is_reserved_core_filename("common.MPQ"));
        assert!(is_reserved_core_filename("base-enUS.MPQ"));
        assert!(is_reserved_core_filename("backup-enUS.MPQ"));
        assert!(is_reserved_core_filename("patch-2.MPQ"));
        assert!(is_reserved_core_filename("patch-enUS-3.MPQ"));
        assert!(!is_reserved_core_filename("patch-enUS-M.MPQ"));
        assert!(!is_reserved_core_filename("patch-F.MPQ"));
    }

    #[test]
    fn core_classification_can_be_overridden_and_resets_after_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        let core = wow.join("Data/common.MPQ");
        write_valid_mpq(&core);
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();

        let initial = engine.list_mpq_protection(&wow).unwrap();
        assert!(initial[0].core);
        assert!(initial[0].protected);

        engine
            .set_mpq_core_classification(&wow, "Data/common.MPQ", false)
            .unwrap();
        engine
            .set_mpq_protected(&wow, "Data/common.MPQ", false)
            .unwrap();
        let overridden = engine.list_mpq_protection(&wow).unwrap();
        assert!(!overridden[0].core);
        assert!(!overridden[0].protected);

        let source = temp.path().join("replacement.MPQ");
        write_valid_mpq_variant(&source, 8);
        let selection = MpqInstallSelection {
            source_key: "replacement.MPQ".to_string(),
            display_name: "Core-name override".to_string(),
            file_name: "common.MPQ".to_string(),
            destination: MpqDestination::DataRoot,
            replace_unprotected: true,
            version: None,
        };
        assert_eq!(
            engine
                .preview_local_mpq_targets(&wow, &source, &[selection])
                .unwrap()[0]
                .status,
            MpqTargetStatus::UnprotectedReplacement
        );

        fs::remove_file(&core).unwrap();
        write_valid_mpq_variant(&core, 9);
        let replaced = engine.list_mpq_protection(&wow).unwrap();
        assert!(replaced[0].core);
        assert!(replaced[0].protected);
    }

    #[test]
    fn untracked_mpq_enabled_state_requires_editor_unlock() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        let archive = wow.join("Data/patch-Manual.MPQ");
        write_valid_mpq(&archive);
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();

        let initial = engine.list_mpq_protection(&wow).unwrap();
        assert_eq!(initial.len(), 1);
        assert!(initial[0].enabled);
        assert!(initial[0].protected);
        engine
            .rename_untracked_mpq_display_name(&wow, &initial[0].path, "My manual patch", false)
            .unwrap();
        assert!(engine
            .set_untracked_mpq_enabled(&wow, &initial[0].path, false)
            .is_err());
        assert!(archive.is_file());
        engine
            .set_untracked_mpq_editor_unlocked(&wow, &initial[0].path, true)
            .unwrap();
        engine
            .set_untracked_mpq_enabled(&wow, &initial[0].path, false)
            .unwrap();
        assert!(!archive.exists());
        assert!(wow.join("Data/patch-Manual.MPQ.disabled").is_file());

        let disabled = engine.list_mpq_protection(&wow).unwrap();
        assert_eq!(disabled.len(), 1);
        assert!(!disabled[0].enabled);
        assert!(disabled[0].protected);
        assert!(disabled[0].editor_unlocked);
        assert_eq!(disabled[0].display_name.as_deref(), Some("My manual patch"));
        engine
            .set_untracked_mpq_enabled(&wow, &disabled[0].path, true)
            .unwrap();
        assert!(archive.is_file());
        assert!(!wow.join("Data/patch-Manual.MPQ.disabled").exists());
    }

    #[test]
    fn unlocked_untracked_mpq_can_be_renamed_without_losing_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        let original = wow.join("Data/patch-Manual.MPQ");
        write_valid_mpq(&original);
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();

        let initial = engine.list_mpq_protection(&wow).unwrap();
        engine
            .rename_untracked_mpq_display_name(&wow, &initial[0].path, "HD models", false)
            .unwrap();
        assert!(engine
            .rename_untracked_mpq_file(&wow, &initial[0].path, "patch-HD.MPQ")
            .is_err());

        engine
            .set_mpq_protected(&wow, &initial[0].path, false)
            .unwrap();
        let renamed = engine
            .rename_untracked_mpq_file(&wow, &initial[0].path, "patch-HD.MPQ")
            .unwrap();
        assert_eq!(renamed, "Data/patch-HD.MPQ");
        assert!(!original.exists());
        assert!(wow.join("Data/patch-HD.MPQ").is_file());

        let entries = engine.list_mpq_protection(&wow).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "Data/patch-HD.MPQ");
        assert_eq!(entries[0].display_name.as_deref(), Some("HD models"));
        assert!(!entries[0].protected);
        assert!(!entries[0].core);
    }

    #[test]
    fn mpq_editor_updates_properties_without_changing_the_lock_state() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        write_valid_mpq(&wow.join("Data/patch-Manual.MPQ"));
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();

        let initial = engine.list_mpq_protection(&wow).unwrap();
        engine
            .unlock_untracked_mpq_for_editing(&wow, &initial[0].path)
            .unwrap();
        let renamed = engine
            .edit_untracked_mpq(
                &wow,
                &initial[0].path,
                "Base game archive",
                "common.MPQ",
                &MpqDestination::DataRoot,
                true,
                false,
            )
            .unwrap();

        assert_eq!(renamed, "Data/common.MPQ");
        let edited = engine.list_mpq_protection(&wow).unwrap();
        assert_eq!(edited[0].display_name.as_deref(), Some("Base game archive"));
        assert!(edited[0].core);
        assert!(edited[0].protected);
        assert!(edited[0].editor_unlocked);
        assert!(wow.join("Data/common.MPQ").is_file());
    }

    #[test]
    fn unlocking_a_core_mpq_preserves_its_classification() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        write_valid_mpq(&wow.join("Data/common.MPQ"));
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();

        let initial = engine.list_mpq_protection(&wow).unwrap();
        engine
            .unlock_untracked_mpq_for_editing(&wow, &initial[0].path)
            .unwrap();
        let unlocked = engine.list_mpq_protection(&wow).unwrap();
        assert!(unlocked[0].core);
        assert!(unlocked[0].protected);
        assert!(unlocked[0].editor_unlocked);
        engine
            .set_untracked_mpq_editor_unlocked(&wow, &unlocked[0].path, false)
            .unwrap();
        let relocked = engine.list_mpq_protection(&wow).unwrap();
        assert!(relocked[0].core);
        assert!(relocked[0].protected);
        assert!(!relocked[0].editor_unlocked);
    }

    #[test]
    fn tracked_editor_moves_the_file_without_losing_package_ownership() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data/enUS")).unwrap();
        let source = temp.path().join("source.MPQ");
        write_valid_mpq(&source);
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();
        let selection = MpqInstallSelection {
            source_key: "source.MPQ".to_string(),
            display_name: "Original name".to_string(),
            file_name: "patch-X.MPQ".to_string(),
            destination: MpqDestination::DataRoot,
            replace_unprotected: false,
            version: None,
        };
        let repo_id = engine
            .install_local_mpq_package(&wow, &source, &[selection], false)
            .unwrap();
        engine
            .set_tracked_mpq_editor_unlocked(repo_id, &wow, "Data/patch-X.MPQ", true)
            .unwrap();

        let path = engine
            .edit_tracked_mpq(
                repo_id,
                &wow,
                "Data/patch-X.MPQ",
                "Moved patch",
                "patch-enUS-X.MPQ",
                &MpqDestination::Locale("enUS".to_string()),
                false,
            )
            .unwrap();
        assert_eq!(path, "Data/enUS/patch-enUS-X.MPQ");
        assert!(!wow.join("Data/patch-X.MPQ").exists());
        assert!(wow.join(&path).is_file());
        let installed = engine.list_installed_mpqs(repo_id, &wow).unwrap();
        assert_eq!(installed[0].path, path);
        assert_eq!(installed[0].display_name, "Moved patch");
        engine.remove_mpq_package(repo_id, &wow, false).unwrap();
        assert!(!wow.join("Data/enUS/patch-enUS-X.MPQ").exists());
    }

    #[test]
    fn renaming_a_disabled_untracked_mpq_preserves_its_enabled_state() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        write_valid_mpq(&wow.join("Data/patch-Manual.MPQ"));
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();

        let initial = engine.list_mpq_protection(&wow).unwrap();
        engine
            .set_mpq_protected(&wow, &initial[0].path, false)
            .unwrap();
        engine
            .set_untracked_mpq_editor_unlocked(&wow, &initial[0].path, true)
            .unwrap();
        engine
            .set_untracked_mpq_enabled(&wow, &initial[0].path, false)
            .unwrap();
        let disabled = engine.list_mpq_protection(&wow).unwrap();
        let renamed = engine
            .rename_untracked_mpq_file(&wow, &disabled[0].path, "patch-HD.MPQ")
            .unwrap();

        assert_eq!(renamed, "Data/patch-HD.MPQ.disabled");
        assert!(wow.join("Data/patch-HD.MPQ.disabled").is_file());
        let entries = engine.list_mpq_protection(&wow).unwrap();
        assert!(!entries[0].enabled);
        assert!(!entries[0].protected);
    }

    #[test]
    fn disabled_core_mpq_is_still_detected_as_core() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        write_valid_mpq(&wow.join("Data/common.MPQ.disabled"));

        let scanned = scan_existing_mpqs(&wow).unwrap();
        assert_eq!(scanned.len(), 1);
        assert!(scanned[0].core);
        assert!(!scanned[0].enabled);
    }

    #[cfg(unix)]
    #[test]
    fn protection_scan_does_not_open_mpq_contents() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        let archive = wow.join("Data/common.MPQ");
        write_valid_mpq(&archive);
        fs::set_permissions(&archive, fs::Permissions::from_mode(0o000)).unwrap();

        let scanned = scan_existing_mpqs(&wow).unwrap();
        fs::set_permissions(&archive, fs::Permissions::from_mode(0o600)).unwrap();

        assert_eq!(scanned.len(), 1);
        assert!(scanned[0].core);
        assert!(!scanned[0].fingerprint.is_empty());
    }

    #[test]
    fn rejects_cross_platform_unsafe_target_names() {
        for name in [
            "../patch-X.MPQ",
            "patch:X.MPQ",
            "patch?.MPQ",
            "patch-X.MPQ.",
        ] {
            assert!(validate_target_file_name(name).is_err(), "accepted {name}");
        }
    }

    #[test]
    fn engine_installs_labels_detects_modification_and_removes() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data/enUS")).unwrap();
        let source = temp.path().join("source.MPQ");
        write_valid_mpq_variant(&source, 1);
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();
        let selection = MpqInstallSelection {
            source_key: "source.MPQ".to_string(),
            display_name: "My Map Patch".to_string(),
            file_name: "patch-enUS-X.MPQ".to_string(),
            destination: MpqDestination::Locale("enUS".to_string()),
            replace_unprotected: false,
            version: None,
        };

        let repo_id = engine
            .install_local_mpq_package(&wow, &source, std::slice::from_ref(&selection), false)
            .unwrap();
        let installed = engine.list_installed_mpqs(repo_id, &wow).unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].display_name, "My Map Patch");
        assert_eq!(installed[0].status, MpqFileStatus::Installed);
        assert!(installed[0].editor_unlocked);

        fs::write(
            wow.join("Data/enUS/patch-enUS-X.MPQ"),
            b"changed externally",
        )
        .unwrap();
        assert_eq!(
            engine.list_installed_mpqs(repo_id, &wow).unwrap()[0].status,
            MpqFileStatus::Modified
        );
        assert!(engine
            .remove_mpq_component(repo_id, "Data/enUS/patch-enUS-X.MPQ", &wow, false)
            .is_err());
        engine
            .protect_modified_mpq(repo_id, "Data/enUS/patch-enUS-X.MPQ", &wow)
            .unwrap();
        assert!(engine
            .install_local_mpq_package(&wow, &source, &[selection], false)
            .is_err());
        engine
            .set_tracked_mpq_protected(repo_id, &wow, "Data/enUS/patch-enUS-X.MPQ", false)
            .unwrap();
        engine
            .set_tracked_mpq_editor_unlocked(repo_id, &wow, "Data/enUS/patch-enUS-X.MPQ", true)
            .unwrap();
        engine
            .remove_mpq_component(repo_id, "Data/enUS/patch-enUS-X.MPQ", &wow, true)
            .unwrap();
        assert!(!wow.join("Data/enUS/patch-enUS-X.MPQ").exists());
    }

    #[test]
    fn listing_legacy_mpq_without_fingerprint_is_read_only() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        let source = temp.path().join("source.MPQ");
        write_valid_mpq(&source);
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();
        let selection = MpqInstallSelection {
            source_key: "source.MPQ".to_string(),
            display_name: "Legacy patch".to_string(),
            file_name: "patch-L.MPQ".to_string(),
            destination: MpqDestination::DataRoot,
            replace_unprotected: false,
            version: None,
        };
        let repo_id = engine
            .install_local_mpq_package(&wow, &source, &[selection], false)
            .unwrap();

        // Recreate the manifest in the pre-fingerprint form. Listing this row
        // used to attempt a nested DB lock and leave startup stuck forever.
        engine
            .db()
            .add_named_install_with_hash(
                repo_id,
                "Data/patch-L.MPQ",
                "mpq",
                None,
                None,
                Some("Legacy patch"),
            )
            .unwrap();

        let listed = engine.list_installed_mpqs(repo_id, &wow).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, MpqFileStatus::Installed);
        let manifest = engine.db().list_installs(repo_id).unwrap();
        assert_eq!(manifest[0].file_fingerprint, None);
    }

    #[test]
    fn unprotected_replacement_is_backed_up_and_restored() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        let existing = wow.join("Data/patch-Custom.MPQ");
        write_valid_mpq_variant(&existing, 2);
        let original = fs::read(&existing).unwrap();
        let source = temp.path().join("replacement.MPQ");
        write_valid_mpq_variant(&source, 3);
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();

        let protected = engine.list_mpq_protection(&wow).unwrap();
        assert!(protected[0].protected);
        engine
            .set_mpq_protected(&wow, "Data/patch-Custom.MPQ", false)
            .unwrap();
        let selection = MpqInstallSelection {
            source_key: "replacement.MPQ".to_string(),
            display_name: "Replacement".to_string(),
            file_name: "patch-Custom.MPQ".to_string(),
            destination: MpqDestination::DataRoot,
            replace_unprotected: true,
            version: None,
        };
        assert_eq!(
            engine
                .preview_local_mpq_targets(&wow, &source, std::slice::from_ref(&selection))
                .unwrap()[0]
                .status,
            MpqTargetStatus::UnprotectedReplacement
        );
        let repo_id = engine
            .install_local_mpq_package(&wow, &source, &[selection], false)
            .unwrap();
        assert_ne!(fs::read(&existing).unwrap(), original);
        engine.remove_mpq_package(repo_id, &wow, false).unwrap();
        assert_eq!(fs::read(&existing).unwrap(), original);
    }

    #[test]
    fn moving_a_replacement_restores_the_displaced_file_at_its_original_path() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data/enUS")).unwrap();
        let existing = wow.join("Data/patch-Custom.MPQ");
        write_valid_mpq_variant(&existing, 2);
        let original = fs::read(&existing).unwrap();
        let source = temp.path().join("replacement.MPQ");
        write_valid_mpq_variant(&source, 3);
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();
        engine.list_mpq_protection(&wow).unwrap();
        engine
            .set_mpq_protected(&wow, "Data/patch-Custom.MPQ", false)
            .unwrap();
        let selection = MpqInstallSelection {
            source_key: "replacement.MPQ".to_string(),
            display_name: "Replacement".to_string(),
            file_name: "patch-Custom.MPQ".to_string(),
            destination: MpqDestination::DataRoot,
            replace_unprotected: true,
            version: None,
        };
        let repo_id = engine
            .install_local_mpq_package(&wow, &source, &[selection], false)
            .unwrap();
        engine
            .set_tracked_mpq_editor_unlocked(repo_id, &wow, "Data/patch-Custom.MPQ", true)
            .unwrap();

        engine
            .edit_tracked_mpq(
                repo_id,
                &wow,
                "Data/patch-Custom.MPQ",
                "Moved replacement",
                "patch-enUS-X.MPQ",
                &MpqDestination::Locale("enUS".to_string()),
                false,
            )
            .unwrap();
        assert_eq!(fs::read(&existing).unwrap(), original);
        assert!(wow.join("Data/enUS/patch-enUS-X.MPQ").is_file());
        engine.remove_mpq_package(repo_id, &wow, false).unwrap();
        assert_eq!(fs::read(&existing).unwrap(), original);
        assert!(!wow.join("Data/enUS/patch-enUS-X.MPQ").exists());
    }

    #[test]
    fn disabling_and_enabling_renames_the_tracked_mpq() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        let source = temp.path().join("toggle.MPQ");
        write_valid_mpq_variant(&source, 4);
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();
        let selection = MpqInstallSelection {
            source_key: "toggle.MPQ".to_string(),
            display_name: "Toggle patch".to_string(),
            file_name: "patch-Toggle.MPQ".to_string(),
            destination: MpqDestination::DataRoot,
            replace_unprotected: false,
            version: None,
        };
        let repo_id = engine
            .install_local_mpq_package(&wow, &source, &[selection], false)
            .unwrap();

        assert_eq!(
            engine.set_mpq_enabled(repo_id, None, false, &wow).unwrap(),
            1
        );
        assert!(!wow.join("Data/patch-Toggle.MPQ").exists());
        assert!(wow.join("Data/patch-Toggle.MPQ.disabled").is_file());
        let disabled = engine.list_installed_mpqs(repo_id, &wow).unwrap();
        assert!(!disabled[0].enabled);
        assert_eq!(disabled[0].path, "Data/patch-Toggle.MPQ.disabled");
        assert!(!engine.db().get_repo(repo_id).unwrap().enabled);

        assert_eq!(
            engine.set_mpq_enabled(repo_id, None, true, &wow).unwrap(),
            1
        );
        assert!(wow.join("Data/patch-Toggle.MPQ").is_file());
        assert!(!wow.join("Data/patch-Toggle.MPQ.disabled").exists());
        assert!(engine.list_installed_mpqs(repo_id, &wow).unwrap()[0].enabled);
        assert!(engine.db().get_repo(repo_id).unwrap().enabled);
    }

    #[test]
    fn disabling_a_replacement_restores_and_reprotects_its_backup() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        let target = wow.join("Data/patch-Custom.MPQ");
        write_valid_mpq_variant(&target, 5);
        let original = fs::read(&target).unwrap();
        let source = temp.path().join("managed.MPQ");
        write_valid_mpq_variant(&source, 6);
        let managed = fs::read(&source).unwrap();
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();
        engine.list_mpq_protection(&wow).unwrap();
        engine
            .set_mpq_protected(&wow, "Data/patch-Custom.MPQ", false)
            .unwrap();
        let selection = MpqInstallSelection {
            source_key: "managed.MPQ".to_string(),
            display_name: "Managed replacement".to_string(),
            file_name: "patch-Custom.MPQ".to_string(),
            destination: MpqDestination::DataRoot,
            replace_unprotected: true,
            version: None,
        };
        let repo_id = engine
            .install_local_mpq_package(&wow, &source, &[selection], false)
            .unwrap();

        engine.set_mpq_enabled(repo_id, None, false, &wow).unwrap();
        assert_eq!(fs::read(&target).unwrap(), original);
        assert_eq!(
            fs::read(wow.join("Data/patch-Custom.MPQ.disabled")).unwrap(),
            managed
        );

        fs::write(&target, b"changed while disabled").unwrap();
        assert!(engine.set_mpq_enabled(repo_id, None, true, &wow).is_err());
        assert!(wow.join("Data/patch-Custom.MPQ.disabled").is_file());
        fs::write(&target, &original).unwrap();
        engine
            .set_mpq_protected(&wow, "Data/patch-Custom.MPQ", false)
            .unwrap();

        engine.set_mpq_enabled(repo_id, None, true, &wow).unwrap();
        assert_eq!(fs::read(&target).unwrap(), managed);
        engine.remove_mpq_package(repo_id, &wow, false).unwrap();
        assert_eq!(fs::read(&target).unwrap(), original);
    }

    #[test]
    fn reinstall_with_a_new_filename_removes_the_old_component() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data")).unwrap();
        let source = temp.path().join("rename.MPQ");
        write_valid_mpq(&source);
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();
        let selection = |name: &str| MpqInstallSelection {
            source_key: "rename.MPQ".to_string(),
            display_name: "Renamed patch".to_string(),
            file_name: name.to_string(),
            destination: MpqDestination::DataRoot,
            replace_unprotected: false,
            version: None,
        };
        let first = engine
            .install_local_mpq_package(&wow, &source, &[selection("patch-A.MPQ")], false)
            .unwrap();
        let second = engine
            .install_local_mpq_package(&wow, &source, &[selection("patch-B.MPQ")], false)
            .unwrap();
        assert_eq!(first, second);
        assert!(!wow.join("Data/patch-A.MPQ").exists());
        assert!(wow.join("Data/patch-B.MPQ").is_file());
        assert_eq!(engine.list_installed_mpqs(second, &wow).unwrap().len(), 1);
    }

    #[test]
    fn inspects_standalone_mpq_and_suggests_locale() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("Data/enUS")).unwrap();
        let source = temp.path().join("patch-enUS-M.MPQ");
        write_valid_mpq(&source);

        let inspection = inspect_local_source(temp.path(), &source).unwrap();
        assert_eq!(inspection.candidates.len(), 1);
        assert_eq!(
            inspection.candidates[0].suggested_destination,
            MpqDestination::Locale("enUS".into())
        );
    }

    #[test]
    fn generic_mpq_defaults_to_data_even_when_the_client_locale_is_known() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("Data/enUS")).unwrap();
        let source = temp.path().join("patch-A.MPQ");
        write_valid_mpq(&source);

        let inspection = inspect_local_source(temp.path(), &source).unwrap();
        assert_eq!(
            inspection.candidates[0].suggested_destination,
            MpqDestination::DataRoot
        );
    }

    #[test]
    fn discovers_multiple_mpqs_in_zip() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("Data/enUS")).unwrap();
        let archive_path = temp.path().join("patches.zip");
        let archive_file = fs::File::create(&archive_path).unwrap();
        let mut archive = zip::ZipWriter::new(archive_file);
        let options = zip::write::SimpleFileOptions::default();
        for name in ["Data/enUS/patch-enUS-M.MPQ", "Data/patch-F.MPQ"] {
            archive.start_file(name, options).unwrap();
            archive.write_all(MPQ_HEADER).unwrap();
            archive
                .write_all(&MPQ_MIN_HEADER_SIZE.to_le_bytes())
                .unwrap();
            archive.write_all(&[0u8; 64]).unwrap();
        }
        archive.start_file("readme.txt", options).unwrap();
        archive.write_all(b"ignored").unwrap();
        archive.finish().unwrap();

        let inspection = inspect_local_source(temp.path(), &archive_path).unwrap();
        assert_eq!(inspection.candidates.len(), 2);
        assert_eq!(
            inspection
                .candidates
                .iter()
                .find(|candidate| candidate.original_file_name == "patch-enUS-M.MPQ")
                .unwrap()
                .suggested_destination,
            MpqDestination::Locale("enUS".into())
        );
        assert_eq!(
            inspection
                .candidates
                .iter()
                .find(|candidate| candidate.original_file_name == "patch-F.MPQ")
                .unwrap()
                .suggested_destination,
            MpqDestination::DataRoot
        );
        assert!(scan_existing_mpqs(temp.path()).unwrap().is_empty());
    }

    #[test]
    fn rejects_traversing_zip_entries_before_extraction() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(&wow).unwrap();
        let archive_path = temp.path().join("unsafe.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file::<_, ()>("../escape.MPQ", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(MPQ_HEADER).unwrap();
        zip.write_all(&MPQ_MIN_HEADER_SIZE.to_le_bytes()).unwrap();
        zip.write_all(&[0u8; 64]).unwrap();
        zip.finish().unwrap();

        assert!(inspect_local_source(&wow, &archive_path).is_err());
        assert!(!temp.path().join("escape.MPQ").exists());
    }

    #[test]
    fn local_package_label_hides_collision_safe_identity_suffix() {
        assert_eq!(
            crate::Engine::local_mpq_display_name_from_identity("Darker_Nights-a981398b"),
            "Darker_Nights"
        );
        assert_eq!(
            crate::Engine::local_mpq_display_name_from_identity("patch-custom"),
            "patch-custom"
        );
    }

    #[test]
    fn edits_a_local_mpq_package_and_all_components_transactionally() {
        let temp = tempfile::tempdir().unwrap();
        let wow = temp.path().join("wow");
        fs::create_dir_all(wow.join("Data/enUS")).unwrap();
        let archive_path = temp.path().join("Darker_Nights.zip");
        let archive_file = fs::File::create(&archive_path).unwrap();
        let mut archive = zip::ZipWriter::new(archive_file);
        let options = zip::write::SimpleFileOptions::default();
        for name in ["Darker Nights 50.MPQ", "Darker Nights 100.MPQ"] {
            archive.start_file(name, options).unwrap();
            archive.write_all(MPQ_HEADER).unwrap();
            archive
                .write_all(&MPQ_MIN_HEADER_SIZE.to_le_bytes())
                .unwrap();
            archive.write_all(&[0u8; 64]).unwrap();
        }
        archive.finish().unwrap();

        let inspection = inspect_local_source(&wow, &archive_path).unwrap();
        let selections = inspection
            .candidates
            .iter()
            .map(|candidate| MpqInstallSelection {
                source_key: candidate.source_key.clone(),
                display_name: candidate.suggested_display_name.clone(),
                file_name: candidate.original_file_name.clone(),
                destination: MpqDestination::DataRoot,
                replace_unprotected: false,
                version: None,
            })
            .collect::<Vec<_>>();
        let engine = crate::Engine::open(&temp.path().join("profile.sqlite3")).unwrap();
        let repo_id = engine
            .install_local_mpq_package(&wow, &archive_path, &selections, false)
            .unwrap();
        assert_eq!(
            engine.mpq_package_display_name(repo_id).unwrap(),
            "Darker_Nights"
        );

        let files = engine.list_installed_mpqs(repo_id, &wow).unwrap();
        let edits = files
            .iter()
            .enumerate()
            .map(|(index, file)| MpqPackageFileEdit {
                path: file.path.clone(),
                display_name: format!("Darkness {}", index + 1),
                file_name: format!("patch-Dark-{}.MPQ", index + 1),
                destination: if index == 0 {
                    MpqDestination::Locale("enUS".to_string())
                } else {
                    MpqDestination::DataRoot
                },
                enabled: index != 0,
            })
            .collect::<Vec<_>>();
        engine
            .edit_tracked_mpq_package(repo_id, &wow, "Darker Nights", &edits, false)
            .unwrap();

        assert_eq!(
            engine.mpq_package_display_name(repo_id).unwrap(),
            "Darker Nights"
        );
        let edited = engine.list_installed_mpqs(repo_id, &wow).unwrap();
        assert_eq!(edited.len(), 2);
        assert!(edited.iter().any(|file| {
            file.path == "Data/enUS/patch-Dark-1.MPQ.disabled"
                && file.display_name == "Darkness 1"
                && !file.enabled
        }));
        assert!(edited.iter().any(|file| {
            file.path == "Data/patch-Dark-2.MPQ"
                && file.display_name == "Darkness 2"
                && file.enabled
        }));
        assert!(wow.join("Data/enUS/patch-Dark-1.MPQ.disabled").is_file());
        assert!(wow.join("Data/patch-Dark-2.MPQ").is_file());
    }
}
