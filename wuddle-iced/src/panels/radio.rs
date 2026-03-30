use crate::{Message, ThemeColors};
use iced::widget::{button, checkbox, column, container, row, rule, text, text_input, tooltip, Space};
use iced::{Element, Length};
use crate::theme;

pub const BUFFER_PRESETS: &[(usize, &str)] = &[
    (512, "512"),
    (1024, "1K"),
    (2048, "2K"),
    (4096, "4K"),
    (8192, "8K"),
    (16384, "16K"),
];

pub fn view<'a>(
    auto_connect: &bool,
    auto_play: &bool,
    buffer_size: &str,
    custom_buffer: bool,
    persist_volume: &bool,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;

    // Close button (matches close_button() in main.rs)
    let close_btn = button(text("\u{2715}").size(14).color(c.bad))
        .on_press(Message::CloseRadioSettings)
        .padding([4, 8])
        .style(move |_theme, status| match status {
            button::Status::Hovered => button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    c.bad.r, c.bad.g, c.bad.b, 0.15,
                ))),
                text_color: c.bad,
                border: iced::Border {
                    color: iced::Color::from_rgba(c.bad.r, c.bad.g, c.bad.b, 0.4),
                    width: 1.0,
                    radius: iced::border::Radius::from(4),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            },
            _ => button::Style {
                background: None,
                text_color: c.bad,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
        });

    // --- Checkboxes ---

    let auto_connect_cb = checkbox(*auto_connect)
        .label("Auto-connect")
        .text_size(13)
        .on_toggle(Message::SetRadioAutoConnect);

    // Auto-play — disabled unless auto-connect is on
    let auto_play_el: Element<'a, Message> = if *auto_connect {
        checkbox(*auto_play)
            .label("Auto-play when connected")
            .text_size(13)
            .on_toggle(Message::SetRadioAutoPlay)
            .into()
    } else {
        let c2 = c;
        tooltip(
            checkbox(false)
                .label("Auto-play when connected")
                .text_size(13),
            container(
                text("Enable Auto-connect first").size(13).color(c2.text),
            )
            .padding([3, 8])
            .style(move |_| crate::theme::tooltip_style(&c2)),
            tooltip::Position::Top,
        )
        .gap(4.0)
        .into()
    };

    let persist_vol_cb = checkbox(*persist_volume)
        .label("Remember volume between sessions")
        .text_size(13)
        .on_toggle(Message::SetRadioPersistVolume);

    // --- Buffer presets ---

    let current_val: usize = buffer_size.parse().unwrap_or(4096);

    let mut preset_row = row![
        text("Read-ahead buffer:").size(13).color(c.text),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    for &(val, label) in BUFFER_PRESETS {
        let active = !custom_buffer && val == current_val;
        let c3 = c;
        preset_row = preset_row.push(
            button(text(label).size(13))
                .on_press(Message::SetRadioBufferSize(val.to_string()))
                .padding([4, 8])
                .style(move |_theme, status| {
                    if active {
                        theme::tab_button_active_style(&c3)
                    } else {
                        match status {
                            button::Status::Hovered => theme::tab_button_hovered_style(&c3),
                            _ => theme::tab_button_style(&c3),
                        }
                    }
                }),
        );
    }

    // "Custom" button — toggles custom mode
    let c4 = c;
    preset_row = preset_row.push(
        button(text("Custom").size(13))
            .on_press(Message::SetRadioCustomBuffer(!custom_buffer))
            .padding([4, 8])
            .style(move |_theme, status| {
                if custom_buffer {
                    theme::tab_button_active_style(&c4)
                } else {
                    match status {
                        button::Status::Hovered => theme::tab_button_hovered_style(&c4),
                        _ => theme::tab_button_style(&c4),
                    }
                }
            }),
    );

    // Input field — always shown, but only editable when custom is selected
    let input_row = row![
        text("Bytes:").size(13).color(c.muted),
        {
            let inp = text_input("4096", buffer_size).width(100);
            if custom_buffer {
                inp.on_input(Message::SetRadioBufferSize)
            } else {
                inp // read-only
            }
        },
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let buffer_hint = text("Smaller = faster startup, larger = more stable on slow connections")
        .size(11)
        .color(c.muted);

    // --- Bottom buttons (right-aligned) ---

    let cancel_btn = button(text("Cancel").size(13))
        .on_press(Message::CloseRadioSettings)
        .padding([6, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(&c),
            _ => theme::tab_button_style(&c),
        });

    let save_btn = button(text("Save").size(13))
        .on_press(Message::SaveRadioSettings)
        .padding([6, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(&c),
            _ => theme::tab_button_active_style(&c),
        });

    column![
        // Header
        row![
            text("Radio Settings").size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_btn,
        ]
        .align_y(iced::Alignment::Center),

        // Options
        auto_connect_cb,
        auto_play_el,
        persist_vol_cb,

        // Separator
        rule::horizontal(1),

        // Buffer
        preset_row,
        input_row,
        buffer_hint,

        // Bottom buttons (right-aligned)
        row![
            Space::new().width(Length::Fill),
            cancel_btn,
            save_btn,
        ]
        .spacing(8),
    ]
    .spacing(12)
    .into()
}
