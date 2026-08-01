use crate::components::helpers::tip;
use crate::settings::{self, AppSettings};
use crate::theme::{self, ThemeColors};
use crate::{App, Dialog, LogLevel, Message, ToastKind};
use iced::widget::{button, checkbox, column, container, row, scrollable, text, Space};
use iced::{Element, Length, Task};
use rusqlite::{backup::Backup, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, File};
use std::io::{Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ARCHIVE_FORMAT_VERSION: u32 = 1;
const MANIFEST_NAME: &str = "manifest.json";
const SETTINGS_NAME: &str = "settings.json";
const DATABASES_DIRECTORY: &str = "databases";
const README_NAME: &str = "README.txt";
const PENDING_MARKER_NAME: &str = ".wuddle-restore-pending.json";
const RESTORE_NOTICE_NAME: &str = ".wuddle-restore-complete.json";
const PENDING_RESET_MARKER_NAME: &str = ".wuddle-reset-pending.json";
const RESET_NOTICE_NAME: &str = ".wuddle-reset-complete";
const OLD_INSTALL_SCAN_DEPTH: usize = 5;
const OLD_INSTALL_SCAN_DIRECTORY_LIMIT: usize = 512;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_SETTINGS_BYTES: u64 = 16 * 1024 * 1024;
const MAX_DATABASE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ARCHIVE_CONTENT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreviewSection {
    Profiles,
    Projects,
    Contents,
}

#[derive(Clone)]
enum RestoreSource {
    Archive(PathBuf),
    DataDirectory(PathBuf),
}

impl std::fmt::Debug for RestoreSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Archive(_) => formatter.write_str("Archive(<private path>)"),
            Self::DataDirectory(_) => formatter.write_str("DataDirectory(<private path>)"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectCounts {
    pub addons: u64,
    pub mods: u64,
    pub patches: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestProfile {
    id: String,
    name: String,
    database: Option<String>,
    projects: ProjectCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupManifest {
    format_version: u32,
    created_unix: i64,
    wuddle_version: String,
    profiles: Vec<ManifestProfile>,
    excluded_secrets: Vec<String>,
}

#[derive(Clone)]
pub struct BackupPreview {
    source: RestoreSource,
    fingerprint: [u8; 32],
    pub created_unix: Option<i64>,
    pub source_version: Option<String>,
    pub profiles: Vec<BackupProfilePreview>,
    pub totals: ProjectCounts,
    pub source_kind: &'static str,
}

impl std::fmt::Debug for BackupPreview {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BackupPreview")
            .field("source", &self.source)
            .field("created_unix", &self.created_unix)
            .field("source_version", &self.source_version)
            .field("profile_count", &self.profiles.len())
            .field("totals", &self.totals)
            .field("source_kind", &self.source_kind)
            .field("fingerprint", &"<content digest>")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct BackupProfilePreview {
    pub name: String,
    pub has_database: bool,
    pub projects: ProjectCounts,
}

#[derive(Debug, Clone)]
pub struct ExportSummary {
    pub profiles: usize,
    pub totals: ProjectCounts,
}

#[derive(Debug, Clone)]
pub struct RestoreSchedule {
    pub profiles: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Exporting,
    Inspecting,
    StagingRestore,
    PreparingReset,
}

#[derive(Debug, Default)]
pub struct UiState {
    pub operation: Option<Operation>,
    pub preview: Option<BackupPreview>,
    pub expanded: HashSet<PreviewSection>,
    pub confirming_restore: bool,
    pub confirming_reset: bool,
    pub reset_credentials: bool,
}

impl UiState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn dismissal_blocked(&self) -> bool {
        matches!(
            self.operation,
            Some(Operation::StagingRestore | Operation::PreparingReset)
        )
    }

    pub fn is_busy(&self) -> bool {
        self.operation.is_some()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PendingRestore {
    format_version: u32,
    live_directory_name: String,
    staging_directory_name: String,
    rollback_directory_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RestoreNotice {
    rollback_directory_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PendingReset {
    format_version: u32,
    live_directory_name: String,
}

pub fn default_backup_filename() -> String {
    chrono::Local::now()
        .format("wuddle-backup-%Y%m%d-%H%M%S.zip")
        .to_string()
}

pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::OpenBackupRestore => {
            app.backup_restore_ui.reset();
            app.dialog = Some(Dialog::BackupRestore);
            app.log(LogLevel::Info, "Opened Backup and Restore.");
            Task::none()
        }
        Message::ExportWuddleBackup => {
            if app.backup_restore_ui.is_busy() {
                return Task::none();
            }
            if let Err(error) = app.try_save_settings() {
                app.log(
                    LogLevel::Error,
                    &format!("Backup preparation failed while saving settings: {error}"),
                );
                app.show_toast(
                    "Wuddle could not save its current settings, so no backup was created.",
                    ToastKind::Error,
                );
                return Task::none();
            }
            let filename = default_backup_filename();
            Task::perform(
                async move {
                    rfd::AsyncFileDialog::new()
                        .set_title("Export Wuddle Backup")
                        .set_file_name(&filename)
                        .add_filter("Wuddle backup", &["zip"])
                        .save_file()
                        .await
                        .map(|handle| handle.path().to_path_buf())
                },
                Message::WuddleBackupExportPathSelected,
            )
        }
        Message::WuddleBackupExportPathSelected(path) => {
            let Some(path) = path else {
                return Task::none();
            };
            app.backup_restore_ui.operation = Some(Operation::Exporting);
            app.log(LogLevel::Info, "Creating a full Wuddle backup...");
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || export_backup(&path))
                        .await
                        .map_err(|error| format!("Backup task failed: {error}"))?
                },
                Message::WuddleBackupExported,
            )
        }
        Message::WuddleBackupExported(result) => {
            app.backup_restore_ui.operation = None;
            match result {
                Ok(summary) => {
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "Backup exported: {} profile(s), {} addon(s), {} mod(s), and {} patch package(s).",
                            summary.profiles,
                            summary.totals.addons,
                            summary.totals.mods,
                            summary.totals.patches
                        ),
                    );
                    app.show_toast("Wuddle backup saved.", ToastKind::Success);
                }
                Err(error) => {
                    app.log(LogLevel::Error, &format!("Backup export failed: {error}"));
                    app.show_toast(
                        format!("Could not create backup: {error}"),
                        ToastKind::Error,
                    );
                }
            }
            Task::none()
        }
        Message::PickWuddleBackupArchive => {
            if app.backup_restore_ui.is_busy() {
                return Task::none();
            }
            Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Import Wuddle Backup")
                        .add_filter("Wuddle backup", &["zip"])
                        .pick_file()
                        .await
                        .map(|handle| handle.path().to_path_buf())
                },
                |path| Message::WuddleBackupSourcePicked {
                    path,
                    directory: false,
                },
            )
        }
        Message::PickOldWuddleFolder => {
            if app.backup_restore_ui.is_busy() {
                return Task::none();
            }
            Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose an old Wuddle folder")
                        .pick_folder()
                        .await
                        .map(|handle| handle.path().to_path_buf())
                },
                |path| Message::WuddleBackupSourcePicked {
                    path,
                    directory: true,
                },
            )
        }
        Message::WuddleBackupSourcePicked { path, directory } => {
            let Some(path) = path else {
                return Task::none();
            };
            app.backup_restore_ui.operation = Some(Operation::Inspecting);
            app.backup_restore_ui.preview = None;
            app.backup_restore_ui.confirming_restore = false;
            app.backup_restore_ui.confirming_reset = false;
            app.log(
                LogLevel::Info,
                if directory {
                    "Inspecting an old Wuddle data folder..."
                } else {
                    "Inspecting a Wuddle backup archive..."
                },
            );
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || inspect_source(&path, directory))
                        .await
                        .map_err(|error| format!("Backup inspection task failed: {error}"))?
                },
                Message::WuddleBackupInspected,
            )
        }
        Message::WuddleBackupInspected(result) => {
            app.backup_restore_ui.operation = None;
            match result {
                Ok(preview) => {
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "Backup inspected successfully: {} profile(s), {} tracked project(s).",
                            preview.profiles.len(),
                            preview.totals.addons + preview.totals.mods + preview.totals.patches
                        ),
                    );
                    app.backup_restore_ui.expanded = [
                        PreviewSection::Profiles,
                        PreviewSection::Projects,
                        PreviewSection::Contents,
                    ]
                    .into_iter()
                    .collect();
                    app.backup_restore_ui.preview = Some(preview);
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Backup inspection failed: {error}"),
                    );
                    app.show_toast(format!("Could not read backup: {error}"), ToastKind::Error);
                }
            }
            Task::none()
        }
        Message::ToggleWuddleBackupSection(section) => {
            if !app.backup_restore_ui.expanded.insert(section) {
                app.backup_restore_ui.expanded.remove(&section);
            }
            Task::none()
        }
        Message::RequestWuddleRestore => {
            if app.backup_restore_ui.preview.is_some() && !app.backup_restore_ui.is_busy() {
                app.backup_restore_ui.confirming_restore = true;
                app.backup_restore_ui.confirming_reset = false;
            }
            Task::none()
        }
        Message::CancelWuddleRestore => {
            if !app.backup_restore_ui.dismissal_blocked() {
                app.backup_restore_ui.confirming_restore = false;
            }
            Task::none()
        }
        Message::ConfirmWuddleRestore => {
            if app.backup_restore_ui.is_busy() || !app.backup_restore_ui.confirming_restore {
                return Task::none();
            }
            let Some(preview) = app.backup_restore_ui.preview.clone() else {
                return Task::none();
            };
            app.backup_restore_ui.operation = Some(Operation::StagingRestore);
            app.backup_restore_ui.confirming_restore = false;
            app.log(
                LogLevel::Info,
                "Validating and staging a full Wuddle restore...",
            );
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || schedule_restore(&preview))
                        .await
                        .map_err(|error| format!("Restore staging task failed: {error}"))?
                },
                Message::WuddleRestoreStaged,
            )
        }
        Message::WuddleRestoreStaged(result) => {
            app.backup_restore_ui.operation = None;
            match result {
                Ok(schedule) => {
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "Restore staged for {} profile(s). Restarting Wuddle to apply it.",
                            schedule.profiles
                        ),
                    );
                    app.show_toast(
                        "Restore is ready. Wuddle will restart now.",
                        ToastKind::Success,
                    );
                    return Task::perform(
                        async { restart_for_restore() },
                        Message::WuddleRestoreRestarted,
                    );
                }
                Err(error) => {
                    app.log(LogLevel::Error, &format!("Restore staging failed: {error}"));
                    app.show_toast(
                        format!("Could not stage restore: {error}"),
                        ToastKind::Error,
                    );
                }
            }
            Task::none()
        }
        Message::WuddleRestoreRestarted(result) => {
            if let Err(error) = result {
                app.log(
                    LogLevel::Error,
                    &format!("Could not restart after restore: {error}"),
                );
                app.show_toast(
                    format!(
                        "Restore is staged, but Wuddle could not restart automatically: {error}\n\nClose and reopen Wuddle to finish restoring."
                    ),
                    ToastKind::Error,
                );
            }
            Task::none()
        }
        Message::RequestWuddleReset => {
            if !app.backup_restore_ui.is_busy() {
                app.backup_restore_ui.confirming_restore = false;
                app.backup_restore_ui.confirming_reset = true;
                app.backup_restore_ui.reset_credentials = true;
            }
            Task::none()
        }
        Message::CancelWuddleReset => {
            if !app.backup_restore_ui.dismissal_blocked() {
                app.backup_restore_ui.confirming_reset = false;
            }
            Task::none()
        }
        Message::ToggleWuddleResetCredentials(remove) => {
            if app.backup_restore_ui.confirming_reset && !app.backup_restore_ui.is_busy() {
                app.backup_restore_ui.reset_credentials = remove;
            }
            Task::none()
        }
        Message::ConfirmWuddleReset => {
            if app.backup_restore_ui.is_busy() || !app.backup_restore_ui.confirming_reset {
                return Task::none();
            }
            app.backup_restore_ui.operation = Some(Operation::PreparingReset);
            app.backup_restore_ui.confirming_reset = false;
            let profiles = app.profiles.clone();
            let reset_credentials = app.backup_restore_ui.reset_credentials;
            app.log(
                LogLevel::Info,
                if reset_credentials {
                    "Preparing a complete Wuddle reset and clearing saved credentials..."
                } else {
                    "Preparing a complete Wuddle reset while retaining system-vault credentials..."
                },
            );
            Task::perform(
                async move { prepare_reset(profiles, reset_credentials).await },
                Message::WuddleResetPrepared,
            )
        }
        Message::WuddleResetPrepared(result) => {
            app.backup_restore_ui.operation = None;
            match result {
                Ok(()) => {
                    app.log(
                        LogLevel::Info,
                        "Reset prepared. Restarting Wuddle to remove its saved data.",
                    );
                    return Task::perform(
                        async { restart_for_restore() },
                        Message::WuddleResetRestarted,
                    );
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Reset preparation failed: {error}"),
                    );
                    app.show_toast(format!("Wuddle was not reset: {error}"), ToastKind::Error);
                }
            }
            Task::none()
        }
        Message::WuddleResetRestarted(result) => {
            if let Err(error) = result {
                app.log(
                    LogLevel::Error,
                    &format!("Could not restart after preparing the reset: {error}"),
                );
                app.show_toast(
                    format!(
                        "Reset is prepared, but Wuddle could not restart automatically: {error}\n\nClose and reopen Wuddle to finish resetting."
                    ),
                    ToastKind::Error,
                );
            }
            Task::none()
        }
        _ => Task::none(),
    }
}

