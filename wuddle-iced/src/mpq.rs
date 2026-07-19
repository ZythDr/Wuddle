//! Thin Iced adapter for generic MPQ management and the curated WDM recipe.
//! Validation, staging, protection, backups, and deployment stay in the engine.

use std::path::PathBuf;

use iced::widget::{
    button, checkbox, column, container, pick_list, row, rule, scrollable, text, text_input, Space,
};
use iced::{Element, Length, Task};

use crate::app::App;
use crate::components::helpers::{badge_tag, tip};
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
    manage_order: Vec<String>,
    manage_core_keys: Vec<String>,
    manage_managed_order: Vec<(i64, String)>,
    manage_snapshot_initialized: bool,
    pub editing_classifications: bool,
    pub catalog: Option<service::WdmCatalog>,
    pub wdm_locale: Option<String>,
    pub wdm_caverns: bool,
    pub wdm_addon: bool,
    pub busy: bool,
    pub error: Option<String>,
}

fn pick_source_task() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter("MPQ packages", &["mpq", "zip", "7z"])
                .set_title("Select an MPQ file or package")
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        Message::MpqSourcePicked,
    )
}

fn inspect_task(app: &App, source: PathBuf) -> Task<Message> {
    Task::perform(
        service::inspect_local_mpq(app.db_path.clone(), app.wow_dir.clone(), source),
        Message::MpqInspectionFinished,
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
            if has_wdm_update {
                Some(Task::done(Message::OpenWdm))
            } else {
                app.show_toast("No curated patch updates are available.", ToastKind::Info);
                Some(Task::none())
            }
        }
        Message::OpenMpqInstall => {
            app.mpq_ui = UiState::default();
            app.dialog = Some(Dialog::MpqInstall);
            Some(Task::none())
        }
        Message::PickMpqSource => Some(pick_source_task()),
        Message::MpqSourcePicked(source) => {
            let Some(source) = source else {
                return Some(Task::none());
            };
            app.mpq_ui.source = Some(source.clone());
            app.mpq_ui.inspection = None;
            app.mpq_ui.selections.clear();
            app.mpq_ui.target_previews.clear();
            app.mpq_ui.targets_reviewed = false;
            app.mpq_ui.error = None;
            app.mpq_ui.busy = true;
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
                app.mpq_ui.error = Some("Drop a local .mpq, .zip, or .7z file.".to_string());
                return Some(Task::none());
            }
            app.mpq_ui.source = Some(path.clone());
            app.mpq_ui.inspection = None;
            app.mpq_ui.selections.clear();
            app.mpq_ui.target_previews.clear();
            app.mpq_ui.targets_reviewed = false;
            app.mpq_ui.error = None;
            app.mpq_ui.busy = true;
            Some(inspect_task(app, path))
        }
        Message::MpqInspectionFinished(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(inspection) => {
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
                Err(error) => app.mpq_ui.error = Some(error),
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
                app.mpq_ui.error = Some("Choose an MPQ source first.".to_string());
                return Some(Task::none());
            };
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            if !app.mpq_ui.targets_reviewed {
                return Some(Task::perform(
                    service::preview_local_mpq_targets(
                        app.db_path.clone(),
                        app.wow_dir.clone(),
                        source,
                        app.mpq_ui.selections.clone(),
                    ),
                    Message::MpqTargetsReviewed,
                ));
            }
            Some(Task::perform(
                service::install_local_mpq(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    source,
                    app.mpq_ui.selections.clone(),
                    app.opt_xattr,
                ),
                Message::MpqInstallFinished,
            ))
        }
        Message::MpqTargetsReviewed(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(previews) => {
                    app.mpq_ui.target_previews = previews;
                    app.mpq_ui.targets_reviewed = true;
                    app.mpq_ui.error = None;
                }
                Err(error) => app.mpq_ui.error = Some(error),
            }
            Some(Task::none())
        }
        Message::MpqInstallFinished(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(_) => {
                    app.dialog = None;
                    app.log(LogLevel::Info, "MPQ package installed.");
                    app.show_toast("MPQ package installed.", ToastKind::Success);
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.mpq_ui.error = Some(error);
                    Some(Task::none())
                }
            }
        }
        Message::ToggleMpqPackageEnabled(repo_id, enabled) => {
            if app.mpq_ui.busy {
                return Some(Task::none());
            }
            app.mpq_ui.busy = true;
            Some(Task::perform(
                service::set_mpq_enabled(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    repo_id,
                    None,
                    enabled,
                ),
                Message::MpqEnabledChanged,
            ))
        }
        Message::ToggleMpqEnabled(repo_id, path, enabled) => {
            if app.mpq_ui.busy {
                return Some(Task::none());
            }
            app.mpq_ui.busy = true;
            Some(Task::perform(
                service::set_mpq_enabled(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    repo_id,
                    Some(path),
                    enabled,
                ),
                Message::MpqEnabledChanged,
            ))
        }
        Message::MpqEnabledChanged(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(enabled) => {
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
                    app.log(LogLevel::Error, &format!("MPQ toggle failed: {error}"));
                    app.show_toast(error, ToastKind::Error);
                    Some(Task::none())
                }
            }
        }
        Message::OpenMpqProtection => {
            app.open_menu = None;
            app.dialog = Some(Dialog::ProtectedMpqs);
            app.mpq_ui.editing_classifications = false;
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
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
            Some(Task::perform(
                service::load_mpq_protection(app.db_path.clone(), app.wow_dir.clone()),
                Message::MpqProtectionLoaded,
            ))
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
        Message::SetMpqProtected(path, protected) => {
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::change_mpq_protection(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    path,
                    protected,
                ),
                Message::MpqProtectionChanged,
            ))
        }
        Message::ToggleMpqClassificationEditing(editing) => {
            app.mpq_ui.editing_classifications = editing;
            Some(Task::none())
        }
        Message::SetMpqCoreClassification(path, core) => {
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::change_mpq_classification(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    path,
                    core,
                ),
                Message::MpqProtectionChanged,
            ))
        }
        Message::ToggleUntrackedMpqEnabled(path, enabled) => {
            if app.mpq_ui.busy {
                return Some(Task::none());
            }
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::set_untracked_mpq_enabled(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    path,
                    enabled,
                ),
                Message::MpqProtectionChanged,
            ))
        }
        Message::MpqProtectionChanged(result) => {
            app.mpq_ui.busy = false;
            if let Err(error) = result {
                app.mpq_ui.error = Some(error);
                return Some(Task::none());
            }
            Some(Task::perform(
                service::load_mpq_protection(app.db_path.clone(), app.wow_dir.clone()),
                Message::MpqProtectionLoaded,
            ))
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
            let Some(Dialog::ManualMpq {
                path,
                edited_display_name,
                ..
            }) = app.dialog.as_ref()
            else {
                return Some(Task::none());
            };
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::rename_untracked_mpq(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    path.clone(),
                    edited_display_name.clone(),
                    app.opt_xattr,
                ),
                Message::ManualMpqDisplayNameSaved,
            ))
        }
        Message::ManualMpqDisplayNameSaved(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(()) => {
                    app.dialog = None;
                    app.show_toast("MPQ friendly name saved.", ToastKind::Success);
                    Some(Task::perform(
                        service::load_mpq_protection(app.db_path.clone(), app.wow_dir.clone()),
                        Message::MpqProtectionLoaded,
                    ))
                }
                Err(error) => {
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
            let Some(Dialog::RenameManualMpq {
                path,
                edited_file_name,
                ..
            }) = app.dialog.as_ref()
            else {
                return Some(Task::none());
            };
            let old_path = path.clone();
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::rename_untracked_mpq_file(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    path.clone(),
                    edited_file_name.clone(),
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
                    app.show_toast("MPQ file renamed.", ToastKind::Success);
                    app.dialog = None;
                    if return_to_manage {
                        Some(Task::done(Message::OpenMpqProtection))
                    } else {
                        Some(crate::update::repos::refresh_repos_task(app))
                    }
                }
                Err(error) => {
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
        Message::SaveMpqComponentDisplayName => {
            let Some(Dialog::MpqComponent {
                repo_id,
                path,
                edited_display_name,
                ..
            }) = app.dialog.as_ref()
            else {
                return Some(Task::none());
            };
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::rename_mpq_component(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    *repo_id,
                    path.clone(),
                    edited_display_name.clone(),
                    app.opt_xattr,
                ),
                Message::MpqComponentDisplayNameSaved,
            ))
        }
        Message::MpqComponentDisplayNameSaved(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(()) => {
                    app.dialog = None;
                    app.show_toast("MPQ label updated.", ToastKind::Success);
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.mpq_ui.error = Some(error);
                    Some(Task::none())
                }
            }
        }
        Message::RemoveMpqComponent(force_modified) => {
            let Some(Dialog::MpqComponent { repo_id, path, .. }) = app.dialog.as_ref() else {
                return Some(Task::none());
            };
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::remove_mpq_component(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    *repo_id,
                    path.clone(),
                    force_modified,
                ),
                Message::MpqComponentRemoved,
            ))
        }
        Message::MpqComponentRemoved(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(()) => {
                    app.dialog = None;
                    app.show_toast("MPQ removed.", ToastKind::Info);
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.mpq_ui.error = Some(error);
                    Some(Task::none())
                }
            }
        }
        Message::KeepModifiedMpqProtected => {
            let Some(Dialog::MpqComponent { repo_id, path, .. }) = app.dialog.as_ref() else {
                return Some(Task::none());
            };
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::protect_modified_mpq(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    *repo_id,
                    path.clone(),
                ),
                Message::ModifiedMpqProtected,
            ))
        }
        Message::ModifiedMpqProtected(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(()) => {
                    app.dialog = None;
                    app.show_toast("Modified MPQ kept and protected.", ToastKind::Info);
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
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
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            app.dialog = Some(Dialog::WdmInstall);
            Some(Task::perform(
                service::resolve_wdm(app.db_path.clone(), app.wow_dir.clone()),
                Message::WdmResolved,
            ))
        }
        Message::WdmResolved(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(catalog) => {
                    app.mpq_ui.wdm_locale = catalog.locale.recommended.clone();
                    app.mpq_ui.catalog = Some(catalog);
                    app.mpq_ui.error = None;
                }
                Err(error) => app.mpq_ui.error = Some(error),
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
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            let options = wuddle_engine::InstallOptions {
                use_symlinks: app.opt_symlinks,
                set_xattr_comment: app.opt_xattr,
                replace_addon_conflicts: false,
                cache_keep_versions: 0,
            };
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
                Message::WdmInstallFinished,
            ))
        }
        Message::WdmInstallFinished(result) => {
            app.mpq_ui.busy = false;
            match result {
                Ok(_) => {
                    app.dialog = None;
                    app.log(LogLevel::Info, "WDM installed successfully.");
                    app.show_toast("WDM installed successfully.", ToastKind::Success);
                    Some(crate::update::repos::refresh_repos_task(app))
                }
                Err(error) => {
                    app.mpq_ui.error = Some(error);
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
            let Some(Dialog::RemoveWdm {
                repo_id,
                addon_repo_id,
                remove_addon,
            }) = app.dialog.as_ref()
            else {
                return Some(Task::none());
            };
            app.mpq_ui.busy = true;
            app.mpq_ui.error = None;
            Some(Task::perform(
                service::remove_wdm(
                    app.db_path.clone(),
                    app.wow_dir.clone(),
                    *repo_id,
                    *addon_repo_id,
                    *remove_addon,
                ),
                Message::WdmRemoved,
            ))
        }
        Message::WdmRemoved(result) => {
            app.mpq_ui.busy = false;
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
            app.markdown_image_cache.clear();
            app.markdown_gif_cache.clear();
            app.dialog = Some(Dialog::Changelog {
                title: "WDM — README".to_string(),
                items: Vec::new(),
                loading: true,
            });
            Some(Task::perform(
                service::fetch_repo_preview("https://github.com/Trimitor/WDM-patch".to_string()),
                Message::WdmReadmeLoaded,
            ))
        }
        Message::WdmReadmeLoaded(result) => {
            let loaded_items = match result {
                Ok(preview) => {
                    app.markdown_image_cache = preview.image_cache;
                    app.markdown_gif_cache = preview.gif_cache;
                    preview.readme_items
                }
                Err(error) => iced::widget::markdown::Content::parse(&format!(
                    "Could not load the WDM README.\n\n{error}"
                ))
                .items()
                .to_vec(),
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
            text(title).size(22).color(colors.title),
            Space::new().width(Length::Fill),
            button(text("\u{2715}").size(14).color(colors.bad))
                .on_press(Message::CloseDialog)
                .padding([2, 6])
                .style(move |_theme, _status| button::Style {
                    background: None,
                    text_color: colors.bad,
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: true,
                }),
        ]
        .align_y(iced::Alignment::Center),
        text(subtitle).size(13).color(colors.muted),
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
        button::Status::Pressed => theme::tab_button_active_style(colors),
        _ => theme::tab_button_style(colors),
    }
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
            badge_tag(
                "WoW 3.3.5a",
                iced::Color::from_rgb8(0xfd, 0xe6, 0x8a),
                iced::Color::from_rgb8(0xfa, 0xcc, 0x15),
            ),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);
        container(
            column![
                row![title, tags]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                text("Adds dungeon maps to the 3.3.5 client, with an optional Caverns & Mines patch and companion addon.")
                    .size(16)
                    .color(colors.title),
                row![
                    Space::new().width(Length::Fill),
                    button(text("Configure").size(12))
                        .on_press(Message::OpenWdm)
                        .padding([4, 14])
                        .style(move |_theme, _status| theme::tab_button_active_style(colors)),
                ],
            ]
            .spacing(6),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(colors))
        .into()
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

    let close = button(text("\u{2715}").size(14).color(colors.bad))
        .on_press(Message::CloseDialog)
        .padding([2, 6])
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: colors.bad,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        });

    column![
        row![
            text("Add an MPQ patch").size(17).color(colors.title),
            Space::new().width(Length::Fill),
            close,
        ]
        .align_y(iced::Alignment::Center),
        text("Quick-add a curated patch or install an MPQ package from your computer.")
            .size(12)
            .color(colors.text_soft),
        rule::horizontal(1).style(move |_theme| theme::update_line_style(colors)),
        text("MPQ URL").size(12).color(colors.text),
        text_input(
            "(e.g. https://example.com/patch-name.mpq)",
            &app.mpq_ui.direct_url,
        )
        .on_input(Message::SetMpqDirectUrl)
        .padding([8, 12]),
        text("Direct downloads are not enabled yet; download hosted files in your browser and use Local Installation.")
            .size(11)
            .color(colors.muted),
        text(quick_add_label)
            .size(12)
            .color(colors.muted),
        quick_add,
        rule::horizontal(1).style(move |_theme| theme::update_line_style(colors)),
        row![
            button(text("Local Installation..."))
                .on_press(Message::OpenMpqInstall)
                .padding([7, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            button(text("Manage MPQs..."))
                .on_press(Message::OpenMpqProtection)
                .padding([7, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            Space::new().width(Length::Fill),
            button(text("Close"))
                .on_press(Message::CloseDialog)
                .padding([7, 14])
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
                        text("Friendly name").size(12).color(colors.muted),
                        text_input("Required label", &selection.display_name)
                            .on_input(move |value| Message::SetMpqDisplayName(index, value))
                    ]
                    .spacing(3)
                    .width(Length::Fill),
                    column![
                        text("On-disk filename").size(12).color(colors.muted),
                        text_input("patch-name.MPQ", &selection.file_name)
                            .on_input(move |value| Message::SetMpqFileName(index, value))
                    ]
                    .spacing(3)
                    .width(Length::Fill),
                ]
                .spacing(10),
                row![
                    text("Destination").size(12).color(colors.muted),
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
    let install = button(text(if app.mpq_ui.busy {
        "Working..."
    } else if app.mpq_ui.targets_reviewed {
        "Confirm install"
    } else {
        "Review installation"
    }))
    .padding([7, 16])
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
    let mut content = column![
        heading(
            "Install local MPQ",
            "MPQs are inspected in staging. Nothing reaches Data/ until you confirm every file.",
            colors,
        ),
        row![
            text(source_label).size(13).color(colors.text),
            Space::new().width(Length::Fill),
            button(text("Choose MPQ / ZIP / 7z...").size(12))
                .on_press(Message::PickMpqSource)
                .padding([6, 10])
                .style(move |_theme, status| secondary_button_style(colors, status)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        text("You can also drop a supported file onto this dialog.")
            .size(12)
            .color(colors.muted),
        error_view(app.mpq_ui.error.as_deref(), colors),
        inspected_files,
        row![
            button(text("Manage MPQs...").size(12))
                .on_press(Message::OpenMpqProtection)
                .padding([7, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            Space::new().width(Length::Fill),
            button(text("Back"))
                .on_press(Message::OpenMpqAdd)
                .padding([7, 14])
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

fn manage_section<'a>(
    title: &'a str,
    subtitle: &'a str,
    entries: Vec<Element<'a, Message>>,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let body: Element<Message> = if entries.is_empty() {
        container(text("None detected.").size(12).color(colors.muted))
            .padding([6, 10])
            .into()
    } else {
        column(entries).spacing(6).into()
    };
    column![
        text(title).size(15).color(colors.title),
        text(subtitle).size(11).color(colors.muted),
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
    let classification_path = entry.path.clone();
    let protection_path = entry.path.clone();
    let enabled_path = entry.path.clone();
    let selected = if entry.core {
        MpqClassification::CoreClient
    } else {
        MpqClassification::Custom
    };
    let classification: Element<Message> = if app.mpq_ui.editing_classifications && !app.mpq_ui.busy
    {
        pick_list(
            [MpqClassification::CoreClient, MpqClassification::Custom],
            Some(selected),
            move |value| {
                Message::SetMpqCoreClassification(
                    classification_path.clone(),
                    value == MpqClassification::CoreClient,
                )
            },
        )
        .width(150)
        .text_size(12)
        .into()
    } else {
        container(text(selected.to_string()).size(12).color(colors.muted))
            .width(150)
            .padding([7, 10])
            .style(move |_theme| theme::card_style(colors))
            .into()
    };

    let lock_color = if entry.protected {
        colors.warn
    } else {
        colors.good
    };
    let lock_button = button(protection_icon(entry.protected, lock_color))
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
    let lock_button: Element<Message> =
        if app.mpq_ui.editing_classifications && !app.mpq_ui.busy && !entry.core {
            lock_button
                .on_press(Message::SetMpqProtected(protection_path, !entry.protected))
                .into()
        } else {
            lock_button.into()
        };
    let lock_tip = if entry.core {
        "Core client files stay locked until reclassified as Custom MPQ."
    } else if entry.protected {
        "Protected: Wuddle cannot replace, remove, disable, or rename this MPQ."
    } else {
        "Unlocked: Wuddle may modify this MPQ when explicitly requested."
    };

    let enabled = checkbox(entry.enabled).size(30);
    let enabled: Element<Message> = if app.mpq_ui.editing_classifications
        && !app.mpq_ui.busy
        && !entry.core
        && !entry.protected
    {
        enabled
            .on_toggle(move |value| Message::ToggleUntrackedMpqEnabled(enabled_path.clone(), value))
            .into()
    } else {
        enabled.into()
    };
    let enabled_tip = if entry.core || entry.protected {
        "Reclassify this as Custom MPQ and unlock it before changing its enabled state."
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
    let name_dialog = Dialog::ManualMpq {
        path: entry.path.clone(),
        display_name: visible_name.clone(),
        edited_display_name: visible_name.clone(),
    };
    let name_button = button(text(visible_name).size(14).color(colors.title))
        .on_press(Message::OpenDialog(name_dialog))
        .padding(0)
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: colors.title,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        });

    let file_name = editable_mpq_file_name(&entry.file_name);
    let rename_dialog = Dialog::RenameManualMpq {
        path: entry.path.clone(),
        file_name: file_name.clone(),
        edited_file_name: file_name,
        return_to_manage: true,
    };
    let rename_button = button(text("Rename file…").size(11))
        .padding([5, 9])
        .style(move |_theme, status| secondary_button_style(colors, status));
    let rename_button: Element<Message> = if app.mpq_ui.editing_classifications
        && !app.mpq_ui.busy
        && !entry.core
        && !entry.protected
    {
        rename_button
            .on_press(Message::OpenDialog(rename_dialog))
            .into()
    } else {
        rename_button.into()
    };

    container(
        row![
            column![
                tip(
                    name_button,
                    "Set a friendly name for this MPQ.",
                    iced::widget::tooltip::Position::Top,
                    colors
                ),
                text(&entry.path).size(11).color(colors.muted),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
            classification,
            tip(
                lock_button,
                lock_tip,
                iced::widget::tooltip::Position::Top,
                colors
            ),
            tip(
                rename_button,
                "Unlock this custom MPQ and enable editing before renaming its file.",
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
    let dialog = Dialog::MpqComponent {
        repo_id: repo.id,
        path: entry.path.clone(),
        display_name: entry.display_name.clone(),
        edited_display_name: entry.display_name.clone(),
        status: entry.status,
    };
    let name = button(text(&entry.display_name).size(14).color(colors.title))
        .on_press(Message::OpenDialog(dialog))
        .padding(0)
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: colors.title,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        });
    let enabled = checkbox(entry.enabled).size(30);
    let enabled: Element<Message> = if app.mpq_ui.busy {
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
                name,
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
            badge_tag(
                "Wuddle managed",
                colors.link,
                iced::Color {
                    a: 0.35,
                    ..colors.link
                },
            ),
            tip(
                enabled,
                if entry.enabled {
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
            "Custom and manual MPQs",
            "Detected archives that are not owned by a Wuddle package.",
            custom_entries,
            colors,
        ),
        manage_section(
            "Wuddle-installed MPQs",
            "Tracked local and curated packages, including WDM.",
            managed_entries,
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
        checkbox(app.mpq_ui.editing_classifications)
            .label("Edit MPQ classifications, protection, and enabled states")
            .on_toggle(Message::ToggleMpqClassificationEditing),
        if app.mpq_ui.editing_classifications {
            text("Caution: reclassifying or unprotecting a stock client archive allows Wuddle to replace it. Changed files return to the safe detected default.")
                .size(12)
                .color(colors.warn)
        } else {
            text("Enable editing to change a detected classification, protection, or enabled state.")
                .size(12)
                .color(colors.muted)
        },
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
            button(text("+ Add MPQ..."))
                .on_press(Message::OpenMpqAdd)
                .padding([7, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            Space::new().width(Length::Fill),
            button(text("Close"))
                .on_press(Message::CloseDialog)
                .padding([7, 14])
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
    let install = button(text(if app.mpq_ui.busy {
        "Working..."
    } else {
        "Install WDM"
    }))
    .padding([7, 16])
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
        text("Wuddle checks curated WDM releases alongside mods and addons; updates are installed deliberately through this dialog.")
            .size(13)
            .color(colors.muted),
        row![
            text("Client locale").size(13).color(colors.text),
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
        .size(12)
        .color(colors.muted),
        error_view(app.mpq_ui.error.as_deref(), colors),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel"))
                .on_press(Message::CloseDialog)
                .padding([7, 14])
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
        status,
        ..
    } = dialog
    else {
        return Space::new().into();
    };
    let force_modified = *status == wuddle_engine::mpq::MpqFileStatus::Modified;
    column![
        heading(
            "Manage MPQ",
            "The friendly name is Wuddle metadata; it does not rename the deployed archive.",
            colors,
        ),
        text(path).size(12).color(colors.muted),
        text_input(display_name, edited_display_name)
            .on_input(Message::SetMpqComponentDisplayName),
        text(format!("Status: {}", status.label()))
            .size(12)
            .color(if force_modified { colors.warn } else { colors.text }),
        if force_modified {
            text("This file changed outside Wuddle. Removing it requires explicit confirmation; otherwise Wuddle keeps and protects it.")
                .size(12)
                .color(colors.warn)
        } else {
            text("Removing this component also restores any untracked file it replaced from Wuddle's MPQ backup.")
                .size(12)
                .color(colors.muted)
        },
        error_view(app.mpq_ui.error.as_deref(), colors),
        row![
            button(text(if force_modified {
                "Delete modified MPQ"
            } else {
                "Remove MPQ"
            }))
            .on_press(Message::RemoveMpqComponent(force_modified))
            .padding([7, 14])
            .style(move |_theme, _status| theme::btn_danger_style(colors)),
            Space::new().width(Length::Fill),
            if force_modified {
                button(text("Keep and protect"))
                    .on_press(Message::KeepModifiedMpqProtected)
                    .padding([7, 14])
                    .style(move |_theme, status| secondary_button_style(colors, status))
            } else {
                button(text("Cancel"))
                    .on_press(Message::CloseDialog)
                    .padding([7, 14])
                    .style(move |_theme, status| secondary_button_style(colors, status))
            },
            button(text("Save label"))
                .on_press(Message::SaveMpqComponentDisplayName)
                .padding([7, 14])
                .style(move |_theme, _status| theme::tab_button_active_style(colors)),
        ]
        .spacing(8),
    ]
    .spacing(12)
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
    let save = button(text("Save label"))
        .padding([7, 14])
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
        text_input(display_name, edited_display_name).on_input(Message::SetManualMpqDisplayName),
        error_view(app.mpq_ui.error.as_deref(), colors),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel"))
                .on_press(Message::CloseDialog)
                .padding([7, 14])
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
    let save = button(text(if app.mpq_ui.busy {
        "Renaming…"
    } else {
        "Rename file"
    }))
    .padding([7, 14])
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
        text_input(file_name, edited_file_name).on_input(Message::SetManualMpqFileName),
        text("Use a plain filename ending in .MPQ. Core-client names and existing files cannot be overwritten.")
            .size(12)
            .color(colors.muted),
        error_view(app.mpq_ui.error.as_deref(), colors),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel"))
                .on_press(Message::CloseDialog)
                .padding([7, 14])
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
        text("This addon is offered here only because the WDM wizard installed it. Independently installed companions are never linked or removed.")
            .size(12)
            .color(colors.muted),
        error_view(app.mpq_ui.error.as_deref(), colors),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel"))
                .on_press(Message::CloseDialog)
                .padding([7, 14])
                .style(move |_theme, status| secondary_button_style(colors, status)),
            button(text("Remove"))
                .on_press(Message::ConfirmRemoveWdm)
                .padding([7, 14])
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
        Dialog::RemoveWdm { .. } => view_remove_wdm(app, dialog, colors),
        _ => Space::new().into(),
    }
}
