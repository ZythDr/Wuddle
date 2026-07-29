//! Thin Iced adapter for generic MPQ management and the curated WDM recipe.
//! Validation, staging, protection, backups, and deployment stay in the engine.

use std::path::PathBuf;

use iced::widget::{
    button, checkbox, column, container, pick_list, row, rule, scrollable, text, text_input, Space,
};
use iced::{Element, Length, Task};

use crate::app::App;
use crate::components::helpers::{
    badge_tag, close_button, dialog_description, dialog_field_label, tip,
};
use crate::message::Message;
use crate::service;
use crate::theme::{self, ThemeColors};
use crate::types::{Dialog, LogLevel, ToastKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MpqClassification {
    CoreClient,
    Custom,
}

fn manage_path_key(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    lower
        .strip_suffix(".disabled")
        .unwrap_or(&lower)
        .to_string()
}

fn tracked_package_name(app: &App, repo_id: i64) -> String {
    app.repos
        .iter()
        .find(|repo| repo.id == repo_id)
        .map(|repo| repo.name.clone())
        .unwrap_or_else(|| format!("repository #{repo_id}"))
}

fn tracked_component_name(app: &App, repo_id: i64, path: &str) -> String {
    app.repos
        .iter()
        .find(|repo| repo.id == repo_id)
        .and_then(|repo| {
            repo.installed_mpqs
                .iter()
                .find(|entry| entry.path.eq_ignore_ascii_case(path))
        })
        .map(|entry| entry.display_name.clone())
        .unwrap_or_else(|| "MPQ component".to_string())
}

fn untracked_component_name(app: &App, path: &str) -> String {
    app.mpq_ui
        .protection
        .iter()
        .chain(app.untracked_mpqs.iter())
        .find(|entry| entry.path.eq_ignore_ascii_case(path))
        .map(|entry| {
            entry
                .display_name
                .clone()
                .unwrap_or_else(|| entry.file_name.clone())
        })
        .unwrap_or_else(|| "untracked MPQ".to_string())
}

fn enabled_state(enabled: bool) -> &'static str {
    if enabled {
        "enabled"
    } else {
        "disabled"
    }
}

impl std::fmt::Display for MpqClassification {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::CoreClient => "Core client file",
            Self::Custom => "Custom MPQ",
        })
    }
}

fn protection_icon(locked: bool, color: iced::Color) -> Element<'static, Message> {
    let svg = if locked {
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></svg>"#
    } else {
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 7.5-2"/></svg>"#
    };
    iced::widget::svg(iced::widget::svg::Handle::from_memory(
        svg.as_bytes().to_vec(),
    ))
    .width(18)
    .height(18)
    .style(move |_theme, _status| iced::widget::svg::Style { color: Some(color) })
    .into()
}

#[derive(Debug, Clone, Default)]
pub struct UiState {
    pub direct_url: String,
    pub source: Option<PathBuf>,
    pub inspection: Option<wuddle_engine::mpq::MpqInspection>,
    pub selections: Vec<wuddle_engine::mpq::MpqInstallSelection>,
    pub target_previews: Vec<wuddle_engine::mpq::MpqTargetPreview>,
    pub targets_reviewed: bool,
    pub protection: Vec<wuddle_engine::mpq::MpqProtectionEntry>,
    pub detected_locale: Option<String>,
    manage_order: Vec<String>,
    manage_core_keys: Vec<String>,
    manage_managed_order: Vec<(i64, String)>,
    manage_snapshot_initialized: bool,
    pub catalog: Option<service::WdmCatalog>,
    pub wdm_locale: Option<String>,
    pub wdm_caverns: bool,
    pub wdm_addon: bool,
    pub busy: bool,
    pub error: Option<String>,
    pub active_operation_id: Option<u64>,
    pub commit_operation_id: Option<u64>,
    pub pending_picker_id: Option<u64>,
}

impl UiState {
    pub fn commit_in_progress(&self) -> bool {
        self.commit_operation_id.is_some()
    }

    pub fn dismissal_blocked(&self) -> bool {
        self.commit_in_progress() || (self.busy && self.active_operation_id.is_none())
    }

    pub fn cancel_precommit_work(&mut self) {
        if !self.commit_in_progress() {
            self.active_operation_id = None;
            self.pending_picker_id = None;
            self.busy = false;
        }
    }
}

fn begin_operation(app: &mut App, commit: bool) -> (u64, crate::ProfileOperationScope) {
    let operation_id = app.next_async_request_id();
    app.mpq_ui.active_operation_id = Some(operation_id);
    app.mpq_ui.commit_operation_id = commit.then_some(operation_id);
    app.mpq_ui.busy = true;
    (operation_id, app.profile_operation_scope())
}

fn accept_operation<T>(
    app: &mut App,
    operation_id: u64,
    result: crate::ProfileScoped<T>,
    label: &str,
) -> Option<T> {
    if app.mpq_ui.active_operation_id != Some(operation_id) {
        app.log(
            LogLevel::Info,
            &format!("Discarded stale {label} result after its MPQ dialog changed."),
        );
        return None;
    }
    let result = app.accept_profile_result(result, label);
    app.mpq_ui.active_operation_id = None;
    if app.mpq_ui.commit_operation_id == Some(operation_id) {
        app.mpq_ui.commit_operation_id = None;
    }
    app.mpq_ui.busy = false;
    result
}

fn pick_source_task(request_id: u64, scope: crate::ProfileOperationScope) -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter("MPQ packages", &["mpq", "zip", "7z"])
                .set_title("Select an MPQ file or package")
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        move |path| Message::MpqSourcePicked {
            request_id,
            scope: scope.clone(),
            path,
        },
    )
}

fn inspect_task(app: &mut App, source: PathBuf) -> Task<Message> {
    let (operation_id, scope) = begin_operation(app, false);
    Task::perform(
        service::inspect_local_mpq(app.db_path.clone(), app.wow_dir.clone(), source),
        move |result| Message::MpqInspectionFinished {
            operation_id,
            result: crate::ProfileScoped::new(scope.clone(), result),
        },
    )
}

