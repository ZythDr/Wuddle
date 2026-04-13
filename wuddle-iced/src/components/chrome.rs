//! Top bar, tab buttons, panel body and footer rendering.
//!
//! These are the structural "chrome" elements of the main app window.
//! Functions take `&App` rather than `self` so they can live outside
//! the main `impl App` block.

use iced::widget::{button, canvas, container, row, rule, Space};
use iced::{Element, Length};

use crate::{App, Message, Tab, LIFECRAFT};
use crate::theme::{self, ThemeColors};
use crate::components::helpers::SpinnerCanvas;

// ---------------------------------------------------------------------------
// Top bar
// ---------------------------------------------------------------------------

pub fn view_topbar<'a>(app: &'a App, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;

    let title = iced::widget::text("Wuddle")
        .size(44)
        .font(LIFECRAFT)
        .color(colors.title)
        .line_height(1.0);

    let view_tabs = row![
        view_tab_button(app, Tab::Home, colors),
        view_tab_button(app, Tab::Mods, colors),
        view_tab_button(app, Tab::Addons, colors),
        view_tab_button(app, Tab::Tweaks, colors),
    ]
    .spacing(8);

    let action_tabs = row![
        view_tab_button(app, Tab::Options, colors),
        view_tab_button(app, Tab::Logs, colors),
        view_tab_button(app, Tab::About, colors),
    ]
    .spacing(8);

    // Busy spinner — always reserve space so the layout never shifts
    let spinner_el: Element<Message> = if app.is_busy() {
        let tick = app.spinner_tick;
        let primary = colors.primary;
        canvas(SpinnerCanvas { tick, color: primary })
            .width(26)
            .height(26)
            .into()
    } else {
        Space::new().width(26).height(26).into()
    };

    let left_section = row![title, spinner_el]
        .spacing(12)
        .align_y(iced::Alignment::Center);

    let mut right_items: Vec<Element<Message>> = Vec::new();

    if app.profiles.len() > 1 {
        let display_labels: Vec<String> = app.profiles.iter().map(|p| {
            let dupes = app.profiles.iter().filter(|q| q.name == p.name).count();
            if dupes > 1 { format!("{} ({})", p.name, p.id) } else { p.name.clone() }
        }).collect();

        let active_display = app.profiles.iter()
            .find(|p| p.id == app.active_profile_id)
            .map(|p| {
                let dupes = app.profiles.iter().filter(|q| q.name == p.name).count();
                if dupes > 1 { format!("{} ({})", p.name, p.id) } else { p.name.clone() }
            })
            .unwrap_or_else(|| "Default".to_string());

        let profile_picker: Element<Message> = iced::widget::pick_list(
            display_labels,
            Some(active_display),
            {
                let profiles = app.profiles.clone();
                move |display: String| {
                    let profile = profiles.iter().find(|p| {
                        let dupes = profiles.iter().filter(|q| q.name == p.name).count();
                        let label = if dupes > 1 { format!("{} ({})", p.name, p.id) } else { p.name.clone() };
                        label == display
                    });
                    Message::SwitchProfile(profile.map(|p| p.id.clone()).unwrap_or_default())
                }
            },
        )
        .text_size(13)
        .into();

        let divider = rule::vertical(1).style(move |_theme| theme::divider_style(&c));
        right_items.push(profile_picker);
        right_items.push(divider.into());
    }

    right_items.push(action_tabs.into());
    let right_section = row(right_items).spacing(10).align_y(iced::Alignment::Center);

    const BAR_H: f32 = 58.0;

    let sides = container(
        row![
            left_section,
            Space::new().width(Length::Fill),
            right_section,
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(BAR_H)
    .align_y(iced::Alignment::Center)
    .padding([0, 12]);

    let center = container(view_tabs)
        .width(Length::Fill)
        .height(BAR_H)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center)
        .padding([0, 0]);

    let bar = iced::widget::stack![sides, center].width(Length::Fill).height(BAR_H);

    container(bar)
        .width(Length::Fill)
        .style(move |_theme| theme::topbar_style(&c))
        .into()
}

// ---------------------------------------------------------------------------
// Tab button
// ---------------------------------------------------------------------------

pub fn view_tab_button<'a>(app: &'a App, tab: Tab, colors: &ThemeColors) -> Element<'a, Message> {
    let is_active = app.active_tab == tab;
    let c = *colors;

    let is_icon = matches!(tab, Tab::Options | Tab::Logs);
    let is_unicode_icon = tab == Tab::About;

    let content: Element<Message> = if is_icon {
        let icon_color = if is_active { c.primary_text } else { c.text };
        container(
            iced::widget::svg(tab_icon_svg(tab))
                .width(17)
                .height(17)
                .style(move |_t, _s| iced::widget::svg::Style { color: Some(icon_color) })
        )
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into()
    } else if is_unicode_icon {
        let icon_color = if is_active { c.primary_text } else { c.text };
        container(
            iced::widget::text(tab.icon_label()).size(17).color(icon_color).line_height(1.0),
        )
        .center_x(Length::Fill)
        .into()
    } else {
        let lbl = app.tab_label(tab);
        container(iced::widget::text(lbl).size(14))
            .width(Length::Fill)
            .center_x(Length::Fill)
            .into()
    };

    let btn = button(content)
        .on_press(Message::SetTab(tab))
        .padding([7, 0])
        .width(if is_icon || is_unicode_icon { Length::Fixed(32.0) } else { Length::Fixed(114.0) });

    let styled_btn: Element<Message> = if is_active {
        btn.style(move |_theme, _status| theme::tab_button_active_style(&c)).into()
    } else {
        btn.style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(&c),
            button::Status::Pressed => theme::tab_button_active_style(&c),
            _ => theme::tab_button_style(&c),
        })
        .into()
    };

    if is_icon || tab == Tab::About {
        iced::widget::tooltip(
            styled_btn,
            container(iced::widget::text(tab.tooltip()).size(13).color(c.text))
                .padding([3, 8])
                .style(move |_theme| theme::tooltip_style(&c)),
            iced::widget::tooltip::Position::Bottom,
        )
        .into()
    } else {
        styled_btn
    }
}

