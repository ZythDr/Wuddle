use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InstallMode {
    /// Automatically infer what to install from the downloaded asset:
    /// - If it's a .dll => copy into WoW root
    /// - If it's a .zip => extract, then:
    ///   - copy any *.dll into WoW root
    ///   - copy any addon folder(s) into Interface/AddOns/, renaming folder to match the .toc stem exactly
    Auto,
    Addon,
    /// Track addon directly from its Git repository using clone/fetch/pull.
    /// Synced into hidden staging under WoW/Interface/AddOns/.wuddle/,
    /// then addon folders are deployed into Interface/AddOns by .toc detection.
    AddonGit,
    Dll,
    Mixed,
    Raw, // downloads asset to a chosen folder (no unzip)
}

impl InstallMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            InstallMode::Auto => "auto",
            InstallMode::Addon => "addon",
            InstallMode::AddonGit => "addon_git",
            InstallMode::Dll => "dll",
            InstallMode::Mixed => "mixed",
            InstallMode::Raw => "raw",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "auto" => Some(InstallMode::Auto),
            "addon" => Some(InstallMode::Addon),
            "addon_git" | "addongit" | "git_addon" => Some(InstallMode::AddonGit),
            "dll" => Some(InstallMode::Dll),
            "mixed" => Some(InstallMode::Mixed),
            "raw" => Some(InstallMode::Raw),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Repo {
    pub id: i64,
    pub url: String,   // canonical repo URL (no /releases)
    pub forge: String, // "github" | "gitlab" | "gitea"
    pub host: String,  // github.com | gitlab.com | codeberg.org | ...

    // For uniqueness and display:
    // - GitHub/Gitea: owner=owner, name=repo
    // - GitLab: owner=full namespace path (e.g. group/subgroup), name=project
    pub owner: String,
    pub name: String,

    pub mode: InstallMode,
    pub enabled: bool,
    pub git_branch: Option<String>, // only used by addon_git mode (None = remote default HEAD)
    pub asset_regex: Option<String>, // optional override for picking asset
    pub last_version: Option<String>, // tag_name last installed
    pub etag: Option<String>,        // for conditional GET (if supported)
    pub installed_asset_id: Option<String>,
    pub installed_asset_name: Option<String>,
    pub installed_asset_size: Option<i64>,
    pub installed_asset_url: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LatestRelease {
    pub tag: String,
    pub name: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    pub id: Option<String>,
    pub name: String,
    pub download_url: String,
    pub size: Option<u64>,
    pub content_type: Option<String>,
    pub sha256: Option<String>,
}