pub fn update(app: &mut App, message: Message) -> Option<Task<Message>> {
    match message {
        Message::OpenMpqAdd => {
            app.mpq_ui = UiState::default();
            app.dialog = Some(Dialog::MpqAdd);
            Some(Task::none())
        }
        Message::SetMpqDirectUrl(value) => {
            app.mpq_ui.direct_url = value;
            Some(Task::none())
        }
        Message::RescanMpqs => {
            app.log(
                LogLevel::Info,
                "MPQ rescan requested for Data/ and the detected locale directory.",
            );
            app.mpq_ui.busy = true;
            Some(Task::perform(
                service::rescan_mpqs(app.db_path.clone(), app.wow_dir.clone()),
                Message::MpqRescanFinished,
            ))
        }
        Message::MpqRescanFinished(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(count) => {
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "MPQ rescan completed: {count} untracked archive(s) detected; managed packages refreshed."
                        ),
                    );
                    app.show_toast(
                        format!("Rescanned Data folders; found {count} untracked MPQ(s)."),
                        ToastKind::Info,
                    );
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.log(LogLevel::Error, &format!("MPQ rescan failed: {error}"));
                    app.show_toast(error, ToastKind::Error);
                    Some(Task::none())
                }
            }
        }
        Message::UpdateAllPatches => {
            let has_wdm_update = app.repos.iter().any(|repo| {
                service::is_wdm_repo(repo)
                    && app
                        .plans
                        .iter()
                        .any(|plan| plan.repo_id == repo.id && plan.has_update)
            });
            let has_epoch_water_update = app.repos.iter().any(|repo| {
                service::is_epoch_water_repo(repo)
                    && app
                        .plans
                        .iter()
                        .any(|plan| plan.repo_id == repo.id && plan.has_update)
            });
            match (has_wdm_update, has_epoch_water_update) {
                (false, false) => {
                    app.show_toast("No curated patch updates are available.", ToastKind::Info);
                    Some(Task::none())
                }
                (true, false) => Some(Task::done(Message::OpenWdm)),
                (false, true) => Some(Task::done(Message::InstallEpochWater)),
                // WDM update choices need its configuration dialog. Keep that
                // deliberate choice in front when both curated patches update;
                // Epoch Water remains available through its row update button.
                (true, true) => Some(Task::done(Message::OpenWdm)),
            }
        }
        Message::InstallEpochWater => {
            if app.wow_dir.is_empty() {
                app.show_toast(
                    "Set a WoW directory before installing Epoch Water.",
                    ToastKind::Error,
                );
                return Some(Task::none());
            }
            app.mpq_ui.error = None;
            let (operation_id, scope) = begin_operation(app, true);
            Some(Task::perform(
                service::install_epoch_water(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    app.install_options(),
                ),
                move |result| Message::EpochWaterInstalled {
                    operation_id,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::EpochWaterInstalled {
            operation_id,
            result,
        } => {
            let Some(result) =
                accept_operation(app, operation_id, result, "Epoch Water installation")
            else {
                return Some(Task::none());
            };
            match result {
                Ok(_) => {
                    if matches!(app.dialog, Some(Dialog::MpqAdd)) {
                        app.dialog = None;
                    }
                    app.log(LogLevel::Info, "Epoch Water installed successfully.");
                    app.show_toast("Epoch Water installed successfully.", ToastKind::Success);
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Epoch Water install failed: {error}"),
                    );
                    let rate_limited =
                        app.show_github_rate_limit("Epoch Water could not be installed.", &error);
                    let error = crate::github_api::user_facing_error(&error);
                    if matches!(app.dialog, Some(Dialog::MpqAdd)) {
                        app.mpq_ui.error = Some(error);
                    } else if !rate_limited {
                        app.show_toast(
                            format!("Epoch Water install failed: {error}"),
                            ToastKind::Error,
                        );
                    }
                    Some(Task::none())
                }
            }
        }
        Message::OpenEpochWaterReadme => {
            let generation = app.begin_preview_request();
            app.markdown_image_cache.clear();
            app.markdown_gif_cache.clear();
            app.dialog = Some(Dialog::Changelog {
                title: "Epoch Water — README".to_string(),
                items: Vec::new(),
                loading: true,
            });
            Some(Task::perform(
                service::fetch_repo_preview(service::EPOCH_WATER_URL.to_string()),
                move |result| Message::EpochWaterReadmeLoaded(generation, result),
            ))
        }
        Message::EpochWaterReadmeLoaded(generation, result) => {
            if !app.preview_request_is_current(generation, "Epoch Water README") {
                return Some(Task::none());
            }
            let loaded_items = match result {
                Ok(preview) => {
                    app.markdown_image_cache = preview.image_cache;
                    app.markdown_gif_cache = preview.gif_cache;
                    preview.readme_items
                }
                Err(error) => {
                    app.show_github_rate_limit(
                        "The Epoch Water README could not be loaded.",
                        &error,
                    );
                    iced::widget::markdown::Content::parse(&format!(
                        "Could not load the Epoch Water README.\n\n{}",
                        crate::github_api::user_facing_error(&error)
                    ))
                    .items()
                    .to_vec()
                }
            };
            if let Some(Dialog::Changelog {
                items: dialog_items,
                loading,
                ..
            }) = app.dialog.as_mut()
            {
                *loading = false;
                *dialog_items = loaded_items;
            }
            Some(Task::none())
        }
        Message::OpenMpqInstall => {
            app.mpq_ui = UiState::default();
            app.dialog = Some(Dialog::MpqInstall);
            Some(Task::none())
        }
        Message::PickMpqSource => {
            let request_id = app.next_async_request_id();
            let scope = app.profile_operation_scope();
            app.mpq_ui.pending_picker_id = Some(request_id);
            Some(pick_source_task(request_id, scope))
        }
        Message::MpqSourcePicked {
            request_id,
            scope,
            path,
        } => {
            if app.mpq_ui.pending_picker_id != Some(request_id)
                || !scope.matches(&app.active_profile_id, app.profile_generation)
                || !matches!(app.dialog, Some(Dialog::MpqInstall))
            {
                app.log(
                    LogLevel::Info,
                    "Discarded a stale MPQ source picker result.",
                );
                return Some(Task::none());
            }
            app.mpq_ui.pending_picker_id = None;
            let Some(source) = path else {
                app.log(LogLevel::Info, "MPQ source selection cancelled.");
                return Some(Task::none());
            };
            app.log(
                LogLevel::Info,
                "Local MPQ source selected; inspection started (source path omitted).",
            );
            app.mpq_ui.source = Some(source.clone());
            app.mpq_ui.inspection = None;
            app.mpq_ui.selections.clear();
            app.mpq_ui.target_previews.clear();
            app.mpq_ui.targets_reviewed = false;
            app.mpq_ui.error = None;
            Some(inspect_task(app, source))
        }
        Message::LocalArchiveHovered(path) if matches!(app.dialog, Some(Dialog::MpqInstall)) => {
            app.local_archive_hover_path =
                wuddle_engine::mpq::is_supported_local_source(&path).then_some(path);
            Some(Task::none())
        }
        Message::LocalArchiveDropped(path) if matches!(app.dialog, Some(Dialog::MpqInstall)) => {
            app.local_archive_hover_path = None;
            if !wuddle_engine::mpq::is_supported_local_source(&path) {
                app.log(
                    LogLevel::Error,
                    "Rejected an unsupported local MPQ drop; source path omitted.",
                );
                app.mpq_ui.error = Some("Drop a local .mpq, .zip, or .7z file.".to_string());
                return Some(Task::none());
            }
            app.log(
                LogLevel::Info,
                "Local MPQ source dropped; inspection started (source path omitted).",
            );
            app.mpq_ui.source = Some(path.clone());
            app.mpq_ui.inspection = None;
            app.mpq_ui.selections.clear();
            app.mpq_ui.target_previews.clear();
            app.mpq_ui.targets_reviewed = false;
            app.mpq_ui.error = None;
            Some(inspect_task(app, path))
        }
        Message::MpqInspectionFinished {
            operation_id,
            result,
        } => {
            let Some(result) = accept_operation(app, operation_id, result, "MPQ inspection") else {
                return Some(Task::none());
            };
            match result {
                Ok(inspection) => {
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "Local MPQ inspection completed: {} valid MPQ candidate(s) staged for review.",
                            inspection.candidates.len()
                        ),
                    );
                    app.mpq_ui.selections = inspection
                        .candidates
                        .iter()
                        .map(|candidate| wuddle_engine::mpq::MpqInstallSelection {
                            source_key: candidate.source_key.clone(),
                            display_name: candidate.suggested_display_name.clone(),
                            file_name: candidate.original_file_name.clone(),
                            destination: candidate.suggested_destination.clone(),
                            replace_unprotected: false,
                            version: None,
                        })
                        .collect();
                    app.mpq_ui.inspection = Some(inspection);
                    app.mpq_ui.target_previews.clear();
                    app.mpq_ui.targets_reviewed = false;
                    app.mpq_ui.error = None;
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Local MPQ inspection failed: {error}"),
                    );
                    app.mpq_ui.error = Some(error);
                }
            }
            Some(Task::none())
        }
        Message::SetMpqDisplayName(index, value) => {
            if let Some(selection) = app.mpq_ui.selections.get_mut(index) {
                selection.display_name = value;
            }
            app.mpq_ui.targets_reviewed = false;
            Some(Task::none())
        }
        Message::SetMpqFileName(index, value) => {
            if let Some(selection) = app.mpq_ui.selections.get_mut(index) {
                selection.file_name = value;
            }
            app.mpq_ui.targets_reviewed = false;
            Some(Task::none())
        }
        Message::SetMpqDestination(index, destination) => {
            if let Some(selection) = app.mpq_ui.selections.get_mut(index) {
                selection.destination = destination;
            }
            app.mpq_ui.targets_reviewed = false;
            Some(Task::none())
        }
        Message::ToggleMpqReplacement(index, replace) => {
            if let Some(selection) = app.mpq_ui.selections.get_mut(index) {
                selection.replace_unprotected = replace;
            }
            app.mpq_ui.targets_reviewed = false;
            Some(Task::none())
        }
        Message::InstallMpqPackage => {
            let Some(source) = app.mpq_ui.source.clone() else {
                app.log(
                    LogLevel::Error,
                    "Local MPQ installation could not start because no source was selected.",
                );
                app.mpq_ui.error = Some("Choose an MPQ source first.".to_string());
                return Some(Task::none());
            };
            app.mpq_ui.error = None;
            if !app.mpq_ui.targets_reviewed {
                app.log(
                    LogLevel::Info,
                    &format!(
                        "Reviewing {} local MPQ target(s) before installation; filenames and paths omitted.",
                        app.mpq_ui.selections.len()
                    ),
                );
                let (operation_id, scope) = begin_operation(app, false);
                return Some(Task::perform(
                    service::preview_local_mpq_targets(
                        app.db_path.clone(),
                        app.wow_dir.clone(),
                        source,
                        app.mpq_ui.selections.clone(),
                    ),
                    move |result| Message::MpqTargetsReviewed {
                        operation_id,
                        result: crate::ProfileScoped::new(scope.clone(), result),
                    },
                ));
            }
            app.log(
                LogLevel::Info,
                &format!(
                    "Local MPQ installation commit requested: component_count={}; source and target paths omitted.",
                    app.mpq_ui.selections.len()
                ),
            );
            let (operation_id, scope) = begin_operation(app, true);
            Some(Task::perform(
                service::install_local_mpq(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    source,
                    app.mpq_ui.selections.clone(),
                    app.opt_xattr,
                ),
                move |result| Message::MpqInstallFinished {
                    operation_id,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::MpqTargetsReviewed {
            operation_id,
            result,
        } => {
            let Some(result) = accept_operation(app, operation_id, result, "MPQ target preview")
            else {
                return Some(Task::none());
            };
            match result {
                Ok(previews) => {
                    let collisions = previews
                        .iter()
                        .filter(|preview| {
                            preview.status != wuddle_engine::mpq::MpqTargetStatus::Available
                        })
                        .count();
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "Local MPQ target review completed: target_count={}; collision_count={collisions}.",
                            previews.len()
                        ),
                    );
                    app.mpq_ui.target_previews = previews;
                    app.mpq_ui.targets_reviewed = true;
                    app.mpq_ui.error = None;
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Local MPQ target review failed: {error}"),
                    );
                    app.mpq_ui.error = Some(error);
                }
            }
            Some(Task::none())
        }
        Message::MpqInstallFinished {
            operation_id,
            result,
        } => {
            let Some(result) = accept_operation(app, operation_id, result, "MPQ installation")
            else {
                return Some(Task::none());
            };
            match result {
                Ok(_) => {
                    app.dialog = None;
                    app.log(LogLevel::Info, "MPQ package installed.");
                    app.show_toast("MPQ package installed.", ToastKind::Success);
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!(
                            "MPQ package installation failed; staged data was not committed: {error}"
                        ),
                    );
                    app.mpq_ui.error = Some(error);
                    Some(Task::none())
                }
            }
        }
        Message::ToggleMpqPackageEnabled(repo_id, enabled) => {
            if app.mpq_ui.busy {
                return Some(Task::none());
            }
            let target_name = tracked_package_name(app, repo_id);
            app.log(
                LogLevel::Info,
                &format!(
                    "MPQ state change requested: package \"{target_name}\" (repo id={repo_id}) -> {}.",
                    enabled_state(enabled)
                ),
            );
            app.mpq_ui.busy = true;
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::set_mpq_enabled(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    repo_id,
                    None,
                    enabled,
                ),
                move |result| Message::MpqEnabledChanged {
                    repo_id,
                    target_name: target_name.clone(),
                    package: true,
                    enabled,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::ToggleMpqEnabled(repo_id, path, enabled) => {
            if app.mpq_ui.busy {
                return Some(Task::none());
            }
            if matches!(app.dialog.as_ref(), Some(Dialog::ProtectedMpqs))
                && app
                    .repos
                    .iter()
                    .find(|repo| repo.id == repo_id)
                    .and_then(|repo| repo.installed_mpqs.iter().find(|entry| entry.path == path))
                    .map(|entry| !entry.editor_unlocked)
                    .unwrap_or(true)
            {
                return Some(Task::none());
            }
            let target_name = tracked_component_name(app, repo_id, &path);
            app.log(
                LogLevel::Info,
                &format!(
                    "MPQ state change requested: component \"{target_name}\" (repo id={repo_id}) -> {}.",
                    enabled_state(enabled)
                ),
            );
            app.mpq_ui.busy = true;
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::set_mpq_enabled(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    repo_id,
                    Some(path),
                    enabled,
                ),
                move |result| Message::MpqEnabledChanged {
                    repo_id,
                    target_name: target_name.clone(),
                    package: false,
                    enabled,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::MpqEnabledChanged {
            repo_id,
            target_name,
            package,
            enabled,
            result,
        } => {
            app.mpq_ui.busy = false;
            let Some(result) = app.accept_profile_result(result, "MPQ enable-state update") else {
                return Some(Task::none());
            };
            let target_kind = if package { "package" } else { "component" };
            match result {
                Ok(changed) => {
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "MPQ {target_kind} {}: \"{target_name}\" (repo id={repo_id}; filesystem rename(s)={changed}; metadata committed).",
                            enabled_state(enabled)
                        ),
                    );
                    app.show_toast(
                        if enabled {
                            "MPQ patch enabled."
                        } else {
                            "MPQ patch disabled."
                        },
                        ToastKind::Info,
                    );
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!(
                            "MPQ {target_kind} state change failed: \"{target_name}\" (repo id={repo_id}; requested_state={}): {error}",
                            enabled_state(enabled)
                        ),
                    );
                    app.show_toast(error, ToastKind::Error);
                    Some(Task::none())
                }
            }
        }
        Message::OpenMpqProtection => {
            app.open_menu = None;
            app.dialog = Some(Dialog::ProtectedMpqs);
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            app.mpq_ui.detected_locale = None;
            app.mpq_ui.manage_order.clear();
            app.mpq_ui.manage_core_keys.clear();
            let mut managed_order = app
                .repos
                .iter()
                .filter(|repo| repo.mode == "mpq")
                .flat_map(|repo| {
                    repo.installed_mpqs.iter().map(move |entry| {
                        (
                            entry.display_name.to_ascii_lowercase(),
                            repo.id,
                            manage_path_key(&entry.path),
                        )
                    })
                })
                .collect::<Vec<_>>();
            managed_order.sort_by(|left, right| left.0.cmp(&right.0));
            app.mpq_ui.manage_managed_order = managed_order
                .into_iter()
                .map(|(_, repo_id, path)| (repo_id, path))
                .collect();
            app.mpq_ui.manage_snapshot_initialized = false;
            Some(Task::batch([
                Task::perform(
                    service::load_mpq_protection(app.db_path.clone(), app.wow_dir.clone()),
                    Message::MpqProtectionLoaded,
                ),
                Task::perform(
                    service::detect_mpq_locale(app.db_path.clone(), app.wow_dir.clone()),
                    Message::MpqLocaleDetected,
                ),
            ]))
        }
        Message::MpqLocaleDetected(result) => {
            match result {
                Ok(locale) => app.mpq_ui.detected_locale = locale,
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("MPQ locale detection failed: {error}"),
                    );
                    app.mpq_ui.detected_locale = None;
                }
            }
            Some(Task::none())
        }
        Message::MpqProtectionLoaded(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(entries) => {
                    app.untracked_mpqs = entries.clone();
                    if !app.mpq_ui.manage_snapshot_initialized {
                        let mut ordered = entries.iter().collect::<Vec<_>>();
                        ordered.sort_by(|left, right| {
                            let left_core = left.core;
                            let right_core = right.core;
                            left_core.cmp(&right_core).then_with(|| {
                                let left_name = left
                                    .display_name
                                    .as_deref()
                                    .unwrap_or(&left.file_name)
                                    .to_ascii_lowercase();
                                let right_name = right
                                    .display_name
                                    .as_deref()
                                    .unwrap_or(&right.file_name)
                                    .to_ascii_lowercase();
                                left_name.cmp(&right_name)
                            })
                        });
                        app.mpq_ui.manage_order = ordered
                            .iter()
                            .map(|entry| manage_path_key(&entry.path))
                            .collect();
                        app.mpq_ui.manage_core_keys = ordered
                            .iter()
                            .filter(|entry| entry.core)
                            .map(|entry| manage_path_key(&entry.path))
                            .collect();
                        app.mpq_ui.manage_snapshot_initialized = true;
                    }
                    app.mpq_ui.protection = entries;
                    app.mpq_ui.error = None;
                }
                Err(error) => app.mpq_ui.error = Some(error),
            }
            Some(Task::none())
        }
        Message::SetUntrackedMpqEditorUnlocked(path, editor_unlocked) => {
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            let target_name = untracked_component_name(app, &path);
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::set_untracked_mpq_editor_unlocked(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    path,
                    editor_unlocked,
                ),
                move |result| Message::MpqEditorLockChanged {
                    repo_id: None,
                    target_name: target_name.clone(),
                    editor_unlocked,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::SetTrackedMpqEditorUnlocked(repo_id, path, editor_unlocked) => {
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            let target_name = tracked_component_name(app, repo_id, &path);
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::set_tracked_mpq_editor_unlocked(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    repo_id,
                    path,
                    editor_unlocked,
                ),
                move |result| Message::MpqEditorLockChanged {
                    repo_id: Some(repo_id),
                    target_name: target_name.clone(),
                    editor_unlocked,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::MpqEditorLockChanged {
            repo_id,
            target_name,
            editor_unlocked,
            result,
        } => {
            app.mpq_ui.busy = false;
            let Some(result) = app.accept_profile_result(result, "MPQ editor lock update") else {
                return Some(Task::none());
            };
            let identifier = repo_id
                .map(|id| format!("repo id={id}"))
                .unwrap_or_else(|| "untracked component".to_string());
            match result {
                Ok(()) => app.log(
                    LogLevel::Info,
                    &format!(
                        "MPQ editor {}: \"{target_name}\" ({identifier}).",
                        if editor_unlocked {
                            "unlocked"
                        } else {
                            "locked"
                        }
                    ),
                ),
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!(
                            "MPQ editor lock change failed: \"{target_name}\" ({identifier}): {error}"
                        ),
                    );
                    app.mpq_ui.error = Some(error);
                    return Some(Task::none());
                }
            }
            Some(Task::batch([
                Task::perform(
                    service::load_mpq_protection(app.db_path.clone(), app.wow_dir.clone()),
                    Message::MpqProtectionLoaded,
                ),
                crate::update::repos::refresh_repos_task(app),
            ]))
        }
        Message::ToggleUntrackedMpqEnabled(path, enabled) => {
            if app.mpq_ui.busy {
                return Some(Task::none());
            }
            if matches!(app.dialog.as_ref(), Some(Dialog::ProtectedMpqs))
                && app
                    .mpq_ui
                    .protection
                    .iter()
                    .find(|entry| entry.path == path)
                    .map(|entry| !entry.editor_unlocked)
                    .unwrap_or(true)
            {
                return Some(Task::none());
            }
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            let target_name = untracked_component_name(app, &path);
            app.log(
                LogLevel::Info,
                &format!(
                    "MPQ state change requested: untracked component \"{target_name}\" -> {}.",
                    enabled_state(enabled)
                ),
            );
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::set_untracked_mpq_enabled(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    path,
                    enabled,
                ),
                move |result| Message::UntrackedMpqEnabledChanged {
                    target_name: target_name.clone(),
                    enabled,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::UntrackedMpqEnabledChanged {
            target_name,
            enabled,
            result,
        } => {
            app.mpq_ui.busy = false;
            let Some(result) =
                app.accept_profile_result(result, "untracked MPQ enable-state update")
            else {
                return Some(Task::none());
            };
            match result {
                Ok(()) => app.log(
                    LogLevel::Info,
                    &format!(
                        "Untracked MPQ {}: \"{target_name}\" (filesystem rename and metadata commit completed).",
                        enabled_state(enabled)
                    ),
                ),
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!(
                            "Untracked MPQ state change failed: \"{target_name}\" (requested_state={}): {error}",
                            enabled_state(enabled)
                        ),
                    );
                    app.mpq_ui.error = Some(error);
                    return Some(Task::none());
                }
            }
            Some(Task::batch([
                Task::perform(
                    service::load_mpq_protection(app.db_path.clone(), app.wow_dir.clone()),
                    Message::MpqProtectionLoaded,
                ),
                crate::update::repos::refresh_repos_task(app),
            ]))
        }
        Message::SetMpqEditorDisplayName(value) => {
            if let Some(Dialog::EditUntrackedMpq {
                edited_display_name,
                ..
            }) = app.dialog.as_mut()
            {
                *edited_display_name = value;
            }
            Some(Task::none())
        }
        Message::SetMpqEditorFileName(value) => {
            if let Some(Dialog::EditUntrackedMpq {
                edited_file_name, ..
            }) = app.dialog.as_mut()
            {
                *edited_file_name = value;
            }
            Some(Task::none())
        }
        Message::SetMpqEditorDestination(destination) => {
            if let Some(Dialog::EditUntrackedMpq {
                edited_destination, ..
            }) = app.dialog.as_mut()
            {
                *edited_destination = destination;
            }
            Some(Task::none())
        }
        Message::SetMpqEditorCore(core) => {
            if let Some(Dialog::EditUntrackedMpq { edited_core, .. }) = app.dialog.as_mut() {
                *edited_core = core;
            }
            Some(Task::none())
        }
        Message::SaveMpqEditor => {
            let (
                path,
                target_name,
                edited_display_name,
                edited_file_name,
                edited_destination,
                edited_core,
                friendly_name_changed,
                filename_changed,
                destination_changed,
                classification_changed,
            ) = match app.dialog.as_ref() {
                Some(Dialog::EditUntrackedMpq {
                    path,
                    display_name,
                    edited_display_name,
                    file_name,
                    edited_file_name,
                    destination,
                    edited_destination,
                    core,
                    edited_core,
                }) => (
                    path.clone(),
                    display_name.clone(),
                    edited_display_name.clone(),
                    edited_file_name.clone(),
                    edited_destination.clone(),
                    *edited_core,
                    display_name != edited_display_name,
                    file_name != edited_file_name,
                    destination != edited_destination,
                    core != edited_core,
                ),
                _ => return Some(Task::none()),
            };
            app.log(
                LogLevel::Info,
                &format!(
                    "Untracked MPQ edit requested: \"{target_name}\" (friendly_name_changed={friendly_name_changed}; on_disk_rename={filename_changed}; destination_changed={destination_changed}; classification_changed={classification_changed})."
                ),
            );
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::edit_untracked_mpq(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    path,
                    edited_display_name,
                    edited_file_name,
                    edited_destination,
                    edited_core,
                    app.opt_xattr,
                ),
                Message::MpqEditorSaved,
            ))
        }
        Message::MpqEditorSaved(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(_) => {
                    app.log(
                        LogLevel::Info,
                        "Untracked MPQ edit committed: filesystem and metadata are synchronized.",
                    );
                    app.show_toast("MPQ settings updated.", ToastKind::Success);
                    Some(Task::done(Message::OpenMpqProtection))
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Untracked MPQ edit failed; changes were not committed: {error}"),
                    );
                    app.mpq_ui.error = Some(error);
                    Some(Task::none())
                }
            }
        }
        Message::SetManualMpqDisplayName(value) => {
            if let Some(Dialog::ManualMpq {
                edited_display_name,
                ..
            }) = app.dialog.as_mut()
            {
                *edited_display_name = value;
            }
            Some(Task::none())
        }
        Message::SaveManualMpqDisplayName => {
            let (path, target_name, edited_display_name) = match app.dialog.as_ref() {
                Some(Dialog::ManualMpq {
                    path,
                    display_name,
                    edited_display_name,
                }) => (
                    path.clone(),
                    display_name.clone(),
                    edited_display_name.clone(),
                ),
                _ => return Some(Task::none()),
            };
            app.log(
                LogLevel::Info,
                &format!("Untracked MPQ friendly-name change requested: \"{target_name}\"."),
            );
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::rename_untracked_mpq(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    path,
                    edited_display_name,
                    app.opt_xattr,
                ),
                Message::ManualMpqDisplayNameSaved,
            ))
        }
        Message::ManualMpqDisplayNameSaved(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(()) => {
                    app.log(
                        LogLevel::Info,
                        "Untracked MPQ friendly-name metadata committed.",
                    );
                    app.dialog = None;
                    app.show_toast("MPQ friendly name saved.", ToastKind::Success);
                    Some(Task::perform(
                        service::load_mpq_protection(app.db_path.clone(), app.wow_dir.clone()),
                        Message::MpqProtectionLoaded,
                    ))
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Untracked MPQ friendly-name change failed: {error}"),
                    );
                    app.mpq_ui.error = Some(error);
                    Some(Task::none())
                }
            }
        }
        Message::SetManualMpqFileName(value) => {
            if let Some(Dialog::RenameManualMpq {
                edited_file_name, ..
            }) = app.dialog.as_mut()
            {
                *edited_file_name = value;
            }
            Some(Task::none())
        }
        Message::SaveManualMpqFileName => {
            let (old_path, edited_file_name) = match app.dialog.as_ref() {
                Some(Dialog::RenameManualMpq {
                    path,
                    edited_file_name,
                    ..
                }) => (path.clone(), edited_file_name.clone()),
                _ => return Some(Task::none()),
            };
            let target_name = untracked_component_name(app, &old_path);
            app.log(
                LogLevel::Info,
                &format!(
                    "Untracked MPQ on-disk rename requested: \"{target_name}\"; filenames and paths omitted from diagnostics."
                ),
            );
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::rename_untracked_mpq_file(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    old_path.clone(),
                    edited_file_name,
                ),
                move |result| Message::ManualMpqFileRenamed(old_path.clone(), result),
            ))
        }
        Message::ManualMpqFileRenamed(old_path, result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(new_path) => {
                    let return_to_manage = matches!(
                        app.dialog.as_ref(),
                        Some(Dialog::RenameManualMpq {
                            return_to_manage: true,
                            ..
                        })
                    );
                    let old_key = manage_path_key(&old_path);
                    let new_key = manage_path_key(&new_path);
                    if let Some(key) = app
                        .mpq_ui
                        .manage_order
                        .iter_mut()
                        .find(|key| **key == old_key)
                    {
                        *key = new_key;
                    }
                    app.log(
                        LogLevel::Info,
                        "Untracked MPQ on-disk rename committed: filesystem and metadata are synchronized.",
                    );
                    app.show_toast("MPQ file renamed.", ToastKind::Success);
                    app.dialog = None;
                    if return_to_manage {
                        Some(Task::done(Message::OpenMpqProtection))
                    } else {
                        Some(crate::update::repos::refresh_repos_task(app))
                    }
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Untracked MPQ on-disk rename failed: {error}"),
                    );
                    app.mpq_ui.error = Some(error);
                    Some(Task::none())
                }
            }
        }
        Message::SetMpqComponentDisplayName(value) => {
            if let Some(Dialog::MpqComponent {
                edited_display_name,
                ..
            }) = app.dialog.as_mut()
            {
                *edited_display_name = value;
            }
            Some(Task::none())
        }
        Message::SetMpqComponentFileName(value) => {
            if let Some(Dialog::MpqComponent {
                edited_file_name, ..
            }) = app.dialog.as_mut()
            {
                *edited_file_name = value;
            }
            Some(Task::none())
        }
        Message::SetMpqComponentDestination(destination) => {
            if let Some(Dialog::MpqComponent {
                edited_destination, ..
            }) = app.dialog.as_mut()
            {
                *edited_destination = destination;
            }
            Some(Task::none())
        }
        Message::SaveMpqComponentDisplayName => {
            let (
                repo_id,
                path,
                target_name,
                edited_display_name,
                edited_file_name,
                edited_destination,
                friendly_name_changed,
                filename_changed,
                destination_changed,
            ) = match app.dialog.as_ref() {
                Some(Dialog::MpqComponent {
                    repo_id,
                    path,
                    display_name,
                    edited_display_name,
                    file_name,
                    edited_file_name,
                    destination,
                    edited_destination,
                    ..
                }) => (
                    *repo_id,
                    path.clone(),
                    display_name.clone(),
                    edited_display_name.clone(),
                    edited_file_name.clone(),
                    edited_destination.clone(),
                    display_name != edited_display_name,
                    file_name != edited_file_name,
                    destination != edited_destination,
                ),
                _ => return Some(Task::none()),
            };
            app.log(
                LogLevel::Info,
                &format!(
                    "Tracked MPQ edit requested: \"{target_name}\" (repo id={repo_id}; friendly_name_changed={friendly_name_changed}; on_disk_rename={filename_changed}; destination_changed={destination_changed})."
                ),
            );
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::rename_mpq_component(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    repo_id,
                    path,
                    edited_display_name,
                    edited_file_name,
                    edited_destination,
                    app.opt_xattr,
                ),
                Message::MpqComponentDisplayNameSaved,
            ))
        }
        Message::MpqComponentDisplayNameSaved(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(_) => {
                    app.log(
                        LogLevel::Info,
                        "Tracked MPQ edit committed: filesystem and package metadata are synchronized.",
                    );
                    app.dialog = None;
                    app.show_toast("MPQ settings updated.", ToastKind::Success);
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Tracked MPQ edit failed; changes were not committed: {error}"),
                    );
                    app.mpq_ui.error = Some(error);
                    Some(Task::none())
                }
            }
        }
        Message::RemoveMpqComponent(force_modified) => {
            let (repo_id, path, target_name) = match app.dialog.as_ref() {
                Some(Dialog::MpqComponent {
                    repo_id,
                    path,
                    display_name,
                    ..
                }) => (*repo_id, path.clone(), display_name.clone()),
                _ => return Some(Task::none()),
            };
            app.log(
                LogLevel::Info,
                &format!(
                    "Tracked MPQ removal requested: \"{target_name}\" (repo id={repo_id}; force_modified={force_modified})."
                ),
            );
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::remove_mpq_component(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    repo_id,
                    path,
                    force_modified,
                ),
                Message::MpqComponentRemoved,
            ))
        }
        Message::MpqComponentRemoved(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(()) => {
                    app.log(
                        LogLevel::Info,
                        "Tracked MPQ component removed; package metadata and any applicable backup restoration were committed.",
                    );
                    app.dialog = None;
                    app.show_toast("MPQ removed.", ToastKind::Info);
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Tracked MPQ component removal failed: {error}"),
                    );
                    app.mpq_ui.error = Some(error);
                    Some(Task::none())
                }
            }
        }
        Message::KeepModifiedMpqProtected => {
            let (repo_id, path, target_name) = match app.dialog.as_ref() {
                Some(Dialog::MpqComponent {
                    repo_id,
                    path,
                    display_name,
                    ..
                }) => (*repo_id, path.clone(), display_name.clone()),
                _ => return Some(Task::none()),
            };
            app.log(
                LogLevel::Info,
                &format!(
                    "Keeping externally modified MPQ requested: \"{target_name}\" (repo id={repo_id}); the file will become protected."
                ),
            );
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::protect_modified_mpq(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    repo_id,
                    path,
                ),
                Message::ModifiedMpqProtected,
            ))
        }
        Message::ModifiedMpqProtected(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(()) => {
                    app.log(
                        LogLevel::Info,
                        "Externally modified MPQ retained and protection metadata committed.",
                    );
                    app.dialog = None;
                    app.show_toast("Modified MPQ kept and protected.", ToastKind::Info);
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Could not retain and protect the modified MPQ: {error}"),
                    );
                    app.mpq_ui.error = Some(error);
                    Some(Task::none())
                }
            }
        }
        Message::OpenWdm => {
            app.open_menu = None;
            let existing_wdm = app.repos.iter().find(|repo| service::is_wdm_repo(repo));
            let had_caverns = existing_wdm
                .map(|repo| {
                    repo.installed_mpqs.iter().any(|entry| {
                        let path = entry.path.to_ascii_lowercase();
                        path.strip_suffix(".disabled")
                            .unwrap_or(&path)
                            .ends_with("-n.mpq")
                    })
                })
                .unwrap_or(false);
            let had_companion = existing_wdm
                .map(|repo| {
                    repo.dependencies
                        .iter()
                        .any(|(_, relationship)| relationship == "wdm-companion")
                })
                .unwrap_or(false);
            app.mpq_ui.catalog = None;
            app.mpq_ui.wdm_locale = None;
            app.mpq_ui.wdm_caverns = had_caverns;
            app.mpq_ui.wdm_addon = existing_wdm.is_none() || had_companion || had_caverns;
            app.mpq_ui.error = None;
            app.dialog = Some(Dialog::WdmInstall);
            let (operation_id, scope) = begin_operation(app, false);
            Some(Task::perform(
                service::resolve_wdm(app.db_path.clone(), app.wow_dir.clone()),
                move |result| Message::WdmResolved {
                    operation_id,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::WdmResolved {
            operation_id,
            result,
        } => {
            let Some(result) = accept_operation(app, operation_id, result, "WDM resolution") else {
                return Some(Task::none());
            };
            match result {
                Ok(catalog) => {
                    app.mpq_ui.wdm_locale = catalog.locale.recommended.clone();
                    app.mpq_ui.catalog = Some(catalog);
                    app.mpq_ui.error = None;
                }
                Err(error) => {
                    app.show_github_rate_limit("WDM information could not be loaded.", &error);
                    app.mpq_ui.error = Some(crate::github_api::user_facing_error(&error));
                }
            }
            Some(Task::none())
        }
        Message::SetWdmLocale(locale) => {
            app.mpq_ui.wdm_locale = Some(locale);
            Some(Task::none())
        }
        Message::ToggleWdmCaverns(enabled) => {
            app.mpq_ui.wdm_caverns = enabled;
            if enabled {
                app.mpq_ui.wdm_addon = true;
            }
            Some(Task::none())
        }
        Message::ToggleWdmAddon(enabled) => {
            if !app.mpq_ui.wdm_caverns {
                app.mpq_ui.wdm_addon = enabled;
            }
            Some(Task::none())
        }
        Message::InstallWdm => {
            let (Some(catalog), Some(locale)) =
                (app.mpq_ui.catalog.clone(), app.mpq_ui.wdm_locale.clone())
            else {
                app.mpq_ui.error = Some("Choose the WoW client locale first.".to_string());
                return Some(Task::none());
            };
            app.mpq_ui.error = None;
            let options = wuddle_engine::InstallOptions {
                use_symlinks: app.opt_symlinks,
                set_xattr_comment: app.opt_xattr,
                replace_addon_conflicts: false,
                replace_file_conflicts: false,
                cache_keep_versions: 0,
            };
            let (operation_id, scope) = begin_operation(app, true);
            Some(Task::perform(
                service::install_wdm(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    catalog,
                    locale,
                    app.mpq_ui.wdm_caverns,
                    app.mpq_ui.wdm_addon,
                    options,
                ),
                move |result| Message::WdmInstallFinished {
                    operation_id,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::WdmInstallFinished {
            operation_id,
            result,
        } => {
            let Some(result) = accept_operation(app, operation_id, result, "WDM installation")
            else {
                return Some(Task::none());
            };
            match result {
                Ok(_) => {
                    app.dialog = None;
                    app.log(LogLevel::Info, "WDM installed successfully.");
                    app.show_toast("WDM installed successfully.", ToastKind::Success);
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.show_github_rate_limit("WDM could not be installed.", &error);
                    app.mpq_ui.error = Some(crate::github_api::user_facing_error(&error));
                    Some(Task::none())
                }
            }
        }
        Message::ToggleRemoveWdmAddon(remove) => {
            if let Some(Dialog::RemoveWdm { remove_addon, .. }) = app.dialog.as_mut() {
                *remove_addon = remove;
            }
            Some(Task::none())
        }
        Message::ConfirmRemoveWdm => {
            let Some((repo_id, addon_repo_id, remove_addon)) =
                app.dialog.as_ref().and_then(|dialog| match dialog {
                    Dialog::RemoveWdm {
                        repo_id,
                        addon_repo_id,
                        remove_addon,
                    } => Some((*repo_id, *addon_repo_id, *remove_addon)),
                    _ => None,
                })
            else {
                return Some(Task::none());
            };
            app.mpq_ui.error = None;
            let (operation_id, scope) = begin_operation(app, true);
            Some(Task::perform(
                service::remove_wdm(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    repo_id,
                    addon_repo_id,
                    remove_addon,
                ),
                move |result| Message::WdmRemoved {
                    operation_id,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::WdmRemoved {
            operation_id,
            result,
        } => {
            let Some(result) = accept_operation(app, operation_id, result, "WDM removal") else {
                return Some(Task::none());
            };
            match result {
                Ok(()) => {
                    app.dialog = None;
                    app.show_toast("WDM removed.", ToastKind::Info);
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.mpq_ui.error = Some(error);
                    Some(Task::none())
                }
            }
        }
        Message::OpenWdmReadme => {
            let generation = app.begin_preview_request();
            app.markdown_image_cache.clear();
            app.markdown_gif_cache.clear();
            app.dialog = Some(Dialog::Changelog {
                title: "WDM — README".to_string(),
                items: Vec::new(),
                loading: true,
            });
            Some(Task::perform(
                service::fetch_repo_preview("https://github.com/Trimitor/WDM-patch".to_string()),
                move |result| Message::WdmReadmeLoaded(generation, result),
            ))
        }
        Message::WdmReadmeLoaded(generation, result) => {
            if !app.preview_request_is_current(generation, "WDM README") {
                return Some(Task::none());
            }
            let loaded_items = match result {
                Ok(preview) => {
                    app.markdown_image_cache = preview.image_cache;
                    app.markdown_gif_cache = preview.gif_cache;
                    preview.readme_items
                }
                Err(error) => {
                    app.show_github_rate_limit("The WDM README could not be loaded.", &error);
                    iced::widget::markdown::Content::parse(&format!(
                        "Could not load the WDM README.\n\n{}",
                        crate::github_api::user_facing_error(&error)
                    ))
                    .items()
                    .to_vec()
                }
            };
            if let Some(Dialog::Changelog {
                items: dialog_items,
                loading,
                ..
            }) = app.dialog.as_mut()
            {
                *loading = false;
                *dialog_items = loaded_items;
            }
            Some(Task::none())
        }
        _ => None,
    }
}

fn heading<'a>(title: &'a str, subtitle: &'a str, colors: ThemeColors) -> Element<'a, Message> {
    column![
        row![
            text(title).size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(colors),
        ]
        .align_y(iced::Alignment::Center),
        dialog_description(subtitle, colors),
    ]
    .spacing(4)
    .into()
}

fn error_view(error: Option<&str>, colors: ThemeColors) -> Element<'_, Message> {
    match error {
        Some(error) => container(text(error).size(13).color(colors.bad))
            .padding(8)
            .width(Length::Fill)
            .style(move |_theme| theme::card_style(colors))
            .into(),
        None => Space::new().height(0).into(),
    }
}

fn secondary_button_style(colors: ThemeColors, status: button::Status) -> button::Style {
    match status {
        button::Status::Hovered => theme::tab_button_hovered_style(colors),
        button::Status::Disabled => {
            let mut style = theme::tab_button_style(colors);
            style.text_color.a = 0.38;
            if let Some(iced::Background::Color(mut background)) = style.background {
                background.a *= 0.35;
                style.background = Some(iced::Background::Color(background));
            }
            style.border.color.a *= 0.35;
            style
        }
        button::Status::Pressed | button::Status::Active => theme::tab_button_style(colors),
    }
}

fn installed_badge() -> Element<'static, Message> {
    container(
        text("Installed")
            .size(12)
            .color(iced::Color::from_rgb8(0x34, 0xd3, 0x99)),
    )
    .padding([4, 10])
    .style(move |_theme| container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba8(
            0x10, 0xb9, 0x81, 0.15,
        ))),
        border: iced::Border {
            color: iced::Color::from_rgba8(0x10, 0xb9, 0x81, 0.4),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn view_add(app: &App, colors: ThemeColors) -> Element<'_, Message> {
    let supports_wdm = app
        .tweak_client_info
        .as_ref()
        .map(|info| info.is_wotlk_335a_12340)
        .unwrap_or(false);
    let quick_add_label = if supports_wdm {
        "Quick Add · Wrath of the Lich King 3.0–3.3.5"
    } else {
        "Quick Add"
    };

    let quick_add: Element<Message> = if supports_wdm {
        let wdm_installed = app.repos.iter().any(service::is_wdm_repo);
        let epoch_water_installed = app.repos.iter().any(service::is_epoch_water_repo);
        let title = button(iced::widget::rich_text::<(), _, _, _>([
            iced::widget::span("WDM")
                .underline(true)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                })
                .color(colors.link)
                .size(22.0_f32),
        ]))
        .on_press(Message::OpenWdm)
        .padding(0)
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: colors.link,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        });
        let wdm_readme = crate::components::presets::quick_add_readme_button(
            "WDM",
            "https://github.com/Trimitor/WDM-patch",
            colors,
        );
        let tags = row![
            badge_tag(
                "Recommended",
                iced::Color::from_rgb8(0x34, 0xd3, 0x99),
                iced::Color::from_rgb8(0x10, 0xb9, 0x81),
            ),
            badge_tag(
                "MPQ",
                iced::Color::from_rgb8(0x93, 0xc5, 0xfd),
                iced::Color::from_rgb8(0x3b, 0x82, 0xf6),
            ),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);
        let wdm_button = button(
            text(if app.mpq_ui.busy {
                "Working..."
            } else if wdm_installed {
                "Configure"
            } else {
                "Install"
            })
            .size(12),
        )
        .padding([4, 14])
        .style(move |_theme, _status| theme::tab_button_active_style(colors));
        let wdm_button: Element<Message> = if app.mpq_ui.busy {
            wdm_button.into()
        } else {
            wdm_button.on_press(Message::OpenWdm).into()
        };
        let wdm_card = container(
            column![
                row![title, wdm_readme, tags]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                text("Adds dungeon maps to the 3.3.5 client, with an optional Caverns & Mines patch and companion addon.")
                    .size(16)
                    .color(colors.title),
                row![
                    Space::new().width(Length::Fill),
                    wdm_button,
                ],
            ]
            .spacing(6),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(colors));

        let epoch_title = button(iced::widget::rich_text::<(), _, _, _>([
            iced::widget::span("Epoch Water")
                .underline(true)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                })
                .color(colors.link)
                .size(22.0_f32),
        ]))
        .on_press(Message::OpenUrl(service::EPOCH_WATER_URL.to_string()))
        .padding(0)
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: colors.link,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        });
        let epoch_readme = crate::components::presets::quick_add_readme_button(
            "Epoch Water",
            service::EPOCH_WATER_URL,
            colors,
        );
        let epoch_tags = row![badge_tag(
            "MPQ",
            iced::Color::from_rgb8(0x93, 0xc5, 0xfd),
            iced::Color::from_rgb8(0x3b, 0x82, 0xf6),
        )]
        .spacing(4)
        .align_y(iced::Alignment::Center);
        let epoch_button = button(
            text(if app.mpq_ui.busy {
                "Working..."
            } else {
                "Install"
            })
            .size(12),
        )
        .padding([4, 14])
        .style(move |_theme, _status| theme::tab_button_active_style(colors));
        let epoch_button: Element<Message> = if epoch_water_installed {
            installed_badge()
        } else if app.mpq_ui.busy {
            epoch_button.into()
        } else {
            epoch_button.on_press(Message::InstallEpochWater).into()
        };
        let epoch_card = container(
            column![
                row![epoch_title, epoch_readme, epoch_tags]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                text("Replaces the default water texture with Project Epoch's water.")
                    .size(16)
                    .color(colors.title),
                row![Space::new().width(Length::Fill), epoch_button],
            ]
            .spacing(6),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(colors));

        column![wdm_card, epoch_card].spacing(8).into()
    } else {
        container(
            text("No curated MPQ packages are available for the detected client.")
                .size(16)
                .color(colors.muted),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(colors))
        .into()
    };

    column![
        row![
            text("Add an MPQ patch").size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(colors),
        ]
        .align_y(iced::Alignment::Center),
        dialog_description(
            "Quick-add a curated patch or install an MPQ package from your computer.",
            colors,
        ),
        rule::horizontal(1).style(move |_theme| theme::update_line_style(colors)),
        dialog_field_label("MPQ URL", colors),
        text_input(
            "(e.g. https://example.com/patch-name.mpq)",
            &app.mpq_ui.direct_url,
        )
        .on_input(Message::SetMpqDirectUrl)
        .padding([8, 12]),
        dialog_description(
            "Direct downloads are not enabled yet; download hosted files in your browser and use Local Installation.",
            colors,
        ),
        dialog_field_label(quick_add_label, colors),
        quick_add,
        rule::horizontal(1).style(move |_theme| theme::update_line_style(colors)),
        row![
            button(text("Local Installation...").size(13))
                .on_press(Message::OpenMpqInstall)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            button(text("Manage MPQs...").size(13))
                .on_press(Message::OpenMpqProtection)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            Space::new().width(Length::Fill),
            button(text("Close").size(13))
                .on_press(Message::CloseDialog)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
        ]
        .spacing(8),
    ]
    .spacing(6)
    .width(Length::Fill)
    .into()
}

