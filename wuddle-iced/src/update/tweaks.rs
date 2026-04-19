use iced::Task;
use crate::{App, Message, LogLevel, Tab, ToastKind, TweakId, TweakValues};
use crate::service;

pub fn update(app: &mut App, message: Message) -> Option<Task<Message>> {
    match message {
        Message::DetectTweakClientResult(result) => {
            app.tweak_client_checking = false;
            match result {
                Ok(info) => {
                    let file_version = info
                        .file_version
                        .clone()
                        .or(info.product_version.clone())
                        .unwrap_or_else(|| "unknown version".to_string());
                    let exe_name = info.executable_name.clone();
                    let supported = info.supports_legacy_1121_tweaks;
                    app.tweak_client_error = None;
                    app.tweak_client_info = Some(info);
                    app.log(
                        LogLevel::Info,
                        &format!("Tweaks target detected: {} ({})", exe_name, file_version),
                    );
                    if !supported && app.active_tab == Tab::Tweaks {
                        app.active_tab = Tab::Home;
                        app.show_toast(
                            "Tweaks are disabled for this client. Only legacy 1.12.1 executables are supported.",
                            ToastKind::Info,
                        );
                    }
                }
                Err(e) => {
                    app.tweak_client_info = None;
                    app.tweak_client_error = Some(e.clone());
                    app.log(LogLevel::Error, &format!("Client detection failed: {}", e));
                    if app.active_tab == Tab::Tweaks {
                        app.active_tab = Tab::Home;
                    }
                }
            }
            return Some(Task::none());
        }
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
            if let Some(reason) = app.tweaks_disabled_reason() {
                app.log(LogLevel::Error, &reason);
            } else {
                let auto_launch_exe = app.active_profile().and_then(|profile| profile.auto_launch_exe.clone());
                let exe_name = app
                    .tweak_client_info
                    .as_ref()
                    .map(|info| info.executable_name.clone())
                    .or(auto_launch_exe.clone())
                    .unwrap_or_else(|| "WoW.exe".to_string());
                app.log(LogLevel::Info, &format!("Reading current tweaks from {}...", exe_name));
                let wow = app.wow_dir.clone();
                return Some(Task::perform(
                    service::read_tweaks(wow, auto_launch_exe),
                    Message::ReadTweaksResult,
                ));
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
            if let Some(reason) = app.tweaks_disabled_reason() {
                app.log(LogLevel::Error, &reason);
            } else {
                let auto_launch_exe = app.active_profile().and_then(|profile| profile.auto_launch_exe.clone());
                let exe_name = app
                    .tweak_client_info
                    .as_ref()
                    .map(|info| info.executable_name.clone())
                    .or(auto_launch_exe.clone())
                    .unwrap_or_else(|| "WoW.exe".to_string());
                app.log(LogLevel::Info, &format!("Applying tweaks to {}...", exe_name));
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
                return Some(Task::perform(
                    service::apply_tweaks(wow, auto_launch_exe, opts),
                    Message::ApplyTweaksResult,
                ));
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
            if let Some(reason) = app.tweaks_disabled_reason() {
                app.log(LogLevel::Error, &reason);
            } else {
                let auto_launch_exe = app.active_profile().and_then(|profile| profile.auto_launch_exe.clone());
                let exe_name = app
                    .tweak_client_info
                    .as_ref()
                    .map(|info| info.executable_name.clone())
                    .or(auto_launch_exe.clone())
                    .unwrap_or_else(|| "WoW.exe".to_string());
                app.log(LogLevel::Info, &format!("Restoring {} from backup...", exe_name));
                let wow = app.wow_dir.clone();
                return Some(Task::perform(
                    service::restore_tweaks(wow, auto_launch_exe),
                    Message::RestoreTweaksResult,
                ));
            }
            return Some(Task::none());
        }
        Message::RestoreTweaksResult(result) => {
            match result {
                Ok(msg) => {
                    app.log(LogLevel::Info, &msg);
                    app.show_toast(msg, ToastKind::Info);
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
