use iced::Task;
use crate::{App, Message, LogLevel, Dialog, ToastKind, CheckStats};
use crate::settings::UpdateChannel;
use crate::service;
use crate::components::presets::{WEIRD_UTILS_DESCRIPTIONS, WEIRD_UTILS_DLLS, is_av_false_positive};
use wuddle_engine;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

pub const INFREQUENT_CHECK_INTERVAL_SECS: i64 = 4 * 3600;

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}



pub fn update(app: &mut App, message: Message) -> Option<Task<Message>> {
    match message {
        Message::ReposLoaded(result) => {
            app.loading = false;
            match result {
                Ok(repos) => {
                    let count = repos.len();
                    let mod_count = repos.iter().filter(|r| service::is_mod(r)).count();
                    let addon_count = count - mod_count;
                    app.repos = repos;
                    app.log(LogLevel::Info, &format!("Loaded {} repos ({} mods, {} addons).", count, mod_count, addon_count));
                    // Fetch branches for addon_git repos that aren't cached yet
                    let mut tasks: Vec<Task<Message>> = app
                        .repos
                        .iter()
                        .filter(|r| r.mode == "addon_git" && !app.branches.contains_key(&r.id))
                        .map(|r| {
                            let db = app.db_path.clone();
                            Task::perform(
                                service::list_repo_branches(db, r.id),
                                Message::FetchBranchesResult,
                            )
                        })
                        .collect();
                    // Auto-check on launch if the option is enabled (only once per session)
                    if app.opt_auto_check && !app.repos.is_empty() && !app.checking_updates && !app.autocheck_done {
                        app.autocheck_done = true;
                        app.checking_updates = true;
                        app.log(LogLevel::Api, "Auto-checking for updates on launch...");
                        tasks.push(check_updates_task(app));
                    }
                    // Always fire self-update check on launch
                    tasks.push(Task::perform(service::check_self_update_full(app.update_channel == UpdateChannel::Beta), Message::CheckSelfUpdateResult));
                    if !tasks.is_empty() {
                        return Some(Task::batch(tasks));
                    }
                }
                Err(e) => {
                    app.error = Some(e.clone());
                    app.log(LogLevel::Error, &format!("Failed to load repos: {}", e));
                }
            }
            Some(Task::none())
        }
        Message::PlansLoaded(result) => {
            match result {
                Ok(plans) => {
                    app.plans = plans;
                    recompute_infrequent_ids(app);
                }
                Err(e) => app.log(LogLevel::Error, &format!("Plans error: {}", e)),
            }
            Some(Task::none())
        }
        Message::RefreshRepos => {
            app.loading = true;
            app.log(LogLevel::Info, "Rescanning for repos and fixing casing...");
            Some(refresh_repos_task_inner(app, true))
        }
        Message::CheckUpdates => {
            app.log(LogLevel::Info, "Checking for updates...");
            app.checking_updates = true;
            Some(check_updates_task(app))
        }
        Message::GithubRateTick => {
            return Some(Task::perform(
                service::fetch_github_rate_limit(),
                Message::GithubRateInfoResult,
            ));
        }

        Message::CheckUpdatesResult(result) => {
            // If checking_updates is true, this was a user-initiated or auto-check;
            // if false, it was a silent post-update refresh — skip toasts/notifications.
            let is_explicit_check = app.checking_updates;
            app.checking_updates = false;
            match result {
                Ok(mut plans) => {
                    let update_count = plans.iter().filter(|p| p.has_update && !app.ignored_update_ids.contains(&p.repo_id)).count();
                    
                    let mut stats = CheckStats {
                        updates_found: update_count,
                        ..Default::default()
                    };

                    // Compute stats ONLY for the repos that were just checked (returned in plans)
                    for p in &plans {
                        if p.mode == "addon_git" {
                            stats.git_syncs += 1;
                        } else if p.host.contains("github.com") {
                            if p.not_modified {
                                stats.api_cached += 1;
                            } else {
                                stats.api_hits += 1;
                            }
                        } else {
                            stats.other_hits += 1;
                        }
                    }

                    for p in &plans {
                        if let Some(err) = &p.error {
                            // Suppress -16 (GIT_EAUTH): deleted/private repos the user
                            // has acknowledged; they generate noise on every check.
                            if !is_silenced_git_error(err) {
                                app.log(LogLevel::Error, &format!("{}/{} - {}", p.owner, p.name, simplify_git_error(err)));
                            }
                        }
                    }

                    if is_explicit_check {
                        if update_count > 0 {
                            app.show_toast(
                                format!("{} update{} available.", update_count, if update_count == 1 { "" } else { "s" }),
                                ToastKind::Info,
                            );
                        } else {
                            app.show_toast("No updates available.", ToastKind::Info);
                        }
                    }

                    // Merge in cached plans for repos that were skipped (infrequent).
                    let returned_ids: HashSet<i64> = plans.iter().map(|p| p.repo_id).collect();
                    for old_plan in &app.plans {
                        if !returned_ids.contains(&old_plan.repo_id) {
                            plans.push(old_plan.clone());
                        }
                    }

                    // Update infrequent check timestamp: if a token is present,
                    // we always check everything, so update the timestamp.
                    // If no token, only update if the window actually expired.
                    let now = now_unix();
                    let was_full_check = wuddle_engine::github_token().is_some() ||
                        (now - app.last_infrequent_check_unix) >= INFREQUENT_CHECK_INTERVAL_SECS;

                    if was_full_check || app.last_infrequent_check_unix == 0 {
                        app.last_infrequent_check_unix = now;
                    }

                    app.plans = plans;
                    recompute_infrequent_ids(app);
                    app.last_checked = Some(crate::chrono_now_fmt(app.opt_clock12));
                    app.cached_plans.insert(
                        app.active_profile_id.clone(),
                        (app.plans.clone(), app.last_checked.clone()),
                    );

                    if is_explicit_check && app.opt_desktop_notify && update_count > 0 {
                        let _ = notify_rust::Notification::new()
                            .appname("Wuddle")
                            .summary("Wuddle")
                            .body(&format!("{} update{} available", update_count, if update_count == 1 { "" } else { "s" }))
                            .icon(crate::notification_icon_path())
                            .show();
                    }

                    // Auto-fetch versions for all mod repos that haven't been loaded yet
                    let mut version_tasks: Vec<Task<Message>> = Vec::new();
                    for repo in &app.repos {
                        if service::is_mod(repo)
                            && !app.repo_versions.contains_key(&repo.id)
                            && !app.repo_versions_loading.contains(&repo.id)
                        {
                            let db = app.db_path.clone();
                            let url = repo.url.clone();
                            let id = repo.id;
                            app.repo_versions_loading.insert(id);
                            version_tasks.push(Task::perform(
                                service::list_repo_versions(db, url),
                                move |result| Message::FetchVersionsResult((id, result)),
                            ));
                        }
                    }
                    
                    // Final summary rate fetch
                    version_tasks.push(Task::perform(
                        service::fetch_github_rate_limit(),
                        move |info| Message::UpdateCheckRateLimitResult(stats.clone(), info)
                    ));

                    if !version_tasks.is_empty() {
                        return Some(Task::batch(version_tasks));
                    }
                }
                Err(e) => {
                    app.error = Some(e.clone());
                    app.log(LogLevel::Error, &format!("Update check failed: {}", e));
                    app.show_toast(format!("Update check failed: {}", e), ToastKind::Error);
                }
            }
            Some(Task::none())
        }
        Message::AddRepoSubmit => {
            if let Some(Dialog::AddRepo { ref url, ref mode, .. }) = app.dialog {
                let url = url.clone();
                let mode = mode.clone();

                // Check if this mod requires an AV warning
                if is_av_false_positive(&url) {
                    app.dialog = Some(Dialog::AvWarning { url, mode });
                    return Some(Task::none());
                }

                let db = app.db_path.clone();
                app.dialog = None;
                app.log(LogLevel::Info, &format!("Adding repo: {}", url));
                return Some(Task::perform(
                    service::add_repo(db, url, mode),
                    Message::AddRepoResult,
                ));
            }
            Some(Task::none())
        }
        Message::AddRepoResult(result) => {
            match result {
                Ok(id) => {
                    app.log(LogLevel::Info, &format!("Repo added (id={}).", id));
                    app.show_toast("Repo added successfully.", ToastKind::Info);
                    if !app.wow_dir.is_empty() {
                        let db = app.db_path.clone();
                        let wow = app.wow_dir.clone();
                        let opts = app.install_options();
                        app.log(LogLevel::Info, "Installing…");
                        app.updating_repo_ids.insert(id);
                        return Some(Task::perform(
                            service::install_new_repo(db, id, wow, opts),
                            Message::InstallAfterAddResult,
                        ));
                    }
                    return Some(refresh_repos_task(app));
                }
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Add repo failed: {}", e));
                    app.show_toast(format!("Add repo failed: {}", e), ToastKind::Error);
                    app.error = Some(e);
                }
            }
            Some(Task::none())
        }
        Message::InstallAfterAddResult(result) => {
            match result {
                Ok(msg) => app.log(LogLevel::Info, &msg),
                Err(e) => app.log(LogLevel::Error, &format!("Install failed: {}", e)),
            }
            app.updating_repo_ids.clear();
            Some(refresh_repos_task(app))
        }
        Message::InstallRepoOverride { url, mode } => {
            let db = app.db_path.clone();
            app.dialog = None;
            app.add_repo_preview = None;
            app.add_repo_preview_loading = false;
            app.add_repo_release_notes = None;
            app.add_repo_show_releases = false;
            app.add_repo_file_preview = None;
            app.add_repo_expanded_dirs.clear();
            app.add_repo_dir_contents.clear();
            app.log(LogLevel::Info, &format!("Adding repo (override): {}", url));
            Some(Task::perform(
                service::add_repo(db, url, mode),
                Message::AddRepoResult,
            ))
        }
        Message::RemoveRepoConfirm(id, remove_files) => {
            let db = app.db_path.clone();
            let wow = if app.wow_dir.is_empty() { None } else { Some(app.wow_dir.clone()) };
            app.dialog = None;
            app.log(LogLevel::Info, &format!("Removing repo id={} (remove_files={})...", id, remove_files));
            Some(Task::perform(
                service::remove_repo(db, id, wow, remove_files),
                Message::RemoveRepoResult,
            ))
        }
        Message::RemoveRepoResult(result) => {
            match result {
                Ok(()) => {
                    app.log(LogLevel::Info, "Repo removed.");
                    app.show_toast("Repo removed.", ToastKind::Info);
                    return Some(refresh_repos_task(app));
                }
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Remove failed: {}", e));
                    app.show_toast(format!("Remove failed: {}", e), ToastKind::Error);
                }
            }
            Some(Task::none())
        }
        Message::ToggleIgnoreUpdates(id) => {
            let was_ignored = app.ignored_update_ids.contains(&id);
            if was_ignored {
                app.ignored_update_ids.remove(&id);
            } else {
                app.ignored_update_ids.insert(id);
            }
            app.save_settings();
            let repo_name = app.repos.iter().find(|r| r.id == id).map(|r| r.name.as_str()).unwrap_or("?");
            app.log(LogLevel::Info, &format!("Repo '{}': updates {}.", repo_name, if was_ignored { "unignored" } else { "ignored" }));
            Some(Task::none())
        }
        Message::ToggleMergeInstalls(id, merge) => {
            let repo_name = app.repos.iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_default();
            app.log(LogLevel::Info, &format!("Repo '{}': merge installs {}.", repo_name, if merge { "enabled" } else { "disabled" }));
            let db = app.db_path.clone();
            Some(iced::Task::perform(
                service::set_merge_installs(db, id, merge),
                Message::ToggleMergeInstallsResult,
            ))
        }
        Message::ToggleMergeInstallsResult(Ok(_id)) => {
            Some(refresh_repos_task(app))
        }
        Message::ToggleMergeInstallsResult(Err(e)) => {
            app.log(LogLevel::Error, &format!("Toggle merge failed: {}", e));
            Some(Task::none())
        }
        Message::FetchVersions(id) => {
            let db = app.db_path.clone();
            let url = app.repos.iter().find(|r| r.id == id).map(|r| r.url.clone());
            if let Some(url) = url {
                return Some(Task::perform(
                    async move {
                        let res = service::list_repo_versions(db, url).await;
                        (id, res)
                    },
                    |result| Message::FetchVersionsResult(result),
                ));
            }
            Some(Task::none())
        }
        Message::FetchVersionsResult((id, Ok(versions))) => {
            app.repo_versions.insert(id, versions);
            Some(Task::none())
        }
        Message::FetchVersionsResult((id, Err(e))) => {
            app.log(LogLevel::Error, &format!("Fetch versions failed for id={}: {}", id, e));
            Some(Task::none())
        }
        Message::SetPinnedVersion(id, version) => {
            let db = app.db_path.clone();
            let v_str = version.clone().unwrap_or_else(|| "none".to_string());
            app.log(LogLevel::Info, &format!("Pinning version to '{}' for repo id={}...", v_str, id));
            Some(Task::perform(
                service::set_pinned_version(db, id, version),
                Message::SetPinnedVersionResult,
            ))
        }
        Message::SetPinnedVersionResult(Ok(_id)) => {
            app.log(LogLevel::Info, "Version pin updated. Re-checking updates...");
            Some(check_updates_task(app))
        }
        Message::SetPinnedVersionResult(Err(e)) => {
            app.log(LogLevel::Error, &format!("Set version failed: {}", e));
            Some(Task::none())
        }
        Message::DllCountWarningChoice { repo_id, merge } => {
            app.dialog = None;
            if merge {
                let db = app.db_path.clone();
                return Some(Task::batch(vec![
                    Task::perform(service::set_merge_installs(db, repo_id, true), Message::ToggleMergeInstallsResult),
                    Task::done(Message::UpdateRepo(repo_id)),
                ]));
            } else {
                return Some(Task::done(Message::UpdateRepo(repo_id)));
            }
        }
        Message::UpdateRepo(id) => {
            app.open_menu = None;
            if app.wow_dir.is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
            } else {
                if let Some(plan) = app.plans.iter().find(|p| p.repo_id == id) {
                    if plan.previous_dll_count > 0
                        && plan.new_dll_count > 0
                        && plan.previous_dll_count != plan.new_dll_count
                    {
                        let repo = app.repos.iter().find(|r| r.id == id);
                        let already_merge = repo.map(|r| r.merge_installs).unwrap_or(false);
                        if !already_merge {
                            let repo_name = repo
                                .map(|r| format!("{}/{}", r.owner, r.name))
                                .unwrap_or_default();
                            app.dialog = Some(Dialog::DllCountWarning {
                                repo_id: id,
                                repo_name,
                                previous_count: plan.previous_dll_count,
                                new_count: plan.new_dll_count,
                            });
                            return Some(Task::none());
                        }
                    }
                }
                if let Some(repo) = app.repos.iter().find(|r| r.id == id) {
                    app.log(LogLevel::Info, &format!("Updating {}/{}...", repo.owner, repo.name));
                }
                app.updating_repo_ids.insert(id);
                let db = app.db_path.clone();
                let wow = app.wow_dir.clone();
                let opts = app.install_options();
                return Some(Task::perform(
                    service::update_repo(db, id, wow, opts),
                    Message::UpdateRepoResult,
                ));
            }
            Some(Task::none())
        }
        Message::UpdateRepoResult(result) => {
            match result {
                Ok(Some(plan)) => {
                    app.updating_repo_ids.remove(&plan.repo_id);
                    let name = format!("{}/{}", plan.owner, plan.name);
                    app.log(LogLevel::Info, &format!("Updated {}.", name));
                    app.show_toast(format!("Updated {}.", name), ToastKind::Info);
                    // Remove from plans so it disappears from 'Updates' list in UI immediately
                    app.plans.retain(|p| p.repo_id != plan.repo_id);
                }
                Ok(None) => app.log(LogLevel::Info, "Already up to date."),
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Update failed: {}", e));
                    app.show_toast(format!("Update failed: {}", e), ToastKind::Error);
                }
            }
            return Some(refresh_repos_task(app));
        }
        Message::ToggleRepoEnabled(id, enabled) => {
            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            Some(Task::perform(
                service::set_repo_enabled(db, id, enabled, wow),
                Message::ToggleRepoEnabledResult,
            ))
        }
        Message::ToggleRepoEnabledResult(result) => {
            match result {
                Ok(()) => return Some(refresh_repos_task(app)),
                Err(e) => app.log(LogLevel::Error, &format!("Toggle enabled failed: {}", e)),
            }
            Some(Task::none())
        }
        Message::ToggleRepoExpanded(id) => {
            if app.expanded_repo_ids.contains(&id) {
                app.expanded_repo_ids.remove(&id);
            } else {
                app.expanded_repo_ids.insert(id);
            }
            Some(Task::none())
        }
        Message::ToggleDllEnabled(_repo_id, dll_name, enabled) => {
            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            Some(Task::perform(
                service::set_dll_enabled(db, wow, dll_name, enabled),
                Message::ToggleDllEnabledResult,
            ))
        }
        Message::ToggleDllEnabledResult(result) => {
            match result {
                Ok(()) => return Some(refresh_repos_task(app)),
                Err(e) => app.log(LogLevel::Error, &format!("Toggle DLL failed: {}", e)),
            }
            Some(Task::none())
        }
        Message::UpdateAll => {
            if app.wow_dir.is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
            } else {
                let db = app.db_path.clone();
                let wow = app.wow_dir.clone();
                let opts = app.install_options();
                let mut targets = Vec::new();
                let mut names = Vec::new();
                for plan in &app.plans {
                    if plan.has_update && !app.ignored_update_ids.contains(&plan.repo_id) {
                        targets.push(plan.repo_id);
                        names.push(format!("{}/{}", plan.owner, plan.name));
                    }
                }
                for name in names {
                    app.log(LogLevel::Info, &format!("Updating {}...", name));
                }
                for id in &targets {
                    app.updating_repo_ids.insert(*id);
                }
                if targets.is_empty() {
                    app.log(LogLevel::Info, "Nothing to update.");
                } else {
                    app.log(LogLevel::Info, &format!("Updating {} repo(s)...", targets.len()));
                    return Some(Task::perform(
                        service::update_all(db, wow, targets, opts),
                        Message::UpdateAllResult,
                    ));
                }
            }
            Some(Task::none())
        }
        Message::UpdateAllResult(result) => {
            match result {
                Ok(results) => {
                    let mut applied = 0;
                    let mut errors = 0;
                    for r in results {
                        app.updating_repo_ids.remove(&r.repo_id);
                        let name = format!("{}/{}", r.owner, r.name);
                        if let Some(e) = r.error {
                            errors += 1;
                            app.log(LogLevel::Error, &format!("{} update failed: {}", name, simplify_git_error(&e)));
                        } else {
                            applied += 1;
                            app.log(LogLevel::Info, &format!("Updated {}.", name));
                            // Remove from plans so it disappears from UI immediately
                            app.plans.retain(|p| p.repo_id != r.repo_id);
                        }
                    }
                    if errors > 0 {
                        app.show_toast(format!("Update all partial: {} OK, {} failed.", applied, errors), ToastKind::Warn);
                    } else if applied > 0 {
                        app.log(LogLevel::Info, &format!("Done. Updated {} repo(s).", applied));
                        app.show_toast(format!("Updated {} repo(s).", applied), ToastKind::Info);
                    }
                    return Some(refresh_repos_task(app));
                }
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Update all failed: {}", e));
                    app.show_toast(format!("Update all failed: {}", e), ToastKind::Error);
                }
            }
            Some(Task::none())
        }
        Message::ReinstallRepo(id) => {
            app.open_menu = None;
            if app.wow_dir.is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
            } else {
                app.dialog = None;
                app.log(LogLevel::Info, &format!("Reinstalling repo id={}...", id));
                let db = app.db_path.clone();
                let wow = app.wow_dir.clone();
                let opts = app.install_options();
                return Some(Task::perform(
                    service::reinstall_repo(db, id, wow, opts),
                    Message::ReinstallRepoResult,
                ));
            }
            Some(Task::none())
        }
        Message::ReinstallRepoResult(result) => {
            match result {
                Ok(plan) => {
                    app.log(LogLevel::Info, &format!("Reinstalled {}/{}.", plan.owner, plan.name));
                    return Some(refresh_repos_task(app));
                }
                Err(e) => app.log(LogLevel::Error, &format!("Reinstall failed: {}", e)),
            }
            Some(Task::none())
        }
        Message::FetchBranches(repo_id) => {
            let db = app.db_path.clone();
            Some(Task::perform(
                service::list_repo_branches(db, repo_id),
                Message::FetchBranchesResult,
            ))
        }
        Message::FetchBranchesResult((repo_id, result)) => {
            match result {
                Ok(branch_list) => {
                    app.branches.insert(repo_id, branch_list);
                }
                Err(e) => {
                    let repo_name = app.repos.iter()
                        .find(|r| r.id == repo_id)
                        .map(|r| format!("{}/{}", r.owner, r.name))
                        .unwrap_or_else(|| format!("repo#{}", repo_id));
                    if !is_silenced_git_error(&e) {
                        app.log(LogLevel::Error, &format!("Failed to fetch branches for {}: {}", repo_name, simplify_git_error(&e)));
                    }
                }
            }
            Some(Task::none())
        }
        Message::SetRepoBranch(repo_id, branch) => {
            let db = app.db_path.clone();
            app.log(LogLevel::Info, &format!("Setting branch to '{}' for repo id={}...", branch, repo_id));
            Some(Task::perform(
                service::set_repo_branch(db, repo_id, branch),
                Message::SetRepoBranchResult,
            ))
        }
        Message::SetRepoBranchResult(result) => {
            match result {
                Ok(repo_id) => {
                    app.log(LogLevel::Info, "Branch updated. Refreshing repos...");
                    app.branches.remove(&repo_id);
                    return Some(refresh_repos_task(app));
                }
                Err(e) => app.log(LogLevel::Error, &format!("Set branch failed: {}", simplify_git_error(&e))),
            }
            Some(Task::none())
        }
        Message::UpdateCheckRateLimitResult(stats, info) => {
            app.github_rate_info = info;

            let updates = if stats.updates_found == 1 { "update" } else { "updates" };
            let mut parts = vec![
                format!("{} {}", stats.updates_found, updates)
            ];

            if stats.api_hits > 0 {
                parts.push(format!("spent {} API point{}", stats.api_hits, if stats.api_hits == 1 { "" } else { "s" }));
            }
            if stats.api_cached > 0 {
                parts.push(format!("{} cached (free)", stats.api_cached));
            }
            if stats.git_syncs > 0 {
                parts.push(format!("{} synced (git)", stats.git_syncs));
            }
            if stats.other_hits > 0 {
                parts.push(format!("{} other check{}", stats.other_hits, if stats.other_hits == 1 { "" } else { "s" }));
            }

            let summary = parts.join(", ");
            let rate_suffix = if let Some(r) = &app.github_rate_info {
                let mins = (r.reset_epoch - now_unix()) / 60;
                format!(". ({}/{} remaining, resets in {} min)", r.remaining, r.limit, mins)
            } else {
                "".to_string()
            };

            app.log(LogLevel::Api, &format!("Check complete: {}{}", summary, rate_suffix));
            None
        }

        Message::GithubRateInfoResult(info) => {
            app.github_rate_info = info;
            Some(Task::none())
        }
        Message::ToggleRemoveFiles(val) => {
            if let Some(Dialog::RemoveRepo { ref mut remove_files, .. }) = app.dialog {
                *remove_files = val;
            }
            Some(Task::none())
        }
        Message::RemoveRepoFilesLoaded(result) => {
            if let Some(Dialog::RemoveRepo { ref mut files, .. }) = app.dialog {
                match result {
                    Ok(mut entries) => {
                        entries.sort_by(|a, b| {
                            let a_is_dir = a.1 == "dir";
                            let b_is_dir = b.1 == "dir";
                            b_is_dir.cmp(&a_is_dir).then(a.0.cmp(&b.0))
                        });
                        *files = entries;
                    }
                    Err(e) => app.log(LogLevel::Error, &format!("Failed to list files for removal: {}", e)),
                }
            }
            Some(Task::none())
        }
        Message::FetchRepoPreview(url) => {
            app.add_repo_preview_loading = true;
            Some(Task::perform(
                service::fetch_repo_preview(url),
                Message::FetchRepoPreviewResult,
            ))
        }
        Message::FetchRepoPreviewResult(result) => {
            app.add_repo_preview_loading = false;
            match result {
                Ok(info) => {
                    app.readme_editor_content = iced::widget::text_editor::Content::with_text(&info.readme_text);
                    app.readme_source_view = false;
                    app.add_repo_preview = Some(info);
                    app.add_repo_release_notes = None;
                    app.add_repo_show_releases = false;
                    app.add_repo_file_preview = None;
                    app.add_repo_expanded_dirs.clear();
                    app.add_repo_dir_contents.clear();
                }
                Err(_) => app.add_repo_preview = None,
            }
            Some(Task::none())
        }
        Message::ToggleAddRepoDir(path) => {
            if app.add_repo_expanded_dirs.contains(&path) {
                app.add_repo_expanded_dirs.remove(&path);
            } else {
                app.add_repo_expanded_dirs.insert(path.clone());
                if !app.add_repo_dir_contents.contains_key(&path) {
                    if let Some(ref preview) = app.add_repo_preview {
                        let forge_url = preview.forge_url.clone();
                        return Some(Task::perform(
                            service::fetch_dir_contents(forge_url, path),
                            Message::FetchDirContentsResult,
                        ));
                    }
                }
            }
            Some(Task::none())
        }
        Message::FetchDirContents(forge_url, path) => {
            Some(Task::perform(
                service::fetch_dir_contents(forge_url, path),
                Message::FetchDirContentsResult,
            ))
        }
        Message::FetchDirContentsResult(result) => {
            if let Ok((dir_path, entries)) = result {
                let mut sorted = entries;
                sorted.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
                app.add_repo_dir_contents.insert(dir_path, sorted);
            }
            Some(Task::none())
        }
        Message::FetchReleaseNotes => {
            if app.add_repo_release_notes.is_some() {
                app.add_repo_show_releases = true;
            } else if let Some(ref preview) = app.add_repo_preview {
                let url = preview.forge_url.clone();
                app.add_repo_show_releases = true;
                return Some(Task::perform(
                    service::fetch_releases(url),
                    Message::FetchReleaseNotesResult,
                ));
            }
            Some(Task::none())
        }
        Message::FetchReleaseNotesResult(result) => {
            match result {
                Ok(releases) => {
                    app.add_repo_release_notes = Some(releases.clone());
                    // Also update dialog if it's the changelog
                    if let Some(Dialog::Changelog { ref mut items, ref mut loading, ref mut title }) = app.dialog {
                        *loading = false;
                        *title = "Changelog".to_string();
                        // Transform ReleaseItem into Markdown Item
                        let mut markdown_text = String::new();
                        for rel in releases {
                            markdown_text.push_str(&format!("# {}\n\n", rel.name));
                            markdown_text.push_str(&rel.body);
                            markdown_text.push_str("\n\n---\n\n");
                        }
                        *items = iced::widget::markdown::Content::parse(&markdown_text).items().to_vec();
                    }
                }
                Err(e) => {
                    app.add_repo_show_releases = false;
                    app.log(LogLevel::Error, &format!("Failed to fetch releases: {}", e));
                    if let Some(Dialog::Changelog { ref mut loading, .. }) = app.dialog {
                        *loading = false;
                    }
                }
            }
            Some(Task::none())
        }
        Message::ShowReadme => {
            app.add_repo_show_releases = false;
            app.add_repo_file_preview = None;
            Some(Task::none())
        }
        Message::PreviewRepoFile(path) => {
            if let Some(ref preview) = app.add_repo_preview {
                let raw_base = preview.raw_base_url.clone();
                return Some(Task::perform(
                    service::fetch_raw_file(raw_base, path),
                    Message::PreviewRepoFileResult,
                ));
            }
            Some(Task::none())
        }
        Message::PreviewRepoFileResult(result) => {
            match result {
                Ok((path, content)) => app.add_repo_file_preview = Some((path, content)),
                Err(e) => app.add_repo_file_preview = Some(("Error".to_string(), e)),
            }
            Some(Task::none())
        }
        Message::QuickInstallPreset(url) => {
            let mode = if let Some(Dialog::AddRepo { ref mode, .. }) = app.dialog {
                mode.clone()
            } else {
                "auto".to_string()
            };

            // Check if this mod requires an AV warning
            if is_av_false_positive(&url) {
                app.dialog = Some(Dialog::AvWarning { url, mode });
                return Some(Task::none());
            }

            let db = app.db_path.clone();
            app.dialog = None;
            app.add_repo_preview = None;
            app.add_repo_preview_loading = false;
            app.add_repo_release_notes = None;
            app.add_repo_show_releases = false;
            app.add_repo_file_preview = None;
            app.add_repo_expanded_dirs.clear();
            app.add_repo_dir_contents.clear();
            app.log(LogLevel::Info, &format!("Adding repo: {}", url));
            return Some(Task::perform(
                service::add_repo(db, url, mode),
                Message::AddRepoResult,
            ));
        }
        Message::SetAddRepoUrl(url) => {
            if let Some(Dialog::AddRepo { url: ref mut old_url, .. }) = app.dialog {
                *old_url = url.clone();
            }
            // Also trigger preview fetch automatically as the user types
            if !url.is_empty() && (url.contains('/') || url.contains(':')) {
                return Some(Task::done(Message::FetchRepoPreview(url)));
            }
            Some(Task::none())
        }
        Message::OpenModFileInfo(name) => {
            // Priority: if it's a WeirdUtils DLL, try to fetch live info from the README first.
            if WEIRD_UTILS_DLLS.iter().any(|&d| d.eq_ignore_ascii_case(&name)) {
                app.dialog = Some(Dialog::Changelog { title: name.clone(), items: Vec::new(), loading: true });
                return Some(Task::perform(
                    service::fetch_dll_description(name),
                    Message::FetchDllDescriptionResult,
                ));
            }

            // Check if we have a hardcoded description for this DLL (non-WeirdUtils fallback or legacy)
            if let Some((dll, desc)) = WEIRD_UTILS_DESCRIPTIONS.iter().find(|(dll, _)| dll.eq_ignore_ascii_case(&name)) {
                let items = iced::widget::markdown::Content::parse(desc).items().to_vec();
                app.dialog = Some(Dialog::Changelog { title: dll.to_string(), items, loading: false });
                return Some(Task::none());
            }

            // Fallback: search for a repo with this name AND a forge_url (likely release notes)
            app.dialog = Some(Dialog::Changelog { title: name.clone(), items: Vec::new(), loading: true });
            let url = app.repos.iter()
                .find(|r| r.name.eq_ignore_ascii_case(&name) && !r.url.is_empty())
                .map(|r| r.url.clone());

            if let Some(url) = url {
                return Some(Task::perform(
                    service::fetch_releases(url),
                    Message::FetchReleaseNotesResult,
                ));
            } else {
                // If no repo found, just show "No info available"
                if let Some(Dialog::Changelog { ref mut items, ref mut loading, .. }) = app.dialog {
                    *loading = false;
                    *items = iced::widget::markdown::Content::parse("No additional information available for this mod.").items().to_vec();
                }
                return Some(Task::none());
            }
        }

        Message::FetchDllDescriptionResult(result) => {
            match result {
                Ok((name, desc)) => {
                    if let Some(Dialog::Changelog { ref mut title, ref mut items, ref mut loading, .. }) = app.dialog {
                        *title = name;
                        *items = iced::widget::markdown::Content::parse(&desc).items().to_vec();
                        *loading = false;
                    }
                }
                Err(_e) => {
                    // Fallback to hardcoded description if fetch fails
                    let mut found_fallback = false;
                    if let Some(Dialog::Changelog { ref mut title, ref mut items, ref mut loading, .. }) = app.dialog {
                        if let Some((_dll, desc)) = WEIRD_UTILS_DESCRIPTIONS.iter().find(|(dll, _)| dll.eq_ignore_ascii_case(title)) {
                            *items = iced::widget::markdown::Content::parse(desc).items().to_vec();
                            *loading = false;
                            found_fallback = true;
                        }
                    }
                    
                    if !found_fallback {
                        if let Some(Dialog::Changelog { ref mut items, ref mut loading, .. }) = app.dialog {
                            *loading = false;
                            *items = iced::widget::markdown::Content::parse("Could not fetch live information, and no offline description is available.").items().to_vec();
                        }
                    }
                }
            }
            return Some(Task::none());
        }
        _ => None,
    }
}

