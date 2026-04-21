//! Shared UI helper functions and widgets used across multiple panels and dialogs.

use iced::widget::{button, canvas, column, container, text};
use iced::{Element, Length, Theme};
use crate::{Dialog, Message, Tab};
use std::sync::OnceLock;
use crate::theme::{self, ThemeColors};
use crate::service::{self, is_mod};

// ---------------------------------------------------------------------------
// Tooltip wrapper
// ---------------------------------------------------------------------------

pub fn tip<'a>(
    content: impl Into<Element<'a, Message>>,
    tip_text: &str,
    pos: iced::widget::tooltip::Position,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let tip_str = String::from(tip_text);
    iced::widget::tooltip(
        content,
        container(text(tip_str).size(13).color(c.text))
            .padding([3, 8])
            .style(move |_theme| theme::tooltip_style(&c)),
        pos,
    )
    .gap(4.0)
    .into()
}

// ---------------------------------------------------------------------------
// Close button (dialog ✕)
// ---------------------------------------------------------------------------

pub fn close_button<'a>(colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    button(text("\u{2715}").size(14).color(c.bad)) // ✕ in red
        .on_press(Message::CloseDialog)
        .padding([4, 8])
        .style(move |_theme, status| match status {
            button::Status::Hovered => button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    c.bad.r, c.bad.g, c.bad.b, 0.15,
                ))),
                text_color: c.bad,
                border: iced::Border {
                    color: iced::Color::from_rgba(c.bad.r, c.bad.g, c.bad.b, 0.4),
                    width: 1.0,
                    radius: iced::border::Radius::from(4),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            },
            _ => button::Style {
                background: None,
                text_color: c.bad,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
        })
        .into()
}

// ---------------------------------------------------------------------------
// Small colored badge label
// ---------------------------------------------------------------------------

pub fn badge_tag<'a>(
    label: &'static str,
    text_color: iced::Color,
    base_color: iced::Color,
) -> Element<'a, Message> {
    container(
        text(label).size(14).color(text_color)
    )
    .padding([2, 6])
    .style(move |_t| container::Style {
        background: Some(iced::Background::Color(
            iced::Color::from_rgba(base_color.r, base_color.g, base_color.b, 0.18)
        )),
        border: iced::Border {
            color: iced::Color::from_rgba(base_color.r, base_color.g, base_color.b, 0.45),
            width: 1.0,
            radius: 5.0.into(),
        },
        ..Default::default()
    })
    .into()
}

// ---------------------------------------------------------------------------
// Context menu item
// ---------------------------------------------------------------------------

pub fn ctx_menu_item<'a>(label: &str, msg: Message, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    button(text(String::from(label)).size(12))
        .on_press(msg)
        .padding([6, 12])
        .width(Length::Fill)
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(&c),
            _ => button::Style {
                background: None,
                text_color: c.text,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
        })
        .into()
}

// ---------------------------------------------------------------------------
// Inline context menu for a repo row
// ---------------------------------------------------------------------------

/// Build the context menu content for a repo row (used inline in the row itself).
pub fn inline_context_menu<'a>(
    app: &crate::App,
    repo: &service::RepoRow,
    collection_addon: Option<&str>,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let rid = repo.id;
    let has_update = app.plans.iter().any(|p| p.repo_id == rid && p.has_update);
    let enabled = repo.enabled;
    let is_mod_val = is_mod(repo);
    let update_ignored = app.ignored_update_ids.contains(&rid);
    let name = format!("{}/{}", repo.owner, repo.name);

    let mut items: Vec<Element<Message>> = Vec::new();

    if has_update && !update_ignored {
        items.push(ctx_menu_item("\u{2193} Update", Message::UpdateRepo(rid), &c));
    }
    items.push(ctx_menu_item("Reinstall / Repair", Message::ReinstallRepo(rid), &c));
    if let Some(addon_name) = collection_addon {
        items.push(ctx_menu_item(
            "Manage Collection\u{2026}",
            Message::OpenCollectionManager(rid),
            &c,
        ));
        items.push(ctx_menu_item(
            "Browse\u{2026}",
            Message::BrowseAddonInstall {
                repo_id: rid,
                addon_name: addon_name.to_string(),
            },
            &c,
        ));
    } else {
        if repo.is_collection {
            items.push(ctx_menu_item(
                "Manage Collection\u{2026}",
                Message::OpenCollectionManager(rid),
                &c,
            ));
        }
        items.push(ctx_menu_item("Browse\u{2026}", Message::BrowseRepo(rid), &c));
    }
    if crate::panels::projects::is_dxvk_repo(&repo.name) {
        items.push(ctx_menu_item("\u{2699} Configure DXVK\u{2026}", Message::OpenDxvkConfig, &c));
    }
    if is_mod_val {
        let label = if enabled { "Disable" } else { "Enable" };
        items.push(ctx_menu_item(label, Message::ToggleRepoEnabled(rid, !enabled), &c));
    }
    let ignore_label = if update_ignored { "Unignore Updates" } else { "Ignore Updates" };
    items.push(ctx_menu_item(ignore_label, Message::ToggleIgnoreUpdates(rid), &c));

    if is_mod_val {
        let merge_label = if repo.merge_installs { "\u{2713} Merge Updates" } else { "Merge Updates" };
        items.push(ctx_menu_item(merge_label, Message::ToggleMergeInstalls(rid, !repo.merge_installs), &c));
    }

    let c3 = c;
    let remove_message = if let Some(addon_name) = collection_addon {
        Message::RemoveCollectionAddonPrompt {
            repo_id: rid,
            addon_name: addon_name.to_string(),
        }
    } else {
        Message::OpenDialog(Dialog::RemoveRepo {
            id: rid,
            name,
            remove_files: false,
            files: Vec::new(),
        })
    };
    items.push(
        button(text("Remove").size(12).color(c.bad))
            .on_press(remove_message)
            .padding([6, 12])
            .width(Length::Fill)
            .style(move |_theme, status| {
                let mut s = match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c3),
                    _ => button::Style {
                        background: None,
                        text_color: c3.text,
                        border: iced::Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: true,
                    },
                };
                s.text_color = c3.bad;
                s
            })
            .into(),
    );

    container(column(items).spacing(2))
        .padding(6)
        .width(200)
        .style(move |_theme| theme::context_menu_style(&c))
        .into()
}

