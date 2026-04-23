use iced::widget::{button, checkbox, column, container, pick_list, row, rule, scrollable, stack, text, text_input, tooltip, Space};
use iced::{Element, Length};

use crate::anchored_overlay::AnchoredOverlay;
use crate::service::RepoRow;
use crate::theme::{self, ThemeColors};
use crate::{App, Dialog, Filter, Message, SortDir, SortKey};
use crate::service::is_mod;
use crate::theme::name_font;

// ---------------------------------------------------------------------------
// Tauri CSS column widths (from styles.css)
// ---------------------------------------------------------------------------
// Mods:   minmax(280px,1fr) | 100 (installed) | 120 (version picker) | 64 (enabled) | 130 (status) | 96 (actions)
// Addons: minmax(280px,1fr) | minmax(170,220) ≈ 190 (branch) | 130 (status) | 96 (actions)
const COL_INSTALLED: u32 = 100;
const COL_VERSION: u32 = 120;
const COL_ENABLED: u32 = 64;
const COL_STATUS: u32 = 130;
const COL_ACTIONS: u32 = 96;
const COL_BRANCH: u32 = 200;

// WEIRD_UTILS_DLLS moved to components::presets

fn is_weird_utils_item(repo_url: &str, dll_name: &str) -> bool {
    repo_url.to_lowercase().contains("weirdutils") || 
    crate::components::presets::WEIRD_UTILS_DLLS.iter().any(|&d| d.eq_ignore_ascii_case(dll_name))
}

#[derive(Debug, Clone)]
struct AddonDisplayRow<'a> {
    repo: &'a RepoRow,
    addon_name: String,
    /// True for the parent (header) row of a collection repo.
    is_collection_parent: bool,
    /// True for repos that are not collections but have >1 installed module.
    is_modular_parent: bool,
}

fn addon_display_rows<'a>(app: &'a App) -> Vec<AddonDisplayRow<'a>> {
    app.repos
        .iter()
        .filter(|repo| !is_mod(repo))
        .map(|repo| AddonDisplayRow {
            repo,
            addon_name: repo.name.clone(),
            is_collection_parent: repo.is_collection,
            is_modular_parent: !repo.is_collection && repo.installed_addons.len() > 1,
        })
        .collect()
}

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

/// Header cell: 1px divider + centered text.
fn hdr_cell<'a>(content: impl Into<Element<'a, Message>>, width: u32) -> Element<'a, Message> {
    row![
        vdiv(0.06, 20),
        container(content)
            .center_x(width)
            .center_y(Length::Shrink),
    ]
    .spacing(0)
    .align_y(iced::Alignment::Center)
    .into()
}

/// Body cell: 1px divider + centered content.
fn col_cell<'a>(content: impl Into<Element<'a, Message>>, width: u32) -> Element<'a, Message> {
    row![
        vdiv(0.05, 34),
        container(content)
            .center_x(width)
            .center_y(Length::Shrink),
    ]
    .spacing(0)
    .align_y(iced::Alignment::Center)
    .into()
}