fn view_install(app: &App, colors: ThemeColors) -> Element<'_, Message> {
    let source_label = app
        .mpq_ui
        .source
        .as_ref()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "No file selected".to_string());
    let mut items: Vec<Element<Message>> = Vec::new();
    if let Some(inspection) = &app.mpq_ui.inspection {
        for (index, selection) in app.mpq_ui.selections.iter().enumerate() {
            let mut destinations = inspection.destinations.clone();
            if !destinations.contains(&selection.destination) {
                destinations.push(selection.destination.clone());
            }
            let preview = app
                .mpq_ui
                .targets_reviewed
                .then(|| {
                    app.mpq_ui
                        .target_previews
                        .iter()
                        .find(|preview| preview.source_key == selection.source_key)
                })
                .flatten();
            let preview_text = preview
                .map(|preview| {
                    format!(
                        "Target: {} — {}",
                        preview.manifest_path,
                        preview.status.label()
                    )
                })
                .unwrap_or_else(|| "Target has not been reviewed yet".to_string());
            let preview_color = if preview
                .map(|item| item.status.blocks_install())
                .unwrap_or(false)
            {
                colors.bad
            } else {
                colors.muted
            };
            let card = column![
                text(
                    inspection
                        .candidates
                        .get(index)
                        .map(|candidate| candidate.source_key.as_str())
                        .unwrap_or("MPQ")
                )
                .size(12)
                .color(colors.muted),
                row![
                    column![
                        dialog_field_label("Friendly name", colors),
                        text_input("Required label", &selection.display_name)
                            .on_input(move |value| Message::SetMpqDisplayName(index, value))
                    ]
                    .spacing(3)
                    .width(Length::Fill),
                    column![
                        dialog_field_label("On-disk filename", colors),
                        text_input("patch-name.MPQ", &selection.file_name)
                            .on_input(move |value| Message::SetMpqFileName(index, value))
                    ]
                    .spacing(3)
                    .width(Length::Fill),
                ]
                .spacing(10),
                row![
                    dialog_field_label("Destination", colors),
                    pick_list(
                        destinations,
                        Some(selection.destination.clone()),
                        move |value| { Message::SetMpqDestination(index, value) }
                    ),
                    checkbox(selection.replace_unprotected)
                        .label("Allow backed-up replacement")
                        .on_toggle(move |value| Message::ToggleMpqReplacement(index, value)),
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                text(preview_text).size(11).color(preview_color),
            ]
            .spacing(8);
            items.push(
                container(card)
                    .padding(10)
                    .width(Length::Fill)
                    .style(move |_theme| theme::card_style(colors))
                    .into(),
            );
        }
    }
    let ready_to_install = app.mpq_ui.targets_reviewed
        && app.mpq_ui.target_previews.iter().all(|preview| {
            if preview.status.blocks_install() {
                return false;
            }
            if preview.status == wuddle_engine::mpq::MpqTargetStatus::UnprotectedReplacement {
                return app
                    .mpq_ui
                    .selections
                    .iter()
                    .find(|selection| selection.source_key == preview.source_key)
                    .map(|selection| selection.replace_unprotected)
                    .unwrap_or(false);
            }
            true
        });
    let install = button(
        text(if app.mpq_ui.busy {
            "Working..."
        } else if app.mpq_ui.targets_reviewed {
            "Confirm install"
        } else {
            "Review installation"
        })
        .size(13),
    )
    .padding([6, 14])
    .style(move |_theme, _status| theme::tab_button_active_style(colors));
    let install: Element<Message> = if !app.mpq_ui.busy
        && !app.mpq_ui.selections.is_empty()
        && (!app.mpq_ui.targets_reviewed || ready_to_install)
    {
        install.on_press(Message::InstallMpqPackage).into()
    } else {
        install.into()
    };
    let has_inspection = app.mpq_ui.inspection.is_some();
    let inspected_files: Element<Message> = if has_inspection {
        scrollable(column(items).spacing(8))
            .height(Length::FillPortion(1))
            .into()
    } else {
        Space::new().height(0).into()
    };
    let mut content =
        column![
        heading(
            "Install local MPQ",
            "MPQs are inspected in staging. Nothing reaches Data/ until you confirm every file.",
            colors,
        ),
        row![
            text(source_label).size(13).color(colors.text),
            Space::new().width(Length::Fill),
            button(text("Choose MPQ / ZIP / 7z...").size(13))
                .on_press(Message::PickMpqSource)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        dialog_description("You can also drop a supported file onto this dialog.", colors),
        error_view(app.mpq_ui.error.as_deref(), colors),
        inspected_files,
        row![
            button(text("Manage MPQs...").size(13))
                .on_press(Message::OpenMpqProtection)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            Space::new().width(Length::Fill),
            button(text("Back").size(13))
                .on_press(Message::OpenMpqAdd)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            install,
        ]
        .spacing(8),
    ]
        .spacing(12)
        .width(Length::Fill);
    if has_inspection {
        content = content.height(Length::Fill);
    }
    content.into()
}

fn editable_mpq_file_name(file_name: &str) -> String {
    let lower = file_name.to_ascii_lowercase();
    lower
        .strip_suffix(".disabled")
        .map(|_| file_name[..file_name.len() - ".disabled".len()].to_string())
        .unwrap_or_else(|| file_name.to_string())
}

fn mpq_destination_from_path(path: &str) -> wuddle_engine::mpq::MpqDestination {
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("Data") {
        if let Some(locale) = wuddle_engine::mpq::normalize_locale(parts[1]) {
            return wuddle_engine::mpq::MpqDestination::Locale(locale);
        }
    }
    wuddle_engine::mpq::MpqDestination::DataRoot
}

fn edit_destination_options(
    app: &App,
    current: &wuddle_engine::mpq::MpqDestination,
) -> Vec<wuddle_engine::mpq::MpqDestination> {
    let mut destinations = vec![wuddle_engine::mpq::MpqDestination::DataRoot];
    if let Some(locale) = app
        .mpq_ui
        .detected_locale
        .as_deref()
        .and_then(wuddle_engine::mpq::normalize_locale)
    {
        destinations.push(wuddle_engine::mpq::MpqDestination::Locale(locale));
    } else if !destinations.contains(current) {
        // With no reliable locale detection, retain the file's existing
        // location rather than offering every unrelated WoW locale.
        destinations.push(current.clone());
    }
    destinations
}

pub fn component_dialog(repo_id: i64, entry: &wuddle_engine::mpq::MpqInstalledFile) -> Dialog {
    let file_name = std::path::Path::new(&entry.path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(editable_mpq_file_name)
        .unwrap_or_else(|| "patch.MPQ".to_string());
    let destination = mpq_destination_from_path(&entry.path);
    Dialog::MpqComponent {
        repo_id,
        path: entry.path.clone(),
        display_name: entry.display_name.clone(),
        edited_display_name: entry.display_name.clone(),
        file_name: file_name.clone(),
        edited_file_name: file_name,
        destination: destination.clone(),
        edited_destination: destination,
        status: entry.status,
    }
}

pub fn untracked_component_dialog(entry: &wuddle_engine::mpq::MpqProtectionEntry) -> Dialog {
    let visible_name = entry
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(&entry.file_name)
        .to_string();
    let file_name = editable_mpq_file_name(&entry.file_name);
    let destination = mpq_destination_from_path(&entry.path);
    Dialog::EditUntrackedMpq {
        path: entry.path.clone(),
        display_name: visible_name.clone(),
        edited_display_name: visible_name,
        file_name: file_name.clone(),
        edited_file_name: file_name,
        destination: destination.clone(),
        edited_destination: destination,
        core: entry.core,
        edited_core: entry.core,
    }
}

fn manage_section<'a>(
    title: &'a str,
    subtitle: &'a str,
    entries: Vec<Element<'a, Message>>,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let body: Element<Message> = if entries.is_empty() {
        container(text("None detected.").size(14).color(colors.muted))
            .padding([6, 10])
            .into()
    } else {
        column(entries).spacing(6).into()
    };
    column![
        dialog_field_label(title, colors),
        dialog_description(subtitle, colors),
        body,
    ]
    .spacing(4)
    .into()
}