// ---------------------------------------------------------------------------
// Markdown / Code block helpers
// ---------------------------------------------------------------------------

/// Wraps an element (typically a code block) in a stack with a "Copy" button
/// overlaid at the top-right corner.
pub fn with_copy_button<'a>(
    block: Element<'a, Message>,
    code: String,
) -> Element<'a, Message> {
    let copy_btn = container(
        button(text("Copy").size(11))
            .on_press(Message::CopyToClipboard(code))
            .padding([2, 8])
            .style(|_theme, status| match status {
                button::Status::Hovered => button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.15,
                    ))),
                    text_color: iced::Color::WHITE,
                    border: iced::Border { radius: 3.0.into(), ..Default::default() },
                    ..Default::default()
                },
                _ => button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.07,
                    ))),
                    text_color: iced::Color::from_rgb8(0xb0, 0xc4, 0xde),
                    border: iced::Border { radius: 3.0.into(), ..Default::default() },
                    ..Default::default()
                },
            }),
    )
    .width(Length::Fill)
    .align_x(iced::Alignment::End)
    .padding(iced::Padding {
        top: 4.0,
        right: 6.0,
        bottom: 0.0,
        left: 0.0,
    });

    iced::widget::stack![block, copy_btn].into()
}

// ---------------------------------------------------------------------------
// Clipboard helper
// ---------------------------------------------------------------------------

/// Writes `text` to the system clipboard.
///
/// On Linux the clipboard is owned by a process; we spin a background thread
/// that holds `Clipboard` alive so clipboard managers can read and cache the
/// content before the owner exits.
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use arboard::SetExtLinux;
        let text_owned = text.to_string();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        std::thread::spawn(move || {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set().wait_until(deadline).text(text_owned);
            }
        });
        return Ok(());
    }

    #[cfg(not(target_os = "linux"))]
    {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if cb.set_text(text).is_ok() {
                return Ok(());
            }
        }
        Err("Clipboard unavailable".to_string())
    }
}

// ---------------------------------------------------------------------------
// Canvas-drawn spinner widget
// ---------------------------------------------------------------------------

/// A rotating arc spinner drawn on a canvas, matching Tauri's CSS border-top spinner.
pub struct SpinnerCanvas {
    pub tick: usize,
    pub color: iced::Color,
}

impl<Message> canvas::Program<Message> for SpinnerCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = frame.center();
        let radius = bounds.width.min(bounds.height) / 2.0 - 2.0;
        let stroke_width = 3.0;

        let bg_circle = canvas::Path::circle(center, radius);
        frame.stroke(
            &bg_circle,
            canvas::Stroke::default()
                .with_color(iced::Color { a: 0.18, ..self.color })
                .with_width(stroke_width),
        );

        let start_angle = (self.tick as f32) * (std::f32::consts::TAU / 36.0);
        let sweep = std::f32::consts::FRAC_PI_2 * 3.0; // 270 degrees
        let arc = canvas::Path::new(|b| {
            b.arc(canvas::path::Arc {
                center,
                radius,
                start_angle: iced::Radians(start_angle),
                end_angle: iced::Radians(start_angle + sweep),
            });
        });
        frame.stroke(
            &arc,
            canvas::Stroke::default()
                .with_color(self.color)
                .with_width(stroke_width)
                .with_line_cap(canvas::LineCap::Round),
        );

        vec![frame.into_geometry()]
    }
}

