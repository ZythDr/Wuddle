//! Simple warning confirmation dialogs:
//! - `super_wow_warning` — anti-virus false-positive warning for SuperWoW
//! - `addon_conflict`    — conflict resolution for duplicate addon sources

use crate::components::helpers::{close_button, forge_svg_handle, tip};
use crate::{theme, FileConflictAction, Message};
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Background, Border, Color};
use iced::{Element, Length};
use theme::ThemeColors;

fn action_banner_style(colors: ThemeColors, background: Color) -> iced::widget::container::Style {
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
    panel_background: Color,
    groups: Vec<(String, Vec<(String, bool)>)>, // (root_label, Vec<(item_name, is_dir)>)
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;
    let section_label = section_label.into();
    let summary = summary.into();
    let footer = footer.into();

    let mut content = Vec::<Element<Message>>::new();
    if groups.is_empty() {
        content.push(text("\u{1f4c1} none").size(14).color(c.muted).into());
    } else {
        for (index, (root_label, items)) in groups.into_iter().enumerate() {
            if index > 0 {
                content.push(Space::new().height(Length::Fixed(10.0)).into());
            }
            content.push(
                row![
                    text("\u{1f4e6}").size(16),
                    text(root_label).size(14).color(c.title).font(theme::FRIZ),
                ]
                .spacing(8)
                .into(),
            );
            if items.is_empty() {
                content.push(text("   \u{21b3} none").size(13).color(c.muted).into());
            } else {
                content.extend(items.into_iter().map(|(item, is_dir)| {
                    let icon = if is_dir { "\u{21b3}" } else { "\u{1f4c4}" };
                    row![
                        Space::new().width(14),
                        text(icon).size(13).color(c.muted),
                        text(item).size(13).color(c.text),
                    ]
                    .spacing(8)
                    .into()
                }));
            }
        }
    }

    container(
        column![
            text(section_label)
                .size(14)
                .color(c.muted)
                .font(theme::FRIZ),
            container(text(summary).size(13).color(c.muted))
                .height(Length::Fixed(52.0))
                .width(Length::Fill),
            container(
                scrollable(column(content).spacing(4))
                    .width(Length::Fill)
                    .height(Length::Fixed(180.0))
                    .direction(theme::vscroll_overlay())
                    .style(move |t, s| theme::scrollable_style(c)(t, s)),
            )
            .width(Length::Fill)
            .padding(10)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(panel_background)),
                border: Border {
                    color: colors.border,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            }),
            container(
                text(footer)
                    .size(13)
                    .color(c.primary_text)
                    .font(theme::FRIZ)
            )
            .padding([6, 10])
            .width(Length::Fill)
            .style(move |_theme| action_banner_style(c, footer_background)),
        ]
        .spacing(6),
    )
    .padding(10)
    .width(Length::Fill)
    .style(move |_theme| theme::card_style(c))
    .into()
}

pub fn file_conflict<'a>(
    repo_id: i64,
    repo_name: &'a str,
    files: &'a [String],
    action: FileConflictAction,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;
    let action_label = match action {
        FileConflictAction::Install => "installation",
        FileConflictAction::Update => "update",
        FileConflictAction::Reinstall => "reinstallation",
    };
    let file_rows = files.iter().map(|file| {
        container(text(file).size(13).color(c.text))
            .padding([5, 8])
            .width(Length::Fill)
            .style(move |_theme| theme::card_style(c))
            .into()
    });

    column![
        row![
            text("Installation File Conflict")
                .size(18)
                .color(c.title),
            Space::new().width(Length::Fill),
            close_button(c),
        ]
        .align_y(iced::Alignment::Center),
        text(format!(
            "The {} for \"{}\" would replace existing files.",
            action_label, repo_name
        ))
        .size(14)
        .color(c.text),
        column(file_rows).spacing(6),
        text("If you continue, Wuddle will keep a private backup of each displaced file and restore it when this mod is removed. No live files have been changed yet.")
            .size(13)
            .color(c.warn),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(14))
                .on_press(Message::CloseDialog)
                .padding([8, 20])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(c),
                    _ => theme::tab_button_style(c),
                }),
            button(text("Back up & Continue").size(14))
                .on_press(Message::ConfirmFileConflict {
                    repo_id,
                    action,
                })
                .padding([8, 20])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::play_button_hovered_style(c),
                    _ => theme::play_button_style(c),
                }),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(14)
    .into()
}