fn view_untracked_manage_entry<'a>(
    app: &'a App,
    entry: &'a wuddle_engine::mpq::MpqProtectionEntry,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let enabled_path = entry.path.clone();
    let locked = !entry.editor_unlocked;
    let lock_color = if locked { colors.warn } else { colors.good };
    let lock_action = Message::SetUntrackedMpqEditorUnlocked(entry.path.clone(), locked);
    let lock_button = button(protection_icon(locked, lock_color))
        .padding([4, 6])
        .style(move |_theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(iced::Background::Color(iced::Color {
                    a: 0.12,
                    ..lock_color
                })),
                _ => None,
            },
            text_color: lock_color,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        });
    let lock_button: Element<Message> = if app.mpq_ui.busy {
        lock_button.into()
    } else {
        lock_button.on_press(lock_action).into()
    };
    let lock_tip = if entry.core {
        "Unlock this core file to enable its Edit button without changing its classification."
    } else if locked {
        "Unlock this MPQ to enable its Edit button."
    } else {
        "Lock this MPQ to disable its Edit button."
    };

    let enabled = checkbox(entry.enabled).size(27);
    let enabled: Element<Message> = if !app.mpq_ui.busy && !locked {
        enabled
            .on_toggle(move |value| Message::ToggleUntrackedMpqEnabled(enabled_path.clone(), value))
            .into()
    } else {
        enabled.into()
    };
    let enabled_tip = if locked {
        "Unlock this MPQ before changing its enabled state."
    } else if entry.enabled {
        "Disable this MPQ by appending .disabled to its filename."
    } else {
        "Enable this MPQ by removing .disabled from its filename."
    };

    let visible_name = entry
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(&entry.file_name)
        .to_string();
    let edit_dialog = untracked_component_dialog(entry);
    let edit_button = button(text("Edit").size(12))
        .padding([5, 9])
        .style(move |_theme, status| secondary_button_style(colors, status));
    let edit_button: Element<Message> = if !app.mpq_ui.busy && !locked {
        edit_button
            .on_press(Message::OpenDialog(edit_dialog))
            .into()
    } else {
        edit_button.into()
    };

    container(
        row![
            column![
                text(visible_name).size(14).color(colors.title),
                text(&entry.path).size(11).color(colors.muted),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
            tip(
                lock_button,
                lock_tip,
                iced::widget::tooltip::Position::Top,
                colors
            ),
            tip(
                edit_button,
                if locked {
                    "Unlock this MPQ before editing it."
                } else {
                    "Edit this MPQ's friendly name, filename, and classification."
                },
                iced::widget::tooltip::Position::Top,
                colors
            ),
            tip(
                enabled,
                enabled_tip,
                iced::widget::tooltip::Position::Top,
                colors
            ),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
    )
    .padding(9)
    .width(Length::Fill)
    .style(move |_theme| theme::card_style(colors))
    .into()
}

fn view_managed_manage_entry<'a>(
    app: &'a App,
    repo: &'a service::RepoRow,
    entry: &'a wuddle_engine::mpq::MpqInstalledFile,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let dialog = component_dialog(repo.id, entry);
    let locked = !entry.editor_unlocked;
    let lock_color = if locked { colors.warn } else { colors.good };
    let lock_button = button(protection_icon(locked, lock_color))
        .padding([4, 6])
        .style(move |_theme, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(iced::Background::Color(iced::Color {
                    a: 0.12,
                    ..lock_color
                })),
                _ => None,
            },
            text_color: lock_color,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        });
    let lock_button: Element<Message> = if app.mpq_ui.busy {
        lock_button.into()
    } else {
        lock_button
            .on_press(Message::SetTrackedMpqEditorUnlocked(
                repo.id,
                entry.path.clone(),
                locked,
            ))
            .into()
    };
    let edit_button = button(text("Edit").size(12))
        .padding([5, 9])
        .style(move |_theme, status| secondary_button_style(colors, status));
    let edit_button: Element<Message> = if !app.mpq_ui.busy && !locked {
        edit_button.on_press(Message::OpenDialog(dialog)).into()
    } else {
        edit_button.into()
    };
    let enabled = checkbox(entry.enabled).size(27);
    let enabled: Element<Message> = if app.mpq_ui.busy || locked {
        enabled.into()
    } else {
        let path = entry.path.clone();
        enabled
            .on_toggle(move |value| Message::ToggleMpqEnabled(repo.id, path.clone(), value))
            .into()
    };
    container(
        row![
            column![
                text(&entry.display_name).size(14).color(colors.title),
                text(
                    if repo.forge == "local" && repo.owner == "local" && repo.url.is_empty() {
                        format!("Local installation • {}", entry.path)
                    } else {
                        format!("{} • {}", repo.name, entry.path)
                    }
                )
                .size(11)
                .color(colors.muted),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
            tip(
                lock_button,
                if locked {
                    "Unlock this MPQ to enable its Edit button."
                } else {
                    "Lock this MPQ to disable its Edit button."
                },
                iced::widget::tooltip::Position::Top,
                colors
            ),
            tip(
                edit_button,
                if locked {
                    "Unlock this MPQ before editing it."
                } else {
                    "Edit this MPQ's friendly name."
                },
                iced::widget::tooltip::Position::Top,
                colors
            ),
            tip(
                enabled,
                if locked {
                    "Unlock this MPQ before changing its enabled state."
                } else if entry.enabled {
                    "Disable this managed MPQ."
                } else {
                    "Enable this managed MPQ."
                },
                iced::widget::tooltip::Position::Top,
                colors
            ),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
    )
    .padding(9)
    .width(Length::Fill)
    .style(move |_theme| theme::card_style(colors))
    .into()
}

fn view_protection(app: &App, colors: ThemeColors) -> Element<'_, Message> {
    let mut ordered_untracked = app
        .mpq_ui
        .manage_order
        .iter()
        .filter_map(|key| {
            app.mpq_ui
                .protection
                .iter()
                .find(|entry| manage_path_key(&entry.path) == *key)
        })
        .collect::<Vec<_>>();
    ordered_untracked.extend(app.mpq_ui.protection.iter().filter(|entry| {
        !app.mpq_ui
            .manage_order
            .contains(&manage_path_key(&entry.path))
    }));

    let mut custom_entries = Vec::new();
    let mut core_entries = Vec::new();
    for entry in ordered_untracked {
        let key = manage_path_key(&entry.path);
        let known = app.mpq_ui.manage_order.contains(&key);
        let core_group = if known {
            app.mpq_ui.manage_core_keys.contains(&key)
        } else {
            entry.core
        };
        let element = view_untracked_manage_entry(app, entry, colors);
        if core_group {
            core_entries.push(element);
        } else {
            custom_entries.push(element);
        }
    }

    let managed_entries = app
        .mpq_ui
        .manage_managed_order
        .iter()
        .filter_map(|(repo_id, key)| {
            let repo = app.repos.iter().find(|repo| repo.id == *repo_id)?;
            let entry = repo
                .installed_mpqs
                .iter()
                .find(|entry| manage_path_key(&entry.path) == *key)?;
            Some(view_managed_manage_entry(app, repo, entry, colors))
        })
        .collect::<Vec<_>>();

    let sections = column![
        manage_section(
            "Wuddle-installed MPQs",
            "Tracked local and curated packages, including WDM.",
            managed_entries,
            colors,
        ),
        manage_section(
            "Custom and manual MPQs",
            "Detected archives that are not owned by a Wuddle package.",
            custom_entries,
            colors,
        ),
        manage_section(
            "Core client files",
            "Stock archives protected from accidental replacement.",
            core_entries,
            colors,
        ),
    ]
    .spacing(14);

    column![
        heading(
            "Manage MPQs",
            "Review every MPQ in Data/ and its locale directories.",
            colors,
        ),
        dialog_description(
            "Use the padlock to control whether a patch's Edit button is available. Classification and enabled state are independent.",
            colors,
        ),
        error_view(app.mpq_ui.error.as_deref(), colors),
        scrollable(
            container(sections)
                .padding(iced::Padding {
                    top: 0.0,
                    right: 10.0,
                    bottom: 0.0,
                    left: 0.0,
                })
                .width(Length::Fill),
        )
        .height(Length::FillPortion(1))
        .direction(theme::vscroll())
        .style(move |iced_theme, status| {
            theme::scrollable_style(colors)(iced_theme, status)
        }),
        row![
            button(text("+ Add MPQ...").size(13))
                .on_press(Message::OpenMpqAdd)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            Space::new().width(Length::Fill),
            button(text("Close").size(13))
                .on_press(Message::CloseDialog)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
        ],
    ]
    .spacing(12)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn view_wdm(app: &App, colors: ThemeColors) -> Element<'_, Message> {
    let locales = wuddle_engine::mpq::KNOWN_LOCALES
        .iter()
        .map(|locale| (*locale).to_string())
        .collect::<Vec<_>>();
    let locale_available = app.mpq_ui.wdm_locale.as_deref().and_then(|locale| {
        app.mpq_ui
            .catalog
            .as_ref()
            .map(|catalog| catalog.stable.locale_asset(locale, 'M').is_some())
    });
    let catalog_text = app.mpq_ui.catalog.as_ref().map(|catalog| {
        let optional = catalog
            .caverns
            .as_ref()
            .map(|release| release.version.as_str())
            .unwrap_or("unavailable");
        format!(
            "Main patch: {}  |  Caverns & Mines: {}  |  Addon: {}",
            catalog.stable.version, optional, catalog.addon.version
        )
    });
    let install = button(
        text(if app.mpq_ui.busy {
            "Working..."
        } else {
            "Install WDM"
        })
        .size(13),
    )
    .padding([6, 14])
    .style(move |_theme, _status| theme::tab_button_active_style(colors));
    let install: Element<Message> = if !app.mpq_ui.busy
        && app.mpq_ui.catalog.is_some()
        && app.mpq_ui.wdm_locale.is_some()
        && locale_available != Some(false)
    {
        install.on_press(Message::InstallWdm).into()
    } else {
        install.into()
    };
    column![
        heading(
            "Install WDM dungeon maps",
            "Curated for an exactly detected WoW 3.3.5a build 12340 client.",
            colors,
        ),
        dialog_description(
            "Wuddle checks curated WDM releases alongside mods and addons; updates are installed deliberately through this dialog.",
            colors,
        ),
        row![
            dialog_field_label("Client locale", colors),
            pick_list(locales, app.mpq_ui.wdm_locale.clone(), Message::SetWdmLocale),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
        text(catalog_text.unwrap_or_else(|| "Resolving WDM releases...".to_string()))
            .size(12)
            .color(colors.muted),
        checkbox(app.mpq_ui.wdm_addon)
            .label("Install WDM companion addon (recommended)")
            .on_toggle(Message::ToggleWdmAddon),
        checkbox(app.mpq_ui.wdm_caverns)
            .label("Install optional Caverns & Mines patch")
            .on_toggle(Message::ToggleWdmCaverns),
        text(if app.mpq_ui.wdm_caverns {
            "Caverns & Mines requires the companion addon, so the addon is forced on. Embedded Astrolabe satisfies its library requirement."
        } else {
            "The companion addon improves the main patch experience and can be updated normally through Wuddle."
        })
        .size(14)
        .color(colors.muted),
        error_view(app.mpq_ui.error.as_deref(), colors),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(13))
                .on_press(Message::CloseDialog)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            install,
        ]
        .spacing(8),
    ]
    .spacing(12)
    .width(Length::Fill)
    .into()
}