pub fn refresh_repos_task(app: &App) -> Task<Message> {
    refresh_repos_task_inner(app, false)
}

pub fn refresh_repos_task_inner(app: &App, fix_casing: bool) -> Task<Message> {
    let db = app.db_path.clone();
    let wow = if app.wow_dir.is_empty() {
        None
    } else {
        Some(app.wow_dir.clone())
    };
    Task::perform(service::list_repos(db, wow, fix_casing), Message::ReposLoaded)
}

pub fn check_updates_task(app: &mut App) -> Task<Message> {
    let db = app.db_path.clone();
    let wow = if app.wow_dir.is_empty() {
        None
    } else {
        Some(app.wow_dir.clone())
    };
    let skip = if wuddle_engine::github_token().is_none() {
        let s = infrequent_skip_ids(&app.repos, &app.plans, app.last_infrequent_check_unix);
        if !s.is_empty() {
            // Only log skipping in background or if manually triggered without token
            app.log(LogLevel::Api, &format!("Checking active mods and addons ({} infrequent repos skipped to save API quota)...", s.len()));
        }
        s
    } else {
        app.log(LogLevel::Api, "Checking all repositories (authenticated)...");
        HashSet::new()
    };

    Task::perform(
        service::check_updates_skip(db, wow, wuddle_engine::CheckMode::Force, skip),
        Message::CheckUpdatesResult,
    )
}