// ---------------------------------------------------------------------------
// Panel body
// ---------------------------------------------------------------------------

pub fn view_panel<'a>(app: &'a App, colors: &ThemeColors) -> Element<'a, Message> {
    let content: Element<Message> = match app.active_tab {
        Tab::Home    => crate::panels::home::view(app, colors),
        Tab::Mods    => crate::panels::projects::view(app, colors, "Mods"),
        Tab::Addons  => crate::panels::projects::view(app, colors, "Addons"),
        Tab::Tweaks  => crate::panels::tweaks::view(app, colors),
        Tab::Options => crate::panels::options::view(app, colors),
        Tab::Logs    => crate::panels::logs::view(app, colors),
        Tab::About   => crate::panels::about::view(app, colors),
    };

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([8, 8])
        .into()
}

// ---------------------------------------------------------------------------
// Footer
// ---------------------------------------------------------------------------

pub fn view_footer<'a>(app: &'a App, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;

    let hint: Element<Message> = if app.wow_dir.is_empty() {
        iced::widget::text("No WoW directory set. Go to Options to configure.")
            .size(12).color(colors.warn).into()
    } else {
        let active = app.profiles.iter()
            .find(|p| p.id == app.active_profile_id)
            .cloned()
            .unwrap_or_default();
        let (mode_label, tooltip_detail) = match active.launch_method.as_str() {
            "lutris" => {
                let target = if active.lutris_target.trim().is_empty() {
                    "(no target set)".to_string()
                } else {
                    active.lutris_target.clone()
                };
                ("Launch Mode: Lutris".to_string(), format!("Target: {}", target))
            }
            "wine" => {
                let cmd = if active.wine_command.trim().is_empty() { "wine".to_string() } else { active.wine_command.clone() };
                ("Launch Mode: Wine".to_string(), format!("Command: {}", cmd))
            }
            "custom" => {
                let cmd = if active.custom_command.trim().is_empty() { "(no command set)".to_string() } else { active.custom_command.clone() };
                ("Launch Mode: Custom".to_string(), format!("Command: {}", cmd))
            }
            _ => (
                "Launch Mode: Auto".to_string(),
                "Launches VanillaFixes.exe if present, otherwise Wow.exe".to_string(),
            ),
        };
        let tooltip_content = container(
            iced::widget::text(tooltip_detail).size(13).color(colors.text)
        )
        .padding([6, 10]);
        iced::widget::tooltip(
            iced::widget::text(mode_label).size(12).color(colors.muted),
            tooltip_content,
            iced::widget::tooltip::Position::Top,
        )
        .style(move |_t| theme::tooltip_style(&c))
        .into()
    };

    let play_btn = button(
        container(iced::widget::text("PLAY").size(16))
            .center_x(Length::Shrink),
    )
    .on_press(Message::LaunchGame)
    .padding([10, 36])
    .width(108)
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::play_button_hovered_style(&c),
        _ => theme::play_button_style(&c),
    });

    let bar = row![
        hint,
        Space::new().width(Length::Fill),
        play_btn,
    ]
    .spacing(12)
    .padding([10, 12])
    .align_y(iced::Alignment::Center);

    container(bar)
        .width(Length::Fill)
        .style(move |_theme| theme::footer_style(&c))
        .into()
}

// ---------------------------------------------------------------------------
// SVG icon helpers
// ---------------------------------------------------------------------------

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

/// Build an SVG handle for a forge icon (GitHub, GitLab, Codeberg, Gitea/Forgejo).
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

    let svg_owned: String;
    let resolved_svg = if svg.is_empty() {
        if forge_url.contains("codeberg") {
            svg_owned = concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">"#,
                r#"<path d="M11.999.747A11.974 11.974 0 000 12.75c0 2.254.635 4.465 1.833 6.376L11.837 "#,
                r#"6.19c.072-.092.251-.092.323 0l4.178 5.402h-2.992l.065.239h3.113l.882 1.138h-3.674"#,
                r#"l.103.374h3.86l.777 1.003h-4.358l.135.483h4.593l.695.894h-5.038l.165.589h5.326"#,
                r#"l.609.785h-5.717l.182.65h6.038l.562.727h-6.397l.183.65h6.717A12.003 12.003 0 0024"#,
                r#" 12.75 11.977 11.977 0 0011.999.747zm3.654 19.104.182.65h5.326c.173-.204.353-.433"#,
                r#".513-.65zm.385 1.377.18.65h3.563c.233-.198.485-.428.712-.65zm.383 1.377.182.648h"#,
                r#"1.203c.356-.204.685-.412 1.042-.648z"/>"#,
                r#"</svg>"#,
            ).to_string();
        } else {
            svg_owned = concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" "#,
                r#"stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">"#,
                r#"<path d="M5 9h14v7a3 3 0 0 1-3 3H8a3 3 0 0 1-3-3V9z"/>"#,
                r#"<path d="M5 9V7a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v2"/>"#,
                r#"<path d="M19 11.5h1a2 2 0 0 1 0 4h-1"/>"#,
                r#"</svg>"#,
            ).to_string();
        }
        svg_owned.as_str()
    } else {
        svg
    };

    iced::widget::svg::Handle::from_memory(resolved_svg.as_bytes().to_vec())
}
