use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input, Space};
use iced::{Element, Font, Length};

use crate::theme::{self, ThemeColors};
use crate::{App, LogFilter, LogLevel, Message};

const MONO: Font = Font::MONOSPACE;

pub fn view<'a>(app: &'a App, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;

    // Header
    let header = row![
        column![
            text("Logs").size(18).color(colors.title),
            text("Action and error messages.").size(12).color(colors.muted),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
        {
            let c2 = c;
            button(text("Clear").size(13))
                .on_press(Message::ClearLogs)
                .padding([6, 12])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => theme::tab_button_style(&c2),
                })
        },
        {
            let c2 = c;
            let log_text_copy = build_log_text(app);
            button(text("Copy Log").size(13))
                .on_press(Message::CopyToClipboard(log_text_copy))
                .padding([6, 12])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => theme::tab_button_style(&c2),
                })
        },
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    // Filter toolbar
    let toolbar = row![
        filter_btn("All", LogFilter::All, app.log_filter, &c),
        filter_btn("Info", LogFilter::Info, app.log_filter, &c),
        filter_btn("Errors", LogFilter::Errors, app.log_filter, &c),
        Space::new().width(Length::Fill),
        checkbox(app.log_wrap)
            .label("Wrap lines")
            .on_toggle(Message::ToggleLogWrap),
        checkbox(app.log_autoscroll)
            .label("Auto-scroll")
            .on_toggle(Message::ToggleLogAutoScroll),
        text_input("Search logs", &app.log_search)
            .on_input(Message::SetLogSearch)
            .width(180)
            .padding([6, 10]),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    // Build log text from actual log_lines
    let search = app.log_search.to_lowercase();
    let log_text: String = app
        .log_lines
        .iter()
        .filter(|line| match app.log_filter {
            LogFilter::All => true,
            LogFilter::Info => matches!(line.level, LogLevel::Info),
            LogFilter::Errors => matches!(line.level, LogLevel::Error),
        })
        .filter(|line| search.is_empty() || line.text.to_lowercase().contains(&search))
        .map(|line| {
            let prefix = match line.level {
                LogLevel::Info => "[INFO]",
                LogLevel::Error => "[ERROR]",
            };
            format!("[{}] {} {}", line.timestamp, prefix, line.text)
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Terminal-style log area with dark solid background
    let log_text_color = iced::Color::from_rgb8(0xdb, 0xe7, 0xff);
    let log_content = container(
        scrollable(text(log_text).size(12).font(MONO).color(log_text_color)).height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(12)
    .style(move |_theme| theme::log_terminal_style(&c));

    column![header, toolbar, log_content]
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub fn build_log_text(app: &App) -> String {
    app.log_lines
        .iter()
        .map(|line| {
            let prefix = match line.level {
                LogLevel::Info => "[INFO]",
                LogLevel::Error => "[ERROR]",
            };
            format!("[{}] {} {}", line.timestamp, prefix, line.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn filter_btn<'a>(
    label: &str,
    filter: LogFilter,
    active_filter: LogFilter,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let active = filter == active_filter;
    let b = button(text(String::from(label)).size(12))
        .on_press(Message::SetLogFilter(filter))
        .padding([4, 10]);
    if active {
        b.style(move |_theme, _status| theme::tab_button_active_style(&c))
            .into()
    } else {
        b.style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(&c),
            _ => theme::tab_button_style(&c),
        })
        .into()
    }
}
