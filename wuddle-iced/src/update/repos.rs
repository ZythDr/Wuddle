use crate::components::presets::{
    is_av_false_positive, WEIRD_UTILS_DESCRIPTIONS, WEIRD_UTILS_DLLS,
};
use crate::service;
use crate::settings::UpdateChannel;
use crate::{
    AddonLocalChangesEntry, App, CheckStats, Dialog, FileConflictAction, LogLevel, Message,
    ToastKind,
};
use iced::Task;
use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use wuddle_engine;

pub const INFREQUENT_CHECK_INTERVAL_SECS: i64 = 4 * 3600;

fn retain_current_unique_plans(
    plans: &mut Vec<service::PlanRow>,
    repos: &[service::RepoRow],
) -> usize {
    let before = plans.len();
    let current_ids = repos.iter().map(|repo| repo.id).collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    plans.retain(|plan| current_ids.contains(&plan.repo_id) && seen.insert(plan.repo_id));
    before.saturating_sub(plans.len())
}

fn merge_current_update_plans(
    mut checked: Vec<service::PlanRow>,
    previous: &[service::PlanRow],
    repos: &[service::RepoRow],
) -> Vec<service::PlanRow> {
    retain_current_unique_plans(&mut checked, repos);
    // A remote update check does not perform the explicit Rescan's full local
    // worktree comparison. Preserve that last authoritative local result until
    // another Rescan, reinstall, or successful update replaces it.
    for plan in &mut checked {
        if plan.mode == "addon_git"
            && previous
                .iter()
                .any(|cached| cached.repo_id == plan.repo_id && cached.externally_modified)
        {
            plan.externally_modified = true;
        }
    }
    let current_ids = repos.iter().map(|repo| repo.id).collect::<HashSet<_>>();
    let mut seen = checked
        .iter()
        .map(|plan| plan.repo_id)
        .collect::<HashSet<_>>();

    checked.extend(
        previous
            .iter()
            .filter(|plan| current_ids.contains(&plan.repo_id) && seen.insert(plan.repo_id))
            .cloned(),
    );
    checked
}

fn apply_addon_git_rescan_state(
    plans: &mut Vec<service::PlanRow>,
    repos: &[service::RepoRow],
    scan: &wuddle_engine::AddonGitLocalChangeScan,
) -> Vec<(String, String)> {
    let modified = scan
        .modified
        .iter()
        .map(|change| (change.repo_id, change.reason.as_str()))
        .collect::<std::collections::HashMap<_, _>>();
    let successfully_inspected = scan
        .inspected_repo_ids
        .iter()
        .filter(|repo_id| !scan.failed_repo_ids.contains(repo_id))
        .copied()
        .collect::<HashSet<_>>();

    for plan in plans.iter_mut().filter(|plan| plan.mode == "addon_git") {
        if successfully_inspected.contains(&plan.repo_id) {
            plan.externally_modified = modified.contains_key(&plan.repo_id);
        }
    }

    let mut detected = Vec::new();
    for repo in repos.iter().filter(|repo| repo.mode == "addon_git") {
        let Some(reason) = modified.get(&repo.id).copied() else {
            continue;
        };
        if !plans.iter().any(|plan| plan.repo_id == repo.id) {
            let revision = repo
                .last_version
                .clone()
                .unwrap_or_else(|| "checked-out revision".to_string());
            plans.push(service::PlanRow {
                repo_id: repo.id,
                owner: repo.owner.clone(),
                name: repo.name.clone(),
                current: repo.last_version.clone(),
                latest: revision,
                asset_name: String::new(),
                has_update: false,
                repair_needed: false,
                externally_modified: true,
                not_modified: true,
                mode: repo.mode.clone(),
                host: String::new(),
                error: None,
                previous_dll_count: 0,
                new_dll_count: 0,
            });
        }
        detected.push((
            format!("{}/{}", repo.owner, repo.name),
            addon_local_change_reason(reason),
        ));
    }
    detected
}

fn sync_active_plan_cache(app: &mut App) {
    app.cached_plans.insert(
        app.active_profile_id.clone(),
        (app.plans.clone(), app.last_checked.clone()),
    );
}

fn reconcile_active_update_plans(app: &mut App) -> usize {
    let removed = retain_current_unique_plans(&mut app.plans, &app.repos);
    sync_active_plan_cache(app);
    removed
}

fn forget_repo_update_plan(app: &mut App, repo_id: i64) {
    app.plans.retain(|plan| plan.repo_id != repo_id);
    sync_active_plan_cache(app);
}

fn update_check_stage_description(stage: wuddle_engine::UpdateCheckProgressStage) -> &'static str {
    match stage {
        wuddle_engine::UpdateCheckProgressStage::Started => "starting the update check",
        wuddle_engine::UpdateCheckProgressStage::InspectingInstallation => {
            "preparing the repository check"
        }
        wuddle_engine::UpdateCheckProgressStage::CheckingGitRemote => {
            "checking the Git repository remote"
        }
        wuddle_engine::UpdateCheckProgressStage::FetchingRelease => {
            "fetching remote release metadata"
        }
        wuddle_engine::UpdateCheckProgressStage::SelectingRelease => {
            "selecting the release and assets"
        }
        wuddle_engine::UpdateCheckProgressStage::VerifyingFiles => "verifying installed files",
        wuddle_engine::UpdateCheckProgressStage::Finished => "finishing the update check",
    }
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Extract the conflicting file/folder names from an engine ADDON_CONFLICT error string.
///
/// The engine formats these as:
///   "ADDON_CONFLICT: Existing addon files were found for: NAME (/path) [owner]; NAME2 ..."
///
/// Returns a deduplicated list of names in the order they appear.
pub fn parse_addon_conflict_error(err: &str) -> Vec<String> {
    // Find the part after the prefix
    let Some(after_for) = err
        .find("found for: ")
        .map(|pos| &err[pos + "found for: ".len()..])
    else {
        // Fallback: return a single generic entry so the dialog still appears
        return vec!["conflicting files".to_string()];
    };

    // Each entry is "NAME (path) [owner_text]" separated by "; "
    let mut names = Vec::new();
    for entry in after_for.split("; ") {
        let name = entry
            .find(" (")
            .map(|pos| entry[..pos].trim())
            .unwrap_or_else(|| entry.trim());
        if !name.is_empty() {
            let name = name.to_string();
            if !names.iter().any(|n: &String| n.eq_ignore_ascii_case(&name)) {
                names.push(name);
            }
        }
    }

    if names.is_empty() {
        names.push("conflicting files".to_string());
    }
    names
}

fn parse_file_conflict_error(error: &str) -> Vec<String> {
    let details = error
        .split_once("found for: ")
        .map(|(_, details)| details)
        .unwrap_or("existing installation files")
        .split_once(". Confirm replacement")
        .map(|(details, _)| details)
        .unwrap_or("existing installation files");
    let files = details
        .split("; ")
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if files.is_empty() {
        vec!["existing installation files".to_string()]
    } else {
        files
    }
}

fn show_file_conflict(app: &mut App, repo_id: i64, action: FileConflictAction, error: &str) {
    app.updating_repo_ids.remove(&repo_id);
    let repo_name = app
        .repos
        .iter()
        .find(|repo| repo.id == repo_id)
        .map(|repo| format!("{}/{}", repo.owner, repo.name))
        .unwrap_or_else(|| "this mod".to_string());
    let files = parse_file_conflict_error(error);
    app.log(
        LogLevel::Info,
        &format!(
            "File replacement approval required for repo id={repo_id}: conflict_count={}",
            files.len()
        ),
    );
    app.dialog = Some(Dialog::FileConflict {
        repo_id,
        repo_name,
        files,
        action,
    });
}

fn addon_local_change_reason(error: &str) -> String {
    if error.contains("moved addon folder differs") {
        "An exposed addon folder contains added, changed, or missing files.".to_string()
    } else if error.contains("worktree contains unexpected changes") {
        "The installed Git worktree contains added, changed, or deleted files.".to_string()
    } else {
        "The installed addon contains local file changes.".to_string()
    }
}

fn addon_local_changes_entry(app: &App, repo_id: i64, error: &str) -> AddonLocalChangesEntry {
    let repo_name = app
        .repos
        .iter()
        .find(|repo| repo.id == repo_id)
        .map(|repo| format!("{}/{}", repo.owner, repo.name))
        .unwrap_or_else(|| format!("repository #{repo_id}"));
    AddonLocalChangesEntry {
        repo_id,
        repo_name,
        reason: addon_local_change_reason(error),
    }
}

fn show_addon_local_changes_dialog(app: &mut App, repos: Vec<AddonLocalChangesEntry>) {
    if repos.is_empty() {
        return;
    }
    for repo in &repos {
        app.log(
            LogLevel::Info,
            &format!(
                "Local changes detected for {}: {}",
                repo.repo_name, repo.reason
            ),
        );
    }
    app.log(
        LogLevel::Info,
        &format!(
            "Local-change approval required before updating {} addon {}.",
            repos.len(),
            if repos.len() == 1 {
                "repository"
            } else {
                "repositories"
            }
        ),
    );
    app.dialog = Some(Dialog::AddonLocalChanges { repos });
}

fn install_local_archive(app: &mut App, path: std::path::PathBuf) -> Option<Task<Message>> {
    crate::diagnostics::register_private_path(&path, "<LOCAL_ARCHIVE>");
    if !service::is_local_archive_path(&path) {
        app.log(
            LogLevel::Error,
            &format!("Selected file is not a supported archive: {:?}", path),
        );
        app.show_toast("Choose a .zip or .7z addon archive.", ToastKind::Warn);
        return Some(Task::none());
    }
    if app.wow_dir.is_empty() {
        app.log(
            LogLevel::Error,
            "Set a WoW directory before installing an addon archive.",
        );
        app.show_toast(
            "Set a WoW directory before installing an addon archive.",
            ToastKind::Warn,
        );
        return Some(Task::none());
    }

    let db = app.db_path.clone();
    app.dialog = None;
    app.reset_add_repo_state();
    app.log(LogLevel::Info, &format!("Adding local archive: {:?}", path));
    let scope = app.profile_operation_scope();
    Some(Task::perform(
        service::add_local_archive_file(db, path),
        move |result| Message::AddRepoResult(crate::ProfileScoped::new(scope.clone(), result)),
    ))
}

