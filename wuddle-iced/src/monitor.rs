/// A connected monitor rectangle in desktop coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Detect the primary monitor's resolution (width, height) in physical pixels.
/// Returns `None` if detection fails. Works on both Linux (X11/xrandr) and Windows (Win32 GDI).
pub fn primary_monitor_size() -> Option<(u32, u32)> {
    monitor_rects()
        .first()
        .map(|monitor| (monitor.width, monitor.height))
}

#[cfg(target_os = "linux")]
pub fn monitor_rects() -> Vec<MonitorRect> {
    // Try X11 via x11rb first (works on X11 and XWayland)
    let monitors = x11_monitor_rects();
    if !monitors.is_empty() {
        return monitors;
    }
    // Fallback: parse xrandr output
    xrandr_fallback()
}

#[cfg(target_os = "linux")]
fn x11_monitor_rects() -> Vec<MonitorRect> {
    use x11rb::connection::Connection;
    use x11rb::protocol::randr;

    let Ok((conn, screen_num)) = x11rb::connect(None) else {
        return Vec::new();
    };
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let Ok(cookie) = randr::get_screen_resources_current(&conn, root) else {
        return Vec::new();
    };
    let Ok(res) = cookie.reply() else {
        return Vec::new();
    };

    let mut monitors = Vec::new();
    for &crtc in res.crtcs.iter() {
        let Ok(cookie) = randr::get_crtc_info(&conn, crtc, 0) else {
            continue;
        };
        if let Ok(info) = cookie.reply() {
            if info.width > 0 && info.height > 0 {
                monitors.push(MonitorRect {
                    x: i32::from(info.x),
                    y: i32::from(info.y),
                    width: u32::from(info.width),
                    height: u32::from(info.height),
                });
            }
        }
    }
    monitors
}

#[cfg(target_os = "linux")]
fn xrandr_fallback() -> Vec<MonitorRect> {
    let Ok(output) = std::process::Command::new("xrandr")
        .arg("--current")
        .output()
    else {
        return Vec::new();
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut monitors = Vec::new();
    for line in stdout.lines() {
        if line.contains(" connected") {
            if let Some(rect) = line.split_whitespace().find_map(parse_xrandr_geometry) {
                monitors.push((line.contains(" connected primary "), rect));
            }
        }
    }
    monitors.sort_by_key(|(primary, _)| !*primary);
    monitors.into_iter().map(|(_, rect)| rect).collect()
}

#[cfg(target_os = "windows")]
pub fn monitor_rects() -> Vec<MonitorRect> {
    use windows_sys::Win32::Foundation::{BOOL, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};

    unsafe {
        unsafe extern "system" fn collect_monitor(
            _monitor: HMONITOR,
            _dc: HDC,
            rect: *mut RECT,
            data: LPARAM,
        ) -> BOOL {
            if rect.is_null() || data == 0 {
                return 1;
            }
            let rect = *rect;
            let monitors = &mut *(data as *mut Vec<MonitorRect>);
            monitors.push(MonitorRect {
                x: rect.left,
                y: rect.top,
                width: (rect.right - rect.left).max(0) as u32,
                height: (rect.bottom - rect.top).max(0) as u32,
            });
            1
        }

        let mut monitors = Vec::new();
        EnumDisplayMonitors(
            0,
            std::ptr::null(),
            Some(collect_monitor),
            &mut monitors as *mut Vec<MonitorRect> as LPARAM,
        );
        monitors.sort_by_key(|monitor| !(monitor.x == 0 && monitor.y == 0));
        monitors
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn monitor_rects() -> Vec<MonitorRect> {
    Vec::new()
}

fn parse_xrandr_geometry(value: &str) -> Option<MonitorRect> {
    let (width, rest) = value.split_once('x')?;
    let split_height = rest.find(['+', '-'])?;
    let (height, positions) = rest.split_at(split_height);
    let split_y = positions[1..].find(['+', '-'])? + 1;
    let (x, y) = positions.split_at(split_y);
    let width = width.parse::<u32>().ok()?;
    let height = height.parse::<u32>().ok()?;
    let x = x.parse::<i32>().ok()?;
    let y = y.parse::<i32>().ok()?;
    (width > 0 && height > 0).then_some(MonitorRect {
        x,
        y,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_positive_and_negative_xrandr_positions() {
        assert_eq!(
            parse_xrandr_geometry("2560x1440+0+120"),
            Some(MonitorRect {
                x: 0,
                y: 120,
                width: 2560,
                height: 1440,
            })
        );
        assert_eq!(
            parse_xrandr_geometry("1920x1080-1920+0"),
            Some(MonitorRect {
                x: -1920,
                y: 0,
                width: 1920,
                height: 1080,
            })
        );
    }
}
