use iced::gradient::Linear;
use iced::widget::{
    button, column, container, image, mouse_area, row, rule, scrollable, stack, text, tooltip,
    Space,
};
use iced::{Color, Element, Gradient, Length, Radians};

use crate::anchored_overlay::AnchoredOverlay;
use crate::service::is_mod;
use crate::theme::{self, ThemeColors};
use crate::{App, Dialog, Message};

// Turtle WoW official links
const URL_HOMEPAGE: &str = "https://turtlecraft.gg/";
const URL_DATABASE: &str = "https://database.turtlecraft.gg/";
const URL_FORUM: &str = "https://forum.turtlecraft.gg/";
const URL_TALENTS: &str = "https://talents.turtlecraft.gg/";
const URL_DISCORD: &str = "https://discord.gg/turtlewow";
const URL_ARMORY: &str = "https://turtlecraft.gg/armory";

// Community links
const URL_ADDONS: &str = "https://turtle-wow.fandom.com/wiki/Addons#Full_Addons_List";
const URL_TURTLETIMERS: &str = "https://turtletimers.com/";
const URL_RETROCRO: &str = "https://github.com/RetroCro/TurtleWoW-Mods";
const URL_WOWAUCTIONS: &str = "https://www.wowauctions.net/";
const URL_RAIDRES: &str = "https://raidres.top/";
const URL_TURTLOGS: &str = "https://www.turtlogs.com/";

pub fn turtle_artwork() -> &'static image::Handle {
    static HANDLE: std::sync::OnceLock<image::Handle> = std::sync::OnceLock::new();
    HANDLE.get_or_init(|| {
        image::Handle::from_bytes(&include_bytes!("../../assets/artwork/turtle-bg.jpg")[..])
    })
}

