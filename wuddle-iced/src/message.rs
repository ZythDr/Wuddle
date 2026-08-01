use crate::service::{self, PlanRow, RepoLoadResult};
use crate::settings::{self, UpdateChannel};
use crate::theme::WuddleTheme;
use crate::tweaks;
use crate::types::*;
use iced::Point;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct TextInputAction(Arc<dyn Fn(String) -> Message + Send + Sync>);

impl std::fmt::Debug for TextInputAction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TextInputAction(<redacted>)")
    }
}

impl TextInputAction {
    pub fn new(action: impl Fn(String) -> Message + Send + Sync + 'static) -> Self {
        Self(Arc::new(action))
    }

    pub fn apply(&self, value: String) -> Message {
        (self.0)(value)
    }
}

#[derive(Clone)]
pub struct TextInputContext {
    pub key: String,
    pub value: String,
    pub selection: Option<(usize, usize)>,
    pub cursor: usize,
    pub position: Point,
    pub widget_id: iced::widget::Id,
    pub action: Option<TextInputAction>,
    pub secure: bool,
}

impl std::fmt::Debug for TextInputContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TextInputContext")
            .field("key", &self.key)
            .field("value", &"<redacted>")
            .field("selection", &self.selection)
            .field("cursor", &self.cursor)
            .field("position", &self.position)
            .field("secure", &self.secure)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    SetTab(Tab),
    SetTheme(WuddleTheme),

    // Projects
    SetFilter(Filter),
    SetProjectSearch(String),
    ToggleSort(SortKey),
    InstallRepoOverride {
        url: String,
        mode: String,
    },
    OpenModFileInfo(String),
    FetchDllDescriptionResult(u64, Result<(String, String), String>),

    // Options toggles
    ToggleAutoCheck(bool),
    ToggleConserveGithubApi(bool),
    SetAutoCheckMinutes(String),
    ToggleDesktopNotify(bool),
    ToggleSymlinks(bool),
    ToggleXattr(bool),
    ToggleClock12(bool),
    ToggleFrizFont(bool),
    ToggleRememberWindowGeometry(bool),
    SetUiScaleMode(settings::UiScaleMode),
    SetGithubTokenInput(String),

    // Tweaks
    ToggleTweak(TweakId, bool),

    // Logs
    SetLogFilter(LogFilter),
    SetLogSearch(String),
    ToggleLogWrap(bool),
    ToggleLogAutoScroll(bool),
    ToggleVerboseDiagnostics(bool),
    ToggleLogErrorFetch(bool),
    ToggleLogErrorMisc(bool),
    ClearLogs,
    ExportDiagnostics,
    DiagnosticsExportPathSelected(Option<PathBuf>),
    DiagnosticsExported(Result<(), String>),

    // Full settings/profile backup and restore
    OpenBackupRestore,
    ExportWuddleBackup,
    WuddleBackupExportPathSelected(Option<PathBuf>),
    WuddleBackupExported(Result<crate::backup_restore::ExportSummary, String>),
    PickWuddleBackupArchive,
    PickOldWuddleFolder,
    WuddleBackupSourcePicked {
        path: Option<PathBuf>,
        directory: bool,
    },
    WuddleBackupInspected(Result<crate::backup_restore::BackupPreview, String>),
    ToggleWuddleBackupSection(crate::backup_restore::PreviewSection),
    RequestWuddleRestore,
    CancelWuddleRestore,
    ConfirmWuddleRestore,
    WuddleRestoreStaged(Result<crate::backup_restore::RestoreSchedule, String>),
    WuddleRestoreRestarted(Result<(), String>),
    RequestWuddleReset,
    CancelWuddleReset,
    ToggleWuddleResetCredentials(bool),
    ConfirmWuddleReset,
    WuddleResetPrepared(Result<(), String>),
    WuddleResetRestarted(Result<(), String>),

    // Toast notifications
    DismissToast(usize),
    ToastHovered(usize, bool),
    ToastAnimationTick,
    OpenGithubTokenOptions,

    // Dialogs
    OpenDialog(Dialog),
    CloseDialog,
    FocusNextDialogField,
    FocusPreviousDialogField,
    ToggleModsWarningDoNotShow(bool),
    AcceptModsWarning,
    TogglePatchesWarningDoNotShow(bool),
    AcceptPatchesWarning,
    RequestExit,
    ShutdownTick,
    WindowMoved(iced::Point),
    WindowResized(iced::Size),
    ConsumeDialogClick,

    // MPQ patch management
    OpenMpqAdd,
    SetMpqDirectUrl(String),
    RescanMpqs,
    MpqRescanFinished(Result<usize, String>),
    OpenMpqInstall,
    PickMpqSource,
    MpqSourcePicked {
        request_id: u64,
        scope: ProfileOperationScope,
        path: Option<PathBuf>,
    },
    MpqInspectionFinished {
        operation_id: u64,
        result: ProfileScoped<Result<wuddle_engine::mpq::MpqInspection, String>>,
    },
    SetMpqDisplayName(usize, String),
    SetMpqFileName(usize, String),
    SetMpqDestination(usize, wuddle_engine::mpq::MpqDestination),
    ToggleMpqReplacement(usize, bool),
    InstallMpqPackage,
    MpqTargetsReviewed {
        operation_id: u64,
        result: ProfileScoped<Result<Vec<wuddle_engine::mpq::MpqTargetPreview>, String>>,
    },
    MpqInstallFinished {
        operation_id: u64,
        result: ProfileScoped<Result<i64, String>>,
    },
    ToggleMpqPackageEnabled(i64, bool),
    ToggleMpqEnabled(i64, String, bool),
    MpqEnabledChanged {
        repo_id: i64,
        target_name: String,
        package: bool,
        enabled: bool,
        result: ProfileScoped<Result<usize, String>>,
    },
    OpenMpqProtection,
    MpqProtectionLoaded(Result<Vec<wuddle_engine::mpq::MpqProtectionEntry>, String>),
    MpqLocaleDetected(Result<Option<String>, String>),
    SetUntrackedMpqEditorUnlocked(String, bool),
    SetTrackedMpqEditorUnlocked(i64, String, bool),
    MpqEditorLockChanged {
        repo_id: Option<i64>,
        target_name: String,
        editor_unlocked: bool,
        result: ProfileScoped<Result<(), String>>,
    },
    ToggleUntrackedMpqEnabled(String, bool),
    UntrackedMpqEnabledChanged {
        target_name: String,
        enabled: bool,
        result: ProfileScoped<Result<(), String>>,
    },
    SetMpqEditorDisplayName(String),
    SetMpqEditorFileName(String),
    SetMpqEditorDestination(wuddle_engine::mpq::MpqDestination),
    SetMpqEditorCore(bool),
    SaveMpqEditor,
    MpqEditorSaved(Result<String, String>),
    SetManualMpqDisplayName(String),
    SaveManualMpqDisplayName,
    ManualMpqDisplayNameSaved(Result<(), String>),
    SetManualMpqFileName(String),
    SaveManualMpqFileName,
    ManualMpqFileRenamed(String, Result<String, String>),
    SetMpqComponentDisplayName(String),
    SetMpqComponentFileName(String),
    SetMpqComponentDestination(wuddle_engine::mpq::MpqDestination),
    SaveMpqComponentDisplayName,
    MpqComponentDisplayNameSaved(Result<String, String>),
    SetMpqPackageDisplayName(String),
    SetMpqPackageFileDisplayName(usize, String),
    SetMpqPackageFileName(usize, String),
    SetMpqPackageFileDestination(usize, wuddle_engine::mpq::MpqDestination),
    SetMpqPackageFileEnabled(usize, bool),
    SaveMpqPackage,
    MpqPackageSaved(ProfileScoped<Result<(), String>>),
    RemoveMpqComponent(bool),
    MpqComponentRemoved(Result<(), String>),
    KeepModifiedMpqProtected,
    ModifiedMpqProtected(Result<(), String>),
    OpenWdm,
    WdmResolved {
        operation_id: u64,
        result: ProfileScoped<Result<service::WdmCatalog, String>>,
    },
    SetWdmLocale(String),
    ToggleWdmCaverns(bool),
    ToggleWdmAddon(bool),
    InstallWdm,
    WdmInstallFinished {
        operation_id: u64,
        result: ProfileScoped<Result<i64, String>>,
    },
    ToggleRemoveWdmAddon(bool),
    ConfirmRemoveWdm,
    WdmRemoved {
        operation_id: u64,
        result: ProfileScoped<Result<(), String>>,
    },
    OpenWdmReadme,
    WdmReadmeLoaded(u64, Result<service::RepoPreviewInfo, String>),
    InstallEpochWater,
    EpochWaterInstalled {
        operation_id: u64,
        result: ProfileScoped<Result<i64, String>>,
    },
    OpenEpochWaterReadme,
    EpochWaterReadmeLoaded(u64, Result<service::RepoPreviewInfo, String>),
    UpdateAllPatches,

    // Context menu
    ToggleMenu(String),
    CloseMenu,
    ToggleAddNewMenu,
    OpenTextInputContext(TextInputContext),
    CloseTextInputContext,
    CopyTextInputSelection,
    PasteIntoTextInput,
    TextInputClipboardRead(Option<String>),

    // Engine data (Phase 2)
    ReposLoaded(ProfileScoped<Result<RepoLoadResult, String>>),
    PlansLoaded(ProfileScoped<Result<Vec<PlanRow>, String>>),
    SettingsLoaded(settings::LoadedSettings),

    // Operations (Phase 3)
    CheckUpdates,
    PollRescanProgress,
    PollUpdateCheckProgress,
    LocalArchiveHovered(PathBuf),
    LocalArchiveHoverLeft,
    PickLocalAddonArchive,
    LocalArchivePicked {
        request_id: u64,
        scope: ProfileOperationScope,
        dialog_url: String,
        dialog_mode: String,
        path: Option<PathBuf>,
    },
    LocalArchiveDropped(PathBuf),
    CheckUpdatesResult(ProfileScoped<Result<Vec<PlanRow>, String>>),
    UpdateCheckRateLimitResult(ProfileScoped<(CheckStats, Option<service::GitHubRateInfo>)>),
    GithubRateInfoResult(Option<service::GitHubRateInfo>),
    AddRepoSubmit,
    AddRepoResult(ProfileScoped<Result<i64, String>>),
    /// Result of the lightweight pre-install conflict check that runs after add_repo.
    PreInstallConflictResult {
        repo_id: i64,
        result: ProfileScoped<Result<service::PreInstallConflictInfo, String>>,
    },
    /// Result of the install that fires immediately after a repo is added.
    /// Carries `repo_id` so the conflict handler can force-reinstall the right repo.
    InstallAfterAddResult {
        repo_id: i64,
        result: ProfileScoped<Result<String, String>>,
    },
    /// Fires when the user confirms overwriting file conflicts for a repo that is
    /// already in the DB (the initial install attempt raised ADDON_CONFLICT).
    InstallConflictOverride {
        repo_id: i64,
    },
    ConfirmFileConflict {
        repo_id: i64,
        action: crate::types::FileConflictAction,
    },
    /// Fires when the user clicks Cancel on the conflict dialog for a freshly-added
    /// repo. Removes the repo from the DB so it doesn't remain tracked.
    CancelConflictInstall {
        repo_id: i64,
    },
    CancelConflictInstallResult {
        repo_id: i64,
        result: ProfileScoped<Result<(), String>>,
    },
    RemoveRepoConfirm(i64, bool),
    ToggleRemoveFiles(bool),
    RemoveRepoFilesLoaded(ProfileScoped<Result<Vec<(String, String)>, String>>),
    RepoDetailsLoaded(ProfileScoped<Result<Vec<service::RepoDetailEntry>, String>>),
    ToggleRepoDetailsPath(String),
    RepoDetailsChildrenLoaded(
        ProfileScoped<(String, Result<Vec<service::RepoDetailChild>, String>)>,
    ),
    RemoveRepoResult {
        repo_id: i64,
        repo_name: String,
        remove_files: bool,
        result: ProfileScoped<Result<usize, String>>,
    },
    ToggleRepoEnabled(i64, bool),
    ToggleRepoEnabledResult {
        repo_id: i64,
        enabled: bool,
        result: ProfileScoped<Result<usize, String>>,
    },
    ToggleRepoExpanded(i64),
    ToggleDllEnabled(i64, String, bool),
    ToggleDllEnabledResult {
        repo_id: i64,
        dll_name: String,
        enabled: bool,
        result: ProfileScoped<Result<bool, String>>,
    },
    UpdateAll,
    UpdateAllResult {
        repo_ids: Vec<i64>,
        result: ProfileScoped<Result<Vec<service::UpdateOneResult>, String>>,
    },
    UpdateRepo(i64),
    UpdateRepoResult {
        repo_id: i64,
        replace_local_changes: bool,
        result: ProfileScoped<Result<Option<PlanRow>, String>>,
    },
    ConfirmAddonLocalChangesUpdate(Vec<i64>),
    IgnoreAddonLocalChangesUpdates(Vec<i64>),
    ReinstallRepo(i64),
    ReinstallRepoProbeResult {
        repo_id: i64,
        result: ProfileScoped<Result<wuddle_engine::AddonProbeResult, String>>,
    },
    ReinstallRepoResult {
        repo_id: i64,
        result: ProfileScoped<Result<PlanRow, String>>,
    },
    FetchBranches(i64),
    GithubRateTick,
    FetchBranchesResult(ProfileScoped<(i64, Result<Vec<String>, String>)>),
    SetRepoBranch(i64, String),
    SetRepoBranchResult(ProfileScoped<Result<i64, String>>),
    RefreshRepos,
    SaveSettings,

    // Shared actions
    OpenUrl(String),
    OpenDirectory(String),
    BrowseGamePath(String),
    BrowseGamePathResult(Result<(), String>),
    BrowseRepo(i64),
    BrowseAddonInstall {
        repo_id: i64,
        addon_name: String,
    },
    CopyToClipboard(String),
    LaunchGame,
    LaunchGameResult(Result<String, String>),
    PollSingleInstanceActivation,

    #[cfg(feature = "auto-login")]
    OpenAutoLoginAccounts,
    #[cfg(feature = "auto-login")]
    AddAutoLoginAccount,
    #[cfg(feature = "auto-login")]
    EditAutoLoginAccount(wuddle_engine::auto_login::AccountId),
    #[cfg(feature = "auto-login")]
    AutoLoginAccountLoaded {
        profile_id: String,
        account_id: wuddle_engine::auto_login::AccountId,
        result: Result<wuddle_engine::auto_login::AccountDetails, String>,
    },
    #[cfg(feature = "auto-login")]
    SetAutoLoginLabel(String),
    #[cfg(feature = "auto-login")]
    SetAutoLoginLogin(wuddle_engine::auto_login::SecretText),
    #[cfg(feature = "auto-login")]
    SetAutoLoginPassword(wuddle_engine::auto_login::SecretText),
    #[cfg(feature = "auto-login")]
    SetAutoLoginRealmlist(wuddle_engine::auto_login::SecretText),
    #[cfg(feature = "auto-login")]
    SetAutoLoginRealmName(wuddle_engine::auto_login::SecretText),
    #[cfg(feature = "auto-login")]
    ToggleAutoLoginWarningAcknowledged(bool),
    #[cfg(feature = "auto-login")]
    SaveAutoLoginAccount,
    #[cfg(feature = "auto-login")]
    SaveAutoLoginAccountResult {
        profile_id: String,
        account: wuddle_engine::auto_login::AccountRef,
        is_new: bool,
        result: Result<(), String>,
    },
    #[cfg(feature = "auto-login")]
    RollbackAutoLoginAccountResult(Result<(), String>),
    #[cfg(feature = "auto-login")]
    SelectAutoLoginAccount(Option<wuddle_engine::auto_login::AccountId>),
    #[cfg(feature = "auto-login")]
    SetAutoLoginAccountPickerTooltipVisible(bool),
    #[cfg(feature = "auto-login")]
    DismissAutoLoginAccountPickerTooltip,
    #[cfg(feature = "auto-login")]
    DeleteAutoLoginAccount(wuddle_engine::auto_login::AccountId),
    #[cfg(feature = "auto-login")]
    ConfirmDeleteAutoLoginAccount,
    #[cfg(feature = "auto-login")]
    RetryDeleteAutoLoginAccount {
        profile_id: String,
        account_id: wuddle_engine::auto_login::AccountId,
    },
    #[cfg(feature = "auto-login")]
    DeleteAutoLoginAccountResult {
        profile_id: String,
        account_id: wuddle_engine::auto_login::AccountId,
        result: Result<(), String>,
    },

    // Collection addon management
    OpenCollectionManager(i64),
    FetchCollectionProbe(String),
    FetchCollectionProbeResult(
        ProfileScoped<(String, Result<wuddle_engine::AddonProbeResult, String>)>,
    ),
    SetAddRepoCollectionMode(bool),
    SetCollectionSelection(Vec<String>),
    ToggleCollectionFolder(String),
    ToggleCollectionAddon(String),
    SaveCollectionSelection,
    SaveCollectionSelectionOverride {
        repo_id: i64,
        selected_addons: Vec<String>,
    },
    SaveCollectionSelectionResult(ProfileScoped<Result<String, service::CollectionSelectionError>>),
    SetAddRepoPrimaryAddon(String),
    FetchReleaseAssetOptions(String),
    FetchReleaseAssetOptionsResult(String, Result<Vec<service::ReleaseAssetOption>, String>),
    SetAddRepoReleaseAsset(String),
    RemoveCollectionAddonPrompt {
        repo_id: i64,
        addon_name: String,
    },
    RemoveCollectionAddonConfirm {
        repo_id: i64,
        addon_name: String,
    },

    // GitHub token
    SaveGithubToken,
    SaveGithubTokenResult(Result<(), String>),
    ValidateGithubTokenResult {
        generation: u64,
        result: service::GitHubTokenValidation,
    },
    ForgetGithubToken,
    ForgetGithubTokenResult(Result<service::GitHubTokenSource, String>),

    // Instance settings
    SaveInstanceSettings,
    UpdateInstanceField(InstanceField),
    SwitchProfile(String),
    RemoveProfile(String),
    RemoveProfileResult(String, Result<(), String>),
    InitializeProfileDbResult(String, Result<usize, String>),

    // File dialog
    PickWowDirectory,
    PickWowExecutable,
    WowPathPicked {
        request_id: u64,
        scope: ProfileOperationScope,
        dialog_profile_id: Option<String>,
        path: Option<PathBuf>,
    },

    // Tweaks
    SetTweakFov(f32),
    SetTweakFarclip(f32),
    SetTweakFrilldistance(f32),
    SetTweakNameplateDist(f32),
    SetTweakMaxCameraDist(String),
    SetTweakSoundChannels(String),
    DetectTweakClientResult {
        profile_id: String,
        wow_dir: String,
        auto_launch_exe: Option<String>,
        result: Result<service::ClientVersionInfo, String>,
    },
    ReadTweaks,
    ReadTweaksResult(Result<tweaks::ReadTweakValues, String>),
    ApplyTweaks,
    ApplyTweaksResult(Result<String, String>),
    RestoreTweaks,
    RestoreTweaksResult(Result<String, String>),
    ResetTweaksToDefault,

    ToggleIgnoreUpdates(i64),

    // Merge installs / version pinning
    ToggleMergeInstalls(i64, bool),
    ToggleMergeInstallsResult(ProfileScoped<Result<i64, String>>),
    FetchVersions(i64),
    FetchVersionsResult(ProfileScoped<(i64, Result<Vec<service::VersionItem>, String>)>),
    SetPinnedVersion(i64, Option<String>),
    SetPinnedVersionResult(ProfileScoped<Result<i64, String>>),

    // DLL count change warning
    /// User chose merge (keep existing DLLs) or clean (replace all) from the warning dialog.
    DllCountWarningChoice {
        repo_id: i64,
        merge: bool,
    },

    // About
    CheckSelfUpdate,
    CheckSelfUpdateResult(Result<service::SelfUpdateStatus, String>),
    ApplySelfUpdate,
    ApplySelfUpdateResult(Result<String, String>),
    RestartAfterUpdate,
    ShowChangelog,
    ChangelogLoaded(u64, Result<String, String>),

    // Add-repo preview
    QuickInstallPreset(String),
    SetAddRepoUrl(String),
    DebouncedResolveAddRepoUrl {
        generation: u64,
        url: String,
    },
    RefocusAddRepoUrl,
    ResolveAddRepoUrl,
    FetchRepoPreview(String),
    FetchRepoPreviewResult(u64, String, Result<service::RepoPreviewInfo, String>),
    OpenRepoReadmePreview(String, String),
    RepoReadmePreviewLoaded(u64, Result<service::RepoPreviewInfo, String>),
    ToggleAddRepoDir(String),
    PreviewRepoFile(String),
    PreviewRepoFileResult(u64, Result<(String, String), String>),
    FetchDirContents(String, String),
    FetchDirContentsResult(u64, Result<(String, Vec<service::RepoFileEntry>), String>),

    // Release notes (in-app)
    FetchReleaseNotes,
    FetchReleaseNotesResult(u64, Result<Vec<service::ReleaseItem>, String>),
    ShowReadme,

    // Auto-check tick
    AutoCheckTick,

    // Spinner animation
    SpinnerTick,
    SetCollectionMarqueeHover(bool),

    // Selectable log view
    LogEditorAction(iced::widget::text_editor::Action),

    // README source toggle
    ToggleReadmeSourceView,
    ReadmeEditorAction(iced::widget::text_editor::Action),

    // DXVK config dialog
    OpenDxvkConfig,
    LaunchWowOptimize,
    LaunchWowOptimizeResult(Result<String, String>),
    PromptAwesomeWotlkPatch,
    PromptAwesomeWotlkPatchIfInstalled(ProfileScoped<bool>),
    RunAwesomeWotlkPatch,
    RunAwesomeWotlkPatchResult(Result<String, String>),
    SetDxvkField(DxvkField),
    SaveDxvkConfig,
    DxvkConfigSaved(Result<(), String>),
    ToggleDxvkPreview,
    DxvkPreviewEditorAction(iced::widget::text_editor::Action),

    // Release channel
    SetUpdateChannel(UpdateChannel),
    SwitchToStableChannel,
}
