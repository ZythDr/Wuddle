use iced::widget::{
    button, checkbox, column, container, grid, row, scrollable, stack, text, tooltip, Space,
};
use iced::{Element, Length};

use crate::components::text_input_context::context_text_input;
use crate::settings::{self, UiScaleMode};
use crate::theme::{self, ThemeColors, WuddleTheme};
use crate::{App, Dialog, Message};

pub fn view<'a>(app: &'a App, colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;

    // --- Profiles section ---
    let instances_head = row![
        column![
            text("Profiles").size(18).color(colors.title),
            text("Each profile has its own tracked mod/addon list. Click a card to switch profiles, or use its cogwheel to edit it.")
                .size(12)
                .color(colors.muted),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
        tip(
            {
                let c2 = c;
                button(text("+ Add Profile").size(13))
                    .on_press(Message::OpenDialog(Dialog::InstanceSettings {
                        is_new: true,
                        profile_id: String::new(),
                        name: String::new(),
                        wow_dir: String::new(),
                        launch_method: String::from("auto"),
                        show_mods_tab: true,
                        show_addons_tab: true,
                        show_patches_tab: true,
                        show_tweaks_tab: true,
                        clear_wdb: false,
                        auto_login_enabled: false,
                        lutris_target: String::new(),
                        wine_command: String::from("wine"),
                        wine_args: String::new(),
                        custom_command: String::new(),
                        custom_args: String::new(),
                    }))
                    .padding([6, 12])
                    .style(move |_theme, status| match status {
                        button::Status::Hovered => theme::tab_button_hovered_style(c2),
                        _ => theme::tab_button_style(c2),
                    })
            },
            "Create a new WoW profile",
            tooltip::Position::Bottom,
            colors,
        ),
    ]
    .align_y(iced::Alignment::Center);

    let profile_cards: Vec<Element<Message>> = app
        .profiles
        .iter()
        .map(|p| {
            let c2 = c;
            let is_active = p.id == app.active_profile_id;
            let edit_dialog = Dialog::InstanceSettings {
                is_new: false,
                profile_id: p.id.clone(),
                name: p.name.clone(),
                wow_dir: settings::wow_path_display(&p.wow_dir, p.auto_launch_exe.as_deref()),
                launch_method: p.launch_method.clone(),
                show_mods_tab: p.show_mods_tab,
                show_addons_tab: p.show_addons_tab,
                show_patches_tab: p.show_patches_tab,
                show_tweaks_tab: p.show_tweaks_tab,
                clear_wdb: p.clear_wdb,
                auto_login_enabled: p.auto_login_enabled,
                lutris_target: p.lutris_target.clone(),
                wine_command: p.wine_command.clone(),
                wine_args: p.wine_args.clone(),
                custom_command: p.custom_command.clone(),
                custom_args: p.custom_args.clone(),
            };
            let switch_card = button(
                container(text(&p.name).size(14).color(if is_active {
                    colors.title
                } else {
                    colors.muted
                }))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
            )
            .on_press(Message::SwitchProfile(p.id.clone()))
            .padding([0, 12])
            .width(Length::Fill)
            .height(40)
            .style(move |_theme, status| theme::choice_button_style(c2, is_active, status));

            let edit_button = tip(
                button(crate::cogwheel_button_icon(
                    26.0,
                    crate::dim_icon_color(c2.muted),
                    c2.title,
                ))
                .on_press(Message::OpenDialog(edit_dialog))
                .width(26)
                .height(26)
                .padding(0)
                .style(move |_theme, status| button::Style {
                    background: None,
                    text_color: match status {
                        button::Status::Hovered => c2.title,
                        _ => c2.muted,
                    },
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: true,
                }),
                "Edit this profile",
                tooltip::Position::Top,
                colors,
            );

            stack![
                switch_card,
                container(row![Space::new().width(Length::Fill), edit_button])
                    .width(Length::Fill)
                    .height(40)
                    .align_y(iced::Alignment::Center)
                    .padding([0, 6]),
            ]
            .width(Length::Fill)
            .height(40)
            .into()
        })
        .collect();

    let instances_section = settings_card(
        column![
            instances_head,
            grid::Grid::with_children(profile_cards)
                .fluid(240)
                .height(Length::Shrink)
                .spacing(10),
        ]
        .spacing(10),
        c,
    );

    // --- Behavior section ---
    let interval_value = app.auto_check_minutes.to_string();
    let interval_input =
        context_text_input(app, colors, "auto-check-interval", "60", &interval_value)
            .width(60)
            .padding([4, 8]);
    let interval_input = if app.opt_auto_check {
        interval_input.on_input(Message::SetAutoCheckMinutes)
    } else {
        interval_input
    };

    let github_token_active = wuddle_engine::github_token().is_some();
    let api_conservation_available = app.opt_auto_check && !github_token_active;
    let conserve_api_toggle = checkbox(app.opt_conserve_github_api).label("Conserve GitHub API");
    let conserve_api_toggle = if api_conservation_available {
        conserve_api_toggle.on_toggle(Message::ToggleConserveGithubApi)
    } else {
        conserve_api_toggle
    };
    let conserve_api_tooltip = if !app.opt_auto_check {
        "Unavailable because automatic update checks are disabled.\n\nReduces anonymous GitHub API usage.\nInfrequently updated projects are checked only when scheduled, even during Check for updates.\nIndividual project actions are unaffected."
    } else if github_token_active {
        "Unavailable because a GitHub token is active.\n\nAuthenticated requests have a much larger API allowance, so Wuddle does not throttle infrequently updated projects."
    } else {
        "Reduces anonymous GitHub API usage.\nInfrequently updated projects are checked only when scheduled, even during Check for updates.\n\nIndividual project actions are unaffected."
    };
    let child_padding = iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 28.0,
    };

    let behavior_section = settings_card_fill(
        column![
            text("Behavior").size(18).color(colors.title),
            checkbox(app.opt_auto_check)
                .label("Automatically check for updates")
                .on_toggle(Message::ToggleAutoCheck),
            container(
                row![
                    text("Interval (minutes):").size(12).color(
                        if app.opt_auto_check { colors.text } else { colors.muted }
                    ),
                    interval_input,
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
            )
            .padding(child_padding),
            container(tip(
                conserve_api_toggle,
                conserve_api_tooltip,
                tooltip::Position::Top,
                colors,
            ))
            .padding(child_padding),
            checkbox(app.opt_desktop_notify)
                .label("Desktop notifications for updates")
                .on_toggle(Message::ToggleDesktopNotify),
            tip(
                checkbox(app.remember_window_geometry)
                    .label("Remember window size and position")
                    .on_toggle(Message::ToggleRememberWindowGeometry),
                "Restores Wuddle's previous window size and position after restarting.\n\nOn Wayland, the desktop compositor may choose the window position.",
                tooltip::Position::Top,
                colors,
            ),
            tip(
                checkbox(app.opt_symlinks)
                    .label("Use symlinks for extracted addon folders")
                    .on_toggle(Message::ToggleSymlinks),
                "When possible, Wuddle links addon folders extracted from release archives instead of copying them.\n\nGit-based addons follow GAM-compatible layout rules. DLLs and individual files are copied.",
                tooltip::Position::Top,
                colors,
            ),
            checkbox(app.opt_xattr)
                .label("Set xattr file comments")
                .on_toggle(Message::ToggleXattr),
        ]
        .spacing(8),
        c,
    );

    // --- Time and display section ---
    let theme_buttons: Vec<Element<Message>> = WuddleTheme::ALL
        .iter()
        .map(|&t| {
            let c2 = c;
            let is_active = t == app.wuddle_theme;
            let (top_hex, bot_hex): (u32, u32) = match t {
                WuddleTheme::Cata => (0xd18a38, 0x9d581f),
                WuddleTheme::Obsidian => (0x4f8bc4, 0x223a56),
                WuddleTheme::Emerald => (0x4aa475, 0x1f4d39),
                WuddleTheme::Ashen => (0xcb6a62, 0x5d2d2f),
                WuddleTheme::WowUi => (0xd63d2f, 0x7a1717),
            };
            let swatch = container(Space::new().width(0).height(0))
                .width(34)
                .height(34)
                .style(move |_| {
                    let fh = |h: u32| {
                        iced::Color::from_rgb(
                            ((h >> 16) & 0xFF) as f32 / 255.0,
                            ((h >> 8) & 0xFF) as f32 / 255.0,
                            (h & 0xFF) as f32 / 255.0,
                        )
                    };
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
                container(
                    text(t.label())
                        .size(theme::TOOLTIP_TEXT_SIZE)
                        .color(c2.text),
                )
                .padding([3, 8])
                .style(move |_| theme::tooltip_style(c2)),
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
                            theme::tab_button_active_style(c2)
                        } else {
                            theme::tab_button_style(c2)
                        }
                    }),
                container(
                    text(mode.tooltip())
                        .size(theme::TOOLTIP_TEXT_SIZE)
                        .color(c2.text),
                )
                .padding([3, 8])
                .style(move |_| theme::tooltip_style(c2)),
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
        c,
    );

    // --- GitHub Authentication section ---
    let (token_status, token_status_color) = if app.github_token_storage_error.is_some() {
        ("Saved token unavailable", colors.bad)
    } else if !app.github_token_input.is_empty() {
        ("Token entered — click Save to activate", colors.warn)
    } else {
        match app.github_token_status {
            crate::app::GitHubTokenStatus::None => ("No token set", colors.muted),
            crate::app::GitHubTokenStatus::StoredUnverified => {
                ("Token stored — verification pending", colors.warn)
            }
            crate::app::GitHubTokenStatus::EnvironmentUnverified => {
                ("Environment token — verification pending", colors.warn)
            }
            crate::app::GitHubTokenStatus::Validated => {
                ("Token active (validated by GitHub)", colors.good)
            }
            crate::app::GitHubTokenStatus::Invalid => {
                ("Saved token rejected by GitHub", colors.bad)
            }
            crate::app::GitHubTokenStatus::OfflineUnverified => (
                "Token stored — GitHub verification unavailable",
                colors.warn,
            ),
        }
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
                                button::Status::Hovered => theme::tab_button_hovered_style(c2),
                                _ => theme::tab_button_style(c2),
                            }),
                        container(text("Opens GitHub in your browser so you can create or manage a token.").size(theme::TOOLTIP_TEXT_SIZE).color(c.text))
                            .padding([3, 8])
                            .style(move |_theme| theme::tooltip_style(c2)),
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
                    let placeholder = if app.github_token_status.is_configured() {
                        "Saved securely — enter a replacement"
                    } else {
                        "ghp_..."
                    };
                    stack![
                        context_text_input(
                            app,
                            colors,
                            "github-token",
                            placeholder,
                            &app.github_token_input,
                        )
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
                            .style(move |_theme, _status| theme::tab_button_active_style(c2))
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
                                let mut s = theme::tab_button_style(c2);
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
            {
                let error_detail: Element<Message> = if let Some(error) = app.github_token_storage_error.as_deref() {
                    text(error).size(12).color(colors.bad).into()
                } else {
                    Space::new().height(0).into()
                };
                error_detail
            },
        ]
        .spacing(8),
        c,
    );

    // --- Backup and Restore section ---
    let backup_section = settings_card(
        row![
            column![
                text("Backup and Restore").size(18).color(colors.title),
                text("Save or restore Wuddle's profiles, preferences, and tracked project data.")
                    .size(12)
                    .color(colors.muted),
                text("Installed game files and credentials are not copied into the backup.")
                    .size(12)
                    .color(colors.muted),
            ]
            .spacing(4),
            Space::new().width(Length::Fill),
            tip(
                {
                    let c2 = c;
                    button(text("Backup and Restore...").size(13))
                        .on_press(Message::OpenBackupRestore)
                        .padding([6, 12])
                        .style(move |_theme, status| match status {
                            button::Status::Hovered => theme::tab_button_hovered_style(c2),
                            _ => theme::tab_button_style(c2),
                        })
                },
                "Create a complete Wuddle settings backup, restore a backup ZIP, or import an old Wuddle data folder.\n\nGitHub tokens and auto-login passwords remain in the operating system credential vault.",
                tooltip::Position::Top,
                colors,
            ),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
        c,
    );

    scrollable(
        column![
            instances_section,
            row![behavior_section, display_section,]
                .spacing(8)
                .height(280),
            github_section,
            backup_section,
        ]
        .spacing(8)
        .width(Length::Fill),
    )
    .height(Length::Fill)
    .direction(theme::vscroll())
    .style(move |t, s| theme::scrollable_style(c)(t, s))
    .into()
}

/// Wrap any element in a tooltip with consistent styling.
fn tip<'a>(
    content: impl Into<Element<'a, Message>>,
    tip_text: &str,
    pos: tooltip::Position,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;
    let tip_str = String::from(tip_text);
    tooltip(
        content,
        container(text(tip_str).size(theme::TOOLTIP_TEXT_SIZE).color(c.text))
            .padding([3, 8])
            .style(move |_theme| theme::tooltip_style(c)),
        pos,
    )
    .gap(4.0)
    .into()
}

fn settings_card<'a>(
    content: impl Into<Element<'a, Message>>,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;
    container(container(content).padding(16))
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(c))
        .into()
}

fn settings_card_fill<'a>(
    content: impl Into<Element<'a, Message>>,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;
    container(container(content).padding(16))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| theme::card_style(c))
        .into()
}