pub fn view<'a>(app: &'a App, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;

    let update_count = app
        .plans
        .iter()
        .filter(|p| p.has_update && !app.ignored_update_ids.contains(&p.repo_id))
        .count();

    // --- Updates card header ---
    let header = row![
        text("Updates").size(18).color(colors.title),
        Space::new().width(Length::Fill),
        {
            let c2 = c;
            let menu_open = app.add_new_menu_open;
            let add_btn = button(text("+ Add new").size(13))
                .on_press(Message::ToggleAddNewMenu)
                .padding([6, 14])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => {
                        if menu_open {
                            theme::tab_button_active_style(&c2)
                        } else {
                            theme::tab_button_style(&c2)
                        }
                    }
                });
            let menu_items: Element<Message> = container(
                column![
                    add_new_menu_item(
                        "Add Mod",
                        Message::OpenDialog(Dialog::AddRepo {
                            url: String::new(),
                            mode: String::from("auto"),
                            is_addons: false,
                            advanced: false,
                        }),
                        &c,
                    ),
                    add_new_menu_item(
                        "Add Addon",
                        Message::OpenDialog(Dialog::AddRepo {
                            url: String::new(),
                            mode: String::from("addon_git"),
                            is_addons: true,
                            advanced: false,
                        }),
                        &c,
                    ),
                ]
                .spacing(2),
            )
            .padding(6)
            .width(160)
            .style(move |_theme| theme::context_menu_style(&c2))
            .into();
            let overlay: Element<Message> = AnchoredOverlay::new(add_btn, menu_items, menu_open)
                .on_dismiss(Message::CloseMenu)
                .into();
            overlay
        },
        separator(&c),
        tip(
            btn_styled("Check for updates", Message::CheckUpdates, &c),
            "Fetch the latest versions for all addons and mods",
            tooltip::Position::Bottom,
            colors,
        ),
        tip(
            btn_styled(
                &format!("Update All ({})", update_count),
                Message::UpdateAll,
                &c,
            ),
            "Download and install all available updates",
            tooltip::Position::Bottom,
            colors,
        ),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    // MODS column
    let mod_plans: Vec<_> = app
        .plans
        .iter()
        .filter(|p| {
            p.has_update
                && !app.ignored_update_ids.contains(&p.repo_id)
                && app.repos.iter().any(|r| r.id == p.repo_id && is_mod(r))
        })
        .collect();

    let mods_header = {
        let c2 = c;
        container(
            row![
                text("MODS").size(12).color(colors.muted),
                Space::new().width(Length::Fill),
                text(format!("{}", mod_plans.len()))
                    .size(12)
                    .color(colors.text),
            ]
            .padding([6, 8]),
        )
        .width(Length::Fill)
        .style(move |_theme| theme::col_header_style(&c2))
    };

    let mods_body: Element<Message> = if mod_plans.is_empty() {
        scrollable(
            container(text("No mod updates.").size(13).color(colors.muted))
                .padding([12, 8])
                .width(Length::Fill),
        )
        .height(Length::Fill)
        .direction(theme::vscroll())
        .style(move |t, s| theme::scrollable_style(&c)(t, s))
        .into()
    } else {
        let lines: Vec<Element<Message>> = mod_plans
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let c2 = c;
                let mut col_items: Vec<Element<Message>> = Vec::new();
                if i > 0 {
                    col_items.push(
                        rule::horizontal(1)
                            .style(move |_theme| theme::update_line_style(&c2))
                            .into(),
                    );
                }
                col_items.push(
                    text(&p.name)
                        .size(13)
                        .color(colors.text)
                        .width(Length::Fill)
                        .into(),
                );
                container(column(col_items)).padding([4, 8]).into()
            })
            .collect();
        scrollable(column(lines).width(Length::Fill))
            .height(Length::Fill)
            .direction(theme::vscroll())
            .style(move |t, s| theme::scrollable_style(&c)(t, s))
            .into()
    };

    let mods_col = {
        let c2 = c;
        container(
            column![mods_header, mods_body]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::FillPortion(1))
        .height(Length::Fill)
        .style(move |_theme| theme::update_col_style(&c2))
    };

    // ADDONS column
    let addon_plans: Vec<_> = app
        .plans
        .iter()
        .filter(|p| {
            p.has_update
                && !app.ignored_update_ids.contains(&p.repo_id)
                && app.repos.iter().any(|r| r.id == p.repo_id && !is_mod(r))
        })
        .collect();

    let addons_header = {
        let c2 = c;
        container(
            row![
                text("ADDONS").size(12).color(colors.muted),
                Space::new().width(Length::Fill),
                text(format!("{}", addon_plans.len()))
                    .size(12)
                    .color(colors.text),
            ]
            .padding([6, 8]),
        )
        .width(Length::Fill)
        .style(move |_theme| theme::col_header_style(&c2))
    };

    let addons_body: Element<Message> = if addon_plans.is_empty() {
        scrollable(
            container(text("No addon updates.").size(13).color(colors.muted))
                .padding([12, 8])
                .width(Length::Fill),
        )
        .height(Length::Fill)
        .direction(theme::vscroll())
        .style(move |t, s| theme::scrollable_style(&c)(t, s))
        .into()
    } else {
        let lines: Vec<Element<Message>> = addon_plans
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let c2 = c;
                let mut col_items: Vec<Element<Message>> = Vec::new();
                if i > 0 {
                    col_items.push(
                        rule::horizontal(1)
                            .style(move |_theme| theme::update_line_style(&c2))
                            .into(),
                    );
                }
                col_items.push(
                    text(&p.name)
                        .size(13)
                        .color(colors.text)
                        .width(Length::Fill)
                        .into(),
                );
                container(column(col_items)).padding([4, 8]).into()
            })
            .collect();
        scrollable(column(lines).width(Length::Fill))
            .height(Length::Fill)
            .direction(theme::vscroll())
            .style(move |t, s| theme::scrollable_style(&c)(t, s))
            .into()
    };

    let addons_col = {
        let c2 = c;
        container(
            column![addons_header, addons_body]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::FillPortion(1))
        .height(Length::Fill)
        .style(move |_theme| theme::update_col_style(&c2))
    };

    let updates_row = row![mods_col, addons_col].spacing(12).height(200);

    let updates_card = {
        let c2 = c;
        container(column![header, updates_row].spacing(12).padding(18))
            .width(Length::Fill)
            .style(move |_theme| theme::card_style(&c2))
    };

    // Show Turtle WoW links only when the active profile has "I like turtles!" enabled
    let like_turtles = app
        .profiles
        .iter()
        .find(|p| p.id == app.active_profile_id)
        .map(|p| p.like_turtles)
        .unwrap_or(true);

    let mut page_items: Vec<Element<Message>> = vec![updates_card.into()];

    if like_turtles {
        page_items.push(radio_card(app, &c));

        let official_links = column![
            text("Official Links").size(16).color(colors.title),
            link_button("Homepage", URL_HOMEPAGE, &c),
            link_button("Database", URL_DATABASE, &c),
            link_button("Forum", URL_FORUM, &c),
            link_button("Talent Calculator", URL_TALENTS, &c),
            link_button("Join Discord", URL_DISCORD, &c),
            link_button("Armory", URL_ARMORY, &c),
        ]
        .spacing(8)
        .width(Length::Fill)
        .align_x(iced::Alignment::Center);

        let community_links = column![
            text("Useful Community Links").size(16).color(colors.title),
            link_button("AddOns", URL_ADDONS, &c),
            link_button("Turtletimers", URL_TURTLETIMERS, &c),
            link_button("RetroCro Mods Guide", URL_RETROCRO, &c),
            link_button("Wowauctions", URL_WOWAUCTIONS, &c),
            link_button("RaidRes", URL_RAIDRES, &c),
            link_button("Turtlogs", URL_TURTLOGS, &c),
        ]
        .spacing(8)
        .width(Length::Fill)
        .align_x(iced::Alignment::Center);

        let links_card = {
            let c2 = c;
            // Background artwork - explicit height (340px) to prevent truncation of links
            let bg_image = image(turtle_artwork().clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(iced::ContentFit::Cover);

            // Base overlay - 20% opacity everywhere
            let base_overlay = container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Color { a: 0.2, ..c2.card }.into()),
                    ..Default::default()
                });

            // Vertical vignette fringe - adds another 70% at the very top/bottom (total 90%)
            let v_overlay = container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| {
                    let gradient = Linear::new(0.0)
                        .add_stop(0.0, Color { a: 0.7, ..c2.card })
                        .add_stop(0.4, Color::TRANSPARENT)
                        .add_stop(0.6, Color::TRANSPARENT)
                        .add_stop(1.0, Color { a: 0.7, ..c2.card });

                    container::Style {
                        background: Some(iced::Background::Gradient(Gradient::Linear(gradient))),
                        ..Default::default()
                    }
                });

            // Horizontal vignette fringe - adds another 70% at the very left/right (total 90%)
            let h_overlay = container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| {
                    let gradient = Linear::new(Radians::PI / 2.0)
                        .add_stop(0.0, Color { a: 0.7, ..c2.card })
                        .add_stop(0.1, Color::TRANSPARENT)
                        .add_stop(0.9, Color::TRANSPARENT)
                        .add_stop(1.0, Color { a: 0.7, ..c2.card });

                    container::Style {
                        background: Some(iced::Background::Gradient(Gradient::Linear(gradient))),
                        ..Default::default()
                    }
                });

            // Border overlay - must be on top of the image to be visible
            let border_overlay = container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| theme::card_artwork_style(&c2));

            let content = row![official_links, community_links]
                .spacing(16)
                .padding(18)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(iced::Alignment::Center);

            container(
                stack![
                    bg_image,
                    base_overlay,
                    v_overlay,
                    h_overlay,
                    border_overlay,
                    content
                ]
                .width(Length::Fill)
                .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(340)
            .clip(true)
        };

        page_items.push(links_card.into());
    }

    scrollable(
        iced::widget::column(page_items)
            .spacing(10)
            .width(Length::Fill),
    )
    .height(Length::Fill)
    .direction(theme::vscroll())
    .style(move |t, s| theme::scrollable_style(&c)(t, s))
    .into()
}

