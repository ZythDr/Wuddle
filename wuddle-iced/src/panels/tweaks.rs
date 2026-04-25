use iced::widget::{button, checkbox, column, container, row, scrollable, slider, text, text_input, tooltip, Space};
use iced::{Element, Length};

use crate::theme::{self, ThemeColors};
use crate::{App, Message, TweakId};

pub fn view<'a>(app: &'a App, colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;
    let tv = &app.tweak_values;
    let t = &app.tweaks;
    let has_wow_dir = !app.wow_dir.is_empty();
    let selected_exe = app.active_profile().and_then(|profile| profile.auto_launch_exe.as_deref());
    let has_backup = has_wow_dir
        && crate::tweaks::has_backup(std::path::Path::new(&app.wow_dir), selected_exe);
    let tweaks_disabled_reason = app.tweaks_disabled_reason();
    let tweaks_enabled = tweaks_disabled_reason.is_none();
    let tweak_target_name = app
        .tweak_client_info
        .as_ref()
        .map(|info| info.executable_name.clone())
        .or_else(|| selected_exe.map(|name| name.to_string()))
        .unwrap_or_else(|| "WoW.exe".to_string());
    let tweak_target_version = app
        .tweak_client_info
        .as_ref()
        .and_then(|info| info.file_version.as_deref().or(info.product_version.as_deref()))
        .unwrap_or("unknown version");

    let header = row![
        column![
            text("Tweaks").size(18).color(colors.title),
            text("Patch legacy 1.12.1 client executables with quality-of-life improvements.")
                .size(12)
                .color(colors.muted),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
        tip(btn_action("Read Current", tweaks_enabled.then_some(Message::ReadTweaks), c), &format!("Read current tweak values from {}", tweak_target_name), tooltip::Position::Bottom, colors),
        tip(btn("Reset to Default", Message::ResetTweaksToDefault, c), "Reset all sliders to default values", tooltip::Position::Bottom, colors),
        tip(btn_action("Restore", tweaks_enabled.then_some(Message::RestoreTweaks), c), &format!("Restore {} from backup", tweak_target_name), tooltip::Position::Bottom, colors),
        tip(btn_primary_action("Apply", tweaks_enabled.then_some(Message::ApplyTweaks), c), &format!("Patch {} with selected tweaks (creates backup first)", tweak_target_name), tooltip::Position::Bottom, colors),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    let hint: Element<Message> = if let Some(reason) = tweaks_disabled_reason {
        text(reason)
            .size(13)
            .color(colors.warn)
            .into()
    } else {
        let support_label = format!(
            "Tweaks target: {}  ·  Detected version: {}  ·  Compatible with vanilla-tweaks",
            tweak_target_name,
            tweak_target_version
        );
        let backup_label = if has_backup {
            format!("WoW directory: {}  ·  Backup: {}.bak ✓", app.wow_dir, tweak_target_name)
        } else {
            format!("WoW directory: {}  ·  No backup yet — Apply to create one", app.wow_dir)
        };
        column![
            text(support_label).size(13).color(colors.good),
            text(backup_label)
                .size(13)
                .color(if has_backup { colors.good } else { colors.muted }),
        ]
        .spacing(2)
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
        c,
    );

    // Camera section — height(Fill) stretches to match Rendering's natural height.
    let camera = settings_card_fill(
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
        c,
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
        c,
    );

    // System section — height(Fill) stretches to match Audio's natural height.
    let system = settings_card_fill(
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
        c,
    );

    let footnote = text("* Raising this option too high can result in a severe loss of FPS/performance.")
        .size(16)
        .color(colors.muted);

    scrollable(
        column![
            header,
            hint,
            row![rendering, camera].spacing(8).width(Length::Fill),
            row![audio, system].spacing(8).width(Length::Fill),
            footnote,
        ]
        .spacing(8)
        .width(Length::Fill),
    )
    .height(Length::Fill)
    .direction(theme::vscroll())
    .style(move |t, s| theme::scrollable_style(c)(t, s))
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
    colors: ThemeColors,
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
    _colors: ThemeColors,
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
    colors: ThemeColors,
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
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;
    container(container(content).padding(16))
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(c))
        .into()
}

/// Like settings_card but fills the cross-axis height of its Row parent,
/// so sibling cards in the same row always match the tallest card's height.
fn settings_card_fill<'a>(
    content: impl Into<Element<'a, Message>>,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;
    container(container(content).padding(16))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| theme::card_style(c))
        .into()
}

/// Wrap any element in a tooltip with consistent styling.
fn tip<'a>(content: impl Into<Element<'a, Message>>, tip_text: &str, pos: tooltip::Position, colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;
    let tip_str = String::from(tip_text);
    tooltip(
        content,
        container(text(tip_str).size(13).color(c.text))
            .padding([3, 8])
            .style(move |_theme| theme::tooltip_style(c)),
        pos,
    )
    .gap(4.0)
    .into()
}

fn btn<'a>(label: &str, msg: Message, colors: ThemeColors) -> Element<'a, Message> {
    btn_action(label, Some(msg), colors)
}

fn btn_action<'a>(label: &str, msg: Option<Message>, colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;
    let mut button = button(text(String::from(label)).size(13));
    if let Some(ref message) = msg {
        button = button.on_press(message.clone());
    }
    button
        .padding([6, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered if msg.is_some() => theme::tab_button_hovered_style(c),
            button::Status::Pressed if msg.is_some() => theme::tab_button_active_style(c),
            _ if msg.is_some() => theme::tab_button_style(c),
            _ => {
                let mut style = theme::tab_button_style(c);
                style.text_color = c.muted;
                style
            }
        })
        .into()
}

fn btn_primary_action<'a>(label: &str, msg: Option<Message>, colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;
    let mut button = button(text(String::from(label)).size(13));
    if let Some(ref message) = msg {
        button = button.on_press(message.clone());
    }
    button
        .padding([6, 12])
        .style(move |_theme, _status| {
            if msg.is_some() {
                theme::tab_button_active_style(c)
            } else {
                let mut style = theme::tab_button_style(c);
                style.text_color = c.muted;
                style
            }
        })
        .into()
}
