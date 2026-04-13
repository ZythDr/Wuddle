#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Home,
    Mods,
    Addons,
    Tweaks,
    Options,
    Logs,
    About,
}

impl Tab {
    pub const ALL: &[Tab] = &[
        Tab::Home,
        Tab::Mods,
        Tab::Addons,
        Tab::Tweaks,
        Tab::Options,
        Tab::Logs,
        Tab::About,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Mods => "Mods",
            Tab::Addons => "Addons",
            Tab::Tweaks => "Tweaks",
            Tab::Options => "Options",
            Tab::Logs => "Logs",
            Tab::About => "About",
        }
    }

    pub fn icon_label(self) -> &'static str {
        match self {
            Tab::Options => "\u{2699}",  // ⚙
            Tab::Logs => "\u{2630}",    // ☰
            Tab::About => "\u{24D8}",   // ⓘ
            _ => "",
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Mods => "Mods",
            Tab::Addons => "Addons",
            Tab::Tweaks => "Tweaks",
            Tab::Options => "Options",
            Tab::Logs => "Logs",
            Tab::About => "About",
        }
    }
}