fn btn_styled<'a>(label: &str, msg: Message, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    button(text(String::from(label)).size(13))
        .on_press(msg)
        .padding([6, 14])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(&c),
            _ => theme::tab_button_style(&c),
        })
        .into()
}

/// Wrap any element in a tooltip with consistent styling.
fn tip<'a>(
    content: impl Into<Element<'a, Message>>,
    tip_text: &str,
    pos: tooltip::Position,
    colors: &ThemeColors,
) -> Element<'a, Message> {
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

fn link_button<'a>(label: &str, url: &str, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    let url_owned = String::from(url);
    let url_tip = format!("Open in browser: {}", url);
    let btn = button(
        container(text(String::from(label)).size(13).color(c.text))
            .width(Length::Fill)
            .center_x(Length::Shrink),
    )
    .on_press(Message::OpenUrl(url_owned))
    .padding([8, 16])
    .width(220)
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::tab_button_hovered_style(&c),
        _ => theme::tab_button_style(&c),
    });
    tooltip(
        btn,
        container(text(url_tip).size(13).color(c.text))
            .padding([3, 8])
            .style(move |_theme| crate::theme::tooltip_style(&c)),
        tooltip::Position::Top,
    )
    .gap(0.0)
    .into()
}

/// Frameless icon button: dim icon color when idle, bright on hover. No background.
fn icon_btn_hover<'a>(
    svg_bytes: &'static [u8],
    size: u32,
    msg: Message,
    idle_color: iced::Color,
    hover_color: iced::Color,
) -> Element<'a, Message> {
    let icon = iced::widget::svg(iced::widget::svg::Handle::from_memory(svg_bytes.to_vec()))
        .width(size)
        .height(size)
        .style(move |_t, status| iced::widget::svg::Style {
            color: Some(match status {
                iced::widget::svg::Status::Hovered => hover_color,
                _ => idle_color,
            }),
        });

    button(icon)
        .on_press(msg)
        .padding([4, 4])
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: idle_color,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        })
        .into()
}