pub fn view_dialog<'a>(app: &'a App, colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;
    let header = row![
        column![
            text("Backup and Restore").size(24).color(colors.title),
            text(
                "Back up Wuddle's settings and tracked project data, or restore a complete backup."
            )
            .size(16)
            .color(colors.muted),
        ]
        .spacing(5),
        Space::new().width(Length::Fill),
        crate::close_button(colors),
    ]
    .align_y(iced::Alignment::Start);

    let operation_text = match app.backup_restore_ui.operation {
        Some(Operation::Exporting) => Some("Creating backup..."),
        Some(Operation::Inspecting) => Some("Inspecting backup..."),
        Some(Operation::StagingRestore) => Some("Validating and staging restore..."),
        Some(Operation::PreparingReset) if app.backup_restore_ui.reset_credentials => {
            Some("Clearing credentials and preparing reset...")
        }
        Some(Operation::PreparingReset) => {
            Some("Preparing reset while keeping system-vault credentials...")
        }
        None => None,
    };
    let busy = app.backup_restore_ui.is_busy();
    #[cfg(feature = "auto-login")]
    let reset_credential_text = "The confirmation lets you either remove or retain saved GitHub and auto-login credentials in the operating system vault.";
    #[cfg(not(feature = "auto-login"))]
    let reset_credential_text = "The confirmation lets you either remove or retain the saved GitHub token. This build does not include the optional auto-login vault capability.";

    let mut body = column![
        section_card(
            column![
                text("Create a backup").size(17).color(colors.title),
                text("Includes profiles, preferences, launch configuration, and tracked addon, mod, and patch metadata.")
                    .size(14)
                    .color(colors.muted),
                text("GitHub tokens and auto-login passwords stay in the operating system vault and are never included.")
                    .size(14)
                    .color(colors.warn),
                tip(
                    styled_button("Export backup...", colors, !busy)
                        .on_press_maybe((!busy).then_some(Message::ExportWuddleBackup)),
                    "Export Wuddle settings and tracked project data.\n\nSaves as wuddle-backup-YYYYMMDD-HHMMSS.zip.",
                    iced::widget::tooltip::Position::Top,
                    colors,
                ),
            ]
            .spacing(8),
            colors,
        ),
        section_card(
            column![
                text("Restore Wuddle").size(17).color(colors.title),
                text("Choose a Wuddle backup ZIP, or select the main folder of an older Wuddle installation. Wuddle will locate its data folder automatically.")
                    .size(14)
                    .color(colors.muted),
                row![
                    tip(
                        styled_button("Choose backup ZIP...", colors, !busy)
                            .on_press_maybe((!busy).then_some(Message::PickWuddleBackupArchive)),
                        "Import settings from a previous Wuddle backup ZIP.",
                        iced::widget::tooltip::Position::Top,
                        colors,
                    ),
                    tip(
                        styled_button("Choose old Wuddle folder...", colors, !busy)
                            .on_press_maybe((!busy).then_some(Message::PickOldWuddleFolder)),
                        "Import settings from a previous Wuddle installation.\n\nThis also supports versions from before\nBackup and Restore was introduced.",
                        iced::widget::tooltip::Position::Top,
                        colors,
                    ),
                ]
                .spacing(8),
            ]
            .spacing(8),
            colors,
        ),
        section_card(
            column![
                text("Reset Wuddle").size(17).color(colors.bad),
                text("Remove every current and known legacy Wuddle setting, profile database, cache, and diagnostic file, then restart as a fresh installation.")
                    .size(14)
                    .color(colors.text),
                text(format!("Installed WoW, addon, mod, and MPQ files are not deleted. {reset_credential_text}"))
                    .size(14)
                    .color(colors.warn),
                {
                    let c2 = colors;
                    let reset: button::Button<'_, Message> =
                        button(text("Reset Wuddle...").size(14))
                            .padding([8, 14])
                            .style(move |_theme, _status| theme::btn_danger_style(c2));
                    let reset: Element<Message> = if busy {
                        reset.into()
                    } else {
                        reset.on_press(Message::RequestWuddleReset).into()
                    };
                    tip(
                        reset,
                        "Open a warning and confirmation before resetting Wuddle.\n\nNothing is removed until you confirm.",
                        iced::widget::tooltip::Position::Top,
                        colors,
                    )
                },
            ]
            .spacing(8),
            colors,
        ),
    ]
    .spacing(10);

    if let Some(status) = operation_text {
        body = body.push(text(status).size(14).color(colors.warn));
    }

    if let Some(preview) = app.backup_restore_ui.preview.as_ref() {
        let profile_rows = preview
            .profiles
            .iter()
            .map(|profile| {
                let db = if profile.has_database {
                    "tracked data included"
                } else {
                    "no profile database"
                };
                text(format!("• {} — {db}", profile.name))
                    .size(14)
                    .color(c.text)
                    .into()
            })
            .collect::<Vec<Element<Message>>>();

        let source_detail = match (&preview.source_version, preview.created_unix) {
            (Some(version), Some(created)) => format!(
                "{} created by Wuddle v{} at {}",
                preview.source_kind,
                version,
                format_timestamp(created)
            ),
            _ => format!(
                "{} from an existing Wuddle data folder",
                preview.source_kind
            ),
        };
        let profiles_content: Element<Message> = if app
            .backup_restore_ui
            .expanded
            .contains(&PreviewSection::Profiles)
        {
            column(profile_rows).spacing(4).into()
        } else {
            Space::new().height(0).into()
        };
        let projects_content: Element<Message> = if app
            .backup_restore_ui
            .expanded
            .contains(&PreviewSection::Projects)
        {
            column![
                text(format!("• Addons: {}", preview.totals.addons)).size(14),
                text(format!("• Mods: {}", preview.totals.mods)).size(14),
                text(format!("• Patch packages: {}", preview.totals.patches)).size(14),
            ]
            .spacing(4)
            .into()
        } else {
            Space::new().height(0).into()
        };
        let contents_content: Element<Message> = if app
            .backup_restore_ui
            .expanded
            .contains(&PreviewSection::Contents)
        {
            column![
                text("• Application preferences and window settings").size(14),
                text("• Profile names, game paths, visible tabs, and launch configuration")
                    .size(14),
                text("• Tracked repositories, installed-file records, ignored updates, and MPQ metadata")
                    .size(14),
                text("• No addon, mod, patch, or game files are copied").size(14),
                text("• No GitHub token or auto-login password is included")
                    .size(14)
                    .color(colors.warn),
            ]
            .spacing(4)
            .into()
        } else {
            Space::new().height(0).into()
        };

        body = body.push(section_card(
            column![
                text("Restore preview").size(17).color(colors.title),
                text(source_detail).size(14).color(colors.muted),
                collapsible_header(
                    "Profiles",
                    PreviewSection::Profiles,
                    app.backup_restore_ui
                        .expanded
                        .contains(&PreviewSection::Profiles),
                    colors,
                ),
                profiles_content,
                collapsible_header(
                    "Tracked projects",
                    PreviewSection::Projects,
                    app.backup_restore_ui
                        .expanded
                        .contains(&PreviewSection::Projects),
                    colors,
                ),
                projects_content,
                collapsible_header(
                    "What will be restored",
                    PreviewSection::Contents,
                    app.backup_restore_ui
                        .expanded
                        .contains(&PreviewSection::Contents),
                    colors,
                ),
                contents_content,
            ]
            .spacing(8),
            colors,
        ));
    }

    if app.backup_restore_ui.confirming_restore {
        body = body.push(section_card(
            column![
                text("Replace current Wuddle data?")
                    .size(17)
                    .color(colors.warn),
                text("This restores the complete backup and restarts Wuddle. Current profiles and settings will be replaced together.")
                    .size(14)
                    .color(colors.text),
                text("Your current Wuddle data will be kept in a separate rollback folder and will not be deleted automatically.")
                    .size(14)
                    .color(colors.muted),
            ]
            .spacing(7),
            colors,
        ));
    }

    if app.backup_restore_ui.confirming_reset {
        #[cfg(feature = "auto-login")]
        let credential_checkbox_label = "Also remove saved GitHub and auto-login credentials";
        #[cfg(not(feature = "auto-login"))]
        let credential_checkbox_label = "Also remove the saved GitHub token";

        let credential_consequence = if app.backup_restore_ui.reset_credentials {
            "The selected credentials will be permanently removed from the operating system vault and must be entered again after a restore."
        } else {
            "Credentials will remain in the operating system vault. A later import on this computer can reconnect to them through the restored non-secret references."
        };

        body = body.push(section_card(
            column![
                text("Permanently reset Wuddle?")
                    .size(17)
                    .color(colors.bad),
                text("This permanently removes all Wuddle profiles, preferences, tracked-project databases, logs, caches, and known legacy Wuddle data after restarting.")
                    .size(14)
                    .color(colors.text),
                checkbox(app.backup_restore_ui.reset_credentials)
                    .label(credential_checkbox_label)
                    .on_toggle(Message::ToggleWuddleResetCredentials)
                    .text_size(14),
                text(credential_consequence)
                    .size(14)
                    .color(colors.warn),
                text("Game installations and their deployed addon, mod, and MPQ files remain untouched.")
                    .size(14)
                    .color(colors.muted),
                text("Export a backup first if you may want this data later. This reset does not create a rollback copy.")
                    .size(14)
                    .color(colors.bad),
            ]
            .spacing(7),
            colors,
        ));
    }

    let restore_button: Element<Message> = if app.backup_restore_ui.confirming_restore {
        if !busy {
            let c2 = colors;
            button(text("Replace and restart").size(14))
                .on_press(Message::ConfirmWuddleRestore)
                .padding([8, 14])
                .style(move |_theme, _status| theme::tab_button_active_style(c2))
                .into()
        } else {
            styled_button("Replace and restart", colors, false).into()
        }
    } else {
        Space::new().width(0).into()
    };

    let cancel_restore: Element<Message> = if app.backup_restore_ui.confirming_restore && !busy {
        styled_button("Cancel", colors, true)
            .on_press(Message::CancelWuddleRestore)
            .into()
    } else {
        Space::new().width(0).into()
    };

    let begin_restore: Element<Message> = if app.backup_restore_ui.preview.is_some()
        && !app.backup_restore_ui.confirming_restore
        && !app.backup_restore_ui.confirming_reset
        && !busy
    {
        let c2 = colors;
        button(text("Restore and restart").size(14))
            .on_press(Message::RequestWuddleRestore)
            .padding([8, 14])
            .style(move |_theme, _status| theme::tab_button_active_style(c2))
            .into()
    } else if app.backup_restore_ui.confirming_restore {
        Space::new().width(0).into()
    } else if app.backup_restore_ui.preview.is_none() {
        tip(
            styled_button("Restore and restart", colors, false),
            "Choose a backup ZIP or an old Wuddle folder first.\n\nWuddle will inspect it before Restore and restart becomes available.",
            iced::widget::tooltip::Position::Top,
            colors,
        )
    } else {
        styled_button("Restore and restart", colors, false).into()
    };

    let cancel_reset: Element<Message> = if app.backup_restore_ui.confirming_reset && !busy {
        styled_button("Cancel", colors, true)
            .on_press(Message::CancelWuddleReset)
            .into()
    } else {
        Space::new().width(0).into()
    };

    let confirm_reset: Element<Message> = if app.backup_restore_ui.confirming_reset && !busy {
        let c2 = colors;
        button(text("Reset and restart").size(14))
            .on_press(Message::ConfirmWuddleReset)
            .padding([8, 14])
            .style(move |_theme, _status| theme::btn_danger_style(c2))
            .into()
    } else {
        Space::new().width(0).into()
    };

    let footer = row![
        Space::new().width(Length::Fill),
        {
            let close = styled_button("Close", colors, !app.backup_restore_ui.dismissal_blocked());
            let close: Element<Message> = if app.backup_restore_ui.dismissal_blocked() {
                close.into()
            } else {
                close.on_press(Message::CloseDialog).into()
            };
            close
        },
        cancel_reset,
        confirm_reset,
        cancel_restore,
        begin_restore,
        restore_button,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    column![
        header,
        iced::widget::rule::horizontal(1),
        scrollable(body)
            .height(Length::Fill)
            .direction(theme::vscroll())
            .style(move |t, s| theme::scrollable_style(c)(t, s)),
        iced::widget::rule::horizontal(1),
        footer,
    ]
    .spacing(12)
    .height(Length::Fill)
    .into()
}

