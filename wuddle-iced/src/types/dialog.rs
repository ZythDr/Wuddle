/// Three-state option: let DXVK auto-detect, force on, or force off.
#[derive(Debug, Clone, PartialEq)]
pub enum TriState {
    Auto,
    True,
    False,
}

/// How to handle d3d9.presentInterval (VSync override).
#[derive(Debug, Clone, PartialEq)]
pub enum PresentInterval {
    Default, // -1: do not override the in-game setting
    NoSync,  // 0: always no VSync
    Vsync,   // 1: always VSync
    Half,    // 2: half refresh rate (e.g. 30 fps on 60 Hz)
}

/// Anisotropic filtering level for d3d9.samplerAnisotropy.
#[derive(Debug, Clone, PartialEq)]
pub enum AnisotropyLevel {
    NoOverride, // -1: let the game / driver decide
    Off,        // 0: force disabled
    X2,
    X4,
    X8,
    X16,
}

/// Field mutation carried by SetDxvkField messages.
#[derive(Debug, Clone)]
pub enum DxvkField {
    MaxFrameRate(String),
    MaxFrameLatency(String),
    LatencySleep(TriState),
    EnableDialogMode(bool),
    DpiAware(bool),
    PresentInterval(PresentInterval),
    TearFree(TriState),
    SamplerAnisotropy(AnisotropyLevel),
    ClampNegativeLodBias(bool),
    NumCompilerThreads(String),
    EnableGpl(TriState),
    TrackPipelineLifetime(TriState),
    DeferSurfaceCreation(bool),
    LenientClear(bool),
    LogPath(String),
    Hud(String),
    EnableAsync(bool),
}

/// State held inside Dialog::DxvkConfig.
#[derive(Debug, Clone)]
pub struct DxvkConfig {
    pub max_frame_rate: String,              // d3d9.maxFrameRate
    pub max_frame_latency: String,           // d3d9.maxFrameLatency
    pub latency_sleep: TriState,             // dxvk.latencySleep
    pub enable_dialog_mode: bool,            // d3d9.enableDialogMode
    pub dpi_aware: bool,                     // d3d9.dpiAware
    pub present_interval: PresentInterval,   // d3d9.presentInterval
    pub tear_free: TriState,                 // dxvk.tearFree
    pub sampler_anisotropy: AnisotropyLevel, // d3d9.samplerAnisotropy
    pub clamp_negative_lod_bias: bool,       // d3d9.clampNegativeLodBias
    pub num_compiler_threads: String,        // dxvk.numCompilerThreads
    pub enable_gpl: TriState,                // dxvk.enableGraphicsPipelineLibrary
    pub track_pipeline_lifetime: TriState,   // dxvk.trackPipelineLifetime
    pub defer_surface_creation: bool,        // d3d9.deferSurfaceCreation
    pub lenient_clear: bool,                 // d3d9.lenientClear
    pub log_path: String,                    // dxvk.logPath
    pub hud: String,                         // dxvk.hud
    pub enable_async: bool,                  // dxvk.enableAsync (gplasync fork)
}

