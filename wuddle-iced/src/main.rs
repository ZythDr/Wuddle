#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod anchored_overlay;
mod monitor;
pub mod panels;
pub mod service;
pub(crate) mod settings;
#[allow(dead_code)]
pub(crate) mod theme;
pub(crate) mod tweaks;

pub mod app;
pub mod components;
pub mod dialogs;
pub mod update;
pub mod types;
pub mod message;

pub use app::App;
pub use types::*;
pub use message::Message;
pub use components::markdown::ImageViewer;
pub use components::helpers::*;

use settings::{detect_auto_scale, AUTO_UI_SCALE};
use theme::{FRIZ, NOTO, LIFECRAFT};

fn main() -> iced::Result {
    prefer_x11_for_file_drops_if_requested();

    // Detect monitor resolution before iced starts
    let auto_scale = detect_auto_scale();
    AUTO_UI_SCALE.set(auto_scale).ok();

    // Read settings early so we can set the default font.
    // Noto Sans is the default UI font (matches Tauri's system-ui stack on Linux);
    // Friz Quadrata overrides it when the user opts in.
    let saved = settings::load_settings();
    let default_font = if saved.opt_friz_font { FRIZ } else { NOTO };

    let window_icon = iced::window::icon::from_file_data(
        include_bytes!("../assets/icons/128x128.png"),
        None,
    ).ok();

    iced::application(App::new, App::update, App::view)
        .title("Wuddle")
        .theme(App::theme)
        .subscription(App::subscription)
        .font(include_bytes!("../assets/fonts/LifeCraft_Font.ttf"))
        .font(include_bytes!("../assets/fonts/FrizQuadrataStd-Regular.otf"))
        .font(include_bytes!("../assets/fonts/NotoSans-Regular.ttf"))
        .font(include_bytes!("../assets/fonts/NotoSans-Bold.ttf"))
        .default_font(default_font)
        .window(iced::window::Settings {
            size: iced::Size::new(1100.0, 850.0),
            icon: window_icon,
            ..Default::default()
        })
        .scale_factor(|app| app.ui_scale)
        .run()
}

#[cfg(target_os = "linux")]
fn prefer_x11_for_file_drops_if_requested() {
    // Winit 0.30 receives file drop events on X11, but not through its Wayland
    // backend. Keep native Wayland by default; this opt-in is for users who
    // prefer drag-and-drop over native Wayland.
    if std::env::var_os("DISPLAY").is_some()
        && std::env::var_os("WUDDLE_USE_X11_FOR_DND").is_some()
    {
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("WAYLAND_SOCKET");
    }
}

#[cfg(not(target_os = "linux"))]
fn prefer_x11_for_file_drops_if_requested() {}