fn section_card<'a>(
    content: impl Into<Element<'a, Message>>,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;
    container(content)
        .padding(12)
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(c))
        .into()
}

fn styled_button<'a>(
    label: &'a str,
    colors: ThemeColors,
    enabled: bool,
) -> button::Button<'a, Message> {
    let c = colors;
    button(text(label).size(14))
        .padding([8, 14])
        .style(move |_theme, status| {
            if !enabled {
                button::Style {
                    background: Some(iced::Background::Color(iced::Color { a: 0.34, ..c.card })),
                    text_color: iced::Color { a: 0.42, ..c.muted },
                    border: iced::Border {
                        color: iced::Color {
                            a: 0.35,
                            ..c.btn_border
                        },
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                }
            } else if matches!(status, button::Status::Hovered) {
                theme::tab_button_hovered_style(c)
            } else {
                theme::tab_button_style(c)
            }
        })
}

fn collapsible_header<'a>(
    label: &'a str,
    section: PreviewSection,
    expanded: bool,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;
    button(
        row![
            text(if expanded { "⌄" } else { "›" }).size(17),
            text(label).size(15),
            Space::new().width(Length::Fill),
        ]
        .spacing(7)
        .align_y(iced::Alignment::Center),
    )
    .on_press(Message::ToggleWuddleBackupSection(section))
    .width(Length::Fill)
    .padding([6, 8])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::tab_button_hovered_style(c),
        _ => theme::tab_button_style(c),
    })
    .into()
}

pub fn export_backup(destination: &Path) -> Result<ExportSummary, String> {
    let app_dir = settings::app_dir()?;
    export_backup_from(&app_dir, destination)
}

fn export_backup_from(app_dir: &Path, destination: &Path) -> Result<ExportSummary, String> {
    let source_settings = read_regular_file(&app_dir.join(SETTINGS_NAME), MAX_SETTINGS_BYTES)?;
    let mut parsed: AppSettings = serde_json::from_slice(&source_settings)
        .map_err(|error| format!("Current settings are invalid: {error}"))?;
    normalize_settings_for_restore(&mut parsed)?;

    let temporary = tempfile::tempdir()
        .map_err(|error| format!("Could not create backup staging storage: {error}"))?;
    let staged_data = temporary.path().join("data");
    fs::create_dir_all(staged_data.join(DATABASES_DIRECTORY))
        .map_err(|error| format!("Could not create backup staging directory: {error}"))?;
    write_json_file(&staged_data.join(SETTINGS_NAME), &parsed)?;

    let mut profiles = Vec::with_capacity(parsed.profiles.len());
    let mut totals = ProjectCounts::default();
    for profile in &parsed.profiles {
        let database_name = database_name_for_profile(&profile.id)?;
        let source = app_dir.join(&database_name);
        let staged = staged_data.join(DATABASES_DIRECTORY).join(&database_name);
        let (database, projects) = if source.is_file() {
            ensure_regular_file(&source)?;
            snapshot_database(&source, &staged)?;
            let counts = summarize_database(&staged)?;
            add_counts(&mut totals, &counts);
            (Some(database_name), counts)
        } else {
            (None, ProjectCounts::default())
        };
        profiles.push(ManifestProfile {
            id: profile.id.clone(),
            name: profile.name.clone(),
            database,
            projects,
        });
    }

    let manifest = BackupManifest {
        format_version: ARCHIVE_FORMAT_VERSION,
        created_unix: now_unix(),
        wuddle_version: env!("CARGO_PKG_VERSION").to_string(),
        profiles,
        excluded_secrets: vec![
            "GitHub token (stored in the operating system credential vault)".to_string(),
            "Auto-login passwords (stored in the operating system credential vault)".to_string(),
        ],
    };
    write_json_file(&staged_data.join(MANIFEST_NAME), &manifest)?;

    let parent = destination
        .parent()
        .ok_or_else(|| "The selected backup destination has no parent directory.".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create the backup destination: {error}"))?;
    let mut output = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("Could not create the temporary backup archive: {error}"))?;
    write_backup_zip(output.as_file_mut(), &staged_data, &manifest)?;
    output
        .as_file_mut()
        .sync_all()
        .map_err(|error| format!("Could not synchronize the backup archive: {error}"))?;
    output
        .persist(destination)
        .map_err(|error| format!("Could not save the backup archive: {}", error.error))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(destination, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("Could not secure the backup archive permissions: {error}"))?;
        sync_directory(parent)?;
    }

    Ok(ExportSummary {
        profiles: parsed.profiles.len(),
        totals,
    })
}

