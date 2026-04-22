//! Simple warning confirmation dialogs:
//! - `super_wow_warning` — anti-virus false-positive warning for SuperWoW
//! - `addon_conflict`    — conflict resolution for duplicate addon sources

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Element, Length};
use iced::{Background, Border, Color};
use crate::{Message, theme};
use crate::components::helpers::close_button;
use theme::ThemeColors;

fn action_banner_style(colors: &ThemeColors, background: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            color: colors.border,
            width: 1.0,
            radius: 0.0.into(),
        },
        shadow: iced::Shadow::default(),
        text_color: None,
        snap: true,
    }
}

fn tree_panel<'a>(
    section_label: impl Into<String>,
    summary: impl Into<String>,
    footer: impl Into<String>,
    footer_background: Color,
    groups: &[(String, Vec<String>)],
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let section_label = section_label.into();
    let summary = summary.into();
    let footer = footer.into();

    let mut content = Vec::<Element<Message>>::new();
    if groups.is_empty() {
        content.push(text("📁 none").size(16).color(c.muted).into());
    } else {
        for (index, (root_label, items)) in groups.iter().enumerate() {
            if index > 0 {
                content.push(Space::new().height(Length::Fixed(8.0)).into());
            }
            content.push(text(format!("📁 {}", root_label)).size(16).color(c.title).into());
            if items.is_empty() {
                content.push(text("  📁 none").size(15).color(c.muted).into());
            } else {
                content.extend(items.iter().map(|item| {
                    text(format!("  📁 {}", item)).size(15).color(c.text).into()
                }));
            }
        }
    }

    container(
        column![
            text(section_label).size(15).color(c.muted),
            container(text(summary).size(14).color(c.muted))
                .height(Length::Fixed(52.0))
                .width(Length::Fill),
            container(
                scrollable(column(content).spacing(4))
                    .width(Length::Fill)
                    .height(Length::Fixed(172.0))
                    .direction(theme::vscroll_overlay())
                    .style(move |t, s| theme::scrollable_style(&c)(t, s)),
            )
            .width(Length::Fill)
            .style(move |_theme| theme::card_style(&c)),
            container(
                text(footer).size(14).color(c.primary_text)
            )
            .padding([6, 10])
            .width(Length::Fill)
            .style(move |_theme| action_banner_style(&c, footer_background)),
        ]
        .spacing(6),
    )
    .padding(10)
    .width(Length::Fill)
    .style(move |_theme| theme::card_style(&c))
    .into()
}

/// Anti-virus warning shown before installing certain mods.
pub fn av_false_positive_warning<'a>(
    url: &'a str,
    mode: &'a str,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    column![
        row![
            text("Anti-Virus Warning").size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(&c),
        ]
        .align_y(iced::Alignment::Center),
        text(
            "This client mod is known to trigger false-positives from anti-virus software.\n\
             Please add your WoW installation directory to your anti-virus' exclusion/whitelist.\n\n\
             If the WoW directory is not added to exclusions in your anti-virus software, \
             the files will be deleted immediately after being downloaded."
        )
        .size(15)
        .color(colors.warn),
        text(
            "If you understand and have whitelisted your WoW installation directory \
             in your anti-virus software, you may continue."
        )
        .size(15)
        .color(colors.text),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(13))
                .on_press(Message::CloseDialog)
                .padding([6, 12])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c),
                    _ => theme::tab_button_style(&c),
                }),
            button(text("I understand, install").size(13).color(colors.bad))
                .on_press(Message::InstallRepoOverride {
                    url: url.to_string(),
                    mode: mode.to_string(),
                })
                .padding([6, 12])
                .style(move |_theme, _status| {
                    let mut s = theme::tab_button_style(&c);
                    s.border.color = c.bad;
                    s
                }),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .into()
}

