use crate::{App, LogLevel, Message, ToastKind};
use crate::components::helpers::copy_to_clipboard;
use crate::service;
use iced::Task;
use std::time::{Duration, Instant};

const MINIMUM_LAUNCH_FEEDBACK: Duration = Duration::from_secs(1);

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
    if app.launch_in_progress {
        return Task::none();
    }
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
            #[cfg(feature = "auto-login")]
            profile_id: active.id.clone(),
            #[cfg(feature = "auto-login")]
            auto_login_account_id: active
                .auto_login_enabled
                .then(|| active.selected_auto_login_account_id.clone())
                .flatten(),
        };
        app.log(LogLevel::Info, &format!(
            "Launching game (method: {})...", cfg.method
        ));
        app.launch_in_progress = true;
        let wow = app.wow_dir.clone();
        Task::perform(launch_game_with_minimum_feedback(wow, cfg), Message::LaunchGameResult)
    }
}

/// Keep the launch affordance visible long enough to acknowledge the click.
/// The launcher still starts immediately; only the UI result is delayed.
async fn launch_game_with_minimum_feedback(
    wow_dir: String,
    cfg: service::LaunchConfig,
) -> Result<String, String> {
    let started_at = Instant::now();
    let result = service::launch_game(wow_dir, cfg).await;
    if let Some(remaining) = MINIMUM_LAUNCH_FEEDBACK.checked_sub(started_at.elapsed()) {
        tokio::time::sleep(remaining).await;
    }
    result
}

pub fn launch_game_result(app: &mut App, result: Result<String, String>) -> Task<Message> {
    app.launch_in_progress = false;
    match result {
        Ok(msg) => app.log(LogLevel::Info, &msg),
        Err(e) => {
            app.log(LogLevel::Error, &format!("Launch failed: {}", e));
            app.show_toast(format!("Launch failed: {}", e), ToastKind::Error);
        }
    }
    Task::none()
}

fn focus_existing_window() -> Task<Message> {
    iced::window::latest().and_then(|id| {
        Task::batch([
            iced::window::minimize(id, false),
            iced::window::gain_focus(id),
            iced::window::request_user_attention(
                id,
                Some(iced::window::UserAttention::Informational),
            ),
        ])
    })
}

fn launch_root_tool(app: &mut App, candidates: &[&str], result: fn(Result<String, String>) -> Message) -> Task<Message> {
    if app.wow_dir.is_empty() {
        app.show_toast("Set a WoW directory in Options first.", ToastKind::Warn);
        return Task::none();
    }
    let active = app.active_profile().cloned().unwrap_or_default();
    let cfg = service::LaunchConfig {
        method: active.launch_method,
        auto_launch_exe: active.auto_launch_exe,
        lutris_target: active.lutris_target,
        wine_command: active.wine_command,
        wine_args: active.wine_args,
        custom_command: active.custom_command,
        custom_args: active.custom_args,
        clear_wdb: false,
        #[cfg(feature = "auto-login")]
        profile_id: active.id.clone(),
        #[cfg(feature = "auto-login")]
        auto_login_account_id: None,
    };
    Task::perform(
        service::launch_wow_root_tool(
            app.wow_dir.clone(),
            cfg,
            candidates.iter().map(|name| (*name).to_string()).collect(),
        ),
        result,
    )
}

fn launch_tool_result(app: &mut App, result: Result<String, String>) -> Task<Message> {
    match result {
        Ok(message) => {
            app.log(LogLevel::Info, &message);
            app.show_toast(message, ToastKind::Info);
        }
        Err(error) => {
            app.log(LogLevel::Error, &error);
            app.show_toast(error, ToastKind::Error);
        }
    }
    Task::none()
}