fn write_backup_zip(
    output: &mut File,
    staged_data: &Path,
    manifest: &BackupManifest,
) -> Result<(), String> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o600);
    let mut archive = zip::ZipWriter::new(output);
    archive
        .start_file(MANIFEST_NAME, options)
        .map_err(|error| format!("Could not add the backup manifest: {error}"))?;
    archive
        .write_all(
            &serde_json::to_vec_pretty(manifest)
                .map_err(|error| format!("Could not serialize the backup manifest: {error}"))?,
        )
        .map_err(|error| format!("Could not write the backup manifest: {error}"))?;

    archive
        .start_file(SETTINGS_NAME, options)
        .map_err(|error| format!("Could not add settings to the backup: {error}"))?;
    let settings_bytes = fs::read(staged_data.join(SETTINGS_NAME))
        .map_err(|error| format!("Could not read staged settings: {error}"))?;
    archive
        .write_all(&settings_bytes)
        .map_err(|error| format!("Could not write settings into the backup: {error}"))?;

    archive
        .add_directory(format!("{DATABASES_DIRECTORY}/"), options)
        .map_err(|error| format!("Could not add the database directory: {error}"))?;
    for profile in &manifest.profiles {
        let Some(database) = profile.database.as_deref() else {
            continue;
        };
        archive
            .start_file(format!("{DATABASES_DIRECTORY}/{database}"), options)
            .map_err(|error| format!("Could not add a profile database: {error}"))?;
        let mut database_file = File::open(staged_data.join(DATABASES_DIRECTORY).join(database))
            .map_err(|error| format!("Could not read a staged profile database: {error}"))?;
        std::io::copy(&mut database_file, &mut archive)
            .map_err(|error| format!("Could not write a profile database: {error}"))?;
    }

    archive
        .start_file(README_NAME, options)
        .map_err(|error| format!("Could not add the backup explanation: {error}"))?;
    archive
        .write_all(BACKUP_README.as_bytes())
        .map_err(|error| format!("Could not write the backup explanation: {error}"))?;
    archive
        .finish()
        .map_err(|error| format!("Could not finish the backup archive: {error}"))?;
    Ok(())
}

const BACKUP_README: &str = "Wuddle settings backup\n\nThis archive contains Wuddle settings and per-profile SQLite metadata. It may contain personal paths, repository URLs, and launch configuration. It does not contain game files, installed addons/mods/MPQs, GitHub tokens, or auto-login passwords. Restore it through Wuddle's Backup and Restore dialog.\n";

pub fn inspect_source(path: &Path, directory: bool) -> Result<BackupPreview, String> {
    let source = if directory {
        RestoreSource::DataDirectory(resolve_old_data_directory(path)?)
    } else {
        if !path.is_file() {
            return Err("The selected backup archive no longer exists.".to_string());
        }
        RestoreSource::Archive(path.to_path_buf())
    };
    let temporary = tempfile::tempdir()
        .map_err(|error| format!("Could not create backup inspection storage: {error}"))?;
    let metadata = materialize_source(&source, temporary.path())?;
    build_preview(
        source,
        &metadata.settings,
        metadata.manifest.as_ref(),
        temporary.path(),
    )
}

struct MaterializedSource {
    settings: AppSettings,
    manifest: Option<BackupManifest>,
}

fn materialize_source(
    source: &RestoreSource,
    destination: &Path,
) -> Result<MaterializedSource, String> {
    fs::create_dir_all(destination.join(DATABASES_DIRECTORY))
        .map_err(|error| format!("Could not create restore staging storage: {error}"))?;
    match source {
        RestoreSource::Archive(path) => extract_backup_archive(path, destination),
        RestoreSource::DataDirectory(path) => copy_data_directory(path, destination),
    }
}

fn extract_backup_archive(path: &Path, destination: &Path) -> Result<MaterializedSource, String> {
    ensure_regular_file(path)?;
    let file =
        File::open(path).map_err(|error| format!("Could not open backup archive: {error}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("The selected file is not a readable ZIP archive: {error}"))?;
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("Could not inspect backup entry: {error}"))?;
        let name = safe_archive_name(entry.name())?;
        if !seen.insert(name.clone()) {
            return Err(format!("The backup contains a duplicate entry: {name}"));
        }
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            if kind != 0 && kind != 0o100000 && kind != 0o040000 {
                return Err(format!(
                    "The backup contains a linked or special entry: {name}"
                ));
            }
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| "The backup's declared size is invalid.".to_string())?;
        if total > MAX_ARCHIVE_CONTENT_BYTES {
            return Err("The backup is larger than Wuddle's safety limit.".to_string());
        }
    }

    let manifest_bytes = read_zip_entry(&mut archive, MANIFEST_NAME, MAX_MANIFEST_BYTES)?;
    let manifest: BackupManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("The backup manifest is invalid: {error}"))?;
    if manifest.format_version != ARCHIVE_FORMAT_VERSION {
        return Err(format!(
            "This backup uses format version {}, but this Wuddle build supports version {}.",
            manifest.format_version, ARCHIVE_FORMAT_VERSION
        ));
    }
    let settings_bytes = read_zip_entry(&mut archive, SETTINGS_NAME, MAX_SETTINGS_BYTES)?;
    let mut parsed: AppSettings = serde_json::from_slice(&settings_bytes)
        .map_err(|error| format!("The backup settings are invalid: {error}"))?;
    normalize_settings_for_restore(&mut parsed)?;
    verify_manifest_matches_settings(&manifest, &parsed)?;

    let allowed = allowed_archive_entries(&manifest)?;
    for name in &seen {
        if !allowed.contains(name) {
            return Err(format!("The backup contains an unexpected entry: {name}"));
        }
    }
    write_json_file(&destination.join(SETTINGS_NAME), &parsed)?;
    for profile in &manifest.profiles {
        let Some(database) = profile.database.as_deref() else {
            continue;
        };
        let archive_name = format!("{DATABASES_DIRECTORY}/{database}");
        let target = destination.join(DATABASES_DIRECTORY).join(database);
        extract_zip_entry(&mut archive, &archive_name, &target, MAX_DATABASE_BYTES)?;
        verify_database(&target)?;
    }
    Ok(MaterializedSource {
        settings: parsed,
        manifest: Some(manifest),
    })
}

fn copy_data_directory(source: &Path, destination: &Path) -> Result<MaterializedSource, String> {
    fs::create_dir_all(destination.join(DATABASES_DIRECTORY))
        .map_err(|error| format!("Could not create restore database staging storage: {error}"))?;
    let settings_bytes = read_regular_file(&source.join(SETTINGS_NAME), MAX_SETTINGS_BYTES)?;
    let mut parsed: AppSettings = serde_json::from_slice(&settings_bytes)
        .map_err(|error| format!("The old Wuddle settings are invalid: {error}"))?;
    normalize_settings_for_restore(&mut parsed)?;
    write_json_file(&destination.join(SETTINGS_NAME), &parsed)?;
    for profile in &parsed.profiles {
        let database = database_name_for_profile(&profile.id)?;
        let source_db = source.join(&database);
        if source_db.exists() {
            ensure_regular_file(&source_db)?;
            snapshot_database(
                &source_db,
                &destination.join(DATABASES_DIRECTORY).join(&database),
            )?;
        }
    }
    Ok(MaterializedSource {
        settings: parsed,
        manifest: None,
    })
}

fn resolve_old_data_directory(selected: &Path) -> Result<PathBuf, String> {
    if !selected.is_dir() {
        return Err("The selected old Wuddle folder no longer exists.".to_string());
    }
    if selected.join(SETTINGS_NAME).is_file() {
        return Ok(selected.to_path_buf());
    }
    let authoritative = selected.join("wuddle-data");
    if authoritative.join(SETTINGS_NAME).is_file() {
        return Ok(authoritative);
    }

    let mut queue = VecDeque::from([(selected.to_path_buf(), 0usize)]);
    let mut candidates = Vec::new();
    let mut inspected = 0usize;
    while let Some((directory, depth)) = queue.pop_front() {
        if depth >= OLD_INSTALL_SCAN_DEPTH {
            continue;
        }
        let entries = fs::read_dir(&directory).map_err(|error| {
            format!("Could not inspect the selected old Wuddle folder: {error}")
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!("Could not inspect an entry in the old Wuddle folder: {error}")
            })?;
            let file_type = entry.file_type().map_err(|error| {
                format!("Could not inspect an old Wuddle folder entry: {error}")
            })?;
            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }
            inspected += 1;
            if inspected > OLD_INSTALL_SCAN_DIRECTORY_LIMIT {
                return Err(
                    "The selected folder contains too many subfolders to search safely. Select its Wuddle data folder more directly."
                        .to_string(),
                );
            }
            let child = entry.path();
            if child.join(SETTINGS_NAME).is_file() {
                candidates.push(child.clone());
            }
            queue.push_back((child, depth + 1));
        }
    }
    match candidates.len() {
        0 => Err(
            "No Wuddle settings.json was found within five folders. Select the old Wuddle installation, or its exact wuddle-data folder."
                .to_string(),
        ),
        1 => Ok(candidates.remove(0)),
        _ => Err(
            "Several old Wuddle data folders were found. Select the exact installation or wuddle-data folder you want to import."
                .to_string(),
        ),
    }
}

