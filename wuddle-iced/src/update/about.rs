use iced::Task;
use crate::{App, Message, LogLevel, ToastKind, Tab, Dialog};
use crate::service;
use crate::settings::UpdateChannel;

pub fn update(app: &mut App, message: Message) -> Option<Task<Message>> {
    match message {
        Message::CheckSelfUpdate => {
            app.log(LogLevel::Info, "Checking for Wuddle updates...");
            return Some(Task::perform(
                service::check_self_update_full(app.update_channel == UpdateChannel::Beta),
                Message::CheckSelfUpdateResult
            ));
        }
        Message::CheckSelfUpdateResult(result) => {
            match result {
                Ok(status) => {
                    app.self_update_supported = status.supported;
                    app.self_update_available = status.update_available;
                    app.self_update_assets_pending = status.assets_pending;
                    app.latest_version = status.latest_version;
                    app.update_message = Some(status.message.clone());
                    app.log(LogLevel::Info, &format!("Version check: {}", status.message));
                    if status.update_available {
                        let ver = app.latest_version.as_deref().unwrap_or("new version");
                        app.show_toast_with_action(
                            format!("Wuddle {} is available. Click to view.", ver),
                            ToastKind::Info,
                            Message::SetTab(Tab::About),
                        );
                    }
                }
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Version check failed: {}", e));
                    app.show_toast(format!("Version check failed: {}", e), ToastKind::Error);
                }
            }
            return Some(Task::none());
        }
        Message::ApplySelfUpdate => {
            if app.self_update_in_progress { return Some(Task::none()); }
            app.self_update_in_progress = true;
            app.update_message = Some("Downloading update...".to_string());
            app.log(LogLevel::Info, "Downloading Wuddle update...");
            let beta = app.update_channel == UpdateChannel::Beta;
            return Some(Task::perform(
                service::apply_self_update(beta),
                Message::ApplySelfUpdateResult,
            ));
        }
        Message::ApplySelfUpdateResult(result) => {
            app.self_update_in_progress = false;
            match result {
                Ok(msg) => {
                    app.self_update_done = true;
                    app.self_update_available = false;
                    app.update_message = Some(msg.clone());
                    app.log(LogLevel::Info, &msg);
                    app.show_toast("Update downloaded — restarting...", ToastKind::Info);
                    return Some(Task::done(Message::RestartAfterUpdate));
                }
                Err(e) => {
                    app.update_message = Some(format!("Update failed: {}", e));
                    app.log(LogLevel::Error, &format!("Self-update failed: {}", e));
                    app.show_toast(format!("Self-update failed: {}", e), ToastKind::Error);
                }
            }
            return Some(Task::none());
        }
        Message::RestartAfterUpdate => {
            app.log(LogLevel::Info, "Restarting Wuddle...");
            if let Err(e) = service::restart_app() {
                app.log(LogLevel::Error, &format!("Restart failed: {}", e));
                app.update_message = Some(format!("Restart failed: {}", e));
                app.show_toast(format!("Restart failed: {}", e), ToastKind::Error);
            }
            return Some(Task::none());
        }
        Message::ShowChangelog => {
            app.dialog = Some(Dialog::Changelog { title: "Wuddle Changelog".to_string(), items: Vec::new(), loading: true });
            return Some(Task::perform(service::fetch_changelog(), Message::ChangelogLoaded));
        }
        Message::ChangelogLoaded(result) => {
            if let Some(Dialog::Changelog { ref mut items, ref mut loading, .. }) = app.dialog {
                *loading = false;
                let text = result.unwrap_or_else(|e| format!("Failed to load changelog: {}", e));
                *items = iced::widget::markdown::Content::parse(&text).items().to_vec();
            }
            return Some(Task::none());
        }
        Message::SetUpdateChannel(ch) => {
            app.update_channel = ch;
            app.save_settings();
            app.log(LogLevel::Info, &format!("Update channel set to {:?}.", ch));
            return Some(Task::none());
        }
        Message::SwitchToStableChannel => {
            app.log(LogLevel::Info, "Switching to stable (Tauri) channel...");
            if !switch_to_stable_channel() {
                let _ = open::that("https://github.com/ZythDr/Wuddle/releases");
            }
            return Some(Task::none());
        }
        _ => None,
    }
}

fn switch_to_stable_channel() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false; };
    // Expect layout: <launcher_dir>/versions/<version>/<binary>
    let Some(launcher_dir) = exe.parent().and_then(|v| v.parent()).and_then(|v| v.parent()) else {
        return false;
    };
    let Ok(entries) = std::fs::read_dir(launcher_dir.join("versions")) else { return false; };

    let mut stables: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| {
            let l = n.to_lowercase();
            !l.contains("alpha") && !l.contains("beta") && !l.contains("pre") && !l.contains("rc")
        })
        .collect();

    if stables.is_empty() { return false; }

    stables.sort_by(|a, b| semver_parts(b).cmp(&semver_parts(a)));
    let best = &stables[0];
    let json = format!("{{\"current\":\"{}\"}}\n", best);
    if std::fs::write(launcher_dir.join("current.json"), json).is_err() { return false; }

    std::process::exit(0);
}

fn semver_parts(s: &str) -> Vec<u64> {
    s.trim_start_matches(|c: char| !c.is_ascii_digit())
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|seg| seg.parse::<u64>().ok())
        .collect()
}