/// Like `icon_btn_hover` but with a fixed button width to prevent layout shift.
fn icon_btn_fixed<'a>(
    svg_bytes: &'static [u8],
    icon_size: u32,
    btn_width: u32,
    msg: Message,
    idle_color: iced::Color,
    hover_color: iced::Color,
) -> Element<'a, Message> {
    let icon = iced::widget::svg(iced::widget::svg::Handle::from_memory(svg_bytes.to_vec()))
        .width(icon_size)
        .height(icon_size)
        .style(move |_t, status| iced::widget::svg::Style {
            color: Some(match status {
                iced::widget::svg::Status::Hovered => hover_color,
                _ => idle_color,
            }),
        });

    button(container(icon).center_x(Length::Fill))
        .on_press(msg)
        .width(btn_width)
        .padding([4, 4])
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: idle_color,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        })
        .into()
}

/// Choose the volume icon based on current level (0.0–1.0).
fn volume_icon_bytes(volume: f32) -> &'static [u8] {
    if volume <= 0.0 {
        include_bytes!("../../assets/icons/volume-mute.svg")
    } else if volume <= 0.25 {
        include_bytes!("../../assets/icons/volume-off.svg")
    } else if volume <= 0.50 {
        include_bytes!("../../assets/icons/volume-low.svg")
    } else if volume <= 0.75 {
        include_bytes!("../../assets/icons/volume-medium.svg")
    } else {
        include_bytes!("../../assets/icons/volume-high.svg")
    }
}