fn view_component<'a>(
    app: &'a App,
    dialog: &'a Dialog,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let Dialog::MpqComponent {
        path,
        display_name,
        edited_display_name,
        file_name,
        edited_file_name,
        edited_destination,
        status,
        ..
    } = dialog
    else {
        return Space::new().into();
    };
    let force_modified = *status == wuddle_engine::mpq::MpqFileStatus::Modified;
    let destinations = edit_destination_options(app, edited_destination);
    let valid = !edited_display_name.trim().is_empty()
        && !edited_file_name.trim().is_empty()
        && edited_file_name
            .trim()
            .to_ascii_lowercase()
            .ends_with(".mpq");
    column![
        heading("Edit MPQ", "Edit this Wuddle-installed patch.", colors),
        text(path).size(11).color(colors.muted),
        dialog_field_label("Friendly name", colors),
        text_input(display_name, edited_display_name)
            .on_input(Message::SetMpqComponentDisplayName),
        dialog_field_label("Filename on disk", colors),
        text_input(file_name, edited_file_name).on_input(Message::SetMpqComponentFileName),
        dialog_field_label("Location", colors),
        pick_list(
            destinations,
            Some(edited_destination.clone()),
            Message::SetMpqComponentDestination,
        ),
        dialog_field_label("Classification", colors),
        container(text("Wuddle-installed MPQ").size(13).color(colors.text))
            .padding([7, 10])
            .style(move |_theme| theme::card_style(colors)),
        dialog_description(
            "Wuddle keeps package ownership, update tracking, and backup metadata attached when this file is renamed or moved.",
            colors,
        ),
        text(format!("Status: {}", status.label()))
            .size(12)
            .color(if force_modified { colors.warn } else { colors.text }),
        if force_modified {
            text("This file changed outside Wuddle. Removing it requires explicit confirmation; otherwise Wuddle keeps and protects it.")
                .size(14)
                .color(colors.warn)
        } else {
            text("Remove MPQ removes only this component and restores any file it replaced. Other bundled patches and companion addons remain; use Remove package from the patch row's menu to remove a complete bundle.")
                .size(14)
                .color(colors.muted)
        },
        error_view(app.mpq_ui.error.as_deref(), colors),
        row![
            button(text(if force_modified {
                "Delete modified MPQ"
            } else {
                "Remove this MPQ"
            }).size(13))
            .on_press(Message::RemoveMpqComponent(force_modified))
            .padding([6, 14])
            .style(move |_theme, _status| theme::btn_danger_style(colors)),
            Space::new().width(Length::Fill),
            if force_modified {
                button(text("Keep and protect").size(13))
                    .on_press(Message::KeepModifiedMpqProtected)
                    .padding([6, 14])
                    .style(move |_theme, status| secondary_button_style(colors, status))
            } else {
                button(text("Cancel").size(13))
                    .on_press(Message::CloseDialog)
                    .padding([6, 14])
                    .style(move |_theme, status| secondary_button_style(colors, status))
            },
            if valid && !app.mpq_ui.busy {
                button(text("Save changes").size(13))
                    .on_press(Message::SaveMpqComponentDisplayName)
                    .padding([6, 14])
                    .style(move |_theme, _status| theme::tab_button_active_style(colors))
            } else {
                button(text("Save changes").size(13))
                    .padding([6, 14])
                    .style(move |_theme, status| secondary_button_style(colors, status))
            },
        ]
        .spacing(8),
    ]
    .spacing(12)
    .width(Length::Fill)
    .into()
}

