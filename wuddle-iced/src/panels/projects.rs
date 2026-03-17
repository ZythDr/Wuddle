use iced::widget::{button, checkbox, column, container, pick_list, row, rule, scrollable, text, text_input, Space};
use iced::{Element, Length};

use crate::service::RepoRow;
use crate::theme::{self, ThemeColors};
use crate::{is_mod, App, Dialog, Filter, Message, SortDir, SortKey};

// ---------------------------------------------------------------------------
// Tauri CSS column widths (from styles.css)
// ---------------------------------------------------------------------------
// Mods:   minmax(280px,1fr) | 116 | 116 | 86 | 146 | 96
// Addons: minmax(280px,1fr) | minmax(170,220) ≈ 190 | 146 | 96
const COL_CURRENT: u32 = 116;
const COL_LATEST: u32 = 116;
const COL_ENABLED: u32 = 86;
const COL_STATUS: u32 = 146;
const COL_ACTIONS: u32 = 96;
const COL_BRANCH: u32 = 200;

/// 1px vertical divider as a colored container with fixed height.
/// Using a container instead of rule::vertical because rule::vertical expands
/// to fill all available height in scrollable layouts, breaking row sizing.
fn vdiv<'a>(alpha: f32, height: u32) -> Element<'a, Message> {
    container(Space::new().width(1).height(height))
        .width(1)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, alpha))),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            text_color: None,
            snap: true,
        })
        .into()
}

/// Header cell: 1px divider + centered text, padding-left 10px (matches Tauri)
fn hdr_cell<'a>(content: impl Into<Element<'a, Message>>, width: u32) -> Element<'a, Message> {
    row![
        vdiv(0.06, 20),
        container(content)
            .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 10.0 })
            .center_x(width),
    ]
    .spacing(0)
    .align_y(iced::Alignment::Center)
    .into()
}

/// Body cell: 1px divider + centered content, padding-left 10px (matches Tauri)
fn col_cell<'a>(content: impl Into<Element<'a, Message>>, width: u32) -> Element<'a, Message> {
    row![
        vdiv(0.05, 34),
        container(content)
            .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 10.0 })
            .center_x(width),
    ]
    .spacing(0)
    .align_y(iced::Alignment::Center)
    .into()
}