fn radio_card<'a>(app: &'a App, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;

    let is_live = app.radio_playing && app.radio_handle.is_some();
    let is_connecting = app.radio_connecting;

    // LIVE indicator colors — green when live, dim gray when not.
    let good = colors.good;
    let (live_text_color, grad_transparent, grad_end) = if is_live {
        (
            good,
            iced::Color { a: 0.0, ..good },
            iced::Color { a: 0.25, ..good },
        )
    } else {
        (
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.25),
            iced::Color::TRANSPARENT,
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.07),
        )
    };

    // Gradient wash that fills the full card height.
    let live_right = container(
        container(text("● LIVE").size(16).color(live_text_color))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding {
                top: 0.0,
                right: 16.0,
                bottom: 0.0,
                left: 0.0,
            })
            .align_x(iced::Alignment::End)
            .align_y(iced::Alignment::Center),
    )
    .width(Length::FillPortion(1))
    .height(Length::Fill)
    .padding(iced::Padding {
        top: 1.0,
        right: 1.0,
        bottom: 1.0,
        left: 0.0,
    })
    .style(move |_| container::Style {
        background: Some(iced::Background::Gradient(iced::Gradient::Linear(
            iced::gradient::Linear::new(iced::Radians(std::f32::consts::PI / 2.0))
                .add_stop(0.0, grad_transparent)
                .add_stop(0.5, grad_transparent)
                .add_stop(1.0, grad_end),
        ))),
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
        text_color: None,
        snap: true,
    });

    // Left column: title + subtitle
    let left_col = column![
        text("Everlook Broadcasting Co.")
            .size(15)
            .color(colors.title),
        text("Turtle WoW in-game radio stream")
            .size(11)
            .color(colors.muted),
    ]
    .spacing(2);

    // --- Icon colors ---
    // Blend a hint of the theme accent into the grays so icons feel cohesive.
    let accent = c.primary;
    let mix = |gray: f32, a: f32| -> iced::Color {
        // 80% gray + 20% accent
        iced::Color::from_rgba(
            gray * 0.80 + accent.r * 0.20,
            gray * 0.80 + accent.g * 0.20,
            gray * 0.80 + accent.b * 0.20,
            a,
        )
    };
    let dim = mix(0.55, 0.55);
    let extra_dim = mix(0.50, 0.40);
    let hover_bright = mix(0.92, 1.0);
    let active_bright = mix(0.88, 1.0);

    // Settings (cogwheel)
    let settings_btn = tip(
        icon_btn_hover(
            include_bytes!("../../assets/icons/cogwheel.svg"),
            20,
            Message::OpenRadioSettings,
            dim,
            hover_bright,
        ),
        "Radio settings",
        tooltip::Position::Top,
        colors,
    );

    // Refresh / reconnect — sits left of play/stop
    let refresh_btn = tip(
        icon_btn_hover(
            include_bytes!("../../assets/icons/refresh.svg"),
            18,
            Message::ReconnectRadio,
            dim,
            hover_bright,
        ),
        "Reconnect to the radio stream",
        tooltip::Position::Top,
        colors,
    );

    // Play / Stop / Connecting spinner — all use a fixed 44px button width
    // so the layout never shifts between states.
    let play_stop_btn: Element<Message> = if is_connecting {
        let spinner_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let frame = spinner_frames[app.spinner_tick % spinner_frames.len()];
        tip(
            button(
                container(text(format!("{}", frame)).size(36).color(dim)).center_x(Length::Fill),
            )
            .width(44)
            .padding([4, 4])
            .style(move |_theme, _status| button::Style {
                background: None,
                text_color: dim,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            }),
            "Connecting to radio stream…",
            tooltip::Position::Top,
            colors,
        )
    } else if app.radio_playing {
        tip(
            icon_btn_fixed(
                include_bytes!("../../assets/icons/stop.svg"),
                36,
                44,
                Message::ToggleRadio,
                active_bright,
                hover_bright,
            ),
            "Stop the radio stream",
            tooltip::Position::Top,
            colors,
        )
    } else {
        tip(
            icon_btn_fixed(
                include_bytes!("../../assets/icons/play.svg"),
                36,
                44,
                Message::ToggleRadio,
                dim,
                hover_bright,
            ),
            if app.radio_handle.is_some() {
                "Play"
            } else {
                "Tune in to the radio stream"
            },
            tooltip::Position::Top,
            colors,
        )
    };

    // Volume: [−] [speaker] [+]
    let vol = app.radio_volume;
    let vol_down = (vol - 0.05).clamp(0.0, 1.0);
    let vol_up = (vol + 0.05).clamp(0.0, 1.0);

    let minus_btn = icon_btn_hover(
        include_bytes!("../../assets/icons/minus.svg"),
        10,
        Message::SetRadioVolume(vol_down),
        extra_dim,
        hover_bright,
    );

    // Volume icon: bright when muted (to draw attention), dim otherwise
    let is_muted = vol <= 0.0;
    let vol_idle_color = if is_muted { active_bright } else { dim };
    let vol_icon_bytes = volume_icon_bytes(vol);
    let vol_icon = iced::widget::svg(iced::widget::svg::Handle::from_memory(
        vol_icon_bytes.to_vec(),
    ))
    .width(36)
    .height(36)
    .style(move |_t, status| iced::widget::svg::Style {
        color: Some(match status {
            iced::widget::svg::Status::Hovered => hover_bright,
            _ => vol_idle_color,
        }),
    });

    // Wrap in button for click-to-mute, then mouse_area for scroll-to-adjust
    let vol_btn = button(vol_icon)
        .on_press(Message::ToggleRadioMute)
        .padding([2, 2])
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: vol_idle_color,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        });

    let vol_icon_scrollable = mouse_area(vol_btn).on_scroll(move |delta| {
        let step: f32 = match delta {
            iced::mouse::ScrollDelta::Lines { y, .. } => y * 0.05,
            iced::mouse::ScrollDelta::Pixels { y, .. } => y * 0.005,
        };
        Message::SetRadioVolume((vol + step).clamp(0.0, 1.0))
    });

    let vol_pct = format!("{}%", (vol * 100.0).round() as u32);
    let mute_hint = if is_muted {
        "Click to unmute"
    } else {
        "Click to mute"
    };
    let vol_icon_with_tip = tip(
        vol_icon_scrollable,
        &format!("Volume: {} — {} — scroll to adjust", vol_pct, mute_hint),
        tooltip::Position::Top,
        colors,
    );

    let plus_btn = icon_btn_hover(
        include_bytes!("../../assets/icons/plus.svg"),
        10,
        Message::SetRadioVolume(vol_up),
        extra_dim,
        hover_bright,
    );

    // Left half: title + [⚙ ↻], right-aligned so controls sit near center
    let left_half = row![
        container(left_col)
            .width(Length::Fill)
            .padding(iced::Padding {
                top: 12.0,
                right: 0.0,
                bottom: 12.0,
                left: 16.0
            }),
        row![settings_btn, refresh_btn]
            .spacing(2)
            .align_y(iced::Alignment::Center),
        Space::new().width(20),
    ]
    .align_y(iced::Alignment::Center);

    // Right half: [− 🔊 +] + LIVE, left-aligned so controls sit near center
    let right_half = row![
        Space::new().width(20),
        row![minus_btn, vol_icon_with_tip, plus_btn]
            .spacing(2)
            .align_y(iced::Alignment::Center),
        live_right,
    ]
    .align_y(iced::Alignment::Center);

    // Main row: [left_half (fill)] [▶/■] [right_half (fill)]
    // Equal FillPortion on both sides centers the play button in the card.
    let main_row = row![
        container(left_half)
            .width(Length::FillPortion(1))
            .align_y(iced::Alignment::Center),
        container(play_stop_btn)
            .width(Length::Shrink)
            .align_y(iced::Alignment::Center),
        container(right_half)
            .width(Length::FillPortion(1))
            .align_y(iced::Alignment::Center),
    ]
    .align_y(iced::Alignment::Center);

    let mut card_col = column![main_row];
    if let Some(e) = &app.radio_error {
        card_col = card_col.push(
            container(
                text(format!("Could not connect: {e}"))
                    .size(11)
                    .color(colors.bad),
            )
            .padding(iced::Padding {
                top: 0.0,
                right: 16.0,
                bottom: 8.0,
                left: 16.0,
            }),
        );
    }

    container(card_col)
        .width(Length::Fill)
        .style(move |_| theme::card_style(&c))
        .into()
}

fn separator<'a>(colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    container(Space::new().width(1).height(24))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(c.border)),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            text_color: None,
            snap: true,
        })
        .width(1)
        .into()
}

fn add_new_menu_item<'a>(label: &str, msg: Message, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    button(text(String::from(label)).size(13))
        .on_press(msg)
        .padding([6, 14])
        .width(Length::Fill)
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(&c),
            _ => button::Style {
                background: None,
                text_color: c.text,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
        })
        .into()
}