fn build_preview(
    source: RestoreSource,
    settings: &AppSettings,
    manifest: Option<&BackupManifest>,
    materialized: &Path,
) -> Result<BackupPreview, String> {
    let manifest_by_id = manifest
        .map(|manifest| {
            manifest
                .profiles
                .iter()
                .map(|profile| (profile.id.as_str(), profile))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let mut profiles = Vec::with_capacity(settings.profiles.len());
    let mut totals = ProjectCounts::default();
    for profile in &settings.profiles {
        let database = database_name_for_profile(&profile.id)?;
        let database_path = materialized.join(DATABASES_DIRECTORY).join(&database);
        let has_database = database_path.is_file();
        let projects = if has_database {
            summarize_database(&database_path)?
        } else {
            ProjectCounts::default()
        };
        if let Some(expected) = manifest_by_id.get(profile.id.as_str()) {
            if expected.projects != projects {
                return Err(format!(
                    "The backup project summary does not match profile '{}'.",
                    profile.name
                ));
            }
        }
        add_counts(&mut totals, &projects);
        profiles.push(BackupProfilePreview {
            name: profile.name.clone(),
            has_database,
            projects,
        });
    }
    Ok(BackupPreview {
        source,
        fingerprint: fingerprint_materialized(settings, materialized)?,
        created_unix: manifest.map(|manifest| manifest.created_unix),
        source_version: manifest.map(|manifest| manifest.wuddle_version.clone()),
        profiles,
        totals,
        source_kind: if manifest.is_some() {
            "Backup"
        } else {
            "Import"
        },
    })
}

pub fn schedule_restore(preview: &BackupPreview) -> Result<RestoreSchedule, String> {
    let live = settings::app_dir()?;
    schedule_restore_at(preview, &live)
}

fn schedule_restore_at(preview: &BackupPreview, live: &Path) -> Result<RestoreSchedule, String> {
    let parent = live
        .parent()
        .ok_or_else(|| "Wuddle's data directory has no parent directory.".to_string())?;
    let pending_marker = parent.join(PENDING_MARKER_NAME);
    if parent.join(PENDING_RESET_MARKER_NAME).exists() {
        return Err(
            "A Wuddle reset is already prepared. Restart Wuddle before restoring a backup."
                .to_string(),
        );
    }
    if pending_marker.exists() {
        return Err(
            "Another Wuddle restore is already staged. Restart Wuddle to apply it before preparing another restore."
                .to_string(),
        );
    }
    let live_name = safe_file_name(live)?;
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let staging_name = format!(".{live_name}-restore-stage-{nonce}");
    let rollback_name = unique_rollback_name(parent, &live_name);
    let staging = parent.join(&staging_name);
    fs::create_dir(&staging)
        .map_err(|error| format!("Could not create restore staging directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("Could not secure restore staging storage: {error}"))?;
    }

    let result = (|| {
        let materialized = materialize_source(&preview.source, &staging)?;
        let rebuilt = build_preview(
            preview.source.clone(),
            &materialized.settings,
            materialized.manifest.as_ref(),
            &staging,
        )?;
        if rebuilt.profiles.len() != preview.profiles.len()
            || rebuilt.totals != preview.totals
            || rebuilt.fingerprint != preview.fingerprint
        {
            return Err("The selected backup changed after it was inspected. Inspect it again before restoring.".to_string());
        }
        promote_databases_to_live_layout(&staging, &materialized.settings)?;
        write_json_file(
            &staging.join(RESTORE_NOTICE_NAME),
            &RestoreNotice {
                rollback_directory_name: rollback_name.clone(),
            },
        )?;
        sync_directory(&staging)?;
        let pending = PendingRestore {
            format_version: ARCHIVE_FORMAT_VERSION,
            live_directory_name: live_name,
            staging_directory_name: staging_name,
            rollback_directory_name: rollback_name,
        };
        write_json_atomic(&pending_marker, &pending)?;
        Ok(RestoreSchedule {
            profiles: rebuilt.profiles.len(),
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

async fn prepare_reset(
    profiles: Vec<settings::ProfileConfig>,
    reset_credentials: bool,
) -> Result<(), String> {
    if !reset_credentials {
        return tokio::task::spawn_blocking(schedule_reset)
            .await
            .map_err(|error| format!("Reset preparation task failed: {error}"))?;
    }

    #[cfg(feature = "auto-login")]
    for profile in profiles {
        if !profile.auto_login_accounts.is_empty()
            || !profile.pending_auto_login_deletion_ids.is_empty()
        {
            crate::auto_login::delete_profile_credentials_for_reset(
                profile.id.clone(),
                profile.auto_login_accounts,
                profile.pending_auto_login_deletion_ids,
            )
            .await
            .map_err(|error| {
                format!(
                    "Saved auto-login credentials could not be cleared, so the reset was stopped: {error}"
                )
            })?;
        }
    }
    #[cfg(not(feature = "auto-login"))]
    let _ = profiles;

    crate::service::clear_github_token()
        .await
        .map_err(|error| {
            format!(
                "The saved GitHub token could not be cleared, so the reset was stopped: {error}"
            )
        })?;

    tokio::task::spawn_blocking(schedule_reset)
        .await
        .map_err(|error| format!("Reset preparation task failed: {error}"))?
}

fn schedule_reset() -> Result<(), String> {
    let live = settings::app_dir()?;
    let parent = live
        .parent()
        .ok_or_else(|| "Wuddle's data directory has no parent directory.".to_string())?;
    if parent.join(PENDING_MARKER_NAME).exists() {
        return Err(
            "A Wuddle restore is already staged. Restart Wuddle before resetting it.".to_string(),
        );
    }
    let marker = parent.join(PENDING_RESET_MARKER_NAME);
    if marker.exists() {
        return Err("A Wuddle reset is already prepared. Restart Wuddle to apply it.".to_string());
    }
    write_json_atomic(
        &marker,
        &PendingReset {
            format_version: ARCHIVE_FORMAT_VERSION,
            live_directory_name: safe_file_name(&live)?,
        },
    )
}

pub fn apply_pending_reset() -> Result<bool, String> {
    let live = settings::app_dir()?;
    let legacy = known_legacy_data_directories(&live);
    apply_pending_reset_at(&live, &legacy)
}

fn apply_pending_reset_at(live: &Path, legacy: &[PathBuf]) -> Result<bool, String> {
    let parent = live
        .parent()
        .ok_or_else(|| "Wuddle's data directory has no parent directory.".to_string())?;
    let marker = parent.join(PENDING_RESET_MARKER_NAME);
    if !marker.is_file() {
        return Ok(false);
    }
    ensure_regular_file(&marker)?;
    let pending_bytes = read_regular_file(&marker, MAX_MANIFEST_BYTES)?;
    let pending: PendingReset = serde_json::from_slice(&pending_bytes)
        .map_err(|error| format!("The pending reset marker is invalid: {error}"))?;
    if pending.format_version != ARCHIVE_FORMAT_VERSION {
        return Err("The pending reset was created by an incompatible Wuddle version.".to_string());
    }
    validate_simple_name(&pending.live_directory_name)?;
    if safe_file_name(live)? != pending.live_directory_name {
        return Err("The pending reset targets a different Wuddle data directory.".to_string());
    }

    let mut targets = Vec::with_capacity(legacy.len() + 1);
    targets.push(live.to_path_buf());
    for path in legacy {
        if !targets.iter().any(|known| known == path) {
            targets.push(path.clone());
        }
    }
    for target in targets {
        remove_reset_target(&target)?;
    }

    fs::create_dir_all(live)
        .map_err(|error| format!("Could not recreate Wuddle's empty data directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(live, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("Could not secure Wuddle's new data directory: {error}"))?;
    }
    File::create(live.join(RESET_NOTICE_NAME))
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("Could not record reset completion: {error}"))?;
    fs::remove_file(&marker)
        .map_err(|error| format!("Could not clear the one-time reset marker: {error}"))?;
    sync_directory(parent)?;
    Ok(true)
}

fn remove_reset_target(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("Could not inspect saved Wuddle data: {error}")),
    };
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        #[cfg(target_os = "windows")]
        if metadata.is_dir() {
            return fs::remove_dir(path)
                .map_err(|error| format!("Could not remove a linked Wuddle data folder: {error}"));
        }
        return fs::remove_file(path)
            .map_err(|error| format!("Could not remove a linked Wuddle data path: {error}"));
    }
    if metadata.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Could not remove saved Wuddle data: {error}"))
    } else {
        fs::remove_file(path)
            .map_err(|error| format!("Could not remove a saved Wuddle data file: {error}"))
    }
}

fn known_legacy_data_directories(live: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let (Some(parent), Ok(live_name)) = (live.parent(), safe_file_name(live)) {
        if let Ok(entries) = fs::read_dir(parent) {
            let rollback_prefix = format!("{live_name}-before-restore-");
            let restore_stage_prefix = format!(".{live_name}-restore-stage-");
            let migration_prefix = format!(".{live_name}-migration-");
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if (name.starts_with(&rollback_prefix)
                    || name.starts_with(&restore_stage_prefix)
                    || name.starts_with(&migration_prefix))
                    && entry
                        .file_type()
                        .is_ok_and(|kind| kind.is_dir() || kind.is_symlink())
                {
                    paths.push(entry.path());
                }
            }
        }
    }
    if let Some(data_dir) = dirs::data_dir() {
        paths.push(data_dir.join("wuddle"));
        paths.push(data_dir.join("io.github.zythdr.wuddle"));
    }

    #[cfg(target_os = "windows")]
    if let Ok((root, launcher_layout)) = crate::self_update::detect_windows_launcher_root() {
        if launcher_layout {
            let versions = root.join("versions");
            paths.push(versions.join("wuddle-data"));
            if let Ok(entries) = fs::read_dir(&versions) {
                for entry in entries.flatten() {
                    if entry
                        .file_type()
                        .is_ok_and(|kind| kind.is_dir() && !kind.is_symlink())
                    {
                        paths.push(entry.path().join("wuddle-data"));
                    }
                }
            }
        }
    }

    paths.retain(|path| path != live);
    paths.sort();
    paths.dedup();
    paths
}

pub fn take_reset_notice() -> bool {
    let Some(path) = settings::app_dir()
        .ok()
        .map(|directory| directory.join(RESET_NOTICE_NAME))
    else {
        return false;
    };
    path.is_file() && fs::remove_file(path).is_ok()
}

pub fn apply_pending_restore() -> Result<bool, String> {
    let live = settings::app_dir()?;
    apply_pending_restore_at(&live)
}

fn apply_pending_restore_at(live: &Path) -> Result<bool, String> {
    let parent = live
        .parent()
        .ok_or_else(|| "Wuddle's data directory has no parent directory.".to_string())?;
    let marker = parent.join(PENDING_MARKER_NAME);
    if !marker.is_file() {
        return Ok(false);
    }
    ensure_regular_file(&marker)?;
    let pending_bytes = read_regular_file(&marker, MAX_MANIFEST_BYTES)?;
    let pending: PendingRestore = serde_json::from_slice(&pending_bytes)
        .map_err(|error| format!("The pending restore marker is invalid: {error}"))?;
    if pending.format_version != ARCHIVE_FORMAT_VERSION {
        return Err(
            "The pending restore was created by an incompatible Wuddle version.".to_string(),
        );
    }
    validate_simple_name(&pending.live_directory_name)?;
    validate_simple_name(&pending.staging_directory_name)?;
    validate_simple_name(&pending.rollback_directory_name)?;
    if safe_file_name(live)? != pending.live_directory_name {
        return Err("The pending restore targets a different Wuddle data directory.".to_string());
    }
    let staging = parent.join(&pending.staging_directory_name);
    let rollback = parent.join(&pending.rollback_directory_name);
    if rollback.exists() {
        return Err("The pending restore rollback directory already exists.".to_string());
    }
    validate_staged_restore(&staging)?;

    if live.exists() {
        fs::rename(live, &rollback)
            .map_err(|error| format!("Could not preserve the current Wuddle data: {error}"))?;
    }
    if let Err(error) = fs::rename(&staging, live) {
        let rollback_error = if rollback.exists() {
            fs::rename(&rollback, live).err()
        } else {
            None
        };
        return match rollback_error {
            None => Err(format!(
                "Could not activate the restored Wuddle data; the previous data was put back: {error}"
            )),
            Some(rollback_error) => Err(format!(
                "Could not activate the restored Wuddle data ({error}), and could not automatically put the previous data back ({rollback_error}). The preserved data is in '{}'.",
                pending.rollback_directory_name
            )),
        };
    }
    if let Err(error) = fs::remove_file(&marker) {
        // The restore is already active. Retaining the marker would cause a
        // confusing second attempt, so surface this as a startup failure.
        return Err(format!(
            "The restored data is active, but Wuddle could not clear the one-time restore marker: {error}"
        ));
    }
    sync_directory(parent)?;
    Ok(true)
}

pub fn take_restore_notice() -> Option<String> {
    let path = settings::app_dir().ok()?.join(RESTORE_NOTICE_NAME);
    let bytes = read_regular_file(&path, MAX_MANIFEST_BYTES).ok()?;
    let notice: RestoreNotice = serde_json::from_slice(&bytes).ok()?;
    validate_simple_name(&notice.rollback_directory_name).ok()?;
    fs::remove_file(&path).ok()?;
    Some(format!(
        "Wuddle was restored successfully. Your previous data was kept in '{}'.",
        notice.rollback_directory_name
    ))
}

fn validate_staged_restore(staging: &Path) -> Result<(), String> {
    if !staging.is_dir() {
        return Err("The pending restore staging directory is missing.".to_string());
    }
    let settings_bytes = read_regular_file(&staging.join(SETTINGS_NAME), MAX_SETTINGS_BYTES)?;
    let mut parsed: AppSettings = serde_json::from_slice(&settings_bytes)
        .map_err(|error| format!("The staged restore settings are invalid: {error}"))?;
    normalize_settings_for_restore(&mut parsed)?;
    for profile in &parsed.profiles {
        let database = database_name_for_profile(&profile.id)?;
        let database_path = staging.join(database);
        if database_path.exists() {
            verify_database(&database_path)?;
        }
    }
    if staging.join(DATABASES_DIRECTORY).exists() {
        return Err(
            "The staged restore still contains its temporary database directory.".to_string(),
        );
    }
    read_regular_file(&staging.join(RESTORE_NOTICE_NAME), MAX_MANIFEST_BYTES)?;
    Ok(())
}

fn promote_databases_to_live_layout(staging: &Path, settings: &AppSettings) -> Result<(), String> {
    let databases = staging.join(DATABASES_DIRECTORY);
    for profile in &settings.profiles {
        let database = database_name_for_profile(&profile.id)?;
        let source = databases.join(&database);
        if source.exists() {
            fs::rename(&source, staging.join(&database)).map_err(|error| {
                format!("Could not prepare profile database '{database}' for restore: {error}")
            })?;
        }
    }
    if databases.exists() {
        let mut remaining = fs::read_dir(&databases)
            .map_err(|error| format!("Could not inspect restore database staging: {error}"))?;
        if remaining.next().is_some() {
            return Err("Restore database staging contains unexpected files.".to_string());
        }
        fs::remove_dir(&databases)
            .map_err(|error| format!("Could not finish restore database staging: {error}"))?;
    }
    Ok(())
}

fn restart_for_restore() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let (root, launcher_layout) = crate::self_update::detect_windows_launcher_root()
            .map_err(|error| format!("Cannot detect Wuddle's launcher: {error}"))?;
        if !launcher_layout {
            return Err("The stable Wuddle.exe launcher was not found. Close and reopen Wuddle manually to finish restoring.".to_string());
        }
        crate::self_update::restart_windows_portable(&root)?;
    }

    #[cfg(target_os = "linux")]
    {
        let executable = std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .filter(|path| path.is_file())
            .or_else(|| std::env::current_exe().ok())
            .ok_or_else(|| "Could not locate the running Wuddle executable.".to_string())?;
        std::process::Command::new(executable)
            .env(
                crate::single_instance::RESTART_PARENT_PID_ENV,
                std::process::id().to_string(),
            )
            .spawn()
            .map_err(|error| format!("Could not relaunch Wuddle: {error}"))?;
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        return Err("Automatic restart is not supported on this platform. Close and reopen Wuddle manually to finish restoring.".to_string());
    }

    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(200));
        std::process::exit(0);
    });
    Ok(())
}