// ---------------------------------------------------------------------------
// Git error helpers
// ---------------------------------------------------------------------------

/// Returns `true` for error codes the user has chosen to silence
/// (e.g. -16 = GIT_EAUTH, produced by deleted or private repositories).
pub fn is_silenced_git_error(raw: &str) -> bool {
    raw.contains("(-16)")
}

/// Converts a verbose libgit2/network error chain into a short human-readable
/// message, appending the numeric error code when one is present.
pub fn simplify_git_error(raw: &str) -> String {
    let error_code: Option<String> = raw
        .find("code=")
        .and_then(|i| {
            let after = &raw[i..];
            let lparen = after.find('(')?;
            let rparen = after.find(')')?;
            if rparen > lparen {
                let num = after[lparen + 1..rparen].trim();
                if num.chars().all(|c| c.is_ascii_digit() || c == '-') {
                    return Some(num.to_string());
                }
            }
            None
        });

    let mut inner = raw;
    while let Some(pos) = inner.find("): ") {
        inner = &inner[pos + 3..];
    }
    if let Some(start) = inner.find("(auth failed: ") {
        inner = inner[start + 14..].trim_end_matches(|c: char| c == ')' || c == ' ');
    }
    inner = inner.strip_prefix("Git sync check failed: ").unwrap_or(inner);

    let lower = inner.to_lowercase();
    let msg = if lower.contains("authentication required")
        || lower.contains("code=auth")
        || lower.contains("class=http (34)")
        || lower.contains("auth failed")
    {
        "Repository not found or requires authentication".to_string()
    } else if lower.contains("not found") || lower.contains("404") {
        "Repository not found".to_string()
    } else if lower.contains("timed out")
        || lower.contains("connection refused")
        || lower.contains("network unreachable")
    {
        "Network error — check your connection".to_string()
    } else if inner.len() > 120 {
        format!("{}…", &inner[..120])
    } else {
        inner.to_string()
    };

    match error_code {
        Some(code) => format!("{} (Error Code {})", msg, code),
        None => msg,
    }
}

// ---------------------------------------------------------------------------
// Time utilities
// ---------------------------------------------------------------------------

pub fn chrono_now_fmt(use_12h: bool) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h24 = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    let s = secs % 60;
    if use_12h {
        let ampm = if h24 < 12 { "AM" } else { "PM" };
        let h12 = match h24 % 12 { 0 => 12, h => h };
        format!("{:02}:{:02}:{:02} {}", h12, mins, s, ampm)
    } else {
        format!("{:02}:{:02}:{:02}", h24, mins, s)
    }
}