pub fn view<'a>(app: &'a App, colors: &ThemeColors, label: &str) -> Element<'a, Message> {
    let c = *colors;
    let is_mods_tab = label == "Mods";

    // Filter repos for this tab
    let filtered_repos: Vec<&RepoRow> = app
        .repos
        .iter()
        .filter(|r| if is_mods_tab { is_mod(r) } else { !is_mod(r) })
        .filter(|r| match app.filter {
            Filter::All => true,
            Filter::Updates => app.plans.iter().any(|p| p.repo_id == r.id && p.has_update),
            Filter::Errors => app.plans.iter().any(|p| p.repo_id == r.id && p.error.is_some()),
            Filter::Ignored => !r.enabled,
        })
        .filter(|r| {
            app.project_search.is_empty()
                || r.name.to_lowercase().contains(&app.project_search.to_lowercase())
                || r.owner.to_lowercase().contains(&app.project_search.to_lowercase())
        })
        .collect();

    let total = app.repos.iter().filter(|r| if is_mods_tab { is_mod(r) } else { !is_mod(r) }).count();
    let update_count = app.repos.iter().filter(|r| {
        (if is_mods_tab { is_mod(r) } else { !is_mod(r) })
            && app.plans.iter().any(|p| p.repo_id == r.id && p.has_update)
    }).count();
    let error_count = app.repos.iter().filter(|r| {
        (if is_mods_tab { is_mod(r) } else { !is_mod(r) })
            && app.plans.iter().any(|p| p.repo_id == r.id && p.error.is_some())
    }).count();
    let ignored_count = app.repos.iter().filter(|r| {
        (if is_mods_tab { is_mod(r) } else { !is_mod(r) }) && !r.enabled
    }).count();

    // Toolbar row 1: filters + API status
    let filters = row![
        filter_button(&format!("All ({})", total), Filter::All, app.filter, &c),
        filter_button(&format!("Updates ({})", update_count), Filter::Updates, app.filter, &c),
        filter_button(&format!("Errors ({})", error_count), Filter::Errors, app.filter, &c),
        filter_button(&format!("{} ({})", if is_mods_tab { "Disabled" } else { "Ignored" }, ignored_count), Filter::Ignored, app.filter, &c),
        Space::new().width(8),
        text(if wuddle_engine::github_token().is_some() {
            "API status: authenticated"
        } else {
            "API status: anonymous"
        })
        .size(12)
        .color(colors.muted),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    // Toolbar row 2: search + rescan (addons only) + add
    let mut action_items: Vec<Element<Message>> = Vec::new();
    action_items.push(Space::new().width(Length::Fill).into());
    action_items.push(
        text_input("Search...", &app.project_search)
            .on_input(Message::SetProjectSearch)
            .width(180)
            .padding([6, 10])
            .into(),
    );
    if !is_mods_tab {
        let c2 = c;
        action_items.push(
            button(text("\u{27F2}").size(14)) // ⟲ rescan
                .on_press(Message::RefreshRepos)
                .padding([6, 10])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => theme::tab_button_style(&c2),
                })
                .into(),
        );
    }
    {
        let c2 = c;
        action_items.push(
            button(text("+ Add").size(13))
                .on_press(Message::OpenDialog(Dialog::AddRepo {
                    url: String::new(),
                    mode: String::from("auto"),
                }))
                .padding([6, 14])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                    _ => theme::tab_button_style(&c2),
                })
                .into(),
        );
    }
    let actions = row(action_items).spacing(8).align_y(iced::Alignment::Center);

    let toolbar = column![filters, actions].spacing(6);

    // Sort indicator
    let sort_arrow = |key: SortKey| -> &'static str {
        if app.sort_key == key {
            match app.sort_dir {
                SortDir::Asc => " \u{25B2}",   // ▲
                SortDir::Desc => " \u{25BC}",  // ▼
                SortDir::None => "",
            }
        } else {
            ""
        }
    };

    // Table header — different columns for Addons vs Mods
    let header_row = {
        let c2 = c;
        let name_hdr = sort_header_button(&format!("Name{}", sort_arrow(SortKey::Name)), SortKey::Name, &c);
        let status_hdr = sort_header_button(&format!("Status{}", sort_arrow(SortKey::Status)), SortKey::Status, &c);

        let header_inner = if is_mods_tab {
            row![
                name_hdr,
                hdr_cell(text("Current").size(13).color(colors.muted), COL_CURRENT),
                hdr_cell(text("Latest").size(13).color(colors.muted), COL_LATEST),
                hdr_cell(text("Enabled").size(13).color(colors.muted), COL_ENABLED),
                hdr_cell(status_hdr, COL_STATUS),
                hdr_cell(text("Actions").size(13).color(colors.muted), COL_ACTIONS),
            ]
        } else {
            row![
                name_hdr,
                hdr_cell(text("Branch").size(13).color(colors.muted), COL_BRANCH),
                hdr_cell(status_hdr, COL_STATUS),
                hdr_cell(text("Actions").size(13).color(colors.muted), COL_ACTIONS),
            ]
        };
        container(header_inner.spacing(0).padding([10, 12]))
            .width(Length::Fill)
            .style(move |_theme| theme::table_head_style(&c2))
    };

    // Sort filtered repos
    let mut filtered_repos = filtered_repos;
    if app.sort_dir != SortDir::None {
        let dir: i8 = if app.sort_dir == SortDir::Desc { -1 } else { 1 };
        filtered_repos.sort_by(|a, b| {
            let cmp = match app.sort_key {
                SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortKey::Status => {
                    let sa = status_rank(app, a);
                    let sb = status_rank(app, b);
                    sa.cmp(&sb)
                }
            };
            if dir < 0 { cmp.reverse() } else { cmp }
        });
    }

    // Table body
    let body: Element<Message> = if filtered_repos.is_empty() {
        container(
            text(if app.loading {
                format!("Loading {}...", label.to_lowercase())
            } else {
                format!("No {} yet. Click \"+ Add\".", label.to_lowercase())
            })
            .size(14)
            .color(colors.muted),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Shrink)
        .center_y(Length::Shrink)
        .into()
    } else {
        let rows: Vec<Element<Message>> = filtered_repos
            .iter()
            .enumerate()
            .map(|(idx, repo)| {
                if is_mods_tab {
                    mod_row(app, repo, idx, colors)
                } else {
                    addon_row(app, repo, idx, colors)
                }
            })
            .collect();
        scrollable(column(rows).spacing(0).width(Length::Fill))
            .height(Length::Fill)
            .into()
    };

    // Card wrapping the table
    let card = {
        let c2 = c;
        container(
            column![toolbar, header_row, body]
                .spacing(0)
                .padding([12, 12]),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| theme::card_style(&c2))
    };

    // Footer
    let last_checked_text = match &app.last_checked {
        Some(t) => format!("Last checked: {}", t),
        None => "Last checked: never".into(),
    };
    let footer = row![
        text(last_checked_text).size(12).color(colors.muted),
        Space::new().width(Length::Fill),
        btn("Retry Failed", Message::CheckUpdates, &c),
        btn("Check for updates", Message::CheckUpdates, &c),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    column![card, footer]
        .spacing(8)
        .padding([4, 0])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Mods row: Name | Current | Latest | Enabled | Status | Actions
fn mod_row<'a>(app: &App, repo: &'a RepoRow, row_idx: usize, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    let plan = app.plans.iter().find(|p| p.repo_id == repo.id);
    let current_str = plan.and_then(|p| p.current.clone())
        .or_else(|| repo.last_version.clone())
        .unwrap_or_else(|| String::from("\u{2014}"));
    let latest_str = plan.map(|p| p.latest.clone())
        .unwrap_or_else(|| String::from("\u{2014}"));
    let has_update = plan.map(|p| p.has_update).unwrap_or(false);
    let has_error = plan.and_then(|p| p.error.as_ref()).is_some();
    let rid = repo.id;
    let enabled = repo.enabled;

    let (status_text, status_color) = status_info(has_error, has_update, enabled, colors);

    let name_col = name_cell(repo, colors);

    let row_content = row![
        name_col,
        col_cell(text(current_str).size(12).color(colors.muted), COL_CURRENT),
        col_cell(text(latest_str).size(12).color(colors.muted), COL_LATEST),
        col_cell(checkbox(enabled).on_toggle(move |b| Message::ToggleRepoEnabled(rid, b)), COL_ENABLED),
        col_cell(text(status_text).size(12).color(status_color), COL_STATUS),
        col_cell(action_buttons(repo, row_idx, has_update, &c), COL_ACTIONS),
    ]
    .spacing(0)
    .padding([9, 12])
    .align_y(iced::Alignment::Center);

    let separator = rule::horizontal(1).style(move |_theme| theme::update_line_style(&c));

    if app.open_menu.map(|(rid, _)| rid) == Some(repo.id) {
        let menu = crate::inline_context_menu(app, repo, &c);
        let menu_row = row![
            Space::new().width(Length::Fill),
            menu,
            Space::new().width(4),
        ];
        column![separator, row_content, menu_row].into()
    } else {
        column![separator, row_content].into()
    }
}

