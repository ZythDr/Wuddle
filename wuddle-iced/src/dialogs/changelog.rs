//! Changelog dialog — shows the self-updater release notes as scrollable markdown.

use iced::widget::{column, row, scrollable, text, Space};
use iced::{Element, Length};
use theme::ThemeColors;
use crate::{Message, theme};
use crate::components::helpers::close_button;
use crate::components::markdown::{ImageViewer, empty_image_cache, empty_gif_cache};

pub fn view<'a>(
    items: &'a [iced::widget::markdown::Item],
    loading: bool,
    app_theme: &iced::Theme,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;

    let body: Element<Message> = if loading {
        iced::widget::container(text("Loading changelog…").size(13).color(colors.muted))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fixed(300.0))
            .into()
    } else {
        let mut cl_style = iced::widget::markdown::Style::from(app_theme);
        cl_style.link_color = c.link;
        let md_settings = iced::widget::markdown::Settings::with_text_size(13, cl_style);
        let viewer = ImageViewer {
            cache: empty_image_cache(),
            gif_cache: empty_gif_cache(),
            raw_base_url: "",
        };
        scrollable(iced::widget::markdown::view_with(items, md_settings, &viewer))
            .height(Length::Fixed(480.0))
            .direction(theme::vscroll())
            .style(move |t, s| theme::scrollable_style(&c)(t, s))
            .into()
    };

    column![
        row![
            text("Changelog").size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(&c),
        ]
        .align_y(iced::Alignment::Center),
        body,
    ]
    .spacing(12)
    .width(Length::Fixed(700.0))
    .into()
}