impl Default for DxvkConfig {
    fn default() -> Self {
        Self {
            max_frame_rate: "240".into(),
            max_frame_latency: "1".into(),
            latency_sleep: TriState::Auto,
            enable_dialog_mode: true,
            dpi_aware: false,
            present_interval: PresentInterval::Default,
            tear_free: TriState::Auto,
            sampler_anisotropy: AnisotropyLevel::X16,
            clamp_negative_lod_bias: false,
            num_compiler_threads: "0".into(),
            enable_gpl: TriState::Auto,
            track_pipeline_lifetime: TriState::Auto,
            defer_surface_creation: true,
            lenient_clear: true,
            log_path: ".".into(),
            hud: String::new(),
            enable_async: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileConflictAction {
    Install,
    Update,
    UpdateApprovedLocalChanges,
    Reinstall,
}

#[derive(Debug, Clone)]
pub struct AddonLocalChangesEntry {
    pub repo_id: i64,
    pub repo_name: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct MpqPackageFileDraft {
    pub path: String,
    pub display_name: String,
    pub edited_display_name: String,
    pub file_name: String,
    pub edited_file_name: String,
    pub destination: wuddle_engine::mpq::MpqDestination,
    pub edited_destination: wuddle_engine::mpq::MpqDestination,
    pub enabled: bool,
    pub edited_enabled: bool,
    pub editor_unlocked: bool,
    pub status: wuddle_engine::mpq::MpqFileStatus,
}

#[derive(Debug, Clone)]
pub enum Dialog {
    MpqAdd,
    MpqInstall,
    ProtectedMpqs,
    WdmInstall,
    RemoveWdm {
        repo_id: i64,
        addon_repo_id: i64,
        remove_addon: bool,
    },
    MpqComponent {
        repo_id: i64,
        path: String,
        display_name: String,
        edited_display_name: String,
        file_name: String,
        edited_file_name: String,
        destination: wuddle_engine::mpq::MpqDestination,
        edited_destination: wuddle_engine::mpq::MpqDestination,
        status: wuddle_engine::mpq::MpqFileStatus,
    },
    MpqPackage {
        repo_id: i64,
        display_name: String,
        edited_display_name: String,
        files: Vec<MpqPackageFileDraft>,
    },
    ManualMpq {
        path: String,
        display_name: String,
        edited_display_name: String,
    },
    RenameManualMpq {
        path: String,
        file_name: String,
        edited_file_name: String,
        return_to_manage: bool,
    },
    EditUntrackedMpq {
        path: String,
        display_name: String,
        edited_display_name: String,
        file_name: String,
        edited_file_name: String,
        destination: wuddle_engine::mpq::MpqDestination,
        edited_destination: wuddle_engine::mpq::MpqDestination,
        core: bool,
        edited_core: bool,
    },
    AddRepo {
        url: String,
        mode: String,
        is_addons: bool,
        advanced: bool,
    },
    ModsWarning {
        do_not_show_again: bool,
    },
    PatchesWarning {
        do_not_show_again: bool,
    },
    RemoveRepo {
        id: i64,
        name: String,
        remove_files: bool,
        files: Vec<(String, String)>,
    },
    RepoDetails {
        id: Option<i64>,
        name: String,
        files: Vec<crate::service::RepoDetailEntry>,
        loading: bool,
        expanded_paths: std::collections::HashSet<String>,
        loading_paths: std::collections::HashSet<String>,
        children: std::collections::HashMap<String, Vec<crate::service::RepoDetailChild>>,
    },
    RemoveCollectionAddon {
        repo_id: i64,
        repo_name: String,
        addon_name: String,
        files: Vec<(String, String)>,
    },
    Changelog {
        title: String,
        items: Vec<iced::widget::markdown::Item>,
        loading: bool,
    },
    DxvkConfig {
        config: DxvkConfig,
        show_preview: bool,
    },
    AwesomeWotlkPatchWarning,
    DllCountWarning {
        repo_id: i64,
        repo_name: String,
        previous_count: usize,
        new_count: usize,
    },
    AddonLocalChanges {
        repos: Vec<AddonLocalChangesEntry>,
    },
    InstanceSettings {
        is_new: bool,
        profile_id: String,
        name: String,
        wow_dir: String,
        launch_method: String, // "auto", "lutris", "wine", "custom"
        show_mods_tab: bool,
        show_addons_tab: bool,
        show_patches_tab: bool,
        show_tweaks_tab: bool,
        clear_wdb: bool,
        auto_login_enabled: bool,
        lutris_target: String,
        wine_command: String,
        wine_args: String,
        custom_command: String,
        custom_args: String,
    },
    #[cfg(feature = "auto-login")]
    AutoLoginAccounts,
    #[cfg(feature = "auto-login")]
    AutoLoginEditor,
    #[cfg(feature = "auto-login")]
    DeleteAutoLoginAccount {
        account_id: wuddle_engine::auto_login::AccountId,
        label: String,
    },
    AvWarning {
        url: String,
        mode: String,
    },
    FileConflict {
        repo_id: i64,
        repo_name: String,
        files: Vec<String>,
        action: FileConflictAction,
    },
    AddonConflict {
        url: String,
        mode: String,
        conflicts: Vec<wuddle_engine::AddonProbeConflict>,
        /// If the repo is already in the DB (install failed mid-way), store its id so the
        /// "Overwrite" button can force-reinstall rather than re-adding from scratch.
        pending_repo_id: Option<i64>,
        /// Display label for the new repo, e.g. "owner/name".
        new_repo_label: String,
        /// Existing repos that own the conflicting files (for the "Old" panel).
        existing_repos: Vec<crate::service::CollectionConflictOwnerGroup>,
        /// All addons that the new repo will install (for the "New" panel).
        selected_addons: Vec<String>,
        /// Full file list for the new repo (for a richer preview).
        new_repo_preview: Option<Vec<crate::service::RepoFileEntry>>,
    },
    CollectionAddonConflict {
        repo_id: i64,
        repo_name: String,
        repo_url: String,
        selected_addons: Vec<String>,
        conflicts: Vec<wuddle_engine::AddonProbeConflict>,
        existing_repos: Vec<crate::service::CollectionConflictOwnerGroup>,
    },
    /// Shown when the probe finishes and detects >1 addon folder in a new-add flow.
    /// Asking the user whether to treat the repo as a collection or a single modular addon.
    CollectionChoice {
        url: String,
        addon_names: Vec<String>,
    },
    /// Shown when a single-addon repo has multiple .toc files at the root.
    SelectMainAddon {
        url: String,
        options: Vec<String>,
        suggested: Option<String>,
        /// Set for a Reinstall / Repair TOC choice; absent for a new repo.
        reinstall_repo_id: Option<i64>,
    },
    /// Shown when a release publishes multiple compatible archive assets.
    SelectReleaseAsset {
        url: String,
        options: Vec<String>,
    },
}

impl Dialog {
    pub(crate) fn is_mpq_workflow(&self) -> bool {
        matches!(
            self,
            Self::MpqAdd
                | Self::MpqInstall
                | Self::ProtectedMpqs
                | Self::WdmInstall
                | Self::RemoveWdm { .. }
                | Self::MpqComponent { .. }
                | Self::MpqPackage { .. }
                | Self::ManualMpq { .. }
                | Self::RenameManualMpq { .. }
                | Self::EditUntrackedMpq { .. }
        )
    }

    /// A newly tracked repository must be removed when its conflict prompt is
    /// dismissed by any route (Cancel, X, Escape, or scrim).
    pub(crate) fn pending_conflict_cleanup_repo_id(&self) -> Option<i64> {
        match self {
            Self::AddonConflict {
                pending_repo_id: Some(repo_id),
                ..
            } => Some(*repo_id),
            Self::FileConflict {
                repo_id,
                action: FileConflictAction::Install,
                ..
            } => Some(*repo_id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Dialog;

    #[test]
    fn only_newly_tracked_addon_conflicts_request_cleanup_on_close() {
        let pending = Dialog::AddonConflict {
            url: "https://example.invalid/repo".to_string(),
            mode: "addon_git".to_string(),
            conflicts: Vec::new(),
            pending_repo_id: Some(42),
            new_repo_label: "repo".to_string(),
            existing_repos: Vec::new(),
            selected_addons: Vec::new(),
            new_repo_preview: None,
        };
        assert_eq!(pending.pending_conflict_cleanup_repo_id(), Some(42));

        let untracked = Dialog::AddonConflict {
            url: "https://example.invalid/repo".to_string(),
            mode: "addon_git".to_string(),
            conflicts: Vec::new(),
            pending_repo_id: None,
            new_repo_label: "repo".to_string(),
            existing_repos: Vec::new(),
            selected_addons: Vec::new(),
            new_repo_preview: None,
        };
        assert_eq!(untracked.pending_conflict_cleanup_repo_id(), None);

        let file_conflict = Dialog::FileConflict {
            repo_id: 43,
            repo_name: "owner/repo".to_string(),
            files: vec!["shared.dll".to_string()],
            action: super::FileConflictAction::Install,
        };
        assert_eq!(file_conflict.pending_conflict_cleanup_repo_id(), Some(43));

        let update_conflict = Dialog::FileConflict {
            repo_id: 43,
            repo_name: "owner/repo".to_string(),
            files: vec!["shared.dll".to_string()],
            action: super::FileConflictAction::Update,
        };
        assert_eq!(update_conflict.pending_conflict_cleanup_repo_id(), None);
    }

    #[test]
    fn mpq_workflow_dialogs_are_classified_for_commit_dismissal_guards() {
        assert!(Dialog::MpqAdd.is_mpq_workflow());
        assert!(Dialog::WdmInstall.is_mpq_workflow());
        assert!(Dialog::RemoveWdm {
            repo_id: 1,
            addon_repo_id: 2,
            remove_addon: true,
        }
        .is_mpq_workflow());
        assert!(!Dialog::PatchesWarning {
            do_not_show_again: false,
        }
        .is_mpq_workflow());
    }
}
