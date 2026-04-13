//! DllCountWarning dialog — confirms how to proceed when a mod's DLL count changes between releases.

use iced::widget::{button, column, row, text, Space};
use iced::{Element, Length};
use crate::{Message, theme};
use crate::components::helpers::close_button;
use theme::ThemeColors;

pub fn view<'a>(
    repo_id: i64,
    repo_name: &'a str,
    previous_count: usize,
    new_count: usize,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let fewer = new_count < previous_count;

    let description = if fewer {
        format!(
            "This release has {} DLL file{} but you currently have {} installed. \
             A clean update will remove {} existing DLL{}.",
            new_count,
            if new_count == 1 { "" } else { "s" },
            previous_count,
            previous_count - new_count,
            if previous_count - new_count == 1 { "" } else { "s" },
        )
    } else {
        format!(
            "This release has {} DLL file{} but you currently have {} installed.",
            new_count,
            if new_count == 1 { "" } else { "s" },
            previous_count,
        )
    };

    column![
        row![
            text("DLL File Count Changed").size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(&c),
        ]
        .align_y(iced::Alignment::Center),
        text(format!("\"{}\"", repo_name)).size(13).color(colors.text),
        text(description).size(13).color(colors.warn),
        text("How would you like to proceed?").size(13).color(colors.text),
        row![
            {
                let c2 = c;
                button(
                    column![
                        text("Merge Update").size(13),
                        text("Keep existing DLLs, only overwrite matching files")
                            .size(11)
                            .color(c2.muted),
                    ]
                    .spacing(2),
                )
                .on_press(Message::DllCountWarningChoice { repo_id, merge: true })
                .padding([10, 16])
                .width(Length::FillPortion(1))
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => theme::tab_button_style(&c2),
                })
            },
            {
                let c2 = c;
                button(
                    column![
                        text("Clean Update").size(13),
                        text("Remove old DLLs first, then install new release")
                            .size(11)
                            .color(c2.muted),
                    ]
                    .spacing(2),
                )
                .on_press(Message::DllCountWarningChoice { repo_id, merge: false })
                .padding([10, 16])
                .width(Length::FillPortion(1))
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => theme::tab_button_style(&c2),
                })
            },
        ]
        .spacing(8),
    ]
    .spacing(12)
    .into()
}
