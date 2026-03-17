use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input, Space};
use iced::{Element, Length};

use crate::theme::{self, ThemeColors, WuddleTheme};
use crate::{App, Dialog, Message};

pub fn view<'a>(app: &'a App, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;

    // --- Instances section ---
    let instances_head = row![
        column![
            text("Instances").size(18).color(colors.title),
            text("Each instance has its own tracked mod/addon list. Click a card to edit details.")
                .size(12)
                .color(colors.muted),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
        {
            let c2 = c;
            button(text("+ Add Instance").size(13))
                .on_press(Message::OpenDialog(Dialog::InstanceSettings {
                    is_new: true,
                    name: String::new(),
                    wow_dir: String::new(),
                    launch_method: String::from("auto"),
                    like_turtles: true,
                    clear_wdb: false,
                    lutris_target: String::new(),
                    wine_command: String::from("wine"),
                    wine_args: String::new(),
                    custom_command: String::new(),
                    custom_args: String::new(),
                }))
                .padding([6, 12])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => theme::tab_button_style(&c2),
                })
        },
    ]
    .align_y(iced::Alignment::Center);

    let profile_cards: Vec<Element<Message>> = app.profiles.iter().map(|p| {
        let c2 = c;
        let dir_display = if p.wow_dir.is_empty() { "No directory set" } else { &p.wow_dir };
        let is_active = p.id == app.active_profile_id;
        let active_label = if is_active { " (active)" } else { "" };
        button(
            column![
                text(format!("{}{}", p.name, active_label)).size(14).color(colors.text),
                text(dir_display).size(12).color(colors.muted),
                text("Click to edit").size(11).color(colors.muted),
            ]
            .spacing(4),
        )
        .on_press(Message::OpenDialog(Dialog::InstanceSettings {
            is_new: false,
            name: p.name.clone(),
            wow_dir: p.wow_dir.clone(),
            launch_method: p.launch_method.clone(),
            like_turtles: p.like_turtles,
            clear_wdb: p.clear_wdb,
            lutris_target: p.lutris_target.clone(),
            wine_command: p.wine_command.clone(),
            wine_args: p.wine_args.clone(),
            custom_command: p.custom_command.clone(),
            custom_args: p.custom_args.clone(),
        }))
        .padding(12)
        .width(260)
        .style(move |_theme, status| {
            let base = theme::card_style(&c2);
            match status {
                button::Status::Hovered => button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
                    text_color: c2.text,
                    border: base.border,
                    shadow: base.shadow,
                    snap: true,
                },
                _ => button::Style {
                    background: base.background,
                    text_color: c2.text,
                    border: base.border,
                    shadow: base.shadow,
                    snap: true,
                },
            }
        })
        .into()
    }).collect();

    let instances_section = settings_card(
        column![instances_head, row(profile_cards).spacing(12)].spacing(10),
        &c,
    );

    // --- Behavior section ---
    let behavior_section = settings_card(
        column![
            text("Behavior").size(18).color(colors.title),
            checkbox(app.opt_auto_check)
                .label("Automatically check for updates")
                .on_toggle(Message::ToggleAutoCheck),
            checkbox(app.opt_desktop_notify)
                .label("Desktop notifications for updates")
                .on_toggle(Message::ToggleDesktopNotify),
            checkbox(app.opt_symlinks)
                .label("Use symlink installs when possible")
                .on_toggle(Message::ToggleSymlinks),
        ]
        .spacing(8),
        &c,
    );

    // --- Time and display section ---
    let theme_buttons: Vec<Element<Message>> = WuddleTheme::ALL
        .iter()
        .map(|&t| {
            let c2 = c;
            let btn = button(text(t.label()).size(13))
                .on_press(Message::SetTheme(t))
                .padding([6, 12]);
            if t == app.wuddle_theme {
                btn.style(move |_theme, _status| theme::theme_button_active_style(&c2))
                    .into()
            } else {
                btn.style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => theme::theme_button_style(&c2),
                })
                .into()
            }
        })
        .collect();

    let display_section = settings_card(
        column![
            text("Time and display").size(18).color(colors.title),
            checkbox(app.opt_clock12)
                .label("Use 12-hour time format (AM/PM)")
                .on_toggle(Message::ToggleClock12),
            checkbox(app.opt_friz_font)
                .label("Use Friz Quadrata font")
                .on_toggle(Message::ToggleFrizFont),
            Space::new().height(4),
            text("Theme").size(14).color(colors.text),
            row(theme_buttons).spacing(6),
        ]
        .spacing(8),
        &c,
    );

    // --- GitHub Authentication section ---
    let token_status = if wuddle_engine::github_token().is_some() {
        "Token active (authenticated)"
    } else if app.github_token_input.is_empty() {
        "No token"
    } else {
        "Token entered (not yet saved)"
    };

    let github_section = settings_card(
        column![
            row![
                text("GitHub Authentication").size(18).color(colors.title),
                Space::new().width(Length::Fill),
                {
                    let c2 = c;
                    button(text("GitHub Tokens").size(13))
                        .on_press(Message::OpenUrl("https://github.com/settings/tokens".to_string()))
                        .padding([6, 12])
                        .style(move |_theme, status| match status {
                            button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                            _ => theme::tab_button_style(&c2),
                        })
                },
            ]
            .align_y(iced::Alignment::Center),
            text("Optional: add a GitHub token to avoid anonymous API rate limits.")
                .size(12)
                .color(colors.muted),
            text("Recommended: create a classic token with no scopes/permissions selected, and set a custom expiration of 1 year.")
                .size(12)
                .color(colors.muted),
            row![
                text_input("ghp_...", &app.github_token_input)
                    .on_input(Message::SetGithubTokenInput)
                    .width(Length::Fill)
                    .padding([8, 12]),
                {
                    let c2 = c;
                    button(text("Save token").size(13))
                        .on_press(Message::SaveGithubToken)
                        .padding([6, 12])
                        .style(move |_theme, _status| theme::tab_button_active_style(&c2))
                },
                {
                    let c2 = c;
                    button(text("Forget").size(13).color(c.bad))
                        .on_press(Message::ForgetGithubToken)
                        .padding([6, 12])
                        .style(move |_theme, _status| {
                            let mut s = theme::tab_button_style(&c2);
                            s.border.color = c2.bad;
                            s
                        })
                },
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text(token_status).size(12).color(colors.muted),
        ]
        .spacing(8),
        &c,
    );

    scrollable(
        column![
            instances_section,
            behavior_section,
            display_section,
            github_section,
        ]
        .spacing(8)
        .width(Length::Fill),
    )
    .height(Length::Fill)
    .into()
}

fn settings_card<'a>(
    content: impl Into<Element<'a, Message>>,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    container(container(content).padding(16))
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(&c))
        .into()
}
