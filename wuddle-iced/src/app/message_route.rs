use crate::{Dialog, Message};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MessageRoute {
    Mpq,
    #[cfg(feature = "auto-login")]
    AutoLogin,
    Misc,
    Tweaks,
    About,
    Settings,
    Repos,
    App,
}

/// Classify a message by borrowing it. `App::update` can then move the value
/// directly into its one owning handler instead of cloning it through every
/// feature router.
pub(super) fn classify(message: &Message, dialog: &Option<Dialog>) -> MessageRoute {
    match message {
        Message::LocalArchiveHovered(_) | Message::LocalArchiveDropped(_)
            if matches!(dialog, Some(Dialog::MpqInstall)) =>
        {
            MessageRoute::Mpq
        }
        Message::LocalArchiveHovered(_) | Message::LocalArchiveDropped(_) => MessageRoute::Repos,

        Message::OpenMpqAdd
        | Message::SetMpqDirectUrl(_)
        | Message::RescanMpqs
        | Message::MpqRescanFinished(_)
        | Message::UpdateAllPatches
        | Message::InstallEpochWater
        | Message::EpochWaterInstalled { .. }
        | Message::OpenEpochWaterReadme
        | Message::EpochWaterReadmeLoaded(..)
        | Message::OpenMpqInstall
        | Message::PickMpqSource
        | Message::MpqSourcePicked { .. }
        | Message::MpqInspectionFinished { .. }
        | Message::SetMpqDisplayName(..)
        | Message::SetMpqFileName(..)
        | Message::SetMpqDestination(..)
        | Message::ToggleMpqReplacement(..)
        | Message::InstallMpqPackage
        | Message::MpqTargetsReviewed { .. }
        | Message::MpqInstallFinished { .. }
        | Message::ToggleMpqPackageEnabled(..)
        | Message::ToggleMpqEnabled(..)
        | Message::MpqEnabledChanged { .. }
        | Message::OpenMpqProtection
        | Message::MpqLocaleDetected(..)
        | Message::MpqProtectionLoaded(..)
        | Message::SetUntrackedMpqEditorUnlocked(..)
        | Message::SetTrackedMpqEditorUnlocked(..)
        | Message::MpqEditorLockChanged { .. }
        | Message::ToggleUntrackedMpqEnabled(..)
        | Message::UntrackedMpqEnabledChanged { .. }
        | Message::SetMpqEditorDisplayName(..)
        | Message::SetMpqEditorFileName(..)
        | Message::SetMpqEditorDestination(..)
        | Message::SetMpqEditorCore(..)
        | Message::SaveMpqEditor
        | Message::MpqEditorSaved(..)
        | Message::SetManualMpqDisplayName(..)
        | Message::SaveManualMpqDisplayName
        | Message::ManualMpqDisplayNameSaved(..)
        | Message::SetManualMpqFileName(..)
        | Message::SaveManualMpqFileName
        | Message::ManualMpqFileRenamed(..)
        | Message::SetMpqComponentDisplayName(..)
        | Message::SetMpqComponentFileName(..)
        | Message::SetMpqComponentDestination(..)
        | Message::SaveMpqComponentDisplayName
        | Message::MpqComponentDisplayNameSaved(..)
        | Message::SetMpqPackageDisplayName(..)
        | Message::SetMpqPackageFileDisplayName(..)
        | Message::SetMpqPackageFileName(..)
        | Message::SetMpqPackageFileDestination(..)
        | Message::SetMpqPackageFileEnabled(..)
        | Message::SaveMpqPackage
        | Message::MpqPackageSaved(..)
        | Message::RemoveMpqComponent(..)
        | Message::MpqComponentRemoved(..)
        | Message::KeepModifiedMpqProtected
        | Message::ModifiedMpqProtected(..)
        | Message::OpenWdm
        | Message::WdmResolved { .. }
        | Message::SetWdmLocale(..)
        | Message::ToggleWdmCaverns(..)
        | Message::ToggleWdmAddon(..)
        | Message::InstallWdm
        | Message::WdmInstallFinished { .. }
        | Message::ToggleRemoveWdmAddon(..)
        | Message::ConfirmRemoveWdm
        | Message::WdmRemoved { .. }
        | Message::OpenWdmReadme
        | Message::WdmReadmeLoaded(..) => MessageRoute::Mpq,

        #[cfg(feature = "auto-login")]
        Message::OpenAutoLoginAccounts
        | Message::SetAutoLoginAccountPickerTooltipVisible(..)
        | Message::DismissAutoLoginAccountPickerTooltip
        | Message::AddAutoLoginAccount
        | Message::EditAutoLoginAccount(..)
        | Message::AutoLoginAccountLoaded { .. }
        | Message::SetAutoLoginLabel(..)
        | Message::SetAutoLoginLogin(..)
        | Message::SetAutoLoginPassword(..)
        | Message::SetAutoLoginRealmlist(..)
        | Message::SetAutoLoginRealmName(..)
        | Message::ToggleAutoLoginWarningAcknowledged(..)
        | Message::SaveAutoLoginAccount
        | Message::SaveAutoLoginAccountResult { .. }
        | Message::RollbackAutoLoginAccountResult(..)
        | Message::SelectAutoLoginAccount(..)
        | Message::DeleteAutoLoginAccount(..)
        | Message::ConfirmDeleteAutoLoginAccount
        | Message::RetryDeleteAutoLoginAccount { .. }
        | Message::DeleteAutoLoginAccountResult { .. } => MessageRoute::AutoLogin,

        Message::WindowMoved(..)
        | Message::WindowResized(..)
        | Message::OpenUrl(..)
        | Message::OpenDirectory(..)
        | Message::CopyToClipboard(..)
        | Message::LaunchGame
        | Message::LaunchGameResult(..)
        | Message::PollSingleInstanceActivation
        | Message::LaunchWowOptimize
        | Message::LaunchWowOptimizeResult(..)
        | Message::RunAwesomeWotlkPatch
        | Message::RunAwesomeWotlkPatchResult(..)
        | Message::SpinnerTick
        | Message::DismissToast(..)
        | Message::ToastHovered(..)
        | Message::ToastAnimationTick => MessageRoute::Misc,

        Message::DetectTweakClientResult { .. }
        | Message::ToggleTweak(..)
        | Message::SetTweakFov(..)
        | Message::SetTweakFarclip(..)
        | Message::SetTweakFrilldistance(..)
        | Message::SetTweakNameplateDist(..)
        | Message::SetTweakMaxCameraDist(..)
        | Message::SetTweakSoundChannels(..)
        | Message::ReadTweaks
        | Message::ReadTweaksResult(..)
        | Message::ApplyTweaks
        | Message::ApplyTweaksResult(..)
        | Message::RestoreTweaks
        | Message::RestoreTweaksResult(..)
        | Message::ResetTweaksToDefault => MessageRoute::Tweaks,

        Message::CheckSelfUpdate
        | Message::CheckSelfUpdateResult(..)
        | Message::ApplySelfUpdate
        | Message::ApplySelfUpdateResult(..)
        | Message::RestartAfterUpdate
        | Message::ShowChangelog
        | Message::ChangelogLoaded(..)
        | Message::SetUpdateChannel(..)
        | Message::SwitchToStableChannel => MessageRoute::About,

        Message::SetTheme(..)
        | Message::ToggleAutoCheck(..)
        | Message::ToggleConserveGithubApi(..)
        | Message::SetAutoCheckMinutes(..)
        | Message::ToggleDesktopNotify(..)
        | Message::ToggleSymlinks(..)
        | Message::ToggleXattr(..)
        | Message::ToggleClock12(..)
        | Message::ToggleFrizFont(..)
        | Message::ToggleRememberWindowGeometry(..)
        | Message::SetUiScaleMode(..)
        | Message::SetGithubTokenInput(..)
        | Message::SaveGithubToken
        | Message::SaveGithubTokenResult(..)
        | Message::ValidateGithubTokenResult { .. }
        | Message::ForgetGithubToken
        | Message::ForgetGithubTokenResult(..)
        | Message::UpdateInstanceField(..)
        | Message::SaveInstanceSettings
        | Message::SwitchProfile(..)
        | Message::RemoveProfile(..)
        | Message::RemoveProfileResult(..)
        | Message::InitializeProfileDbResult(..)
        | Message::SettingsLoaded(..)
        | Message::SaveSettings
        | Message::PickWowDirectory
        | Message::PickWowExecutable
        | Message::WowPathPicked { .. }
        | Message::AutoCheckTick => MessageRoute::Settings,

        Message::ReposLoaded(..)
        | Message::PollRescanProgress
        | Message::PlansLoaded(..)
        | Message::RefreshRepos
        | Message::CheckUpdates
        | Message::PollUpdateCheckProgress
        | Message::GithubRateTick
        | Message::CheckUpdatesResult(..)
        | Message::AddRepoSubmit
        | Message::LocalArchiveHoverLeft
        | Message::PickLocalAddonArchive
        | Message::LocalArchivePicked { .. }
        | Message::AddRepoResult(..)
        | Message::PreInstallConflictResult { .. }
        | Message::InstallAfterAddResult { .. }
        | Message::CancelConflictInstall { .. }
        | Message::CancelConflictInstallResult { .. }
        | Message::InstallConflictOverride { .. }
        | Message::ConfirmFileConflict { .. }
        | Message::InstallRepoOverride { .. }
        | Message::OpenCollectionManager(..)
        | Message::FetchCollectionProbe(..)
        | Message::FetchCollectionProbeResult(..)
        | Message::SetAddRepoCollectionMode(..)
        | Message::SetCollectionSelection(..)
        | Message::SetAddRepoPrimaryAddon(..)
        | Message::ToggleCollectionFolder(..)
        | Message::ToggleCollectionAddon(..)
        | Message::SaveCollectionSelection
        | Message::SaveCollectionSelectionOverride { .. }
        | Message::SaveCollectionSelectionResult(..)
        | Message::BrowseAddonInstall { .. }
        | Message::RemoveCollectionAddonPrompt { .. }
        | Message::RemoveCollectionAddonConfirm { .. }
        | Message::RemoveRepoConfirm(..)
        | Message::RemoveRepoResult { .. }
        | Message::ToggleIgnoreUpdates(..)
        | Message::ToggleMergeInstalls(..)
        | Message::ToggleMergeInstallsResult(..)
        | Message::FetchVersions(..)
        | Message::FetchVersionsResult(..)
        | Message::SetPinnedVersion(..)
        | Message::SetPinnedVersionResult(..)
        | Message::DllCountWarningChoice { .. }
        | Message::BrowseRepo(..)
        | Message::BrowseGamePath(..)
        | Message::BrowseGamePathResult(..)
        | Message::UpdateRepo(..)
        | Message::UpdateRepoResult { .. }
        | Message::ToggleRepoEnabled(..)
        | Message::ToggleRepoEnabledResult { .. }
        | Message::ToggleRepoExpanded(..)
        | Message::ToggleDllEnabled(..)
        | Message::ToggleDllEnabledResult { .. }
        | Message::UpdateAll
        | Message::UpdateAllResult { .. }
        | Message::ReinstallRepo(..)
        | Message::ReinstallRepoProbeResult { .. }
        | Message::ReinstallRepoResult { .. }
        | Message::FetchBranches(..)
        | Message::FetchBranchesResult(..)
        | Message::SetRepoBranch(..)
        | Message::SetRepoBranchResult(..)
        | Message::UpdateCheckRateLimitResult(..)
        | Message::GithubRateInfoResult(..)
        | Message::ToggleRemoveFiles(..)
        | Message::RemoveRepoFilesLoaded(..)
        | Message::FetchRepoPreview(..)
        | Message::OpenRepoReadmePreview(..)
        | Message::RepoReadmePreviewLoaded(..)
        | Message::FetchRepoPreviewResult(..)
        | Message::FetchReleaseAssetOptions(..)
        | Message::FetchReleaseAssetOptionsResult(..)
        | Message::SetAddRepoReleaseAsset(..)
        | Message::ToggleAddRepoDir(..)
        | Message::FetchDirContents(..)
        | Message::FetchDirContentsResult(..)
        | Message::FetchReleaseNotes
        | Message::FetchReleaseNotesResult(..)
        | Message::ShowReadme
        | Message::PreviewRepoFile(..)
        | Message::PreviewRepoFileResult(..)
        | Message::QuickInstallPreset(..)
        | Message::SetAddRepoUrl(..)
        | Message::DebouncedResolveAddRepoUrl { .. }
        | Message::RefocusAddRepoUrl
        | Message::ResolveAddRepoUrl
        | Message::OpenModFileInfo(..)
        | Message::FetchDllDescriptionResult(..) => MessageRoute::Repos,

        Message::SetTab(..)
        | Message::SetFilter(..)
        | Message::SetProjectSearch(..)
        | Message::ToggleSort(..)
        | Message::SetLogFilter(..)
        | Message::SetLogSearch(..)
        | Message::ToggleLogWrap(..)
        | Message::ToggleLogAutoScroll(..)
        | Message::ToggleVerboseDiagnostics(..)
        | Message::ToggleLogErrorFetch(..)
        | Message::ToggleLogErrorMisc(..)
        | Message::ClearLogs
        | Message::ExportDiagnostics
        | Message::DiagnosticsExportPathSelected(..)
        | Message::DiagnosticsExported(..)
        | Message::OpenGithubTokenOptions
        | Message::OpenDialog(..)
        | Message::CloseDialog
        | Message::FocusNextDialogField
        | Message::FocusPreviousDialogField
        | Message::ToggleModsWarningDoNotShow(..)
        | Message::AcceptModsWarning
        | Message::TogglePatchesWarningDoNotShow(..)
        | Message::AcceptPatchesWarning
        | Message::RequestExit
        | Message::ShutdownTick
        | Message::ConsumeDialogClick
        | Message::ToggleMenu(..)
        | Message::CloseMenu
        | Message::ToggleAddNewMenu
        | Message::RepoDetailsLoaded(..)
        | Message::ToggleRepoDetailsPath(..)
        | Message::RepoDetailsChildrenLoaded(..)
        | Message::SetCollectionMarqueeHover(..)
        | Message::LogEditorAction(..)
        | Message::ToggleReadmeSourceView
        | Message::ReadmeEditorAction(..)
        | Message::OpenDxvkConfig
        | Message::PromptAwesomeWotlkPatch
        | Message::PromptAwesomeWotlkPatchIfInstalled(..)
        | Message::SetDxvkField(..)
        | Message::SaveDxvkConfig
        | Message::DxvkConfigSaved(..)
        | Message::ToggleDxvkPreview
        | Message::DxvkPreviewEditorAction(..) => MessageRoute::App,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn secret_bearing_messages_route_directly_to_settings() {
        assert_eq!(
            classify(
                &Message::SetGithubTokenInput("sensitive test value".to_string()),
                &None
            ),
            MessageRoute::Settings
        );
    }

    #[test]
    fn archive_hover_routing_respects_the_active_workflow() {
        let message = Message::LocalArchiveHovered(PathBuf::from("package.zip"));
        assert_eq!(
            classify(&message, &Some(Dialog::MpqInstall)),
            MessageRoute::Mpq
        );
        assert_eq!(
            classify(
                &message,
                &Some(Dialog::AddRepo {
                    url: String::new(),
                    mode: "addon".to_string(),
                    is_addons: true,
                    advanced: false,
                })
            ),
            MessageRoute::Repos
        );
    }

    #[test]
    fn coordinator_messages_remain_owned_by_app() {
        assert_eq!(
            classify(&Message::OpenGithubTokenOptions, &None),
            MessageRoute::App
        );
    }
}
