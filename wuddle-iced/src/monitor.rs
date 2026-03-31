/// Detect the primary monitor's resolution (width, height) in physical pixels.
/// Returns `None` if detection fails. Works on both Linux (X11/xrandr) and Windows (Win32 GDI).

#[cfg(target_os = "linux")]
pub fn primary_monitor_size() -> Option<(u32, u32)> {
    // Try X11 via x11rb first (works on X11 and XWayland)
    if let Some(size) = x11_monitor_size() {
        return Some(size);
    }
    // Fallback: parse xrandr output
    xrandr_fallback()
}

#[cfg(target_os = "linux")]
fn x11_monitor_size() -> Option<(u32, u32)> {
    use x11rb::connection::Connection;
    use x11rb::protocol::randr;

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let res = randr::get_screen_resources_current(&conn, root).ok()?.reply().ok()?;

    // Find the first active CRTC (has non-zero dimensions)
    for &crtc in res.crtcs.iter() {
        let info = randr::get_crtc_info(&conn, crtc, 0).ok()?.reply().ok()?;
        if info.width > 0 && info.height > 0 {
            return Some((info.width as u32, info.height as u32));
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn xrandr_fallback() -> Option<(u32, u32)> {
    let output = std::process::Command::new("xrandr")
        .arg("--current")
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Look for line like: "DP-1 connected primary 2560x1440+0+0 ..."
    // or fallback to first connected output with a resolution
    for line in stdout.lines() {
        if line.contains(" connected") {
            // Find WxH pattern
            for part in line.split_whitespace() {
                if let Some((w, rest)) = part.split_once('x') {
                    // Handle "2560x1440+0+0" format
                    let h = rest.split('+').next().unwrap_or(rest);
                    if let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                        if w > 0 && h > 0 {
                            return Some((w, h));
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
pub fn primary_monitor_size() -> Option<(u32, u32)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };

    unsafe {
        let hmon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        if hmon == 0 {
            return None;
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(hmon, &mut info) == 0 {
            return None;
        }
        let r = info.rcMonitor;
        Some(((r.right - r.left) as u32, (r.bottom - r.top) as u32))
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn primary_monitor_size() -> Option<(u32, u32)> {
    None
}
