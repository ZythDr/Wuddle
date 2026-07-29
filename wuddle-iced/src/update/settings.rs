use crate::app::GitHubTokenStatus;
use crate::service;
use crate::settings::{self, resolve_ui_scale, ProfileConfig};
use crate::theme::WuddleTheme;
use crate::types::LogLevel;
use crate::{App, Dialog, InstanceField, Message, ToastKind};
use iced::Task;

fn validate_github_token_task(app: &mut App) -> Task<Message> {
    app.github_token_validation_generation = app.github_token_validation_generation.wrapping_add(1);
    let generation = app.github_token_validation_generation;
    Task::perform(service::validate_github_token(), move |result| {
        Message::ValidateGithubTokenResult { generation, result }
    })
}

fn schedule_tweak_client_detection(app: &mut App) -> Option<Task<Message>> {
    if app.wow_dir.trim().is_empty() {
        app.tweak_client_info = None;
        app.tweak_client_error = None;
        app.tweak_client_checking = false;
        return None;
    }

    let auto_launch_exe = app
        .active_profile()
        .and_then(|profile| profile.auto_launch_exe.clone());
    let profile_id = app.active_profile_id.clone();
    let wow_dir = app.wow_dir.clone();

    if let Some((cached_dir, cached_exe, cached_info)) =
        app.tweak_client_info_by_profile.get(&profile_id)
    {
        if cached_dir == &wow_dir && cached_exe == &auto_launch_exe {
            app.tweak_client_info = Some(cached_info.clone());
            app.tweak_client_error = None;
            app.tweak_client_checking = false;
            return None;
        }
    }

    app.tweak_client_info = None;
    app.tweak_client_error = None;
    app.tweak_client_checking = true;

    Some(Task::perform(
        service::detect_tweak_client(wow_dir.clone(), auto_launch_exe.clone()),
        move |result| Message::DetectTweakClientResult {
            profile_id: profile_id.clone(),
            wow_dir: wow_dir.clone(),
            auto_launch_exe: auto_launch_exe.clone(),
            result,
        },
    ))
}