/// Anti-virus warning shown before installing certain mods.
pub fn av_false_positive_warning<'a>(
    url: &'a str,
    mode: &'a str,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;
    let forge = crate::service::parse_forge_url(url)
        .map(|info| info.forge)
        .unwrap_or("gitea");
    let forge_url = url.to_string();
    let forge_icon = forge_svg_handle(forge, url);
    let forge_button = tip(
        button(
            row![
                text("Open on").size(14).color(c.text),
                iced::widget::svg(forge_icon)
                    .width(15)
                    .height(15)
                    .style(move |_t, _s| iced::widget::svg::Style {
                        color: Some(c.text),
                    }),
            ]
            .spacing(5)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::OpenUrl(forge_url))
        .padding([8, 12])
        .style(move |_t, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(c),
            _ => theme::tab_button_style(c),
        }),
        "Open this mod's repository in your browser",
        iced::widget::tooltip::Position::Top,
        colors,
    );
    let source_notice = if url.contains("github.com/suprepupre/wow-optimize") {
        "Wuddle is not affiliated with wow-optimize or its author, and does not endorse, audit, or accept responsibility for the mod's behavior. Only continue if you trust its repository author."
    } else {
        "Only proceed if you trust the repository author and understand the mod's behavior."
    };
    column![
        row![
            text("Anti-Virus Warning").size(18).color(colors.title),
            Space::new().width(Length::Fill),
            close_button(c),
        ]
        .align_y(iced::Alignment::Center),
        text("A potential security warning has been flagged for this mod.")
            .size(15)
            .color(colors.text),
        Space::new().height(Length::Fixed(10.0)),
        container(
            column![
                text("Various client mods can unavoidably trigger anti-virus heuristics (for example, SuperWoW). This does not mean you will see a warning, but it is likely.").size(14).color(colors.text),
                Space::new().height(Length::Fixed(8.0)),
                text(source_notice).size(14).color(colors.warn),
            ]
            .spacing(2)
        )
        .padding(15)
        .width(Length::Fill)
        .style(move |_theme| theme::card_style(c)),
        Space::new().height(Length::Fixed(15.0)),
        row![
            forge_button,
            Space::new().width(Length::Fill),
            button(text("Cancel").size(14))
                .on_press(Message::CloseDialog)
                .padding([8, 20])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(c),
                    _ => theme::tab_button_style(c),
                }),
            button(text("I trust this mod, proceed").size(14))
                .on_press(Message::InstallRepoOverride {
                    url: url.to_string(),
                    mode: mode.to_string(),
                })
                .padding([8, 20])
                .style(move |_theme, _status| theme::tab_button_active_style(c)),
        ]
        .spacing(10),
    ]
    .spacing(10)
    .into()
}

/// Addon conflict confirmation dialog.
#[allow(clippy::too_many_arguments)]
pub fn addon_conflict<'a>(
    url: &'a str,
    mode: &'a str,
    conflicts: &'a [wuddle_engine::AddonProbeConflict],
    pending_repo_id: Option<i64>,
    new_repo_label: &'a str,
    existing_repos: &'a [crate::service::CollectionConflictOwnerGroup],
    selected_addons: &'a [String],
    new_repo_preview: Option<&'a [crate::service::RepoFileEntry]>,
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;

    let direct_conflicts = conflicts
        .iter()
        .map(|conflict| conflict.addon_name.clone())
        .collect::<Vec<_>>();

    // Build "Old" (left) panel groups from existing repo owners.
    let old_groups: Vec<(String, Vec<(String, bool)>)> = if existing_repos.is_empty() {
        let items = direct_conflicts.iter().map(|n| (n.clone(), true)).collect();
        vec![("Untracked local folders".to_string(), items)]
    } else {
        existing_repos
            .iter()
            .map(|group| {
                let items = group
                    .conflicting_addons
                    .iter()
                    .map(|n| (n.clone(), true))
                    .collect();
                (group.repo_label.clone(), items)
            })
            .collect()
    };

    // Build "New" (right) panel group showing what will be installed.
    // We prioritize showing the actual discovered addons (selected_addons) because
    // these are the folders that Wuddle will actually create.
    let mut new_items = Vec::new();
    for name in selected_addons {
        if !name.starts_with('.') {
            new_items.push((name.clone(), true));
        }
    }
    // Then, supplement with other top-level directories from the preview if available.
    if let Some(preview) = new_repo_preview {
        for f in preview {
            if f.is_dir && !f.name.starts_with('.') && !new_items.iter().any(|(n, _)| n == &f.name)
            {
                new_items.push((f.name.clone(), true));
            }
        }
    }
    let new_groups = vec![(new_repo_label.to_string(), new_items)];

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
            "The new install would add this conflicting addon from {}.",
            new_repo_label
        )
    } else {
        format!(
            "The new install would add these conflicting addons from {}.",
            new_repo_label
        )
    };

    let header = row![
        column![
            text("Addon Conflict")
                .size(20)
                .color(colors.title)
                .font(theme::FRIZ),
            text("Duplicate addon folders detected")
                .size(12)
                .color(colors.muted),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
        close_button(c),
    ]
    .align_y(iced::Alignment::Center);

    let repo_card = container(
        row![
            text("\u{1f4e6}").size(28),
            column![
                text(new_repo_label)
                    .size(16)
                    .color(colors.primary)
                    .font(theme::FRIZ),
                text(url).size(11).color(colors.muted),
            ]
            .spacing(2)
        ]
        .spacing(16)
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding(12)
    .style(move |_t| container::Style {
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.03))),
        border: Border {
            color: colors.border,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    });

    column![
        header,
        Space::new().height(14),
        repo_card,
        Space::new().height(20),
        row![
            tree_panel("Old", old_summary, "REMOVE", colors.bad, Color::from_rgba(0.2, 0.05, 0.05, 0.15), old_groups, colors),
            tree_panel("New", new_summary, "INSTALL", colors.good, Color::from_rgba(0.05, 0.2, 0.05, 0.15), new_groups, colors),
        ]
        .spacing(20)
        .height(Length::Fill),
        Space::new().height(20),
        text("Overwriting will remove only the conflicting existing folders shown on the left and replace them with the new versions shown on the right. Other addons from their collections will be kept.")
            .size(13)
            .color(colors.text_soft),
        Space::new().height(16),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(14))
                .on_press(if let Some(id) = pending_repo_id {
                    Message::CancelConflictInstall { repo_id: id }
                } else {
                    Message::CloseDialog
                })
                .padding([8, 20])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(c),
                    _ => theme::tab_button_style(c),
                }),
            Space::new().width(8),
            button(text("Overwrite & Install").size(14))
                .on_press(if let Some(id) = pending_repo_id {
                    Message::InstallConflictOverride { repo_id: id }
                } else {
                    Message::InstallRepoOverride {
                        url: url.to_string(),
                        mode: mode.to_string(),
                    }
                })
                .padding([8, 24])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::play_button_hovered_style(c),
                    _ => theme::play_button_style(c),
                }),
        ]
        .align_y(iced::Alignment::Center),
    ]
    .spacing(0)
    .into()
}

