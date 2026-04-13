//! Simple warning confirmation dialogs:
//! - `super_wow_warning` — anti-virus false-positive warning for SuperWoW
//! - `addon_conflict`    — conflict resolution for duplicate addon sources

use iced::widget::{button, column, row, text, Space};
use iced::{Element, Length};
use crate::{Message, theme};
use crate::components::helpers::close_button;
use theme::ThemeColors;

/// Anti-virus warning shown before installing SuperWoW.
pub fn super_wow_warning<'a>(
    url: &'a str,
    mode: &'a str,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    column![
        row![
            text("Anti-Virus Warning").size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(&c),
        ]
        .align_y(iced::Alignment::Center),
        text(
            "This client mod is known to trigger false-positives from anti-virus software.\n\
             Please add your WoW installation directory to your anti-virus' exclusion/whitelist.\n\n\
             If the WoW directory is not added to exclusions in your anti-virus software, \
             the files will be deleted immediately after being downloaded."
        )
        .size(15)
        .color(colors.warn),
        text(
            "If you understand and have whitelisted your WoW installation directory \
             in your anti-virus software, you may continue."
        )
        .size(15)
        .color(colors.text),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(13))
                .on_press(Message::CloseDialog)
                .padding([6, 12])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c),
                    _ => theme::tab_button_style(&c),
                }),
            button(text("I understand, install").size(13).color(colors.bad))
                .on_press(Message::InstallRepoOverride {
                    url: url.to_string(),
                    mode: mode.to_string(),
                })
                .padding([6, 12])
                .style(move |_theme, _status| {
                    let mut s = theme::tab_button_style(&c);
                    s.border.color = c.bad;
                    s
                }),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .into()
}

/// Shown when an addon repo conflicts with an already-installed addon from another source.
pub fn addon_conflict<'a>(
    url: &'a str,
    mode: &'a str,
    conflicts: &'a [wuddle_engine::AddonProbeConflict],
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;

    let conflict_rows: Vec<Element<Message>> = conflicts
        .iter()
        .map(|conflict| {
            text(format!("- {}", conflict.addon_name))
                .size(13)
                .color(colors.warn)
                .into()
        })
        .collect();

    column![
        row![
            text("Addon Conflict").size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(&c),
        ]
        .align_y(iced::Alignment::Center),
        text("The following addons are already installed from other sources:")
            .size(13)
            .color(colors.text),
        column(conflict_rows).spacing(4),
        text(
            "This repository provides the same addons. Would you like to overwrite \
             them and track them via this repository instead?"
        )
        .size(13)
        .color(colors.text),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(13))
                .on_press(Message::CloseDialog)
                .padding([6, 12])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c),
                    _ => theme::tab_button_style(&c),
                }),
            button(text("Overwrite").size(13).color(colors.bad))
                .on_press(Message::InstallRepoOverride {
                    url: url.to_string(),
                    mode: mode.to_string(),
                })
                .padding([6, 12])
                .style(move |_theme, _status| {
                    let mut s = theme::tab_button_style(&c);
                    s.border.color = c.bad;
                    s
                }),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .into()
}