pub fn update(app: &mut App, message: Message) -> Option<Task<Message>> {
    match message {
        Message::SetTheme(theme) => {
            app.wuddle_theme = theme;
            let mut colors = theme.colors();
            colors.body_font = app.body_font();
            app.theme_colors = colors;
            app.save_settings();
            app.log(
                LogLevel::Info,
                &format!("Theme switched to: {}.", theme.key()),
            );
            Some(Task::none())
        }
        Message::ToggleAutoCheck(b) => {
            app.opt_auto_check = b;
            app.save_settings();
            app.log(
                LogLevel::Info,
                &format!(
                    "Auto-check updates: {}.",
                    if b { "enabled" } else { "disabled" }
                ),
            );
            Some(Task::none())
        }
        Message::ToggleConserveGithubApi(b) => {
            app.opt_conserve_github_api = b;
            app.save_settings();
            app.log(
                LogLevel::Info,
                &format!(
                    "GitHub API conservation: {}.",
                    if b { "enabled" } else { "disabled" }
                ),
            );
            Some(Task::none())
        }
        Message::SetAutoCheckMinutes(s) => {
            if let Ok(n) = s.parse::<u32>() {
                app.auto_check_minutes = n.max(1);
            } else if s.is_empty() {
                app.auto_check_minutes = 1;
            }
            app.save_settings();
            app.log(
                LogLevel::Info,
                &format!("Auto-check interval set to {} min.", app.auto_check_minutes),
            );
            Some(Task::none())
        }
        Message::ToggleDesktopNotify(b) => {
            app.opt_desktop_notify = b;
            app.save_settings();
            app.log(
                LogLevel::Info,
                &format!(
                    "Desktop notifications: {}.",
                    if b { "enabled" } else { "disabled" }
                ),
            );
            Some(Task::none())
        }
        Message::ToggleSymlinks(b) => {
            app.opt_symlinks = b;
            app.save_settings();
            app.log(
                LogLevel::Info,
                &format!("Symlinks: {}.", if b { "enabled" } else { "disabled" }),
            );
            Some(Task::none())
        }
        Message::ToggleXattr(b) => {
            app.opt_xattr = b;
            app.save_settings();
            app.log(
                LogLevel::Info,
                &format!(
                    "Extended attributes: {}.",
                    if b { "enabled" } else { "disabled" }
                ),
            );
            Some(Task::none())
        }
        Message::ToggleClock12(b) => {
            app.opt_clock12 = b;
            app.save_settings();
            app.log(
                LogLevel::Info,
                &format!("12-hour clock: {}.", if b { "enabled" } else { "disabled" }),
            );
            Some(Task::none())
        }
        Message::ToggleFrizFont(b) => {
            app.opt_friz_font = b;
            app.theme_colors.body_font = app.body_font();
            app.save_settings();
            app.log(
                LogLevel::Info,
                "Friz Quadrata font setting saved. Restart Wuddle to apply.",
            );
            Some(Task::none())
        }
        Message::ToggleRememberWindowGeometry(b) => {
            app.remember_window_geometry = b;
            app.save_settings();
            app.log(
                LogLevel::Info,
                &format!(
                    "Remember window size and position: {}.",
                    if b { "enabled" } else { "disabled" }
                ),
            );
            Some(Task::none())
        }
        Message::SetUiScaleMode(mode) => {
            app.ui_scale_mode = mode;
            app.ui_scale = resolve_ui_scale(mode);
            app.save_settings();
            app.log(
                LogLevel::Info,
                &format!(
                    "UI scale set to {} ({}%)",
                    mode.label(),
                    (app.ui_scale * 100.0) as u32
                ),
            );
            Some(Task::none())
        }
        Message::SetGithubTokenInput(s) => {
            app.github_token_input = s;
            Some(Task::none())
        }
        Message::SaveGithubToken => {
            let token = app.github_token_input.trim().to_string();
            Some(Task::perform(
                async move { crate::service::save_github_token(token).await },
                Message::SaveGithubTokenResult,
            ))
        }
        Message::SaveGithubTokenResult(result) => {
            match result {
                Ok(_) => {
                    app.github_token_storage_error = None;
                    app.github_token_status = GitHubTokenStatus::StoredUnverified;
                    app.log(LogLevel::Info, "GitHub token saved successfully.");
                    app.show_toast(
                        "GitHub token saved. Verifying it with GitHub…",
                        ToastKind::Info,
                    );
                    app.github_token_input.clear();
                    return Some(validate_github_token_task(app));
                }
                Err(e) => {
                    app.github_token_storage_error = Some(e.clone());
                    app.log(LogLevel::Error, &format!("Token save error: {}", e));
                    app.show_toast(format!("Failed to save token: {}", e), ToastKind::Error);
                }
            }
            Some(Task::none())
        }
        Message::ValidateGithubTokenResult { generation, result } => {
            if generation != app.github_token_validation_generation {
                app.log(
                    LogLevel::Info,
                    "Discarded a superseded GitHub token validation result.",
                );
                return Some(Task::none());
            }
            match result {
                service::GitHubTokenValidation::Valid => {
                    app.github_token_status = GitHubTokenStatus::Validated;
                    app.log(LogLevel::Info, "GitHub token validated successfully.");
                }
                service::GitHubTokenValidation::Invalid => {
                    app.github_token_status = GitHubTokenStatus::Invalid;
                    wuddle_engine::set_github_token(None);
                    app.log(
                        LogLevel::Error,
                        "The stored GitHub token was rejected and has been deactivated.",
                    );
                    app.show_toast(
                        "GitHub rejected the saved token. Replace or remove it in Options.",
                        ToastKind::Error,
                    );
                }
                service::GitHubTokenValidation::Unverified(reason) => {
                    app.github_token_status = GitHubTokenStatus::OfflineUnverified;
                    app.log(LogLevel::Info, &reason);
                }
            }
            Some(Task::none())
        }
        Message::ForgetGithubToken => Some(Task::perform(
            async move { crate::service::clear_github_token().await },
            Message::ForgetGithubTokenResult,
        )),
        Message::ForgetGithubTokenResult(result) => {
            match result {
                Ok(source) => {
                    app.github_token_validation_generation =
                        app.github_token_validation_generation.wrapping_add(1);
                    app.github_token_status = match source {
                        service::GitHubTokenSource::None => GitHubTokenStatus::None,
                        service::GitHubTokenSource::Stored => GitHubTokenStatus::StoredUnverified,
                        service::GitHubTokenSource::Environment => {
                            GitHubTokenStatus::EnvironmentUnverified
                        }
                    };
                    app.github_token_storage_error = None;
                    app.log(LogLevel::Info, "GitHub token removed from secure storage.");
                    app.show_toast("GitHub token cleared.", ToastKind::Info);
                    if app.github_token_status.is_configured() {
                        return Some(validate_github_token_task(app));
                    }
                }
                Err(e) => {
                    app.github_token_storage_error = Some(e.clone());
                    app.log(LogLevel::Error, &format!("Clear token failed: {}", e));
                    app.show_toast(format!("Clear token failed: {}", e), ToastKind::Error);
                }
            }
            Some(Task::none())
        }

        // --- Instance settings ---
        Message::UpdateInstanceField(field) => {
            if let Some(Dialog::InstanceSettings {
                ref mut name,
                ref mut wow_dir,
                ref mut launch_method,
                ref mut show_mods_tab,
                ref mut show_addons_tab,
                ref mut show_patches_tab,
                ref mut show_tweaks_tab,
                ref mut clear_wdb,
                ref mut auto_login_enabled,
                ref mut lutris_target,
                ref mut wine_command,
                ref mut wine_args,
                ref mut custom_command,
                ref mut custom_args,
                ..
            }) = app.dialog
            {
                match field {
                    InstanceField::Name(v) => *name = v,
                    InstanceField::WowDir(v) => *wow_dir = v,
                    InstanceField::LaunchMethod(v) => *launch_method = v,
                    InstanceField::ShowModsTab(v) => *show_mods_tab = v,
                    InstanceField::ShowAddonsTab(v) => *show_addons_tab = v,
                    InstanceField::ShowPatchesTab(v) => *show_patches_tab = v,
                    InstanceField::ShowTweaksTab(v) => *show_tweaks_tab = v,
                    InstanceField::ClearWdb(v) => *clear_wdb = v,
                    InstanceField::AutoLoginEnabled(v) => *auto_login_enabled = v,
                    InstanceField::LutrisTarget(v) => *lutris_target = v,
                    InstanceField::WineCommand(v) => *wine_command = v,
                    InstanceField::WineArgs(v) => *wine_args = v,
                    InstanceField::CustomCommand(v) => *custom_command = v,
                    InstanceField::CustomArgs(v) => *custom_args = v,
                }
            }
            Some(Task::none())
        }
        Message::SaveInstanceSettings => {
            if let Some(Dialog::InstanceSettings {
                is_new,
                profile_id: dialog_profile_id,
                name,
                wow_dir,
                launch_method,
                show_mods_tab,
                show_addons_tab,
                show_patches_tab,
                show_tweaks_tab,
                clear_wdb,
                auto_login_enabled,
                lutris_target,
                wine_command,
                wine_args,
                custom_command,
                custom_args,
            }) = app.dialog.take()
            {
                if !is_new
                    && !app
                        .profiles
                        .iter()
                        .any(|profile| profile.id == dialog_profile_id)
                {
                    app.log(
                        LogLevel::Error,
                        "Ignored an attempt to save a profile that no longer exists.",
                    );
                    app.show_toast("That profile has already been removed.", ToastKind::Warn);
                    return Some(Task::none());
                }
                let was_new = is_new;
                let profile_name = if name.trim().is_empty() {
                    String::from("Default")
                } else {
                    name.trim().to_string()
                };
                let (dir, auto_launch_exe) = settings::normalize_wow_path_input(&wow_dir);
                let profile_id = if is_new {
                    settings::unique_profile_id(&profile_name, &app.profiles)
                } else if !dialog_profile_id.is_empty() {
                    dialog_profile_id
                } else {
                    app.profiles
                        .iter()
                        .find(|p| p.name == profile_name)
                        .map(|p| p.id.clone())
                        .unwrap_or_else(|| app.active_profile_id.clone())
                };

                let config = ProfileConfig {
                    id: profile_id.clone(),
                    name: profile_name.clone(),
                    wow_dir: dir.clone(),
                    auto_launch_exe,
                    launch_method,
                    show_mods_tab,
                    show_addons_tab,
                    show_patches_tab,
                    show_tweaks_tab,
                    clear_wdb,
                    auto_login_enabled,
                    lutris_target,
                    wine_command,
                    wine_args,
                    custom_command,
                    custom_args,
                    working_dir: String::new(),
                    env_text: String::new(),
                    last_infrequent_check_unix: app
                        .profiles
                        .iter()
                        .find(|profile| profile.id == profile_id)
                        .map(|profile| profile.last_infrequent_check_unix)
                        .unwrap_or_default(),
                    #[cfg(feature = "auto-login")]
                    auto_login_accounts: app
                        .profiles
                        .iter()
                        .find(|profile| profile.id == profile_id)
                        .map(|profile| profile.auto_login_accounts.clone())
                        .unwrap_or_default(),
                    #[cfg(not(feature = "auto-login"))]
                    auto_login_accounts: app
                        .profiles
                        .iter()
                        .find(|profile| profile.id == profile_id)
                        .map(|profile| profile.auto_login_accounts.clone())
                        .unwrap_or_default(),
                    #[cfg(feature = "auto-login")]
                    selected_auto_login_account_id: app
                        .profiles
                        .iter()
                        .find(|profile| profile.id == profile_id)
                        .and_then(|profile| profile.selected_auto_login_account_id.clone()),
                    #[cfg(not(feature = "auto-login"))]
                    selected_auto_login_account_id: app
                        .profiles
                        .iter()
                        .find(|profile| profile.id == profile_id)
                        .and_then(|profile| profile.selected_auto_login_account_id.clone()),
                    pending_auto_login_deletion_ids: app
                        .profiles
                        .iter()
                        .find(|profile| profile.id == profile_id)
                        .map(|profile| profile.pending_auto_login_deletion_ids.clone())
                        .unwrap_or_default(),
                };

                if let Some(existing) = app.profiles.iter_mut().find(|p| p.id == profile_id) {
                    *existing = config;
                } else {
                    app.profiles.push(config);
                }

                if app.active_profile_id == profile_id {
                    app.wow_dir = dir.clone();
                    app.advance_profile_generation();
                    if !app.profile_tab_enabled(app.active_tab) {
                        app.active_tab = crate::Tab::Home;
                        app.filter = crate::Filter::All;
                        app.project_search.clear();
                    }
                }
                crate::diagnostics::register_private_value(&profile_id, "<PROFILE_ID>");
                crate::diagnostics::register_private_value(&profile_name, "<PROFILE_NAME>");
                if !dir.trim().is_empty() {
                    crate::diagnostics::register_private_path(&dir, "<GAME_PATH>");
                }
                app.save_settings();
                app.log(LogLevel::Info, &format!("Profile saved: {}", profile_name));

                if was_new && !dir.trim().is_empty() {
                    if let Ok(db_path) = settings::profile_db_path(&profile_id) {
                        let init_profile_id = profile_id.clone();
                        return Some(Task::perform(
                            service::initialize_profile_database(db_path, dir.clone()),
                            move |result| {
                                Message::InitializeProfileDbResult(init_profile_id.clone(), result)
                            },
                        ));
                    }
                }

                if app.active_profile_id == profile_id {
                    let mut tasks = vec![crate::update::repos::refresh_repos_task(app)];
                    if let Some(task) = schedule_tweak_client_detection(app) {
                        tasks.push(task);
                    }
                    return Some(Task::batch(tasks));
                }
            }
            Some(Task::none())
        }
        Message::SwitchProfile(pid) => {
            if pid != app.active_profile_id && app.mpq_ui.dismissal_blocked() {
                app.show_toast(
                    "Finish the active MPQ operation before switching profiles.",
                    ToastKind::Info,
                );
                return Some(Task::none());
            }
            if pid != app.active_profile_id {
                if let Some(p) = app.profiles.iter().find(|p| p.id == pid).cloned() {
                    let pname = p.name.clone();
                    app.ignored_update_ids_by_profile.insert(
                        app.active_profile_id.clone(),
                        app.ignored_update_ids.clone(),
                    );
                    app.active_profile_id = pid.clone();
                    app.wow_dir = p.wow_dir.clone();
                    app.advance_profile_generation();
                    if !app.profile_tab_enabled(app.active_tab) {
                        app.active_tab = crate::Tab::Home;
                        app.filter = crate::Filter::All;
                        app.project_search.clear();
                    }
                    app.db_path = settings::resolve_profile_db_path(&pid).ok();
                    app.repos.clear();
                    if let Some((plans, last_checked)) = app.cached_plans.get(&pid).cloned() {
                        app.plans = plans;
                        app.last_checked = last_checked;
                    } else {
                        app.plans.clear();
                        app.last_checked = None;
                    }
                    app.ignored_update_ids = app
                        .ignored_update_ids_by_profile
                        .get(&pid)
                        .cloned()
                        .unwrap_or_default();
                    app.branches.clear();
                    app.repo_versions.clear();
                    app.repo_versions_loading.clear();
                    app.expanded_repo_ids.clear();
                    app.infrequent_repo_ids.clear();
                    app.updating_repo_ids.clear();
                    app.open_menu = None;
                    // Profile-specific dialogs can carry numeric repository IDs
                    // that are meaningful only in the database that opened them.
                    app.dialog = None;
                    app.reset_add_repo_state();
                    app.mpq_ui = crate::mpq::UiState::default();
                    app.last_infrequent_check_unix = p.last_infrequent_check_unix;
                    if app.active_tab == crate::Tab::Mods
                        && !app
                            .mods_warning_dismissed_profile_ids
                            .contains(&app.active_profile_id)
                    {
                        app.dialog = Some(Dialog::ModsWarning {
                            do_not_show_again: false,
                        });
                    }
                    if app.active_tab == crate::Tab::Patches
                        && !app
                            .patches_warning_dismissed_profile_ids
                            .contains(&app.active_profile_id)
                    {
                        app.dialog = Some(Dialog::PatchesWarning {
                            do_not_show_again: false,
                        });
                    }
                    if app.db_path.as_ref().is_some_and(|p| p.exists()) {
                        app.loading = true;
                    }
                    app.loading = true;
                    app.log(
                        LogLevel::Info,
                        &format!("Switched to profile: {} ({})", pname, pid),
                    );
                    app.save_settings();
                    let mut tasks = vec![crate::update::repos::refresh_repos_task(app)];
                    if let Some(task) = schedule_tweak_client_detection(app) {
                        tasks.push(task);
                    }
                    return Some(Task::batch(tasks));
                }
            }
            Some(Task::none())
        }
        Message::RemoveProfile(profile_id) => {
            if profile_id == app.active_profile_id {
                app.log(LogLevel::Error, "Cannot remove the active profile.");
                return Some(Task::none());
            }
            if !app.profiles.iter().any(|profile| profile.id == profile_id) {
                app.pending_profile_deletion_ids.remove(&profile_id);
                app.save_settings();
                return Some(Task::none());
            }
            if app.profile_deletions_in_progress.contains(&profile_id) {
                return Some(Task::none());
            }
            if app.pending_profile_deletion_ids.insert(profile_id.clone()) {
                if let Err(error) = app.try_save_settings() {
                    app.pending_profile_deletion_ids.remove(&profile_id);
                    app.log(
                        LogLevel::Error,
                        &format!("Could not stage profile removal: {error}"),
                    );
                    app.show_toast(
                        "Profile removal was stopped because Wuddle could not save its retry state.",
                        ToastKind::Error,
                    );
                    return Some(Task::none());
                }
            }
            let db_path = match settings::profile_db_path(&profile_id) {
                Ok(path) => path,
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Could not locate the profile database for removal: {error}"),
                    );
                    app.show_toast(
                        "Profile removal remains pending because Wuddle could not locate its database storage.",
                        ToastKind::Error,
                    );
                    return Some(Task::none());
                }
            };
            app.profile_deletions_in_progress.insert(profile_id.clone());
            if matches!(
                app.dialog.as_ref(),
                Some(Dialog::InstanceSettings {
                    profile_id: dialog_profile_id,
                    ..
                }) if dialog_profile_id == &profile_id
            ) {
                // Close the editor immediately. Keeping its populated fields alive
                // would allow a stale Save action to recreate the removed profile.
                app.dialog = None;
            }
            #[cfg(feature = "auto-login")]
            let auto_login_accounts = app
                .profiles
                .iter()
                .find(|profile| profile.id == profile_id)
                .map(|profile| profile.auto_login_accounts.clone())
                .unwrap_or_default();
            let pid_clone = profile_id.clone();
            Some(Task::perform(
                async move {
                    #[cfg(feature = "auto-login")]
                    if let Err(error) = crate::auto_login::delete_profile_accounts(
                        pid_clone.clone(),
                        auto_login_accounts,
                    )
                    .await
                    {
                        return (pid_clone, Err(format!(
                            "Profile removal was stopped because its auto-login credentials could not be removed: {error}"
                        )));
                    }
                    if let Err(error) = service::delete_profile_database_files(db_path).await {
                        return (
                            pid_clone,
                            Err(format!(
                                "Profile removal was stopped because its database could not be deleted safely: {error}"
                            )),
                        );
                    }
                    (pid_clone, Ok(()))
                },
                |res| Message::RemoveProfileResult(res.0, res.1),
            ))
        }
        Message::RemoveProfileResult(pid, result) => {
            app.profile_deletions_in_progress.remove(&pid);
            match result {
                Ok(()) => {}
                Err(error) => {
                    app.log(LogLevel::Error, &error);
                    app.show_toast(
                        format!("{error}\n\nThe profile remains pending and can be retried."),
                        ToastKind::Error,
                    );
                    return Some(Task::none());
                }
            }
            app.profiles.retain(|p| p.id != pid);
            app.pending_profile_deletion_ids.remove(&pid);
            app.mods_warning_dismissed_profile_ids.remove(&pid);
            app.patches_warning_dismissed_profile_ids.remove(&pid);
            app.ignored_update_ids_by_profile.remove(&pid);
            app.tweak_client_info_by_profile.remove(&pid);
            app.autocheck_done_profile_ids.remove(&pid);
            if let Err(error) = app.try_save_settings() {
                app.log(
                    LogLevel::Error,
                    &format!(
                        "Profile cleanup completed, but its settings tombstone could not be finalized: {error}"
                    ),
                );
                app.show_toast(
                    "Profile files were removed, but Wuddle must finish metadata cleanup after restart.",
                    ToastKind::Warn,
                );
                return Some(Task::none());
            }
            app.log(LogLevel::Info, &format!("Profile removed: {}", pid));
            app.show_toast(format!("Profile '{}' removed.", pid), ToastKind::Success);
            Some(Task::none())
        }
        Message::InitializeProfileDbResult(profile_id, result) => {
            match result {
                Ok(imported) => app.log(
                    LogLevel::Info,
                    &format!(
                        "Initialized profile database for {} ({} existing addon repo(s) imported).",
                        profile_id, imported
                    ),
                ),
                Err(err) => app.log(
                    LogLevel::Error,
                    &format!(
                        "Failed to initialize profile database for {}: {}",
                        profile_id, err
                    ),
                ),
            }
            Some(Task::none())
        }
        Message::SettingsLoaded(loaded) => {
            let settings_warning = loaded.warning;
            let s = loaded.settings;
            let theme = WuddleTheme::from_key(&s.theme);
            app.wuddle_theme = theme;
            app.opt_friz_font = s.opt_friz_font;
            app.remember_window_geometry = s.remember_window_geometry;
            let mut colors = theme.colors();
            colors.body_font = app.body_font();
            app.theme_colors = colors;
            app.active_profile_id = s.active_profile_id.clone();

            app.opt_auto_check = s.opt_auto_check;
            app.opt_conserve_github_api = s.opt_conserve_github_api;
            app.opt_desktop_notify = s.opt_desktop_notify;
            app.opt_symlinks = s.opt_symlinks;
            app.opt_xattr = s.opt_xattr;
            app.opt_clock12 = s.opt_clock12;
            app.migrated_from_tauri = s.migrated_from_tauri;
            app.auto_login_warning_acknowledged = s.auto_login_warning_acknowledged;
            app.window_geometry = s.window_geometry;

            app.log_wrap = s.log_wrap;
            app.log_autoscroll = s.log_autoscroll;
            app.verbose_diagnostics = s.verbose_diagnostics;
            crate::diagnostics::set_verbose(s.verbose_diagnostics);
            app.auto_check_minutes = s.auto_check_minutes.max(1);
            app.ignored_update_ids_by_profile = s
                .ignored_update_ids_by_profile
                .into_iter()
                .map(|(profile_id, ids)| (profile_id, ids.into_iter().collect()))
                .collect();
            app.pending_profile_deletion_ids = s.pending_profile_deletion_ids.into_iter().collect();
            if app.ignored_update_ids_by_profile.is_empty() && !s.ignored_update_ids.is_empty() {
                app.ignored_update_ids_by_profile.insert(
                    app.active_profile_id.clone(),
                    s.ignored_update_ids.into_iter().collect(),
                );
            }
            app.ignored_update_ids = app
                .ignored_update_ids_by_profile
                .get(&app.active_profile_id)
                .cloned()
                .unwrap_or_default();
            app.mods_warning_dismissed_profile_ids =
                s.mods_warning_dismissed_profile_ids.into_iter().collect();
            app.patches_warning_dismissed_profile_ids = s
                .patches_warning_dismissed_profile_ids
                .into_iter()
                .collect();
            app.update_channel = s.update_channel;
            app.ui_scale_mode = s.ui_scale_mode;
            app.ui_scale = resolve_ui_scale(s.ui_scale_mode);
            app.profiles = if s.profiles.is_empty() {
                vec![ProfileConfig::default()]
            } else {
                s.profiles
            };
            for profile in &app.profiles {
                crate::diagnostics::register_private_value(&profile.id, "<PROFILE_ID>");
                crate::diagnostics::register_private_value(&profile.name, "<PROFILE_NAME>");
                if !profile.wow_dir.trim().is_empty() {
                    crate::diagnostics::register_private_path(&profile.wow_dir, "<GAME_PATH>");
                }
                if !profile.working_dir.trim().is_empty() {
                    crate::diagnostics::register_private_path(
                        &profile.working_dir,
                        "<WORKING_DIR>",
                    );
                }
            }
            #[cfg(feature = "auto-login")]
            for profile in &mut app.profiles {
                let selection_exists =
                    profile
                        .selected_auto_login_account_id
                        .as_ref()
                        .is_none_or(|selected| {
                            profile
                                .auto_login_accounts
                                .iter()
                                .any(|account| &account.id == selected)
                        });
                if !selection_exists {
                    profile.selected_auto_login_account_id = None;
                }
            }
            if let Some(p) = app.profiles.iter().find(|p| p.id == app.active_profile_id) {
                app.wow_dir = p.wow_dir.clone();
                app.last_infrequent_check_unix = p.last_infrequent_check_unix;
            } else if let Some(first) = app.profiles.first() {
                app.active_profile_id = first.id.clone();
                app.wow_dir = first.wow_dir.clone();
                app.last_infrequent_check_unix = first.last_infrequent_check_unix;
            }
            app.db_path = settings::resolve_profile_db_path(&app.active_profile_id).ok();
            app.advance_profile_generation();
            app.log(LogLevel::Info, "Settings loaded.");
            if let Some(warning) = settings_warning {
                app.log(LogLevel::Error, &warning);
                app.show_toast(warning, ToastKind::Warn);
            }
            let mut tasks = vec![crate::update::repos::refresh_repos_task(app)];
            if app.github_token_status.is_configured() && wuddle_engine::github_token().is_some() {
                tasks.push(validate_github_token_task(app));
            }
            #[cfg(feature = "auto-login")]
            tasks.extend(crate::auto_login::pending_deletion_tasks(app));
            let pending_profile_deletions = app
                .pending_profile_deletion_ids
                .iter()
                .filter(|profile_id| {
                    profile_id.as_str() != app.active_profile_id
                        && app
                            .profiles
                            .iter()
                            .any(|profile| &profile.id == *profile_id)
                })
                .cloned()
                .collect::<Vec<_>>();
            for profile_id in pending_profile_deletions {
                tasks.push(Task::done(Message::RemoveProfile(profile_id)));
            }
            if let Some(task) = schedule_tweak_client_detection(app) {
                tasks.push(task);
            }
            Some(Task::batch(tasks))
        }
        Message::SaveSettings => {
            app.save_settings();
            Some(Task::none())
        }
        Message::PickWowDirectory => {
            let request_id = app.next_async_request_id();
            app.pending_wow_path_picker = Some(request_id);
            let scope = app.profile_operation_scope();
            let dialog_profile_id = match &app.dialog {
                Some(Dialog::InstanceSettings { profile_id, .. }) => Some(profile_id.clone()),
                _ => None,
            };
            Some(Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Select WoW Directory")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                move |path| Message::WowPathPicked {
                    request_id,
                    scope: scope.clone(),
                    dialog_profile_id: dialog_profile_id.clone(),
                    path,
                },
            ))
        }
        Message::PickWowExecutable => {
            let request_id = app.next_async_request_id();
            app.pending_wow_path_picker = Some(request_id);
            let scope = app.profile_operation_scope();
            let dialog_profile_id = match &app.dialog {
                Some(Dialog::InstanceSettings { profile_id, .. }) => Some(profile_id.clone()),
                _ => None,
            };
            Some(Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("Windows executable", &["exe"])
                        .set_title("Select Game Executable")
                        .pick_file()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                move |path| Message::WowPathPicked {
                    request_id,
                    scope: scope.clone(),
                    dialog_profile_id: dialog_profile_id.clone(),
                    path,
                },
            ))
        }
        Message::WowPathPicked {
            request_id,
            scope,
            dialog_profile_id,
            path,
        } => {
            if app.pending_wow_path_picker != Some(request_id) {
                app.log(
                    LogLevel::Info,
                    "Discarded a superseded WoW path picker result.",
                );
                return Some(Task::none());
            }
            app.pending_wow_path_picker = None;
            if !scope.matches(&app.active_profile_id, app.profile_generation) {
                app.log(
                    LogLevel::Info,
                    "Discarded a WoW path picker result after the profile context changed.",
                );
                return Some(Task::none());
            }
            let dialog_matches = match (&dialog_profile_id, &app.dialog) {
                (
                    Some(expected),
                    Some(Dialog::InstanceSettings {
                        profile_id: current,
                        ..
                    }),
                ) => expected == current,
                (None, Some(Dialog::InstanceSettings { .. })) => false,
                (None, _) => true,
                (Some(_), _) => false,
            };
            if !dialog_matches {
                app.log(
                    LogLevel::Info,
                    "Discarded a WoW path picker result after its dialog changed.",
                );
                return Some(Task::none());
            }
            if let Some(path) = path {
                let selected = path.to_string_lossy().to_string();
                let (dir, auto_launch_exe) = settings::normalize_wow_path_input(&selected);
                let display = settings::wow_path_display(&dir, auto_launch_exe.as_deref());
                crate::diagnostics::register_private_path(&dir, "<GAME_PATH>");
                app.log(LogLevel::Info, &format!("WoW path set: {}", display));
                if let Some(Dialog::InstanceSettings {
                    ref mut wow_dir, ..
                }) = app.dialog
                {
                    *wow_dir = display;
                } else {
                    app.wow_dir = dir.clone();
                    app.advance_profile_generation();
                    if let Some(profile) = app
                        .profiles
                        .iter_mut()
                        .find(|p| p.id == app.active_profile_id)
                    {
                        profile.wow_dir = dir;
                        profile.auto_launch_exe = auto_launch_exe;
                    }
                    app.save_settings();
                    let mut tasks = vec![crate::update::repos::refresh_repos_task(app)];
                    if let Some(task) = schedule_tweak_client_detection(app) {
                        tasks.push(task);
                    }
                    return Some(Task::batch(tasks));
                }
            }
            Some(Task::none())
        }
        Message::AutoCheckTick => {
            if app.opt_auto_check && !app.checking_updates {
                app.checking_updates = true;
                app.update_check_trigger = Some(crate::app::UpdateCheckTrigger::Scheduled);
                return Some(crate::update::repos::check_updates_task(app));
            }
            Some(Task::none())
        }
        _ => None,
    }
}