pub const INFREQUENT_THRESHOLD_SECS: i64 = 3 * 24 * 3600;

pub fn recompute_infrequent_ids(app: &mut App) {
    let now = now_unix();
    let has_update: HashSet<i64> = app.plans.iter()
        .filter(|p| p.has_update)
        .map(|p| p.repo_id)
        .collect();
    app.infrequent_repo_ids = app.repos.iter()
        .filter(|r| {
            if has_update.contains(&r.id) {
                return false; 
            }
            match r.published_at_unix {
                Some(pub_at) => (now - pub_at) > INFREQUENT_THRESHOLD_SECS,
                None => false,
            }
        })
        .map(|r| r.id)
        .collect();
}

pub fn infrequent_skip_ids(repos: &[service::RepoRow], plans: &[service::PlanRow], last_infrequent_check_unix: i64) -> HashSet<i64> {
    let now = now_unix();
    let recently_checked = (now - last_infrequent_check_unix) < INFREQUENT_CHECK_INTERVAL_SECS;

    if !recently_checked {
        return HashSet::new();
    }

    let has_update: HashSet<i64> = plans.iter()
        .filter(|p| p.has_update)
        .map(|p| p.repo_id)
        .collect();

    repos.iter()
        .filter(|r| {
            if has_update.contains(&r.id) {
                return false; 
            }
            match r.published_at_unix {
                Some(pub_at) => (now - pub_at) > INFREQUENT_THRESHOLD_SECS,
                None => false,
            }
        })
        .map(|r| r.id)
        .collect()
}

