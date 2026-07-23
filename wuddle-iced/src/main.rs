#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod anchored_overlay;
#[cfg(feature = "auto-login")]
mod auto_login;
mod diagnostics;
mod github_api;
mod monitor;
mod mpq;
pub mod panels;
mod platform_identity;
pub mod service;
pub(crate) mod settings;
mod single_instance;
mod storage;
#[allow(dead_code)]
pub(crate) mod theme;
pub(crate) mod tweaks;

pub mod app;
pub mod components;
pub mod dialogs;
pub mod message;
pub mod types;
pub mod update;

pub use app::App;
pub use components::helpers::*;
pub use components::markdown::ImageViewer;
pub use message::Message;
pub use types::*;

use settings::{detect_auto_scale, AUTO_UI_SCALE};
use theme::{FRIZ, LIFECRAFT, NOTO};

fn main() -> iced::Result {
    prefer_x11_for_file_drops_if_requested();
    platform_identity::initialize();

    #[cfg(target_os = "windows")]
    if let Err(error) = storage::initialize() {
        eprintln!("Wuddle storage initialization failed: {error}");
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Wuddle storage error")
            .set_description(format!(
                "Wuddle could not initialize its data directory and will not start.\n\n{error}\n\nIf Wuddle is installed in a read-only folder, set WUDDLE_DATA_DIR to a writable directory."
            ))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    single_instance::wait_for_restart_parent();

    // Detect monitor resolution before iced starts
    let auto_scale = detect_auto_scale();
    AUTO_UI_SCALE.set(auto_scale).ok();

    // Read settings early so we can set the default font.
    // Noto Sans is the default UI font (matches Tauri's system-ui stack on Linux);
    // Friz Quadrata overrides it when the user opts in.
    let saved = settings::load_settings();
    let default_font = if saved.opt_friz_font { FRIZ } else { NOTO };
    let _single_instance_guard = match single_instance::acquire() {
        Ok(single_instance::AcquireResult::Primary(guard)) => Some(guard),
        Ok(single_instance::AcquireResult::ExistingInstanceActivated) => return Ok(()),
        Err(error) => {
            eprintln!("Wuddle single-instance setup failed: {error}");
            None
        }
    };
    if let Err(error) = diagnostics::init(saved.verbose_diagnostics) {
        eprintln!("Wuddle diagnostic logging failed: {error}");
    }
    diagnostics::register_settings_paths(&saved);
    let default_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            })
            .unwrap_or_else(|| "unknown".to_string());
        diagnostics::write_system(
            "ERROR",
            "panic",
            &format!("Unexpected application panic at {location}; payload omitted for privacy"),
        );
        default_panic_hook(info);
    }));

    let window_icon =
        iced::window::icon::from_file_data(include_bytes!("../assets/icons/128x128.png"), None)
            .ok();

    let mut window_settings = iced::window::Settings {
        size: iced::Size::new(1100.0, 850.0),
        icon: window_icon,
        // Route title-bar closes through `Message::RequestExit` so settings
        // are saved and the Windows hard-exit watchdog can terminate any
        // blocked background work after the window disappears.
        exit_on_close_request: false,
        ..Default::default()
    };
    if saved.remember_window_geometry {
        if let Some((width, height)) = saved.window_geometry.initial_size() {
            window_settings.size = iced::Size::new(width, height);
        }
        if let Some((x, y)) = saved.window_geometry.initial_position() {
            window_settings.position = iced::window::Position::Specific(iced::Point::new(x, y));
        }
    }
    #[cfg(target_os = "linux")]
    {
        window_settings.platform_specific.application_id =
            platform_identity::LINUX_APPLICATION_ID.to_string();
    }

    iced::application(App::new, App::update, App::view)
        .title("Wuddle")
        .theme(App::theme)
        .subscription(App::subscription)
        .font(include_bytes!("../assets/fonts/LifeCraft_Font.ttf"))
        .font(include_bytes!(
            "../assets/fonts/FrizQuadrataStd-Regular.otf"
        ))
        .font(include_bytes!("../assets/fonts/NotoSans-Regular.ttf"))
        .font(include_bytes!("../assets/fonts/NotoSans-Bold.ttf"))
        .default_font(default_font)
        .window(window_settings)
        .scale_factor(|app| app.ui_scale)
        .run()
}

#[cfg(target_os = "linux")]
fn prefer_x11_for_file_drops_if_requested() {
    // Winit 0.30 receives file drop events on X11, but not through its Wayland
    // backend. Keep native Wayland by default; this opt-in is for users who
    // prefer drag-and-drop over native Wayland.
    if std::env::var_os("DISPLAY").is_some() && std::env::var_os("WUDDLE_USE_X11_FOR_DND").is_some()
    {
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("WAYLAND_SOCKET");
    }
}

#[cfg(not(target_os = "linux"))]
fn prefer_x11_for_file_drops_if_requested() {}
