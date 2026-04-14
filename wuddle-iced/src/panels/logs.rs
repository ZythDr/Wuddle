use iced::widget::{button, checkbox, column, container, row, stack, text, text_editor, text_input, tooltip, Space};
use iced::{Element, Font, Length};
use iced::advanced::text::Wrapping;

use crate::theme::{self, ThemeColors};
use crate::{App, LogFilter, LogLevel, Message};

// ---------------------------------------------------------------------------
// Log syntax highlighter — colors entire lines based on [INFO]/[ERROR] prefix.
// Settings = error color (passed at view time); Highlight = Option<Color>
// (None = use default text color for INFO lines, Some(c) = error color).
// ---------------------------------------------------------------------------

pub struct LogHighlighter {
    error_color: iced::Color,
    api_color: iced::Color,
    current_line: usize,
}

impl iced::advanced::text::Highlighter for LogHighlighter {
    type Settings = (iced::Color, iced::Color); // (error, api)
    type Highlight = Option<iced::Color>;
    type Iterator<'a> = std::iter::Once<(std::ops::Range<usize>, Option<iced::Color>)>;

    fn new(settings: &Self::Settings) -> Self {
        Self { error_color: settings.0, api_color: settings.1, current_line: 0 }
    }
    fn update(&mut self, new_settings: &Self::Settings) {
        self.error_color = new_settings.0;
        self.api_color = new_settings.1;
    }
    fn change_line(&mut self, line: usize) {
        self.current_line = line;
    }
    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let color = if line.contains("[ERROR]") {
            Some(self.error_color)
        } else if line.contains("[API]") {
            Some(self.api_color)
        } else {
            None
        };
        std::iter::once((0..line.len(), color))
    }
    fn current_line(&self) -> usize {
        self.current_line
    }
}

fn log_to_format(
    h: &Option<iced::Color>,
    _theme: &iced::Theme,
) -> iced::advanced::text::highlighter::Format<Font> {
    iced::advanced::text::highlighter::Format { color: *h, font: None }
}

/// Returns true if an error message is a network/fetch error (git fetch, API call, etc.).
pub fn is_fetch_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("fetch branches")
        || m.contains("list remote")
        || m.contains("connect remote")
        || m.contains("code=auth")
        || m.contains("auth failed")
        || m.contains("authentication required")
        || m.contains("failed to fetch")
        || m.contains("no such remote")
        || m.contains("network")
        || m.contains("dns")
        || m.contains("connection refused")
        || m.contains("timed out")
}

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
        tip(
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
            "Clear all log messages",
            tooltip::Position::Bottom,
            colors,
        ),
        tip(
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
            "Copy all log output to clipboard",
            tooltip::Position::Bottom,
            colors,
        ),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    // Filter toolbar
    let toolbar = row![
        filter_btn("All", LogFilter::All, app.log_filter, &c),
        filter_btn("Info", LogFilter::Info, app.log_filter, &c),
        filter_btn("API", LogFilter::Api, app.log_filter, &c),
        filter_btn("Errors", LogFilter::Errors, app.log_filter, &c),
        Space::new().width(Length::Fill),
        checkbox(app.log_wrap)
            .label("Wrap lines")
            .on_toggle(Message::ToggleLogWrap),
        checkbox(app.log_autoscroll)
            .label("Auto-scroll")
            .on_toggle(Message::ToggleLogAutoScroll),
        {
            let c2 = c;
            let show_clear = !app.log_search.is_empty();
            stack![
                text_input("Search logs", &app.log_search)
                    .on_input(Message::SetLogSearch)
                    .width(180)
                    .padding(iced::Padding { top: 4.0, right: if show_clear { 26.0 } else { 10.0 }, bottom: 4.0, left: 10.0 }),
                {
                    let clear_el: Element<Message> = if show_clear {
                        button(text("\u{2715}").size(12).color(c2.muted))
                            .on_press(Message::SetLogSearch(String::new()))
                            .padding([3, 7])
                            .style(move |_t, _s| button::Style {
                                background: None,
                                text_color: c2.muted,
                                border: iced::Border::default(),
                                shadow: iced::Shadow::default(),
                                snap: true,
                            })
                            .into()
                    } else {
                        Space::new().into()
                    };
                    container(clear_el)
                }
                .width(180)
                .height(Length::Fill)
                .align_x(iced::Alignment::End)
                .align_y(iced::Alignment::Center)
                .padding(iced::Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 0.0 }),
            ]
            .width(180)
        },
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    // Error sub-filter row — only visible when Errors filter is active
    let error_subfilter: Option<Element<Message>> = if app.log_filter == LogFilter::Errors {
        Some(
            row![
                text("Show:").size(12).color(colors.muted),
                checkbox(app.log_error_fetch)
                    .label("Fetch / Network")
                    .on_toggle(Message::ToggleLogErrorFetch),
                checkbox(app.log_error_misc)
                    .label("Other")
                    .on_toggle(Message::ToggleLogErrorMisc),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .into()
        )
    } else {
        None
    };

    // Selectable terminal-style log area backed by text_editor (read-only: edits blocked in update())
    let log_content = text_editor(&app.log_editor_content)
        .on_action(Message::LogEditorAction)
        .font(MONO)
        .size(12)
        .height(Length::Fill)
        .padding(12)
        .wrapping(if app.log_wrap { Wrapping::Word } else { Wrapping::None })
        .highlight_with::<LogHighlighter>((c.bad, iced::Color::from_rgb8(0, 191, 255)), log_to_format)
        .style(move |theme, status| theme::log_editor_style(&c)(theme, status));

    let mut col = column![header, toolbar];
    if let Some(sub) = error_subfilter {
        col = col.push(sub);
    }
    col.push(log_content)
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
                LogLevel::Api => "[API]",
                LogLevel::Error => "[ERROR]",
            };
            format!("[{}] {} {}", line.timestamp, prefix, line.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wrap any element in a tooltip with consistent styling.
fn tip<'a>(content: impl Into<Element<'a, Message>>, tip_text: &str, pos: tooltip::Position, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    let tip_str = String::from(tip_text);
    tooltip(
        content,
        container(text(tip_str).size(13).color(c.text))
            .padding([3, 8])
            .style(move |_theme| theme::tooltip_style(&c)),
        pos,
    )
    .gap(4.0)
    .into()
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
