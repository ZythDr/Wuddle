use crate::{App, LogLevel, Message, ToastKind};
use crate::components::helpers::copy_to_clipboard;
use crate::service;
use iced::Task;

pub fn open_url(app: &mut App, url: String) -> Task<Message> {
    if let Err(e) = open::that(&url) {
        app.log(LogLevel::Error, &format!("Failed to open URL: {}", e));
    }
    Task::none()
}

pub fn open_directory(app: &mut App, path: String) -> Task<Message> {
    if let Err(e) = open::that(&path) {
        app.log(LogLevel::Error, &format!("Failed to open directory: {}", e));
    }
    Task::none()
}

pub fn copy_to_clipboard_handler(app: &mut App, text_val: String) -> Task<Message> {
    match copy_to_clipboard(&text_val) {
        Ok(()) => {
            app.log(LogLevel::Info, "Copied to clipboard.");
            app.show_toast("Copied to clipboard.", ToastKind::Info);
        }
        Err(e) => {
            app.log(LogLevel::Error, &format!("Clipboard error: {}", e));
            app.show_toast(format!("Clipboard error: {}", e), ToastKind::Error);
        }
    }
    Task::none()
}

pub fn launch_game(app: &mut App) -> Task<Message> {
    if app.wow_dir.is_empty() {
        app.log(LogLevel::Error, "Set a WoW directory in Options first.");
        Task::none()
    } else {
        let active = app.profiles.iter()
            .find(|p| p.id == app.active_profile_id)
            .cloned()
            .unwrap_or_default();
        let cfg = service::LaunchConfig {
            method: active.launch_method,
            auto_launch_exe: active.auto_launch_exe,
            lutris_target: active.lutris_target,
            wine_command: active.wine_command,
            wine_args: active.wine_args,
            custom_command: active.custom_command,
            custom_args: active.custom_args,
            clear_wdb: active.clear_wdb,
        };
        app.log(LogLevel::Info, &format!(
            "Launching game (method: {})...", cfg.method
        ));
        let wow = app.wow_dir.clone();
        Task::perform(
            service::launch_game(wow, cfg),
            Message::LaunchGameResult,
        )
    }
}

pub fn launch_game_result(app: &mut App, result: Result<String, String>) -> Task<Message> {
    match result {
        Ok(msg) => app.log(LogLevel::Info, &msg),
        Err(e) => {
            app.log(LogLevel::Error, &format!("Launch failed: {}", e));
            app.show_toast(format!("Launch failed: {}", e), ToastKind::Error);
        }
    }
    Task::none()
}

pub fn spinner_tick(app: &mut App) -> Task<Message> {
    app.spinner_tick = (app.spinner_tick + 1) % 36;
    // Auto-dismiss toasts
    for t in &mut app.toasts { t.ttl = t.ttl.saturating_sub(1); }
    app.toasts.retain(|t| t.ttl > 0);
    Task::none()
}

pub fn dismiss_toast(app: &mut App, id: usize) -> Task<Message> {
    app.toasts.retain(|t| t.id != id);
    Task::none()
}

pub fn update(app: &mut App, message: Message) -> Option<Task<Message>> {
    match message {
        Message::OpenUrl(url) => Some(open_url(app, url)),
        Message::OpenDirectory(path) => Some(open_directory(app, path)),
        Message::CopyToClipboard(text) => Some(copy_to_clipboard_handler(app, text)),
        Message::LaunchGame => Some(launch_game(app)),
        Message::LaunchGameResult(res) => Some(launch_game_result(app, res)),
        Message::SpinnerTick => Some(spinner_tick(app)),
        Message::DismissToast(id) => Some(dismiss_toast(app, id)),
        _ => None,
    }
}
