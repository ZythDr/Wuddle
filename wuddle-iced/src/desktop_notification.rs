//! Small platform boundary for desktop notifications.
//!
//! Linux uses the freedesktop notification service. Windows uses the same
//! explicit application identity as Wuddle's window instead of falling back
//! to PowerShell's identity.

pub fn show_updates_available(count: usize) -> Result<(), String> {
    let body = format!(
        "{} update{} available",
        count,
        if count == 1 { "" } else { "s" }
    );
    show("Wuddle", &body)
}

#[cfg(target_os = "windows")]
fn show(title: &str, body: &str) -> Result<(), String> {
    tauri_winrt_notification::Toast::new("ZythDr.Wuddle")
        .title(title)
        .text1(body)
        .show()
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn show(title: &str, body: &str) -> Result<(), String> {
    use std::collections::HashMap;
    use zbus::zvariant::Value;

    let connection = zbus::blocking::Connection::session().map_err(|error| error.to_string())?;
    let hints = HashMap::<&str, Value<'_>>::new();
    let actions = Vec::<&str>::new();
    let reply = connection
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                "Wuddle",
                0_u32,
                crate::notification_icon_path(),
                title,
                body,
                actions,
                hints,
                -1_i32,
            ),
        )
        .map_err(|error| error.to_string())?;
    let _: u32 = reply
        .body()
        .deserialize()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn show(_title: &str, _body: &str) -> Result<(), String> {
    Ok(())
}