fn view_edit_untracked_mpq<'a>(
    app: &'a App,
    dialog: &'a Dialog,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let Dialog::EditUntrackedMpq {
        path,
        display_name,
        edited_display_name,
        file_name,
        edited_file_name,
        edited_destination,
        edited_core,
        ..
    } = dialog
    else {
        return Space::new().into();
    };
    let classifications = vec![MpqClassification::Custom, MpqClassification::CoreClient];
    let destinations = edit_destination_options(app, edited_destination);
    let selected = if *edited_core {
        MpqClassification::CoreClient
    } else {
        MpqClassification::Custom
    };
    let edited_file = edited_file_name.trim();
    let valid = !edited_display_name.trim().is_empty()
        && !edited_file.is_empty()
        && edited_file.to_ascii_lowercase().ends_with(".mpq");
    let save = button(
        text(if app.mpq_ui.busy {
            "Saving…"
        } else {
            "Save changes"
        })
        .size(13),
    )
    .padding([6, 14])
    .style(move |_theme, _status| theme::tab_button_active_style(colors));
    let save: Element<Message> = if valid && !app.mpq_ui.busy {
        save.on_press(Message::SaveMpqEditor).into()
    } else {
        save.into()
    };

    column![
        heading(
            "Edit MPQ",
            "Change this patch's display details, on-disk filename, and classification.",
            colors,
        ),
        text(path).size(11).color(colors.muted),
        dialog_field_label("Friendly name", colors),
        text_input(display_name, edited_display_name)
            .on_input(Message::SetMpqEditorDisplayName),
        dialog_field_label("Filename on disk", colors),
        text_input(file_name, edited_file_name).on_input(Message::SetMpqEditorFileName),
        dialog_field_label("Location", colors),
        pick_list(
            destinations,
            Some(edited_destination.clone()),
            Message::SetMpqEditorDestination,
        ),
        dialog_field_label("Classification", colors),
        pick_list(classifications, Some(selected), |classification| {
            Message::SetMpqEditorCore(classification == MpqClassification::CoreClient)
        }),
        text(if *edited_core {
            "This file remains classified as a core client file. Locking is controlled separately from Manage MPQs."
        } else {
            "Custom MPQs remain unlocked until you lock them from Manage MPQs."
        })
        .size(14)
        .color(if *edited_core { colors.warn } else { colors.muted }),
        dialog_description(
            "Use a plain filename ending in .MPQ. Existing files and reserved core filenames cannot be overwritten.",
            colors,
        ),
        error_view(app.mpq_ui.error.as_deref(), colors),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(13))
                .on_press(Message::OpenMpqProtection)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            save,
        ]
        .spacing(8),
    ]
    .spacing(10)
    .width(Length::Fill)
    .into()
}