pub fn view<'a>(app: &'a App, colors: &ThemeColors, label: &str) -> Element<'a, Message> {
    let c = *colors;
    let is_mods_tab = label == "Mods";

    let addon_rows = if is_mods_tab {
        Vec::new()
    } else {
        addon_display_rows(app)
    };

    // Filter repos for this tab
    let filtered_repos: Vec<&RepoRow> = if is_mods_tab {
        app.repos
            .iter()
            .filter(|r| is_mod(r))
            .filter(|r| match app.filter {
                Filter::All => true,
                Filter::Updates => app.plans.iter().any(|p| p.repo_id == r.id && p.has_update)
                    && !app.ignored_update_ids.contains(&r.id),
                Filter::Errors => app.plans.iter().any(|p| p.repo_id == r.id && p.error.is_some())
                    && r.enabled
                    && !app.ignored_update_ids.contains(&r.id),
                Filter::Ignored => !r.enabled || app.ignored_update_ids.contains(&r.id),
            })
            .filter(|r| {
                app.project_search.is_empty()
                    || r.name.to_lowercase().contains(&app.project_search.to_lowercase())
                    || r.owner.to_lowercase().contains(&app.project_search.to_lowercase())
            })
            .collect()
    } else {
        Vec::new()
    };

    let filtered_addon_rows: Vec<AddonDisplayRow> = if is_mods_tab {
        Vec::new()
    } else {
        addon_rows
            .iter()
            .cloned()
            .filter(|row| match app.filter {
                Filter::All => true,
                Filter::Updates => app.plans.iter().any(|p| p.repo_id == row.repo.id && p.has_update)
                    && !app.ignored_update_ids.contains(&row.repo.id),
                Filter::Errors => app.plans.iter().any(|p| p.repo_id == row.repo.id && p.error.is_some())
                    && row.repo.enabled
                    && !app.ignored_update_ids.contains(&row.repo.id),
                Filter::Ignored => !row.repo.enabled || app.ignored_update_ids.contains(&row.repo.id),
            })
            .filter(|row| {
                if app.project_search.is_empty() {
                    return true;
                }
                let q = app.project_search.to_lowercase();
                if row.addon_name.to_lowercase().contains(&q)
                    || row.repo.name.to_lowercase().contains(&q)
                    || row.repo.owner.to_lowercase().contains(&q)
                {
                    return true;
                }
                // For collection/modular parents, also search member addon/module names.
                if row.is_collection_parent {
                    let members = if row.repo.installed_addons.is_empty() {
                        &row.repo.selected_addons
                    } else {
                        &row.repo.installed_addons
                    };
                    return members.iter().any(|m| m.to_lowercase().contains(&q));
                }
                if row.is_modular_parent {
                    return row.repo.installed_addons.iter().any(|m| m.to_lowercase().contains(&q));
                }
                false
            })
            .collect()
    };

    let total = if is_mods_tab {
        app.repos.iter().filter(|r| is_mod(r)).count()
    } else {
        addon_rows.len()
    };
    let update_count = if is_mods_tab {
        app.repos.iter().filter(|r| {
            is_mod(r)
                && app.plans.iter().any(|p| p.repo_id == r.id && p.has_update)
                && !app.ignored_update_ids.contains(&r.id)
        }).count()
    } else {
        addon_rows.iter().filter(|row| {
            app.plans.iter().any(|p| p.repo_id == row.repo.id && p.has_update)
                && !app.ignored_update_ids.contains(&row.repo.id)
        }).count()
    };
    let error_count = if is_mods_tab {
        app.repos.iter().filter(|r| {
            is_mod(r)
                && app.plans.iter().any(|p| p.repo_id == r.id && p.error.is_some())
                && r.enabled
                && !app.ignored_update_ids.contains(&r.id)
        }).count()
    } else {
        addon_rows.iter().filter(|row| {
            app.plans.iter().any(|p| p.repo_id == row.repo.id && p.error.is_some())
                && row.repo.enabled
                && !app.ignored_update_ids.contains(&row.repo.id)
        }).count()
    };
    let ignored_count = if is_mods_tab {
        app.repos.iter().filter(|r| is_mod(r) && (!r.enabled || app.ignored_update_ids.contains(&r.id))).count()
    } else {
        addon_rows.iter().filter(|row| !row.repo.enabled || app.ignored_update_ids.contains(&row.repo.id)).count()
    };

    // Toolbar: filters on left, search + buttons on right, all on one row
    let filters_part = row![
        filter_button(&format!("All ({})", total), Filter::All, app.filter, &c),
        filter_button(&format!("Updates ({})", update_count), Filter::Updates, app.filter, &c),
        filter_button(&format!("Errors ({})", error_count), Filter::Errors, app.filter, &c),
        filter_button(&format!("{} ({})", if is_mods_tab { "Disabled" } else { "Ignored" }, ignored_count), Filter::Ignored, app.filter, &c),
        Space::new().width(8),
        {
            let c2 = c;
            let has_token = wuddle_engine::github_token().is_some();
            let has_errors = app.plans.iter().any(|p| {
                p.error.as_deref().map(|e| {
                    let e = e.to_lowercase();
                    e.contains("rate") || e.contains("403") || e.contains("429")
                }).unwrap_or(false)
            });
            // Only count partial errors for repos that aren't ignored/disabled
            let partial_errors = !has_errors && app.plans.iter().any(|p| {
                p.error.is_some()
                    && !app.ignored_update_ids.contains(&p.repo_id)
                    && app.repos.iter().any(|r| r.id == p.repo_id && r.enabled)
            });
            let (api_label, api_color) = if has_errors {
                ("API status: rate limited", colors.bad)
            } else if partial_errors {
                ("API status: partial errors", colors.warn)
            } else if has_token {
                ("API status: authenticated", colors.good)
            } else {
                ("API status: anonymous", colors.muted)
            };

            // Build tooltip with rate limit details
            let tip_text = if let Some(info) = &app.github_rate_info {
                let now_epoch = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let reset_secs = info.reset_epoch - now_epoch;
                let reset_mins = (reset_secs / 60).max(0);
                format!(
                    "GitHub API: {}/{} requests remaining\nResets in {} min",
                    info.remaining, info.limit, reset_mins,
                )
            } else if has_token {
                "GitHub API: authenticated (5,000 req/hr)".to_string()
            } else {
                "GitHub API: anonymous (60 req/hr)\nAdd a token in Options for higher limits".to_string()
            };

            let tip_str = tip_text;
            let label: Element<Message> = tooltip(
                text(api_label).size(12).color(api_color),
                container(text(tip_str).size(12).color(c2.text))
                    .padding([4, 8])
                    .style(move |_theme| crate::theme::tooltip_style(&c2)),
                tooltip::Position::Bottom,
            )
            .gap(4.0)
            .into();
            label
        },
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    let mut action_items: Vec<Element<Message>> = Vec::new();
    action_items.push({
        let c2 = c;
        let show_clear = !app.project_search.is_empty();
        stack![
            text_input("Search...", &app.project_search)
                .on_input(Message::SetProjectSearch)
                .width(180)
                .padding(iced::Padding { top: 4.0, right: if show_clear { 26.0 } else { 10.0 }, bottom: 4.0, left: 10.0 }),
            {
                let clear_el: Element<Message> = if show_clear {
                    button(text("\u{2715}").size(12).color(c2.muted))
                        .on_press(Message::SetProjectSearch(String::new()))
                        .padding([3, 7])
                        .style(move |_t, _s| button::Style {
                            background: None,
                            text_color: c2.muted,
                            border: iced::Border::default(),
                            shadow: iced::Shadow::default(),
                            snap: true,
                        })
                        .into()
                } else {
                    Space::new().into()
                };
                container(clear_el)
            }
            .width(180)
            .height(Length::Fill)
            .align_x(iced::Alignment::End)
            .align_y(iced::Alignment::Center)
            .padding(iced::Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 0.0 }),
        ]
        .width(180)
        .into()
    });
    if !is_mods_tab {
        let c2 = c;
        action_items.push(
            tip(
                button(text("\u{27F2}").size(14)) // ⟲ rescan
                    .on_press(Message::RefreshRepos)
                    .padding([4, 10])
                    .style(move |_theme, status| match status {
                        button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                        _ => theme::tab_button_style(&c2),
                    }),
                "Rescan for new addons in your WoW directory",
                tooltip::Position::Bottom,
                colors,
            ),
        );
    }
    {
        let c2 = c;
        let add_tip = if is_mods_tab { "Add a new mod repository" } else { "Add a new addon repository" };
        action_items.push(
            tip(
                button(text("+ Add").size(13))
                    .on_press(Message::OpenDialog(Dialog::AddRepo {
                        url: String::new(),
                        mode: if is_mods_tab { String::from("auto") } else { String::from("addon_git") },
                        is_addons: !is_mods_tab,
                        advanced: false,
                    }))
                    .padding([4, 14])
                    .style(move |_theme, status| match status {
                        button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                        _ => theme::tab_button_style(&c2),
                    }),
                add_tip,
                tooltip::Position::Bottom,
                colors,
            ),
        );
    }
    let actions_part = row(action_items).spacing(8).align_y(iced::Alignment::Center);

    let toolbar = row![
        filters_part,
        Space::new().width(Length::Fill),
        actions_part,
    ]
    .align_y(iced::Alignment::Center);

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
                hdr_cell(text("Installed").size(13).color(colors.muted), COL_INSTALLED),
                hdr_cell(text("Version").size(13).color(colors.muted), COL_VERSION),
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

    // Sort filtered addon rows (addons tab)
    let mut filtered_addon_rows = filtered_addon_rows;
    if app.sort_dir != SortDir::None {
        let dir: i8 = if app.sort_dir == SortDir::Desc { -1 } else { 1 };
        filtered_addon_rows.sort_by(|a, b| {
            let cmp = match app.sort_key {
                SortKey::Name => a.addon_name.to_lowercase().cmp(&b.addon_name.to_lowercase()),
                SortKey::Status => {
                    let sa = status_rank(app, a.repo);
                    let sb = status_rank(app, b.repo);
                    sa.cmp(&sb)
                }
            };
            if dir < 0 { cmp.reverse() } else { cmp }
        });
    }

    // Table body
    let body: Element<Message> = if if is_mods_tab {
        filtered_repos.is_empty()
    } else {
        filtered_addon_rows.is_empty()
    } {
        container(
            text(if app.loading {
                format!("Loading {}...", label.to_lowercase())
            } else if app.filter == Filter::All {
                format!("No {} yet. Click \"+ Add\".", label.to_lowercase())
            } else {
                let filter_name = match app.filter {
                    Filter::Updates => "Updates",
                    Filter::Errors => "Errors",
                    Filter::Ignored => if is_mods_tab { "Disabled" } else { "Ignored" },
                    Filter::All => unreachable!(),
                };
                format!("No {} match the chosen filter: {}", label.to_lowercase(), filter_name)
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
        let mut rows: Vec<Element<Message>> = Vec::new();
        if is_mods_tab {
            for repo in filtered_repos.iter() {
                rows.push(mod_row(app, repo, colors));
                // Inject child DLL rows when expanded
                if repo.installed_dlls.len() > 1 && app.expanded_repo_ids.contains(&repo.id) {
                    for (dll_name, dll_enabled, dll_version) in &repo.installed_dlls {
                        rows.push(dll_child_row(repo.id, &repo.url, dll_name, *dll_enabled, dll_version.as_deref(), colors));
                    }
                }
            }
        } else {
            let search_active = !app.project_search.is_empty();
            let q = app.project_search.to_lowercase();
            for addon_row_data in filtered_addon_rows.iter() {
                if addon_row_data.is_collection_parent || addon_row_data.is_modular_parent {
                    // Auto-expand when a search is active so members are visible.
                    let is_expanded = search_active || app.expanded_repo_ids.contains(&addon_row_data.repo.id);
                    let repo = addon_row_data.repo;
                    let mut member_names = if addon_row_data.is_collection_parent {
                        if repo.installed_addons.is_empty() {
                            repo.selected_addons.clone()
                        } else {
                            repo.installed_addons.clone()
                        }
                    } else {
                        repo.installed_addons.clone()
                    };
                    member_names.sort_by_key(|name| name.to_ascii_lowercase());
                    member_names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
                    let member_count = member_names.len();
                    let member_label = if addon_row_data.is_collection_parent { "addons" } else { "modules" };
                    rows.push(addon_collection_parent_row(app, repo, is_expanded, member_count, member_label, colors));
                    if is_expanded {
                        let repo_itself_matched = !search_active
                            || repo.name.to_lowercase().contains(&q)
                            || repo.owner.to_lowercase().contains(&q);
                        for addon_name in member_names {
                            if search_active && !repo_itself_matched
                                && !addon_name.to_lowercase().contains(&q)
                            {
                                continue;
                            }
                            rows.push(addon_row(app, repo, addon_name, true, colors));
                        }
                    }
                } else {
                    rows.push(addon_row(
                        app,
                        addon_row_data.repo,
                        addon_row_data.addon_name.clone(),
                        false,
                        colors,
                    ));
                }
            }
        }
        let scroll_id = if label == "Mods" {
            iced::widget::Id::new("mods_projects_scrollable")
        } else {
            iced::widget::Id::new("addons_projects_scrollable")
        };

        scrollable(column(rows).spacing(0).width(Length::Fill))
            .id(scroll_id)
            .height(Length::Fill)
            .direction(theme::vscroll_overlay())
            .style(move |t, s| theme::scrollable_style(&c)(t, s))
            .into()
    };

    // Card wrapping the table
    // Nest header+body together so spacing(8) only applies between toolbar and the table,
    // not between the header row and the first data row.
    let table_section = column![header_row, body].spacing(0);
    let card = {
        let c2 = c;
        container(
            column![toolbar, table_section]
                .spacing(8)
                .padding([8, 12]),
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
    let total_update_count = app.repos.iter().filter(|r| {
        app.plans.iter().any(|p| p.repo_id == r.id && p.has_update)
            && !app.ignored_update_ids.contains(&r.id)
    }).count();
    let update_all_btn: Element<Message> = {
        let c2 = c;
        let b = button(text("Update All").size(13)).padding([6, 14]);
        let btn_el: Element<Message> = if total_update_count > 0 {
            b.on_press(Message::UpdateAll)
                .style(move |_t, _s| theme::tab_button_active_style(&c2))
                .into()
        } else {
            b.style(move |_t, _s| {
                let mut s = theme::tab_button_style(&c2);
                s.text_color = iced::Color::from_rgba(1.0, 1.0, 1.0, 0.25);
                s
            })
            .into()
        };
        tip(btn_el, "Download and install all available updates", tooltip::Position::Top, colors)
    };

    let check_btn = tip(
        btn("Check for updates", Message::CheckUpdates, &c),
        "Fetch the latest versions for all addons and mods",
        tooltip::Position::Top,
        colors,
    );

    let footer = row![
        text(last_checked_text).size(12).color(colors.muted),
        Space::new().width(Length::Fill),
        check_btn,
        update_all_btn,
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

/// Mods row: Name | Installed | Version | Enabled | Status | Actions
fn mod_row<'a>(app: &'a App, repo: &'a RepoRow, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    let plan = app.plans.iter().find(|p| p.repo_id == repo.id);
    let current_str = plan.and_then(|p| p.current.clone())
        .or_else(|| repo.last_version.clone())
        .unwrap_or_else(|| String::from("\u{2014}"));
    let latest_str = plan.map(|p| p.latest.clone())
        .unwrap_or_else(|| String::from("\u{2014}"));
    let has_update = plan.map(|p| p.has_update).unwrap_or(false);
    let has_error = plan.and_then(|p| p.error.as_ref()).is_some();
    let externally_modified = plan.map(|p| p.externally_modified).unwrap_or(false);
    let rid = repo.id;
    let enabled = repo.enabled;

    let update_ignored = app.ignored_update_ids.contains(&repo.id);
    let is_multi_dll = repo.installed_dlls.len() > 1;
    let is_expanded = app.expanded_repo_ids.contains(&repo.id);
    let is_infrequent = app.infrequent_repo_ids.contains(&repo.id);
    let name_col = name_cell_with_expand(repo, is_multi_dll, is_expanded, is_infrequent, colors);
    let menu_key = format!("repo:{}", repo.id);
    let is_menu_open = app.open_menu.as_deref() == Some(menu_key.as_str());
    let menu_content = crate::inline_context_menu(app, repo, None, &c);

    // Version picker dropdown — build options list with "Latest" as first entry
    let version_picker = version_picker_cell(app, repo, colors);

    let row_content = row![
        name_col,
        col_cell(text(current_str).size(12).color(colors.muted), COL_INSTALLED),
        col_cell(version_picker, COL_VERSION),
        col_cell(checkbox(enabled).on_toggle(move |b| Message::ToggleRepoEnabled(rid, b)), COL_ENABLED),
        col_cell(status_badge(has_error, has_update, externally_modified, enabled, update_ignored, &latest_str, repo, colors), COL_STATUS),
        col_cell(action_buttons(repo, menu_key, has_update && !update_ignored, is_menu_open, menu_content, &c), COL_ACTIONS),
    ]
    .spacing(0)
    .padding([9, 12])
    .align_y(iced::Alignment::Center);

    let separator = rule::horizontal(1).style(move |_theme| theme::update_line_style(&c));

    column![separator, row_content].into()
}

/// Indented child row for a single DLL within a multi-DLL mod.
fn dll_child_row<'a>(
    repo_id: i64,
    repo_url: &str,
    dll_name: &'a str,
    dll_enabled: bool,
    dll_version: Option<&'a str>,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let name_owned = dll_name.to_string();
    let name_owned2 = dll_name.to_string();

    let enable_cell = col_cell(
        checkbox(dll_enabled)
            .on_toggle(move |b| Message::ToggleDllEnabled(repo_id, name_owned.clone(), b)),
        COL_ENABLED,
    );

    // Version to show in the "Installed" column
    let installed_cell = if let Some(ver) = dll_version {
        col_cell(
            tip(
                text(ver).size(12).color(c.muted),
                &format!("This DLL was last updated in {}", ver),
                tooltip::Position::Top,
                colors,
            ),
            COL_INSTALLED,
        )
    } else {
        col_cell(text("\u{2014}").size(12).color(c.muted), COL_INSTALLED)
    };

    // Placeholder columns to keep alignment
    let empty_version   = col_cell(Space::new().width(Length::Fill), COL_VERSION);
    let empty_status    = col_cell(Space::new().width(Length::Fill), COL_STATUS);
    let empty_actions   = col_cell(Space::new().width(Length::Fill), COL_ACTIONS);

    let name_cell: Element<Message> = container(
        row![
            // Indent
            Space::new().width(28),
            text(format!("\u{21B3} {}", name_owned2))
                .size(12)
                .color(if dll_enabled { c.muted } else { iced::Color { a: 0.35, ..c.muted } }),
            
            // Help button for WeirdUtils DLLs - check name against list
            if is_weird_utils_item(repo_url, dll_name) {
                let dll_name_for_msg = dll_name.to_string();
                let help_bytes = include_bytes!("../../assets/icons/help.svg");
                let help_handle = iced::widget::svg::Handle::from_memory(help_bytes);
                let help_icon = iced::widget::svg(help_handle)
                    .width(16)
                    .height(16)
                    .style(move |_t, _s| iced::widget::svg::Style { color: Some(c.muted) });
                
                let help_btn: Element<Message> = button(help_icon)
                    .on_press(Message::OpenModFileInfo(dll_name_for_msg))
                    .padding(0)
                    .style(move |_theme, _status| button::Style {
                        background: None,
                        text_color: c.muted,
                        border: iced::Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: true,
                    })
                    .into();
                help_btn
            } else {
                Space::new().width(0).into()
            }
        ]
        .align_y(iced::Alignment::Center)
        .spacing(6),
    )
    .width(Length::Fill)
    .into();

    let row_content = row![
        name_cell,
        installed_cell,
        empty_version,
        enable_cell,
        empty_status,
        empty_actions,
    ]
    .spacing(0)
    .padding([6, 12])
    .align_y(iced::Alignment::Center);

    let separator = rule::horizontal(1).style(move |_theme| theme::update_line_style(&c));
    column![separator, row_content].into()
}

/// Version picker dropdown for a mod row.
/// Shows "Latest" + all fetched version tags. Auto-fetches versions if not loaded yet.
fn version_picker_cell<'a>(app: &'a App, repo: &'a RepoRow, _colors: &ThemeColors) -> Element<'a, Message> {
    let rid = repo.id;

    // Build options list: "Latest" first, then fetched version tags
    let mut options: Vec<String> = vec!["Latest".to_string()];
    if let Some(versions) = app.repo_versions.get(&rid) {
        for v in versions {
            options.push(v.tag.clone());
        }
    }

    // Current selection: pinned_version or "Latest"
    let selected = repo.pinned_version.clone().unwrap_or_else(|| "Latest".to_string());

    let is_loading = app.repo_versions_loading.contains(&rid);

    container(
        pick_list(
            options,
            Some(selected),
            move |chosen: String| {
                if chosen == "Latest" {
                    Message::SetPinnedVersion(rid, None)
                } else {
                    Message::SetPinnedVersion(rid, Some(chosen))
                }
            },
        )
        .placeholder(if is_loading { "Loading..." } else { "Latest" })
        .text_size(11)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(iced::Padding { top: 0.0, right: 8.0, bottom: 0.0, left: 8.0 })
    .into()
}

/// Name cell for a collection/modular parent row: repo title, expand chevron, member-count badge.
fn addon_collection_name_cell<'a>(
    repo: &'a RepoRow,
    is_expanded: bool,
    is_infrequent: bool,
    member_count: usize,
    member_label: &str,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let url = repo.url.clone();
    let subtitle = if repo.enabled {
        format!("{} \u{2022} {}", repo.owner, repo.forge)
    } else {
        format!("{} \u{2022} {} \u{2022} disabled", repo.owner, repo.forge)
    };
    let name_font = name_font(colors);

    let title_btn = button(
        iced::widget::rich_text::<(), _, _, _>([
            iced::widget::span(repo.name.clone())
                .underline(true)
                .color(c.link)
                .font(name_font)
                .size(20.0_f32),
        ]),
    )
    .on_press(Message::OpenUrl(url))
    .padding(0)
    .style(move |_theme, _status| button::Style {
        background: None,
        text_color: c.link,
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
        snap: true,
    });

    let rid = repo.id;

    let chevron_bytes: &[u8] = if is_expanded {
        include_bytes!("../../assets/icons/chevron-down.svg")
    } else {
        include_bytes!("../../assets/icons/chevron-right.svg")
    };
    let chevron_handle = iced::widget::svg::Handle::from_memory(chevron_bytes);
    let chevron_icon = iced::widget::svg(chevron_handle)
        .width(14)
        .height(14)
        .style(move |_t, _s| iced::widget::svg::Style { color: Some(c.muted) });

    let count_badge = container(text(format!("{} {}", member_count, member_label)).size(10).color(c.muted))
        .padding([1, 5])
        .style(move |_t| container::Style {
            background: Some(iced::Background::Color(iced::Color { a: 0.12, ..c.muted })),
            border: iced::Border {
                color: iced::Color { a: 0.2, ..c.muted },
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        });

    let title_row: Element<Message> = row![
        title_btn,
        Space::new().width(6),
        chevron_icon,
        Space::new().width(6),
        count_badge,
    ]
    .align_y(iced::Alignment::Center)
    .into();

    let title_row: Element<Message> = if is_infrequent && wuddle_engine::github_token().is_none() {
        let c_inf = c;
        let infreq_badge = container(text("\u{23F3}").size(10).color(c_inf.muted)).padding([0, 4]);
        crate::tip(
            row![title_row, Space::new().width(6), infreq_badge].align_y(iced::Alignment::Center),
            "Infrequently updated \u{2014} checked once every 4h to avoid API rate limits",
            tooltip::Position::Top,
            colors,
        )
        .into()
    } else {
        title_row
    };

    let content = container(
        column![
            title_row,
            text(subtitle).size(12).color(colors.muted).font(colors.body_font),
        ]
        .spacing(2),
    )
    .width(Length::Fill);

    // Transparent backdrop so clicking anywhere in the name cell toggles expand.
    let backdrop = button(Space::new().width(Length::Fill).height(Length::Fill))
        .on_press(Message::ToggleRepoExpanded(rid))
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_t, _s| button::Style {
            background: None,
            text_color: c.text,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        });

    stack![backdrop, content].width(Length::Fill).into()
}

/// A full summary row for a collection or modular-addon repo.
fn addon_collection_parent_row<'a>(
    app: &'a App,
    repo: &'a RepoRow,
    is_expanded: bool,
    member_count: usize,
    member_label: &'a str,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let plan = app.plans.iter().find(|p| p.repo_id == repo.id);
    let has_update = plan.map(|p| p.has_update).unwrap_or(false);
    let has_error = plan.and_then(|p| p.error.as_ref()).is_some();
    let externally_modified = plan.map(|p| p.externally_modified).unwrap_or(false);
    let latest_str = plan.map(|p| p.latest.clone()).unwrap_or_default();
    let enabled = repo.enabled;
    let update_ignored = app.ignored_update_ids.contains(&repo.id);
    let is_infrequent = app.infrequent_repo_ids.contains(&repo.id);
    let rid = repo.id;

    let name_col = addon_collection_name_cell(repo, is_expanded, is_infrequent, member_count, member_label, colors);

    let menu_key = format!("repo:{}", repo.id);
    let is_menu_open = app.open_menu.as_deref() == Some(menu_key.as_str());
    let menu_content = crate::inline_context_menu(app, repo, None, &c);

    let current_branch = repo.git_branch.clone().unwrap_or_else(|| "master".to_string());
    let branch_options = app.branches.get(&repo.id).cloned().unwrap_or_default();
    let branch_display: Element<Message> = container(
        pick_list(
            branch_options,
            Some(current_branch),
            move |branch: String| Message::SetRepoBranch(rid, branch),
        )
        .placeholder("master")
        .text_size(12)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(iced::Padding { top: 0.0, right: 5.0, bottom: 0.0, left: 5.0 })
    .into();

    let row_content = row![
        name_col,
        col_cell(branch_display, COL_BRANCH),
        col_cell(
            status_badge(has_error, has_update, externally_modified, enabled, update_ignored, &latest_str, repo, colors),
            COL_STATUS,
        ),
        col_cell(action_buttons(repo, menu_key, has_update && !update_ignored, is_menu_open, menu_content, &c), COL_ACTIONS),
    ]
    .spacing(0)
    .padding([9, 12])
    .align_y(iced::Alignment::Center);

    let separator = rule::horizontal(1).style(move |_theme| theme::update_line_style(&c));
    column![separator, row_content].into()
}

fn addon_name_cell<'a>(
    repo: &'a RepoRow,
    addon_name: String,
    is_collection_member: bool,
    is_infrequent: bool,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let url = repo.url.clone();
    let subtitle = if repo.enabled {
        format!("{} • {}", repo.owner, repo.forge)
    } else {
        format!("{} • {} • disabled", repo.owner, repo.forge)
    };

    let title_btn = button(
        iced::widget::rich_text::<(), _, _, _>([
            iced::widget::span(addon_name.clone())
                .underline(true)
                .color(c.link)
                .font(name_font(colors))
                .size(20.0_f32),
        ]),
    )
    .on_press(Message::OpenUrl(url))
    .padding(0)
    .style(move |_theme, _status| button::Style {
        background: None,
        text_color: c.link,
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
        snap: true,
    });

    let title_row: Element<Message> = if is_collection_member {
        let c2 = c;
        let badge = tip(
            button(text("Collection").size(10).color(c2.link))
                .on_press(Message::OpenCollectionManager(repo.id))
                .padding([2, 6])
                .style(move |_theme, _status| button::Style {
                    background: Some(iced::Background::Color(iced::Color { a: 0.10, ..c2.link })),
                    text_color: c2.link,
                    border: iced::Border {
                        color: iced::Color { a: 0.28, ..c2.link },
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                }),
            &format!("Installed from collection: {}/{}", repo.owner, repo.name),
            tooltip::Position::Top,
            colors,
        );
        row![title_btn, Space::new().width(8), badge]
            .align_y(iced::Alignment::Center)
            .into()
    } else {
        title_btn.into()
    };

    let title_row: Element<Message> = if is_infrequent && wuddle_engine::github_token().is_none() {
        let c_inf = c;
        let badge = container(text("\u{23F3}").size(10).color(c_inf.muted))
            .padding([0, 4]);
        crate::tip(
            row![title_row, Space::new().width(6), badge]
                .align_y(iced::Alignment::Center),
            "Infrequently updated — checked once every 4h to avoid API rate limits",
            tooltip::Position::Top,
            colors,
        ).into()
    } else {
        title_row
    };

    let inner = container(
        column![
            title_row,
            text(subtitle).size(12).color(colors.muted).font(colors.body_font),
        ]
        .spacing(2),
    )
    .width(Length::Fill);

    if is_collection_member {
        // Indent child rows slightly to visually nest them under the parent.
        row![
            Space::new().width(20),
            inner,
        ]
        .width(Length::Fill)
        .into()
    } else {
        inner.into()
    }
}

/// Addons row: Name | Branch | Status | Actions
fn addon_row<'a>(
    app: &App,
    repo: &'a RepoRow,
    addon_name: String,
    is_collection_member: bool,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let plan = app.plans.iter().find(|p| p.repo_id == repo.id);
    let has_update = plan.map(|p| p.has_update).unwrap_or(false);
    let has_error = plan.and_then(|p| p.error.as_ref()).is_some();
    let externally_modified = plan.map(|p| p.externally_modified).unwrap_or(false);
    let latest_str = plan.map(|p| p.latest.clone()).unwrap_or_default();
    let enabled = repo.enabled;

    let current_branch = repo.git_branch.clone().unwrap_or_else(|| "master".to_string());
    let update_ignored = app.ignored_update_ids.contains(&repo.id);
    let is_infrequent = app.infrequent_repo_ids.contains(&repo.id);
    let name_col = addon_name_cell(repo, addon_name.clone(), is_collection_member, is_infrequent, colors);
    let menu_key = if is_collection_member {
        format!("addon:{}:{}", repo.id, addon_name.to_ascii_lowercase())
    } else {
        format!("repo:{}", repo.id)
    };
    let is_menu_open = app.open_menu.as_deref() == Some(menu_key.as_str());
    let menu_content = crate::inline_context_menu(
        app,
        repo,
        if is_collection_member { Some(addon_name.as_str()) } else { None },
        &c,
    );

    let rid = repo.id;
    let branch_options = app.branches.get(&repo.id).cloned().unwrap_or_default();
    let branch_display: Element<Message> = if is_collection_member {
        container(text(current_branch).size(12).color(colors.text_soft))
            .width(Length::Fill)
            .padding(iced::Padding { top: 0.0, right: 5.0, bottom: 0.0, left: 5.0 })
            .into()
    } else {
        container(
            pick_list(
                branch_options,
                Some(current_branch),
                move |branch: String| Message::SetRepoBranch(rid, branch),
            )
            .placeholder("master")
            .text_size(12)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .padding(iced::Padding { top: 0.0, right: 5.0, bottom: 0.0, left: 5.0 })
        .into()
    };

    let row_content = row![
        name_col,
        col_cell(branch_display, COL_BRANCH),
        col_cell(status_badge(has_error, has_update, externally_modified, enabled, update_ignored, &latest_str, repo, colors), COL_STATUS),
        col_cell(action_buttons(repo, menu_key, has_update && !update_ignored, is_menu_open, menu_content, &c), COL_ACTIONS),
    ]
    .spacing(0)
    .padding([9, 12])
    .align_y(iced::Alignment::Center);

    let separator = rule::horizontal(1).style(move |_theme| theme::update_line_style(&c));

    column![separator, row_content].into()
}

/// Name cell with optional expand chevron and DLL count badge for multi-DLL mod rows.
fn name_cell_with_expand<'a>(
    repo: &'a RepoRow,
    is_multi_dll: bool,
    is_expanded: bool,
    is_infrequent: bool,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let url = repo.url.clone();
    let sub_text = if repo.enabled {
        format!("{} \u{2022} {}", repo.owner, repo.forge)
    } else {
        format!("{} \u{2022} {} \u{2022} disabled", repo.owner, repo.forge)
    };
    let name_font = name_font(colors);

    let title_btn = button(
        iced::widget::rich_text::<(), _, _, _>([
            iced::widget::span(repo.name.clone())
                .underline(true)
                .color(c.link)
                .font(name_font)
                .size(20.0_f32),
        ])
    )
    .on_press(Message::OpenUrl(url))
    .padding(0)
    .style(move |_theme, _status| button::Style {
        background: None,
        text_color: c.link,
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
        snap: true,
    });

    let rid = repo.id;
    let dll_count = repo.installed_dlls.len();

    let show_dxvk_badge = is_dxvk_repo(&repo.name);

    let title_row: Element<Message> = if is_multi_dll {
        let chevron_bytes: &[u8] = match is_expanded {
            true => include_bytes!("../../assets/icons/chevron-down.svg"),
            false => include_bytes!("../../assets/icons/chevron-right.svg"),
        };
        let chevron_handle = iced::widget::svg::Handle::from_memory(chevron_bytes);
        let chevron_icon = iced::widget::svg(chevron_handle)
            .width(14)
            .height(14)
            .style(move |_t, _s| iced::widget::svg::Style { color: Some(c.muted) });

        let badge_label = format!("{} DLLs", dll_count);
        let badge = container(text(badge_label).size(10).color(c.muted))
            .padding([1, 5])
            .style(move |_t| container::Style {
                background: Some(iced::Background::Color(iced::Color { a: 0.12, ..c.muted })),
                border: iced::Border {
                    color: iced::Color { a: 0.2, ..c.muted },
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            });
        let mut title_items: Vec<Element<Message>> = vec![
            title_btn.into(),
            Space::new().width(6).into(),
            chevron_icon.into(),
            Space::new().width(14).into(),
            badge.into(),
            // Help button for WeirdUtils parent row
            if is_weird_utils_item(&repo.url, &repo.name) || is_weird_utils_item(&repo.url, "weirdutils.dll") {
                let help_bytes = include_bytes!("../../assets/icons/help.svg");
                let help_handle = iced::widget::svg::Handle::from_memory(help_bytes);
                let help_icon = iced::widget::svg(help_handle)
                    .width(16)
                    .height(16)
                    .style(move |_t, _s| iced::widget::svg::Style { color: Some(c.muted) });

                let help_btn: Element<Message> = button(help_icon)
                    .on_press(Message::OpenDialog(Dialog::AddRepo {
                        url: repo.url.clone(),
                        mode: repo.mode.clone(),
                        is_addons: false,
                        advanced: false,
                    }))
                    .padding(0)
                    .style(move |_theme, _status| button::Style {
                        background: None,
                        text_color: c.muted,
                        border: iced::Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: true,
                    })
                    .into();
                row![Space::new().width(8), help_btn].into()
            } else {
                Space::new().width(0).into()
            },
        ];
        if show_dxvk_badge {
            let c2 = c;
            title_items.push(Space::new().width(6).into());
            title_items.push(
                button(text("\u{2699} DXVK conf").size(10).color(c2.link))
                    .on_press(Message::OpenDxvkConfig)
                    .padding([2, 6])
                    .style(move |_t, status| dxvk_badge_style(status, c2))
                    .into(),
            );
        }
        row(title_items).align_y(iced::Alignment::Center).into()
    } else if show_dxvk_badge {
        // Non-multi-DLL DXVK repo: show the configure badge next to the title
        let c2 = c;
        let dxvk_badge = button(text("\u{2699} DXVK conf").size(10).color(c2.link))
            .on_press(Message::OpenDxvkConfig)
            .padding([2, 6])
            .style(move |_t, status| dxvk_badge_style(status, c2));
        row![title_btn, Space::new().width(8), dxvk_badge]
            .align_y(iced::Alignment::Center)
            .into()
    } else {
        title_btn.into()
    };

    // Infrequent badge: shown next to title for repos checked less often
    // Only shown for anonymous users since authenticated users have no throttle.
    let title_row: Element<Message> = if is_infrequent && wuddle_engine::github_token().is_none() {
        let c_inf = c;
        let badge = container(text("\u{23F3}").size(10).color(c_inf.muted))
            .padding([0, 4]);
        let tip_text = "Infrequently updated \u{2014} checked once every 4h to avoid API rate limits";
        crate::tip(
            row![title_row, Space::new().width(6), badge]
                .align_y(iced::Alignment::Center),
            tip_text,
            tooltip::Position::Top,
            colors,
        ).into()
    } else {
        title_row
    };

    let content = container(
        column![
            title_row,
            text(sub_text).size(12).color(colors.muted).font(colors.body_font),
        ]
        .spacing(2),
    )
    .width(Length::Fill);

    if is_multi_dll {
        // Transparent backdrop button fills the name cell area. The title_btn sits on top
        // inside `content` and captures its own clicks (URL); all other clicks expand/collapse.
        let backdrop = button(Space::new().width(Length::Fill).height(Length::Fill))
            .on_press(Message::ToggleRepoExpanded(rid))
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_t, _s| button::Style {
                background: None,
                text_color: c.text,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            });
        stack![backdrop, content].width(Length::Fill).into()
    } else {
        content.into()
    }
}

/// Returns (label, text_color, bg_base_color) for a status badge.
fn status_info(
    has_error: bool,
    has_update: bool,
    externally_modified: bool,
    enabled: bool,
    update_ignored: bool,
    colors: &ThemeColors,
) -> (&'static str, iced::Color, iced::Color) {
    if update_ignored {
        ("Ignored", colors.muted, colors.muted)
    } else if !enabled {
        ("Ignored", colors.muted, colors.muted)
    } else if has_error {
        ("Error", colors.bad, colors.bad)
    } else if externally_modified {
        ("Modified", colors.warn, colors.warn)
    } else if has_update {
        ("Update available", colors.warn, colors.warn)
    } else {
        ("Up to date", colors.good, colors.good)
    }
}

/// Colored badge pill matching Tauri's badge style.
/// When an update is available, the badge has a tooltip showing the latest version.
fn status_badge<'a>(
    has_error: bool,
    has_update: bool,
    externally_modified: bool,
    enabled: bool,
    update_ignored: bool,
    latest_str: &str,
    repo: &RepoRow,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let (label, text_color, base_color) = status_info(has_error, has_update, externally_modified, enabled, update_ignored, colors);
    let bg = iced::Color::from_rgba(base_color.r, base_color.g, base_color.b, 0.18);
    let border_color = iced::Color::from_rgba(base_color.r, base_color.g, base_color.b, 0.45);
    let badge = container(
        text(label).size(11).color(text_color),
    )
    .padding([2, 8])
    .style(move |_theme| container::Style {
        background: Some(iced::Background::Color(bg)),
        border: iced::Border { color: border_color, width: 1.0, radius: 4.0.into() },
        shadow: iced::Shadow::default(),
        text_color: None,
        snap: true,
    });

    if (has_update || externally_modified) && !update_ignored && !latest_str.is_empty() {
        let c = *colors;
        let tip = if externally_modified {
            "Modified externally. Reinstall or update to restore.".to_string()
        } else {
            format!("Latest: {}", latest_str)
        };
        tooltip(
            badge,
            text(tip).size(13).color(c.text),
            tooltip::Position::Top,
        )
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(0.1, 0.1, 0.1, 0.95))),
            border: iced::Border { color: c.border, width: 1.0, radius: 4.0.into() },
            shadow: iced::Shadow::default(),
            text_color: Some(c.text),
            snap: true,
        })
        .padding(6.0)
        .into()
    } else if !has_update && !has_error && !externally_modified && enabled && !update_ignored {
        let c = *colors;
        let mut tip_lines = Vec::new();
        if let Some(v) = &repo.last_version {
            if repo.mode == "addon_git" {
                tip_lines.push(format!("Commit: {}", v));
            } else {
                tip_lines.push(format!("Version: {}", v));
            }
        }
        if let Some(ts) = repo.installed_at_unix {
            if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
                let formatted = dt.format("%Y-%m-%d %H:%M").to_string();
                tip_lines.push(format!("Installed: {}", formatted));
            }
        }
        
        if tip_lines.is_empty() {
            badge.into()
        } else {
            tooltip(
                badge,
                text(tip_lines.join("\n")).size(13).color(c.text),
                tooltip::Position::Top,
            )
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(0.1, 0.1, 0.1, 0.95))),
                border: iced::Border { color: c.border, width: 1.0, radius: 4.0.into() },
                shadow: iced::Shadow::default(),
                text_color: Some(c.text),
                snap: true,
            })
            .padding(6.0)
            .into()
        }
    } else {
        badge.into()
    }
}