pub fn update(app: &mut App, message: Message) -> Option<Task<Message>> {
    match message {
        Message::ReposLoaded(scoped) => {
            let Some(result) = app.accept_profile_result(scoped, "repository load") else {
                return Some(Task::none());
            };
            app.loading = false;
            service::clear_rescan_progress();
            app.current_rescan_snapshot = None;
            app.current_rescan_started_at = None;
            app.last_rescan_warning_secs = None;
            match result {
                Ok(load_result) => {
                    crate::diagnostics::register_repository_rows(&load_result.rows);
                    for entry in &load_result.logs {
                        app.log(entry.level, &entry.text);
                    }
                    let addon_git_local_changes = load_result.addon_git_local_changes;
                    app.untracked_mpqs = load_result.untracked_mpqs;
                    let repos = load_result.rows;
                    let count = repos.len();
                    let mod_count = repos.iter().filter(|r| service::is_mod(r)).count();
                    let addon_count = count - mod_count;
                    app.repos = repos;
                    let discarded_plans = reconcile_active_update_plans(app);
                    if discarded_plans > 0 {
                        crate::diagnostics::trace(
                            "updates",
                            format!(
                                "discarded {discarded_plans} stale or duplicate update plan(s) after repository reload"
                            ),
                        );
                    }
                    if let Some(scan) = addon_git_local_changes.as_ref() {
                        let detected =
                            apply_addon_git_rescan_state(&mut app.plans, &app.repos, scan);
                        for (repo_name, reason) in detected {
                            app.log(
                                LogLevel::Info,
                                &format!("Local changes detected for {repo_name}: {reason}"),
                            );
                        }
                        sync_active_plan_cache(app);
                    }
                    // published_at_unix is persisted in each profile database. Rebuild
                    // this derived UI state immediately instead of waiting for another
                    // network update check after a profile switch or application restart.
                    recompute_infrequent_ids(app);
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "Loaded {} repos ({} mods, {} addons).",
                            count, mod_count, addon_count
                        ),
                    );
                    // Fetch branches for addon_git repos that aren't cached yet
                    let mut tasks: Vec<Task<Message>> = app
                        .repos
                        .iter()
                        .filter(|r| r.mode == "addon_git" && !app.branches.contains_key(&r.id))
                        .map(|r| {
                            let db = app.db_path.clone();
                            let scope = app.profile_operation_scope();
                            Task::perform(service::list_repo_branches(db, r.id), move |result| {
                                Message::FetchBranchesResult(crate::ProfileScoped::new(
                                    scope.clone(),
                                    result,
                                ))
                            })
                        })
                        .collect();
                    // Auto-check each loaded profile once per session. A global
                    // flag left secondary profiles unchecked until the timer fired.
                    if app.opt_auto_check
                        && !app.repos.is_empty()
                        && !app.checking_updates
                        && !app
                            .autocheck_done_profile_ids
                            .contains(&app.active_profile_id)
                    {
                        app.autocheck_done_profile_ids
                            .insert(app.active_profile_id.clone());
                        app.checking_updates = true;
                        app.update_check_trigger = Some(crate::app::UpdateCheckTrigger::Launch);
                        app.log(LogLevel::Api, "Auto-checking for updates on launch...");
                        tasks.push(check_updates_task(app));
                    }
                    // A repository reload is also used after adds, toggles, and
                    // updates. Only the first load starts the launch-time Wuddle
                    // update request; later checks are explicit or hourly.
                    if !app.self_update_launch_check_started {
                        app.self_update_launch_check_started = true;
                        tasks.push(Task::perform(
                            service::check_self_update_full(
                                app.update_channel == UpdateChannel::Beta,
                            ),
                            Message::CheckSelfUpdateResult,
                        ));
                    }
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
        Message::PollRescanProgress => {
            let progress = service::latest_rescan_progress();
            let snapshot = progress
                .as_ref()
                .map(|p| format!("{}|{}", p.stage, p.detail));

            if snapshot != app.current_rescan_snapshot {
                app.current_rescan_snapshot = snapshot;
                app.current_rescan_started_at = Some(std::time::Instant::now());
                app.last_rescan_warning_secs = None;
                if let Some(progress) = progress {
                    app.log(
                        LogLevel::Info,
                        &format!("{}: {}", progress.stage, progress.detail),
                    );
                }
            } else if let Some(progress) = progress {
                if let Some(started_at) = app.current_rescan_started_at {
                    let elapsed = started_at.elapsed().as_secs();
                    let should_warn = elapsed >= 10
                        && app
                            .last_rescan_warning_secs
                            .is_none_or(|last| elapsed >= last + 10);
                    if should_warn {
                        app.last_rescan_warning_secs = Some(elapsed);
                        app.log(
                            LogLevel::Error,
                            &format!(
                                "Still working on {} after {}s: {}",
                                progress.stage, elapsed, progress.detail
                            ),
                        );
                    }
                }
            }

            Some(Task::none())
        }
        Message::PlansLoaded(scoped) => {
            let Some(result) = app.accept_profile_result(scoped, "update-plan load") else {
                return Some(Task::none());
            };
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
            if app.checking_updates {
                app.log(
                    LogLevel::Info,
                    "An update check is already in progress; ignoring the duplicate request.",
                );
                return Some(Task::none());
            }
            app.log(LogLevel::Info, "Checking for updates...");
            app.checking_updates = true;
            app.update_check_trigger = Some(crate::app::UpdateCheckTrigger::Manual);
            Some(check_updates_task(app))
        }
        Message::PollUpdateCheckProgress => {
            let active = service::active_update_check_progress();
            let active_ids = active
                .iter()
                .map(|progress| progress.repo_id)
                .collect::<HashSet<_>>();
            let now = std::time::Instant::now();
            let mut log_entries = Vec::new();

            for progress in active {
                let stage_changed = app
                    .update_check_stage_by_repo
                    .get(&progress.repo_id)
                    .copied()
                    != Some(progress.stage);
                if stage_changed {
                    app.update_check_stage_by_repo
                        .insert(progress.repo_id, progress.stage);
                    app.update_check_stage_started_at_by_repo
                        .insert(progress.repo_id, now);
                    app.update_check_last_warning_secs_by_repo
                        .remove(&progress.repo_id);
                    log_entries.push((
                        LogLevel::Api,
                        format!(
                            "Update check for {}/{}: {}.",
                            progress.owner,
                            progress.name,
                            update_check_stage_description(progress.stage)
                        ),
                    ));
                    continue;
                }

                let Some(started_at) = app
                    .update_check_stage_started_at_by_repo
                    .get(&progress.repo_id)
                    .copied()
                else {
                    continue;
                };
                let elapsed = started_at.elapsed().as_secs();
                let should_warn = elapsed >= 10
                    && app
                        .update_check_last_warning_secs_by_repo
                        .get(&progress.repo_id)
                        .is_none_or(|last| elapsed >= *last + 10);
                if should_warn {
                    app.update_check_last_warning_secs_by_repo
                        .insert(progress.repo_id, elapsed);
                    log_entries.push((
                        LogLevel::Error,
                        format!(
                            "Still {} for {}/{} after {}s.",
                            update_check_stage_description(progress.stage),
                            progress.owner,
                            progress.name,
                            elapsed
                        ),
                    ));
                }
            }

            app.update_check_stage_by_repo
                .retain(|repo_id, _| active_ids.contains(repo_id));
            app.update_check_stage_started_at_by_repo
                .retain(|repo_id, _| active_ids.contains(repo_id));
            app.update_check_last_warning_secs_by_repo
                .retain(|repo_id, _| active_ids.contains(repo_id));
            for (level, message) in log_entries {
                app.log(level, &message);
            }
            Some(Task::none())
        }
        Message::GithubRateTick => Some(Task::perform(
            service::fetch_github_rate_limit(),
            Message::GithubRateInfoResult,
        )),

        Message::CheckUpdatesResult(scoped) => {
            let Some(result) = app.accept_profile_result(scoped, "update check") else {
                return Some(Task::none());
            };
            // If checking_updates is true, this was a user-initiated or auto-check;
            // if false, it was a silent post-update refresh — skip toasts/notifications.
            let check_trigger = app.update_check_trigger.take();
            let is_explicit_check = app.checking_updates && check_trigger.is_some();
            app.checking_updates = false;
            app.update_check_stage_by_repo.clear();
            app.update_check_stage_started_at_by_repo.clear();
            app.update_check_last_warning_secs_by_repo.clear();
            service::clear_update_check_progress();
            match result {
                Ok(mut plans) => {
                    let discarded_checked_plans =
                        retain_current_unique_plans(&mut plans, &app.repos);
                    if discarded_checked_plans > 0 {
                        crate::diagnostics::trace(
                            "updates",
                            format!(
                                "discarded {discarded_checked_plans} stale or duplicate plan(s) returned by an update check"
                            ),
                        );
                    }
                    let rate_limit_error = plans
                        .iter()
                        .filter_map(|plan| plan.error.as_deref())
                        .find(|error| crate::github_api::rate_limit_notice(error).is_some())
                        .map(str::to_string);
                    let update_count = plans
                        .iter()
                        .filter(|p| p.has_update && !app.ignored_update_ids.contains(&p.repo_id))
                        .count();

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
                                let user_error = crate::github_api::user_facing_error(err);
                                app.log(
                                    LogLevel::Error,
                                    &format!(
                                        "{}/{} - {}",
                                        p.owner,
                                        p.name,
                                        simplify_git_error(&user_error)
                                    ),
                                );
                            }
                        }
                    }

                    if is_explicit_check {
                        if let Some(error) = rate_limit_error.as_deref() {
                            app.show_github_rate_limit(
                                "Some GitHub update checks could not finish.",
                                error,
                            );
                        } else if update_count > 0 {
                            app.show_toast(
                                format!(
                                    "{} update{} available.",
                                    update_count,
                                    if update_count == 1 { "" } else { "s" }
                                ),
                                ToastKind::Info,
                            );
                        } else {
                            app.show_toast("No updates available.", ToastKind::Info);
                        }
                    }

                    // Merge cached plans only for repositories that still exist.
                    // This preserves intentionally skipped infrequent checks without
                    // resurrecting removed/replaced repositories or duplicate rows.
                    plans = merge_current_update_plans(plans, &app.plans, &app.repos);

                    // Update infrequent check timestamp: if a token is present,
                    // we always check everything, so update the timestamp.
                    // If no token, only update if the window actually expired.
                    let now = now_unix();
                    let was_full_check = wuddle_engine::github_token().is_some()
                        || (now - app.last_infrequent_check_unix) >= INFREQUENT_CHECK_INTERVAL_SECS;

                    if was_full_check || app.last_infrequent_check_unix == 0 {
                        app.last_infrequent_check_unix = now;
                    }
                    if app.last_infrequent_check_unix > 0 {
                        let mut schedule_changed = false;
                        if let Some(profile) = app
                            .profiles
                            .iter_mut()
                            .find(|profile| profile.id == app.active_profile_id)
                        {
                            schedule_changed = profile.last_infrequent_check_unix
                                != app.last_infrequent_check_unix;
                            profile.last_infrequent_check_unix = app.last_infrequent_check_unix;
                        }
                        if schedule_changed {
                            app.save_settings();
                        }
                    }

                    app.plans = plans;
                    recompute_infrequent_ids(app);
                    app.last_checked = Some(crate::chrono_now_fmt(app.opt_clock12));
                    app.cached_plans.insert(
                        app.active_profile_id.clone(),
                        (app.plans.clone(), app.last_checked.clone()),
                    );

                    if is_explicit_check && app.opt_desktop_notify && update_count > 0 {
                        let _ = crate::desktop_notification::show_updates_available(update_count);
                    }

                    // Auto-fetch versions only for repositories backed by the
                    // generic forge release API. Generic MPQs have no remote
                    // source, while curated MPQs use dedicated update resolvers.
                    let mut version_tasks: Vec<Task<Message>> = Vec::new();
                    for repo in &app.repos {
                        if service::supports_release_version_listing(repo)
                            && !app.ignored_update_ids.contains(&repo.id)
                            && !app.repo_versions.contains_key(&repo.id)
                            && !app.repo_versions_loading.contains(&repo.id)
                        {
                            let db = app.db_path.clone();
                            let url = repo.url.clone();
                            let id = repo.id;
                            let scope = app.profile_operation_scope();
                            app.repo_versions_loading.insert(id);
                            version_tasks.push(Task::perform(
                                service::list_repo_versions(db, url),
                                move |result| {
                                    Message::FetchVersionsResult(crate::ProfileScoped::new(
                                        scope.clone(),
                                        (id, result),
                                    ))
                                },
                            ));
                        }
                    }

                    // Final summary rate fetch
                    let scope = app.profile_operation_scope();
                    version_tasks.push(Task::perform(
                        service::fetch_github_rate_limit(),
                        move |info| {
                            Message::UpdateCheckRateLimitResult(crate::ProfileScoped::new(
                                scope.clone(),
                                (stats.clone(), info),
                            ))
                        },
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
            if let Some(Dialog::AddRepo {
                ref url, ref mode, ..
            }) = app.dialog
            {
                if app.add_repo_manage_repo_id.is_some() {
                    return Some(Task::done(Message::SaveCollectionSelection));
                }

                let url = url.clone();
                let mut mode = mode.clone();
                let mut explicit_collection_mode = false;
                let selected_release_asset = app.add_repo_selected_release_asset.clone();
                let asset_regex = selected_release_asset
                    .as_deref()
                    .map(service::exact_asset_regex);
                if mode == "addon_git" && selected_release_asset.is_some() {
                    mode = "addon".to_string();
                }
                let selected_addons = if mode == "addon_git" {
                    let hinted = service::selected_addon_hint_from_url(&url);
                    let treat_as_collection = app.add_repo_manage_repo_id.is_some()
                        || hinted.is_some()
                        || app.add_repo_collection_choice == Some(true);
                    explicit_collection_mode = app.add_repo_collection_choice == Some(true);

                    // If the probe is still scanning, block submit so the user sees the choice prompt.
                    if app.add_repo_probe_loading {
                        app.show_toast(
                            "Scanning addon folders\u{2026} please wait a moment.",
                            ToastKind::Info,
                        );
                        return Some(Task::none());
                    }

                    if let Some(probe) = app.add_repo_probe.as_ref() {
                        let root_options = service::root_probe_addon_names(probe);
                        if root_options.len() > 1
                            && !treat_as_collection
                            && !app.add_repo_primary_toc_confirmed
                        {
                            let suggested = service::suggested_addon_for_expansion(
                                &root_options,
                                app.expansion_hint(),
                            );
                            app.dialog = Some(Dialog::SelectMainAddon {
                                url,
                                options: root_options,
                                suggested,
                                reinstall_repo_id: None,
                            });
                            return Some(Task::none());
                        }
                    }

                    // Collection must be opted into manually. Single-addon repos with
                    // multiple root TOCs are blocked above until one is explicitly chosen.

                    let mut selected = app
                        .add_repo_selected_addons
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>();
                    selected.sort_by_key(|name| name.to_ascii_lowercase());

                    if let Some(probe) = app.add_repo_probe.as_ref() {
                        if probe.addon_names.len() > 1 && treat_as_collection {
                            Some(selected.clone())
                        } else if !selected.is_empty() {
                            Some(std::mem::take(&mut selected))
                        } else {
                            let root_options = service::root_probe_addon_names(probe);
                            if root_options.len() > 1 {
                                service::suggested_addon_for_expansion(
                                    &root_options,
                                    app.expansion_hint(),
                                )
                                .or_else(|| root_options.first().cloned())
                                .map(|name| vec![name])
                            } else {
                                hinted.map(|name| vec![name])
                            }
                        }
                    } else if !selected.is_empty() {
                        Some(std::mem::take(&mut selected))
                    } else {
                        hinted.map(|name| vec![name])
                    }
                } else {
                    None
                };

                if explicit_collection_mode && selected_addons.is_none() {
                    app.log(
                        LogLevel::Error,
                        "Collection scan failed before submit. Re-scan the repo or switch Collection mode off.",
                    );
                    app.show_toast(
                        "Collection scan failed before submit. Re-scan the repo or switch Collection mode off.",
                        ToastKind::Warn,
                    );
                    return Some(Task::none());
                }

                if matches!(selected_addons.as_ref(), Some(selected) if selected.is_empty()) {
                    app.log(
                        LogLevel::Error,
                        "Select at least one addon from the collection.",
                    );
                    app.show_toast(
                        "Select at least one addon from the collection.",
                        ToastKind::Warn,
                    );
                    return Some(Task::none());
                }

                app.pending_add_repo_addon_names = if mode == "addon_git" {
                    selected_addons.clone().unwrap_or_else(|| {
                        app.add_repo_probe
                            .as_ref()
                            .map(|probe| probe.addon_names.clone())
                            .unwrap_or_default()
                    })
                } else {
                    Vec::new()
                };

                // Check if this mod requires an AV warning
                if is_av_false_positive(&url) {
                    app.dialog = Some(Dialog::AvWarning { url, mode });
                    return Some(Task::none());
                }

                let db = app.db_path.clone();
                app.dialog = None;
                crate::diagnostics::register_repository_url(&url);
                app.log(LogLevel::Info, "Adding repository...");
                let scope = app.profile_operation_scope();
                return Some(Task::perform(
                    service::add_repo(db, url, mode, asset_regex, selected_addons),
                    move |result| {
                        Message::AddRepoResult(crate::ProfileScoped::new(scope.clone(), result))
                    },
                ));
            }
            Some(Task::none())
        }
        Message::LocalArchiveHovered(path) => {
            app.local_archive_hover_path = if service::is_local_archive_path(&path) {
                Some(path)
            } else {
                None
            };
            Some(Task::none())
        }
        Message::LocalArchiveHoverLeft => {
            app.local_archive_hover_path = None;
            Some(Task::none())
        }
        Message::PickLocalAddonArchive => {
            let (dialog_url, dialog_mode) = match &app.dialog {
                Some(Dialog::AddRepo {
                    url,
                    mode,
                    is_addons: true,
                    ..
                }) => (url.clone(), mode.clone()),
                _ => return Some(Task::none()),
            };
            let request_id = app.next_async_request_id();
            app.pending_local_archive_picker = Some(request_id);
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("Addon archives", &["zip", "7z"])
                        .set_title("Select addon archive")
                        .pick_file()
                        .await
                        .map(|handle| handle.path().to_path_buf())
                },
                move |path| Message::LocalArchivePicked {
                    request_id,
                    scope: scope.clone(),
                    dialog_url: dialog_url.clone(),
                    dialog_mode: dialog_mode.clone(),
                    path,
                },
            ))
        }
        Message::LocalArchivePicked {
            request_id,
            scope,
            dialog_url,
            dialog_mode,
            path,
        } => {
            if app.pending_local_archive_picker != Some(request_id) {
                app.log(
                    LogLevel::Info,
                    "Discarded a superseded addon archive picker result.",
                );
                return Some(Task::none());
            }
            app.pending_local_archive_picker = None;
            if !scope.matches(&app.active_profile_id, app.profile_generation)
                || !matches!(
                    &app.dialog,
                    Some(Dialog::AddRepo {
                        url,
                        mode,
                        is_addons: true,
                        ..
                    }) if url == &dialog_url && mode == &dialog_mode
                )
            {
                app.log(
                    LogLevel::Info,
                    "Discarded an addon archive picker result after its profile or dialog changed.",
                );
                return Some(Task::none());
            }
            let Some(path) = path else {
                return Some(Task::none());
            };
            install_local_archive(app, path)
        }
        Message::LocalArchiveDropped(path) => {
            app.local_archive_hover_path = None;
            install_local_archive(app, path)
        }
        Message::AddRepoResult(scoped) => {
            let scope = scoped.scope.clone();
            let Some(result) = app.accept_profile_result(scoped, "repository add") else {
                return Some(Task::none());
            };
            let pending_addon_names = std::mem::take(&mut app.pending_add_repo_addon_names);
            match result {
                Ok(id) => {
                    app.log(LogLevel::Info, &format!("Repo added (id={}).", id));
                    if !app.wow_dir.is_empty() {
                        // Run a lightweight pre-install conflict check before installing.
                        let db = app.db_path.clone();
                        let wow = app.wow_dir.clone();
                        app.updating_repo_ids.insert(id);

                        // Collect all addon names that this repo will install.
                        let addon_names = if !pending_addon_names.is_empty() {
                            pending_addon_names
                        } else if !app.add_repo_selected_addons.is_empty() {
                            app.add_repo_selected_addons.iter().cloned().collect()
                        } else if app.add_repo_collection_choice == Some(true) {
                            app.add_repo_probe
                                .as_ref()
                                .map(|p| p.addon_names.clone())
                                .unwrap_or_default()
                        } else {
                            app.add_repo_probe
                                .as_ref()
                                .map(|p| {
                                    let root_names = service::root_probe_addon_names(p);
                                    if root_names.is_empty() {
                                        p.addon_names.clone()
                                    } else {
                                        root_names
                                    }
                                })
                                .unwrap_or_default()
                        };

                        app.log(LogLevel::Info, "Checking for conflicts\u{2026}");
                        return Some(Task::perform(
                            service::check_pre_install_conflicts(db, id, wow, addon_names),
                            move |result| Message::PreInstallConflictResult {
                                repo_id: id,
                                result: crate::ProfileScoped::new(scope.clone(), result),
                            },
                        ));
                    }
                    app.show_toast("Repo added successfully.", ToastKind::Info);
                    return Some(refresh_repos_task(app));
                }
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Add repo failed: {}", e));
                    let rate_limited =
                        app.show_github_rate_limit("The repository could not be added.", &e);
                    let error = crate::github_api::user_facing_error(&e);
                    if !rate_limited {
                        app.show_toast(format!("Add repo failed: {}", error), ToastKind::Error);
                    }
                    app.error = Some(error);
                }
            }
            Some(Task::none())
        }
        Message::PreInstallConflictResult {
            repo_id,
            result: scoped,
        } => {
            let scope = scoped.scope.clone();
            let Some(result) = app.accept_profile_result(scoped, "pre-install conflict check")
            else {
                return Some(Task::none());
            };
            let info = match result {
                Ok(info) => info,
                Err(e) => {
                    // Conflict check itself failed — log and proceed to install
                    // (the engine's own ADDON_CONFLICT guard is still active).
                    app.log(
                        LogLevel::Error,
                        &format!(
                            "Pre-install conflict check failed for repo id={}: {}",
                            repo_id, e
                        ),
                    );
                    service::PreInstallConflictInfo {
                        conflicts: Vec::new(),
                        existing_repos: Vec::new(),
                        new_repo_label: String::new(),
                        addon_names: Vec::new(),
                    }
                }
            };

            if info.conflicts.is_empty() {
                // No conflicts — proceed to install.
                let db = app.db_path.clone();
                let wow = app.wow_dir.clone();
                let opts = app.install_options();
                app.log(LogLevel::Info, "Installing\u{2026}");
                return Some(Task::perform(
                    service::install_new_repo(db, repo_id, wow, opts),
                    move |result| Message::InstallAfterAddResult {
                        repo_id,
                        result: crate::ProfileScoped::new(scope.clone(), result),
                    },
                ));
            }

            // Conflicts detected — show the rich two-panel dialog.
            app.updating_repo_ids.remove(&repo_id);
            let (url, mode) = app
                .repos
                .iter()
                .find(|r| r.id == repo_id)
                .map(|r| (r.url.clone(), r.mode.clone()))
                .unwrap_or_default();
            let (url, mode) = if url.is_empty() {
                if let Some(Dialog::AddRepo { url, mode, .. }) = app.dialog.as_ref() {
                    (url.clone(), mode.clone())
                } else {
                    (url, mode)
                }
            } else {
                (url, mode)
            };
            app.log(
                LogLevel::Error,
                &format!(
                    "Addon conflict detected for repo id={}: {} conflicting file(s).",
                    repo_id,
                    info.conflicts.len()
                ),
            );
            app.dialog = Some(Dialog::AddonConflict {
                url,
                mode,
                conflicts: info.conflicts,
                pending_repo_id: Some(repo_id),
                new_repo_label: info.new_repo_label,
                existing_repos: info.existing_repos,
                selected_addons: info.addon_names,
                new_repo_preview: app.add_repo_preview.as_ref().map(|p| p.files.clone()),
            });
            Some(refresh_repos_task(app))
        }
        Message::InstallAfterAddResult {
            repo_id,
            result: scoped,
        } => {
            let Some(result) = app.accept_profile_result(scoped, "repository installation") else {
                return Some(Task::none());
            };
            app.updating_repo_ids.remove(&repo_id);
            match result {
                Ok(msg) => {
                    app.log(LogLevel::Info, &msg);
                    app.show_toast(msg, ToastKind::Info);
                    let db = app.db_path.clone();
                    let scope = app.profile_operation_scope();
                    let prompt_task =
                        Task::perform(service::is_awesome_wotlk_repo(db, repo_id), move |result| {
                            Message::PromptAwesomeWotlkPatchIfInstalled(crate::ProfileScoped::new(
                                scope.clone(),
                                result,
                            ))
                        });
                    return Some(Task::batch(vec![refresh_repos_task(app), prompt_task]));
                }
                Err(ref e) if e.contains("ADDON_CONFLICT:") => {
                    // Fallback: the engine caught conflicts that the pre-check missed.
                    let conflict_names = parse_addon_conflict_error(e);
                    let conflicts: Vec<wuddle_engine::AddonProbeConflict> = conflict_names
                        .iter()
                        .map(|name| wuddle_engine::AddonProbeConflict {
                            addon_name: name.clone(),
                            target_path: String::new(),
                            owners: Vec::new(),
                        })
                        .collect();
                    app.log(
                        LogLevel::Error,
                        &format!("Addon conflict detected for repo id={}: {}", repo_id, e),
                    );
                    let (url, mode, new_label) = app
                        .repos
                        .iter()
                        .find(|r| r.id == repo_id)
                        .map(|r| {
                            (
                                r.url.clone(),
                                r.mode.clone(),
                                format!("{}/{}", r.owner, r.name),
                            )
                        })
                        .unwrap_or_default();
                    app.dialog = Some(Dialog::AddonConflict {
                        url,
                        mode,
                        conflicts,
                        pending_repo_id: Some(repo_id),
                        new_repo_label: new_label,
                        existing_repos: Vec::new(),
                        selected_addons: conflict_names,
                        new_repo_preview: app.add_repo_preview.as_ref().map(|p| p.files.clone()),
                    });
                    return Some(Task::none());
                }
                Err(ref e) if e.contains("FILE_CONFLICT:") => {
                    show_file_conflict(app, repo_id, FileConflictAction::Install, e);
                    return Some(Task::none());
                }
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Install failed: {}", e));
                    app.show_toast(format!("Install failed: {}", e), ToastKind::Error);
                }
            }
            app.updating_repo_ids.remove(&repo_id);
            Some(refresh_repos_task(app))
        }
        Message::CancelConflictInstall { repo_id } => {
            // User clicked Cancel on the conflict dialog for a freshly-added repo.
            // Remove the repo from the DB so it doesn't remain tracked without files.
            app.dialog = None;
            let db = app.db_path.clone();
            app.log(
                LogLevel::Info,
                &format!("Conflict cancelled, removing repo id={}.", repo_id),
            );
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::remove_repo(db, repo_id, None, false),
                move |result| Message::CancelConflictInstallResult {
                    repo_id,
                    result: crate::ProfileScoped::new(
                        scope.clone(),
                        result.map(|_removed_paths| ()),
                    ),
                },
            ))
        }
        Message::CancelConflictInstallResult {
            repo_id,
            result: scoped,
        } => {
            let Some(result) = app.accept_profile_result(scoped, "cancelled-install cleanup")
            else {
                return Some(Task::none());
            };
            match result {
                Ok(()) => app.log(
                    LogLevel::Info,
                    &format!("Cancelled install cleanup completed for repo id={repo_id}."),
                ),
                Err(error) => {
                    app.log(
                        LogLevel::Error,
                        &format!("Cancelled install cleanup failed for repo id={repo_id}: {error}"),
                    );
                    app.show_toast(
                        "The cancelled repository could not be removed from tracking. Refresh and retry removal.",
                        ToastKind::Error,
                    );
                }
            }
            Some(refresh_repos_task(app))
        }
        Message::InstallConflictOverride { repo_id } => {
            // The user confirmed overwriting conflicts for an already-added repo.
            app.dialog = None;
            if app.wow_dir.is_empty() {
                return Some(Task::none());
            }
            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            let mut opts = app.install_options();
            opts.replace_addon_conflicts = true;
            app.log(
                LogLevel::Info,
                &format!(
                    "Overwriting conflicts and installing repo id={}...",
                    repo_id
                ),
            );
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::install_new_repo(db, repo_id, wow, opts),
                move |result| Message::InstallAfterAddResult {
                    repo_id,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::ConfirmFileConflict { repo_id, action } => {
            app.dialog = None;
            if app.wow_dir.is_empty() {
                return Some(Task::none());
            }
            app.updating_repo_ids.insert(repo_id);
            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            let mut opts = app.install_options();
            opts.replace_file_conflicts = true;
            opts.replace_local_changes =
                matches!(action, FileConflictAction::UpdateApprovedLocalChanges);
            let scope = app.profile_operation_scope();
            app.log(
                LogLevel::Info,
                &format!("Approved backed-up file replacement for repo id={repo_id}."),
            );
            match action {
                FileConflictAction::Install => Some(Task::perform(
                    service::install_new_repo(db, repo_id, wow, opts),
                    move |result| Message::InstallAfterAddResult {
                        repo_id,
                        result: crate::ProfileScoped::new(scope.clone(), result),
                    },
                )),
                FileConflictAction::Update | FileConflictAction::UpdateApprovedLocalChanges => {
                    Some(Task::perform(
                        service::update_repo(db, repo_id, wow, opts),
                        move |result| Message::UpdateRepoResult {
                            repo_id,
                            replace_local_changes: matches!(
                                action,
                                FileConflictAction::UpdateApprovedLocalChanges
                            ),
                            result: crate::ProfileScoped::new(scope.clone(), result),
                        },
                    ))
                }
                FileConflictAction::Reinstall => Some(Task::perform(
                    service::reinstall_repo(db, repo_id, wow, opts),
                    move |result| Message::ReinstallRepoResult {
                        repo_id,
                        result: crate::ProfileScoped::new(scope.clone(), result),
                    },
                )),
            }
        }
        Message::InstallRepoOverride { url, mode } => {
            // Re-add from scratch with replace_addon_conflicts = true so the engine
            // skips its own conflict guard on this install.
            let db = app.db_path.clone();
            app.dialog = None;
            app.reset_add_repo_state();
            crate::diagnostics::register_repository_url(&url);
            app.log(
                LogLevel::Info,
                "Adding repository with conflict override...",
            );
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::add_repo(db, url, mode, None, None),
                move |result| {
                    Message::AddRepoResult(crate::ProfileScoped::new(scope.clone(), result))
                },
            ))
        }
        Message::OpenCollectionManager(repo_id) => {
            let Some(repo) = app.repos.iter().find(|repo| repo.id == repo_id).cloned() else {
                return Some(Task::none());
            };

            app.open_menu = None;
            app.add_new_menu_open = false;
            app.reset_add_repo_state();
            app.add_repo_manage_repo_id = Some(repo_id);
            app.add_repo_collection_choice = Some(true);
            let initial_selection = if repo.selected_addons.is_empty() {
                repo.installed_addons.clone()
            } else {
                repo.selected_addons.clone()
            };
            app.add_repo_existing_addons = initial_selection.iter().cloned().collect();
            app.add_repo_selected_addons = initial_selection.into_iter().collect();
            app.dialog = Some(Dialog::AddRepo {
                url: repo.url.clone(),
                mode: repo.mode.clone(),
                is_addons: true,
                advanced: false,
            });

            let mut tasks = vec![iced::widget::operation::focus(iced::widget::Id::new(
                "add_repo_url",
            ))];
            tasks.push(Task::done(Message::FetchRepoPreview(repo.url.clone())));
            if !app.wow_dir.trim().is_empty() {
                tasks.push(Task::done(Message::FetchCollectionProbe(repo.url)));
            }
            Some(Task::batch(tasks))
        }
        Message::FetchCollectionProbe(url) => {
            app.add_repo_probe_loading = true;
            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            let probe_url = url.clone();
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::probe_conflicts(db, url, wow),
                move |result| {
                    Message::FetchCollectionProbeResult(crate::ProfileScoped::new(
                        scope.clone(),
                        (probe_url.clone(), result),
                    ))
                },
            ))
        }
        Message::FetchCollectionProbeResult(scoped) => {
            let Some((url, result)) = app.accept_profile_result(scoped, "collection probe") else {
                return Some(Task::none());
            };
            app.add_repo_probe_loading = false;
            if let Some(Dialog::AddRepo {
                url: current_url, ..
            }) = app.dialog.as_ref()
            {
                if service::normalize_repo_input_url(current_url)
                    != service::normalize_repo_input_url(&url)
                {
                    return Some(Task::none());
                }
            }
            match result {
                Ok(probe) => {
                    let hinted_addon =
                        if let Some(Dialog::AddRepo { url, .. }) = app.dialog.as_ref() {
                            service::selected_addon_hint_from_url(url)
                        } else {
                            None
                        };
                    let detected_names = probe
                        .addon_names
                        .iter()
                        .map(|name| name.to_ascii_lowercase())
                        .collect::<HashSet<_>>();
                    if hinted_addon.is_some() || app.add_repo_manage_repo_id.is_some() {
                        app.add_repo_collection_choice = Some(true);
                    }

                    let old_selected: Vec<String> =
                        std::mem::take(&mut app.add_repo_selected_addons)
                            .into_iter()
                            .collect();
                    for selected_name in old_selected {
                        let name_lower = selected_name.to_ascii_lowercase();
                        if detected_names.contains(&name_lower) {
                            app.add_repo_selected_addons.insert(selected_name);
                            continue;
                        }
                        let path_prefix = format!("{}/", name_lower);
                        for entry in &probe.addon_entries {
                            let src = entry.source_path.to_ascii_lowercase();
                            if src == name_lower || src.starts_with(&path_prefix) {
                                app.add_repo_selected_addons
                                    .insert(entry.addon_name.clone());
                            }
                        }
                    }
                    if app.add_repo_selected_addons.is_empty()
                        && app.add_repo_collection_choice == Some(true)
                    {
                        if let Some(hint) = hinted_addon {
                            let hint_key = hint.to_ascii_lowercase();
                            if detected_names.contains(&hint_key) {
                                app.add_repo_selected_addons = probe
                                    .addon_names
                                    .iter()
                                    .filter(|name| name.eq_ignore_ascii_case(&hint))
                                    .cloned()
                                    .collect();
                            }
                        }
                    }

                    // Update AddonConflict dialog if visible for this repo
                    if let Some(Dialog::AddonConflict {
                        url: ref d_url,
                        ref mut selected_addons,
                        ..
                    }) = app.dialog
                    {
                        if service::normalize_repo_input_url(d_url)
                            == service::normalize_repo_input_url(&url)
                        {
                            *selected_addons = probe.addon_names.clone();
                        }
                    }

                    app.add_repo_probe = Some(probe);

                    if let Some(probe) = app.add_repo_probe.as_ref() {
                        if probe.addon_names.len() > 1
                            && app.add_repo_manage_repo_id.is_none()
                            && matches!(app.dialog, Some(Dialog::AddRepo { .. }))
                        {
                            let root_options = service::root_probe_addon_names(probe);
                            if root_options.len() > 1
                                && app.add_repo_collection_choice != Some(true)
                                && !app.add_repo_primary_toc_confirmed
                            {
                                let suggested = service::suggested_addon_for_expansion(
                                    &root_options,
                                    app.expansion_hint(),
                                );
                                app.dialog = Some(Dialog::SelectMainAddon {
                                    url: url.clone(),
                                    options: root_options,
                                    suggested,
                                    reinstall_repo_id: None,
                                });
                            } else if root_options.is_empty()
                                && app.add_repo_collection_choice.is_none()
                            {
                                app.dialog = Some(Dialog::CollectionChoice {
                                    url: url.clone(),
                                    addon_names: probe.addon_names.clone(),
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    app.add_repo_probe = None;
                    app.log(LogLevel::Error, &format!("Addon probe failed: {:#}", e));
                }
            }
            if matches!(app.dialog, Some(Dialog::AddRepo { .. })) {
                return Some(delayed_add_repo_url_refocus_task());
            }
            Some(Task::none())
        }
        Message::SetAddRepoCollectionMode(is_collection) => {
            let from_primary_toc_choice =
                matches!(app.dialog, Some(Dialog::SelectMainAddon { .. }));
            app.add_repo_collection_choice = Some(is_collection);
            app.add_repo_primary_toc_confirmed = false;
            if is_collection {
                if let Some(probe) = app.add_repo_probe.as_ref() {
                    if app.add_repo_selected_addons.is_empty() || from_primary_toc_choice {
                        app.add_repo_selected_addons = probe.addon_names.iter().cloned().collect();
                    }
                }
            } else if app.add_repo_selected_addons.len() != 1 {
                app.add_repo_selected_addons.clear();
            }
            // If we came from a choice popup, restore the AddRepo dialog.
            let restore_url = match app.dialog.as_ref() {
                Some(Dialog::CollectionChoice { url, .. })
                | Some(Dialog::SelectMainAddon { url, .. }) => Some(url.clone()),
                _ => None,
            };
            if let Some(url) = restore_url {
                app.dialog = Some(Dialog::AddRepo {
                    url,
                    mode: "addon_git".to_string(),
                    is_addons: true,
                    advanced: false,
                });
            }
            Some(Task::none())
        }
        Message::SetCollectionSelection(selected_addons) => {
            let selected_count = selected_addons.len();
            app.add_repo_selected_addons = selected_addons.into_iter().collect();
            app.log(
                LogLevel::Info,
                &format!("Collection selection set to {} addon(s).", selected_count),
            );
            app.show_toast(
                if selected_count == 0 {
                    "Deselected all collection addons.".to_string()
                } else {
                    format!("Selected all {} collection addons.", selected_count)
                },
                ToastKind::Info,
            );
            Some(Task::none())
        }
        Message::SetAddRepoPrimaryAddon(name) => {
            let reinstall_repo_id = match app.dialog.as_ref() {
                Some(Dialog::SelectMainAddon {
                    reinstall_repo_id, ..
                }) => *reinstall_repo_id,
                _ => None,
            };
            if let Some(repo_id) = reinstall_repo_id {
                app.dialog = None;
                app.log(
                    LogLevel::Info,
                    &format!(
                        "Clean-reinstalling repo id={} with {}.toc...",
                        repo_id, name
                    ),
                );
                let db = app.db_path.clone();
                let wow = app.wow_dir.clone();
                let opts = app.install_options();
                let scope = app.profile_operation_scope();
                app.updating_repo_ids.insert(repo_id);
                return Some(Task::perform(
                    service::reinstall_repo_with_selection(db, repo_id, wow, opts, name),
                    move |result| Message::ReinstallRepoResult {
                        repo_id,
                        result: crate::ProfileScoped::new(scope.clone(), result),
                    },
                ));
            }

            app.add_repo_selected_addons.clear();
            if !name.is_empty() {
                app.add_repo_selected_addons.insert(name);
                app.add_repo_primary_toc_confirmed = true;
            }
            if let Some(Dialog::SelectMainAddon { url, .. }) = app.dialog.as_ref() {
                let url = url.clone();
                app.dialog = Some(Dialog::AddRepo {
                    url,
                    mode: "addon_git".to_string(),
                    is_addons: true,
                    advanced: false,
                });
            }
            if matches!(app.dialog, Some(Dialog::AddRepo { .. })) {
                return Some(delayed_add_repo_url_refocus_task());
            }
            Some(Task::none())
        }
        Message::ToggleCollectionFolder(folder_name) => {
            let folder_display_name = folder_name
                .rsplit('/')
                .next()
                .unwrap_or(folder_name.as_str())
                .to_string();
            let folder_path_key = folder_name.trim_matches('/').to_ascii_lowercase();
            let folder_path_prefix = format!("{}/", folder_path_key);
            let folder_key = service::normalize_collection_entry_key(&folder_display_name);
            let mut matching_addons = app
                .add_repo_probe
                .as_ref()
                .map(|probe| {
                    probe
                        .addon_entries
                        .iter()
                        .filter(|entry| {
                            let source_path = entry.source_path.to_ascii_lowercase();
                            let source_top = entry
                                .source_path
                                .split('/')
                                .next()
                                .unwrap_or(entry.addon_name.as_str());
                            source_path == folder_path_key
                                || source_path.starts_with(&folder_path_prefix)
                                || service::normalize_collection_entry_key(source_top) == folder_key
                                || service::normalize_collection_entry_key(&entry.addon_name)
                                    == folder_key
                        })
                        .map(|entry| entry.addon_name.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            if matching_addons.is_empty() {
                if let Some(probe) = app.add_repo_probe.as_ref() {
                    matching_addons.extend(
                        probe
                            .addon_names
                            .iter()
                            .filter(|addon_name| {
                                service::normalize_collection_entry_key(addon_name) == folder_key
                            })
                            .cloned(),
                    );
                }
            }

            if matching_addons.is_empty() {
                matching_addons.extend(
                    app.add_repo_selected_addons
                        .iter()
                        .filter(|addon_name| {
                            service::normalize_collection_entry_key(addon_name) == folder_key
                        })
                        .cloned(),
                );
            }

            if matching_addons.is_empty() {
                matching_addons.extend(
                    app.add_repo_existing_addons
                        .iter()
                        .filter(|addon_name| {
                            service::normalize_collection_entry_key(addon_name) == folder_key
                        })
                        .cloned(),
                );
            }

            if matching_addons.is_empty() {
                // When the probe is unavailable, keep the full folder path so selection state
                // can still propagate to descendant preview rows and later resolve by path prefix.
                matching_addons.push(folder_name.clone());
            }

            matching_addons.sort_by_key(|name| name.to_ascii_lowercase());
            matching_addons.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

            let resolved_addons = matching_addons.join(", ");
            let folder_path_lower = folder_name.trim().trim_matches('/').to_ascii_lowercase();
            let descendant_prefix = format!("{}/", folder_path_lower);

            let all_selected = matching_addons.iter().all(|name| {
                app.add_repo_selected_addons.iter().any(|selected| {
                    selected.eq_ignore_ascii_case(name)
                        || service::normalize_collection_entry_key(selected)
                            == service::normalize_collection_entry_key(name)
                })
            }) || app.add_repo_selected_addons.iter().any(|selected| {
                let selected_path = selected.trim().trim_matches('/').to_ascii_lowercase();
                selected_path == folder_path_lower
            });

            let has_any_selected = all_selected
                || app.add_repo_selected_addons.iter().any(|selected| {
                    let selected_path = selected.trim().trim_matches('/').to_ascii_lowercase();
                    selected_path == folder_path_lower
                        || selected_path.starts_with(&descendant_prefix)
                });

            if has_any_selected {
                app.add_repo_selected_addons.retain(|selected| {
                    let selected_path = selected.trim().trim_matches('/').to_ascii_lowercase();
                    !matching_addons.iter().any(|addon_name| {
                        addon_name.eq_ignore_ascii_case(selected)
                            || service::normalize_collection_entry_key(addon_name)
                                == service::normalize_collection_entry_key(selected)
                    }) && selected_path != folder_path_lower
                        && !selected_path.starts_with(&descendant_prefix)
                });
            } else {
                for addon_name in matching_addons {
                    if !app.add_repo_selected_addons.iter().any(|selected| {
                        selected.eq_ignore_ascii_case(&addon_name)
                            || service::normalize_collection_entry_key(selected)
                                == service::normalize_collection_entry_key(&addon_name)
                    }) {
                        app.add_repo_selected_addons.insert(addon_name);
                    }
                }
            }

            app.log(
                LogLevel::Info,
                &format!(
                    "Collection folder '{}' toggled via '{}'. Resolved addons: [{}]. {} addon(s) now selected.",
                    folder_display_name,
                    folder_name,
                    resolved_addons,
                    app.add_repo_selected_addons.len()
                ),
            );
            app.show_toast(
                format!(
                    "{} {}",
                    if has_any_selected {
                        "Marked for removal:"
                    } else {
                        "Marked to keep/install:"
                    },
                    folder_display_name
                ),
                ToastKind::Info,
            );
            Some(Task::none())
        }
        Message::ToggleCollectionAddon(addon_name) => {
            let addon_key = service::normalize_collection_entry_key(&addon_name);
            let mut matching_addons = app
                .add_repo_probe
                .as_ref()
                .map(|probe| {
                    probe
                        .addon_entries
                        .iter()
                        .filter(|entry| {
                            let source_top = entry
                                .source_path
                                .split('/')
                                .next()
                                .unwrap_or(entry.addon_name.as_str());
                            service::normalize_collection_entry_key(source_top) == addon_key
                                || service::normalize_collection_entry_key(&entry.addon_name)
                                    == addon_key
                        })
                        .map(|entry| entry.addon_name.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            if matching_addons.is_empty() {
                matching_addons.extend(
                    app.add_repo_selected_addons
                        .iter()
                        .chain(app.add_repo_existing_addons.iter())
                        .filter(|selected| {
                            service::normalize_collection_entry_key(selected) == addon_key
                        })
                        .cloned(),
                );
            }

            if matching_addons.is_empty() {
                matching_addons.push(addon_name.clone());
            }

            matching_addons.sort_by_key(|name| name.to_ascii_lowercase());
            matching_addons.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

            let already_selected = matching_addons.iter().all(|name| {
                app.add_repo_selected_addons.iter().any(|selected| {
                    selected.eq_ignore_ascii_case(name)
                        || service::normalize_collection_entry_key(selected)
                            == service::normalize_collection_entry_key(name)
                })
            });

            if already_selected {
                app.add_repo_selected_addons.retain(|selected| {
                    !matching_addons.iter().any(|name| {
                        selected.eq_ignore_ascii_case(name)
                            || service::normalize_collection_entry_key(selected)
                                == service::normalize_collection_entry_key(name)
                    })
                });
            } else {
                for resolved_name in matching_addons {
                    if !app.add_repo_selected_addons.iter().any(|selected| {
                        selected.eq_ignore_ascii_case(&resolved_name)
                            || service::normalize_collection_entry_key(selected)
                                == service::normalize_collection_entry_key(&resolved_name)
                    }) {
                        app.add_repo_selected_addons.insert(resolved_name);
                    }
                }
            }

            app.log(
                LogLevel::Info,
                &format!(
                    "Collection addon '{}' toggled. {} addon(s) now selected.",
                    addon_name,
                    app.add_repo_selected_addons.len()
                ),
            );
            app.show_toast(
                format!(
                    "{} {}",
                    if already_selected {
                        "Marked for removal:"
                    } else {
                        "Marked to keep/install:"
                    },
                    addon_name
                ),
                ToastKind::Info,
            );
            Some(Task::none())
        }
        Message::SaveCollectionSelection => {
            let Some(repo_id) = app.add_repo_manage_repo_id else {
                return Some(Task::none());
            };
            if app.wow_dir.trim().is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
                return Some(Task::none());
            }

            let mut selected = app
                .add_repo_selected_addons
                .iter()
                .flat_map(|selected_name| {
                    let selected_key = service::normalize_collection_entry_key(selected_name);
                    let mut resolved = app
                        .add_repo_probe
                        .as_ref()
                        .map(|probe| {
                            probe
                                .addon_entries
                                .iter()
                                .filter(|entry| {
                                    let source_top = entry
                                        .source_path
                                        .split('/')
                                        .next()
                                        .unwrap_or(entry.addon_name.as_str());
                                    let source_path_lower = entry.source_path.to_ascii_lowercase();
                                    service::normalize_collection_entry_key(source_top)
                                        == selected_key
                                        || service::normalize_collection_entry_key(
                                            &entry.addon_name,
                                        ) == selected_key
                                        || source_path_lower == selected_key
                                        || source_path_lower
                                            .starts_with(&format!("{}/", selected_key))
                                })
                                .map(|entry| entry.addon_name.clone())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    if resolved.is_empty() {
                        resolved.push(selected_name.clone());
                    }

                    resolved
                })
                .collect::<Vec<_>>();
            selected.sort_by_key(|name| name.to_ascii_lowercase());
            selected.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            let opts = app.install_options();
            app.dialog = None;
            app.log(
                LogLevel::Info,
                &format!("Saving collection selection for repo id={}...", repo_id),
            );
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::update_collection_selection(db, repo_id, wow, selected, opts),
                move |result| {
                    Message::SaveCollectionSelectionResult(crate::ProfileScoped::new(
                        scope.clone(),
                        result,
                    ))
                },
            ))
        }
        Message::SaveCollectionSelectionOverride {
            repo_id,
            selected_addons,
        } => {
            if app.wow_dir.trim().is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
                return Some(Task::none());
            }

            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            let mut opts = app.install_options();
            opts.replace_addon_conflicts = true;
            app.dialog = None;
            app.log(
                LogLevel::Info,
                &format!("Retrying collection selection for repo id={} with conflict replacement enabled...", repo_id),
            );
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::update_collection_selection(db, repo_id, wow, selected_addons, opts),
                move |result| {
                    Message::SaveCollectionSelectionResult(crate::ProfileScoped::new(
                        scope.clone(),
                        result,
                    ))
                },
            ))
        }
        Message::SaveCollectionSelectionResult(scoped) => {
            let Some(result) = app.accept_profile_result(scoped, "collection selection update")
            else {
                return Some(Task::none());
            };
            match result {
                Ok(msg) => {
                    app.log(LogLevel::Info, &msg);
                    app.show_toast(msg, ToastKind::Info);
                    app.reset_add_repo_state();
                    return Some(refresh_repos_task(app));
                }
                Err(service::CollectionSelectionError::Conflict {
                    repo_id,
                    repo_name,
                    repo_url,
                    selected_addons,
                    conflicts,
                    existing_repos,
                }) => {
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "Collection update for '{}' requires replacing {} conflicting addon(s).",
                            repo_name,
                            conflicts.len()
                        ),
                    );
                    app.add_repo_selected_addons = app.add_repo_existing_addons.clone();
                    app.dialog = Some(Dialog::CollectionAddonConflict {
                        repo_id,
                        repo_name,
                        repo_url,
                        selected_addons,
                        conflicts,
                        existing_repos,
                    });
                    return Some(Task::none());
                }
                Err(service::CollectionSelectionError::Other(e)) => {
                    app.log(LogLevel::Error, &format!("Collection update failed: {}", e));
                    app.show_toast(format!("Collection update failed: {}", e), ToastKind::Error);
                }
            }
            app.reset_add_repo_state();
            Some(refresh_repos_task(app))
        }
        Message::BrowseAddonInstall {
            repo_id,
            addon_name,
        } => {
            app.open_menu = None;
            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            if wow.is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
            } else {
                return Some(Task::perform(
                    service::open_addon_folder(db, repo_id, wow.into(), addon_name),
                    |_| Message::CloseMenu,
                ));
            }
            Some(Task::none())
        }
        Message::RemoveCollectionAddonPrompt {
            repo_id,
            addon_name,
        } => {
            let repo_name = app
                .repos
                .iter()
                .find(|repo| repo.id == repo_id)
                .map(|repo| format!("{}/{}", repo.owner, repo.name))
                .unwrap_or_else(|| format!("repo#{}", repo_id));
            app.open_menu = None;
            app.dialog = Some(Dialog::RemoveCollectionAddon {
                repo_id,
                repo_name,
                addon_name: addon_name.clone(),
                files: vec![(
                    format!("Interface/AddOns/{}", addon_name),
                    "addon".to_string(),
                )],
            });
            Some(Task::none())
        }
        Message::RemoveCollectionAddonConfirm {
            repo_id,
            addon_name,
        } => {
            let Some(repo) = app.repos.iter().find(|repo| repo.id == repo_id).cloned() else {
                return Some(Task::none());
            };

            if app.wow_dir.trim().is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
                return Some(Task::none());
            }

            let mut selected = if repo.selected_addons.is_empty() {
                repo.installed_addons.clone()
            } else {
                repo.selected_addons.clone()
            };
            selected.retain(|name| !name.eq_ignore_ascii_case(&addon_name));

            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            let opts = app.install_options();
            app.dialog = None;
            app.log(
                LogLevel::Info,
                &format!(
                    "Removing '{}' from collection repo id={}...",
                    addon_name, repo_id
                ),
            );
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::update_collection_selection(db, repo_id, wow, selected, opts),
                move |result| {
                    Message::SaveCollectionSelectionResult(crate::ProfileScoped::new(
                        scope.clone(),
                        result,
                    ))
                },
            ))
        }
        Message::RemoveRepoConfirm(id, remove_files) => {
            let repo_name = app
                .repos
                .iter()
                .find(|repo| repo.id == id)
                .map(|repo| repo.name.clone())
                .unwrap_or_else(|| format!("repository #{id}"));
            let db = app.db_path.clone();
            let wow = if app.wow_dir.is_empty() {
                None
            } else {
                Some(app.wow_dir.clone())
            };
            app.dialog = None;
            app.log(
                LogLevel::Info,
                &format!(
                    "Removing \"{repo_name}\" (repo id={id}; remove_local_files={remove_files})..."
                ),
            );
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::remove_repo(db, id, wow, remove_files),
                move |result| Message::RemoveRepoResult {
                    repo_id: id,
                    repo_name: repo_name.clone(),
                    remove_files,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::RemoveRepoResult {
            repo_id,
            repo_name,
            remove_files,
            result,
        } => {
            let Some(result) = app.accept_profile_result(result, "repository removal") else {
                return Some(Task::none());
            };
            match result {
                Ok(removed_paths) => {
                    forget_repo_update_plan(app, repo_id);
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "Removed \"{repo_name}\" (repo id={repo_id}; remove_local_files={remove_files}; filesystem_targets_removed_or_restored={removed_paths}; metadata_committed=true)."
                        ),
                    );
                    app.show_toast("Repo removed.", ToastKind::Info);
                    return Some(refresh_repos_task(app));
                }
                Err(e) => {
                    app.log(
                        LogLevel::Error,
                        &format!(
                            "Removal failed for \"{repo_name}\" (repo id={repo_id}; remove_local_files={remove_files}): {e}"
                        ),
                    );
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
            let repo_name = app
                .repos
                .iter()
                .find(|r| r.id == id)
                .map(|r| r.name.as_str())
                .unwrap_or("?");
            app.log(
                LogLevel::Info,
                &format!(
                    "Repo '{}': updates {}.",
                    repo_name,
                    if was_ignored { "unignored" } else { "ignored" }
                ),
            );
            Some(Task::none())
        }
        Message::ToggleMergeInstalls(id, merge) => {
            let repo_name = app
                .repos
                .iter()
                .find(|r| r.id == id)
                .map(|r| r.name.clone())
                .unwrap_or_default();
            app.log(
                LogLevel::Info,
                &format!(
                    "Repo '{}': merge installs {}.",
                    repo_name,
                    if merge { "enabled" } else { "disabled" }
                ),
            );
            let db = app.db_path.clone();
            let scope = app.profile_operation_scope();
            Some(iced::Task::perform(
                service::set_merge_installs(db, id, merge),
                move |result| {
                    Message::ToggleMergeInstallsResult(crate::ProfileScoped::new(
                        scope.clone(),
                        result,
                    ))
                },
            ))
        }
        Message::ToggleMergeInstallsResult(scoped) => {
            let Some(result) = app.accept_profile_result(scoped, "merge-install setting update")
            else {
                return Some(Task::none());
            };
            match result {
                Ok(_id) => Some(refresh_repos_task(app)),
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Toggle merge failed: {}", e));
                    Some(Task::none())
                }
            }
        }
        Message::FetchVersions(id) => {
            let db = app.db_path.clone();
            let url = app
                .repos
                .iter()
                .find(|repo| repo.id == id && service::supports_release_version_listing(repo))
                .map(|repo| repo.url.clone());
            if let Some(url) = url {
                let scope = app.profile_operation_scope();
                return Some(Task::perform(
                    async move {
                        let res = service::list_repo_versions(db, url).await;
                        (id, res)
                    },
                    move |result| {
                        Message::FetchVersionsResult(crate::ProfileScoped::new(
                            scope.clone(),
                            result,
                        ))
                    },
                ));
            }
            Some(Task::none())
        }
        Message::FetchVersionsResult(scoped) => {
            let Some((id, result)) = app.accept_profile_result(scoped, "repository-version load")
            else {
                return Some(Task::none());
            };
            match result {
                Ok(versions) => {
                    app.repo_versions.insert(id, versions);
                }
                Err(e) => {
                    let name = app
                        .repos
                        .iter()
                        .find(|r| r.id == id)
                        .map(|r| r.name.as_str())
                        .unwrap_or("?");
                    app.log(
                        LogLevel::Error,
                        &format!("Fetch versions failed for '{}': {}", name, e),
                    );
                    app.show_github_rate_limit("Versions could not be loaded.", &e);
                }
            }
            Some(Task::none())
        }
        Message::SetPinnedVersion(id, version) => {
            // Reflect the user's selection immediately. The database write is
            // asynchronous, so without this the picker can redraw with its old
            // value and look as though the choice was ignored.
            if let Some(repo) = app.repos.iter_mut().find(|repo| repo.id == id) {
                repo.pinned_version = version.clone();
            }
            let db = app.db_path.clone();
            let v_str = version.clone().unwrap_or_else(|| "none".to_string());
            app.log(
                LogLevel::Info,
                &format!("Pinning version to '{}' for repo id={}...", v_str, id),
            );
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::set_pinned_version(db, id, version),
                move |result| {
                    Message::SetPinnedVersionResult(crate::ProfileScoped::new(
                        scope.clone(),
                        result,
                    ))
                },
            ))
        }
        Message::SetPinnedVersionResult(scoped) => {
            let Some(result) = app.accept_profile_result(scoped, "pinned-version update") else {
                return Some(Task::none());
            };
            match result {
                Ok(_id) => {
                    app.log(
                        LogLevel::Info,
                        "Version pin updated. Re-checking updates...",
                    );
                    // A version choice must always be evaluated, even when this repo
                    // would normally be skipped by the low-frequency API check.
                    Some(Task::batch(vec![
                        refresh_repos_task(app),
                        check_updates_for_version_change_task(app),
                    ]))
                }
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Set version failed: {}", e));
                    // Restore the persisted selection if the optimistic update above
                    // could not be saved.
                    Some(refresh_repos_task(app))
                }
            }
        }
        Message::DllCountWarningChoice { repo_id, merge } => {
            app.dialog = None;
            if merge {
                let db = app.db_path.clone();
                let scope = app.profile_operation_scope();
                Some(Task::batch(vec![
                    Task::perform(
                        service::set_merge_installs(db, repo_id, true),
                        move |result| {
                            Message::ToggleMergeInstallsResult(crate::ProfileScoped::new(
                                scope.clone(),
                                result,
                            ))
                        },
                    ),
                    Task::done(Message::UpdateRepo(repo_id)),
                ]))
            } else {
                Some(Task::done(Message::UpdateRepo(repo_id)))
            }
        }
        Message::BrowseRepo(id) => {
            app.open_menu = None;
            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            if wow.is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
            } else {
                return Some(Task::perform(
                    service::open_repo_folder(db, id, wow.into()),
                    |_| Message::CloseMenu,
                ));
            }
            Some(Task::none())
        }
        Message::BrowseGamePath(path) => {
            app.open_menu = None;
            let wow = app.wow_dir.clone();
            if wow.is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
                return Some(Task::none());
            }
            Some(Task::perform(
                service::open_game_path_folder(wow.into(), path),
                Message::BrowseGamePathResult,
            ))
        }
        Message::BrowseGamePathResult(result) => {
            if let Err(error) = result {
                app.log(LogLevel::Error, &format!("Browse failed: {error}"));
                app.show_toast(format!("Could not open folder: {error}"), ToastKind::Error);
            }
            Some(Task::none())
        }
        Message::UpdateRepo(id) => {
            app.open_menu = None;
            if app.updating_repo_ids.contains(&id) {
                app.show_toast(
                    "That repository already has an operation in progress.",
                    ToastKind::Warn,
                );
                return Some(Task::none());
            }
            if app
                .repos
                .iter()
                .find(|repo| repo.id == id)
                .map(service::is_wdm_repo)
                .unwrap_or(false)
            {
                return Some(Task::done(Message::OpenWdm));
            }
            if app
                .repos
                .iter()
                .find(|repo| repo.id == id)
                .map(service::is_epoch_water_repo)
                .unwrap_or(false)
            {
                return Some(Task::done(Message::InstallEpochWater));
            }
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
                    app.log(
                        LogLevel::Info,
                        &format!("Updating {}/{}...", repo.owner, repo.name),
                    );
                }
                app.updating_repo_ids.insert(id);
                let db = app.db_path.clone();
                let wow = app.wow_dir.clone();
                let opts = app.install_options();
                let scope = app.profile_operation_scope();
                return Some(Task::perform(
                    service::update_repo(db, id, wow, opts),
                    move |result| Message::UpdateRepoResult {
                        repo_id: id,
                        replace_local_changes: false,
                        result: crate::ProfileScoped::new(scope.clone(), result),
                    },
                ));
            }
            Some(Task::none())
        }
        Message::UpdateRepoResult {
            repo_id,
            replace_local_changes,
            result: scoped,
        } => {
            let Some(result) = app.accept_profile_result(scoped, "repository update") else {
                return Some(Task::none());
            };
            app.updating_repo_ids.remove(&repo_id);
            match result {
                Ok(Some(plan)) => {
                    let name = format!("{}/{}", plan.owner, plan.name);
                    app.log(LogLevel::Info, &format!("Updated {}.", name));
                    app.show_toast(format!("Updated {}.", name), ToastKind::Info);
                    // Remove from plans so it disappears from 'Updates' list in UI immediately
                    app.plans.retain(|p| p.repo_id != plan.repo_id);
                    sync_active_plan_cache(app);
                }
                Ok(None) => app.log(LogLevel::Info, "Already up to date."),
                Err(e) => {
                    if e.starts_with("ADDON_GIT_LOCAL_CHANGES:") {
                        let entry = addon_local_changes_entry(app, repo_id, &e);
                        show_addon_local_changes_dialog(app, vec![entry]);
                        return Some(Task::none());
                    } else if e.contains("FILE_CONFLICT:") {
                        let action = if replace_local_changes {
                            FileConflictAction::UpdateApprovedLocalChanges
                        } else {
                            FileConflictAction::Update
                        };
                        show_file_conflict(app, repo_id, action, &e);
                        return Some(Task::none());
                    } else {
                        app.log(LogLevel::Error, &format!("Update failed: {}", e));
                        if !app.show_github_rate_limit("The update could not be downloaded.", &e) {
                            app.show_toast(format!("Update failed: {}", e), ToastKind::Error);
                        }
                    }
                }
            }
            Some(refresh_repos_task(app))
        }
        Message::ConfirmAddonLocalChangesUpdate(repo_ids) => {
            app.dialog = None;
            if app.wow_dir.is_empty() || repo_ids.is_empty() {
                return Some(Task::none());
            }
            let mut opts = app.install_options();
            opts.replace_local_changes = true;
            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            for repo_id in &repo_ids {
                app.updating_repo_ids.insert(*repo_id);
                app.log(
                    LogLevel::Info,
                    &format!("User approved replacing local addon changes for repo id={repo_id}."),
                );
            }
            let scope = app.profile_operation_scope();
            if let [repo_id] = repo_ids.as_slice() {
                let repo_id = *repo_id;
                Some(Task::perform(
                    service::update_repo(db, repo_id, wow, opts),
                    move |result| Message::UpdateRepoResult {
                        repo_id,
                        replace_local_changes: true,
                        result: crate::ProfileScoped::new(scope.clone(), result),
                    },
                ))
            } else {
                let completed_ids = repo_ids.clone();
                Some(Task::perform(
                    service::update_all(db, wow, repo_ids, opts),
                    move |result| Message::UpdateAllResult {
                        repo_ids: completed_ids.clone(),
                        result: crate::ProfileScoped::new(scope.clone(), result),
                    },
                ))
            }
        }
        Message::IgnoreAddonLocalChangesUpdates(repo_ids) => {
            app.dialog = None;
            let ignored_count = repo_ids.len();
            for repo_id in repo_ids {
                app.ignored_update_ids.insert(repo_id);
                app.log(
                    LogLevel::Info,
                    &format!(
                        "Repo id={repo_id}: updates ignored after local changes were detected."
                    ),
                );
            }
            app.save_settings();
            app.show_toast(
                format!(
                    "Ignored updates for {ignored_count} addon{}.",
                    if ignored_count == 1 { "" } else { "s" }
                ),
                ToastKind::Info,
            );
            Some(Task::none())
        }
        Message::ToggleRepoEnabled(id, enabled) => {
            let repo_name = app
                .repos
                .iter()
                .find(|repo| repo.id == id)
                .map(|repo| repo.name.clone())
                .unwrap_or_else(|| format!("repository #{id}"));
            app.log(
                LogLevel::Info,
                &format!(
                    "Mod state change requested: \"{repo_name}\" (repo id={id}) -> {}.",
                    if enabled { "enabled" } else { "disabled" }
                ),
            );
            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            let use_dlls_txt = app.quick_add_client_family() == service::ClientFamily::Vanilla;
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::set_repo_enabled(db, id, enabled, wow, use_dlls_txt),
                move |result| Message::ToggleRepoEnabledResult {
                    repo_id: id,
                    enabled,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::ToggleRepoEnabledResult {
            repo_id,
            enabled,
            result,
        } => {
            let Some(result) = app.accept_profile_result(result, "repository enable-state update")
            else {
                return Some(Task::none());
            };
            let repo_name = app
                .repos
                .iter()
                .find(|repo| repo.id == repo_id)
                .map(|repo| repo.name.clone())
                .unwrap_or_else(|| format!("repository #{repo_id}"));
            match result {
                Ok(changed_files) => {
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "Mod {}: \"{repo_name}\" (repo id={repo_id}; affected file/entry count={changed_files}; metadata committed).",
                            if enabled { "enabled" } else { "disabled" }
                        ),
                    );
                    return Some(refresh_repos_task(app));
                }
                Err(e) => app.log(
                    LogLevel::Error,
                    &format!(
                        "Mod state change failed: \"{repo_name}\" (repo id={repo_id}; requested_state={}): {e}",
                        if enabled { "enabled" } else { "disabled" }
                    ),
                ),
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
        Message::ToggleDllEnabled(repo_id, dll_name, enabled) => {
            let repo_name = app
                .repos
                .iter()
                .find(|repo| repo.id == repo_id)
                .map(|repo| repo.name.clone())
                .unwrap_or_else(|| format!("repository #{repo_id}"));
            app.log(
                LogLevel::Info,
                &format!(
                    "DLL state change requested for mod \"{repo_name}\" (repo id={repo_id}) -> {}; component filename omitted from diagnostics.",
                    if enabled { "enabled" } else { "disabled" }
                ),
            );
            let db = app.db_path.clone();
            let wow = app.wow_dir.clone();
            let use_dlls_txt = app.quick_add_client_family() == service::ClientFamily::Vanilla;
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::set_dll_enabled(db, wow, repo_id, dll_name.clone(), enabled, use_dlls_txt),
                move |result| Message::ToggleDllEnabledResult {
                    repo_id,
                    dll_name: dll_name.clone(),
                    enabled,
                    result: crate::ProfileScoped::new(scope.clone(), result),
                },
            ))
        }
        Message::ToggleDllEnabledResult {
            repo_id,
            dll_name: _dll_name,
            enabled,
            result,
        } => {
            let Some(result) = app.accept_profile_result(result, "DLL enable-state update") else {
                return Some(Task::none());
            };
            let repo_name = app
                .repos
                .iter()
                .find(|repo| repo.id == repo_id)
                .map(|repo| repo.name.clone())
                .unwrap_or_else(|| format!("repository #{repo_id}"));
            match result {
                Ok(changed) => {
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "DLL component {} for mod \"{repo_name}\" (repo id={repo_id}; filesystem_or_dlls_txt_changed={changed}; metadata committed).",
                            if enabled { "enabled" } else { "disabled" }
                        ),
                    );
                    return Some(refresh_repos_task(app));
                }
                Err(e) => app.log(
                    LogLevel::Error,
                    &format!(
                        "DLL state change failed for mod \"{repo_name}\" (repo id={repo_id}; requested_state={}): {e}",
                        if enabled { "enabled" } else { "disabled" }
                    ),
                ),
            }
            Some(Task::none())
        }
        Message::UpdateAll => {
            if app.wow_dir.is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
            } else {
                let discarded_plans = reconcile_active_update_plans(app);
                if discarded_plans > 0 {
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "Discarded {discarded_plans} stale or duplicate update entr{} before Update All.",
                            if discarded_plans == 1 { "y" } else { "ies" }
                        ),
                    );
                }
                let db = app.db_path.clone();
                let wow = app.wow_dir.clone();
                let opts = app.install_options();
                let mut targets = Vec::new();
                let mut names = Vec::new();
                let mut seen_targets = HashSet::new();
                for plan in &app.plans {
                    let Some(repo) = app.repos.iter().find(|repo| repo.id == plan.repo_id) else {
                        continue;
                    };
                    if plan.has_update
                        && !service::is_curated_mpq_repo(repo)
                        && !app.ignored_update_ids.contains(&plan.repo_id)
                        && !app.updating_repo_ids.contains(&plan.repo_id)
                        && seen_targets.insert(plan.repo_id)
                    {
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
                    app.log(
                        LogLevel::Info,
                        &format!("Updating {} repo(s)...", targets.len()),
                    );
                    let scope = app.profile_operation_scope();
                    let completed_ids = targets.clone();
                    return Some(Task::perform(
                        service::update_all(db, wow, targets, opts),
                        move |result| Message::UpdateAllResult {
                            repo_ids: completed_ids.clone(),
                            result: crate::ProfileScoped::new(scope.clone(), result),
                        },
                    ));
                }
            }
            Some(Task::none())
        }
        Message::UpdateAllResult {
            repo_ids,
            result: scoped,
        } => {
            let Some(result) = app.accept_profile_result(scoped, "update-all operation") else {
                return Some(Task::none());
            };
            for repo_id in repo_ids {
                app.updating_repo_ids.remove(&repo_id);
            }
            match result {
                Ok(results) => {
                    let mut applied = 0;
                    let mut errors = 0;
                    let mut skipped = 0;
                    let mut rate_limit_error = None;
                    let mut local_changes = Vec::new();
                    for r in results {
                        let name = if r.owner.is_empty() {
                            r.name.clone()
                        } else {
                            format!("{}/{}", r.owner, r.name)
                        };
                        if r.skipped {
                            skipped += 1;
                            app.plans.retain(|plan| plan.repo_id != r.repo_id);
                            app.log(
                                LogLevel::Info,
                                &format!(
                                    "Skipped {name} because it is no longer tracked by this profile."
                                ),
                            );
                            continue;
                        }
                        if let Some(e) = r.error {
                            if e.starts_with("ADDON_GIT_LOCAL_CHANGES:") {
                                app.log(
                                    LogLevel::Info,
                                    &format!(
                                        "Skipped {name} pending approval to replace local changes."
                                    ),
                                );
                                local_changes.push(addon_local_changes_entry(app, r.repo_id, &e));
                                continue;
                            }
                            errors += 1;
                            if rate_limit_error.is_none()
                                && crate::github_api::rate_limit_notice(&e).is_some()
                            {
                                rate_limit_error = Some(e.clone());
                            }
                            app.log(
                                LogLevel::Error,
                                &format!("{} update failed: {}", name, simplify_git_error(&e)),
                            );
                        } else {
                            applied += 1;
                            app.log(LogLevel::Info, &format!("Updated {}.", name));
                            // Remove from plans so it disappears from UI immediately
                            app.plans.retain(|p| p.repo_id != r.repo_id);
                        }
                    }
                    sync_active_plan_cache(app);
                    if let Some(error) = rate_limit_error.as_deref() {
                        app.show_github_rate_limit("Some updates could not be downloaded.", error);
                    } else if errors > 0 {
                        app.show_toast(
                            format!("Update all partial: {} OK, {} failed.", applied, errors),
                            ToastKind::Warn,
                        );
                    } else if applied > 0 {
                        app.log(
                            LogLevel::Info,
                            &format!("Done. Updated {} repo(s).", applied),
                        );
                        app.show_toast(format!("Updated {} repo(s).", applied), ToastKind::Info);
                    } else if skipped > 0 {
                        app.show_toast(
                            "Removed stale update entries. Check again for current updates.",
                            ToastKind::Info,
                        );
                    }
                    show_addon_local_changes_dialog(app, local_changes);
                    return Some(refresh_repos_task(app));
                }
                Err(e) => {
                    app.log(LogLevel::Error, &format!("Update all failed: {}", e));
                    if !app.show_github_rate_limit("Updates could not be downloaded.", &e) {
                        app.show_toast(format!("Update all failed: {}", e), ToastKind::Error);
                    }
                }
            }
            Some(Task::none())
        }
        Message::ReinstallRepo(id) => {
            app.open_menu = None;
            if app.updating_repo_ids.contains(&id) {
                app.show_toast(
                    "That repository already has an operation in progress.",
                    ToastKind::Warn,
                );
                return Some(Task::none());
            }
            if app.wow_dir.is_empty() {
                app.log(LogLevel::Error, "Set a WoW directory in Options first.");
            } else {
                app.dialog = None;
                app.updating_repo_ids.insert(id);
                let repo = app.repos.iter().find(|repo| repo.id == id).cloned();
                if repo.as_ref().is_some_and(|repo| repo.mode == "addon_git") {
                    let repo = repo.expect("checked addon_git repo");
                    app.log(
                        LogLevel::Info,
                        &format!(
                            "Inspecting {}/{} before clean reinstall...",
                            repo.owner, repo.name
                        ),
                    );
                    let db = app.db_path.clone();
                    let wow = app.wow_dir.clone();
                    let scope = app.profile_operation_scope();
                    return Some(Task::perform(
                        service::probe_conflicts_on_branch(db, repo.url, wow, repo.git_branch),
                        move |result| Message::ReinstallRepoProbeResult {
                            repo_id: id,
                            result: crate::ProfileScoped::new(scope.clone(), result),
                        },
                    ));
                }

                app.log(LogLevel::Info, &format!("Reinstalling repo id={}...", id));
                let db = app.db_path.clone();
                let wow = app.wow_dir.clone();
                let opts = app.install_options();
                let scope = app.profile_operation_scope();
                return Some(Task::perform(
                    service::reinstall_repo(db, id, wow, opts),
                    move |result| Message::ReinstallRepoResult {
                        repo_id: id,
                        result: crate::ProfileScoped::new(scope.clone(), result),
                    },
                ));
            }
            Some(Task::none())
        }
        Message::ReinstallRepoProbeResult {
            repo_id,
            result: scoped,
        } => {
            let scope = scoped.scope.clone();
            let Some(result) = app.accept_profile_result(scoped, "reinstall inspection") else {
                return Some(Task::none());
            };
            let Some(repo) = app.repos.iter().find(|repo| repo.id == repo_id).cloned() else {
                app.updating_repo_ids.remove(&repo_id);
                app.log(
                    LogLevel::Error,
                    "Reinstall failed: repository is no longer tracked.",
                );
                return Some(Task::none());
            };

            match result {
                Ok(probe) => {
                    let root_options = service::root_probe_addon_names(&probe);
                    if root_options.len() > 1 {
                        app.updating_repo_ids.remove(&repo_id);
                        let suggested = service::suggested_addon_for_expansion(
                            &root_options,
                            app.expansion_hint(),
                        );
                        app.dialog = Some(Dialog::SelectMainAddon {
                            url: repo.url,
                            options: root_options,
                            suggested,
                            reinstall_repo_id: Some(repo_id),
                        });
                        return Some(Task::none());
                    }

                    app.log(
                        LogLevel::Info,
                        &format!("Clean-reinstalling {}/{}...", repo.owner, repo.name),
                    );
                    let db = app.db_path.clone();
                    let wow = app.wow_dir.clone();
                    let opts = app.install_options();
                    return Some(Task::perform(
                        service::reinstall_repo(db, repo_id, wow, opts),
                        move |result| Message::ReinstallRepoResult {
                            repo_id,
                            result: crate::ProfileScoped::new(scope.clone(), result),
                        },
                    ));
                }
                Err(error) => {
                    app.updating_repo_ids.remove(&repo_id);
                    app.log(
                        LogLevel::Error,
                        &format!("Reinstall inspection failed: {}", error),
                    );
                    app.show_toast(
                        "Could not inspect the addon; no files were changed.".to_string(),
                        ToastKind::Error,
                    );
                }
            }
            Some(Task::none())
        }
        Message::ReinstallRepoResult {
            repo_id,
            result: scoped,
        } => {
            let Some(result) = app.accept_profile_result(scoped, "repository reinstall") else {
                return Some(Task::none());
            };
            app.updating_repo_ids.remove(&repo_id);
            match result {
                Ok(plan) => {
                    // A clean reinstall establishes a new authoritative local
                    // baseline, so discard any Modified state from the last
                    // explicit Rescan before reloading repository rows.
                    forget_repo_update_plan(app, repo_id);
                    app.log(
                        LogLevel::Info,
                        &format!("Reinstalled {}/{}.", plan.owner, plan.name),
                    );
                    return Some(refresh_repos_task(app));
                }
                Err(e) if e.contains("FILE_CONFLICT:") => {
                    show_file_conflict(app, repo_id, FileConflictAction::Reinstall, &e);
                    return Some(Task::none());
                }
                Err(e) => app.log(LogLevel::Error, &format!("Reinstall failed: {}", e)),
            }
            Some(Task::none())
        }
        Message::FetchBranches(repo_id) => {
            let db = app.db_path.clone();
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::list_repo_branches(db, repo_id),
                move |result| {
                    Message::FetchBranchesResult(crate::ProfileScoped::new(scope.clone(), result))
                },
            ))
        }
        Message::FetchBranchesResult(scoped) => {
            let Some((repo_id, result)) =
                app.accept_profile_result(scoped, "repository-branch load")
            else {
                return Some(Task::none());
            };
            match result {
                Ok(branch_list) => {
                    app.branches.insert(repo_id, branch_list);
                }
                Err(e) => {
                    let repo_name = app
                        .repos
                        .iter()
                        .find(|r| r.id == repo_id)
                        .map(|r| format!("{}/{}", r.owner, r.name))
                        .unwrap_or_else(|| format!("repo#{}", repo_id));
                    if !is_silenced_git_error(&e) {
                        app.log(
                            LogLevel::Error,
                            &format!(
                                "Failed to fetch branches for {}: {}",
                                repo_name,
                                simplify_git_error(&e)
                            ),
                        );
                    }
                }
            }
            Some(Task::none())
        }
        Message::SetRepoBranch(repo_id, branch) => {
            let db = app.db_path.clone();
            app.log(
                LogLevel::Info,
                &format!("Setting branch to '{}' for repo id={}...", branch, repo_id),
            );
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::set_repo_branch(db, repo_id, branch),
                move |result| {
                    Message::SetRepoBranchResult(crate::ProfileScoped::new(scope.clone(), result))
                },
            ))
        }
        Message::SetRepoBranchResult(scoped) => {
            let Some(result) = app.accept_profile_result(scoped, "repository-branch update") else {
                return Some(Task::none());
            };
            match result {
                Ok(repo_id) => {
                    app.log(LogLevel::Info, "Branch updated. Refreshing repos...");
                    app.branches.remove(&repo_id);
                    return Some(refresh_repos_task(app));
                }
                Err(e) => app.log(
                    LogLevel::Error,
                    &format!("Set branch failed: {}", simplify_git_error(&e)),
                ),
            }
            Some(Task::none())
        }
        Message::UpdateCheckRateLimitResult(scoped) => {
            let Some((stats, info)) = app.accept_profile_result(scoped, "update-check summary")
            else {
                return Some(Task::none());
            };
            app.github_rate_info = info;

            let updates = if stats.updates_found == 1 {
                "update"
            } else {
                "updates"
            };
            let mut parts = vec![format!("{} {}", stats.updates_found, updates)];

            if stats.api_hits > 0 {
                parts.push(format!(
                    "spent {} API point{}",
                    stats.api_hits,
                    if stats.api_hits == 1 { "" } else { "s" }
                ));
            }
            if stats.api_cached > 0 {
                parts.push(format!("{} cached (free)", stats.api_cached));
            }
            if stats.git_syncs > 0 {
                parts.push(format!("{} synced (git)", stats.git_syncs));
            }
            if stats.other_hits > 0 {
                parts.push(format!(
                    "{} other check{}",
                    stats.other_hits,
                    if stats.other_hits == 1 { "" } else { "s" }
                ));
            }

            let summary = parts.join(", ");
            let rate_suffix = if let Some(r) = &app.github_rate_info {
                let mins = (r.reset_epoch - now_unix()) / 60;
                format!(
                    ". ({}/{} remaining, resets in {} min)",
                    r.remaining, r.limit, mins
                )
            } else {
                "".to_string()
            };

            app.log(
                LogLevel::Api,
                &format!("Check complete: {}{}", summary, rate_suffix),
            );
            Some(Task::none())
        }

        Message::GithubRateInfoResult(info) => {
            app.github_rate_info = info;
            Some(Task::none())
        }
        Message::ToggleRemoveFiles(val) => {
            if let Some(Dialog::RemoveRepo {
                ref mut remove_files,
                ..
            }) = app.dialog
            {
                *remove_files = val;
            }
            Some(Task::none())
        }
        Message::RemoveRepoFilesLoaded(scoped) => {
            let Some(result) = app.accept_profile_result(scoped, "repository removal preview")
            else {
                return Some(Task::none());
            };
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
                    Err(e) => app.log(
                        LogLevel::Error,
                        &format!("Failed to list files for removal: {}", e),
                    ),
                }
            }
            Some(Task::none())
        }
        Message::FetchRepoPreview(url) => {
            let generation = app.begin_preview_request();
            app.add_repo_preview_loading = true;
            let preview_url = url.clone();
            Some(Task::perform(
                service::fetch_repo_preview(url),
                move |result| {
                    Message::FetchRepoPreviewResult(generation, preview_url.clone(), result)
                },
            ))
        }
        Message::OpenRepoReadmePreview(title, url) => {
            let generation = app.begin_preview_request();
            app.markdown_image_cache.clear();
            app.markdown_gif_cache.clear();
            app.dialog = Some(Dialog::Changelog {
                title: format!("{title} — README"),
                items: Vec::new(),
                loading: true,
            });
            Some(Task::perform(
                service::fetch_repo_preview(url),
                move |result| Message::RepoReadmePreviewLoaded(generation, result),
            ))
        }
        Message::RepoReadmePreviewLoaded(generation, result) => {
            if !app.preview_request_is_current(generation, "README preview") {
                return Some(Task::none());
            }
            let loaded_items = match result {
                Ok(preview) => {
                    app.markdown_image_cache = preview.image_cache;
                    app.markdown_gif_cache = preview.gif_cache;
                    preview.readme_items
                }
                Err(error) => {
                    app.show_github_rate_limit("The README preview could not be loaded.", &error);
                    iced::widget::markdown::Content::parse(&format!(
                        "Could not load the README preview.\n\n{}",
                        crate::github_api::user_facing_error(&error)
                    ))
                    .items()
                    .to_vec()
                }
            };
            if let Some(Dialog::Changelog { items, loading, .. }) = app.dialog.as_mut() {
                *items = loaded_items;
                *loading = false;
            }
            Some(Task::none())
        }
        Message::FetchRepoPreviewResult(generation, url, result) => {
            if !app.preview_request_is_current(generation, "repository preview") {
                return Some(Task::none());
            }
            app.add_repo_preview_loading = false;
            if let Some(Dialog::AddRepo {
                url: current_url, ..
            }) = app.dialog.as_ref()
            {
                if service::normalize_repo_input_url(current_url)
                    != service::normalize_repo_input_url(&url)
                {
                    return Some(Task::none());
                }
            }
            match result {
                Ok(info) => {
                    app.readme_editor_content =
                        iced::widget::text_editor::Content::with_text(&info.readme_text);
                    app.readme_source_view = false;
                    app.add_repo_release_notes = None;
                    app.add_repo_show_releases = false;
                    app.add_repo_file_preview = None;
                    app.add_repo_expanded_dirs.clear();
                    app.add_repo_dir_contents.clear();

                    // In manage/collection mode, pre-fetch contents of all top-level dirs
                    let is_collection = app.add_repo_manage_repo_id.is_some()
                        || app.add_repo_collection_choice == Some(true)
                        || !app.add_repo_selected_addons.is_empty()
                        || !app.add_repo_existing_addons.is_empty();
                    let prefetch_tasks: Vec<iced::Task<Message>> = if is_collection {
                        info.files
                            .iter()
                            .filter(|f| f.is_dir)
                            .map(|f| {
                                let forge_url = info.forge_url.clone();
                                let path = f.path.clone();
                                iced::Task::perform(
                                    service::fetch_dir_contents(forge_url, path),
                                    move |result| {
                                        Message::FetchDirContentsResult(generation, result)
                                    },
                                )
                            })
                            .collect()
                    } else {
                        vec![]
                    };

                    app.add_repo_preview = Some(info.clone());

                    // Update AddonConflict dialog if visible for this repo
                    if let Some(Dialog::AddonConflict {
                        url: ref d_url,
                        ref mut new_repo_preview,
                        ..
                    }) = app.dialog
                    {
                        if service::normalize_repo_input_url(d_url)
                            == service::normalize_repo_input_url(&url)
                        {
                            *new_repo_preview = Some(info.files.clone());
                        }
                    }

                    if prefetch_tasks.is_empty() {
                        return Some(delayed_add_repo_url_refocus_task());
                    }
                    let mut tasks = Vec::with_capacity(prefetch_tasks.len() + 1);
                    tasks.push(delayed_add_repo_url_refocus_task());
                    tasks.extend(prefetch_tasks);
                    return Some(Task::batch(tasks));
                }
                Err(error) => {
                    app.add_repo_preview = None;
                    app.show_github_rate_limit(
                        "The repository preview could not be loaded.",
                        &error,
                    );
                    if matches!(app.dialog, Some(Dialog::AddRepo { .. })) {
                        return Some(delayed_add_repo_url_refocus_task());
                    }
                }
            }
            Some(Task::none())
        }
        Message::FetchReleaseAssetOptions(url) => {
            let db = app.db_path.clone();
            let options_url = url.clone();
            Some(Task::perform(
                service::fetch_latest_release_archive_options(db, url),
                move |result| Message::FetchReleaseAssetOptionsResult(options_url, result),
            ))
        }
        Message::FetchReleaseAssetOptionsResult(url, result) => {
            let Some(Dialog::AddRepo {
                url: current_url, ..
            }) = app.dialog.as_ref()
            else {
                return Some(Task::none());
            };
            if service::normalize_repo_input_url(current_url)
                != service::normalize_repo_input_url(&url)
            {
                return Some(Task::none());
            }

            match result {
                Ok(options) => {
                    app.add_repo_release_asset_options = options.clone();
                    if options.len() == 1 {
                        app.add_repo_selected_release_asset =
                            options.first().map(|asset| asset.name.clone());
                    } else if options.len() > 1 && app.add_repo_selected_release_asset.is_none() {
                        app.dialog = Some(Dialog::SelectReleaseAsset {
                            url,
                            options: options.into_iter().map(|asset| asset.name).collect(),
                        });
                    }
                }
                Err(e) => {
                    app.add_repo_release_asset_options.clear();
                    app.add_repo_selected_release_asset = None;
                    app.log(
                        LogLevel::Error,
                        &format!("Failed to fetch release assets: {}", e),
                    );
                    app.show_github_rate_limit("Release assets could not be loaded.", &e);
                }
            }
            Some(Task::none())
        }
        Message::SetAddRepoReleaseAsset(name) => {
            if !name.is_empty() {
                app.add_repo_selected_release_asset = Some(name);
            }
            if let Some(Dialog::SelectReleaseAsset { url, .. }) = app.dialog.as_ref() {
                let url = url.clone();
                app.dialog = Some(Dialog::AddRepo {
                    url,
                    mode: "addon".to_string(),
                    is_addons: true,
                    advanced: false,
                });
                return Some(delayed_add_repo_url_refocus_task());
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
                        let generation = app.preview_request_generation;
                        return Some(Task::perform(
                            service::fetch_dir_contents(forge_url, path),
                            move |result| Message::FetchDirContentsResult(generation, result),
                        ));
                    }
                }
            }
            Some(Task::none())
        }
        Message::FetchDirContents(forge_url, path) => {
            let generation = app.preview_request_generation;
            Some(Task::perform(
                service::fetch_dir_contents(forge_url, path),
                move |result| Message::FetchDirContentsResult(generation, result),
            ))
        }
        Message::FetchDirContentsResult(generation, result) => {
            if !app.preview_request_is_current(generation, "repository directory preview") {
                return Some(Task::none());
            }
            match result {
                Ok((dir_path, entries)) => {
                    let mut sorted = entries;
                    sorted.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
                    app.add_repo_dir_contents.insert(dir_path, sorted);
                }
                Err(error) => {
                    app.show_github_rate_limit("Repository files could not be loaded.", &error);
                }
            }
            if matches!(app.dialog, Some(Dialog::AddRepo { .. })) {
                return Some(delayed_add_repo_url_refocus_task());
            }
            Some(Task::none())
        }
        Message::FetchReleaseNotes => {
            if app.add_repo_release_notes.is_some() {
                app.add_repo_show_releases = true;
            } else if let Some(ref preview) = app.add_repo_preview {
                let url = preview.forge_url.clone();
                app.add_repo_show_releases = true;
                let generation = app.preview_request_generation;
                return Some(Task::perform(service::fetch_releases(url), move |result| {
                    Message::FetchReleaseNotesResult(generation, result)
                }));
            }
            Some(Task::none())
        }
        Message::FetchReleaseNotesResult(generation, result) => {
            if !app.preview_request_is_current(generation, "release notes") {
                return Some(Task::none());
            }
            match result {
                Ok(releases) => {
                    app.add_repo_release_notes = Some(releases.clone());
                    // Also update dialog if it's the changelog
                    if let Some(Dialog::Changelog {
                        ref mut items,
                        ref mut loading,
                        ref mut title,
                    }) = app.dialog
                    {
                        *loading = false;
                        *title = "Changelog".to_string();
                        // Transform ReleaseItem into Markdown Item
                        let mut markdown_text = String::new();
                        for rel in releases {
                            markdown_text.push_str(&format!("# {}\n\n", rel.name));
                            markdown_text.push_str(&rel.body);
                            markdown_text.push_str("\n\n---\n\n");
                        }
                        *items = iced::widget::markdown::Content::parse(&markdown_text)
                            .items()
                            .to_vec();
                    }
                }
                Err(e) => {
                    app.add_repo_show_releases = false;
                    app.log(LogLevel::Error, &format!("Failed to fetch releases: {}", e));
                    app.show_github_rate_limit("Release notes could not be loaded.", &e);
                    if let Some(Dialog::Changelog {
                        ref mut loading, ..
                    }) = app.dialog
                    {
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
                let generation = app.preview_request_generation;
                return Some(Task::perform(
                    service::fetch_raw_file(raw_base, path),
                    move |result| Message::PreviewRepoFileResult(generation, result),
                ));
            }
            Some(Task::none())
        }
        Message::PreviewRepoFileResult(generation, result) => {
            if !app.preview_request_is_current(generation, "repository file preview") {
                return Some(Task::none());
            }
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
            crate::diagnostics::register_repository_url(&url);
            app.log(LogLevel::Info, "Adding repository...");
            let scope = app.profile_operation_scope();
            Some(Task::perform(
                service::add_repo(db, url, mode, None, None),
                move |result| {
                    Message::AddRepoResult(crate::ProfileScoped::new(scope.clone(), result))
                },
            ))
        }
        Message::SetAddRepoUrl(url) => {
            if let Some(Dialog::AddRepo {
                url: ref mut old_url,
                ..
            }) = app.dialog
            {
                *old_url = url.clone();
            }
            app.add_repo_url_debounce_generation =
                app.add_repo_url_debounce_generation.wrapping_add(1);
            let debounce_generation = app.add_repo_url_debounce_generation;
            let mut tasks = vec![iced::widget::operation::focus(iced::widget::Id::new(
                "add_repo_url",
            ))];
            app.add_repo_preview = None;
            app.add_repo_preview_loading = false;
            app.add_repo_release_notes = None;
            app.add_repo_show_releases = false;
            app.add_repo_file_preview = None;
            app.add_repo_expanded_dirs.clear();
            app.add_repo_dir_contents.clear();
            app.add_repo_release_asset_options.clear();
            app.add_repo_selected_release_asset = None;
            app.add_repo_probe = None;
            app.add_repo_probe_loading = false;
            app.add_repo_primary_toc_confirmed = false;
            if app.add_repo_manage_repo_id.is_none() {
                app.add_repo_collection_choice = None;
            }
            if app.add_repo_manage_repo_id.is_none() {
                app.add_repo_selected_addons.clear();
            }
            let trimmed = url.trim().to_string();
            if service::parse_forge_url(&trimmed).is_some()
                && !service::is_direct_archive_candidate(&trimmed)
            {
                tasks.push(Task::perform(
                    async move {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        Message::DebouncedResolveAddRepoUrl {
                            generation: debounce_generation,
                            url: trimmed,
                        }
                    },
                    |msg| msg,
                ));
            }
            Some(Task::batch(tasks))
        }
        Message::DebouncedResolveAddRepoUrl { generation, url } => {
            if generation != app.add_repo_url_debounce_generation {
                return Some(Task::none());
            }
            if let Some(Dialog::AddRepo {
                url: current_url, ..
            }) = app.dialog.as_ref()
            {
                if service::normalize_repo_input_url(current_url)
                    != service::normalize_repo_input_url(&url)
                {
                    return Some(Task::none());
                }
                if app.add_repo_preview.as_ref().is_some_and(|preview| {
                    service::normalize_repo_input_url(&preview.forge_url)
                        == service::normalize_repo_input_url(&url)
                }) {
                    return Some(Task::none());
                }
                return Some(Task::done(Message::ResolveAddRepoUrl));
            }
            Some(Task::none())
        }
        Message::RefocusAddRepoUrl => {
            if matches!(app.dialog, Some(Dialog::AddRepo { .. })) {
                return Some(iced::widget::operation::focus(iced::widget::Id::new(
                    "add_repo_url",
                )));
            }
            Some(Task::none())
        }
        Message::ResolveAddRepoUrl => {
            app.add_repo_url_debounce_generation =
                app.add_repo_url_debounce_generation.wrapping_add(1);
            let (url, is_addons, mode) = if let Some(Dialog::AddRepo {
                ref url,
                ref mode,
                is_addons,
                ..
            }) = app.dialog
            {
                (url.trim().to_string(), is_addons, mode.clone())
            } else {
                (String::new(), false, String::new())
            };
            if !url.is_empty() {
                if service::is_direct_archive_candidate(&url) {
                    if let Some(Dialog::AddRepo { ref mut mode, .. }) = app.dialog {
                        *mode = "addon".to_string();
                    }
                    return Some(delayed_add_repo_url_refocus_task());
                }

                let is_release_addon_url = is_addons && service::is_release_url(&url);
                if is_release_addon_url {
                    if let Some(Dialog::AddRepo { ref mut mode, .. }) = app.dialog {
                        if mode == "addon_git" {
                            *mode = "addon".to_string();
                        }
                    }
                }

                let mut tasks = vec![Task::done(Message::FetchRepoPreview(url.clone()))];
                if is_release_addon_url || (is_addons && mode != "addon_git") {
                    tasks.push(Task::done(Message::FetchReleaseAssetOptions(url.clone())));
                }
                // Always probe git addon structure — wow_dir is not required for folder detection.
                if is_addons && !is_release_addon_url && mode == "addon_git" {
                    tasks.push(Task::done(Message::FetchCollectionProbe(url)));
                }
                return Some(Task::batch(tasks));
            }
            Some(Task::none())
        }
        Message::OpenModFileInfo(name) => {
            let generation = app.begin_preview_request();
            // Priority: if it's a WeirdUtils DLL, try to fetch live info from the README first.
            if WEIRD_UTILS_DLLS
                .iter()
                .any(|&d| d.eq_ignore_ascii_case(&name))
            {
                app.dialog = Some(Dialog::Changelog {
                    title: name.clone(),
                    items: Vec::new(),
                    loading: true,
                });
                return Some(Task::perform(
                    service::fetch_dll_description(name),
                    move |result| Message::FetchDllDescriptionResult(generation, result),
                ));
            }

            // Check if we have a hardcoded description for this DLL (non-WeirdUtils fallback or legacy)
            if let Some((dll, desc)) = WEIRD_UTILS_DESCRIPTIONS
                .iter()
                .find(|(dll, _)| dll.eq_ignore_ascii_case(&name))
            {
                let items = iced::widget::markdown::Content::parse(desc)
                    .items()
                    .to_vec();
                app.dialog = Some(Dialog::Changelog {
                    title: dll.to_string(),
                    items,
                    loading: false,
                });
                return Some(Task::none());
            }

            // Fallback: search for a repo with this name AND a forge_url (likely release notes)
            app.dialog = Some(Dialog::Changelog {
                title: name.clone(),
                items: Vec::new(),
                loading: true,
            });
            let url = app
                .repos
                .iter()
                .find(|r| r.name.eq_ignore_ascii_case(&name) && !r.url.is_empty())
                .map(|r| r.url.clone());

            if let Some(url) = url {
                Some(Task::perform(service::fetch_releases(url), move |result| {
                    Message::FetchReleaseNotesResult(generation, result)
                }))
            } else {
                // If no repo found, just show "No info available"
                if let Some(Dialog::Changelog {
                    ref mut items,
                    ref mut loading,
                    ..
                }) = app.dialog
                {
                    *loading = false;
                    *items = iced::widget::markdown::Content::parse(
                        "No additional information available for this mod.",
                    )
                    .items()
                    .to_vec();
                }
                Some(Task::none())
            }
        }

        Message::FetchDllDescriptionResult(generation, result) => {
            if !app.preview_request_is_current(generation, "DLL information") {
                return Some(Task::none());
            }
            match result {
                Ok((name, desc)) => {
                    if let Some(Dialog::Changelog {
                        ref mut title,
                        ref mut items,
                        ref mut loading,
                        ..
                    }) = app.dialog
                    {
                        *title = name;
                        *items = iced::widget::markdown::Content::parse(&desc)
                            .items()
                            .to_vec();
                        *loading = false;
                    }
                }
                Err(_e) => {
                    // Fallback to hardcoded description if fetch fails
                    let mut found_fallback = false;
                    if let Some(Dialog::Changelog {
                        ref mut title,
                        ref mut items,
                        ref mut loading,
                        ..
                    }) = app.dialog
                    {
                        if let Some((_dll, desc)) = WEIRD_UTILS_DESCRIPTIONS
                            .iter()
                            .find(|(dll, _)| dll.eq_ignore_ascii_case(title))
                        {
                            *items = iced::widget::markdown::Content::parse(desc)
                                .items()
                                .to_vec();
                            *loading = false;
                            found_fallback = true;
                        }
                    }

                    if !found_fallback {
                        if let Some(Dialog::Changelog {
                            ref mut items,
                            ref mut loading,
                            ..
                        }) = app.dialog
                        {
                            *loading = false;
                            *items = iced::widget::markdown::Content::parse("Could not fetch live information, and no offline description is available.").items().to_vec();
                        }
                    }
                }
            }
            Some(Task::none())
        }
        _ => None,
    }
}