fn view_manual_component<'a>(
    app: &'a App,
    dialog: &'a Dialog,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let Dialog::ManualMpq {
        path,
        display_name,
        edited_display_name,
    } = dialog
    else {
        return Space::new().into();
    };
    let save = button(text("Save label").size(13))
        .padding([6, 14])
        .style(move |_theme, _status| theme::tab_button_active_style(colors));
    let save: Element<Message> = if !app.mpq_ui.busy && !edited_display_name.trim().is_empty() {
        save.on_press(Message::SaveManualMpqDisplayName).into()
    } else {
        save.into()
    };
    column![
        heading(
            "Name manual MPQ",
            "The friendly name is stored by Wuddle and does not rename the archive on disk.",
            colors,
        ),
        text(path).size(12).color(colors.muted),
        dialog_field_label("Friendly name", colors),
        text_input(display_name, edited_display_name).on_input(Message::SetManualMpqDisplayName),
        error_view(app.mpq_ui.error.as_deref(), colors),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(13))
                .on_press(Message::CloseDialog)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            save,
        ]
        .spacing(8),
    ]
    .spacing(12)
    .width(Length::Fill)
    .into()
}

fn view_rename_manual_mpq<'a>(
    app: &'a App,
    dialog: &'a Dialog,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let Dialog::RenameManualMpq {
        path,
        file_name,
        edited_file_name,
        ..
    } = dialog
    else {
        return Space::new().into();
    };
    let edited = edited_file_name.trim();
    let valid = !edited.is_empty() && edited.to_ascii_lowercase().ends_with(".mpq");
    let save = button(
        text(if app.mpq_ui.busy {
            "Renaming…"
        } else {
            "Rename file"
        })
        .size(13),
    )
    .padding([6, 14])
    .style(move |_theme, _status| theme::tab_button_active_style(colors));
    let save: Element<Message> = if valid && !app.mpq_ui.busy && edited != file_name {
        save.on_press(Message::SaveManualMpqFileName).into()
    } else {
        save.into()
    };

    column![
        heading(
            "Rename custom MPQ file",
            "Change the archive filename on disk without moving it from its current Data directory.",
            colors,
        ),
        text(path).size(12).color(colors.muted),
        dialog_field_label("Filename on disk", colors),
        text_input(file_name, edited_file_name).on_input(Message::SetManualMpqFileName),
        dialog_description(
            "Use a plain filename ending in .MPQ. Core-client names and existing files cannot be overwritten.",
            colors,
        ),
        error_view(app.mpq_ui.error.as_deref(), colors),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(13))
                .on_press(Message::CloseDialog)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            save,
        ]
        .spacing(8),
    ]
    .spacing(12)
    .width(Length::Fill)
    .into()
}