/// Addons row: Name | Branch | Status | Actions
fn addon_row<'a>(app: &App, repo: &'a RepoRow, row_idx: usize, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    let plan = app.plans.iter().find(|p| p.repo_id == repo.id);
    let has_update = plan.map(|p| p.has_update).unwrap_or(false);
    let has_error = plan.and_then(|p| p.error.as_ref()).is_some();
    let enabled = repo.enabled;

    let current_branch = repo.git_branch.clone().unwrap_or_else(|| "master".to_string());
    let (status_text, status_color) = status_info(has_error, has_update, enabled, colors);

    let name_col = name_cell(repo, colors);

    let rid = repo.id;
    let branch_options = app.branches.get(&repo.id).cloned().unwrap_or_default();
    let branch_display: Element<Message> = pick_list(
        branch_options,
        Some(current_branch),
        move |branch: String| Message::SetRepoBranch(rid, branch),
    )
    .placeholder("master")
    .text_size(12)
    .width(Length::Fill)
    .into();

    let row_content = row![
        name_col,
        col_cell(branch_display, COL_BRANCH),
        col_cell(text(status_text).size(12).color(status_color), COL_STATUS),
        col_cell(action_buttons(repo, row_idx, has_update, &c), COL_ACTIONS),
    ]
    .spacing(0)
    .padding([9, 12])
    .align_y(iced::Alignment::Center);

    let separator = rule::horizontal(1).style(move |_theme| theme::update_line_style(&c));

    if app.open_menu.map(|(rid, _)| rid) == Some(repo.id) {
        let menu = crate::inline_context_menu(app, repo, &c);
        let menu_row = row![
            Space::new().width(Length::Fill),
            menu,
            Space::new().width(4),
        ];
        column![separator, row_content, menu_row].into()
    } else {
        column![separator, row_content].into()
    }
}