pub fn refresh_repos_task(app: &App) -> Task<Message> {
    refresh_repos_task_inner(app, false)
}

fn delayed_add_repo_url_refocus_task() -> Task<Message> {
    Task::perform(
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
        },
        |_| Message::RefocusAddRepoUrl,
    )
}

pub fn refresh_repos_task_inner(app: &App, fix_casing: bool) -> Task<Message> {
    let db = app.db_path.clone();
    let scope = app.profile_operation_scope();
    let wow = if app.wow_dir.is_empty() {
        None
    } else {
        Some(app.wow_dir.clone())
    };
    Task::perform(service::list_repos(db, wow, fix_casing), move |result| {
        Message::ReposLoaded(crate::ProfileScoped::new(scope.clone(), result))
    })
}

pub fn check_updates_task(app: &mut App) -> Task<Message> {
    let db = app.db_path.clone();
    let scope = app.profile_operation_scope();
    let wow = if app.wow_dir.is_empty() {
        None
    } else {
        Some(app.wow_dir.clone())
    };
    let mut skip = app.ignored_update_ids.clone();
    let ignored_count = skip.len();
    let infrequent_count = if app.opt_auto_check
        && app.opt_conserve_github_api
        && wuddle_engine::github_token().is_none()
    {
        let s = infrequent_skip_ids(&app.repos, &app.plans, app.last_infrequent_check_unix);
        let mut added = 0;
        for repo_id in s {
            if skip.insert(repo_id) {
                added += 1;
            }
        }
        added
    } else {
        0
    };
    let auth = if wuddle_engine::github_token().is_some() {
        "authenticated"
    } else {
        "unauthenticated"
    };
    app.log(
        LogLevel::Api,
        &format!(
            "Checking active repositories ({auth}; {ignored_count} ignored and {infrequent_count} infrequent skipped)..."
        ),
    );

    Task::perform(
        service::check_updates_skip(db, wow, wuddle_engine::CheckMode::Force, skip),
        move |result| Message::CheckUpdatesResult(crate::ProfileScoped::new(scope.clone(), result)),
    )
}

