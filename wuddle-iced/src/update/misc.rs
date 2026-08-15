use crate::components::helpers::copy_to_clipboard;
use crate::service;
use crate::{App, LogLevel, Message, ToastKind};
use iced::Task;
use std::time::{Duration, Instant};

const MINIMUM_LAUNCH_FEEDBACK: Duration = Duration::from_secs(1);

fn validated_external_web_url(raw: &str) -> Result<reqwest::Url, &'static str> {
    let parsed =
        reqwest::Url::parse(raw.trim()).map_err(|_| "This link is not a valid web address.")?;
    if !matches!(parsed.scheme(), "https" | "http") || parsed.host_str().is_none() {
        return Err("For safety, Wuddle only opens HTTP and HTTPS web links.");
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("For safety, Wuddle will not open links containing credentials.");
    }
    Ok(parsed)
}

pub fn open_url(app: &mut App, url: String) -> Task<Message> {
    let url = match validated_external_web_url(&url) {
        Ok(url) => url,
        Err(message) => {
            app.log(LogLevel::Error, message);
            app.show_toast(message, ToastKind::Warn);
            return Task::none();
        }
    };
    if open::that(url.as_str()).is_err() {
        // Platform errors can echo the full URL. Keep signed query parameters
        // out of logs while still giving the user an actionable result.
        let message = "The web link could not be opened by your system.";
        app.log(LogLevel::Error, message);
        app.show_toast(message, ToastKind::Error);
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
        let active = app
            .profiles
            .iter()
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
        app.log(
            LogLevel::Info,
            &format!("Launching game (method: {})...", cfg.method),
        );
        app.launch_in_progress = true;
        let wow = app.wow_dir.clone();
        Task::perform(
            launch_game_with_minimum_feedback(wow, cfg),
            Message::LaunchGameResult,
        )
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

fn launch_root_tool(
    app: &mut App,
    candidates: &[&str],
    result: fn(Result<String, String>) -> Message,
) -> Task<Message> {
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
    Task::none()
}

pub fn set_toast_hovered(app: &mut App, id: usize, hovered: bool) -> Task<Message> {
    if let Some(toast) = app.toasts.iter_mut().find(|toast| toast.id == id) {
        toast.set_hovered(hovered);
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
        // The same ~60 FPS clock drives both the smooth lifetime bar and the
        // enter/exit transitions. Paused notifications retain a full bar.
        if toast.tick_lifetime() {
            toast.animation = crate::ToastAnimation::Exiting(0);
            continue;
        }
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
        Message::WindowMoved(position) => {
            app.window_geometry
                .remember_position(position.x, position.y);
            Some(Task::none())
        }
        Message::WindowResized(size) => {
            app.window_geometry.remember_size(size.width, size.height);
            Some(Task::none())
        }
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
        Message::ToastHovered(id, hovered) => Some(set_toast_hovered(app, id, hovered)),
        Message::ToastAnimationTick => Some(toast_animation_tick(app)),
        _ => None,
    }
}

#[cfg(test)]
mod url_tests {
    use super::validated_external_web_url;

    #[test]
    fn external_links_allow_only_credential_free_web_urls() {
        assert!(validated_external_web_url("https://example.org/readme").is_ok());
        assert!(validated_external_web_url("http://example.org/readme").is_ok());
        for unsafe_url in [
            "file:///home/alice/private.txt",
            "javascript:alert(1)",
            "steam://run/123",
            "https://token@example.org/private",
            "mailto:alice@example.org",
        ] {
            assert!(
                validated_external_web_url(unsafe_url).is_err(),
                "{unsafe_url} should be rejected"
            );
        }
    }
}
