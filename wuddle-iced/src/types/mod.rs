pub mod config;
pub mod dialog;
pub mod tabs;
pub mod toasts;
pub mod tweak_types;

pub use config::*;
pub use dialog::*;
pub use tabs::Tab;
pub use toasts::*;
pub use tweak_types::*;

/// Total horizontal space reserved by the vertical scrollbar (width + spacing).
pub const VSCROLL_RESERVED: f32 = 18.0; // width 10 + spacing 8