/// Collection addon conflict confirmation dialog.
pub fn collection_addon_conflict<'a>(
    repo_id: i64,
    repo_name: &'a str,
    selected_addons: &'a [String],
    conflicts: &'a [wuddle_engine::AddonProbeConflict],
    existing_repos: &'a [crate::service::CollectionConflictOwnerGroup],
    colors: ThemeColors,
) -> Element<'a, Message> {
    let c = colors;

    let direct_conflicts = conflicts
        .iter()
        .map(|conflict| conflict.addon_name.clone())
        .collect::<Vec<_>>();

    let old_groups: Vec<(String, Vec<(String, bool)>)> = if existing_repos.is_empty() {
        let items = direct_conflicts.iter().map(|n| (n.clone(), true)).collect();
        vec![("Untracked local folders".to_string(), items)]
    } else {
        existing_repos
            .iter()
            .map(|group| {
                let items = group
                    .conflicting_addons
                    .iter()
                    .map(|n| (n.clone(), true))
                    .collect();
                (group.repo_label.clone(), items)
            })
            .collect()
    };

    // For collections, show the full list of selected addons in the New panel.
    // Filter out hidden folders (starting with '.') for a cleaner preview.
    let new_items = selected_addons
        .iter()
        .filter(|n| !n.starts_with('.'))
        .map(|n| (n.clone(), true))
        .collect();
    let new_groups = vec![(repo_name.to_string(), new_items)];

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
            close_button(c),
        ]
        .align_y(iced::Alignment::Center),
        text(if existing_repos.len() == 1 && direct_conflicts.len() == 1 {
            format!(
                "An existing addon with the same name is already tracked by {}. Replacing it will remove only that conflicting addon from the old collection and install the new version shown on the right.",
                existing_repos[0].repo_label
            )
        } else {
            format!(
                "Some addons in '{}' conflict with addon folders that are already tracked. Wuddle can remove only the conflicting entries from their old collections, then install the new addons on the right.",
                repo_name
            )
        })
        .size(14)
        .color(colors.text),
        row![
            tree_panel("Old", old_summary, "REMOVE", colors.bad, Color::from_rgba(0.2, 0.05, 0.05, 0.15), old_groups, colors),
            tree_panel("New", new_summary, "INSTALL", colors.good, Color::from_rgba(0.05, 0.2, 0.05, 0.15), new_groups, colors),
        ]
        .spacing(20),
        text(
            "Overwriting will remove only the conflicting addon folders shown on the left. Other addons in their collections will remain installed and selected."
        )
        .size(14)
        .color(colors.text),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(14))
                .on_press(Message::CloseDialog)
                .padding([8, 20])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(c),
                    _ => theme::tab_button_style(c),
                }),
            Space::new().width(8),
            button(text("Overwrite & Install").size(14))
                .on_press(Message::SaveCollectionSelectionOverride {
                    repo_id,
                    selected_addons: selected_addons.to_vec(),
                })
                .padding([8, 24])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::play_button_hovered_style(c),
                    _ => theme::play_button_style(c),
                }),
        ]
        .spacing(0),
    ]
    .spacing(10)
    .into()
}
