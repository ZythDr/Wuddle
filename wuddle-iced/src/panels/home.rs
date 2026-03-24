use iced::widget::{button, checkbox, column, container, mouse_area, row, rule, scrollable, slider, text, tooltip, Space};
use iced::{Element, Length};

use crate::anchored_overlay::AnchoredOverlay;
use crate::theme::{self, ThemeColors};
use crate::{is_mod, App, Dialog, Message};

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

pub fn view<'a>(app: &'a App, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;

    let update_count = app.plans.iter().filter(|p| p.has_update && !app.ignored_update_ids.contains(&p.repo_id)).count();

    // --- Updates card header ---
    let header = row![
        text("Updates").size(18).color(colors.title),
        Space::new().width(Length::Fill),
        {
            let c2 = c;
            let menu_open = app.add_new_menu_open;
            let add_btn = button(text("+ Add new \u{25BE}").size(13)) // ▾
                .on_press(Message::ToggleAddNewMenu)
                .padding([6, 14])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => if menu_open { theme::tab_button_active_style(&c2) } else { theme::tab_button_style(&c2) },
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
        btn_styled("Check for updates", Message::CheckUpdates, &c),
        btn_styled(
            &format!("Update All ({})", update_count),
            Message::UpdateAll,
            &c,
        ),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    // MODS column
    let mod_plans: Vec<_> = app
        .plans
        .iter()
        .filter(|p| {
            p.has_update && !app.ignored_update_ids.contains(&p.repo_id) && app.repos.iter().any(|r| r.id == p.repo_id && is_mod(r))
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
            p.has_update && !app.ignored_update_ids.contains(&p.repo_id) && app.repos.iter().any(|r| r.id == p.repo_id && !is_mod(r))
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

    let updates_row = row![mods_col, addons_col]
        .spacing(12)
        .height(200);

    let updates_card = {
        let c2 = c;
        container(
            column![header, updates_row].spacing(12).padding(18),
        )
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(&c2))
    };

    // Show Turtle WoW links only when the active profile has "I like turtles!" enabled
    let like_turtles = app.profiles.iter()
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
            container(row![official_links, community_links].spacing(16).padding(18))
                .width(Length::Fill)
                .style(move |_theme| theme::card_style(&c2))
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
        container(text(url_tip).size(11).color(c.text))
            .padding([3, 8])
            .style(move |_theme| crate::theme::tooltip_style(&c)),
        tooltip::Position::Top,
    )
    .gap(0.0)
    .into()
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

    // Gradient wash that fills the full card height. The card has no outer padding;
    // top/bottom padding is carried by the left-col and center containers instead,
    // so live_right's height(Fill) reaches the card's actual top and bottom borders.
    // 1px inset on top/right/bottom avoids overlapping the card border stroke.
    let live_right = container(
        container(text("● LIVE").size(16).color(live_text_color))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding { top: 0.0, right: 16.0, bottom: 0.0, left: 0.0 })
            .align_x(iced::Alignment::End)
            .align_y(iced::Alignment::Center),
    )
    .width(Length::FillPortion(1))
    .height(Length::Fill)
    .padding(iced::Padding { top: 1.0, right: 1.0, bottom: 1.0, left: 0.0 })
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

    // Left column: title + subtitle + auto-connect checkbox
    let c3 = c;
    let auto_connect_cb = tooltip(
        checkbox(app.radio_auto_connect)
            .label("Auto-connect")
            .text_size(12)
            .on_toggle(Message::ToggleRadioAutoConnect),
        container(
            text("Pre-connect silently so Play is instant").size(11).color(c3.text),
        )
        .padding([3, 8])
        .style(move |_| crate::theme::tooltip_style(&c3)),
        tooltip::Position::Top,
    )
    .gap(4.0);

    let left_col = column![
        text("Everlook Broadcasting Co.").size(15).color(colors.title),
        text("Turtle WoW in-game radio stream").size(11).color(colors.muted),
        Space::new().height(4),
        auto_connect_cb,
    ]
    .spacing(2);

    // Center: Play/Stop button + volume slider
    let btn_label = if is_connecting {
        "Connecting..."
    } else if app.radio_playing {
        "■  Stop"
    } else if app.radio_handle.is_some() {
        "▶  Play"
    } else {
        "▶  Tune In"
    };

    let c2 = c;
    let play_btn = button(text(btn_label).size(14))
        .on_press(Message::ToggleRadio)
        .padding([10, 22])
        .style(move |_theme, status| {
            if is_live {
                match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => theme::tab_button_active_style(&c2),
                }
            } else {
                match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => theme::tab_button_style(&c2),
                }
            }
        });

    let c4 = c;
    let vol_slider = slider(0.0_f32..=1.0_f32, app.radio_volume, Message::SetRadioVolume)
        .step(0.01_f32)
        .width(150)
        .style(move |_theme, status| {
            use iced::widget::slider::{Rail, Status, Style};
            let active = c4.primary;
            let handle_color = match status {
                Status::Hovered | Status::Dragged => active,
                Status::Active => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.80),
            };
            Style {
                rail: Rail {
                    backgrounds: (
                        iced::Background::Color(active),
                        iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.12)),
                    ),
                    width: 5.0,
                    border: iced::Border::default(),
                },
                handle: iced::widget::slider::Handle {
                    shape: iced::widget::slider::HandleShape::Circle { radius: 9.0 },
                    background: iced::Background::Color(handle_color),
                    border_width: 0.0,
                    border_color: iced::Color::TRANSPARENT,
                },
            }
        });

    // Wrap in a padded container so the scroll hitbox is taller than the thin rail
    let vol_scrollable = mouse_area(
        container(vol_slider).padding([10, 0]),
    )
    .on_scroll(move |delta| {
        let step: f32 = match delta {
            iced::mouse::ScrollDelta::Lines { y, .. } => y * 0.02,
            iced::mouse::ScrollDelta::Pixels { y, .. } => y * 0.002,
        };
        Message::SetRadioVolume((app.radio_volume + step).clamp(0.0, 1.0))
    });

    let center_row = row![
        play_btn,
        text("Vol").size(12).color(colors.muted),
        vol_scrollable,
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    // Main row: left and center carry the 12px top/bottom padding so live_right
    // (height Fill) reaches the card's top and bottom borders.
    let main_row = row![
        container(left_col)
            .width(Length::FillPortion(1))
            .padding(iced::Padding { top: 12.0, right: 0.0, bottom: 12.0, left: 16.0 }),
        container(center_row)
            .padding([12, 12])
            .align_y(iced::Alignment::Center),
        live_right,
    ]
    .align_y(iced::Alignment::Center);

    let mut card_col = column![main_row];
    if let Some(e) = &app.radio_error {
        card_col = card_col.push(
            container(text(format!("Could not connect: {e}")).size(11).color(colors.bad))
                .padding(iced::Padding { top: 0.0, right: 16.0, bottom: 8.0, left: 16.0 }),
        );
    }

    // No outer padding — live_right fills edge-to-edge vertically
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