/// Re-check all repositories after a user explicitly selects a release.
/// This intentionally bypasses the infrequent-repository skip list: the chosen
/// release determines whether that row needs an install right now.
fn check_updates_for_version_change_task(app: &App) -> Task<Message> {
    let db = app.db_path.clone();
    let scope = app.profile_operation_scope();
    let wow = if app.wow_dir.is_empty() {
        None
    } else {
        Some(app.wow_dir.clone())
    };
    Task::perform(
        service::check_updates_skip(
            db,
            wow,
            wuddle_engine::CheckMode::Force,
            app.ignored_update_ids.clone(),
        ),
        move |result| Message::CheckUpdatesResult(crate::ProfileScoped::new(scope.clone(), result)),
    )
}

pub const INFREQUENT_THRESHOLD_SECS: i64 = 3 * 24 * 3600;

pub fn recompute_infrequent_ids(app: &mut App) {
    let now = now_unix();
    let has_update: HashSet<i64> = app
        .plans
        .iter()
        .filter(|p| p.has_update)
        .map(|p| p.repo_id)
        .collect();
    app.infrequent_repo_ids = app
        .repos
        .iter()
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

pub fn infrequent_skip_ids(
    repos: &[service::RepoRow],
    plans: &[service::PlanRow],
    last_infrequent_check_unix: i64,
) -> HashSet<i64> {
    let now = now_unix();
    let recently_checked = (now - last_infrequent_check_unix) < INFREQUENT_CHECK_INTERVAL_SECS;

    if !recently_checked {
        return HashSet::new();
    }

    let has_update: HashSet<i64> = plans
        .iter()
        .filter(|p| p.has_update)
        .map(|p| p.repo_id)
        .collect();

    repos
        .iter()
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
    let error_code: Option<String> = raw.find("code=").and_then(|i| {
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
        inner = inner[start + 14..].trim_end_matches([')', ' ']);
    }

    // Strip "Git sync check failed: " prefix if still present.
    inner = inner
        .strip_prefix("Git sync check failed: ")
        .unwrap_or(inner);

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

#[cfg(test)]
mod file_conflict_tests {
    use super::parse_file_conflict_error;

    #[test]
    fn file_conflict_parser_keeps_file_and_owner_labels_without_confirmation_text() {
        let parsed = parse_file_conflict_error(
            "FILE_CONFLICT: Existing managed or local files were found for: d3d9.dll [owner/mod]; helper.dll [existing local file]. Confirm replacement to back them up and continue.",
        );
        assert_eq!(
            parsed,
            vec![
                "d3d9.dll [owner/mod]".to_string(),
                "helper.dll [existing local file]".to_string()
            ]
        );
    }
}

#[cfg(test)]
mod local_change_tests {
    use super::addon_local_change_reason;

    #[test]
    fn local_change_errors_become_safe_actionable_dialog_reasons() {
        assert_eq!(
            addon_local_change_reason(
                "ADDON_GIT_LOCAL_CHANGES: a moved addon folder differs from the checked-out Git revision"
            ),
            "An exposed addon folder contains added, changed, or missing files."
        );
        assert_eq!(
            addon_local_change_reason(
                "ADDON_GIT_LOCAL_CHANGES: the Git worktree contains unexpected changes"
            ),
            "The installed Git worktree contains added, changed, or deleted files."
        );
    }
}

#[cfg(test)]
mod update_plan_tests {
    use super::{
        apply_addon_git_rescan_state, merge_current_update_plans, retain_current_unique_plans,
    };
    use crate::service::{PlanRow, RepoRow};
    use wuddle_engine::{AddonGitLocalChange, AddonGitLocalChangeScan};

    fn repo(id: i64) -> RepoRow {
        RepoRow {
            id,
            forge: "github".to_string(),
            owner: "owner".to_string(),
            name: format!("repo-{id}"),
            url: format!("https://github.com/owner/repo-{id}"),
            mode: "addon_git".to_string(),
            enabled: true,
            last_version: None,
            git_branch: None,
            installed_branch: None,
            installed_dlls: Vec::new(),
            installed_addons: Vec::new(),
            installed_mpqs: Vec::new(),
            mpq_package_name: None,
            dependencies: Vec::new(),
            selected_addons: Vec::new(),
            is_collection: false,
            merge_installs: false,
            pinned_version: None,
            installed_at_unix: None,
            published_at_unix: None,
        }
    }

    fn plan(id: i64, latest: &str) -> PlanRow {
        PlanRow {
            repo_id: id,
            owner: "owner".to_string(),
            name: format!("repo-{id}"),
            current: Some("old".to_string()),
            latest: latest.to_string(),
            asset_name: String::new(),
            has_update: true,
            repair_needed: false,
            externally_modified: false,
            not_modified: false,
            mode: "addon_git".to_string(),
            host: "github.com".to_string(),
            error: None,
            previous_dll_count: 0,
            new_dll_count: 0,
        }
    }

    #[test]
    fn repository_reload_discards_stale_and_duplicate_update_plans() {
        let repos = vec![repo(1), repo(2)];
        let mut plans = vec![plan(1, "fresh"), plan(1, "duplicate"), plan(99, "stale")];

        let removed = retain_current_unique_plans(&mut plans, &repos);

        assert_eq!(removed, 2);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].repo_id, 1);
        assert_eq!(plans[0].latest, "fresh");
    }

    #[test]
    fn checked_plans_replace_cached_rows_and_keep_only_current_skipped_repos() {
        let repos = vec![repo(1), repo(2)];
        let checked = vec![plan(1, "fresh"), plan(1, "duplicate")];
        let previous = vec![
            plan(1, "cached"),
            plan(2, "intentionally-skipped"),
            plan(99, "removed"),
        ];

        let merged = merge_current_update_plans(checked, &previous, &repos);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].repo_id, 1);
        assert_eq!(merged[0].latest, "fresh");
        assert_eq!(merged[1].repo_id, 2);
        assert_eq!(merged[1].latest, "intentionally-skipped");
    }

    #[test]
    fn remote_checks_preserve_the_last_explicit_rescan_modification_state() {
        let repos = vec![repo(1)];
        let checked = vec![plan(1, "fresh")];
        let mut cached = plan(1, "cached");
        cached.externally_modified = true;

        let merged = merge_current_update_plans(checked, &[cached], &repos);

        assert!(merged[0].externally_modified);
    }

    #[test]
    fn rescan_marks_only_git_addons_and_a_later_clean_scan_clears_them() {
        let git_repo = repo(1);
        let mut manual_repo = repo(2);
        manual_repo.mode = "manual".to_string();
        let repos = vec![git_repo, manual_repo];
        let mut plans = Vec::new();
        let scan = AddonGitLocalChangeScan {
            inspected: 1,
            failed: 0,
            inspected_repo_ids: vec![1],
            failed_repo_ids: Vec::new(),
            modified: vec![
                AddonGitLocalChange {
                    repo_id: 1,
                    reason: "the Git worktree contains unexpected changes".to_string(),
                },
                // Defensive UI filtering: even a malformed scan result must
                // never classify a manual addon as modified.
                AddonGitLocalChange {
                    repo_id: 2,
                    reason: "the Git worktree contains unexpected changes".to_string(),
                },
            ],
        };

        let detected = apply_addon_git_rescan_state(&mut plans, &repos, &scan);
        assert_eq!(detected.len(), 1);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].repo_id, 1);
        assert!(plans[0].externally_modified);

        apply_addon_git_rescan_state(
            &mut plans,
            &repos,
            &AddonGitLocalChangeScan {
                inspected: 1,
                failed: 1,
                inspected_repo_ids: vec![1],
                failed_repo_ids: vec![1],
                modified: Vec::new(),
            },
        );
        assert!(plans[0].externally_modified);

        apply_addon_git_rescan_state(
            &mut plans,
            &repos,
            &AddonGitLocalChangeScan {
                inspected: 1,
                failed: 0,
                inspected_repo_ids: vec![1],
                failed_repo_ids: Vec::new(),
                modified: Vec::new(),
            },
        );
        assert!(!plans[0].externally_modified);
    }
}