fn normalize_settings_for_restore(settings: &mut AppSettings) -> Result<(), String> {
    if settings.profiles.is_empty() {
        return Err("The settings do not contain any profiles.".to_string());
    }
    let mut ids = HashSet::new();
    for profile in &mut settings.profiles {
        validate_profile_id(&profile.id)?;
        if !ids.insert(profile.id.clone()) {
            return Err(format!(
                "The settings contain duplicate profile ID '{}'.",
                profile.id
            ));
        }
        if profile.name.trim().is_empty() {
            return Err("A profile in the backup has no name.".to_string());
        }
        // A restore must never resume an interrupted credential deletion.
        // Account references are non-secret and remain useful on the same PC.
        profile.pending_auto_login_deletion_ids.clear();
    }
    if !ids.contains(&settings.active_profile_id) {
        settings.active_profile_id = settings.profiles[0].id.clone();
        settings.wow_dir = settings.profiles[0].wow_dir.clone();
    }
    // A restore must never resume an interrupted destructive profile cleanup,
    // or probe unrelated legacy storage after the recovered settings start.
    settings.pending_profile_deletion_ids.clear();
    settings.migrated_from_tauri = true;
    Ok(())
}

fn validate_profile_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(format!("The backup contains an unsafe profile ID: '{id}'."));
    }
    Ok(())
}

fn database_name_for_profile(profile_id: &str) -> Result<String, String> {
    validate_profile_id(profile_id)?;
    Ok(if profile_id == "default" {
        "wuddle.sqlite".to_string()
    } else {
        format!("wuddle-{profile_id}.sqlite")
    })
}

fn verify_manifest_matches_settings(
    manifest: &BackupManifest,
    settings: &AppSettings,
) -> Result<(), String> {
    if manifest.profiles.len() != settings.profiles.len() {
        return Err(
            "The backup manifest and settings disagree about the number of profiles.".to_string(),
        );
    }
    let settings_by_id = settings
        .profiles
        .iter()
        .map(|profile| (profile.id.as_str(), profile))
        .collect::<HashMap<_, _>>();
    let mut ids = HashSet::new();
    for profile in &manifest.profiles {
        validate_profile_id(&profile.id)?;
        if !ids.insert(profile.id.as_str()) {
            return Err("The backup manifest contains duplicate profile IDs.".to_string());
        }
        let Some(settings_profile) = settings_by_id.get(profile.id.as_str()) else {
            return Err(
                "The backup manifest references a profile missing from settings.".to_string(),
            );
        };
        if settings_profile.name != profile.name {
            return Err(
                "The backup manifest and settings disagree about a profile name.".to_string(),
            );
        }
        if let Some(database) = profile.database.as_deref() {
            if database != database_name_for_profile(&profile.id)? {
                return Err("The backup manifest contains an unsafe database name.".to_string());
            }
        }
    }
    Ok(())
}

fn allowed_archive_entries(manifest: &BackupManifest) -> Result<HashSet<String>, String> {
    let mut allowed = HashSet::from([
        MANIFEST_NAME.to_string(),
        SETTINGS_NAME.to_string(),
        README_NAME.to_string(),
        format!("{DATABASES_DIRECTORY}/"),
    ]);
    for profile in &manifest.profiles {
        if let Some(database) = profile.database.as_deref() {
            if database != database_name_for_profile(&profile.id)? {
                return Err("The backup manifest contains an unsafe database name.".to_string());
            }
            allowed.insert(format!("{DATABASES_DIRECTORY}/{database}"));
        }
    }
    Ok(allowed)
}

fn safe_archive_name(raw: &str) -> Result<String, String> {
    if raw.contains('\\') || raw.starts_with('/') {
        return Err("The backup contains an unsafe path.".to_string());
    }
    let path = Path::new(raw);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("The backup contains an unsafe path.".to_string());
    }
    Ok(raw.to_string())
}

fn read_zip_entry<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>, String> {
    let entry = archive
        .by_name(name)
        .map_err(|_| format!("The backup is missing {name}."))?;
    if entry.is_dir() || entry.size() > limit {
        return Err(format!("The backup entry {name} is invalid or too large."));
    }
    let mut bytes = Vec::with_capacity(entry.size().min(1024 * 1024) as usize);
    entry
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read {name} from the backup: {error}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("The backup entry {name} is too large."));
    }
    Ok(bytes)
}

