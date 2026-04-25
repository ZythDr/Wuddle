use iced::widget::{button, column, container, row, rule, scrollable, text, tooltip, Space};
use iced::{Element, Length};

use crate::anchored_overlay::AnchoredOverlay;
use crate::service::is_mod;
use crate::theme::{self, ThemeColors};
use crate::{App, Dialog, Message};

pub fn view<'a>(app: &'a App, colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;

    let update_count = app
        .plans
        .iter()
        .filter(|p| p.has_update && !app.ignored_update_ids.contains(&p.repo_id))
        .count();

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
                    button::Status::Hovered => theme::tab_button_hovered_style(c2),
                    _ => {
                        if menu_open {
                            theme::tab_button_active_style(c2)
                        } else {
                            theme::tab_button_style(c2)
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
                        c,
                    ),
                    add_new_menu_item(
                        "Add Addon",
                        Message::OpenDialog(Dialog::AddRepo {
                            url: String::new(),
                            mode: String::from("addon_git"),
                            is_addons: true,
                            advanced: false,
                        }),
                        c,
                    ),
                ]
                .spacing(2),
            )
            .padding(6)
            .width(160)
            .style(move |_theme| theme::context_menu_style(c2))
            .into();
            let overlay: Element<Message> = AnchoredOverlay::new(add_btn, menu_items, menu_open)
                .on_dismiss(Message::CloseMenu)
                .into();
            overlay
        },
        separator(c),
        tip(
            btn_styled("Check for updates", Message::CheckUpdates, c),
            "Fetch the latest versions for all addons and mods",
            tooltip::Position::Bottom,
            colors,
        ),
        tip(
            btn_styled(&format!("Update All ({})", update_count), Message::UpdateAll, c),
            "Download and install all available updates",
            tooltip::Position::Bottom,
            colors,
        ),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let mod_plans: Vec<_> = app
        .plans
        .iter()
        .filter(|p| {
            p.has_update
                && !app.ignored_update_ids.contains(&p.repo_id)
                && app.repos.iter().any(|r| r.id == p.repo_id && is_mod(r))
        })
        .collect();
    let addon_plans: Vec<_> = app
        .plans
        .iter()
        .filter(|p| {
            p.has_update
                && !app.ignored_update_ids.contains(&p.repo_id)
                && app.repos.iter().any(|r| r.id == p.repo_id && !is_mod(r))
        })
        .collect();

    let mods_col = update_column("MODS", &mod_plans, colors, c);
    let addons_col = update_column("ADDONS", &addon_plans, colors, c);

    let updates_row = row![mods_col, addons_col].spacing(12).height(200);

    let updates_card = {
        let c2 = c;
        container(column![header, updates_row].spacing(12).padding(18))
            .width(Length::Fill)
            .style(move |_theme| theme::card_style(c2))
    };

    scrollable(column![updates_card].spacing(10).width(Length::Fill))
        .height(Length::Fill)
        .direction(theme::vscroll())
        .style(move |t, s| theme::scrollable_style(c)(t, s))
        .into()
}

fn update_column<'a>(
    title: &'a str,
    plans: &[&'a crate::service::PlanRow],
    colors: ThemeColors,
    c: ThemeColors,
) -> Element<'a, Message> {
    let header = {
        let c2 = c;
        container(
            row![
                text(title).size(12).color(colors.muted),
                Space::new().width(Length::Fill),
                text(format!("{}", plans.len())).size(12).color(colors.text),
            ]
            .padding([6, 8]),
        )
        .width(Length::Fill)
        .style(move |_theme| theme::col_header_style(c2))
    };

    let body: Element<Message> = if plans.is_empty() {
        scrollable(
            container(text(format!("No {} updates.", title.to_lowercase())).size(13).color(colors.muted))
                .padding([12, 8])
                .width(Length::Fill),
        )
        .height(Length::Fill)
        .direction(theme::vscroll())
        .style(move |t, s| theme::scrollable_style(c)(t, s))
        .into()
    } else {
        let lines: Vec<Element<Message>> = plans
            .iter()
            .enumerate()
            .map(|(i, plan)| {
                let c2 = c;
                let mut col_items: Vec<Element<Message>> = Vec::new();
                if i > 0 {
                    col_items.push(
                        rule::horizontal(1)
                            .style(move |_theme| theme::update_line_style(c2))
                            .into(),
                    );
                }
                col_items.push(
                    text(&plan.name)
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
            .style(move |t, s| theme::scrollable_style(c)(t, s))
            .into()
    };

    let c2 = c;
    container(column![header, body].spacing(0).width(Length::Fill).height(Length::Fill))
        .width(Length::FillPortion(1))
        .height(Length::Fill)
        .style(move |_theme| theme::update_col_style(c2))
        .into()
}

fn btn_styled<'a>(label: &str, msg: Message, colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;
    button(text(String::from(label)).size(13))
        .on_press(msg)
        .padding([6, 14])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(c),
            _ => theme::tab_button_style(c),
        })
        .into()
}

fn tip<'a>(
    content: Element<'a, Message>,
    label: &str,
    position: tooltip::Position,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;
    tooltip(
        content,
        container(text(String::from(label)).size(13).color(c.text))
            .padding([4, 8])
            .style(move |_theme| theme::tooltip_style(c)),
        position,
    )
    .gap(4.0)
    .into()
}

fn separator<'a>(colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;
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

fn add_new_menu_item<'a>(label: &str, msg: Message, colors: ThemeColors) -> Element<'a, Message> {
    let c = colors;
    button(text(String::from(label)).size(13))
        .on_press(msg)
        .padding([6, 14])
        .width(Length::Fill)
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(c),
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
