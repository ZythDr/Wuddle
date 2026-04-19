use iced::widget::{button, checkbox, column, container, row, scrollable, stack, text, text_input, tooltip, Space};
use iced::{Element, Length};

use crate::settings::{self, UiScaleMode};
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
        tip(
            {
                let c2 = c;
                button(text("+ Add Instance").size(13))
                    .on_press(Message::OpenDialog(Dialog::InstanceSettings {
                        is_new: true,
                        profile_id: String::new(),
                        name: String::new(),
                        wow_dir: String::new(),
                        launch_method: String::from("auto"),
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
            "Create a new WoW instance profile",
            tooltip::Position::Bottom,
            colors,
        ),
    ]
    .align_y(iced::Alignment::Center);

    let profile_cards: Vec<Element<Message>> = app.profiles.iter().map(|p| {
        let c2 = c;
        let display_path = settings::wow_path_display(&p.wow_dir, p.auto_launch_exe.as_deref());
        let dir_display = if display_path.is_empty() {
            "No directory set".to_string()
        } else {
            display_path
        };
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
            profile_id: p.id.clone(),
            name: p.name.clone(),
            wow_dir: settings::wow_path_display(&p.wow_dir, p.auto_launch_exe.as_deref()),
            launch_method: p.launch_method.clone(),
            clear_wdb: p.clear_wdb,
            lutris_target: p.lutris_target.clone(),
            wine_command: p.wine_command.clone(),
            wine_args: p.wine_args.clone(),
            custom_command: p.custom_command.clone(),
            custom_args: p.custom_args.clone(),
        }))
        .padding([10, 12])
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
    let behavior_section = settings_card_fill(
        column![
            text("Behavior").size(18).color(colors.title),
            checkbox(app.opt_auto_check)
                .label("Automatically check for updates")
                .on_toggle(Message::ToggleAutoCheck),
            row![
                text("Interval (minutes):").size(12).color(
                    if app.opt_auto_check { colors.text } else { colors.muted }
                ),
                text_input("60", &app.auto_check_minutes.to_string())
                    .on_input(Message::SetAutoCheckMinutes)
                    .width(60)
                    .padding([4, 8]),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            checkbox(app.opt_desktop_notify)
                .label("Desktop notifications for updates")
                .on_toggle(Message::ToggleDesktopNotify),
            checkbox(app.opt_symlinks)
                .label("Use symlink installs when possible")
                .on_toggle(Message::ToggleSymlinks),
            checkbox(app.opt_xattr)
                .label("Set xattr file comments")
                .on_toggle(Message::ToggleXattr),
        ]
        .spacing(8),
        &c,
    );

    // --- Time and display section ---
    let theme_buttons: Vec<Element<Message>> = WuddleTheme::ALL
        .iter()
        .map(|&t| {
            let c2 = c;
            let is_active = t == app.wuddle_theme;
            let (top_hex, bot_hex): (u32, u32) = match t {
                WuddleTheme::Cata     => (0xd18a38, 0x9d581f),
                WuddleTheme::Obsidian => (0x4f8bc4, 0x223a56),
                WuddleTheme::Emerald  => (0x4aa475, 0x1f4d39),
                WuddleTheme::Ashen    => (0xcb6a62, 0x5d2d2f),
                WuddleTheme::WowUi    => (0xd63d2f, 0x7a1717),
            };
            let swatch = container(Space::new().width(0).height(0))
                .width(34)
                .height(34)
                .style(move |_| {
                    let fh = |h: u32| iced::Color::from_rgb(
                        ((h >> 16) & 0xFF) as f32 / 255.0,
                        ((h >> 8)  & 0xFF) as f32 / 255.0,
                        (h         & 0xFF) as f32 / 255.0,
                    );
                    let grad = iced::Gradient::Linear(
                        iced::gradient::Linear::new(iced::Radians(std::f32::consts::PI))
                            .add_stop(0.0, fh(top_hex))
                            .add_stop(1.0, fh(bot_hex)),
                    );
                    container::Style {
                        background: Some(iced::Background::Gradient(grad)),
                        border: iced::Border {
                            color: if is_active {
                                iced::Color::from_rgba(1.0, 1.0, 1.0, 0.70)
                            } else {
                                iced::Color::from_rgba(1.0, 1.0, 1.0, 0.12)
                            },
                            width: if is_active { 2.0 } else { 1.0 },
                            radius: iced::border::Radius::new(0.0),
                        },
                        shadow: iced::Shadow::default(),
                        text_color: None,
                        snap: true,
                    }
                });
            tooltip(
                button(swatch)
                    .on_press(Message::SetTheme(t))
                    .padding(0)
                    .style(move |_, _| button::Style {
                        background: None,
                        text_color: c2.text,
                        border: iced::Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: true,
                    }),
                container(text(t.label()).size(13).color(c2.text))
                    .padding([3, 8])
                    .style(move |_| theme::tooltip_style(&c2)),
                tooltip::Position::Bottom,
            )
            .gap(4.0)
            .into()
        })
        .collect();

    // --- UI Scale buttons ---
    let scale_buttons: Vec<Element<Message>> = UiScaleMode::ALL
        .iter()
        .map(|&mode| {
            let c2 = c;
            let is_active = mode == app.ui_scale_mode;
            tooltip(
                button(text(mode.label()).size(12))
                    .on_press(Message::SetUiScaleMode(mode))
                    .padding([6, 12])
                    .style(move |_theme, _status| {
                        if is_active {
                            theme::tab_button_active_style(&c2)
                        } else {
                            theme::tab_button_style(&c2)
                        }
                    }),
                container(text(mode.tooltip()).size(13).color(c2.text))
                    .padding([3, 8])
                    .style(move |_| theme::tooltip_style(&c2)),
                tooltip::Position::Bottom,
            )
            .gap(4.0)
            .into()
        })
        .collect();

    let display_section = settings_card_fill(
        column![
            text("Time and display").size(18).color(colors.title),
            checkbox(app.opt_clock12)
                .label("Use 12-hour time format (AM/PM)")
                .on_toggle(Message::ToggleClock12),
            checkbox(app.opt_friz_font)
                .label("Use Friz Quadrata font")
                .on_toggle(Message::ToggleFrizFont),
            Space::new().height(4),
            text("UI Scale").size(14).color(colors.text),
            row(scale_buttons).spacing(6),
            Space::new().height(4),
            text("Theme").size(14).color(colors.text),
            row(theme_buttons).spacing(6),
        ]
        .spacing(8),
        &c,
    );

    // --- GitHub Authentication section ---
    let (token_status, token_status_color) = if wuddle_engine::github_token().is_some() {
        ("Token active (authenticated)", colors.good)
    } else if app.github_token_input.is_empty() {
        ("No token set", colors.muted)
    } else {
        ("Token entered — click Save to activate", colors.warn)
    };

    let github_section = settings_card(
        column![
            row![
                text("GitHub Authentication").size(18).color(colors.title),
                Space::new().width(Length::Fill),
                {
                    let c2 = c;
                    tooltip(
                        button(text("GitHub Tokens").size(13))
                            .on_press(Message::OpenUrl("https://github.com/settings/tokens".to_string()))
                            .padding([6, 12])
                            .style(move |_theme, status| match status {
                                button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                                _ => theme::tab_button_style(&c2),
                            }),
                        container(text("Opens GitHub in your browser so you can create or manage a token.").size(13).color(c.text))
                            .padding([3, 8])
                            .style(move |_theme| theme::tooltip_style(&c2)),
                        tooltip::Position::Bottom,
                    )
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
                {
                    let c2 = c;
                    let show_clear = !app.github_token_input.is_empty();
                    stack![
                        text_input("ghp_...", &app.github_token_input)
                            .on_input(Message::SetGithubTokenInput)
                            .width(Length::Fill)
                            .padding(iced::Padding { top: 8.0, right: if show_clear { 28.0 } else { 12.0 }, bottom: 8.0, left: 12.0 }),
                        {
                            let clear_el: Element<Message> = if show_clear {
                                button(text("\u{2715}").size(12).color(c2.muted))
                                    .on_press(Message::SetGithubTokenInput(String::new()))
                                    .padding([3, 7])
                                    .style(move |_t, _s| button::Style {
                                        background: None,
                                        text_color: c2.muted,
                                        border: iced::Border::default(),
                                        shadow: iced::Shadow::default(),
                                        snap: true,
                                    })
                                    .into()
                            } else {
                                Space::new().into()
                            };
                            container(clear_el)
                        }
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(iced::Alignment::End)
                        .align_y(iced::Alignment::Center)
                        .padding(iced::Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 0.0 }),
                    ]
                    .width(Length::Fill)
                },
                tip(
                    {
                        let c2 = c;
                        button(text("Save token").size(13))
                            .on_press(Message::SaveGithubToken)
                            .padding([6, 12])
                            .style(move |_theme, _status| theme::tab_button_active_style(&c2))
                    },
                    "Store this token for authenticated GitHub API access",
                    tooltip::Position::Top,
                    colors,
                ),
                tip(
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
                    "Remove the saved GitHub token",
                    tooltip::Position::Top,
                    colors,
                ),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text(token_status).size(12).color(token_status_color),
        ]
        .spacing(8),
        &c,
    );

    scrollable(
        column![
            instances_section,
            row![
                behavior_section,
                display_section,
            ]
            .spacing(8)
            .height(280),
            github_section,
        ]
        .spacing(8)
        .width(Length::Fill),
    )
    .height(Length::Fill)
    .direction(theme::vscroll())
    .style(move |t, s| theme::scrollable_style(&c)(t, s))
    .into()
}

/// Wrap any element in a tooltip with consistent styling.
fn tip<'a>(content: impl Into<Element<'a, Message>>, tip_text: &str, pos: tooltip::Position, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    let tip_str = String::from(tip_text);
    tooltip(
        content,
        container(text(tip_str).size(13).color(c.text))
            .padding([3, 8])
            .style(move |_theme| theme::tooltip_style(&c)),
        pos,
    )
    .gap(4.0)
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

fn settings_card_fill<'a>(
    content: impl Into<Element<'a, Message>>,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    container(container(content).padding(16))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| theme::card_style(&c))
        .into()
}
