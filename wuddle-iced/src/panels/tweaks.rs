use iced::widget::{button, checkbox, column, container, row, scrollable, slider, text, text_input, Space};
use iced::{Element, Length};

use crate::theme::{self, ThemeColors};
use crate::{App, Message, TweakId};

pub fn view<'a>(app: &'a App, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    let tv = &app.tweak_values;
    let t = &app.tweaks;
    let has_wow_dir = !app.wow_dir.is_empty();

    let header = row![
        column![
            text("Tweaks").size(18).color(colors.title),
            text("Patch WoW.exe with quality-of-life improvements.")
                .size(12)
                .color(colors.muted),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
        btn("Read Current", Message::ReadTweaks, &c),
        btn("Reset to Default", Message::ResetTweaksToDefault, &c),
        btn("Restore", Message::RestoreTweaks, &c),
        btn_primary("Apply", Message::ApplyTweaks, &c),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    let hint: Element<Message> = if !has_wow_dir {
        text("Select a WoW directory in Options to enable tweaks.")
            .size(13)
            .color(colors.warn)
            .into()
    } else {
        text(format!("WoW directory: {}", app.wow_dir))
            .size(13)
            .color(colors.muted)
            .into()
    };

    // Rendering section
    let rendering = settings_card(
        column![
            text("Rendering").size(16).color(colors.title),
            tweak_row_slider(
                "Widescreen FoV",
                "Wider field of view for widescreen monitors (~110 degrees).",
                TweakId::Fov,
                t.fov,
                tv.fov,
                1.0..=2.5,
                0.025,
                |v| Message::SetTweakFov(v),
                format!("{:.2} ({:.0}°)", tv.fov, tv.fov.to_degrees()),
                colors,
            ),
            tweak_row_slider(
                "Farclip (Terrain Distance) *",
                "Maximum terrain render distance.",
                TweakId::Farclip,
                t.farclip,
                tv.farclip,
                777.0..=10000.0,
                1.0,
                |v| Message::SetTweakFarclip(v),
                format!("{:.0}", tv.farclip),
                colors,
            ),
            tweak_row_slider(
                "Frilldistance (Grass Distance) *",
                "Grass and foliage render distance.",
                TweakId::Frilldistance,
                t.frilldistance,
                tv.frilldistance,
                70.0..=1000.0,
                1.0,
                |v| Message::SetTweakFrilldistance(v),
                format!("{:.0}", tv.frilldistance),
                colors,
            ),
            tweak_row_slider(
                "Nameplate Distance",
                "Maximum distance for visible nameplates (yards).",
                TweakId::NameplateDist,
                t.nameplate_dist,
                tv.nameplate_dist,
                20.0..=80.0,
                1.0,
                |v| Message::SetTweakNameplateDist(v),
                format!("{:.0}", tv.nameplate_dist),
                colors,
            ),
        ]
        .spacing(8),
        &c,
    );

    // Camera section
    let camera = settings_card(
        column![
            text("Camera").size(16).color(colors.title),
            tweak_row_check(
                "Camera Skip Fix",
                "Fixes the camera skip/jitter glitch when rotating.",
                TweakId::CameraSkip,
                t.camera_skip,
                colors,
            ),
            tweak_row_input(
                "Max Camera Distance",
                "Override maximum camera zoom-out distance (10-200).",
                TweakId::MaxCameraDist,
                t.max_camera_dist,
                &format!("{:.0}", tv.max_camera_dist),
                |s| Message::SetTweakMaxCameraDist(s),
                colors,
            ),
        ]
        .spacing(8),
        &c,
    );

    // Audio section
    let audio = settings_card(
        column![
            text("Audio").size(16).color(colors.title),
            tweak_row_check(
                "Sound in Background",
                "Keep playing sounds when the game window is not focused.",
                TweakId::SoundBg,
                t.sound_bg,
                colors,
            ),
            tweak_row_input(
                "Sound Channels",
                "Number of simultaneous sound channels (1-999).",
                TweakId::SoundChannels,
                t.sound_channels,
                &format!("{}", tv.sound_channels),
                |s| Message::SetTweakSoundChannels(s),
                colors,
            ),
        ]
        .spacing(8),
        &c,
    );

    // System section
    let system = settings_card(
        column![
            text("System").size(16).color(colors.title),
            tweak_row_check(
                "Quickloot (Reverse)",
                "Auto-loot by default; hold Shift for manual loot window.",
                TweakId::Quickloot,
                t.quickloot,
                colors,
            ),
            tweak_row_check(
                "Large Address Aware",
                "Allow WoW.exe to use up to 4 GB of memory (recommended).",
                TweakId::LargeAddress,
                t.large_address,
                colors,
            ),
        ]
        .spacing(8),
        &c,
    );

    let footnote = text("* Raising this option too high can result in a severe loss of FPS/performance.")
        .size(12)
        .color(colors.muted);

    scrollable(
        column![header, hint, rendering, camera, audio, system, footnote]
            .spacing(8)
            .width(Length::Fill),
    )
    .height(Length::Fill)
    .into()
}

/// Tweak row with checkbox + slider + value display
fn tweak_row_slider<'a, F>(
    name: &str,
    desc: &str,
    id: TweakId,
    checked: bool,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    on_change: F,
    value_display: String,
    colors: &ThemeColors,
) -> Element<'a, Message>
where
    F: 'a + Fn(f32) -> Message,
{
    column![
        checkbox(checked)
            .label(String::from(name))
            .on_toggle(move |b| Message::ToggleTweak(id, b)),
        row![
            slider(range, value, on_change).step(step).width(Length::Fill),
            text(value_display)
                .size(12)
                .color(colors.muted)
                .width(80),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        text(String::from(desc))
            .size(12)
            .color(iced::Color::from_rgba8(200, 200, 200, 0.6)),
    ]
    .spacing(2)
    .into()
}

/// Tweak row with checkbox only (boolean tweaks)
fn tweak_row_check<'a>(
    name: &str,
    desc: &str,
    id: TweakId,
    checked: bool,
    _colors: &ThemeColors,
) -> Element<'a, Message> {
    column![
        checkbox(checked)
            .label(String::from(name))
            .on_toggle(move |b| Message::ToggleTweak(id, b)),
        text(String::from(desc))
            .size(12)
            .color(iced::Color::from_rgba8(200, 200, 200, 0.6)),
    ]
    .spacing(2)
    .into()
}

/// Tweak row with checkbox + text input for numeric value
fn tweak_row_input<'a, F>(
    name: &str,
    desc: &str,
    id: TweakId,
    checked: bool,
    value_str: &str,
    on_change: F,
    colors: &ThemeColors,
) -> Element<'a, Message>
where
    F: 'a + Fn(String) -> Message,
{
    column![
        checkbox(checked)
            .label(String::from(name))
            .on_toggle(move |b| Message::ToggleTweak(id, b)),
        row![
            text_input("", value_str)
                .on_input(on_change)
                .width(80)
                .padding([6, 8]),
            text(String::from(desc))
                .size(12)
                .color(colors.muted),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(2)
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

fn btn_primary<'a>(label: &str, msg: Message, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    button(text(String::from(label)).size(13))
        .on_press(msg)
        .padding([6, 12])
        .style(move |_theme, _status| theme::tab_button_active_style(&c))
        .into()
}