/// Shown when an addon repo conflicts with an already-installed addon from another source.
pub fn addon_conflict<'a>(
    url: &'a str,
    mode: &'a str,
    conflicts: &'a [wuddle_engine::AddonProbeConflict],
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;

    let conflict_rows: Vec<Element<Message>> = conflicts
        .iter()
        .map(|conflict| {
            text(format!("- {}", conflict.addon_name))
                .size(13)
                .color(colors.warn)
                .into()
        })
        .collect();

    column![
        row![
            text("Addon Conflict").size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(&c),
        ]
        .align_y(iced::Alignment::Center),
        text("The following addons are already installed from other sources:")
            .size(13)
            .color(colors.text),
        column(conflict_rows).spacing(4),
        text(
            "This repository provides the same addons. Would you like to overwrite \
             them and track them via this repository instead?"
        )
        .size(13)
        .color(colors.text),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(13))
                .on_press(Message::CloseDialog)
                .padding([6, 12])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c),
                    _ => theme::tab_button_style(&c),
                }),
            button(text("Overwrite").size(13).color(colors.bad))
                .on_press(Message::InstallRepoOverride {
                    url: url.to_string(),
                    mode: mode.to_string(),
                })
                .padding([6, 12])
                .style(move |_theme, _status| {
                    let mut s = theme::tab_button_style(&c);
                    s.border.color = c.bad;
                    s
                }),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .into()
}

pub fn collection_addon_conflict<'a>(
    repo_id: i64,
    repo_name: &'a str,
    selected_addons: &'a [String],
    conflicts: &'a [wuddle_engine::AddonProbeConflict],
    existing_repos: &'a [crate::service::CollectionConflictOwnerGroup],
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;

    let direct_conflicts = conflicts
        .iter()
        .map(|conflict| conflict.addon_name.clone())
        .collect::<Vec<_>>();

    let old_groups: Vec<(String, Vec<String>)> = if existing_repos.is_empty() {
        vec![("Untracked local folders".to_string(), direct_conflicts.clone())]
    } else {
        existing_repos
            .iter()
            .map(|group| (group.repo_label.clone(), group.addon_names.clone()))
            .collect()
    };

    let new_groups = vec![(repo_name.to_string(), direct_conflicts.clone())];

    let old_summary = if existing_repos.len() == 1 && direct_conflicts.len() == 1 {
        format!(
            "{} currently tracks the addon folder that would be replaced.",
            existing_repos[0].repo_label
        )
    } else if existing_repos.is_empty() {
        "These local addon folders already exist and would be replaced.".to_string()
    } else {
        "These tracked addon folders already exist and would be replaced.".to_string()
    };

    let new_summary = if direct_conflicts.len() == 1 {
        format!(
            "The new collection selection would install this conflicting addon from {}.",
            repo_name
        )
    } else {
        format!(
            "The new collection selection would install these conflicting addons from {}.",
            repo_name
        )
    };

    column![
        row![
            text("Addon Conflict").size(19).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(&c),
        ]
        .align_y(iced::Alignment::Center),
        text(if existing_repos.len() == 1 && direct_conflicts.len() == 1 {
            format!(
                "An existing addon with the same name is already tracked by {}. Replacing it will stop tracking that repo's addon folders listed on the left and install the new conflicting addon shown on the right.",
                existing_repos[0].repo_label
            )
        } else {
            format!(
                "Some addons in '{}' conflict with addon folders that are already tracked. Wuddle can stop tracking and remove the existing folders on the left, then install the conflicting new addons on the right.",
                repo_name
            )
        })
        .size(14)
        .color(colors.text),
        row![
            container(tree_panel(
                "Old",
                &old_summary,
                "REMOVE",
                Color::from_rgba(c.bad.r, c.bad.g, c.bad.b, 0.28),
                &old_groups,
                colors,
            ))
            .width(Length::FillPortion(1)),
            Space::new().width(Length::Fixed(28.0)),
            container(tree_panel(
                "New",
                &new_summary,
                "INSTALL",
                Color::from_rgba(c.good.r, c.good.g, c.good.b, 0.28),
                &new_groups,
                colors,
            ))
            .width(Length::FillPortion(1)),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Start),
        text(
            "Overwriting will stop tracking the existing addon folders shown on the left, remove them from AddOns, and then install and track the conflicting addon folders shown on the right."
        )
        .size(14)
        .color(colors.text),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(14))
                .on_press(Message::CloseDialog)
                .padding([6, 12])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c),
                    _ => theme::tab_button_style(&c),
                }),
            button(text("Overwrite").size(14).color(colors.bad))
                .on_press(Message::SaveCollectionSelectionOverride {
                    repo_id,
                    selected_addons: selected_addons.to_vec(),
                })
                .padding([6, 12])
                .style(move |_theme, _status| {
                    let mut s = theme::tab_button_style(&c);
                    s.border.color = c.bad;
                    s
                }),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .into()
}
