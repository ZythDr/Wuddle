use iced::widget::{button, column, container, row, rule, scrollable, text, Space};
use iced::{Element, Length};

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

    let update_count = app.plans.iter().filter(|p| p.has_update).count();

    // --- Updates card header ---
    let header = row![
        text("Updates").size(18).color(colors.title),
        Space::new().width(Length::Fill),
        {
            let c2 = c;
            button(text("+ Add new").size(13))
                .on_press(Message::OpenDialog(Dialog::AddRepo {
                    url: String::new(),
                    mode: String::from("auto"),
                }))
                .padding([6, 14])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => theme::tab_button_style(&c2),
                })
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
            p.has_update && app.repos.iter().any(|r| r.id == p.repo_id && is_mod(r))
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
            p.has_update && app.repos.iter().any(|r| r.id == p.repo_id && !is_mod(r))
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

    // --- Turtle links card ---
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

    let links_row = row![official_links, community_links].spacing(16);

    let links_card = {
        let c2 = c;
        container(links_row.padding(18))
            .width(Length::Fill)
            .style(move |_theme| theme::card_style(&c2))
    };

    scrollable(
        column![updates_card, links_card]
            .spacing(10)
            .width(Length::Fill),
    )
    .height(Length::Fill)
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
    button(
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
    })
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
