use iced::widget::{button, column, container, row, text, Space};
use iced::{Element, Length};

use crate::theme::{self, ThemeColors};
use crate::{App, Message};

const GITHUB_URL: &str = "https://github.com/ZythDr/Wuddle";
const RELEASES_URL: &str = "https://github.com/ZythDr/Wuddle/releases";

pub fn view<'a>(app: &'a App, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;

    let update_label = app
        .update_message
        .as_deref()
        .unwrap_or("Check for updates");

    // Header
    let header = row![
        column![
            text("About").size(18).color(colors.title),
            text("Basic application metadata.").size(12).color(colors.muted),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
        btn("Refresh details", Message::CheckSelfUpdate, &c),
        btn("Changelog", Message::OpenUrl(RELEASES_URL.to_string()), &c),
        btn(update_label, Message::CheckSelfUpdate, &c),
        btn(
            "Open Wuddle on GitHub",
            Message::OpenUrl(GITHUB_URL.to_string()),
            &c,
        ),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    let latest_display = app
        .latest_version
        .as_deref()
        .unwrap_or("\u{2014}");

    // Application card
    let app_card = settings_card(
        column![
            text("Application").size(16).color(colors.title),
            about_row("Current version:", "3.0.0-alpha.1", colors),
            about_row("Latest version:", latest_display, colors),
            about_row("Package name:", "wuddle-iced", colors),
        ]
        .spacing(8),
        &c,
    );

    // Credits card
    let credits_card = settings_card(
        column![
            text("Credits").size(16).color(colors.title),
            credit_row(
                "Addon management",
                "GitAddonsManager by WobLight (GPLv3)",
                "https://github.com/WobLight/GitAddonsManager",
                colors,
            ),
            credit_row(
                "WoW.exe patcher",
                "vanilla-tweaks by brndd (MIT)",
                "https://github.com/brndd/vanilla-tweaks",
                colors,
            ),
        ]
        .spacing(8),
        &c,
    );

    let status = text("Application details loaded.")
        .size(12)
        .color(colors.muted);

    column![header, app_card, credits_card, status]
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn about_row<'a>(key: &str, value: &str, colors: &ThemeColors) -> Element<'a, Message> {
    row![
        text(String::from(key)).size(13).color(colors.muted).width(160),
        text(String::from(value)).size(13).color(colors.text),
    ]
    .spacing(8)
    .into()
}

fn credit_row<'a>(
    key: &str,
    label: &str,
    url: &str,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let url_owned = String::from(url);
    row![
        text(String::from(key)).size(13).color(colors.muted).width(160),
        button(text(String::from(label)).size(13).color(c.primary))
            .on_press(Message::OpenUrl(url_owned))
            .padding(0)
            .style(move |_theme, _status| button::Style {
                background: None,
                text_color: c.primary,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            }),
    ]
    .spacing(8)
    .into()
}

fn settings_card<'a>(
    content: impl Into<Element<'a, Message>>,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    container(container(content).padding(16))
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(&c))
        .into()
}

fn btn<'a>(label: &str, msg: Message, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    button(text(String::from(label)).size(13))
        .on_press(msg)
        .padding([6, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(&c),
            _ => theme::tab_button_style(&c),
        })
        .into()
}
