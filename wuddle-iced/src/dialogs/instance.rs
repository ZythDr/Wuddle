//! InstanceSettings dialog — create or edit a WoW instance profile.
//!
//! Needs `active_profile_id` from App to decide whether the "Remove" button
//! is enabled (active profile cannot be removed).

use iced::widget::{button, column, container, row, text, Space};
use iced::{Element, Length};
use crate::{Message, InstanceField, theme};
use crate::components::helpers::{close_button, tip};
use theme::ThemeColors;

#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    is_new: bool,
    profile_id: &'a str,
    name: &'a str,
    wow_dir: &'a str,
    launch_method: &'a str,
    like_turtles: bool,
    clear_wdb: bool,
    lutris_target: &'a str,
    wine_command: &'a str,
    wine_args: &'a str,
    custom_command: &'a str,
    custom_args: &'a str,
    // From App state:
    active_profile_id: &str,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let title_text = if is_new { "Add Instance" } else { "Instance Settings" };
    let can_remove = !is_new;
    let is_active_profile = profile_id == active_profile_id;
    let remove_id = profile_id.to_string();

    let method_buttons: Vec<Element<Message>> = [
        ("Auto", "auto"),
        ("Lutris", "lutris"),
        ("Wine", "wine"),
        ("Custom", "custom"),
    ]
    .iter()
    .map(|&(label, m)| {
        let c2 = c;
        let is_active = launch_method == m;
        let m_str = String::from(m);
        let btn = button(text(label).size(12))
            .on_press(Message::UpdateInstanceField(InstanceField::LaunchMethod(m_str)))
            .padding([4, 10]);
        if is_active {
            btn.style(move |_t, _s| theme::tab_button_active_style(&c2)).into()
        } else {
            btn.style(move |_t, s| match s {
                button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                _ => theme::tab_button_style(&c2),
            })
            .into()
        }
    })
    .collect();

    let launch_fields: Element<Message> = match launch_method {
        "lutris" => column![
            text("Lutris target").size(13).color(colors.text),
            iced::widget::text_input("lutris:rungameid/2", lutris_target)
                .on_input(|s| Message::UpdateInstanceField(InstanceField::LutrisTarget(s)))
                .padding([8, 12]),
            text("Example: lutris:rungameid/2").size(11).color(colors.muted),
        ]
        .spacing(4)
        .into(),
        "wine" => column![
            text("Wine command").size(13).color(colors.text),
            iced::widget::text_input("wine", wine_command)
                .on_input(|s| Message::UpdateInstanceField(InstanceField::WineCommand(s)))
                .padding([8, 12]),
            text("Wine arguments").size(13).color(colors.text),
            iced::widget::text_input("--some-arg value", wine_args)
                .on_input(|s| Message::UpdateInstanceField(InstanceField::WineArgs(s)))
                .padding([8, 12]),
        ]
        .spacing(4)
        .into(),
        "custom" => column![
            text("Custom command").size(13).color(colors.text),
            iced::widget::text_input("command", custom_command)
                .on_input(|s| Message::UpdateInstanceField(InstanceField::CustomCommand(s)))
                .padding([8, 12]),
            text("Custom arguments").size(13).color(colors.text),
            iced::widget::text_input("--flag value", custom_args)
                .on_input(|s| Message::UpdateInstanceField(InstanceField::CustomArgs(s)))
                .padding([8, 12]),
            text("Tip: use {exe} in args to inject the detected game executable path.")
                .size(11)
                .color(colors.muted),
        ]
        .spacing(4)
        .into(),
        _ => text("Auto: launches VanillaFixes.exe if present, otherwise Wow.exe")
            .size(12)
            .color(colors.muted)
            .into(),
    };

    let footer: Element<Message> = {
        let mut footer_items: Vec<Element<Message>> = Vec::new();

        if can_remove {
            let c2 = c;
            let remove_el: Element<Message> = if is_active_profile {
                let dimmed_btn = button(text("Remove").size(13))
                    .padding([6, 14])
                    .style(move |_theme, _status| button::Style {
                        background: None,
                        text_color: iced::Color::from_rgba(1.0, 0.4, 0.4, 0.35),
                        border: iced::Border {
                            color: iced::Color::from_rgba(1.0, 0.4, 0.4, 0.25),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        shadow: iced::Shadow::default(),
                        snap: true,
                    });
                iced::widget::tooltip(
                    dimmed_btn,
                    container(
                        text("Cannot remove the active instance")
                            .size(13)
                            .color(c2.text),
                    )
                    .padding([4, 8])
                    .style(move |_theme| theme::tooltip_style(&c2)),
                    iced::widget::tooltip::Position::Top,
                )
                .into()
            } else {
                let rm_btn = button(text("Remove").size(13).color(c.bad))
                    .on_press(Message::RemoveProfile(remove_id))
                    .padding([6, 14])
                    .style(move |_theme, _status| {
                        let mut s = theme::tab_button_style(&c2);
                        s.border.color = c2.bad;
                        s
                    });
                tip(
                    rm_btn,
                    "Delete this instance profile",
                    iced::widget::tooltip::Position::Top,
                    colors,
                )
            };
            footer_items.push(remove_el);
        }

        footer_items.push(Space::new().width(Length::Fill).into());
        footer_items.push(
            button(text("Cancel").size(13))
                .on_press(Message::CloseDialog)
                .padding([6, 14])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c),
                    _ => theme::tab_button_style(&c),
                })
                .into(),
        );
        footer_items.push(tip(
            button(text("Save").size(13))
                .on_press(Message::SaveInstanceSettings)
                .padding([6, 14])
                .style(move |_theme, _status| theme::tab_button_active_style(&c)),
            "Save instance settings",
            iced::widget::tooltip::Position::Top,
            colors,
        ));

        row(footer_items).spacing(8).into()
    };

    column![
        row![
            text(title_text).size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(&c),
        ]
        .align_y(iced::Alignment::Center),
        text("Configure name, game path, and launch behavior for this instance.")
            .size(12)
            .color(colors.muted),
        text("Instance name").size(13).color(colors.text),
        iced::widget::text_input("My WoW Install", name)
            .on_input(|s| Message::UpdateInstanceField(InstanceField::Name(s)))
            .padding([8, 12]),
        iced::widget::checkbox(like_turtles)
            .label("I like turtles!")
            .on_toggle(|b| Message::UpdateInstanceField(InstanceField::LikeTurtles(b))),
        text("WoW directory").size(13).color(colors.text),
        row![
            iced::widget::text_input("/path/to/WoW", wow_dir)
                .on_input(|s| Message::UpdateInstanceField(InstanceField::WowDir(s)))
                .width(Length::Fill)
                .padding([8, 12]),
            {
                let c2 = c;
                tip(
                    button(text("Browse").size(12))
                        .on_press(Message::PickWowDirectory)
                        .padding([8, 12])
                        .style(move |_t, s| match s {
                            button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                            _ => theme::tab_button_style(&c2),
                        }),
                    "Pick the WoW installation folder",
                    iced::widget::tooltip::Position::Top,
                    colors,
                )
            },
        ]
        .spacing(6),
        iced::widget::checkbox(clear_wdb)
            .label("Auto-clear WDB cache on launch")
            .on_toggle(|b| Message::UpdateInstanceField(InstanceField::ClearWdb(b))),
        text("Launch method").size(13).color(colors.text),
        row(method_buttons).spacing(4),
        launch_fields,
        Space::new().height(4),
        footer,
    ]
    .spacing(8)
    .into()
}
