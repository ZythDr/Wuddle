//! ModsWarning dialog - reminds users that DLL client modifications must be server-approved.

use iced::widget::{button, checkbox, column, container, row, text, Space};
use iced::{Element, Length};

use crate::components::helpers::close_button;
use crate::{theme, Message};
use theme::ThemeColors;

pub fn view<'a>(do_not_show_again: bool, colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;

    column![
        row![
            text("DLL Client Modification Warning")
                .size(18)
                .color(colors.title),
            Space::new().width(Length::Fill),
            close_button(c),
        ]
        .align_y(iced::Alignment::Center),
        container(
            column![
                text("The Mods tab is for DLL-based client modifications.")
                    .size(16)
                    .color(colors.text),
                text(
                    "Only use DLL client tweaks or mods on servers where they are explicitly allowed. \
                     Some private servers may consider DLL injection, client patching, or memory modification \
                     against their rules."
                )
                .size(15)
                .color(colors.text_soft),
                text(
                    "Wuddle can help install and update these projects, but it cannot determine whether a \
                     specific server permits them."
                )
                .size(15)
                .color(colors.warn),
            ]
            .spacing(8),
        )
        .padding(12)
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(c)),
        row![
            checkbox(do_not_show_again)
                .label("Do not show again for this profile")
                .on_toggle(Message::ToggleModsWarningDoNotShow)
                .text_size(15),
            Space::new().width(Length::Fill),
            button(text("I understand").size(13))
                .on_press(Message::AcceptModsWarning)
                .padding([8, 18])
                .style(move |_theme, _status| theme::tab_button_active_style(c)),
        ]
        .align_y(iced::Alignment::Center),
    ]
    .spacing(12)
    .into()
}
