//! Update handler modules.
//!
//! The monolithic `App::update` match is split by domain:
//! - `misc` — clipboard, toasts, spinner, game launch
//! - `radio` — all radio messages
//! - `repos` — repo CRUD and update operations
//! - `settings` — options, profiles, instance settings
//! - `about` — self-update, changelog, release channel
//! - `tweaks` - all tweak messages

pub mod misc;
pub mod radio;
pub mod tweaks;
pub mod about;
pub mod settings;
pub mod repos;
