use iced::widget::{button, column, container, row, text, tooltip, Space};
use iced::{Element, Length};

use crate::theme::{self, ThemeColors};
use crate::{App, Message};

const GITHUB_URL: &str = "https://github.com/ZythDr/Wuddle";
const RELEASES_URL: &str = "https://github.com/ZythDr/Wuddle/releases";
const APP_VERSION: &str = "3.0.0-alpha.3";

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
            text("Wuddle — Addon & mod manager for Turtle WoW.")
                .size(12)
                .color(colors.muted),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
        btn_tip("Refresh", "Re-check for Wuddle updates", Message::CheckSelfUpdate, &c),
        btn_tip("Changelog", "View Wuddle changelog in-app", Message::ShowChangelog, &c),
        btn_tip(update_label, "Current update status of Wuddle", Message::CheckSelfUpdate, &c),
        open_on_github_btn(GITHUB_URL, &c),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    let latest_display = app.latest_version.as_deref().unwrap_or("\u{2014}");

    // Application card — height(Shrink), acts as the row's cross-axis anchor.
    // Credits card — height(Fill), stretches to match the Application card.
    // This works because Iced's Row sets Fill children to the max Shrink-child height.
    let app_card = card(
        column![
            text("Application").size(16).color(colors.title),
            about_row("Current version:", APP_VERSION, colors),
            about_row_btn("Latest version:", latest_display, Message::OpenUrl(RELEASES_URL.to_string()), colors),
            about_row("Package name:", "wuddle-iced", colors),
        ]
        .spacing(8),
        &c,
    );

    // Space(17) + gap(8) = 25px ≈ 1 missing row (size-13 text ~17px at 1.3 line-height) + 1 spacing,
    // making the Credits card the same height as the Application card.
    let credits_card = card(
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
            Space::new().height(17.0),
        ]
        .spacing(8),
        &c,
    );

    let status_text = app
        .update_message
        .as_deref()
        .unwrap_or("Application details loaded.");
    let status = text(status_text).size(12).color(colors.muted);

    let cards_row = row![app_card, credits_card].spacing(8).width(Length::Fill);

    column![header, cards_row, status]
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

fn about_row_btn<'a>(key: &str, value: &str, msg: Message, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    let val_owned = String::from(value);
    row![
        text(String::from(key)).size(13).color(colors.muted).width(160),
        button(
            iced::widget::rich_text::<(), _, _, _>([
                iced::widget::span(val_owned)
                    .underline(true)
                    .color(c.link)
                    .size(13.0_f32),
            ])
        )
        .on_press(msg)
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

fn card<'a>(
    content: impl Into<Element<'a, Message>>,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    container(container(content).padding(16))
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(&c))
        .into()
}

fn open_on_github_btn<'a>(url: &str, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    let url_owned = url.to_string();
    let icon = crate::forge_svg_handle("github", url);
    let icon_color = c.text;
    let btn = button(
        row![
            text("Open Wuddle on").size(13).color(c.text),
            iced::widget::svg(icon)
                .width(14)
                .height(14)
                .style(move |_t, _s| iced::widget::svg::Style { color: Some(icon_color) }),
        ]
        .spacing(5)
        .align_y(iced::Alignment::Center),
    )
    .on_press(Message::OpenUrl(url_owned))
    .padding([6, 12])
    .style(move |_theme, _status| theme::tab_button_active_style(&c));
    tooltip(
        btn,
        container(text("Open the Wuddle repository on GitHub").size(11).color(c.text))
            .padding([3, 8])
            .style(move |_theme| theme::tooltip_style(&c)),
        tooltip::Position::Bottom,
    )
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

