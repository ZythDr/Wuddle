use iced::Task;
use crate::{App, Message, LogLevel, Dialog};

pub fn update(app: &mut App, message: Message) -> Option<Task<Message>> {
    match message {
        Message::ToggleRadio => {
            if app.radio_playing {
                app.radio_playing = false;
                if let Some(h) = app.radio_handle.as_ref() {
                    h.fade_out();
                }
                app.log(LogLevel::Info, "Radio: paused.");
                return Some(Task::none());
            }

            if let Some(h) = app.radio_handle.as_ref() {
                app.radio_playing = true;
                app.radio_error = None;
                h.fade_in(app.radio_volume);
                app.log(LogLevel::Info, "Radio: playing (instant resume).");
                return Some(Task::none());
            } else {
                app.radio_playing = true;
                app.radio_connecting = true;
                app.radio_error = None;
                app.log(LogLevel::Info, "Radio: connecting…");
                let (tx, rx) = tokio::sync::oneshot::channel::<Result<crate::radio::RadioHandle, String>>();
                let buffer_size = app.radio_buffer_size;
                std::thread::spawn(move || { let _ = tx.send(crate::radio::start(0.0, buffer_size)); });
                return Some(Task::perform(
                    async move { rx.await.unwrap_or_else(|_| Err("Thread died".to_string())) },
                    Message::RadioStarted,
                ));
            }
        }
        Message::ReconnectRadio => {
            if app.radio_connecting {
                return Some(Task::none());
            }
            if let Some(h) = app.radio_handle.take() {
                h.fade_out();
            }
            app.radio_playing = true;
            app.radio_connecting = true;
            app.radio_error = None;
            app.log(LogLevel::Info, "Radio: reconnecting…");
            let (tx, rx) = tokio::sync::oneshot::channel::<Result<crate::radio::RadioHandle, String>>();
            let buffer_size = app.radio_buffer_size;
            std::thread::spawn(move || { let _ = tx.send(crate::radio::start(0.0, buffer_size)); });
            return Some(Task::perform(
                async move { rx.await.unwrap_or_else(|_| Err("Thread died".to_string())) },
                Message::RadioStarted,
            ));
        }
        Message::RadioStarted(Ok(handle)) => {
            app.radio_connecting = false;
            if app.radio_playing {
                handle.fade_in(app.radio_volume);
                app.log(LogLevel::Info, "Radio: connected and playing.");
            } else if app.radio_auto_play {
                app.radio_playing = true;
                handle.fade_in(app.radio_volume);
                app.log(LogLevel::Info, "Radio: auto-playing.");
            } else {
                app.log(LogLevel::Info, "Radio: pre-loaded (muted).");
            }
            app.radio_handle = Some(handle);
            return Some(Task::none());
        }
        Message::RadioStarted(Err(e)) => {
            app.radio_connecting = false;
            app.radio_playing = false;
            app.log(LogLevel::Error, &format!("Radio: connection failed — {e}"));
            app.radio_error = Some(e);
            return Some(Task::none());
        }
        Message::SetRadioVolume(v) => {
            app.radio_volume = v;
            if app.radio_playing {
                if let Some(h) = app.radio_handle.as_ref() {
                    h.set_volume(v);
                }
            }
            return Some(Task::none());
        }
        Message::ToggleRadioMute => {
            if app.radio_volume > 0.0 {
                app.radio_pre_mute_volume = Some(app.radio_volume);
                return Some(Task::done(Message::SetRadioVolume(0.0)));
            } else if let Some(prev) = app.radio_pre_mute_volume.take() {
                return Some(Task::done(Message::SetRadioVolume(prev)));
            } else {
                return Some(Task::done(Message::SetRadioVolume(0.25)));
            }
        }
        Message::ToggleRadioAutoConnect(b) => {
            app.radio_auto_connect = b;
            app.save_settings();
            if b && app.radio_handle.is_none() && !app.radio_connecting {
                return Some(Task::done(Message::AutoConnectRadio));
            } else if !b && !app.radio_playing {
                if let Some(h) = app.radio_handle.take() { h.stop(); }
            }
            return Some(Task::none());
        }
        Message::AutoConnectRadio => {
            let like_turtles = app.profiles.iter()
                .find(|p| p.id == app.active_profile_id)
                .map(|p| p.like_turtles)
                .unwrap_or(true);
            if like_turtles && app.radio_auto_connect && app.radio_handle.is_none() && !app.radio_connecting {
                app.radio_connecting = true;
                app.log(LogLevel::Info, "Radio: pre-loading in background…");
                let (tx, rx) = tokio::sync::oneshot::channel::<Result<crate::radio::RadioHandle, String>>();
                let buffer_size = app.radio_buffer_size;
                std::thread::spawn(move || { let _ = tx.send(crate::radio::start(0.0, buffer_size)); });
                return Some(Task::perform(
                    async move { rx.await.unwrap_or_else(|_| Err("Thread died".to_string())) },
                    Message::RadioStarted,
                ));
            }
            return Some(Task::none());
        }
        Message::OpenRadioSettings => {
            let is_custom = !crate::panels::radio::BUFFER_PRESETS.iter().any(|(v, _)| *v == app.radio_buffer_size);
            app.dialog = Some(Dialog::RadioSettings {
                auto_connect: app.radio_auto_connect,
                auto_play: app.radio_auto_play,
                buffer_size: app.radio_buffer_size.to_string(),
                custom_buffer: is_custom,
                persist_volume: app.radio_persist_volume,
            });
            return Some(Task::none());
        }
        Message::CloseRadioSettings => {
            app.dialog = None;
            return Some(Task::none());
        }
        Message::SetRadioAutoConnect(b) => {
            if let Some(Dialog::RadioSettings { ref mut auto_connect, .. }) = app.dialog {
                *auto_connect = b;
            }
            return Some(Task::none());
        }
        Message::SetRadioAutoPlay(b) => {
            if let Some(Dialog::RadioSettings { ref mut auto_play, .. }) = app.dialog {
                *auto_play = b;
            }
            return Some(Task::none());
        }
        Message::SetRadioBufferSize(s) => {
            if let Some(Dialog::RadioSettings { ref mut buffer_size, ref mut custom_buffer, .. }) = app.dialog {
                *buffer_size = s;
                let val: usize = buffer_size.parse().unwrap_or(0);
                if crate::panels::radio::BUFFER_PRESETS.iter().any(|(v, _)| *v == val) {
                    *custom_buffer = false;
                }
            }
            return Some(Task::none());
        }
        Message::SetRadioCustomBuffer(b) => {
            if let Some(Dialog::RadioSettings { ref mut custom_buffer, .. }) = app.dialog {
                *custom_buffer = b;
            }
            return Some(Task::none());
        }
        Message::SetRadioPersistVolume(b) => {
            if let Some(Dialog::RadioSettings { ref mut persist_volume, .. }) = app.dialog {
                *persist_volume = b;
            }
            return Some(Task::none());
        }
        Message::SaveRadioSettings => {
            if let Some(Dialog::RadioSettings { auto_connect, auto_play, buffer_size, persist_volume, .. }) = &app.dialog {
                app.radio_auto_connect = *auto_connect;
                app.radio_auto_play = *auto_play;
                app.radio_persist_volume = *persist_volume;
                if let Ok(size) = buffer_size.parse::<usize>() {
                    let old_size = app.radio_buffer_size;
                    app.radio_buffer_size = size;
                    if old_size != size && app.radio_handle.is_some() {
                        app.log(LogLevel::Info, "Radio: Buffer size changed, restarting stream...");
                        return Some(Task::batch([
                            Task::done(Message::CloseRadioSettings),
                            Task::done(Message::ReconnectRadio)
                        ]));
                    }
                }
                app.dialog = None;
                app.log(LogLevel::Info, &format!(
                    "Radio settings saved. Auto-connect: {}, Auto-play: {}, Persist volume: {}, Buffer: {} bytes.",
                    app.radio_auto_connect, app.radio_auto_play, app.radio_persist_volume, app.radio_buffer_size
                ));
                app.save_settings();
            }
            return Some(Task::none());
        }
        _ => return None,
    }
}
