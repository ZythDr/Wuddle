use iced::Task;
use crate::{App, Message, Dialog, InstanceField, ToastKind};
use crate::theme::WuddleTheme;
use crate::types::LogLevel;
use crate::settings::{self, ProfileConfig, resolve_ui_scale};

pub fn update(app: &mut App, message: Message) -> Option<Task<Message>> {
    match message {
        Message::SetTheme(theme) => {
            app.wuddle_theme = theme;
            app.save_settings();
            app.log(LogLevel::Info, &format!("Theme switched to: {}.", theme.key()));
            Some(Task::none())
        }
        Message::ToggleAutoCheck(b) => {
            app.opt_auto_check = b;
            app.save_settings();
            app.log(LogLevel::Info, &format!("Auto-check updates: {}.", if b { "enabled" } else { "disabled" }));
            Some(Task::none())
        }
        Message::SetAutoCheckMinutes(s) => {
            if let Ok(n) = s.parse::<u32>() {
                app.auto_check_minutes = n.max(1);
            } else if s.is_empty() {
                app.auto_check_minutes = 1;
            }
            app.save_settings();
            app.log(LogLevel::Info, &format!("Auto-check interval set to {} min.", app.auto_check_minutes));
            Some(Task::none())
        }
        Message::ToggleDesktopNotify(b) => {
            app.opt_desktop_notify = b;
            app.save_settings();
            app.log(LogLevel::Info, &format!("Desktop notifications: {}.", if b { "enabled" } else { "disabled" }));
            Some(Task::none())
        }
        Message::ToggleSymlinks(b) => {
            app.opt_symlinks = b;
            app.save_settings();
            app.log(LogLevel::Info, &format!("Symlinks: {}.", if b { "enabled" } else { "disabled" }));
            Some(Task::none())
        }
        Message::ToggleXattr(b) => {
            app.opt_xattr = b;
            app.save_settings();
            app.log(LogLevel::Info, &format!("Extended attributes: {}.", if b { "enabled" } else { "disabled" }));
            Some(Task::none())
        }
        Message::ToggleClock12(b) => {
            app.opt_clock12 = b;
            app.save_settings();
            app.log(LogLevel::Info, &format!("12-hour clock: {}.", if b { "enabled" } else { "disabled" }));
            Some(Task::none())
        }
        Message::ToggleFrizFont(b) => {
            app.opt_friz_font = b;
            app.save_settings();
            app.log(LogLevel::Info, "Friz Quadrata font setting saved. Restart Wuddle to apply.");
            Some(Task::none())
        }
        Message::SetUiScaleMode(mode) => {
            app.ui_scale_mode = mode;
            app.ui_scale = resolve_ui_scale(mode);
            app.save_settings();
            app.log(LogLevel::Info, &format!("UI scale set to {} ({}%)", mode.label(), (app.ui_scale * 100.0) as u32));
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
        Message::ForgetGithubToken => {
            Some(Task::perform(
                async move { crate::service::clear_github_token().await },
                Message::ForgetGithubTokenResult,
            ))
        }
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
                ref mut name, ref mut wow_dir, ref mut launch_method,
                ref mut like_turtles, ref mut clear_wdb,
                ref mut lutris_target, ref mut wine_command, ref mut wine_args,
                ref mut custom_command, ref mut custom_args, ..
            }) = app.dialog {
                match field {
                    InstanceField::Name(v) => *name = v,
                    InstanceField::WowDir(v) => *wow_dir = v,
                    InstanceField::LaunchMethod(v) => *launch_method = v,
                    InstanceField::LikeTurtles(v) => *like_turtles = v,
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
                is_new, profile_id: dialog_profile_id, name, wow_dir, launch_method,
                like_turtles, clear_wdb,
                lutris_target, wine_command, wine_args,
                custom_command, custom_args,
            }) = app.dialog.take() {
                let profile_name = if name.trim().is_empty() { String::from("Default") } else { name.trim().to_string() };
                let dir = wow_dir.trim().to_string();
                let profile_id = if is_new {
                    profile_name.to_lowercase().replace(' ', "-")
                } else if !dialog_profile_id.is_empty() {
                    dialog_profile_id
                } else {
                    app.profiles.iter()
                        .find(|p| p.name == profile_name)
                        .map(|p| p.id.clone())
                        .unwrap_or_else(|| app.active_profile_id.clone())
                };

                let config = ProfileConfig {
                    id: profile_id.clone(),
                    name: profile_name.clone(),
                    wow_dir: dir.clone(),
                    launch_method,
                    like_turtles,
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
                    app.wow_dir = dir;
                }
                app.save_settings();
                app.log(LogLevel::Info, &format!("Instance profile saved: {}", profile_name));
            }
            Some(Task::none())
        }
        Message::SwitchProfile(pid) => {
            if pid != app.active_profile_id {
                if let Some(p) = app.profiles.iter().find(|p| p.id == pid) {
                    let pname = p.name.clone();
                    app.active_profile_id = pid.clone();
                    app.wow_dir = p.wow_dir.clone();
                    app.db_path = settings::resolve_profile_db_path(&pid).ok();
                    app.repos.clear();
                    app.plans.clear();
                    app.last_checked = None;
                    if app.db_path.as_ref().map_or(false, |p| p.exists()) {
                        app.loading = true;
                    }
                    app.loading = true;
                    app.log(LogLevel::Info, &format!("Switched to profile: {} ({})", pname, pid));
                    app.save_settings();
                    return Some(crate::update::repos::refresh_repos_task(app));
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
            Some(Task::perform(
                async move {
                    if db_path.exists() {
                        let _ = std::fs::remove_file(db_path);
                    }
                    Ok(profile_id)
                },
                Message::RemoveProfileResult,
            ))
        }
        Message::RemoveProfileResult(result) => {
            if let Ok(pid) = result {
                app.profiles.retain(|p| p.id != pid);
                app.log(LogLevel::Info, &format!("Profile removed: {}", pid));
                app.save_settings();
            }
            Some(Task::none())
        }
        Message::SettingsLoaded(s) => {
            app.wuddle_theme = WuddleTheme::from_key(&s.theme);
            app.active_profile_id = s.active_profile_id.clone();
            app.opt_auto_check = s.opt_auto_check;
            app.opt_desktop_notify = s.opt_desktop_notify;
            app.opt_symlinks = s.opt_symlinks;
            app.opt_xattr = s.opt_xattr;
            app.radio_auto_connect = s.radio_auto_connect;
            app.radio_volume = s.radio_volume;
            app.radio_auto_play = s.radio_auto_play;
            app.radio_buffer_size = s.radio_buffer_size;
            app.radio_persist_volume = s.radio_persist_volume;
            app.opt_clock12 = s.opt_clock12;
            app.opt_friz_font = s.opt_friz_font;
            app.log_wrap = s.log_wrap;
            app.log_autoscroll = s.log_autoscroll;
            app.auto_check_minutes = s.auto_check_minutes.max(1);
            app.ignored_update_ids = s.ignored_update_ids.into_iter().collect();
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
            if app.radio_auto_connect {
                tasks.push(Task::done(Message::AutoConnectRadio));
            }
            Some(Task::batch(tasks))
        }
        Message::SaveSettings => {
            app.save_settings();
            Some(Task::none())
        }
        Message::PickWowDirectory => {
            Some(Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Select WoW Directory")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                Message::WowDirectoryPicked,
            ))
        }
        Message::WowDirectoryPicked(opt) => {
            if let Some(path) = opt {
                let dir = path.to_string_lossy().to_string();
                app.log(LogLevel::Info, &format!("WoW directory set: {}", dir));
                if let Some(Dialog::InstanceSettings { ref mut wow_dir, .. }) = app.dialog {
                    *wow_dir = dir;
                } else {
                    app.wow_dir = dir;
                    app.save_settings();
                    return Some(crate::update::repos::refresh_repos_task(app));
                }
            }
            Some(Task::none())
        }
        Message::AutoCheckTick => {
            if app.opt_auto_check && !app.checking_updates {
                app.checking_updates = true;
                let skip = if wuddle_engine::github_token().is_some() {
                    std::collections::HashSet::new()
                } else {
                    crate::update::repos::infrequent_skip_ids(&app.repos, &app.plans, app.last_infrequent_check_unix)
                };
                let skipped = skip.len();
                if skipped > 0 {
                    app.log(LogLevel::Info, &format!(
                        "Auto-checking for updates ({} infrequent repos skipped)...", skipped
                    ));
                } else {
                    app.log(LogLevel::Info, "Auto-checking for updates...");
                }
                let db = app.db_path.clone();
                let wow = if app.wow_dir.is_empty() { None } else { Some(app.wow_dir.clone()) };
                return Some(Task::perform(
                    crate::service::check_updates_skip(db, wow, wuddle_engine::CheckMode::Force, skip),
                    Message::CheckUpdatesResult,
                ));
            }
            Some(Task::none())
        }
        _ => None,
    }
}
