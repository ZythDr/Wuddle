mod theme;

use iced::widget::{button, column, container, row, text, Space};
use iced::{Element, Length, Task, Theme};
use theme::WuddleTheme;

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("Wuddle")
        .theme(App::theme)
        .window_size((1100.0, 850.0))
        .run()
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Mods,
    Addons,
    Tweaks,
    Options,
    Logs,
    About,
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Mods => "Mods",
            Tab::Addons => "Addons",
            Tab::Tweaks => "Tweaks",
            Tab::Options => "\u{2699}",  // ⚙
            Tab::Logs => "\u{1f4cb}",    // 📋
            Tab::About => "\u{2139}",    // ℹ
        }
    }
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct App {
    active_tab: Tab,
    wuddle_theme: WuddleTheme,
}

impl Default for Tab {
    fn default() -> Self {
        Tab::Home
    }
}

impl Default for WuddleTheme {
    fn default() -> Self {
        WuddleTheme::Cata
    }
}

#[derive(Debug, Clone)]
enum Message {
    SetTab(Tab),
    SetTheme(WuddleTheme),
}

impl App {
    fn theme(&self) -> Theme {
        self.wuddle_theme.to_iced_theme()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetTab(tab) => {
                self.active_tab = tab;
            }
            Message::SetTheme(theme) => {
                self.wuddle_theme = theme;
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let topbar = self.view_topbar();
        let body = self.view_panel();

        column![topbar, body].into()
    }

    // -----------------------------------------------------------------------
    // Topbar
    // -----------------------------------------------------------------------

    fn view_topbar(&self) -> Element<'_, Message> {
        let title = text("Wuddle").size(28);

        let view_tabs = row![
            self.tab_button(Tab::Home),
            self.tab_button(Tab::Mods),
            self.tab_button(Tab::Addons),
            self.tab_button(Tab::Tweaks),
        ]
        .spacing(4);

        let action_tabs = row![
            self.tab_button(Tab::Options),
            self.tab_button(Tab::Logs),
            self.tab_button(Tab::About),
        ]
        .spacing(4);

        let bar = row![
            title,
            Space::new().width(Length::Fill),
            view_tabs,
            Space::new().width(Length::Fill),
            action_tabs,
        ]
        .spacing(12)
        .padding(12)
        .align_y(iced::Alignment::Center);

        container(bar)
            .width(Length::Fill)
            .into()
    }

    fn tab_button(&self, tab: Tab) -> Element<'_, Message> {
        let label = text(tab.label()).size(14);
        let btn = button(label).on_press(Message::SetTab(tab));

        if self.active_tab == tab {
            btn.style(button::primary).into()
        } else {
            btn.into()
        }
    }

    // -----------------------------------------------------------------------
    // Panel body
    // -----------------------------------------------------------------------

    fn view_panel(&self) -> Element<'_, Message> {
        let content: Element<Message> = match self.active_tab {
            Tab::Home => self.view_home(),
            Tab::Mods | Tab::Addons => self.view_projects(),
            Tab::Tweaks => view_placeholder("Tweaks"),
            Tab::Options => self.view_options(),
            Tab::Logs => view_placeholder("Logs"),
            Tab::About => view_placeholder("About"),
        };

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(16)
            .into()
    }

    fn view_home(&self) -> Element<'_, Message> {
        column![
            text("Home").size(20),
            text("Update summary and Play button will go here.").size(14),
        ]
        .spacing(8)
        .into()
    }

    fn view_projects(&self) -> Element<'_, Message> {
        let view_label = match self.active_tab {
            Tab::Addons => "Addons",
            _ => "Mods",
        };

        column![
            text(view_label).size(20),
            text("Project list with filtering, sorting, and search will go here.").size(14),
        ]
        .spacing(8)
        .into()
    }

    fn view_options(&self) -> Element<'_, Message> {
        let theme_buttons: Vec<Element<Message>> = WuddleTheme::ALL
            .iter()
            .map(|&t| {
                let label = text(t.label()).size(13);
                let btn = button(label).on_press(Message::SetTheme(t));
                if t == self.wuddle_theme {
                    btn.style(button::primary).into()
                } else {
                    btn.into()
                }
            })
            .collect();

        column![
            text("Settings").size(20),
            text("Theme").size(16),
            row(theme_buttons).spacing(6),
        ]
        .spacing(12)
        .into()
    }
}

fn view_placeholder(label: &str) -> Element<'_, Message> {
    column![
        text(String::from(label)).size(20),
        text(format!("{label} panel — work in progress")).size(14),
    ]
    .spacing(8)
    .into()
}
