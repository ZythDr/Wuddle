use crate::service;
use crate::settings::{self, resolve_ui_scale, ProfileConfig};
use crate::theme::WuddleTheme;
use crate::types::LogLevel;
use crate::{App, Dialog, InstanceField, Message, ToastKind};
use iced::Task;

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

    app.tweak_client_info = None;
    app.tweak_client_error = None;
    app.tweak_client_checking = true;

    Some(Task::perform(
        service::detect_tweak_client(app.wow_dir.clone(), auto_launch_exe),
        Message::DetectTweakClientResult,
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
                    app.log(LogLevel::Info, "GitHub token saved successfully.");
                    app.show_toast("GitHub token saved.", ToastKind::Info);
                    app.github_token_input.clear();
                }
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Token save error: {}", e));
                    app.show_toast(format!("Failed to save token: {}", e), ToastKind::Error);
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
                Ok(_) => {
                    app.log(LogLevel::Info, "GitHub token removed from database.");
                    app.show_toast("GitHub token cleared.", ToastKind::Info);
                }
                Err(e) => {
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
                ref mut clear_wdb,
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
                    InstanceField::ClearWdb(v) => *clear_wdb = v,
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
                clear_wdb,
                lutris_target,
                wine_command,
                wine_args,
                custom_command,
                custom_args,
            }) = app.dialog.take()
            {
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
                    clear_wdb,
                    lutris_target,
                    wine_command,
                    wine_args,
                    custom_command,
                    custom_args,
                    working_dir: String::new(),
                    env_text: String::new(),
                };

                if let Some(existing) = app.profiles.iter_mut().find(|p| p.id == profile_id) {
                    *existing = config;
                } else {
                    app.profiles.push(config);
                }

                if app.active_profile_id == profile_id {
                    app.wow_dir = dir.clone();
                }
                app.save_settings();
                app.log(
                    LogLevel::Info,
                    &format!("Instance profile saved: {}", profile_name),
                );

                if was_new && !dir.trim().is_empty() {
                    if let Ok(db_path) = settings::profile_db_path(&profile_id) {
                        let init_profile_id = profile_id.clone();
                        return Some(Task::perform(
                            service::initialize_profile_database(db_path, dir.clone()),
                            move |result| {
                                Message::InitializeProfileDbResult(
                                    init_profile_id.clone(),
                                    result,
                                )
                            },
                        ));
                    }
                }

                if app.active_profile_id == profile_id {
                    if let Some(task) = schedule_tweak_client_detection(app) {
                        return Some(task);
                    }
                }
            }
            Some(Task::none())
        }
        Message::SwitchProfile(pid) => {
            if pid != app.active_profile_id {
                if let Some(p) = app.profiles.iter().find(|p| p.id == pid).cloned() {
                    let pname = p.name.clone();
                    app.ignored_update_ids_by_profile.insert(
                        app.active_profile_id.clone(),
                        app.ignored_update_ids.clone(),
                    );
                    app.active_profile_id = pid.clone();
                    app.wow_dir = p.wow_dir.clone();
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
                    app.last_infrequent_check_unix = 0;
                    if app.active_tab == crate::Tab::Mods
                        && !app
                            .mods_warning_dismissed_profile_ids
                            .contains(&app.active_profile_id)
                    {
                        app.dialog = Some(Dialog::ModsWarning {
                            do_not_show_again: false,
                        });
                    }
                    if app.db_path.as_ref().map_or(false, |p| p.exists()) {
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
            let db_path = settings::profile_db_path(&profile_id).unwrap_or_default();
            let pid_clone = profile_id.clone();
            Some(Task::perform(
                async move {
                    let mut err = None;
                    if db_path.exists() {
                        if let Err(e) = std::fs::remove_file(&db_path) {
                            err = Some(format!(
                                "Failed to delete database file {}: {}",
                                db_path.display(),
                                e
                            ));
                        }
                    }
                    (pid_clone, err)
                },
                |res| Message::RemoveProfileResult(res.0, res.1),
            ))
        }
        Message::RemoveProfileResult(pid, err) => {
            app.profiles.retain(|p| p.id != pid);
            app.mods_warning_dismissed_profile_ids.remove(&pid);
            app.ignored_update_ids_by_profile.remove(&pid);
            if let Some(e) = err {
                app.log(LogLevel::Error, &e);
                app.show_toast(
                    format!("Profile metadata removed, but database file was locked."),
                    ToastKind::Warn,
                );
            } else {
                app.log(LogLevel::Info, &format!("Profile removed: {}", pid));
                app.show_toast(format!("Profile '{}' removed.", pid), ToastKind::Success);
            }
            app.save_settings();
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
                    &format!("Failed to initialize profile database for {}: {}", profile_id, err),
                ),
            }
            Some(Task::none())
        }
        Message::SettingsLoaded(s) => {
            let theme = WuddleTheme::from_key(&s.theme);
            app.wuddle_theme = theme;
            app.opt_friz_font = s.opt_friz_font;
            let mut colors = theme.colors();
            colors.body_font = app.body_font();
            app.theme_colors = colors;
            app.active_profile_id = s.active_profile_id.clone();

            app.opt_auto_check = s.opt_auto_check;
            app.opt_desktop_notify = s.opt_desktop_notify;
            app.opt_symlinks = s.opt_symlinks;
            app.opt_xattr = s.opt_xattr;
            app.opt_clock12 = s.opt_clock12;
            app.migrated_from_tauri = s.migrated_from_tauri;

            app.log_wrap = s.log_wrap;
            app.log_autoscroll = s.log_autoscroll;
            app.auto_check_minutes = s.auto_check_minutes.max(1);
            app.ignored_update_ids_by_profile = s
                .ignored_update_ids_by_profile
                .into_iter()
                .map(|(profile_id, ids)| (profile_id, ids.into_iter().collect()))
                .collect();
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
            app.update_channel = s.update_channel;
            app.ui_scale_mode = s.ui_scale_mode;
            app.ui_scale = resolve_ui_scale(s.ui_scale_mode);
            app.profiles = if s.profiles.is_empty() {
                vec![ProfileConfig::default()]
            } else {
                s.profiles
            };
            if let Some(p) = app.profiles.iter().find(|p| p.id == app.active_profile_id) {
                app.wow_dir = p.wow_dir.clone();
            } else if let Some(first) = app.profiles.first() {
                app.active_profile_id = first.id.clone();
                app.wow_dir = first.wow_dir.clone();
            }
            app.db_path = settings::resolve_profile_db_path(&app.active_profile_id).ok();
            app.log(LogLevel::Info, "Settings loaded.");
            let mut tasks = vec![crate::update::repos::refresh_repos_task(app)];
            if let Some(task) = schedule_tweak_client_detection(app) {
                tasks.push(task);
            }
            Some(Task::batch(tasks))
        }
        Message::SaveSettings => {
            app.save_settings();
            Some(Task::none())
        }
        Message::PickWowDirectory => Some(Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Select WoW Directory")
                    .pick_folder()
                    .await
                    .map(|h| h.path().to_path_buf())
            },
            Message::WowPathPicked,
        )),
        Message::PickWowExecutable => Some(Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .add_filter("Windows executable", &["exe"])
                    .set_title("Select Game Executable")
                    .pick_file()
                    .await
                    .map(|h| h.path().to_path_buf())
            },
            Message::WowPathPicked,
        )),
        Message::WowPathPicked(opt) => {
            if let Some(path) = opt {
                let selected = path.to_string_lossy().to_string();
                let (dir, auto_launch_exe) = settings::normalize_wow_path_input(&selected);
                let display = settings::wow_path_display(&dir, auto_launch_exe.as_deref());
                app.log(LogLevel::Info, &format!("WoW path set: {}", display));
                if let Some(Dialog::InstanceSettings {
                    ref mut wow_dir, ..
                }) = app.dialog
                {
                    *wow_dir = display;
                } else {
                    app.wow_dir = dir.clone();
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
                return Some(crate::update::repos::check_updates_task(app));
            }
            Some(Task::none())
        }
        _ => None,
    }
}
