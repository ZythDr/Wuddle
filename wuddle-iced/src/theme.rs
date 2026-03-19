use iced::border::Radius;
use iced::gradient;
use iced::theme::Palette;
use iced::widget::button;
use iced::widget::container;
use iced::widget::rule;
use iced::{Border, Color, Font, Gradient, Radians, Shadow, Theme, Vector};

/// Wuddle's 5 custom themes, ported from the CSS variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WuddleTheme {
    Cata,
    Obsidian,
    Emerald,
    Ashen,
    WowUi,
}

/// Extended color palette for Wuddle themes — covers gradients, borders, etc.
#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub bg: Color,
    pub card: Color,
    pub card2: Color,
    pub text: Color,
    pub text_soft: Color,
    pub muted: Color,
    pub title: Color,
    pub primary: Color,
    pub primary_text: Color,
    pub link: Color,
    pub good: Color,
    pub warn: Color,
    pub bad: Color,

    // Topbar
    pub topbar_border: Color,
    pub topbar_grad_top: Color,
    pub topbar_grad_bottom: Color,

    // Buttons / tabs
    pub btn_border: Color,
    pub tab_idle_top: Color,
    pub tab_idle_bottom: Color,
    pub tab_active_top: Color,
    pub tab_active_bottom: Color,
    pub btn_hover_top: Color,
    pub btn_hover_bottom: Color,

    // Card gradient
    pub card_grad_top: Color,
    pub card_grad_bottom: Color,

    // Table header gradient
    pub table_head_top: Color,
    pub table_head_bottom: Color,

    // Play button
    pub play_text: Color,
    pub play_top: Color,
    pub play_bottom: Color,
    pub play_border: Color,
    pub play_hover_top: Color,

    // Footer
    pub footer_top: Color,
    pub footer_bottom: Color,

    pub border: Color,

    // Background gradient (approximates the radial orange+blue gradient)
    pub bg_grad_start: Color,  // top-left (orange-tinted)
    pub bg_grad_mid: Color,    // center
    pub bg_grad_end: Color,    // bottom

    /// Body text font (Friz Quadrata when enabled, system default otherwise)
    pub body_font: Font,
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

    pub fn key(self) -> &'static str {
        match self {
            WuddleTheme::Cata => "cata",
            WuddleTheme::Obsidian => "obsidian",
            WuddleTheme::Emerald => "emerald",
            WuddleTheme::Ashen => "ashen",
            WuddleTheme::WowUi => "wowui",
        }
    }

    pub fn from_key(s: &str) -> Self {
        match s {
            "obsidian" => WuddleTheme::Obsidian,
            "emerald" => WuddleTheme::Emerald,
            "ashen" => WuddleTheme::Ashen,
            "wowui" => WuddleTheme::WowUi,
            _ => WuddleTheme::Cata,
        }
    }

    pub fn colors(self) -> ThemeColors {
        match self {
            WuddleTheme::Cata => ThemeColors {
                bg: hex(0x08090d),
                card: hex(0x111018),
                card2: hex(0x0d0c13),
                text: hex(0xefe7d9),
                text_soft: rgba(229, 208, 177, 0.56),
                muted: rgba(231, 212, 181, 0.72),
                title: hex(0xf6e8cb),
                primary: hex(0xbd7427),
                primary_text: hex(0xfff3df),
                link: hex(0x69bbff),
                good: hex(0x10b981),
                warn: hex(0xf59e0b),
                bad: hex(0xef4444),
                topbar_border: rgba(196, 136, 73, 0.38),
                topbar_grad_top: rgba(255, 166, 87, 0.06),
                topbar_grad_bottom: rgba(255, 166, 87, 0.01),
                btn_border: rgba(196, 136, 73, 0.42),
                tab_idle_top: rgba(72, 52, 37, 0.7),
                tab_idle_bottom: rgba(45, 32, 24, 0.8),
                tab_active_top: hex(0xd18a38),
                tab_active_bottom: hex(0x9d581f),
                btn_hover_top: rgba(93, 66, 46, 0.82),
                btn_hover_bottom: rgba(59, 42, 31, 0.92),
                card_grad_top: hex(0x2a1d14),
                card_grad_bottom: hex(0x0d0c13),
                table_head_top: hex(0x39271d),
                table_head_bottom: hex(0x1a141c),
                play_text: hex(0xffe6bf),
                play_top: hex(0xf0a24b),
                play_bottom: hex(0xb46222),
                play_border: rgba(245, 184, 113, 0.75),
                play_hover_top: hex(0xf6ae5d),
                footer_top: hex(0x33231a),
                footer_bottom: hex(0x100e16),
                border: rgba(196, 136, 73, 0.30),
                bg_grad_start: hex(0x1f1614),  // top (warm brown, blending Tauri's orange radial)
                bg_grad_mid: hex(0x0f0c12),    // middle
                bg_grad_end: hex(0x090a0e),     // bottom
                body_font: Font::DEFAULT,
            },
            WuddleTheme::Obsidian => ThemeColors {
                bg: hex(0x080a0f),
                card: hex(0x0f1420),
                card2: hex(0x0a0f18),
                text: hex(0xd8e4f0),
                text_soft: rgba(190, 213, 240, 0.60),
                muted: rgba(186, 210, 232, 0.72),
                title: hex(0xd5e6f5),
                primary: hex(0x3a7cc6),
                primary_text: hex(0xe3f0ff),
                link: hex(0x69bbff),
                good: hex(0x10b981),
                warn: hex(0xf59e0b),
                bad: hex(0xef4444),
                topbar_border: rgba(83, 132, 178, 0.42),
                topbar_grad_top: rgba(115, 174, 229, 0.10),
                topbar_grad_bottom: rgba(115, 174, 229, 0.02),
                btn_border: rgba(86, 145, 198, 0.44),
                tab_idle_top: rgba(38, 63, 90, 0.72),
                tab_idle_bottom: rgba(25, 41, 62, 0.84),
                tab_active_top: hex(0x4f8ec6),
                tab_active_bottom: hex(0x2a679d),
                btn_hover_top: rgba(54, 82, 112, 0.82),
                btn_hover_bottom: rgba(31, 52, 76, 0.94),
                card_grad_top: hex(0x142234),
                card_grad_bottom: hex(0x0b111c),
                table_head_top: hex(0x2a435e),
                table_head_bottom: hex(0x111c2c),
                play_text: hex(0xedf7ff),
                play_top: hex(0x5ea5dc),
                play_bottom: hex(0x2d6ea7),
                play_border: rgba(137, 188, 231, 0.78),
                play_hover_top: hex(0x72b5e6),
                footer_top: hex(0x243951),
                footer_bottom: hex(0x0c141f),
                border: rgba(83, 132, 178, 0.30),
                bg_grad_start: hex(0x121520),
                bg_grad_mid: hex(0x0b0f18),
                bg_grad_end: hex(0x080a10),
                body_font: Font::DEFAULT,
            },
            WuddleTheme::Emerald => ThemeColors {
                bg: hex(0x080d0a),
                card: hex(0x0e1812),
                card2: hex(0x0a120d),
                text: hex(0xdceee2),
                text_soft: rgba(186, 221, 202, 0.60),
                muted: rgba(189, 224, 202, 0.72),
                title: hex(0xd4f0de),
                primary: hex(0x2e9c5a),
                primary_text: hex(0xdcffe9),
                link: hex(0x69bbff),
                good: hex(0x10b981),
                warn: hex(0xf59e0b),
                bad: hex(0xef4444),
                topbar_border: rgba(87, 154, 117, 0.40),
                topbar_grad_top: rgba(126, 213, 160, 0.08),
                topbar_grad_bottom: rgba(126, 213, 160, 0.01),
                btn_border: rgba(98, 171, 132, 0.46),
                tab_idle_top: rgba(40, 71, 55, 0.74),
                tab_idle_bottom: rgba(25, 49, 38, 0.86),
                tab_active_top: hex(0x4aa276),
                tab_active_bottom: hex(0x2f7a57),
                btn_hover_top: rgba(56, 88, 70, 0.84),
                btn_hover_bottom: rgba(32, 57, 44, 0.94),
                card_grad_top: hex(0x182c22),
                card_grad_bottom: hex(0x0b1411),
                table_head_top: hex(0x294838),
                table_head_bottom: hex(0x0e1a15),
                play_text: hex(0xeefff5),
                play_top: hex(0x56b989),
                play_bottom: hex(0x33825c),
                play_border: rgba(140, 228, 181, 0.74),
                play_hover_top: hex(0x66c799),
                footer_top: hex(0x234031),
                footer_bottom: hex(0x0a130f),
                border: rgba(87, 154, 117, 0.30),
                bg_grad_start: hex(0x121b14),
                bg_grad_mid: hex(0x0b120e),
                bg_grad_end: hex(0x080d0a),
                body_font: Font::DEFAULT,
            },
            WuddleTheme::Ashen => ThemeColors {
                bg: hex(0x0d0908),
                card: hex(0x181010),
                card2: hex(0x120c0e),
                text: hex(0xf0e4da),
                text_soft: rgba(229, 187, 183, 0.60),
                muted: rgba(232, 210, 192, 0.72),
                title: hex(0xf5e0d0),
                primary: hex(0xc4523a),
                primary_text: hex(0xffe4dd),
                link: hex(0x69bbff),
                good: hex(0x10b981),
                warn: hex(0xf59e0b),
                bad: hex(0xef4444),
                topbar_border: rgba(188, 108, 101, 0.42),
                topbar_grad_top: rgba(228, 124, 114, 0.08),
                topbar_grad_bottom: rgba(228, 124, 114, 0.02),
                btn_border: rgba(198, 118, 110, 0.46),
                tab_idle_top: rgba(92, 49, 46, 0.74),
                tab_idle_bottom: rgba(63, 34, 36, 0.86),
                tab_active_top: hex(0xc26458),
                tab_active_bottom: hex(0x96453d),
                btn_hover_top: rgba(113, 63, 59, 0.84),
                btn_hover_bottom: rgba(74, 42, 43, 0.94),
                card_grad_top: hex(0x372022),
                card_grad_bottom: hex(0x120c0e),
                table_head_top: hex(0x492b2d),
                table_head_bottom: hex(0x190f12),
                play_text: hex(0xfff2ef),
                play_top: hex(0xd8796d),
                play_bottom: hex(0xa75249),
                play_border: rgba(241, 161, 153, 0.72),
                play_hover_top: hex(0xe48b81),
                footer_top: hex(0x422728),
                footer_bottom: hex(0x150e10),
                border: rgba(188, 108, 101, 0.30),
                bg_grad_start: hex(0x1b1215),
                bg_grad_mid: hex(0x120c0e),
                bg_grad_end: hex(0x0d0908),
                body_font: Font::DEFAULT,
            },
            WuddleTheme::WowUi => ThemeColors {
                bg: hex(0x0a0808),
                card: hex(0x181418),
                card2: hex(0x121418),
                text: hex(0xf0e0c8),
                text_soft: rgba(204, 209, 218, 0.58),
                muted: rgba(232, 214, 186, 0.72),
                title: hex(0xf0dcc0),
                primary: hex(0xc8a040),
                primary_text: hex(0xfff4d8),
                link: hex(0x69bbff),
                good: hex(0x10b981),
                warn: hex(0xf59e0b),
                bad: hex(0xef4444),
                topbar_border: rgba(200, 168, 88, 0.52),
                topbar_grad_top: rgba(114, 118, 126, 0.20),
                topbar_grad_bottom: rgba(57, 61, 68, 0.08),
                btn_border: rgba(212, 173, 86, 0.52),
                tab_idle_top: rgba(138, 28, 28, 0.82),
                tab_idle_bottom: rgba(83, 16, 16, 0.90),
                tab_active_top: hex(0xd1382a),
                tab_active_bottom: hex(0x8c1818),
                btn_hover_top: rgba(166, 35, 35, 0.88),
                btn_hover_bottom: rgba(105, 19, 19, 0.95),
                card_grad_top: hex(0x3e434a),
                card_grad_bottom: hex(0x121418),
                table_head_top: hex(0x4f535a),
                table_head_bottom: hex(0x20242a),
                play_text: hex(0xf8d980),
                play_top: hex(0xe34a37),
                play_bottom: hex(0x9e201e),
                play_border: rgba(221, 183, 93, 0.78),
                play_hover_top: hex(0xee5a44),
                footer_top: hex(0x4a4e54),
                footer_bottom: hex(0x171a1f),
                border: rgba(200, 168, 88, 0.30),
                bg_grad_start: hex(0x1a1820),
                bg_grad_mid: hex(0x121418),
                bg_grad_end: hex(0x0a0808),
                body_font: Font::DEFAULT,
            },
        }
    }

    pub fn to_iced_theme(self) -> Theme {
        let c = self.colors();
        Theme::custom(
            self.label().to_string(),
            Palette {
                background: c.bg,
                text: c.text,
                primary: c.primary,
                success: c.good,
                warning: c.warn,
                danger: c.bad,
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Custom style functions for Iced widgets
// ---------------------------------------------------------------------------

/// Tab button — idle state
pub fn tab_button_style(colors: &ThemeColors) -> button::Style {
    button::Style {
        background: Some(v_gradient(colors.tab_idle_top, colors.tab_idle_bottom)),
        text_color: colors.text,
        border: Border {
            color: colors.btn_border,
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Tab button — active/selected state
pub fn tab_button_active_style(colors: &ThemeColors) -> button::Style {
    button::Style {
        background: Some(v_gradient(colors.tab_active_top, colors.tab_active_bottom)),
        text_color: colors.primary_text,
        border: Border {
            color: rgba_color(243, 190, 128, 0.55),
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Tab button — hovered state
pub fn tab_button_hovered_style(colors: &ThemeColors) -> button::Style {
    button::Style {
        background: Some(v_gradient(colors.btn_hover_top, colors.btn_hover_bottom)),
        text_color: colors.text,
        border: Border {
            color: colors.btn_border,
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Topbar container style — subtle gradient with orange/blue tint
pub fn topbar_style(colors: &ThemeColors) -> container::Style {
    // Horizontal gradient: blue tint on left, orange tint on right (matches Tauri's radial gradients)
    let bg = iced::Background::Gradient(
        Gradient::Linear(
            gradient::Linear::new(Radians(std::f32::consts::FRAC_PI_2 * 3.0))  // left to right
                .add_stop(0.0, Color::from_rgba(0.22, 0.46, 0.67, 0.12))   // blue tint
                .add_stop(0.35, colors.topbar_grad_bottom)                   // fade
                .add_stop(0.65, colors.topbar_grad_top)                      // theme tint
                .add_stop(1.0, Color::from_rgba(0.84, 0.35, 0.11, 0.22)),   // orange tint
        ),
    );
    container::Style {
        background: Some(bg),
        border: Border {
            color: colors.topbar_border,
            width: 0.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow {
            color: rgba_color(0, 0, 0, 0.28),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 24.0,
        },
        text_color: None,
        snap: true,
    }
}

/// Card / panel container style — nearly transparent with border (matches Tauri's see-through cards)
pub fn card_style(colors: &ThemeColors) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
        border: Border {
            color: colors.border,
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        text_color: None,
        snap: true,
    }
}

/// Table head uses a slightly more visible background
pub fn table_card_style(colors: &ThemeColors) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
        border: Border {
            color: colors.border,
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
            offset: Vector::new(0.0, 14.0),
            blur_radius: 34.0,
        },
        text_color: None,
        snap: true,
    }
}

/// Topbar bottom rule/divider
pub fn topbar_rule_style(colors: &ThemeColors) -> rule::Style {
    rule::Style {
        color: colors.topbar_border,
        radius: Radius::new(0.0),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }
}

/// Vertical divider between profile picker and icon buttons
pub fn divider_style(colors: &ThemeColors) -> rule::Style {
    rule::Style {
        color: colors.border,
        radius: Radius::new(0.0),
        fill_mode: rule::FillMode::Percent(60.0),
        snap: true,
    }
}

/// Theme picker button — idle
pub fn theme_button_style(colors: &ThemeColors) -> button::Style {
    button::Style {
        background: Some(iced::Background::Color(blend_over_bg(
            colors.tab_idle_top,
            colors.bg,
        ))),
        text_color: colors.text,
        border: Border {
            color: colors.btn_border,
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Theme picker button — selected
pub fn theme_button_active_style(colors: &ThemeColors) -> button::Style {
    button::Style {
        background: Some(iced::Background::Color(colors.primary)),
        text_color: colors.primary_text,
        border: Border {
            color: rgba_color(243, 190, 128, 0.55),
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Play button style — gradient from play_top to play_bottom
pub fn play_button_style(colors: &ThemeColors) -> button::Style {
    button::Style {
        background: Some(v_gradient(colors.play_top, colors.play_bottom)),
        text_color: colors.play_text,
        border: Border {
            color: colors.play_border,
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow {
            color: rgba_color(0, 0, 0, 0.25),
            offset: Vector::new(0.0, 8.0),
            blur_radius: 20.0,
        },
        snap: true,
    }
}

/// Play button hovered — brighter gradient
pub fn play_button_hovered_style(colors: &ThemeColors) -> button::Style {
    let mut s = play_button_style(colors);
    s.background = Some(v_gradient(colors.play_hover_top, colors.play_bottom));
    s
}

/// Footer bar container — gradient
pub fn footer_style(colors: &ThemeColors) -> container::Style {
    container::Style {
        background: Some(v_gradient(colors.footer_top, colors.footer_bottom)),
        border: Border {
            color: colors.border,
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        text_color: None,
        snap: true,
    }
}

/// Column header (MODS / ADDONS) style — dark bg, bottom border only
pub fn col_header_style(_colors: &ThemeColors) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.20))),
        border: Border {
            color: rgba_color(255, 255, 255, 0.08),
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        text_color: None,
        snap: true,
    }
}

/// Update column container — border + dark overlay
pub fn update_col_style(colors: &ThemeColors) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.20))),
        border: Border {
            color: colors.border,
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        text_color: None,
        snap: true,
    }
}

/// Table header gradient style
pub fn table_head_style(colors: &ThemeColors) -> container::Style {
    container::Style {
        background: Some(iced::Background::Gradient(Gradient::Linear(
            gradient::Linear::new(Radians(std::f32::consts::PI))
                .add_stop(0.0, colors.table_head_top)
                .add_stop(1.0, colors.table_head_bottom),
        ))),
        border: Border {
            color: colors.border,
            width: 0.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        text_color: None,
        snap: true,
    }
}

/// Table row hover style
pub fn row_hover_style(colors: &ThemeColors) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            colors.primary.r,
            colors.primary.g,
            colors.primary.b,
            0.08,
        ))),
        border: Border {
            color: colors.border,
            width: 0.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        text_color: None,
        snap: true,
    }
}

/// Update line separator (thin border between rows)
pub fn update_line_style(_colors: &ThemeColors) -> rule::Style {
    rule::Style {
        color: Color::from_rgba(1.0, 1.0, 1.0, 0.06),
        radius: Radius::new(0.0),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }
}

/// Danger button style (red border/text)
pub fn btn_danger_style(_colors: &ThemeColors) -> button::Style {
    button::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            0.937, 0.267, 0.267, 0.18,
        ))),
        text_color: Color::from_rgb8(254, 202, 202),
        border: Border {
            color: Color::from_rgba(0.937, 0.267, 0.267, 0.52),
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Primary button style (gradient look)
pub fn btn_primary_style(colors: &ThemeColors) -> button::Style {
    button::Style {
        background: Some(iced::Background::Color(colors.primary)),
        text_color: colors.primary_text,
        border: Border {
            color: rgba_color(241, 190, 130, 0.60),
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Dialog scrim (dark overlay) style
pub fn scrim_style() -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.55))),
        border: Border::default(),
        shadow: Shadow::default(),
        text_color: None,
        snap: true,
    }
}

/// Dialog box style (distinct from card_style — uses card gradient, more prominent shadow)
pub fn dialog_style(colors: &ThemeColors) -> container::Style {
    container::Style {
        background: Some(v_gradient(colors.card_grad_top, colors.card_grad_bottom)),
        border: Border {
            color: colors.border,
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
            offset: iced::Vector::new(0.0, 20.0),
            blur_radius: 70.0,
        },
        text_color: None,
        snap: true,
    }
}

/// Log terminal area — dark solid background like a terminal
pub fn log_terminal_style(colors: &ThemeColors) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(hex(0x0a0f16))),
        border: Border {
            color: colors.border,
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow::default(),
        text_color: Some(hex(0xdbe7ff)),
        snap: true,
    }
}

/// Floating context menu style (solid background, border, drop shadow)
pub fn context_menu_style(colors: &ThemeColors) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(colors.card)),
        border: Border {
            color: colors.border,
            width: 1.0,
            radius: Radius::new(0.0),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: iced::Vector::new(2.0, 4.0),
            blur_radius: 12.0,
        },
        text_color: None,
        snap: true,
    }
}

pub fn tooltip_style(colors: &ThemeColors) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(colors.card)),
        border: Border {
            color: colors.border,
            width: 1.0,
            radius: Radius::new(4.0),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: iced::Vector::new(1.0, 2.0),
            blur_radius: 6.0,
        },
        text_color: Some(colors.text),
        snap: true,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn hex(rgb: u32) -> Color {
    Color::from_rgb8(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color::from_rgba8(r, g, b, a)
}

fn rgba_color(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color::from_rgba8(r, g, b, a)
}

/// Create a vertical linear gradient from top to bottom.
fn v_gradient(top: Color, bottom: Color) -> iced::Background {
    iced::Background::Gradient(Gradient::Linear(
        gradient::Linear::new(Radians(std::f32::consts::PI)) // 180deg = top to bottom
            .add_stop(0.0, top)
            .add_stop(1.0, bottom),
    ))
}

/// Blend a semi-transparent color over a solid background.
fn blend_over_bg(fg: Color, bg: Color) -> Color {
    let a = fg.a;
    Color {
        r: fg.r * a + bg.r * (1.0 - a),
        g: fg.g * a + bg.g * (1.0 - a),
        b: fg.b * a + bg.b * (1.0 - a),
        a: 1.0,
    }
}
