//! ModFileInfo dialog — shows a markdown README/description for a single DLL/addon file.

use iced::widget::{column, row, scrollable, text, Space};
use iced::widget::markdown;
use iced::{Element, Font, Length};
use crate::{Message, theme};
use crate::components::helpers::close_button;
use crate::components::markdown::{ImageViewer, empty_image_cache, empty_gif_cache};
use theme::ThemeColors;

pub fn view<'a>(
    name: &'a str,
    items: &'a [markdown::Item],
    app_theme: &iced::Theme,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;

    let mut md_style = markdown::Style::from(app_theme);
    md_style.link_color = colors.link;
    md_style.font = Font::DEFAULT;

    // "Terminal aesthetic" — warm amber inline code on dark background
    md_style.inline_code_color = iced::Color::from_rgb8(0xe0, 0xc0, 0x80);
    md_style.inline_code_highlight = markdown::Highlight {
        background: iced::Color::from_rgb8(0x14, 0x18, 0x24).into(),
        border: iced::Border {
            color: iced::Color::from_rgb8(0x2a, 0x2f, 0x3d),
            width: 1.0,
            radius: 3.0.into(),
        },
    };

    let mut md_settings = markdown::Settings::with_text_size(13, md_style);
    md_settings.h1_size = 22.0.into();
    md_settings.h2_size = 19.0.into();
    md_settings.h3_size = 16.0.into();
    md_settings.h4_size = 14.0.into();

    let viewer = ImageViewer {
        cache: empty_image_cache(),
        gif_cache: empty_gif_cache(),
        raw_base_url: "",
    };

    column![
        row![
            text(name)
                .size(20)
                .color(colors.title)
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
            Space::new().width(Length::Fill),
            close_button(c),
        ]
        .align_y(iced::Alignment::Center),
        scrollable(markdown::view_with(items, md_settings, &viewer))
            .height(400)
            .style(move |t, s| theme::scrollable_style(c)(t, s)),
    ]
    .spacing(16)
    .width(Length::Fixed(750.0))
    .into()
}
