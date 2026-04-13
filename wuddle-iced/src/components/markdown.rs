//! Custom Markdown renderer for Wuddle.
//!
//! Provides `ImageViewer`, a [`iced::widget::markdown::Viewer`] implementation
//! that adds:
//! - Click-to-copy inline code blocks
//! - Syntax-highlighted fenced code blocks (via `syntect`)
//! - GitHub-flavoured admonitions (`> [!NOTE]`, `> [!WARNING]`, etc.)
//! - Bold headings
//! - Cached image / GIF rendering

use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};

use crate::Message;

// ---------------------------------------------------------------------------
// Copy button helper
// ---------------------------------------------------------------------------

/// Wraps any element with a "Copy" button overlaid at the top-right corner.
pub fn with_copy_button(block: Element<'_, Message>, code: String) -> Element<'_, Message> {
    let copy_btn = container(
        button(text("Copy").size(11))
            .on_press(Message::CopyToClipboard(code))
            .padding([2, 8])
            .style(|_theme, status| match status {
                button::Status::Hovered => button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.15))),
                    text_color: iced::Color::WHITE,
                    border: iced::Border { radius: 3.0.into(), ..Default::default() },
                    ..Default::default()
                },
                _ => button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.07))),
                    text_color: iced::Color::from_rgb8(0xb0, 0xc4, 0xde),
                    border: iced::Border { radius: 3.0.into(), ..Default::default() },
                    ..Default::default()
                },
            }),
    )
    .width(Length::Fill)
    .align_x(iced::Alignment::End)
    .padding(iced::Padding { top: 4.0, right: 6.0, bottom: 0.0, left: 0.0 });

    iced::widget::stack![block, copy_btn].into()
}

// ---------------------------------------------------------------------------
// Syntect helpers
// ---------------------------------------------------------------------------

pub fn syntax_set() -> &'static syntect::parsing::SyntaxSet {
    static SS: std::sync::OnceLock<syntect::parsing::SyntaxSet> = std::sync::OnceLock::new();
    SS.get_or_init(syntect::parsing::SyntaxSet::load_defaults_newlines)
}

pub fn highlight_theme() -> &'static syntect::highlighting::Theme {
    static TS: std::sync::OnceLock<syntect::highlighting::ThemeSet> = std::sync::OnceLock::new();
    let ts = TS.get_or_init(syntect::highlighting::ThemeSet::load_defaults);
    &ts.themes["base16-ocean.dark"]
}

// ---------------------------------------------------------------------------
// Empty image / GIF cache singletons
// ---------------------------------------------------------------------------

pub fn empty_image_cache() -> &'static std::collections::HashMap<String, iced::widget::image::Handle> {
    static CACHE: std::sync::OnceLock<std::collections::HashMap<String, iced::widget::image::Handle>> = std::sync::OnceLock::new();
    CACHE.get_or_init(std::collections::HashMap::new)
}

pub fn empty_gif_cache() -> &'static std::collections::HashMap<String, std::sync::Arc<iced_gif::Frames>> {
    static CACHE: std::sync::OnceLock<std::collections::HashMap<String, std::sync::Arc<iced_gif::Frames>>> = std::sync::OnceLock::new();
    CACHE.get_or_init(std::collections::HashMap::new)
}

// ---------------------------------------------------------------------------
// GitHub-flavoured admonition colours
// ---------------------------------------------------------------------------