pub fn chrono_now() -> String {
    chrono_now_fmt(false)
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ---------------------------------------------------------------------------
// Infrequent repo skip logic
// ---------------------------------------------------------------------------

/// 3 days in seconds — repos whose latest release is older than this are "infrequent".
pub const INFREQUENT_THRESHOLD_SECS: i64 = 3 * 24 * 3600;
/// 4 hours in seconds — minimum interval between infrequent repo checks.
pub const INFREQUENT_CHECK_INTERVAL_SECS: i64 = 4 * 3600;

/// Returns the set of repo IDs that should be skipped on this auto-check tick
/// because they are infrequently updated and were checked recently enough.
pub fn infrequent_skip_ids(
    repos: &[service::RepoRow],
    plans: &[service::PlanRow],
    last_infrequent_check_unix: i64,
) -> std::collections::HashSet<i64> {
    let now = now_unix();
    let recently_checked = (now - last_infrequent_check_unix) < INFREQUENT_CHECK_INTERVAL_SECS;

    if !recently_checked {
        return std::collections::HashSet::new();
    }

    let has_update: std::collections::HashSet<i64> = plans.iter()
        .filter(|p| p.has_update)
        .map(|p| p.repo_id)
        .collect();

    repos.iter()
        .filter(|r| {
            if has_update.contains(&r.id) {
                return false;
            }
            match r.published_at_unix {
                Some(pub_at) => (now - pub_at) > INFREQUENT_THRESHOLD_SECS,
                None => false,
            }
        })
        .map(|r| r.id)
        .collect()
}

// ---------------------------------------------------------------------------
// Forge / Tab Icons
// ---------------------------------------------------------------------------

/// Build an SVG handle for a forge icon.
/// `forge_url` is used to distinguish Codeberg from other Gitea-based instances.
pub fn forge_svg_handle(forge: &str, forge_url: &str) -> iced::widget::svg::Handle {
    let svg: &str = match forge {
        "github" => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">"#,
            r#"<path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 "#,
            r#"11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61"#,
            r#"-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 "#,
            r#"1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776"#,
            r#".417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22"#,
            r#"-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 "#,
            r#"1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 "#,
            r#"3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 "#,
            r#"2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 "#,
            r#"12.297c0-6.627-5.373-12-12-12"/></svg>"#,
        ),
        "gitlab" => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">"#,
            r#"<path d="M23.955 13.587l-1.342-4.135-2.664-8.189c-.135-.423-.73-.423-.867 0L16.42 "#,
            r#"9.452H7.582L4.918 1.263c-.135-.423-.731-.423-.867 0L1.386 9.452.044 13.587c-.121"#,
            r#".374.014.784.33 1.016L12 22.047l11.625-8.444c.317-.232.452-.642.33-1.016"/></svg>"#,
        ),
        _ => "",
    };

    let resolved_svg = if svg.is_empty() {
        if forge_url.contains("codeberg") {
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">"#,
                r#"<path d="M11.999.747A11.974 11.974 0 000 12.75c0 2.254.635 4.465 1.833 6.376L11.837 "#,
                r#"6.19c.072-.092.251-.092.323 0l4.178 5.402h-2.992l.065.239h3.113l.882 1.138h-3.674"#,
                r#"l.103.374h3.86l.777 1.003h-4.358l.135.483h4.593l.695.894h-5.038l.165.589h5.326"#,
                r#"l.609.785h-5.717l.182.65h6.038l.562.727h-6.397l.183.65h6.717A12.003 12.003 0 0024"#,
                r#" 12.75 11.977 11.977 0 0011.999.747zm3.654 19.104.182.65h5.326c.173-.204.353-.433"#,
                r#".513-.65zm.385 1.377.18.65h3.563c.233-.198.485-.428.712-.65zm.383 1.377.182.648h"#,
                r#"1.203c.356-.204.685-.412 1.042-.648z"/>"#,
                r#"</svg>"#,
            ).to_string()
        } else {
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" "#,
                r#"stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">"#,
                r#"<path d="M5 9h14v7a3 3 0 0 1-3 3H8a3 3 0 0 1-3-3V9z"/>"#,
                r#"<path d="M5 9V7a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v2"/>"#,
                r#"<path d="M19 11.5h1a2 2 0 0 1 0 4h-1"/>"#,
                r#"</svg>"#,
            ).to_string()
        }
    } else {
        svg.to_string()
    };

    iced::widget::svg::Handle::from_memory(resolved_svg.as_bytes().to_vec())
}

/// SVG icons for the Options / Logs / About tab buttons.
pub fn tab_icon_svg(tab: Tab) -> iced::widget::svg::Handle {
    let svg: &'static str = match tab {
        Tab::Options => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" "#,
            r#"stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">"#,
            r#"<path d="M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6z"/>"#,
            r#"<path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06"#,
            r#"a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09"#,
            r#"A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83"#,
            r#"l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09"#,
            r#"A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0"#,
            r#"l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09"#,
            r#"a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83"#,
            r#"l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09"#,
            r#"a1.65 1.65 0 0 0-1.51 1z"/></svg>"#,
        ),
        Tab::Logs => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" "#,
            r#"stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">"#,
            r#"<path d="M5 4.5A1.5 1.5 0 0 1 6.5 3h9l4.5 4.5V19.5A1.5 1.5 0 0 1 18.5 21h-12"#,
            r#"A1.5 1.5 0 0 1 5 19.5v-15Zm10 .5v3h3"/>"#,
            r#"<path d="M8 11h8M8 14h8M8 17h6"/>"#,
            r#"</svg>"#,
        ),
        Tab::About => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">"#,
            r#"<path fill-rule="evenodd" d="M12 2a10 10 0 0 1 0 20a10 10 0 0 1 0-20z "#,
            r#"M12 6.8a1.2 1.2 0 0 1 0 2.4a1.2 1.2 0 0 1 0-2.4z "#,
            r#"M10.5 11h3v7h-3z"/></svg>"#,
        ),
        _ => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"></svg>"#,
    };
    iced::widget::svg::Handle::from_memory(svg.as_bytes().to_vec())
}

// ---------------------------------------------------------------------------
// Notification icon helper
// ---------------------------------------------------------------------------

/// Returns a path to a temp copy of the app icon, suitable for desktop notifications.
pub fn notification_icon_path() -> &'static str {
    static ICON_PATH: OnceLock<String> = OnceLock::new();
    ICON_PATH.get_or_init(|| {
        let icon_bytes = include_bytes!("../../assets/icons/128x128.png");
        let dir = std::env::temp_dir().join("wuddle");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("notification-icon.png");
        if !path.exists()
            || std::fs::metadata(&path)
                .map(|m| m.len())
                .unwrap_or(0)
                != icon_bytes.len() as u64
        {
            let _ = std::fs::write(&path, icon_bytes);
        }
        path.to_string_lossy().into_owned()
    })
}