fn view_remove_wdm<'a>(
    app: &'a App,
    dialog: &'a Dialog,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let Dialog::RemoveWdm { remove_addon, .. } = dialog else {
        return Space::new().into();
    };
    column![
        heading(
            "Remove WDM",
            "Wuddle will remove the tracked dungeon-map MPQs and restore any displaced files.",
            colors,
        ),
        checkbox(*remove_addon)
            .label("Also remove the WDM companion addon")
            .on_toggle(Message::ToggleRemoveWdmAddon),
        dialog_description(
            "This addon is offered here only because the WDM wizard installed it. Independently installed companions are never linked or removed.",
            colors,
        ),
        error_view(app.mpq_ui.error.as_deref(), colors),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(13))
                .on_press(Message::CloseDialog)
                .padding([6, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            button(text("Remove").size(13))
                .on_press(Message::ConfirmRemoveWdm)
                .padding([6, 14])
                .style(move |_theme, _status| theme::btn_danger_style(colors)),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .width(Length::Fill)
    .into()
}

pub fn view_dialog<'a>(
    app: &'a App,
    dialog: &'a Dialog,
    colors: ThemeColors,
) -> Element<'a, Message> {
    match dialog {
        Dialog::MpqAdd => view_add(app, colors),
        Dialog::MpqInstall => view_install(app, colors),
        Dialog::ProtectedMpqs => view_protection(app, colors),
        Dialog::WdmInstall => view_wdm(app, colors),
        Dialog::MpqComponent { .. } => view_component(app, dialog, colors),
        Dialog::ManualMpq { .. } => view_manual_component(app, dialog, colors),
        Dialog::RenameManualMpq { .. } => view_rename_manual_mpq(app, dialog, colors),
        Dialog::EditUntrackedMpq { .. } => view_edit_untracked_mpq(app, dialog, colors),
        Dialog::RemoveWdm { .. } => view_remove_wdm(app, dialog, colors),
        _ => Space::new().into(),
    }
}

#[cfg(test)]
mod operation_tests {
    use super::UiState;

    #[test]
    fn precommit_dialog_close_invalidates_pending_work() {
        let mut state = UiState {
            active_operation_id: Some(7),
            pending_picker_id: Some(8),
            busy: true,
            ..UiState::default()
        };

        state.cancel_precommit_work();

        assert_eq!(state.active_operation_id, None);
        assert_eq!(state.pending_picker_id, None);
        assert!(!state.busy);
    }

    #[test]
    fn commit_work_cannot_be_cancelled_by_dialog_dismissal() {
        let mut state = UiState {
            active_operation_id: Some(9),
            commit_operation_id: Some(9),
            busy: true,
            ..UiState::default()
        };

        state.cancel_precommit_work();

        assert_eq!(state.active_operation_id, Some(9));
        assert_eq!(state.commit_operation_id, Some(9));
        assert!(state.busy);
        assert!(state.dismissal_blocked());
    }
}