fn extract_zip_entry<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    destination: &Path,
    limit: u64,
) -> Result<(), String> {
    let entry = archive
        .by_name(name)
        .map_err(|_| format!("The backup is missing {name}."))?;
    if entry.is_dir() || entry.size() > limit {
        return Err(format!("The backup entry {name} is invalid or too large."));
    }
    let mut output = File::create(destination)
        .map_err(|error| format!("Could not stage a profile database: {error}"))?;
    let copied = std::io::copy(&mut entry.take(limit + 1), &mut output)
        .map_err(|error| format!("Could not extract a profile database: {error}"))?;
    if copied > limit {
        return Err(format!("The backup entry {name} is too large."));
    }
    output
        .sync_all()
        .map_err(|error| format!("Could not synchronize a staged profile database: {error}"))?;
    Ok(())
}

fn snapshot_database(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        fs::remove_file(destination)
            .map_err(|error| format!("Could not replace a staged database: {error}"))?;
    }
    let source_connection =
        Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| format!("Could not open a profile database for backup: {error}"))?;
    let mut destination_connection = Connection::open(destination)
        .map_err(|error| format!("Could not create a profile database backup: {error}"))?;
    {
        let backup = Backup::new(&source_connection, &mut destination_connection)
            .map_err(|error| format!("Could not begin a profile database backup: {error}"))?;
        backup
            .run_to_completion(64, Duration::from_millis(5), None)
            .map_err(|error| format!("Could not finish a profile database backup: {error}"))?;
    }
    drop(destination_connection);
    verify_database(destination)
}

fn verify_database(path: &Path) -> Result<(), String> {
    ensure_regular_file(path)?;
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Could not inspect a profile database: {error}"))?;
    if metadata.len() > MAX_DATABASE_BYTES {
        return Err("A profile database exceeds Wuddle's backup safety limit.".to_string());
    }
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("Could not open a restored profile database: {error}"))?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("Could not verify a restored profile database: {error}"))?;
    if !integrity.eq_ignore_ascii_case("ok") {
        return Err(format!(
            "A restored profile database failed its integrity check: {integrity}"
        ));
    }
    Ok(())
}

fn summarize_database(path: &Path) -> Result<ProjectCounts, String> {
    verify_database(path)?;
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("Could not inspect a profile database: {error}"))?;
    let has_repos: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='repos')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect a profile database schema: {error}"))?;
    if !has_repos {
        return Ok(ProjectCounts::default());
    }
    let mut statement = connection
        .prepare("SELECT mode, COUNT(*) FROM repos GROUP BY mode")
        .map_err(|error| format!("Could not summarize tracked projects: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
        })
        .map_err(|error| format!("Could not summarize tracked projects: {error}"))?;
    let mut counts = ProjectCounts::default();
    for row in rows {
        let (mode, count) =
            row.map_err(|error| format!("Could not read project summary: {error}"))?;
        match mode.as_str() {
            "addon" | "addon_git" | "manual" => counts.addons += count,
            "mpq" => counts.patches += count,
            _ => counts.mods += count,
        }
    }
    Ok(counts)
}

fn add_counts(total: &mut ProjectCounts, counts: &ProjectCounts) {
    total.addons += counts.addons;
    total.mods += counts.mods;
    total.patches += counts.patches;
}

fn fingerprint_materialized(
    settings: &AppSettings,
    materialized: &Path,
) -> Result<[u8; 32], String> {
    let mut hasher = Sha256::new();
    hasher.update(b"wuddle-backup-preview-v1\0");
    let canonical_settings = serde_json::to_value(settings)
        .and_then(|value| serde_json::to_vec(&value))
        .map_err(|error| format!("Could not fingerprint restored settings: {error}"))?;
    hasher.update((canonical_settings.len() as u64).to_le_bytes());
    hasher.update(canonical_settings);
    for profile in &settings.profiles {
        let database = database_name_for_profile(&profile.id)?;
        hasher.update((database.len() as u64).to_le_bytes());
        hasher.update(database.as_bytes());
        let path = materialized.join(DATABASES_DIRECTORY).join(database);
        if path.is_file() {
            hasher.update([1]);
            let mut file = File::open(&path)
                .map_err(|error| format!("Could not fingerprint a profile database: {error}"))?;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let read = file.read(&mut buffer).map_err(|error| {
                    format!("Could not fingerprint a profile database: {error}")
                })?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
        } else {
            hasher.update([0]);
        }
    }
    Ok(hasher.finalize().into())
}

fn read_regular_file(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    ensure_regular_file(path)?;
    let metadata =
        fs::metadata(path).map_err(|error| format!("Could not inspect a file: {error}"))?;
    if metadata.len() > limit {
        return Err("A backup file exceeds Wuddle's safety limit.".to_string());
    }
    let file =
        File::open(path).map_err(|error| format!("Could not open a backup file: {error}"))?;
    let mut bytes = Vec::with_capacity(metadata.len().min(1024 * 1024) as usize);
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read a backup file: {error}"))?;
    if bytes.len() as u64 > limit {
        return Err("A backup file exceeds Wuddle's safety limit.".to_string());
    }
    Ok(bytes)
}

fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("A required backup file is missing or unreadable: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err("A backup input is linked or is not a regular file.".to_string());
    }
    Ok(())
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Could not serialize backup data: {error}"))?;
    let mut file = File::create(path)
        .map_err(|error| format!("Could not create staged backup data: {error}"))?;
    file.write_all(&bytes)
        .map_err(|error| format!("Could not write staged backup data: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("Could not synchronize staged backup data: {error}"))?;
    Ok(())
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "The restore marker has no parent directory.".to_string())?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("Could not create the temporary restore marker: {error}"))?;
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Could not serialize the restore marker: {error}"))?;
    temporary
        .as_file_mut()
        .write_all(&bytes)
        .map_err(|error| format!("Could not write the restore marker: {error}"))?;
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|error| format!("Could not synchronize the restore marker: {error}"))?;
    temporary
        .persist(path)
        .map_err(|error| format!("Could not activate the restore marker: {}", error.error))?;
    sync_directory(parent)
}

fn safe_file_name(path: &Path) -> Result<String, String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Wuddle's data directory has an invalid name.".to_string())?
        .to_string();
    validate_simple_name(&name)?;
    Ok(name)
}

fn validate_simple_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || Path::new(name).components().count() != 1
    {
        return Err("The restore metadata contains an unsafe directory name.".to_string());
    }
    Ok(())
}