pub fn spinner_tick(app: &mut App) -> Task<Message> {
    app.spinner_tick = (app.spinner_tick + 1) % 36;
    if app.collection_marquee_hovered {
        app.collection_marquee_tick = app.collection_marquee_tick.wrapping_add(1);
    }
    // Auto-dismiss visible toasts. Entering and exiting transitions do not
    // consume any of the notification's readable lifetime.
    for toast in &mut app.toasts {
        if matches!(toast.animation, crate::ToastAnimation::Visible) {
            toast.ttl = toast.ttl.saturating_sub(1);
            if toast.ttl == 0 {
                toast.animation = crate::ToastAnimation::Exiting(0);
            }
        }
    }
    Task::none()
}

pub fn dismiss_toast(app: &mut App, id: usize) -> Task<Message> {
    if let Some(toast) = app.toasts.iter_mut().find(|toast| toast.id == id) {
        if !matches!(toast.animation, crate::ToastAnimation::Exiting(_)) {
            toast.animation = crate::ToastAnimation::Exiting(0);
        }
    }
    Task::none()
}

pub fn toast_animation_tick(app: &mut App) -> Task<Message> {
    for toast in &mut app.toasts {
        toast.animation = match toast.animation {
            crate::ToastAnimation::Entering(tick)
                if tick.saturating_add(1) >= crate::TOAST_ANIMATION_TICKS =>
            {
                crate::ToastAnimation::Visible
            }
            crate::ToastAnimation::Entering(tick) => {
                crate::ToastAnimation::Entering(tick.saturating_add(1))
            }
            crate::ToastAnimation::Visible => crate::ToastAnimation::Visible,
            crate::ToastAnimation::Exiting(tick) => {
                crate::ToastAnimation::Exiting(tick.saturating_add(1))
            }
        };
    }
    app.toasts.retain(|toast| {
        !matches!(
            toast.animation,
            crate::ToastAnimation::Exiting(tick) if tick >= crate::TOAST_ANIMATION_TICKS
        )
    });
    Task::none()
}

pub fn update(app: &mut App, message: Message) -> Option<Task<Message>> {
    match message {
        Message::OpenUrl(url) => Some(open_url(app, url)),
        Message::OpenDirectory(path) => Some(open_directory(app, path)),
        Message::CopyToClipboard(text) => Some(copy_to_clipboard_handler(app, text)),
        Message::LaunchGame => Some(launch_game(app)),
        Message::LaunchGameResult(res) => Some(launch_game_result(app, res)),
        Message::PollSingleInstanceActivation => {
            if crate::single_instance::take_focus_request() {
                Some(focus_existing_window())
            } else {
                Some(Task::none())
            }
        }
        Message::LaunchWowOptimize => Some(launch_root_tool(
            app,
            &["wow_optimize_launcher.exe"],
            Message::LaunchWowOptimizeResult,
        )),
        Message::LaunchWowOptimizeResult(res) => Some(launch_tool_result(app, res)),
        Message::RunAwesomeWotlkPatch => {
            app.dialog = None;
            if app.wow_dir.is_empty() {
                app.show_toast("Set a WoW directory in Options first.", ToastKind::Warn);
                return Some(Task::none());
            }
            let active = app.active_profile().cloned().unwrap_or_default();
            let cfg = service::LaunchConfig {
                method: active.launch_method,
                auto_launch_exe: active.auto_launch_exe,
                lutris_target: active.lutris_target,
                wine_command: active.wine_command,
                wine_args: active.wine_args,
                custom_command: active.custom_command,
                custom_args: active.custom_args,
                clear_wdb: false,
                #[cfg(feature = "auto-login")]
                profile_id: active.id.clone(),
                #[cfg(feature = "auto-login")]
                auto_login_account_id: None,
            };
            Some(Task::perform(
                service::patch_wow_with_awesome_wotlk(app.wow_dir.clone(), cfg),
                Message::RunAwesomeWotlkPatchResult,
            ))
        }
        Message::RunAwesomeWotlkPatchResult(res) => Some(launch_tool_result(app, res)),
        Message::SpinnerTick => Some(spinner_tick(app)),
        Message::DismissToast(id) => Some(dismiss_toast(app, id)),
        Message::ToastAnimationTick => Some(toast_animation_tick(app)),
        _ => None,
    }
}
