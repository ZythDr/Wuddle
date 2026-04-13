//! RemoveRepo dialog — confirms removal of a tracked repository with optional file deletion.

use iced::widget::{button, checkbox, column, container, row, scrollable, text, Space};
use iced::{Element, Length};
use crate::{Message, theme};
use crate::components::helpers::{close_button, tip};
use theme::ThemeColors;

pub fn view<'a>(
    repo_id: i64,
    name: &'a str,
    remove_files: bool,
    files: &'a [(String, String)],
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let rf = remove_files;

    let file_rows: Vec<Element<Message>> = files.iter().map(|(path, kind)| {
        let icon = match kind.as_str() {
            "dll"   => "\u{2699}",  // ⚙
            "addon" => "\u{1f4c1}", // 📁
            _       => "\u{1f4c4}", // 📄
        };
        let color = if rf { colors.warn } else { colors.text_soft };
        container(
            text(format!("{} {}", icon, path))
                .size(12)
                .color(color),
        )
        .padding([2, 6])
        .into()
    })
    .collect();

    let file_tree: Element<Message> = if files.is_empty() {
        text("No tracked files found.").size(12).color(colors.muted).into()
    } else {
        scrollable(column(file_rows).spacing(0).width(Length::Fill))
            .height(Length::Fixed(160.0))
            .direction(theme::vscroll_overlay())
            .style(move |t, s| theme::scrollable_style(&c)(t, s))
            .into()
    };

    let file_section: Element<Message> = container(file_tree)
        .width(Length::Fill)
        .padding([6, 0])
        .style(move |_t| container::Style {
            background: Some(iced::Background::Color(iced::Color { a: 0.5, ..c.card })),
            border: iced::Border {
                color: iced::Color { a: 0.15, ..c.border },
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into();

    column![
        row![
            text("Remove Repository").size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(&c),
        ]
        .align_y(iced::Alignment::Center),
        text(format!("Remove \"{}\" from Wuddle?", name))
            .size(13)
            .color(colors.text),
        file_section,
        checkbox(rf)
            .label("Also delete local files (DLLs / addon folders)")
            .on_toggle(Message::ToggleRemoveFiles)
            .text_size(13),
        text(if rf {
            "⚠ Installed files will be permanently deleted from your WoW directory."
        } else {
            "Wuddle will stop tracking this mod. Local files will be left on disk."
        })
        .size(12)
        .color(if rf { colors.warn } else { colors.muted }),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(13))
                .on_press(Message::CloseDialog)
                .padding([6, 12])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c),
                    _ => theme::tab_button_style(&c),
                }),
            {
                let c2 = c;
                let rm_tip = if rf {
                    "Remove and delete local files"
                } else {
                    "Stop tracking this repository"
                };
                tip(
                    button(text("Remove").size(13).color(c.bad))
                        .on_press(Message::RemoveRepoConfirm(repo_id, rf))
                        .padding([6, 12])
                        .style(move |_theme, _status| {
                            let mut s = theme::tab_button_style(&c2);
                            s.border.color = c2.bad;
                            s
                        }),
                    rm_tip,
                    iced::widget::tooltip::Position::Top,
                    colors,
                )
            },
        ]
        .spacing(8),
    ]
    .spacing(12)
    .into()
}