pub fn is_silenced_git_error(raw: &str) -> bool {
    raw.contains("(-16)")
}

pub fn simplify_git_error(raw: &str) -> String {
    // Extract numeric error code from "code=Something (-NN)" anywhere in the raw string.
    let error_code: Option<String> = raw
        .find("code=")
        .and_then(|i| {
            let after = &raw[i..];
            let lparen = after.find('(')?;
            let rparen = after.find(')')?;
            if rparen > lparen {
                let num = after[lparen + 1..rparen].trim();
                if num.chars().all(|c| c.is_ascii_digit() || c == '-') {
                    return Some(num.to_string());
                }
            }
            None
        });

    // Unwrap "list remote ... (last tried ...): INNER" chains.
    let mut inner = raw;
    while let Some(pos) = inner.find("): ") {
        inner = &inner[pos + 3..];
    }

    // Unwrap "connect remote URL (auth failed: DETAIL)" → keep DETAIL.
    if let Some(start) = inner.find("(auth failed: ") {
        inner = inner[start + 14..].trim_end_matches(|c: char| c == ')' || c == ' ');
    }

    // Strip "Git sync check failed: " prefix if still present.
    inner = inner.strip_prefix("Git sync check failed: ").unwrap_or(inner);

    let lower = inner.to_lowercase();
    let msg = if lower.contains("authentication required")
        || lower.contains("code=auth")
        || lower.contains("class=http (34)")
        || lower.contains("auth failed")
    {
        "Repository not found or requires authentication".to_string()
    } else if lower.contains("not found") || lower.contains("404") {
        "Repository not found".to_string()
    } else if lower.contains("timed out")
        || lower.contains("connection refused")
        || lower.contains("network unreachable")
    {
        "Network error — check your connection".to_string()
    } else if inner.len() > 120 {
        format!("{}…", &inner[..120])
    } else {
        inner.to_string()
    };

    match error_code {
        Some(code) => format!("{} (Error Code {})", msg, code),
        None => msg,
    }
}