/// Action column: Update button + triple-dot menu button (with anchored overlay).
fn action_buttons<'a>(
    repo: &RepoRow,
    menu_key: String,
    has_update: bool,
    is_menu_open: bool,
    menu_content: Element<'a, Message>,
    colors: &ThemeColors,
) -> Element<'a, Message> {
    let c = *colors;
    let rid = repo.id;

    let mut items: Vec<Element<Message>> = Vec::new();

    // Download/update button — always shown, active only when update available
    {
        let c2 = c;
        let btn = button(container(text("\u{2193}").size(14)).center_x(Length::Fill)) // ↓
            .padding([4, 0])
            .width(30);
        let btn_el: Element<Message> = if has_update {
            btn.on_press(Message::UpdateRepo(rid))
                .style(move |_theme, _status| theme::tab_button_active_style(&c2))
                .into()
        } else {
            btn.style(move |_theme, _status| {
                let mut s = theme::tab_button_style(&c2);
                s.text_color = iced::Color::from_rgba(1.0, 1.0, 1.0, 0.2);
                s
            })
            .into()
        };
        let dl_tip = if has_update { "Download and install this update" } else { "No update available" };
        items.push(tip(btn_el, dl_tip, tooltip::Position::Top, colors));
    }

    // Triple-dot button wrapped in AnchoredOverlay so the popup is pinned
    // to the button's actual screen position via Iced's overlay system.
    let c2 = c;
    let dots_btn = button(container(text("\u{22EE}").size(14)).center_x(Length::Fill)) // ⋮
        .on_press(Message::ToggleMenu(menu_key))
        .padding([4, 0])
        .width(30)
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::tab_button_hovered_style(&c2),
            _ => theme::tab_button_style(&c2),
        });

    items.push(
        AnchoredOverlay::new(dots_btn, menu_content, is_menu_open)
            .on_dismiss(Message::CloseMenu)
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

/// Wrap any element in a tooltip with consistent styling.
fn tip<'a>(content: impl Into<Element<'a, Message>>, tip_text: &str, pos: tooltip::Position, colors: &ThemeColors) -> Element<'a, Message> {
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

