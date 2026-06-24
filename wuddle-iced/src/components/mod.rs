//! Shared components and utility modules used across the Wuddle application.
//!
//! - `helpers` — small utility functions (badges, context menus, spinners, etc.)
//! - `chrome`  — app shell rendering (topbar, tab buttons, footer, forge icons)
//! - `drop_overlay` — full-window archive drag-and-drop hint
//! - `markdown` — markdown viewer with syntax highlighting, admonitions, copy-to-clipboard
//! - `presets` — Quick-Add preset data for the AddRepo dialog

#[allow(dead_code)]
pub mod chrome;
pub mod drop_overlay;
pub mod helpers;
pub mod markdown;
#[allow(dead_code)]
pub mod presets;
