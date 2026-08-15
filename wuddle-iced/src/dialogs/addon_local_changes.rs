//! Confirmation shown when an addon Git update would replace local edits.

use crate::components::helpers::close_button;
use crate::theme::{self, ThemeColors};
use crate::{AddonLocalChangesEntry, Message};
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Element, Length};

pub fn view<'a>(repos: &'a [AddonLocalChangesEntry], colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;
    let repo_ids = repos.iter().map(|repo| repo.repo_id).collect::<Vec<_>>();
    let plural = repos.len() != 1;
    let rows = repos.iter().map(|repo| {
        container(
            column![
                text(&repo.repo_name).size(15).color(c.title),
                text(&repo.reason).size(13).color(c.muted),
            ]
            .spacing(3),
        )
        .padding([8, 10])
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(c))
        .into()
    });
    let list_height = (repos.len().max(1) as f32 * 62.0).min(248.0);

    column![
        row![
            text("Local Changes Detected").size(18).color(c.title),
            Space::new().width(Length::Fill),
            close_button(c),
        ]
        .align_y(iced::Alignment::Center),
        text(if plural {
            "Some addons contain files that differ from their checked-out Git revisions."
        } else {
            "This addon contains files that differ from its checked-out Git revision."
        })
        .size(14)
        .color(c.text),
        scrollable(column(rows).spacing(6))
            .height(Length::Fixed(list_height))
            .direction(theme::vscroll_overlay())
            .style(move |theme, status| theme::scrollable_style(c)(theme, status)),
        text(
            "If you did not make these changes yourself, another addon manager, an older installation, or another program may have changed the folder. Rescanning repairs Wuddle's tracking, but does not erase real file differences.\n\nOverwrite & Update will permanently replace the local changes, but only after the new version has been prepared successfully in staging.\n\nIgnore Updates keeps the current files and excludes the selected addon from future update checks.",
        )
        .size(13)
        .color(c.warn),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(14))
                .on_press(Message::CloseDialog)
                .padding([8, 18])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(c),
                    _ => theme::tab_button_style(c),
                }),
            button(text("Ignore Updates").size(14))
                .on_press(Message::IgnoreAddonLocalChangesUpdates(repo_ids.clone()))
                .padding([8, 18])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(c),
                    _ => theme::tab_button_style(c),
                }),
            button(text("Overwrite & Update").size(14))
                .on_press(Message::ConfirmAddonLocalChangesUpdate(repo_ids))
                .padding([8, 18])
                .style(move |_theme, status| theme::btn_danger_style(c, status)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(14)
    .into()
}
