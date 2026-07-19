//! Update handler modules.
//!
//! The monolithic `App::update` match is split by domain:
//! - `misc` — clipboard, toasts, spinner, game launch
//! - `repos` — repo CRUD and update operations
//! - `settings` — options, profiles, instance settings
//! - `about` — self-update, changelog, release channel
//! - `tweaks` - all tweak messages

pub mod about;
pub mod misc;
pub mod repos;
pub mod settings;
pub mod tweaks;
