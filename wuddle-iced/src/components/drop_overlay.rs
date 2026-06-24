//! Full-window drag-and-drop hint shown while a supported addon archive hovers.

use iced::widget::{column, container, text};
use iced::{Color, Element, Length};

use crate::theme::ThemeColors;
use crate::Message;

pub fn view(colors: ThemeColors) -> Element<'static, Message> {
    let overlay_bg = Color::from_rgba(0.0, 0.0, 0.0, 0.64);
    let c = colors;

    let panel = container(
        column![
            text("Drop addon archive to install")
                .size(24)
                .color(c.title),
            text(".zip and .7z supported").size(13).color(c.text_soft),
        ]
        .spacing(8)
        .align_x(iced::Alignment::Center),
    )
    .padding([24, 34])
    .style(move |_theme| container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            c.card.r, c.card.g, c.card.b, 0.9,
        ))),
        border: iced::Border {
            color: Color::from_rgba(c.primary.r, c.primary.g, c.primary.b, 0.72),
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow::default(),
        text_color: None,
        snap: true,
    });

    iced::widget::opaque(
        container(panel)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(overlay_bg)),
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                text_color: None,
                snap: true,
            }),
    )
    .into()
}