/// Returns true if the repo is a DXVK distribution (any name containing "dxvk").
/// Used to show the DXVK badge button and context-menu item.
pub fn is_dxvk_repo(name: &str) -> bool {
    name.to_lowercase().contains("dxvk")
}

/// Button style for the ⚙ DXVK conf badge, shared across multi-DLL and single-DLL rows.
fn dxvk_badge_style(status: button::Status, c: crate::theme::ThemeColors) -> button::Style {
    let alpha = if matches!(status, button::Status::Hovered) { 0.18 } else { 0.10 };
    let border_alpha = if matches!(status, button::Status::Hovered) { 0.45 } else { 0.28 };
    button::Style {
        background: Some(iced::Background::Color(iced::Color { a: alpha, ..c.link })),
        text_color: c.link,
        border: iced::Border {
            color: iced::Color { a: border_alpha, ..c.link },
            width: 1.0,
            radius: 4.0.into(),
        },
        shadow: iced::Shadow::default(),
        snap: true,
    }
}

fn status_rank(app: &App, repo: &RepoRow) -> u8 {
    let plan = app.plans.iter().find(|p| p.repo_id == repo.id);
    let has_error = plan.and_then(|p| p.error.as_ref()).is_some();
    let has_update = plan.map(|p| p.has_update).unwrap_or(false);
    let update_ignored = app.ignored_update_ids.contains(&repo.id);
    if has_error { 0 }
    else if has_update && !update_ignored { 1 }
    else if !repo.enabled || update_ignored { 3 }
    else { 2 }
}