/// Returns `(icon, label, border_color, title_color)` for a GitHub admonition keyword.
pub fn admonition_style(keyword: &str) -> Option<(&'static str, &'static str, iced::Color, iced::Color)> {
    match keyword {
        "[!NOTE]" => Some((
            "\u{2139}",  // ℹ information source
            "Note",
            iced::Color::from_rgb8(0x1f, 0x6f, 0xeb),
            iced::Color::from_rgb8(0x58, 0xa6, 0xff),
        )),
        "[!TIP]" => Some((
            "\u{1F4A1}", // 💡 light bulb
            "Tip",
            iced::Color::from_rgb8(0x23, 0x86, 0x36),
            iced::Color::from_rgb8(0x3f, 0xb9, 0x50),
        )),
        "[!IMPORTANT]" => Some((
            "\u{1F4AC}", // 💬 speech bubble
            "Important",
            iced::Color::from_rgb8(0x89, 0x57, 0xe5),
            iced::Color::from_rgb8(0xa3, 0x71, 0xf7),
        )),
        "[!WARNING]" => Some((
            "\u{26A0}",  // ⚠ warning sign
            "Warning",
            iced::Color::from_rgb8(0x9e, 0x6a, 0x03),
            iced::Color::from_rgb8(0xd2, 0x99, 0x22),
        )),
        "[!CAUTION]" => Some((
            "\u{26D4}",  // ⛔ no entry
            "Caution",
            iced::Color::from_rgb8(0xda, 0x36, 0x33),
            iced::Color::from_rgb8(0xf8, 0x51, 0x49),
        )),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Custom markdown viewer
// ---------------------------------------------------------------------------

/// A [`iced::widget::markdown::Viewer`] that handles images (static + GIF),
/// bold headings, syntax-highlighted code blocks with copy buttons, and
/// GitHub-style admonitions.
pub struct ImageViewer<'a> {
    pub cache: &'a std::collections::HashMap<String, iced::widget::image::Handle>,
    pub gif_cache: &'a std::collections::HashMap<String, std::sync::Arc<iced_gif::Frames>>,
    pub raw_base_url: &'a str,
}

impl<'a> iced::widget::markdown::Viewer<'a, Message> for ImageViewer<'a> {
    fn on_link_click(url: iced::widget::markdown::Uri) -> Message {
        if let Some(text) = url.strip_prefix("wuddle-copy://") {
            Message::CopyToClipboard(text.to_string())
        } else {
            Message::OpenUrl(url)
        }
    }

    fn paragraph(
        &self,
        settings: iced::widget::markdown::Settings,
        text: &iced::widget::markdown::Text,
    ) -> Element<'a, Message> {
        // Inject copy links into inline-code spans
        let raw_spans = text.spans(settings.style);
        let has_code = raw_spans.iter().any(|s| s.highlight.is_some() && s.link.is_none());
        if !has_code {
            return iced::widget::markdown::paragraph(settings, text, Self::on_link_click);
        }
        let patched: Vec<iced::widget::text::Span<'static, iced::widget::markdown::Uri>> =
            raw_spans.iter().cloned().map(|mut s| {
                if s.highlight.is_some() && s.link.is_none() {
                    let copy_text = s.text.as_ref().trim().to_string();
                    s.link = Some(format!("wuddle-copy://{copy_text}"));
                    s.underline = true;
                }
                s
            }).collect();
        iced::widget::rich_text(patched)
            .size(settings.text_size)
            .on_link_click(Self::on_link_click)
            .into()
    }

    fn heading(
        &self,
        settings: iced::widget::markdown::Settings,
        level: &'a iced::widget::markdown::HeadingLevel,
        text: &'a iced::widget::markdown::Text,
        index: usize,
    ) -> Element<'a, Message> {
        let bold_settings = iced::widget::markdown::Settings {
            style: iced::widget::markdown::Style {
                font: iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..settings.style.font
                },
                ..settings.style
            },
            ..settings
        };
        iced::widget::markdown::heading(bold_settings, level, text, index, Self::on_link_click)
    }

    fn image(
        &self,
        _settings: iced::widget::markdown::Settings,
        url: &'a iced::widget::markdown::Uri,
        _title: &'a str,
        _alt: &iced::widget::markdown::Text,
    ) -> Element<'a, Message> {
        let abs = crate::service::resolve_image_url(url, self.raw_base_url);

        // GIF cache first
        let gif_frames = self.gif_cache.get(url.as_str())
            .or_else(|| self.gif_cache.get(abs.as_str()));
        if let Some(frames) = gif_frames {
            return container(
                iced_gif::widget::gif(frames)
                    .width(Length::Fill)
            )
            .width(Length::Fill)
            .padding([4, 0])
            .into();
        }

        // Static image
        let handle = self.cache.get(url.as_str())
            .or_else(|| self.cache.get(abs.as_str()));
        if let Some(handle) = handle {
            container(
                iced::widget::image(handle.clone())
                    .width(Length::Fill)
            )
            .width(Length::Fill)
            .padding([4, 0])
            .into()
        } else {
            container(
                text(format!("[image: {}]", url.split('/').last().unwrap_or(url)))
                    .size(11)
                    .color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.25))
            )
            .padding([2, 0])
            .into()
        }
    }

    fn quote(
        &self,
        settings: iced::widget::markdown::Settings,
        contents: &'a [iced::widget::markdown::Item],
    ) -> Element<'a, Message> {
        if let Some(iced::widget::markdown::Item::Paragraph(first_text)) = contents.first() {
            let spans = first_text.spans(settings.style);
            let first_span_text = spans.first().map(|s| s.text.trim()).unwrap_or("");
            if let Some((icon, label, border_color, title_color)) = admonition_style(first_span_text) {
                let body_spans: Vec<iced::widget::text::Span<'static, iced::widget::markdown::Uri>> =
                    spans.iter()
                        .skip(1)
                        .skip_while(|s| s.text.trim().is_empty())
                        .cloned()
                        .collect();

                let title_row = row![
                    text(icon)
                        .size(settings.text_size)
                        .color(title_color),
                    text(label)
                        .size(settings.text_size)
                        .font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
                        .color(title_color),
                ]
                .spacing(5)
                .align_y(iced::Alignment::Center);

                let mut body_col = column![title_row].spacing(4);

                if !body_spans.is_empty() {
                    body_col = body_col.push(
                        iced::widget::rich_text(body_spans)
                            .size(settings.text_size)
                            .on_link_click(Self::on_link_click),
                    );
                }
                for item in contents.iter().skip(1) {
                    body_col = body_col.push(
                        iced::widget::markdown::view_with(
                            std::slice::from_ref(item),
                            settings,
                            self,
                        )
                    );
                }

                let stripe = container(iced::widget::Space::new())
                    .width(3)
                    .height(Length::Fill)
                    .style(move |_t| container::Style {
                        background: Some(iced::Background::Color(border_color)),
                        border: iced::Border { radius: 2.0.into(), ..Default::default() },
                        ..Default::default()
                    });
                let content_box = container(body_col)
                    .width(Length::Fill)
                    .padding(iced::Padding { top: 6.0, right: 10.0, bottom: 6.0, left: 10.0 });
                return container(
                    row![stripe, content_box].spacing(0).height(Length::Shrink)
                )
                .width(Length::Fill)
                .padding([4, 0])
                .style(move |_t| container::Style {
                    background: Some(iced::Background::Color(
                        iced::Color { a: 0.06, ..border_color }
                    )),
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into();
            }
        }
        iced::widget::markdown::quote(self, settings, contents)
    }

    fn code_block(
        &self,
        settings: iced::widget::markdown::Settings,
        language: Option<&'a str>,
        code: &'a str,
        lines: &'a [iced::widget::markdown::Text],
    ) -> Element<'a, Message> {
        use syntect::easy::HighlightLines;
        use syntect::util::LinesWithEndings;

        if let Some(lang_str) = language {
            let ps = syntax_set();
            let syntax = ps.find_syntax_by_token(lang_str)
                .or_else(|| ps.find_syntax_by_extension(lang_str));

            if let Some(syntax) = syntax {
                let theme = highlight_theme();
                let mut h = HighlightLines::new(syntax, theme);
                let code_font = settings.style.code_block_font;
                let code_size = settings.code_size;

                let line_elements: Vec<Element<'a, Message>> = LinesWithEndings::from(code)
                    .filter_map(|line| {
                        let tokens = h.highlight_line(line, ps).ok()?;
                        let spans: Vec<iced::widget::text::Span<'static, iced::widget::markdown::Uri>> = tokens
                            .iter()
                            .filter(|(_, s)| !s.is_empty())
                            .map(|(style, token)| {
                                iced::widget::span(token.to_string())
                                    .color(iced::Color::from_rgb(
                                        style.foreground.r as f32 / 255.0,
                                        style.foreground.g as f32 / 255.0,
                                        style.foreground.b as f32 / 255.0,
                                    ))
                                    .font(code_font)
                            })
                            .collect();
                        Some(
                            iced::widget::rich_text(spans)
                                .size(code_size)
                                .into(),
                        )
                    })
                    .collect();

                let bg = iced::Color::from_rgb8(0x14, 0x18, 0x24);
                let border_color = iced::Color::from_rgb8(0x2a, 0x2f, 0x3d);
                let code_owned = code.to_string();

                let inner = container(
                    iced::widget::scrollable(
                        container(column(line_elements))
                            .padding(settings.code_size),
                    )
                    .direction(iced::widget::scrollable::Direction::Horizontal(
                        iced::widget::scrollable::Scrollbar::default()
                            .width(settings.code_size / 2)
                            .scroller_width(settings.code_size / 2),
                    )),
                )
                .width(Length::Fill)
                .padding(settings.code_size / 4)
                .style(move |_t| container::Style {
                    background: Some(iced::Background::Color(bg)),
                    border: iced::Border {
                        color: border_color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                });

                return with_copy_button(inner.into(), code_owned);
            }
        }

        // Fallback: unstyled code block + copy button
        let code_owned = code.to_string();
        let fallback = iced::widget::markdown::code_block(settings, lines, Self::on_link_click);
        with_copy_button(fallback, code_owned)
    }
}
