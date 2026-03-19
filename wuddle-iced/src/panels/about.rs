use iced::widget::{button, column, container, row, text, tooltip, Space};
use iced::{Element, Length};

use crate::theme::{self, ThemeColors};
use crate::{App, Message};

const GITHUB_URL: &str = "https://github.com/ZythDr/Wuddle";

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
        btn_tip("Refresh", "Re-check for Wuddle updates", Message::CheckSelfUpdate, &c),
        btn_tip("Changelog", "View Wuddle changelog in-app", Message::ShowChangelog, &c),
        btn_tip(update_label, "Current update status of Wuddle", Message::CheckSelfUpdate, &c),
        btn_tip(
            "Open on GitHub",
            "Open Wuddle repository on GitHub",
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
            about_row("Current version:", "3.0.0-alpha.3", colors),
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

    column![header, row![app_card, credits_card].spacing(8), status]
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
        button(
            iced::widget::rich_text::<(), _, _, _>([
                iced::widget::span(String::from(label))
                    .underline(true)
                    .color(c.link)
                    .size(13.0_f32),
            ])
        )
        .on_press(Message::OpenUrl(url_owned))
        .padding(0)
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: c.link,
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

fn btn_tip<'a>(label: &str, tip: &str, msg: Message, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    let tip_str = String::from(tip);
    let btn = button(text(String::from(label)).size(13))
        .on_press(msg)
        .padding([6, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(&c),
            _ => theme::tab_button_style(&c),
        });
    tooltip(
        btn,
        container(text(tip_str).size(11).color(c.text))
            .padding([3, 8])
            .style(move |_theme| theme::tooltip_style(&c)),
        tooltip::Position::Bottom,
    )
    .into()
}
