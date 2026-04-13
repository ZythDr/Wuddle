use iced::Task;
use crate::{App, Message, LogLevel, ToastKind, TweakId, TweakValues};
use crate::service;

pub fn update(app: &mut App, message: Message) -> Option<Task<Message>> {
    match message {
        Message::ToggleTweak(id, b) => {
            match id {
                TweakId::Fov => app.tweaks.fov = b,
                TweakId::Farclip => app.tweaks.farclip = b,
                TweakId::Frilldistance => app.tweaks.frilldistance = b,
                TweakId::NameplateDist => app.tweaks.nameplate_dist = b,
                TweakId::CameraSkip => app.tweaks.camera_skip = b,
                TweakId::MaxCameraDist => app.tweaks.max_camera_dist = b,
                TweakId::SoundBg => app.tweaks.sound_bg = b,
                TweakId::SoundChannels => app.tweaks.sound_channels = b,
                TweakId::Quickloot => app.tweaks.quickloot = b,
                TweakId::LargeAddress => app.tweaks.large_address = b,
            }
            return Some(Task::none());
        }
        Message::SetTweakFov(v) => {
            app.tweak_values.fov = v;
            return Some(Task::none());
        }
        Message::SetTweakFarclip(v) => {
            app.tweak_values.farclip = v;
            return Some(Task::none());
        }
        Message::SetTweakFrilldistance(v) => {
            app.tweak_values.frilldistance = v;
            return Some(Task::none());
        }
        Message::SetTweakNameplateDist(v) => {
            app.tweak_values.nameplate_dist = v;
            return Some(Task::none());
        }
        Message::SetTweakMaxCameraDist(s) => {
            if let Ok(v) = s.parse::<f32>() {
                app.tweak_values.max_camera_dist = v.clamp(10.0, 200.0);
            }
            return Some(Task::none());
        }
        Message::SetTweakSoundChannels(s) => {
            if let Ok(v) = s.parse::<u32>() {
                app.tweak_values.sound_channels = v.clamp(1, 999);
            }
            return Some(Task::none());
        }
        Message::ReadTweaks => {
            if app.wow_dir.is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
            } else {
                app.log(LogLevel::Info, "Reading current tweaks from WoW.exe...");
                let wow = app.wow_dir.clone();
                return Some(Task::perform(service::read_tweaks(wow), Message::ReadTweaksResult));
            }
            return Some(Task::none());
        }
        Message::ReadTweaksResult(result) => {
            match result {
                Ok(tv) => {
                    app.log(LogLevel::Info, "Tweaks read successfully.");
                    app.tweak_values.fov = tv.fov;
                    app.tweak_values.farclip = tv.farclip;
                    app.tweak_values.frilldistance = tv.frilldistance;
                    app.tweak_values.nameplate_dist = tv.nameplate_distance;
                    app.tweak_values.max_camera_dist = tv.max_camera_distance;
                    app.tweak_values.sound_channels = tv.sound_channels;
                    
                    // Update enabled states based on what was read
                    // In the monolith it didn't seem to do this, but it makes sense to enable them if they are found.
                    // However, we'll stick to just updating values for now as per current logic.
                    app.tweaks.quickloot = tv.quickloot;
                    app.tweaks.sound_bg = tv.sound_in_background;
                    app.tweaks.large_address = tv.large_address_aware;
                    app.tweaks.camera_skip = tv.camera_skip_fix;
                }
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Failed to read tweaks: {}", e));
                    app.show_toast(format!("Read failed: {}", e), ToastKind::Error);
                }
            }
            return Some(Task::none());
        }
        Message::ApplyTweaks => {
            if app.wow_dir.is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
            } else {
                app.log(LogLevel::Info, "Applying tweaks to WoW.exe...");
                let wow = app.wow_dir.clone();
                let tv = &app.tweak_values;
                let ts = &app.tweaks;
                let opts = crate::tweaks::TweakOptions {
                    fov:                if ts.fov { Some(tv.fov) } else { None },
                    farclip:            if ts.farclip { Some(tv.farclip) } else { None },
                    frilldistance:      if ts.frilldistance { Some(tv.frilldistance) } else { None },
                    nameplate_distance: if ts.nameplate_dist { Some(tv.nameplate_dist) } else { None },
                    sound_channels:     if ts.sound_channels { Some(tv.sound_channels) } else { None },
                    max_camera_distance: if ts.max_camera_dist { Some(tv.max_camera_dist) } else { None },
                    quickloot:          ts.quickloot,
                    sound_in_background:ts.sound_bg,
                    large_address_aware:ts.large_address,
                    camera_skip_fix:    ts.camera_skip,
                };
                return Some(Task::perform(service::apply_tweaks(wow, opts), Message::ApplyTweaksResult));
            }
            return Some(Task::none());
        }
        Message::ApplyTweaksResult(result) => {
            match result {
                Ok(msg) => {
                    app.log(LogLevel::Info, &msg);
                    app.show_toast("Tweaks applied successfully.", ToastKind::Info);
                }
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Apply tweaks failed: {}", e));
                    app.show_toast(format!("Apply tweaks failed: {}", e), ToastKind::Error);
                }
            }
            return Some(Task::none());
        }
        Message::RestoreTweaks => {
            if app.wow_dir.is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
            } else {
                app.log(LogLevel::Info, "Restoring WoW.exe from backup...");
                let wow = app.wow_dir.clone();
                return Some(Task::perform(service::restore_tweaks(wow), Message::RestoreTweaksResult));
            }
            return Some(Task::none());
        }
        Message::RestoreTweaksResult(result) => {
            match result {
                Ok(msg) => {
                    app.log(LogLevel::Info, &msg);
                    app.show_toast("WoW.exe restored from backup.", ToastKind::Info);
                }
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Restore tweaks failed: {}", e));
                    app.show_toast(format!("Restore failed: {}", e), ToastKind::Error);
                }
            }
            return Some(Task::none());
        }
        Message::ResetTweaksToDefault => {
            app.tweak_values = TweakValues::default();
            app.log(LogLevel::Info, "Tweak values reset to defaults.");
            return Some(Task::none());
        }
        _ => None,
    }
}
