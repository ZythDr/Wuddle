use iced::theme::Palette;
use iced::{Color, Theme};

/// Wuddle's 5 custom themes, ported from the CSS variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WuddleTheme {
    Cata,
    Obsidian,
    Emerald,
    Ashen,
    WowUi,
}

impl WuddleTheme {
    pub const ALL: &[WuddleTheme] = &[
        WuddleTheme::Cata,
        WuddleTheme::Obsidian,
        WuddleTheme::Emerald,
        WuddleTheme::Ashen,
        WuddleTheme::WowUi,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WuddleTheme::Cata => "Cata",
            WuddleTheme::Obsidian => "Obsidian",
            WuddleTheme::Emerald => "Emerald",
            WuddleTheme::Ashen => "Ashen",
            WuddleTheme::WowUi => "WoW UI",
        }
    }

    pub fn to_iced_theme(self) -> Theme {
        let (bg, text, primary, success, danger) = match self {
            WuddleTheme::Cata => (
                hex(0x08090d),
                hex(0xefe7d9),
                hex(0xbd7427),
                hex(0x10b981),
                hex(0xef4444),
            ),
            WuddleTheme::Obsidian => (
                hex(0x080a0f),
                hex(0xd8e4f0),
                hex(0x3a7cc6),
                hex(0x10b981),
                hex(0xef4444),
            ),
            WuddleTheme::Emerald => (
                hex(0x080d0a),
                hex(0xdceee2),
                hex(0x2e9c5a),
                hex(0x10b981),
                hex(0xef4444),
            ),
            WuddleTheme::Ashen => (
                hex(0x0d0908),
                hex(0xf0e4da),
                hex(0xc4523a),
                hex(0x10b981),
                hex(0xef4444),
            ),
            WuddleTheme::WowUi => (
                hex(0x0a0808),
                hex(0xf0e0c8),
                hex(0xc8a040),
                hex(0x10b981),
                hex(0xef4444),
            ),
        };

        Theme::custom(
            self.label().to_string(),
            Palette {
                background: bg,
                text,
                primary,
                success,
                warning: hex(0xf59e0b),
                danger,
            },
        )
    }
}

fn hex(rgb: u32) -> Color {
    Color::from_rgb8(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}
