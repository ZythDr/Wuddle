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
    ToggleLogErrorFetch(bool),
    ToggleLogErrorMisc(bool),
    ClearLogs,

    // Toast notifications
    DismissToast(usize),

    // Dialogs
    OpenDialog(Dialog),
    CloseDialog,
    RequestExit,
    ConsumeDialogClick,

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
    PollUpdateCheckProgress,
    CheckUpdatesResult(Result<Vec<PlanRow>, String>),
    UpdateCheckRateLimitResult(CheckStats, Option<service::GitHubRateInfo>),
    GithubRateInfoResult(Option<service::GitHubRateInfo>),
    AddRepoSubmit,
    AddRepoResult(Result<i64, String>),
    InstallAfterAddResult(Result<String, String>),
    RemoveRepoConfirm(i64, bool),
    ToggleRemoveFiles(bool),
    RemoveRepoFilesLoaded(Result<Vec<(String, String)>, String>),
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
    BrowseRepo(i64),
    BrowseAddonInstall { repo_id: i64, addon_name: String },
    CopyToClipboard(String),
    LaunchGame,
    LaunchGameResult(Result<String, String>),

    // Collection addon management
    OpenCollectionManager(i64),
    FetchCollectionProbe(String),
    FetchCollectionProbeResult(Result<wuddle_engine::AddonProbeResult, String>),
    SetAddRepoCollectionMode(bool),
    ToggleCollectionFolder(String),
    ToggleCollectionAddon(String),
    SaveCollectionSelection,
    SaveCollectionSelectionOverride { repo_id: i64, selected_addons: Vec<String> },
    SaveCollectionSelectionResult(Result<String, service::CollectionSelectionError>),
    RemoveCollectionAddonPrompt { repo_id: i64, addon_name: String },
    RemoveCollectionAddonConfirm { repo_id: i64, addon_name: String },

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
    RemoveProfileResult(Result<String, String>),

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
    DetectTweakClientResult(Result<service::ClientVersionInfo, String>),
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
    FetchRepoPreview(String),
    FetchRepoPreviewResult(Result<service::RepoPreviewInfo, String>),
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

    // Selectable log view
    LogEditorAction(iced::widget::text_editor::Action),

    // README source toggle
    ToggleReadmeSourceView,
    ReadmeEditorAction(iced::widget::text_editor::Action),

    // DXVK config dialog
    OpenDxvkConfig,
    SetDxvkField(DxvkField),
    SaveDxvkConfig,
    DxvkConfigSaved(Result<(), String>),
    ToggleDxvkPreview,
    DxvkPreviewEditorAction(iced::widget::text_editor::Action),

    // Release channel
    SetUpdateChannel(UpdateChannel),
    SwitchToStableChannel,
}