/// Shared name cell (left-aligned, clickable link)
fn name_cell<'a>(repo: &'a RepoRow, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    let url = repo.url.clone();
    let sub_text = if repo.enabled {
        format!("{} \u{2022} {}", repo.owner, repo.forge)
    } else {
        format!("{} \u{2022} {} \u{2022} disabled", repo.owner, repo.forge)
    };
    let name_font = crate::name_font(colors);
    button(
        column![
            text(repo.name.clone()).size(20).color(colors.link).font(name_font),
            text(sub_text).size(12).color(colors.muted).font(colors.body_font),
        ]
        .spacing(2),
    )
    .on_press(Message::OpenUrl(url))
    .padding(0)
    .width(Length::Fill)
    .style(move |_theme, _status| button::Style {
        background: None,
        text_color: c.link,
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
        snap: true,
    })
    .into()
}

fn status_info(
    has_error: bool,
    has_update: bool,
    enabled: bool,
    colors: &ThemeColors,
) -> (&'static str, iced::Color) {
    if has_error {
        ("Error", colors.bad)
    } else if has_update {
        ("Update available", colors.warn)
    } else if !enabled {
        ("Ignored", colors.muted)
    } else {
        ("Up to date", colors.good)
    }
}

/// Action column: Update button + triple-dot menu button
fn action_buttons<'a>(
    repo: &RepoRow,
    row_idx: usize,
    has_update: bool,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let rid = repo.id;

    let mut items: Vec<Element<Message>> = Vec::new();

    if has_update {
        let c2 = c;
        items.push(
            button(text("\u{2193}").size(14)) // ↓
                .on_press(Message::UpdateRepo(rid))
                .padding([4, 8])
                .style(move |_theme, _status| theme::tab_button_active_style(&c2))
                .into(),
        );
    }

    let c2 = c;
    items.push(
        button(text("\u{22EE}").size(14)) // ⋮
            .on_press(Message::ToggleMenu(rid, row_idx))
            .padding([4, 8])
            .style(move |_theme, status| match status {
                button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                _ => theme::tab_button_style(&c2),
            })
            .into(),
    );

    row(items).spacing(4).into()
}

fn filter_button<'a>(
    label: &str,
    filter: Filter,
    active_filter: Filter,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let active = filter == active_filter;
    let b = button(text(String::from(label)).size(12))
        .on_press(Message::SetFilter(filter))
        .padding([4, 10]);
    if active {
        b.style(move |_theme, _status| theme::tab_button_active_style(&c))
            .into()
    } else {
        b.style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(&c),
            _ => theme::tab_button_style(&c),
        })
        .into()
    }
}

fn btn<'a>(label: &str, msg: Message, colors: &ThemeColors) -> Element<'a, Message> {
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

fn sort_header_button<'a>(label: &str, key: SortKey, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    button(text(String::from(label)).size(13).color(c.muted))
        .on_press(Message::ToggleSort(key))
        .padding(0)
        .width(if matches!(key, SortKey::Name) { Length::Fill } else { Length::Shrink })
        .style(move |_theme, status| match status {
            button::Status::Hovered => button::Style {
                background: None,
                text_color: c.text,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            _ => button::Style {
                background: None,
                text_color: c.muted,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
        })
        .into()
}

fn status_rank(app: &App, repo: &RepoRow) -> u8 {
    let plan = app.plans.iter().find(|p| p.repo_id == repo.id);
    let has_error = plan.and_then(|p| p.error.as_ref()).is_some();
    let has_update = plan.map(|p| p.has_update).unwrap_or(false);
    if has_error { 0 }
    else if has_update { 1 }
    else if !repo.enabled { 3 }
    else { 2 }
}