fn unique_rollback_name(parent: &Path, live_name: &str) -> String {
    let timestamp = now_unix();
    for suffix in 0..1000u16 {
        let candidate = if suffix == 0 {
            format!("{live_name}-before-restore-{timestamp}")
        } else {
            format!("{live_name}-before-restore-{timestamp}-{suffix}")
        };
        if !parent.join(&candidate).exists() {
            return candidate;
        }
    }
    format!(
        "{live_name}-before-restore-{}",
        uuid::Uuid::new_v4().simple()
    )
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn format_timestamp(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "an unknown time".to_string())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("Could not synchronize backup storage: {error}"))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ProfileConfig;

    fn settings_with_profiles(ids: &[&str]) -> AppSettings {
        AppSettings {
            profiles: ids
                .iter()
                .map(|id| ProfileConfig {
                    id: (*id).to_string(),
                    name: format!("Profile {id}"),
                    ..ProfileConfig::default()
                })
                .collect(),
            active_profile_id: ids[0].to_string(),
            ..AppSettings::default()
        }
    }

    #[test]
    fn profile_database_names_are_exact_and_isolated() {
        assert_eq!(
            database_name_for_profile("default").unwrap(),
            "wuddle.sqlite"
        );
        assert_eq!(
            database_name_for_profile("chromie-1234").unwrap(),
            "wuddle-chromie-1234.sqlite"
        );
        assert!(database_name_for_profile("../default").is_err());
    }

    #[test]
    fn restore_normalization_clears_destructive_tombstones() {
        let mut settings = settings_with_profiles(&["default"]);
        settings.pending_profile_deletion_ids = vec!["default".to_string()];
        settings.profiles[0].pending_auto_login_deletion_ids = vec!["account".to_string()];
        normalize_settings_for_restore(&mut settings).unwrap();
        assert!(settings.pending_profile_deletion_ids.is_empty());
        assert!(settings.profiles[0]
            .pending_auto_login_deletion_ids
            .is_empty());
        assert!(settings.migrated_from_tauri);
    }

    #[test]
    fn manifest_must_match_settings_profile_identity() {
        let settings = settings_with_profiles(&["default"]);
        let manifest = BackupManifest {
            format_version: ARCHIVE_FORMAT_VERSION,
            created_unix: 0,
            wuddle_version: "test".to_string(),
            profiles: vec![ManifestProfile {
                id: "other".to_string(),
                name: "Other".to_string(),
                database: None,
                projects: ProjectCounts::default(),
            }],
            excluded_secrets: Vec::new(),
        };
        assert!(verify_manifest_matches_settings(&manifest, &settings).is_err());
    }

    #[test]
    fn archive_paths_reject_traversal_and_backslashes() {
        assert!(safe_archive_name("databases/wuddle.sqlite").is_ok());
        assert!(safe_archive_name("../settings.json").is_err());
        assert!(safe_archive_name("databases\\wuddle.sqlite").is_err());
    }

    #[test]
    fn database_summary_separates_addons_mods_and_patches() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("wuddle.sqlite");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE repos (mode TEXT NOT NULL);\n\
                 INSERT INTO repos(mode) VALUES ('addon_git'), ('manual'), ('dll'), ('mixed'), ('mpq');",
            )
            .unwrap();
        drop(connection);
        assert_eq!(
            summarize_database(&path).unwrap(),
            ProjectCounts {
                addons: 2,
                mods: 2,
                patches: 1,
            }
        );
    }

    #[test]
    fn folder_import_snapshots_only_profile_databases() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let settings = settings_with_profiles(&["default", "second"]);
        write_json_file(&source.path().join(SETTINGS_NAME), &settings).unwrap();
        for name in ["wuddle.sqlite", "wuddle-second.sqlite"] {
            let connection = Connection::open(source.path().join(name)).unwrap();
            connection
                .execute_batch("CREATE TABLE repos (mode TEXT NOT NULL); INSERT INTO repos VALUES ('addon_git');")
                .unwrap();
        }
        fs::write(source.path().join("diagnostics.log"), b"private logs").unwrap();

        let materialized = copy_data_directory(source.path(), destination.path()).unwrap();
        assert_eq!(materialized.settings.profiles.len(), 2);
        assert!(destination
            .path()
            .join(DATABASES_DIRECTORY)
            .join("wuddle.sqlite")
            .is_file());
        assert!(!destination.path().join("diagnostics.log").exists());
    }

    #[test]
    fn old_install_selection_accepts_the_main_wuddle_directory() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("wuddle-data");
        fs::create_dir_all(&data).unwrap();
        write_json_file(
            &data.join(SETTINGS_NAME),
            &settings_with_profiles(&["default"]),
        )
        .unwrap();

        assert_eq!(resolve_old_data_directory(root.path()).unwrap(), data);
    }

    #[test]
    fn old_install_selection_searches_bounded_version_directories() {
        let root = tempfile::tempdir().unwrap();
        let data = root
            .path()
            .join("versions")
            .join("v3.6.0")
            .join("portable")
            .join("wuddle-data");
        fs::create_dir_all(&data).unwrap();
        write_json_file(
            &data.join(SETTINGS_NAME),
            &settings_with_profiles(&["default"]),
        )
        .unwrap();

        assert_eq!(resolve_old_data_directory(root.path()).unwrap(), data);
    }

    #[test]
    fn old_install_selection_rejects_ambiguous_nested_data() {
        let root = tempfile::tempdir().unwrap();
        for name in ["first", "second"] {
            let data = root.path().join(name);
            fs::create_dir_all(&data).unwrap();
            write_json_file(
                &data.join(SETTINGS_NAME),
                &settings_with_profiles(&["default"]),
            )
            .unwrap();
        }

        assert!(resolve_old_data_directory(root.path())
            .unwrap_err()
            .contains("Several old Wuddle data folders"));
    }

    #[test]
    fn archive_round_trip_preserves_settings_and_database_summary() {
        let temporary = tempfile::tempdir().unwrap();
        let staged = temporary.path().join("staged");
        fs::create_dir_all(staged.join(DATABASES_DIRECTORY)).unwrap();
        let settings = settings_with_profiles(&["default"]);
        write_json_file(&staged.join(SETTINGS_NAME), &settings).unwrap();
        let database = staged.join(DATABASES_DIRECTORY).join("wuddle.sqlite");
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE repos (mode TEXT NOT NULL);\n\
                 INSERT INTO repos VALUES ('addon_git'), ('dll'), ('mpq');",
            )
            .unwrap();
        drop(connection);
        let projects = summarize_database(&database).unwrap();
        let manifest = BackupManifest {
            format_version: ARCHIVE_FORMAT_VERSION,
            created_unix: 123,
            wuddle_version: "test-version".to_string(),
            profiles: vec![ManifestProfile {
                id: "default".to_string(),
                name: "Profile default".to_string(),
                database: Some("wuddle.sqlite".to_string()),
                projects: projects.clone(),
            }],
            excluded_secrets: vec!["Auto-login passwords".to_string()],
        };
        let archive_path = temporary.path().join("backup.zip");
        let mut archive_file = File::create(&archive_path).unwrap();
        write_backup_zip(&mut archive_file, &staged, &manifest).unwrap();
        drop(archive_file);

        let preview = inspect_source(&archive_path, false).unwrap();
        assert_eq!(preview.source_version.as_deref(), Some("test-version"));
        assert_eq!(preview.created_unix, Some(123));
        assert_eq!(preview.profiles.len(), 1);
        assert_eq!(preview.totals, projects);
    }

    #[test]
    fn full_export_contains_only_declared_recovery_data() {
        let temporary = tempfile::tempdir().unwrap();
        let app_data = temporary.path().join("wuddle-data");
        fs::create_dir_all(&app_data).unwrap();
        let settings = settings_with_profiles(&["default"]);
        write_json_file(&app_data.join(SETTINGS_NAME), &settings).unwrap();
        let connection = Connection::open(app_data.join("wuddle.sqlite")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE repos (mode TEXT NOT NULL); INSERT INTO repos VALUES ('addon_git');",
            )
            .unwrap();
        drop(connection);
        fs::write(app_data.join("diagnostics.log"), b"not part of recovery").unwrap();
        fs::write(app_data.join(".github_token"), b"must never be exported").unwrap();

        let archive_path = temporary.path().join("export.zip");
        let summary = export_backup_from(&app_data, &archive_path).unwrap();
        assert_eq!(summary.profiles, 1);
        assert_eq!(summary.totals.addons, 1);

        let file = File::open(&archive_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names = (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name().to_string())
            .collect::<HashSet<_>>();
        assert!(names.contains(MANIFEST_NAME));
        assert!(names.contains(SETTINGS_NAME));
        assert!(names.contains("databases/wuddle.sqlite"));
        assert!(!names.contains("diagnostics.log"));
        assert!(!names.contains(".github_token"));

        let preview = inspect_source(&archive_path, false).unwrap();
        assert_eq!(preview.profiles.len(), 1);
        assert_eq!(preview.totals.addons, 1);
    }

    #[test]
    fn staged_restore_swaps_data_and_preserves_the_previous_directory() {
        let parent = tempfile::tempdir().unwrap();
        let live = parent.path().join("wuddle-data");
        let source = parent.path().join("old-wuddle-data");
        fs::create_dir_all(&live).unwrap();
        fs::create_dir_all(&source).unwrap();

        let mut old_settings = settings_with_profiles(&["default"]);
        old_settings.profiles[0].name = "Before restore".to_string();
        write_json_file(&live.join(SETTINGS_NAME), &old_settings).unwrap();

        let mut restored_settings = settings_with_profiles(&["default"]);
        restored_settings.profiles[0].name = "Restored profile".to_string();
        write_json_file(&source.join(SETTINGS_NAME), &restored_settings).unwrap();
        let connection = Connection::open(source.join("wuddle.sqlite")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE repos (mode TEXT NOT NULL); INSERT INTO repos VALUES ('mpq');",
            )
            .unwrap();
        drop(connection);

        let preview = inspect_source(&source, true).unwrap();
        schedule_restore_at(&preview, &live).unwrap();
        assert!(apply_pending_restore_at(&live).unwrap());

        let restored: AppSettings =
            serde_json::from_slice(&fs::read(live.join(SETTINGS_NAME)).unwrap()).unwrap();
        assert_eq!(restored.profiles[0].name, "Restored profile");
        assert!(live.join("wuddle.sqlite").is_file());
        assert!(!live.join("databases").exists());

        let rollback = fs::read_dir(parent.path())
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("wuddle-data-before-restore-")
            })
            .unwrap()
            .path();
        let preserved: AppSettings =
            serde_json::from_slice(&fs::read(rollback.join(SETTINGS_NAME)).unwrap()).unwrap();
        assert_eq!(preserved.profiles[0].name, "Before restore");
        assert!(!parent.path().join(PENDING_MARKER_NAME).exists());
    }

    #[test]
    fn invalid_staged_database_never_replaces_live_data() {
        let parent = tempfile::tempdir().unwrap();
        let live = parent.path().join("wuddle-data");
        let staging_name = ".wuddle-data-restore-stage-test";
        let staging = parent.path().join(staging_name);
        fs::create_dir_all(&live).unwrap();
        fs::create_dir_all(&staging).unwrap();

        let mut old_settings = settings_with_profiles(&["default"]);
        old_settings.profiles[0].name = "Still live".to_string();
        write_json_file(&live.join(SETTINGS_NAME), &old_settings).unwrap();
        write_json_file(
            &staging.join(SETTINGS_NAME),
            &settings_with_profiles(&["default"]),
        )
        .unwrap();
        fs::write(staging.join("wuddle.sqlite"), b"not sqlite").unwrap();
        write_json_file(
            &staging.join(RESTORE_NOTICE_NAME),
            &RestoreNotice {
                rollback_directory_name: "wuddle-data-before-restore-test".to_string(),
            },
        )
        .unwrap();
        write_json_file(
            &parent.path().join(PENDING_MARKER_NAME),
            &PendingRestore {
                format_version: ARCHIVE_FORMAT_VERSION,
                live_directory_name: "wuddle-data".to_string(),
                staging_directory_name: staging_name.to_string(),
                rollback_directory_name: "wuddle-data-before-restore-test".to_string(),
            },
        )
        .unwrap();

        assert!(apply_pending_restore_at(&live).is_err());
        let preserved: AppSettings =
            serde_json::from_slice(&fs::read(live.join(SETTINGS_NAME)).unwrap()).unwrap();
        assert_eq!(preserved.profiles[0].name, "Still live");
        assert!(!parent
            .path()
            .join("wuddle-data-before-restore-test")
            .exists());
    }

    #[test]
    fn reset_removes_live_and_legacy_data_without_touching_game_files() {
        let parent = tempfile::tempdir().unwrap();
        let live = parent.path().join("wuddle-data");
        let legacy = parent.path().join("legacy-wuddle");
        let rollback = parent.path().join("wuddle-data-before-restore-1234567890");
        let game = parent.path().join("World of Warcraft");
        for directory in [&live, &legacy, &rollback, &game] {
            fs::create_dir_all(directory).unwrap();
        }
        fs::write(live.join(SETTINGS_NAME), b"saved settings").unwrap();
        fs::write(legacy.join("wuddle.sqlite"), b"saved database").unwrap();
        fs::write(rollback.join("wuddle.sqlite"), b"rollback database").unwrap();
        fs::write(game.join("Wow.exe"), b"game").unwrap();
        write_json_file(
            &parent.path().join(PENDING_RESET_MARKER_NAME),
            &PendingReset {
                format_version: ARCHIVE_FORMAT_VERSION,
                live_directory_name: "wuddle-data".to_string(),
            },
        )
        .unwrap();

        let reset_targets = vec![legacy.clone(), rollback.clone()];
        assert!(apply_pending_reset_at(&live, &reset_targets).unwrap());
        assert!(live.is_dir());
        assert!(live.join(RESET_NOTICE_NAME).is_file());
        assert!(!live.join(SETTINGS_NAME).exists());
        assert!(!legacy.exists());
        assert!(!rollback.exists());
        assert!(game.join("Wow.exe").is_file());
        assert!(!parent.path().join(PENDING_RESET_MARKER_NAME).exists());
    }
}
