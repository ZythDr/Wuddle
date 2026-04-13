pub mod tabs;
pub mod dialog;
pub mod toasts;
pub mod config;
pub mod tweak_types;

pub use tabs::Tab;
pub use dialog::*;
pub use toasts::*;
pub use config::*;
pub use tweak_types::*;

/// Total horizontal space reserved by the vertical scrollbar (width + spacing).
pub const VSCROLL_RESERVED: f32 = 18.0; // width 10 + spacing 8
