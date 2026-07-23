use crate::service::{self, PlanRow, RepoLoadResult};
use crate::settings::{self, UpdateChannel};
use crate::theme::WuddleTheme;
use crate::tweaks;
use crate::types::*;
use std::path::PathBuf;

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
    FetchDllDescriptionResult(Result<(String, String), String>),

    // Options toggles
    ToggleAutoCheck(bool),
    ToggleConserveGithubApi(bool),
    SetAutoCheckMinutes(String),
    ToggleDesktopNotify(bool),
    ToggleSymlinks(bool),
    ToggleXattr(bool),
    ToggleClock12(bool),
    ToggleFrizFont(bool),
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
    ConsumeDialogClick,

    // MPQ patch management
    OpenMpqAdd,
    SetMpqDirectUrl(String),
    RescanMpqs,
    MpqRescanFinished(Result<usize, String>),
    OpenMpqInstall,
    PickMpqSource,
    MpqSourcePicked(Option<PathBuf>),
    MpqInspectionFinished(Result<wuddle_engine::mpq::MpqInspection, String>),
    SetMpqDisplayName(usize, String),
    SetMpqFileName(usize, String),
    SetMpqDestination(usize, wuddle_engine::mpq::MpqDestination),
    ToggleMpqReplacement(usize, bool),
    InstallMpqPackage,
    MpqTargetsReviewed(Result<Vec<wuddle_engine::mpq::MpqTargetPreview>, String>),
    MpqInstallFinished(Result<i64, String>),
    ToggleMpqPackageEnabled(i64, bool),
    ToggleMpqEnabled(i64, String, bool),
    MpqEnabledChanged(Result<bool, String>),
    OpenMpqProtection,
    MpqProtectionLoaded(Result<Vec<wuddle_engine::mpq::MpqProtectionEntry>, String>),
    MpqLocaleDetected(Result<Option<String>, String>),
    SetUntrackedMpqEditorUnlocked(String, bool),
    SetTrackedMpqEditorUnlocked(i64, String, bool),
    ToggleUntrackedMpqEnabled(String, bool),
    MpqProtectionChanged(Result<(), String>),
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
    RemoveMpqComponent(bool),
    MpqComponentRemoved(Result<(), String>),
    KeepModifiedMpqProtected,
    ModifiedMpqProtected(Result<(), String>),
    OpenWdm,
    WdmResolved(Result<service::WdmCatalog, String>),
    SetWdmLocale(String),
    ToggleWdmCaverns(bool),
    ToggleWdmAddon(bool),
    InstallWdm,
    WdmInstallFinished(Result<i64, String>),
    ToggleRemoveWdmAddon(bool),
    ConfirmRemoveWdm,
    WdmRemoved(Result<(), String>),
    OpenWdmReadme,
    WdmReadmeLoaded(Result<service::RepoPreviewInfo, String>),
    InstallEpochWater,
    EpochWaterInstalled(Result<i64, String>),
    OpenEpochWaterReadme,
    EpochWaterReadmeLoaded(Result<service::RepoPreviewInfo, String>),
    UpdateAllPatches,

    // Context menu
    ToggleMenu(String),
    CloseMenu,
    ToggleAddNewMenu,

    // Engine data (Phase 2)
    ReposLoaded(Result<RepoLoadResult, String>),
    PlansLoaded(Result<Vec<PlanRow>, String>),
    SettingsLoaded(settings::AppSettings),

    // Operations (Phase 3)
    CheckUpdates,
    PollRescanProgress,
    PollUpdateCheckProgress,
    LocalArchiveHovered(PathBuf),
    LocalArchiveHoverLeft,
    PickLocalAddonArchive,
    LocalArchivePicked(Option<PathBuf>),
    LocalArchiveDropped(PathBuf),
    CheckUpdatesResult(Result<Vec<PlanRow>, String>),
    UpdateCheckRateLimitResult(CheckStats, Option<service::GitHubRateInfo>),
    GithubRateInfoResult(Option<service::GitHubRateInfo>),
    AddRepoSubmit,
    AddRepoResult(Result<i64, String>),
    /// Result of the lightweight pre-install conflict check that runs after add_repo.
    PreInstallConflictResult {
        repo_id: i64,
        result: Result<service::PreInstallConflictInfo, String>,
    },
    /// Result of the install that fires immediately after a repo is added.
    /// Carries `repo_id` so the conflict handler can force-reinstall the right repo.
    InstallAfterAddResult {
        repo_id: i64,
        result: Result<String, String>,
    },
    /// Fires when the user confirms overwriting file conflicts for a repo that is
    /// already in the DB (the initial install attempt raised ADDON_CONFLICT).
    InstallConflictOverride {
        repo_id: i64,
    },
    /// Fires when the user clicks Cancel on the conflict dialog for a freshly-added
    /// repo. Removes the repo from the DB so it doesn't remain tracked.
    CancelConflictInstall {
        repo_id: i64,
    },
    CancelConflictInstallResult {
        repo_id: i64,
        result: Result<(), String>,
    },
    RemoveRepoConfirm(i64, bool),
    ToggleRemoveFiles(bool),
    RemoveRepoFilesLoaded(Result<Vec<(String, String)>, String>),
    RepoDetailsLoaded(Result<Vec<service::RepoDetailEntry>, String>),
    ToggleRepoDetailsPath(String),
    RepoDetailsChildrenLoaded(String, Result<Vec<service::RepoDetailChild>, String>),
    RemoveRepoResult(Result<(), String>),
    ToggleRepoEnabled(i64, bool),
    ToggleRepoEnabledResult(Result<(), String>),
    ToggleRepoExpanded(i64),
    ToggleDllEnabled(i64, String, bool),
    ToggleDllEnabledResult(Result<(), String>),
    UpdateAll,
    UpdateAllResult(Result<Vec<service::UpdateOneResult>, String>),
    UpdateRepo(i64),
    UpdateRepoResult(Result<Option<PlanRow>, String>),
    ReinstallRepo(i64),
    ReinstallRepoProbeResult {
        repo_id: i64,
        result: Result<wuddle_engine::AddonProbeResult, String>,
    },
    ReinstallRepoResult(Result<PlanRow, String>),
    FetchBranches(i64),
    GithubRateTick,
    FetchBranchesResult((i64, Result<Vec<String>, String>)),
    SetRepoBranch(i64, String),
    SetRepoBranchResult(Result<i64, String>),
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
    DeleteAutoLoginAccountResult {
        profile_id: String,
        account_id: wuddle_engine::auto_login::AccountId,
        result: Result<(), String>,
    },

    // Collection addon management
    OpenCollectionManager(i64),
    FetchCollectionProbe(String),
    FetchCollectionProbeResult(String, Result<wuddle_engine::AddonProbeResult, String>),
    SetAddRepoCollectionMode(bool),
    SetCollectionSelection(Vec<String>),
    ToggleCollectionFolder(String),
    ToggleCollectionAddon(String),
    SaveCollectionSelection,
    SaveCollectionSelectionOverride {
        repo_id: i64,
        selected_addons: Vec<String>,
    },
    SaveCollectionSelectionResult(Result<String, service::CollectionSelectionError>),
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
    ForgetGithubToken,
    ForgetGithubTokenResult(Result<(), String>),

    // Instance settings
    SaveInstanceSettings,
    UpdateInstanceField(InstanceField),
    SwitchProfile(String),
    RemoveProfile(String),
    RemoveProfileResult(String, Result<Option<String>, String>),
    InitializeProfileDbResult(String, Result<usize, String>),

    // File dialog
    PickWowDirectory,
    PickWowExecutable,
    WowPathPicked(Option<PathBuf>),

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
    ToggleMergeInstallsResult(Result<i64, String>),
    FetchVersions(i64),
    FetchVersionsResult((i64, Result<Vec<service::VersionItem>, String>)),
    SetPinnedVersion(i64, Option<String>),
    SetPinnedVersionResult(Result<i64, String>),

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
    ChangelogLoaded(Result<String, String>),

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
    FetchRepoPreviewResult(String, Result<service::RepoPreviewInfo, String>),
    OpenRepoReadmePreview(String, String),
    RepoReadmePreviewLoaded(Result<service::RepoPreviewInfo, String>),
    ToggleAddRepoDir(String),
    PreviewRepoFile(String),
    PreviewRepoFileResult(Result<(String, String), String>),
    FetchDirContents(String, String),
    FetchDirContentsResult(Result<(String, Vec<service::RepoFileEntry>), String>),

    // Release notes (in-app)
    FetchReleaseNotes,
    FetchReleaseNotesResult(Result<Vec<service::ReleaseItem>, String>),
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
    PromptAwesomeWotlkPatchIfInstalled(bool),
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
