use iced::widget::{
    button, canvas, checkbox, column, container, row, rule, scrollable, stack, text, Space,
};
use iced::{Element, Font, Length, Subscription, Task, Theme};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use crate::components::helpers::*;
use crate::components::markdown::ImageViewer;
use crate::components::presets::build_quick_add_presets;
use crate::dialogs::simple_warnings::{addon_conflict, av_false_positive_warning};
use crate::message::Message;
use crate::service::{self, is_mod, PlanRow, RepoRow};
use crate::settings::{self, UpdateChannel};
use crate::theme::{self, ThemeColors, WuddleTheme, FRIZ, LIFECRAFT, NOTO};
use crate::types::*;
use crate::{chrono_now, chrono_now_fmt, monitor};
use crate::{panels, radio};

pub struct App {
    pub active_tab: Tab,
    pub wuddle_theme: WuddleTheme,

    // Projects filter & search
    pub filter: Filter,
    pub project_search: String,
    pub sort_key: SortKey,
    pub sort_dir: SortDir,

    // Options checkboxes
    pub opt_auto_check: bool,
    pub opt_desktop_notify: bool,
    pub opt_symlinks: bool,
    pub opt_xattr: bool,
    pub opt_clock12: bool,
    pub opt_friz_font: bool,
    // Radio
    pub radio_playing: bool,
    pub radio_volume: f32,
    pub radio_pre_mute_volume: Option<f32>,
    pub radio_connecting: bool,
    pub radio_auto_connect: bool,
    pub radio_auto_play: bool,
    pub radio_buffer_size: usize,
    pub radio_persist_volume: bool,
    pub radio_error: Option<String>,
    pub radio_handle: Option<radio::RadioHandle>,

    // GitHub auth
    pub github_token_input: String,

    // Tweaks
    pub tweaks: TweakState,

    // Logs
    pub log_lines: Vec<LogLine>,
    pub log_filter: LogFilter,
    pub log_search: String,
    pub log_wrap: bool,
    pub log_autoscroll: bool,
    // Error sub-filters (only active when log_filter == Errors)
    pub log_error_fetch: bool,
    pub log_error_misc: bool,

    // Dialog overlay
    pub dialog: Option<Dialog>,

    // Context menu: which repo's menu is open
    pub open_menu: Option<i64>,
    // "Add New" dropdown on the Home tab
    pub add_new_menu_open: bool,

    // Branch data (cached per repo_id)
    pub branches: HashMap<i64, Vec<String>>,

    // Engine data (Phase 2)
    pub repos: Vec<RepoRow>,
    pub plans: Vec<PlanRow>,
    pub loading: bool,
    pub error: Option<String>,
    pub wow_dir: String,
    pub active_profile_id: String,
    pub db_path: Option<PathBuf>,
    pub last_checked: Option<String>,

    // Plans cached per profile so switching profiles restores previous update state
    pub cached_plans: HashMap<String, (Vec<PlanRow>, Option<String>)>,

    // Operation state (Phase 3)
    pub checking_updates: bool,
    pub updating_all: bool,
    pub updating_repo_ids: HashSet<i64>,
    pub current_update_check_snapshot: Option<String>,
    pub current_update_check_started_at: Option<Instant>,
    pub last_update_check_warning_secs: Option<u64>,
    pub busy_started_at: Option<Instant>,
    pub busy_state_snapshot: Option<String>,

    // Tweak values
    pub tweak_values: TweakValues,

    // About / self-update
    pub latest_version: Option<String>,
    pub update_message: Option<String>,
    pub self_update_supported: bool,
    pub self_update_available: bool,
    pub self_update_assets_pending: bool,
    pub self_update_in_progress: bool,
    pub self_update_done: bool,

    // Auto-check
    pub autocheck_done: bool,
    pub auto_check_minutes: u32,
    /// Tracks when infrequent repos were last checked (wall-clock unix seconds).
    pub last_infrequent_check_unix: i64,
    /// Repos considered infrequently updated (last release > 3 days ago, no pending update).
    pub infrequent_repo_ids: std::collections::HashSet<i64>,

    // Profiles/instances
    pub profiles: Vec<settings::ProfileConfig>,

    // Spinner animation tick (0..36, one full rotation = 36 ticks @ 80ms each)
    pub spinner_tick: usize,

    // Add-repo dialog preview
    pub add_repo_preview: Option<service::RepoPreviewInfo>,
    pub add_repo_preview_loading: bool,
    pub add_repo_expanded_dirs: HashSet<String>,
    /// Lazily-loaded directory contents, keyed by dir path.
    pub add_repo_dir_contents: HashMap<String, Vec<service::RepoFileEntry>>,
    /// Currently previewed file (name, content). None = show README or release notes.
    pub add_repo_file_preview: Option<(String, String)>,

    // Add-repo release notes (fetched on demand)
    pub add_repo_release_notes: Option<Vec<service::ReleaseItem>>,
    pub add_repo_show_releases: bool,

    // Toast notifications
    pub toasts: Vec<Toast>,
    pub toast_counter: usize,

    // Repos whose updates are being ignored
    pub ignored_update_ids: HashSet<i64>,

    // GitHub API rate limit info (fetched after update checks)
    pub github_rate_info: Option<service::GitHubRateInfo>,

    // Fetched version lists for the version picker, keyed by repo_id
    pub repo_versions: HashMap<i64, Vec<service::VersionItem>>,
    pub repo_versions_loading: HashSet<i64>,

    // Multi-DLL repos that are currently expanded in the project list
    pub expanded_repo_ids: HashSet<i64>,

    // Selectable log view
    pub log_editor_content: iced::widget::text_editor::Content,

    // Selectable DXVK config preview
    pub dxvk_preview_content: iced::widget::text_editor::Content,

    // README source-view toggle (formatted markdown \u{2194} selectable raw text)
    pub readme_source_view: bool,
    pub readme_editor_content: iced::widget::text_editor::Content,

    // Release channel
    pub update_channel: UpdateChannel,
    pub ui_scale: f32,
    pub ui_scale_mode: settings::UiScaleMode,

    // Global markdown caches (for dialogs)
    pub markdown_image_cache: HashMap<String, iced::widget::image::Handle>,
    pub markdown_gif_cache: HashMap<String, std::sync::Arc<iced_gif::Frames>>,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let app = Self {
            active_tab: Tab::default(),
            wuddle_theme: WuddleTheme::default(),
            filter: Filter::default(),
            project_search: String::new(),
            sort_key: SortKey::default(),
            sort_dir: SortDir::default(),
            opt_auto_check: false,
            opt_desktop_notify: false,
            opt_symlinks: false,
            opt_xattr: true,
            opt_clock12: false,
            opt_friz_font: false,
            radio_playing: false,
            radio_volume: 0.25,
            radio_pre_mute_volume: None,
            radio_connecting: false,
            radio_auto_connect: false,
            radio_auto_play: false,
            radio_buffer_size: 4096,
            radio_persist_volume: true,
            radio_error: None,
            radio_handle: None,
            github_token_input: String::new(),
            tweaks: TweakState::default(),
            log_lines: {
                let mut lines = vec![LogLine {
                    level: LogLevel::Info,
                    text: concat!("Wuddle v", env!("CARGO_PKG_VERSION"), " started").into(),
                    timestamp: chrono_now(),
                }];
                let auto = *crate::AUTO_UI_SCALE.get().unwrap_or(&1.0);
                if auto != 1.0 {
                    let pct = (auto * 100.0) as u32;
                    if let Some((w, h)) = monitor::primary_monitor_size() {
                        lines.push(LogLine {
                            level: LogLevel::Info,
                            text: format!("Monitor {w}x{h} detected \u{2014} auto scale {pct}%")
                                .into(),
                            timestamp: chrono_now(),
                        });
                    }
                }
                lines.push(LogLine {
                    level: LogLevel::Info,
                    text: "Ready.".into(),
                    timestamp: chrono_now(),
                });
                lines
            },
            log_filter: LogFilter::default(),
            log_search: String::new(),
            log_wrap: false,
            log_autoscroll: true,
            log_error_fetch: true,
            log_error_misc: true,
            dialog: None,
            open_menu: None,
            add_new_menu_open: false,
            branches: HashMap::new(),
            repos: Vec::new(),
            plans: Vec::new(),
            cached_plans: HashMap::new(),
            loading: true,
            error: None,
            wow_dir: String::new(),
            active_profile_id: String::from("default"),
            db_path: None,
            last_checked: None,
            checking_updates: false,
            updating_all: false,
            updating_repo_ids: HashSet::new(),
            current_update_check_snapshot: None,
            current_update_check_started_at: None,
            last_update_check_warning_secs: None,
            busy_started_at: None,
            busy_state_snapshot: None,
            tweak_values: TweakValues::default(),
            latest_version: None,
            update_message: None,
            self_update_supported: false,
            self_update_available: false,
            self_update_assets_pending: false,
            self_update_in_progress: false,
            self_update_done: false,
            autocheck_done: false,
            auto_check_minutes: 60,
            last_infrequent_check_unix: 0,
            infrequent_repo_ids: std::collections::HashSet::new(),
            profiles: vec![settings::ProfileConfig::default()],
            spinner_tick: 0,
            add_repo_preview: None,
            add_repo_preview_loading: false,
            add_repo_expanded_dirs: HashSet::new(),
            add_repo_dir_contents: HashMap::new(),
            add_repo_file_preview: None,
            add_repo_release_notes: None,
            add_repo_show_releases: false,
            toasts: Vec::new(),
            toast_counter: 0,
            ignored_update_ids: HashSet::new(),
            github_rate_info: None,
            repo_versions: HashMap::new(),
            repo_versions_loading: HashSet::new(),
            expanded_repo_ids: HashSet::new(),
            log_editor_content: iced::widget::text_editor::Content::with_text(concat!(
                "[INFO] Wuddle v",
                env!("CARGO_PKG_VERSION"),
                " started\n[INFO] Ready."
            )),
            readme_source_view: false,
            readme_editor_content: iced::widget::text_editor::Content::new(),
            dxvk_preview_content: iced::widget::text_editor::Content::new(),
            update_channel: UpdateChannel::Beta,
            ui_scale: *crate::AUTO_UI_SCALE.get().unwrap_or(&1.0),
            ui_scale_mode: settings::UiScaleMode::Auto,
            markdown_image_cache: HashMap::new(),
            markdown_gif_cache: HashMap::new(),
        };

        // Sync GitHub token from keychain/env at startup
        service::sync_github_token();

        // Load settings synchronously (fast, local JSON), then kick off async repo load
        let settings_task =
            Task::perform(async { settings::load_settings() }, Message::SettingsLoaded);

        let task = settings_task;

        (app, task)
    }

    pub fn log(&mut self, level: LogLevel, msg: &str) {
        self.log_lines.push(LogLine {
            level,
            text: msg.to_string(),
            timestamp: chrono_now_fmt(self.opt_clock12),
        });
        self.rebuild_log_content();
    }

    pub fn show_toast(&mut self, message: impl Into<String>, kind: ToastKind) {
        self.push_toast(message, kind, None);
    }

    pub fn show_toast_with_action(
        &mut self,
        message: impl Into<String>,
        kind: ToastKind,
        on_click: Message,
    ) {
        self.push_toast(message, kind, Some(on_click));
    }

    pub fn push_toast(
        &mut self,
        message: impl Into<String>,
        kind: ToastKind,
        on_click: Option<Message>,
    ) {
        self.toast_counter += 1;
        // ~5 seconds at 80ms per tick = 63 ticks
        let ttl = match kind {
            ToastKind::Error => 100, // ~8 seconds
            _ => 63,
        };
        self.toasts.push(Toast {
            id: self.toast_counter,
            message: message.into(),
            kind,
            ttl,
            on_click,
        });
        // Keep max 5 toasts
        while self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
    }

    pub fn save_settings(&self) {
        let s = settings::AppSettings {
            wow_dir: self.wow_dir.clone(),
            theme: self.wuddle_theme.key().to_string(),
            active_profile_id: self.active_profile_id.clone(),
            opt_auto_check: self.opt_auto_check,
            opt_desktop_notify: self.opt_desktop_notify,
            opt_symlinks: self.opt_symlinks,
            opt_xattr: self.opt_xattr,
            radio_auto_connect: self.radio_auto_connect,
            radio_volume: self.radio_volume,
            radio_auto_play: self.radio_auto_play,
            radio_buffer_size: self.radio_buffer_size,
            radio_persist_volume: self.radio_persist_volume,
            opt_clock12: self.opt_clock12,
            opt_friz_font: self.opt_friz_font,
            log_wrap: self.log_wrap,
            log_autoscroll: self.log_autoscroll,
            auto_check_minutes: self.auto_check_minutes,
            profiles: self.profiles.clone(),
            ignored_update_ids: self.ignored_update_ids.iter().cloned().collect(),
            update_channel: self.update_channel,
            ui_scale_mode: self.ui_scale_mode,
        };
        let _ = settings::save_settings(&s);
    }

    pub fn theme(&self) -> Theme {
        self.wuddle_theme.to_iced_theme()
    }

    pub fn rebuild_log_content(&mut self) {
        let search = self.log_search.to_ascii_lowercase();
        let text: String = self
            .log_lines
            .iter()
            .filter(|line| match self.log_filter {
                LogFilter::All => true,
                LogFilter::Info => matches!(line.level, LogLevel::Info),
                LogFilter::Api => matches!(line.level, LogLevel::Api),
                LogFilter::Errors => {
                    if !matches!(line.level, LogLevel::Error) {
                        return false;
                    }
                    let fetch = panels::logs::is_fetch_error(&line.text);
                    (fetch && self.log_error_fetch) || (!fetch && self.log_error_misc)
                }
            })
            .filter(|line| search.is_empty() || line.text.to_ascii_lowercase().contains(&search))
            .map(|line| {
                let prefix = match line.level {
                    LogLevel::Info => "[INFO]",
                    LogLevel::Api => "[API]",
                    LogLevel::Error => "[ERROR]",
                };
                format!("[{}] {} {}", line.timestamp, prefix, line.text)
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.log_editor_content = iced::widget::text_editor::Content::with_text(&text);
        if self.log_autoscroll {
            self.log_editor_content
                .perform(iced::widget::text_editor::Action::Move(
                    iced::widget::text_editor::Motion::DocumentEnd,
                ));
        }
    }

    pub fn is_busy(&self) -> bool {
        self.loading
            || self.checking_updates
            || self.updating_all
            || !self.updating_repo_ids.is_empty()
            || self.add_repo_preview_loading
    }

    pub fn busy_reasons(&self) -> Vec<String> {
        let mut reasons = Vec::new();
        if self.loading {
            reasons.push("loading repositories".to_string());
        }
        if self.checking_updates {
            reasons.push("checking updates".to_string());
        }
        if self.updating_all {
            reasons.push("updating all repositories".to_string());
        }
        if !self.updating_repo_ids.is_empty() {
            reasons.push(format!("updating {} repo(s)", self.updating_repo_ids.len()));
        }
        if self.add_repo_preview_loading {
            reasons.push("loading add-repo preview".to_string());
        }
        reasons
    }

    pub fn busy_summary(&self) -> Option<String> {
        let reasons = self.busy_reasons();
        if reasons.is_empty() {
            None
        } else {
            Some(reasons.join(" + "))
        }
    }

    pub fn busy_tooltip(&self) -> String {
        match self.busy_summary() {
            Some(summary) => match self.busy_started_at {
                Some(started_at) => {
                    format!("Busy: {} ({}s)", summary, started_at.elapsed().as_secs())
                }
                None => format!("Busy: {}", summary),
            },
            None => "Idle".to_string(),
        }
    }

    fn sync_busy_tracking(&mut self) {
        let previous = self.busy_state_snapshot.clone();
        let current = self.busy_summary();
        match (previous, current) {
            (None, Some(summary)) => {
                self.busy_started_at = Some(Instant::now());
                self.busy_state_snapshot = Some(summary.clone());
                self.log(LogLevel::Info, &format!("Busy started: {}.", summary));
            }
            (Some(previous), Some(current_summary)) if previous != current_summary => {
                let elapsed = self
                    .busy_started_at
                    .map(|started_at| started_at.elapsed().as_secs())
                    .unwrap_or(0);
                self.busy_started_at = Some(Instant::now());
                self.busy_state_snapshot = Some(current_summary.clone());
                self.log(
                    LogLevel::Info,
                    &format!(
                        "Busy state changed after {}s: {} -> {}.",
                        elapsed, previous, current_summary
                    ),
                );
            }
            (Some(previous), None) => {
                let elapsed = self
                    .busy_started_at
                    .map(|started_at| started_at.elapsed().as_secs())
                    .unwrap_or(0);
                self.busy_started_at = None;
                self.busy_state_snapshot = None;
                self.log(
                    LogLevel::Info,
                    &format!("Busy cleared after {}s: {}.", elapsed, previous),
                );
            }
            _ => {}
        }
    }

    fn finish_update(&mut self, task: Task<Message>) -> Task<Message> {
        self.sync_busy_tracking();
        task
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = Vec::new();

        if self.opt_auto_check {
            let mins = self.auto_check_minutes.max(1) as u64;
            subs.push(
                iced::time::every(std::time::Duration::from_secs(mins * 60))
                    .map(|_| Message::AutoCheckTick),
            );
        }

        // Hourly self-update check for unauthenticated users; authenticated users get
        // checked on launch and on every About-tab navigation.
        if wuddle_engine::github_token().is_none() {
            subs.push(
                iced::time::every(std::time::Duration::from_secs(3600))
                    .map(|_| Message::CheckSelfUpdate),
            );
        }

        if self.is_busy() || !self.toasts.is_empty() {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(80))
                    .map(|_| Message::SpinnerTick),
            );
        }

        if self.checking_updates {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(400))
                    .map(|_| Message::PollUpdateCheckProgress),
            );
        }

        subs.push(
            iced::time::every(std::time::Duration::from_secs(60)).map(|_| Message::GithubRateTick),
        );

        if self.dialog.is_some() {
            subs.push(iced::event::listen_with(
                |event, _status, _window| match event {
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                        ..
                    }) => Some(Message::CloseDialog),
                    _ => None,
                },
            ));
        }

        Subscription::batch(subs)
    }

    pub fn colors(&self) -> ThemeColors {
        let mut c = self.wuddle_theme.colors();
        c.body_font = self.body_font();
        c
    }

    pub fn body_font(&self) -> Font {
        if self.opt_friz_font {
            FRIZ
        } else {
            NOTO
        }
    }

    /// Tab label with live update counts (matches Tauri behavior)
    pub fn tab_label(&self, tab: Tab) -> String {
        match tab {
            Tab::Home => "Home".into(),
            Tab::Mods => format!("Mods ({})", self.mod_update_count()),
            Tab::Addons => format!("Addons ({})", self.addon_update_count()),
            Tab::Tweaks => "Tweaks".into(),
            _ => tab.icon_label().into(),
        }
    }

    pub fn mod_update_count(&self) -> usize {
        self.plans
            .iter()
            .filter(|p| {
                p.has_update
                    && !self.ignored_update_ids.contains(&p.repo_id)
                    && self.repos.iter().any(|r| r.id == p.repo_id && is_mod(r))
            })
            .count()
    }

    pub fn addon_update_count(&self) -> usize {
        self.plans
            .iter()
            .filter(|p| {
                p.has_update
                    && !self.ignored_update_ids.contains(&p.repo_id)
                    && self.repos.iter().any(|r| r.id == p.repo_id && !is_mod(r))
            })
            .count()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        if let Some(task) = crate::update::misc::update(self, message.clone()) {
            return self.finish_update(task);
        }
        if let Some(task) = crate::update::radio::update(self, message.clone()) {
            return self.finish_update(task);
        }
        if let Some(task) = crate::update::tweaks::update(self, message.clone()) {
            return self.finish_update(task);
        }
        if let Some(task) = crate::update::about::update(self, message.clone()) {
            return self.finish_update(task);
        }
        if let Some(task) = crate::update::settings::update(self, message.clone()) {
            return self.finish_update(task);
        }
        if let Some(task) = crate::update::repos::update(self, message.clone()) {
            return self.finish_update(task);
        }

        match message {
            Message::SetTab(tab) => {
                self.active_tab = tab;
                self.log(LogLevel::Info, &format!("Switched to tab: {:?}.", tab));
                // Fire self-update check whenever the About tab becomes active
                if tab == Tab::About {
                    return self.finish_update(Task::perform(
                        service::check_self_update_full(self.update_channel == UpdateChannel::Beta),
                        Message::CheckSelfUpdateResult,
                    ));
                }
            }
            Message::SetTheme(_) => {}

            // Projects
            Message::SetFilter(f) => self.filter = f,
            Message::SetProjectSearch(s) => self.project_search = s,
            Message::ToggleSort(key) => {
                if self.sort_key == key {
                    self.sort_dir = match self.sort_dir {
                        SortDir::Asc => SortDir::Desc,
                        SortDir::Desc => SortDir::None,
                        SortDir::None => SortDir::Asc,
                    };
                } else {
                    self.sort_key = key;
                    self.sort_dir = SortDir::Asc;
                }
            }

            Message::ToggleAutoCheck(_)
            | Message::SetAutoCheckMinutes(_)
            | Message::ToggleDesktopNotify(_)
            | Message::ToggleSymlinks(_)
            | Message::ToggleXattr(_)
            | Message::ToggleClock12(_)
            | Message::ToggleFrizFont(_)
            | Message::SetUiScaleMode(_)
            | Message::SetGithubTokenInput(_) => {}

            // Radio settings dialog

            // Tweaks
            Message::ToggleTweak(id, val) => self.tweaks.set(id, val),

            // Logs
            Message::SetLogFilter(f) => {
                self.log_filter = f;
                self.rebuild_log_content();
            }
            Message::SetLogSearch(s) => {
                self.log_search = s;
                self.rebuild_log_content();
            }
            Message::ToggleLogWrap(b) => {
                self.log_wrap = b;
                self.save_settings();
            }
            Message::ToggleLogAutoScroll(b) => {
                self.log_autoscroll = b;
                self.save_settings();
            }
            Message::ToggleLogErrorFetch(b) => {
                self.log_error_fetch = b;
                self.rebuild_log_content();
            }
            Message::ToggleLogErrorMisc(b) => {
                self.log_error_misc = b;
                self.rebuild_log_content();
            }
            Message::ClearLogs => {
                let count = self.log_lines.len();
                self.log_lines.clear();
                self.rebuild_log_content();
                self.log(
                    LogLevel::Info,
                    &format!("Logs cleared ({} entries removed).", count),
                );
            }
            Message::LogEditorAction(action) => {
                if !action.is_edit() {
                    self.log_editor_content.perform(action);
                }
            }

            // README source toggle
            Message::ToggleReadmeSourceView => {
                self.readme_source_view = !self.readme_source_view;
            }
            Message::ReadmeEditorAction(action) => {
                if !action.is_edit() {
                    self.readme_editor_content.perform(action);
                }
            }

            // Dialogs
            Message::OpenDialog(d) => {
                self.open_menu = None;
                self.add_new_menu_open = false;
                let fetch_task = if let Dialog::RemoveRepo { id, .. } = &d {
                    let db = self.db_path.clone();
                    let repo_id = *id;
                    Task::perform(
                        service::list_repo_installs(db, repo_id),
                        Message::RemoveRepoFilesLoaded,
                    )
                } else if matches!(d, Dialog::AddRepo { .. }) {
                    let mut tasks = vec![iced::widget::operation::focus(iced::widget::Id::new(
                        "add_repo_url",
                    ))];
                    if !matches!(self.dialog, Some(Dialog::AddRepo { .. })) {
                        self.add_repo_preview = None;
                        self.add_repo_preview_loading = false;
                        self.add_repo_release_notes = None;
                        self.add_repo_show_releases = false;
                        self.add_repo_file_preview = None;
                        self.add_repo_expanded_dirs.clear();
                        self.add_repo_dir_contents.clear();
                        if let Dialog::AddRepo { url, .. } = &d {
                            if !url.trim().is_empty() {
                                tasks.push(Task::done(Message::FetchRepoPreview(url.clone())));
                            }
                        }
                    }
                    Task::batch(tasks)
                } else {
                    Task::none()
                };
                self.dialog = Some(d);
                return self.finish_update(fetch_task);
            }
            Message::RemoveRepoFilesLoaded(result) => {
                if let Some(Dialog::RemoveRepo { ref mut files, .. }) = self.dialog {
                    *files = result.unwrap_or_default();
                }
            }
            Message::CloseDialog => {
                self.dialog = None;
                self.add_repo_preview = None;
                self.add_repo_preview_loading = false;
                self.add_repo_release_notes = None;
                self.add_repo_show_releases = false;
                self.add_repo_file_preview = None;
                self.add_repo_expanded_dirs.clear();
                self.add_repo_dir_contents.clear();
            }

            // Context menu
            Message::ToggleMenu(id) => {
                if self.open_menu == Some(id) {
                    self.open_menu = None;
                } else {
                    self.open_menu = Some(id);
                }
                self.add_new_menu_open = false;
            }
            Message::CloseMenu => {
                self.open_menu = None;
                self.add_new_menu_open = false;
            }
            Message::ToggleAddNewMenu => {
                self.add_new_menu_open = !self.add_new_menu_open;
                self.open_menu = None;
            }

            // --- Phase 2: Data loading ---
            Message::SettingsLoaded(_) => {}

            // --- Phase 3: Operations ---
            Message::SaveSettings
            | Message::SaveGithubToken
            | Message::SaveGithubTokenResult(_)
            | Message::ForgetGithubToken
            | Message::ForgetGithubTokenResult(_) => {}

            // --- Shared actions ---
            Message::CopyToClipboard(text_val) => match copy_to_clipboard(&text_val) {
                Ok(()) => {
                    self.log(LogLevel::Info, "Copied to clipboard.");
                    self.show_toast("Copied to clipboard.", ToastKind::Info);
                }
                Err(e) => {
                    self.log(LogLevel::Error, &format!("Clipboard error: {}", e));
                    self.show_toast(format!("Clipboard error: {}", e), ToastKind::Error);
                }
            },

            // --- Instance settings ---
            Message::UpdateInstanceField(_)
            | Message::SaveInstanceSettings
            | Message::SwitchProfile(_) => {}

            // --- File dialog ---
            Message::RemoveProfile(profile_id) => {
                if profile_id == self.active_profile_id {
                    self.log(LogLevel::Error, "Cannot remove the active profile.");
                    return self.finish_update(Task::none());
                }
                let db = settings::profile_db_path(&profile_id).ok();
                self.profiles.retain(|p| p.id != profile_id);
                self.dialog = None;
                self.log(LogLevel::Info, &format!("Removed profile: {}", profile_id));
                self.save_settings();
                // Delete the profile's SQLite database in the background
                return self.finish_update(Task::perform(
                    async move {
                        if let Some(path) = db {
                            // Remove main db + WAL/SHM sidecars
                            for suffix in &["", "-wal", "-shm"] {
                                let p = format!("{}{}", path.display(), suffix);
                                let _ = tokio::fs::remove_file(&p).await;
                            }
                        }
                        Ok(profile_id)
                    },
                    Message::RemoveProfileResult,
                ));
            }
            Message::RemoveProfileResult(result) => match result {
                Ok(id) => self.log(
                    LogLevel::Info,
                    &format!("Deleted database for profile: {}", id),
                ),
                Err(e) => self.log(
                    LogLevel::Error,
                    &format!("Failed to delete profile db: {}", e),
                ),
            },

            // --- File dialog ---
            Message::PickWowDirectory | Message::WowDirectoryPicked(_) => {}

            // --- Tweak value setters ---

            // --- DXVK config dialog ---
            Message::OpenDxvkConfig => {
                self.open_menu = None;
                self.add_new_menu_open = false;
                self.dialog = Some(Dialog::DxvkConfig {
                    config: DxvkConfig::default(),
                    show_preview: false,
                });
                self.log(LogLevel::Info, "Opened DXVK Configurator.");
            }
            Message::SetDxvkField(field) => {
                if let Some(Dialog::DxvkConfig { ref mut config, .. }) = self.dialog {
                    match field {
                        DxvkField::MaxFrameRate(s) => config.max_frame_rate = s,
                        DxvkField::MaxFrameLatency(s) => config.max_frame_latency = s,
                        DxvkField::LatencySleep(v) => config.latency_sleep = v,
                        DxvkField::EnableDialogMode(v) => config.enable_dialog_mode = v,
                        DxvkField::DpiAware(v) => config.dpi_aware = v,
                        DxvkField::PresentInterval(v) => config.present_interval = v,
                        DxvkField::TearFree(v) => config.tear_free = v,
                        DxvkField::SamplerAnisotropy(v) => config.sampler_anisotropy = v,
                        DxvkField::ClampNegativeLodBias(v) => config.clamp_negative_lod_bias = v,
                        DxvkField::NumCompilerThreads(s) => config.num_compiler_threads = s,
                        DxvkField::EnableGpl(v) => config.enable_gpl = v,
                        DxvkField::TrackPipelineLifetime(v) => config.track_pipeline_lifetime = v,
                        DxvkField::DeferSurfaceCreation(v) => config.defer_surface_creation = v,
                        DxvkField::LenientClear(v) => config.lenient_clear = v,
                        DxvkField::LogPath(s) => config.log_path = s,
                        DxvkField::Hud(s) => config.hud = s,
                        DxvkField::EnableAsync(v) => config.enable_async = v,
                    }
                }
                // If currently showing the preview, keep it in sync with settings changes
                if let Some(Dialog::DxvkConfig {
                    ref config,
                    show_preview: true,
                }) = self.dialog
                {
                    let text = panels::dxvk_config::generate_conf(config);
                    self.dxvk_preview_content =
                        iced::widget::text_editor::Content::with_text(&text);
                }
            }
            Message::SaveDxvkConfig => {
                if let Some(Dialog::DxvkConfig { ref config, .. }) = self.dialog {
                    let content = panels::dxvk_config::generate_conf(config);
                    let path = std::path::Path::new(&self.wow_dir).join("dxvk.conf");
                    return self.finish_update(Task::perform(
                        service::save_dxvk_conf(path, content),
                        Message::DxvkConfigSaved,
                    ));
                }
            }
            Message::DxvkConfigSaved(result) => match result {
                Ok(()) => {
                    let path = std::path::Path::new(&self.wow_dir).join("dxvk.conf");
                    self.log(
                        LogLevel::Info,
                        &format!("Saved dxvk.conf \u{2192} {}", path.display()),
                    );
                    self.dialog = None;
                }
                Err(e) => self.log(LogLevel::Error, &format!("Failed to save dxvk.conf: {}", e)),
            },
            Message::ToggleDxvkPreview => {
                if let Some(Dialog::DxvkConfig {
                    ref mut show_preview,
                    ref config,
                }) = self.dialog
                {
                    *show_preview = !*show_preview;
                    if *show_preview {
                        let text = panels::dxvk_config::generate_conf(config);
                        self.dxvk_preview_content =
                            iced::widget::text_editor::Content::with_text(&text);
                    }
                }
            }
            Message::DxvkPreviewEditorAction(action) => {
                if !action.is_edit() {
                    self.dxvk_preview_content.perform(action);
                }
            }

            // Tweak read/apply/restore

            // --- About ---

            // --- Add-repo preview ---
            _ => {}
        }
        self.finish_update(Task::none())
    }

    pub fn install_options(&self) -> wuddle_engine::InstallOptions {
        wuddle_engine::InstallOptions {
            use_symlinks: self.opt_symlinks,
            set_xattr_comment: self.opt_xattr,
            replace_addon_conflicts: false,
            cache_keep_versions: 2,
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let colors = self.colors();
        let bg_start = colors.bg_grad_start;
        let bg_mid = colors.bg_grad_mid;
        let bg_end = colors.bg_grad_end;

        let topbar = self.view_topbar(&colors);
        let topbar_border =
            rule::horizontal(1).style(move |_theme| theme::topbar_rule_style(&colors));
        let body = self.view_panel(&colors);
        let footer = self.view_footer(&colors);

        let main_layout = container(column![topbar, topbar_border, body, footer])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Gradient(iced::Gradient::Linear(
                    iced::gradient::Linear::new(iced::Radians(std::f32::consts::PI))
                        .add_stop(0.0, bg_start)
                        .add_stop(0.35, bg_mid)
                        .add_stop(1.0, bg_end),
                ))),
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                text_color: None,
                snap: true,
            });

        let main_content: Element<Message> = main_layout.into();

        // Determine which overlays to add
        let overlay: Element<Message> = if self.dialog.is_some() {
            let dialog = self.dialog.as_ref().unwrap();
            let c = colors;

            // Two-card layout for AddRepo with a loaded (or loading) preview
            let has_two_cards = matches!(dialog, Dialog::AddRepo { url, .. }
                if self.add_repo_preview.is_some()
                    || (self.add_repo_preview_loading && service::parse_forge_url(url.trim()).is_some())
            );

            // For AddRepo: whether the inner content needs full height
            let add_repo_use_fill_h = match dialog {
                Dialog::AddRepo { url, is_addons, .. } => !is_addons && url.trim().is_empty(),
                _ => false,
            };

            let dialog_box: Element<Message> = if has_two_cards {
                // Two-card layout fills the padded window area
                container(self.view_dialog(dialog, &c))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            } else if matches!(dialog, Dialog::DxvkConfig { .. }) {
                // DXVK config: two-column layout, constrained width, fills height
                let c_dlg = c;
                container(self.view_dialog(dialog, &c))
                    .max_width(960u32)
                    .padding(24)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(move |_theme| theme::dialog_style(&c_dlg))
                    .into()
            } else {
                let (dialog_max_w, dialog_pad) = match dialog {
                    Dialog::AddRepo { .. } => (1400u32, 16),
                    Dialog::InstanceSettings { .. } => (600u32, 24),
                    Dialog::Changelog { .. } => (720u32, 24),
                    Dialog::AvWarning { .. } | Dialog::AddonConflict { .. } => (650u32, 24),
                    _ => (480u32, 24),
                };
                let c_dlg = c;
                let is_add_repo = matches!(dialog, Dialog::AddRepo { .. });
                if is_add_repo && add_repo_use_fill_h {
                    container(self.view_dialog(dialog, &c))
                        .max_width(dialog_max_w)
                        .padding(dialog_pad)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(move |_theme| theme::dialog_style(&c_dlg))
                        .into()
                } else if is_add_repo {
                    container(self.view_dialog(dialog, &c))
                        .max_width(dialog_max_w)
                        .padding(dialog_pad)
                        .width(Length::Fill)
                        .style(move |_theme| theme::dialog_style(&c_dlg))
                        .into()
                } else {
                    container(self.view_dialog(dialog, &c))
                        .max_width(dialog_max_w)
                        .padding(dialog_pad)
                        .style(move |_theme| theme::dialog_style(&c_dlg))
                        .into()
                }
            };

            // Wrap dialog in mouse_area to block click-through to the scrim
            let dialog_blocker = iced::widget::mouse_area(dialog_box).on_press(Message::CloseMenu);

            let scrim = iced::widget::mouse_area(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|_theme| theme::scrim_style()),
            )
            .on_press(Message::CloseDialog);

            // 40px margin on all sides \u{2248} 90% of a typical window
            let centered_dialog = container(dialog_blocker)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .padding(40);

            // Use opaque() to make the entire overlay absorb ALL mouse events,
            // preventing any interaction with main_content while the dialog is open.
            iced::widget::opaque(
                stack![scrim, centered_dialog]
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .into()
        } else {
            Space::new().width(0).height(0).into()
        };

        let base: Element<Message> = stack![main_content, overlay]
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        self.layer_toasts(base, &colors)
    }

    /// Renders the toast notification overlay on top of the given base element.
    pub fn layer_toasts<'a>(
        &'a self,
        base: Element<'a, Message>,
        colors: &ThemeColors,
    ) -> Element<'a, Message> {
        let c = *colors;

        let toast_overlay: Element<Message> = if self.toasts.is_empty() {
            Space::new().width(0).height(0).into()
        } else {
            let toast_items: Vec<Element<Message>> = self
                .toasts
                .iter()
                .map(|t| {
                    let accent = match t.kind {
                        ToastKind::Info => c.primary,
                        ToastKind::Warn => c.warn,
                        ToastKind::Error => c.bad,
                    };
                    let id = t.id;

                    let dismiss_btn = button(
                        text("\u{2715}").size(12).color(c.muted), // \u{2715}
                    )
                    .on_press(Message::DismissToast(id))
                    .padding([2, 6])
                    .style(move |_theme, _status| button::Style {
                        background: None,
                        text_color: c.muted,
                        border: iced::Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: true,
                    });

                    let msg_element: Element<Message> = if let Some(ref action) = t.on_click {
                        button(text(t.message.clone()).size(13).color(c.text))
                            .on_press(action.clone())
                            .padding(0)
                            .style(move |_theme, status| {
                                let underline = matches!(status, button::Status::Hovered);
                                button::Style {
                                    background: None,
                                    text_color: if underline { c.link } else { c.text },
                                    border: iced::Border::default(),
                                    shadow: iced::Shadow::default(),
                                    snap: true,
                                }
                            })
                            .into()
                    } else {
                        text(t.message.clone()).size(13).color(c.text).into()
                    };

                    let toast_row =
                        row![msg_element, Space::new().width(Length::Fill), dismiss_btn,]
                            .spacing(8)
                            .align_y(iced::Alignment::Center);

                    let accent_color = accent;
                    container(toast_row)
                        .padding([8, 12])
                        .width(Length::Fill)
                        .style(move |_theme| container::Style {
                            background: Some(iced::Background::Color(c.card)),
                            border: iced::Border {
                                radius: 4.0.into(),
                                width: 1.0,
                                color: accent_color,
                            },
                            shadow: iced::Shadow {
                                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                                offset: iced::Vector::new(0.0, 4.0),
                                blur_radius: 12.0,
                            },
                            text_color: None,
                            snap: true,
                        })
                        .into()
                })
                .collect();

            let toast_col = column(toast_items).spacing(6).width(480);

            // Position at bottom-center, above the footer (\u{2248}86px from bottom)
            container(
                container(toast_col)
                    .center_x(Length::Fill)
                    .width(Length::Fill)
                    .align_bottom(Length::Fill)
                    .padding(iced::Padding {
                        top: 0.0,
                        right: 0.0,
                        bottom: 86.0,
                        left: 0.0,
                    }),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        };

        stack![base, toast_overlay]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn view_dialog<'a>(
        &'a self,
        dialog: &'a Dialog,
        colors: &ThemeColors,
    ) -> Element<'a, Message> {
        let c = *colors;
        match dialog {
            Dialog::AddRepo {
                url,
                mode,
                is_addons,
                advanced,
            } => {
                let is_addons = *is_addons;
                let advanced = *advanced;
                let title = if is_addons {
                    "Add an addon repo"
                } else {
                    "Add a mod repo"
                };
                let subtitle = if is_addons {
                    "Paste a Git repository URL below. Wuddle will automatically download and install the addon for you."
                } else {
                    "Quick-add from the mods listed, or add your own Git repo URL below."
                };
                let url_label = if is_addons {
                    "Addon Repo URL"
                } else {
                    "Repo URL"
                };
                let placeholder = if is_addons {
                    "(e.g. https://github.com/pepopo978/BigWigs)"
                } else {
                    "(e.g. https://gitea.com/avitasia/nampower)"
                };

                // --- URL input with inline clear (\u{2715}) button ---
                let show_url_clear = !url.is_empty();
                let url_row: Element<Message> = {
                    let c2 = c;
                    stack![
                        iced::widget::text_input(placeholder, url)
                            .id(iced::widget::Id::new("add_repo_url"))
                            .on_input(Message::SetAddRepoUrl)
                            .on_submit(Message::AddRepoSubmit)
                            .padding(iced::Padding {
                                top: 8.0,
                                right: if show_url_clear { 32.0 } else { 12.0 },
                                bottom: 8.0,
                                left: 12.0,
                            })
                            .width(Length::Fill),
                        {
                            let clear_el: Element<Message> = if show_url_clear {
                                button(text("\u{2715}").size(12).color(c2.muted))
                                    .on_press(Message::SetAddRepoUrl(String::new()))
                                    .padding([5, 8])
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
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(iced::Alignment::End)
                        .align_y(iced::Alignment::Center)
                        .padding(iced::Padding {
                            top: 0.0,
                            right: 4.0,
                            bottom: 0.0,
                            left: 0.0
                        }),
                    ]
                    .width(Length::Fill)
                    .into()
                };

                // --- Footer: mode section + optional forge/release-notes + Cancel + Add ---
                let footer: Element<Message> = {
                    let mode_list: Vec<String> = if is_addons {
                        vec!["addon_git", "addon", "dll", "mixed", "raw"]
                    } else {
                        vec!["auto", "addon", "dll", "mixed", "raw"]
                    }
                    .into_iter()
                    .map(String::from)
                    .collect();

                    let url_cb = url.clone();
                    let mode_cb = mode.clone();
                    let url_pl = url.clone();

                    let advanced_section: Element<Message> = if advanced {
                        let picked = Some(mode.clone());
                        row![
                            iced::widget::checkbox(advanced)
                                .label("Advanced")
                                .on_toggle(move |val| Message::OpenDialog(Dialog::AddRepo {
                                    url: url_cb.clone(),
                                    mode: mode_cb.clone(),
                                    is_addons,
                                    advanced: val,
                                })),
                            text("Mode").size(12).color(c.muted),
                            iced::widget::pick_list(mode_list, picked, move |m: String| {
                                Message::OpenDialog(Dialog::AddRepo {
                                    url: url_pl.clone(),
                                    mode: m,
                                    is_addons,
                                    advanced: true,
                                })
                            })
                            .text_size(12)
                            .padding([4, 8]),
                        ]
                        .spacing(6)
                        .align_y(iced::Alignment::Center)
                        .into()
                    } else {
                        let url_c2 = url.clone();
                        let mode_c2 = mode.clone();
                        iced::widget::checkbox(advanced)
                            .label("Advanced")
                            .on_toggle(move |val| {
                                Message::OpenDialog(Dialog::AddRepo {
                                    url: url_c2.clone(),
                                    mode: mode_c2.clone(),
                                    is_addons,
                                    advanced: val,
                                })
                            })
                            .into()
                    };

                    // Forge link button: "Open on [forge icon]" (shown when preview is loaded)
                    let forge_link: Option<Element<Message>> =
                        self.add_repo_preview.as_ref().map(|p| {
                            let furl = p.forge_url.clone();
                            let icon_handle = forge_svg_handle(&p.forge, &p.forge_url);
                            let icon_color = c.text;
                            let forge_btn = button(
                                row![
                                    text("Open on")
                                        .size(12)
                                        .color(c.text)
                                        .line_height(iced::widget::text::LineHeight::Relative(1.0)),
                                    iced::widget::svg(icon_handle).width(14).height(14).style(
                                        move |_t, _s| iced::widget::svg::Style {
                                            color: Some(icon_color),
                                        }
                                    ),
                                ]
                                .spacing(5)
                                .align_y(iced::Alignment::Center),
                            )
                            .on_press(Message::OpenUrl(furl))
                            .padding([6, 10])
                            .style(move |_t, s| match s {
                                button::Status::Hovered => theme::tab_button_hovered_style(&c),
                                _ => theme::tab_button_style(&c),
                            });
                            tip(
                                forge_btn,
                                "View this repository in your browser",
                                iced::widget::tooltip::Position::Top,
                                colors,
                            )
                        });

                    // Release Notes / README toggle button (shown when preview is loaded)
                    let release_notes: Option<Element<Message>> =
                        self.add_repo_preview.as_ref().map(|_p| {
                            let (label, msg, rn_tip) = if self.add_repo_show_releases {
                                ("README", Message::ShowReadme, "Switch back to README view")
                            } else {
                                (
                                    "Release Notes",
                                    Message::FetchReleaseNotes,
                                    "View release notes and changelogs",
                                )
                            };
                            let rn_btn = button(text(label).size(12))
                                .on_press(msg)
                                .padding([6, 10])
                                .style(move |_t, s| match s {
                                    button::Status::Hovered => theme::tab_button_hovered_style(&c),
                                    _ => theme::tab_button_style(&c),
                                });
                            tip(rn_btn, rn_tip, iced::widget::tooltip::Position::Top, colors)
                        });

                    let mut footer_row: Vec<Element<Message>> = Vec::new();
                    if let Some(fl) = forge_link {
                        footer_row.push(fl);
                    }
                    if let Some(rn) = release_notes {
                        footer_row.push(rn);
                    }
                    footer_row.push(advanced_section);
                    footer_row.push(Space::new().width(Length::Fill).into());
                    footer_row.push(
                        button(text("Cancel").size(13))
                            .on_press(Message::CloseDialog)
                            .padding([6, 14])
                            .style(move |_t, s| match s {
                                button::Status::Hovered => theme::tab_button_hovered_style(&c),
                                _ => theme::tab_button_style(&c),
                            })
                            .into(),
                    );
                    {
                        let is_installed = self.repos.iter().any(|r| {
                            let r_url = r.url.trim().trim_end_matches('/').to_lowercase();
                            let d_url = url.trim().trim_end_matches('/').to_lowercase();
                            !d_url.is_empty()
                                && (r_url == d_url
                                    || r_url == format!("{}.git", d_url)
                                    || format!("{}.git", r_url) == d_url)
                        });

                        let add_label = if is_installed {
                            "Installed"
                        } else if is_addons {
                            "Add addon"
                        } else {
                            "Add mod"
                        };
                        let add_tip_text = if is_installed {
                            "This repository is already managed by Wuddle"
                        } else if is_addons {
                            "Add this addon to Wuddle"
                        } else {
                            "Add this mod to Wuddle"
                        };

                        let mut add_btn = button(text(add_label).size(13)).padding([6, 14]).style(
                            move |_t, _s| {
                                if is_installed {
                                    theme::tab_button_style(&c)
                                } else {
                                    theme::tab_button_active_style(&c)
                                }
                            },
                        );

                        if !is_installed {
                            add_btn = add_btn.on_press(Message::AddRepoSubmit);
                        }
                        footer_row.push(tip(
                            add_btn,
                            add_tip_text,
                            iced::widget::tooltip::Position::Top,
                            colors,
                        ));
                    }
                    row(footer_row)
                        .spacing(8)
                        .align_y(iced::Alignment::Center)
                        .into()
                };

                // Use two-card layout whenever there's a preview loaded OR we're actively
                // fetching one \u{2014} prevents the dialog from collapsing/jumping during loads.
                let fetching_preview =
                    self.add_repo_preview_loading && service::parse_forge_url(url.trim()).is_some();

                if self.add_repo_preview.is_some() || fetching_preview {
                    // =========================================================
                    // TWO-CARD LAYOUT: floating side panel + main form card
                    // =========================================================
                    let current_theme = self.theme();
                    let c_sp = c;
                    let c_form = c;
                    let c_divider = c;

                    // --- SIDE PANEL CARD (About + Files) ---
                    let mut sidebar_col: Vec<Element<Message>> = Vec::new();

                    if let Some(ref preview) = self.add_repo_preview {
                        // About section header
                        sidebar_col.push(text("About").size(12).color(colors.muted).into());
                        sidebar_col.push(text(&preview.name).size(15).color(colors.text).into());
                        if !preview.description.is_empty() {
                            sidebar_col.push(
                                text(&preview.description)
                                    .size(12)
                                    .color(colors.text_soft)
                                    .into(),
                            );
                        }

                        // Stats
                        sidebar_col.push(
                            row![
                                text("\u{2b50}").size(12), // \u{2b50}
                                text(format!(
                                    "{} star{}",
                                    preview.stars,
                                    if preview.stars == 1 { "" } else { "s" }
                                ))
                                .size(13)
                                .color(colors.text_soft),
                            ]
                            .spacing(4)
                            .into(),
                        );
                        if preview.forks > 0 {
                            let forks_url = format!("{}/forks", preview.forge_url);
                            let c_fk = c;
                            let fork_count = preview.forks;
                            sidebar_col.push(
                                row![
                                    text("\u{1f374}").size(12), // \u{1f374}
                                    button(iced::widget::rich_text::<(), _, _, _>([
                                        iced::widget::span(format!(
                                            "{} fork{}",
                                            fork_count,
                                            if fork_count == 1 { "" } else { "s" }
                                        ))
                                        .underline(true)
                                        .color(c_fk.link)
                                        .size(13.0_f32),
                                    ]))
                                    .on_press(Message::OpenUrl(forks_url))
                                    .padding(0)
                                    .style(move |_t, _s| button::Style {
                                        background: None,
                                        text_color: c_fk.link,
                                        border: iced::Border::default(),
                                        shadow: iced::Shadow::default(),
                                        snap: true,
                                    }),
                                ]
                                .spacing(4)
                                .align_y(iced::Alignment::Center)
                                .into(),
                            );
                        }
                        if !preview.language.is_empty() {
                            sidebar_col.push(
                                row![
                                    text("\u{1f4bb}").size(12), // \u{1f4bb}
                                    text(&preview.language).size(12).color(colors.text_soft),
                                ]
                                .spacing(4)
                                .into(),
                            );
                        }
                        if !preview.license.is_empty() {
                            sidebar_col.push(
                                row![
                                    text("\u{1f4cb}").size(12), // \u{1f4cb}
                                    text(&preview.license).size(12).color(colors.text_soft),
                                ]
                                .spacing(4)
                                .into(),
                            );
                        }

                        // Files section
                        if !preview.files.is_empty() {
                            sidebar_col.push(
                                rule::horizontal(1)
                                    .style(move |_t| theme::update_line_style(&c_divider))
                                    .into(),
                            );
                            sidebar_col.push(text("Files").size(12).color(colors.muted).into());

                            let mut sorted_files = preview.files.clone();
                            sorted_files
                                .sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));

                            let mut file_rows: Vec<Element<Message>> = Vec::new();
                            for f in sorted_files.iter().take(60) {
                                let c_tree = c;
                                let path = f.path.clone();
                                if f.is_dir {
                                    let expanded = self.add_repo_expanded_dirs.contains(&f.path);
                                    let folder_icon =
                                        if expanded { "\u{1f4c2}" } else { "\u{1f4c1}" }; // \u{1f4c2} / \u{1f4c1}
                                    file_rows.push(
                                        button(
                                            text(format!("{} {}", folder_icon, f.name))
                                                .size(12)
                                                .color(colors.text),
                                        )
                                        .on_press(Message::ToggleAddRepoDir(path.clone()))
                                        .padding([2, 4])
                                        .style(move |_t, status| match status {
                                            button::Status::Hovered => button::Style {
                                                background: Some(iced::Background::Color(
                                                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.07),
                                                )),
                                                text_color: c_tree.text,
                                                border: iced::Border::default(),
                                                shadow: iced::Shadow::default(),
                                                snap: true,
                                            },
                                            _ => button::Style {
                                                background: None,
                                                text_color: c_tree.text,
                                                border: iced::Border::default(),
                                                shadow: iced::Shadow::default(),
                                                snap: true,
                                            },
                                        })
                                        .into(),
                                    );
                                    if expanded {
                                        if let Some(children) =
                                            self.add_repo_dir_contents.get(&f.path)
                                        {
                                            for child in children.iter().take(40) {
                                                let c_ch = c;
                                                let child_path = child.path.clone();
                                                let child_icon = if child.is_dir {
                                                    "\u{1f4c1}"
                                                } else {
                                                    "\u{1f4c4}"
                                                };
                                                let child_msg = if child.is_dir {
                                                    Message::ToggleAddRepoDir(child_path.clone())
                                                } else {
                                                    Message::PreviewRepoFile(child_path.clone())
                                                };
                                                let child_color = if child.is_dir {
                                                    colors.text
                                                } else {
                                                    colors.text_soft
                                                };
                                                file_rows.push(
                                                    button(row![
                                                        Space::new().width(14),
                                                        text(format!(
                                                            "{} {}",
                                                            child_icon, child.name
                                                        ))
                                                        .size(11)
                                                        .color(child_color),
                                                    ])
                                                    .on_press(child_msg)
                                                    .padding([1, 4])
                                                    .style(move |_t, status| match status {
                                                        button::Status::Hovered => button::Style {
                                                            background: Some(
                                                                iced::Background::Color(
                                                                    iced::Color::from_rgba(
                                                                        1.0, 1.0, 1.0, 0.07,
                                                                    ),
                                                                ),
                                                            ),
                                                            text_color: c_ch.text,
                                                            border: iced::Border::default(),
                                                            shadow: iced::Shadow::default(),
                                                            snap: true,
                                                        },
                                                        _ => button::Style {
                                                            background: None,
                                                            text_color: c_ch.text_soft,
                                                            border: iced::Border::default(),
                                                            shadow: iced::Shadow::default(),
                                                            snap: true,
                                                        },
                                                    })
                                                    .into(),
                                                );
                                            }
                                        } else {
                                            file_rows.push(
                                                row![
                                                    Space::new().width(18),
                                                    text("Loading\u{2026}")
                                                        .size(11)
                                                        .color(colors.muted),
                                                ]
                                                .into(),
                                            );
                                        }
                                    }
                                } else {
                                    file_rows.push(
                                        button(
                                            text(format!("\u{1f4c4} {}", f.name))
                                                .size(12)
                                                .color(colors.text_soft),
                                        )
                                        .on_press(Message::PreviewRepoFile(path))
                                        .padding([2, 4])
                                        .style(move |_t, status| match status {
                                            button::Status::Hovered => button::Style {
                                                background: Some(iced::Background::Color(
                                                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.07),
                                                )),
                                                text_color: c_tree.text,
                                                border: iced::Border::default(),
                                                shadow: iced::Shadow::default(),
                                                snap: true,
                                            },
                                            _ => button::Style {
                                                background: None,
                                                text_color: c_tree.text_soft,
                                                border: iced::Border::default(),
                                                shadow: iced::Shadow::default(),
                                                snap: true,
                                            },
                                        })
                                        .into(),
                                    );
                                }
                            }

                            // Files fill remaining sidebar height
                            sidebar_col.push(
                                iced::widget::scrollable(
                                    column(file_rows).spacing(1).width(Length::Fill),
                                )
                                .width(Length::Fill)
                                .height(Length::Fill)
                                .direction(theme::vscroll())
                                .style(move |t, s| theme::scrollable_style(&c_sp)(t, s))
                                .into(),
                            );
                        }
                    } else {
                        // Fetching: fill the sidebar with a centered spinner
                        let tick = self.spinner_tick;
                        let primary = colors.primary;
                        sidebar_col.push(
                            container(
                                column![
                                    canvas(SpinnerCanvas {
                                        tick,
                                        color: primary
                                    })
                                    .width(24)
                                    .height(24),
                                    text("Loading preview\u{2026}").size(14).color(colors.muted),
                                ]
                                .spacing(10)
                                .align_x(iced::Alignment::Center),
                            )
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .align_x(iced::Alignment::Center)
                            .align_y(iced::Alignment::Center)
                            .into(),
                        );
                    }

                    let sidebar_card = container(
                        column(sidebar_col)
                            .spacing(6)
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .width(250)
                    .height(Length::Fill)
                    .padding([16, 14])
                    .style(move |_theme| theme::dialog_style(&c_sp));

                    // --- MAIN FORM CARD ---
                    // Build the content label row \u{2014} includes source toggle when showing README
                    let content_label: Element<Message> = if let Some((ref fname, _)) =
                        self.add_repo_file_preview
                    {
                        let c_lbl = c_form;
                        let label = format!("\u{2190} {}", fname);
                        button(text(label).size(12).color(colors.link))
                            .on_press(Message::ShowReadme)
                            .padding([0, 0])
                            .style(move |_t, _s| button::Style {
                                background: None,
                                text_color: c_lbl.link,
                                border: iced::Border::default(),
                                shadow: iced::Shadow::default(),
                                snap: true,
                            })
                            .into()
                    } else if self.add_repo_show_releases {
                        text("Release Notes").size(14).color(colors.muted).into()
                    } else {
                        // README label + source toggle on the same row
                        let readme_label = text("README").size(14).color(colors.muted);
                        if self.add_repo_preview.is_some() {
                            let source_label = if self.readme_source_view {
                                "Source"
                            } else {
                                "Formatted"
                            };
                            let c2 = c_form;
                            let source_btn = button(text(source_label).size(11))
                                .on_press(Message::ToggleReadmeSourceView)
                                .padding([3, 8])
                                .style(move |_theme, status| match status {
                                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                                    _ => theme::tab_button_style(&c2),
                                });
                            row![readme_label, Space::new().width(Length::Fill), source_btn,]
                                .align_y(iced::Alignment::Center)
                                .into()
                        } else {
                            readme_label.into()
                        }
                    };

                    let scrollable_content: Element<Message> = if let Some((_, ref content)) =
                        self.add_repo_file_preview
                    {
                        // Show file content (plain text, monospace)
                        let inner_content = container(
                            iced::widget::scrollable(
                                text(content.as_str())
                                    .size(12)
                                    .font(Font::MONOSPACE)
                                    .color(colors.text),
                            )
                            .height(Length::Fill)
                            .direction(theme::vscroll())
                            .style(move |t, s| theme::scrollable_style(&c_form)(t, s)),
                        )
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .padding(8)
                        .style(move |_t| container::Style {
                            background: Some(iced::Background::Color(iced::Color::from_rgba(
                                0.0, 0.0, 0.0, 0.38,
                            ))),
                            border: iced::Border {
                                color: c_form.border,
                                width: 1.0,
                                radius: iced::border::Radius::from(0),
                            },
                            ..Default::default()
                        });
                        inner_content.into()
                    } else if self.add_repo_show_releases {
                        // Show release notes or loading indicator
                        if let Some(ref releases) = self.add_repo_release_notes {
                            if releases.is_empty() {
                                container(text("No releases found.").size(13).color(colors.muted))
                                    .padding([8, 0])
                                    .into()
                            } else {
                                let c_rl = c_form;
                                let rn_theme = self.theme();
                                let mut rn_style = iced::widget::markdown::Style::from(&rn_theme);
                                rn_style.link_color = c_rl.link;
                                let rn_settings =
                                    iced::widget::markdown::Settings::with_text_size(12, rn_style);
                                let release_cards: Vec<Element<Message>> = releases
                                    .iter()
                                    .map(|r| {
                                        let date =
                                            r.published_at.get(..10).unwrap_or(&r.published_at);
                                        let mut col_items: Vec<Element<Message>> = vec![row![
                                            text(&r.name).size(14).color(colors.text),
                                            Space::new().width(Length::Fill),
                                            text(date).size(11).color(colors.muted),
                                        ]
                                        .align_y(iced::Alignment::Center)
                                        .into()];
                                        if r.tag_name != r.name && !r.tag_name.is_empty() {
                                            col_items.push(
                                                text(&r.tag_name)
                                                    .size(11)
                                                    .color(colors.muted)
                                                    .into(),
                                            );
                                        }
                                        if r.prerelease {
                                            col_items.push(badge_tag(
                                                "pre-release",
                                                iced::Color::from_rgb8(0xfd, 0xe6, 0x8a),
                                                iced::Color::from_rgb8(0xd4, 0x82, 0x1a),
                                            ));
                                        }
                                        if !r.items.is_empty() {
                                            col_items.push(
                                                iced::widget::markdown::view(&r.items, rn_settings)
                                                    .map(Message::OpenUrl)
                                                    .into(),
                                            );
                                        }
                                        container(column(col_items).spacing(3))
                                            .width(Length::Fill)
                                            .padding([8, 12])
                                            .style(move |_t| theme::card_style(&c_rl))
                                            .into()
                                    })
                                    .collect();
                                iced::widget::scrollable(
                                    column(release_cards).spacing(6).width(Length::Fill),
                                )
                                .height(Length::Fill)
                                .direction(theme::vscroll())
                                .style(move |t, s| theme::scrollable_style(&c_rl)(t, s))
                                .into()
                            }
                        } else {
                            // Still loading
                            let tick = self.spinner_tick;
                            let primary = colors.primary;
                            container(
                                row![
                                    canvas(SpinnerCanvas {
                                        tick,
                                        color: primary
                                    })
                                    .width(18)
                                    .height(18),
                                    text("Resolving...").size(13).color(colors.muted),
                                ]
                                .spacing(8)
                                .align_y(iced::Alignment::Center),
                            )
                            .padding([8, 0])
                            .into()
                        }
                    } else if let Some(ref preview) = self.add_repo_preview {
                        // Show README (or placeholder if empty)
                        let readme_is_source = self.readme_source_view;
                        let inner_scrollable: Element<Message> = if preview.readme_items.is_empty()
                        {
                            container(
                                column![
                                    text("\u{1f4c4}").size(32),
                                    text("No README found for this repository.")
                                        .size(13)
                                        .color(colors.muted),
                                ]
                                .spacing(8)
                                .align_x(iced::Alignment::Center),
                            )
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .align_x(iced::Alignment::Center)
                            .align_y(iced::Alignment::Center)
                            .into()
                        } else if readme_is_source {
                            // Source view: selectable raw markdown text
                            iced::widget::text_editor(&self.readme_editor_content)
                                .on_action(Message::ReadmeEditorAction)
                                .font(Font::MONOSPACE)
                                .size(12)
                                .height(Length::Fill)
                                .padding(10)
                                .style(move |_theme, _status| iced::widget::text_editor::Style {
                                    background: iced::Background::Color(iced::Color::from_rgb8(
                                        0x0d, 0x11, 0x1a,
                                    )),
                                    border: iced::Border::default(),
                                    placeholder: iced::Color::from_rgb8(0x4a, 0x55, 0x68),
                                    value: iced::Color::from_rgb8(0xdb, 0xe7, 0xff),
                                    selection: iced::Color {
                                        a: 0.35,
                                        ..iced::Color::from_rgb8(0x4a, 0x90, 0xd9)
                                    },
                                })
                                .into()
                        } else {
                            let viewer = ImageViewer {
                                cache: &preview.image_cache,
                                gif_cache: &preview.gif_cache,
                                raw_base_url: &preview.raw_base_url,
                            };
                            let mut md_style = iced::widget::markdown::Style::from(&current_theme);
                            md_style.link_color = c_form.link;
                            // Use theme text color for body (not the iced default white)
                            md_style.font = iced::Font::DEFAULT;
                            // Style inline code to match dark terminal aesthetic
                            md_style.inline_code_color = iced::Color::from_rgb8(0xe0, 0xc0, 0x80);
                            md_style.inline_code_highlight = iced::widget::markdown::Highlight {
                                background: iced::Color::from_rgb8(0x14, 0x18, 0x24).into(),
                                border: iced::Border {
                                    color: iced::Color::from_rgb8(0x2a, 0x2f, 0x3d),
                                    width: 1.0,
                                    radius: 3.0.into(),
                                },
                            };
                            let mut md_settings =
                                iced::widget::markdown::Settings::with_text_size(13, md_style);
                            // Tighten heading sizes to be closer to Tauri's rendering
                            md_settings.h1_size = 22.0.into();
                            md_settings.h2_size = 19.0.into();
                            md_settings.h3_size = 16.0.into();
                            md_settings.h4_size = 14.0.into();
                            let readme_view = iced::widget::markdown::view_with(
                                &preview.readme_items,
                                md_settings,
                                &viewer,
                            );
                            iced::widget::scrollable(readme_view)
                                .height(Length::Fill)
                                .direction(theme::vscroll())
                                .style(move |t, s| theme::scrollable_style(&c_form)(t, s))
                                .into()
                        };
                        // Wrap the scrollable in a dark card (source toggle now lives on the label row)
                        container(inner_scrollable)
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .padding(8)
                            .style(move |_t| container::Style {
                                background: Some(iced::Background::Color(iced::Color::from_rgba(
                                    0.0, 0.0, 0.0, 0.38,
                                ))),
                                border: iced::Border {
                                    color: c_form.border,
                                    width: 1.0,
                                    radius: iced::border::Radius::from(0),
                                },
                                ..Default::default()
                            })
                            .into()
                    } else {
                        // Loading state: centered spinner so layout stays stable at full size
                        let tick = self.spinner_tick;
                        let primary = colors.primary;
                        container(
                            column![
                                canvas(SpinnerCanvas {
                                    tick,
                                    color: primary
                                })
                                .width(28)
                                .height(28),
                                text("Loading\u{2026}").size(15).color(colors.muted),
                            ]
                            .spacing(12)
                            .align_x(iced::Alignment::Center),
                        )
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(iced::Alignment::Center)
                        .align_y(iced::Alignment::Center)
                        .padding([12, 8])
                        .style(move |_t| container::Style {
                            background: Some(iced::Background::Color(iced::Color::from_rgba(
                                0.0, 0.0, 0.0, 0.38,
                            ))),
                            border: iced::Border {
                                color: c_form.border,
                                width: 1.0,
                                radius: iced::border::Radius::from(0),
                            },
                            ..Default::default()
                        })
                        .into()
                    };

                    let form_card = container(
                        column![
                            // Sticky header
                            row![
                                text(title).size(17).color(colors.title),
                                Space::new().width(Length::Fill),
                                close_button(&c_form),
                            ]
                            .align_y(iced::Alignment::Center),
                            text(subtitle).size(12).color(colors.text_soft),
                            rule::horizontal(1).style(move |_t| theme::update_line_style(&c_form)),
                            text(url_label).size(12).color(colors.text),
                            url_row,
                            Space::new().height(4),
                            content_label,
                            // Scrollable content fills remaining space
                            scrollable_content,
                            rule::horizontal(1).style(move |_t| theme::update_line_style(&c_form)),
                            footer,
                        ]
                        .spacing(6)
                        .height(Length::Fill),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding([16, 20])
                    .style(move |_theme| theme::dialog_style(&c_form));

                    row![sidebar_card, form_card]
                        .spacing(8)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into()
                } else {
                    // =========================================================
                    // SINGLE-CARD LAYOUT: no preview \u{2014} show Quick Add or loading
                    // =========================================================
                    let body_content: Element<Message> = if self.add_repo_preview_loading {
                        let tick = self.spinner_tick;
                        let primary = colors.primary;
                        container(
                            row![
                                canvas(SpinnerCanvas {
                                    tick,
                                    color: primary
                                })
                                .width(18)
                                .height(18),
                                text("Resolving...").size(13).color(colors.muted),
                            ]
                            .spacing(8)
                            .align_y(iced::Alignment::Center),
                        )
                        .padding([12, 0])
                        .into()
                    } else if !is_addons && url.trim().is_empty() {
                        // Quick Add preset list (mods tab only, when no URL entered)
                        build_quick_add_presets(&self.repos, colors)
                    } else {
                        Space::new().height(0).into()
                    };

                    let body_section: Element<Message> =
                        if self.add_repo_preview_loading || (!is_addons && url.trim().is_empty()) {
                            let section_label: Element<Message> = if is_addons {
                                Space::new().height(0).into()
                            } else {
                                text("Quick Add").size(12).color(colors.muted).into()
                            };
                            column![
                                section_label,
                                iced::widget::scrollable(body_content)
                                    .height(Length::Fill)
                                    .direction(theme::vscroll())
                                    .style(move |t, s| theme::scrollable_style(&c)(t, s)),
                            ]
                            .spacing(4)
                            .height(Length::Fill)
                            .into()
                        } else {
                            Space::new().height(0).into()
                        };

                    column![
                        row![
                            text(title).size(17).color(colors.title),
                            Space::new().width(Length::Fill),
                            close_button(&c),
                        ]
                        .align_y(iced::Alignment::Center),
                        text(subtitle).size(12).color(colors.text_soft),
                        rule::horizontal(1).style(move |_t| theme::update_line_style(&c)),
                        text(url_label).size(12).color(colors.text),
                        url_row,
                        body_section,
                        rule::horizontal(1).style(move |_t| theme::update_line_style(&c)),
                        footer,
                    ]
                    .spacing(6)
                    .width(Length::Fill)
                    .height(if !is_addons && url.trim().is_empty() {
                        Length::Fill
                    } else {
                        Length::Shrink
                    })
                    .into()
                }
            }
            Dialog::Changelog {
                title,
                items,
                loading,
            } => {
                let body: Element<Message> = if *loading {
                    container(
                        text(format!("Loading {}\u{2026}", title.to_lowercase()))
                            .size(13)
                            .color(colors.muted),
                    )
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .width(Length::Fill)
                    .height(Length::Fixed(300.0))
                    .into()
                } else {
                    let mut cl_style = iced::widget::markdown::Style::from(&self.theme());
                    cl_style.link_color = c.link;
                    // Matching dark terminal aesthetic for inline code
                    cl_style.inline_code_color = iced::Color::from_rgb8(0xe0, 0xc0, 0x80);
                    cl_style.inline_code_highlight = iced::widget::markdown::Highlight {
                        background: iced::Color::from_rgb8(0x14, 0x18, 0x24).into(),
                        border: iced::Border {
                            color: iced::Color::from_rgb8(0x2a, 0x2f, 0x3d),
                            width: 1.0,
                            radius: 3.0.into(),
                        },
                    };

                    let md_settings =
                        iced::widget::markdown::Settings::with_text_size(13, cl_style);

                    let viewer = ImageViewer {
                        cache: &self.markdown_image_cache,
                        gif_cache: &self.markdown_gif_cache,
                        raw_base_url: "",
                    };

                    iced::widget::scrollable(iced::widget::markdown::view_with(
                        items,
                        md_settings,
                        &viewer,
                    ))
                    .height(Length::Fixed(480.0))
                    .direction(theme::vscroll())
                    .style(move |t, s| theme::scrollable_style(&c)(t, s))
                    .into()
                };
                column![
                    row![
                        text(title)
                            .size(18)
                            .font(iced::Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            })
                            .color(colors.title),
                        Space::new().width(Length::Fill),
                        close_button(&c),
                    ]
                    .align_y(iced::Alignment::Center),
                    body,
                ]
                .spacing(12)
                .width(Length::Fixed(700.0))
                .into()
            }
            Dialog::RemoveRepo {
                id,
                name,
                remove_files,
                files,
            } => {
                let rid = *id;
                let rf = *remove_files;

                // File tree preview
                let file_rows: Vec<Element<Message>> = files
                    .iter()
                    .map(|(path, kind)| {
                        let icon = match kind.as_str() {
                            "dll" => "\u{2699}",    // \u{2699}
                            "addon" => "\u{1f4c1}", // \u{1f4c1}
                            _ => "\u{1f4c4}",       // \u{1f4c4}
                        };
                        let color = if rf { colors.warn } else { colors.text_soft };
                        container(text(format!("{} {}", icon, path)).size(12).color(color))
                            .padding([2, 6])
                            .into()
                    })
                    .collect();

                let file_tree: Element<Message> = if files.is_empty() {
                    text("No tracked files found.")
                        .size(12)
                        .color(colors.muted)
                        .into()
                } else {
                    scrollable(column(file_rows).spacing(0).width(Length::Fill))
                        .height(iced::Length::Fixed(160.0))
                        .direction(theme::vscroll_overlay())
                        .style(move |t, s| theme::scrollable_style(&c)(t, s))
                        .into()
                };

                let file_section: Element<Message> = container(file_tree)
                    .width(Length::Fill)
                    .padding([6, 0])
                    .style(move |_t| container::Style {
                        background: Some(iced::Background::Color(iced::Color { a: 0.5, ..c.card })),
                        border: iced::Border {
                            color: iced::Color {
                                a: 0.15,
                                ..c.border
                            },
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..Default::default()
                    })
                    .into();

                column![
                    row![
                        text("Remove Repository").size(18).color(colors.title),
                        Space::new().width(Length::Fill),
                        close_button(&c),
                    ].align_y(iced::Alignment::Center),
                    text(format!("Remove \"{}\" from Wuddle?", name))
                        .size(13)
                        .color(colors.text),
                    file_section,
                    checkbox(rf)
                        .label("Also delete local files (DLLs / addon folders)")
                        .on_toggle(Message::ToggleRemoveFiles)
                        .text_size(13),
                    text(if rf {
                        "\u{26a0}\u{fe0f} Installed files will be permanently deleted from your WoW directory."
                    } else {
                        "Wuddle will stop tracking this mod. Local files will be left on disk."
                    })
                    .size(12)
                    .color(if rf { colors.warn } else { colors.muted }),
                    row![
                        Space::new().width(Length::Fill),
                        button(text("Cancel").size(13))
                            .on_press(Message::CloseDialog)
                            .padding([6, 12])
                            .style(move |_theme, status| match status {
                                button::Status::Hovered => theme::tab_button_hovered_style(&c),
                                _ => theme::tab_button_style(&c),
                            }),
                        {
                            let c2 = c;
                            let rm_tip = if rf { "Remove and delete local files" } else { "Stop tracking this repository" };
                            tip(
                                button(text("Remove").size(13).color(c.bad))
                                    .on_press(Message::RemoveRepoConfirm(rid, rf))
                                    .padding([6, 12])
                                    .style(move |_theme, _status| {
                                        let mut s = theme::tab_button_style(&c2);
                                        s.border.color = c2.bad;
                                        s
                                    }),
                                rm_tip,
                                iced::widget::tooltip::Position::Top,
                                colors,
                            )
                        },
                    ]
                    .spacing(8),
                ]
                .spacing(12)
                .into()
            }
            Dialog::DllCountWarning {
                repo_id,
                repo_name,
                previous_count,
                new_count,
            } => {
                let rid = *repo_id;
                let fewer = *new_count < *previous_count;
                let description = if fewer {
                    format!(
                        "This release has {} DLL file{} but you currently have {} installed. \
                         A clean update will remove {} existing DLL{}.",
                        new_count,
                        if *new_count == 1 { "" } else { "s" },
                        previous_count,
                        previous_count - new_count,
                        if previous_count - new_count == 1 {
                            ""
                        } else {
                            "s"
                        },
                    )
                } else {
                    format!(
                        "This release has {} DLL file{} but you currently have {} installed.",
                        new_count,
                        if *new_count == 1 { "" } else { "s" },
                        previous_count,
                    )
                };
                column![
                    row![
                        text("DLL File Count Changed").size(18).color(colors.title),
                        Space::new().width(Length::Fill),
                        close_button(&c),
                    ]
                    .align_y(iced::Alignment::Center),
                    text(format!("\"{}\"", repo_name))
                        .size(13)
                        .color(colors.text),
                    text(description).size(13).color(colors.warn),
                    text("How would you like to proceed?")
                        .size(13)
                        .color(colors.text),
                    row![
                        {
                            let c2 = c;
                            button(
                                column![
                                    text("Merge Update").size(13),
                                    text("Keep existing DLLs, only overwrite matching files")
                                        .size(11)
                                        .color(c2.muted),
                                ]
                                .spacing(2),
                            )
                            .on_press(Message::DllCountWarningChoice {
                                repo_id: rid,
                                merge: true,
                            })
                            .padding([10, 16])
                            .width(Length::FillPortion(1))
                            .style(
                                move |_theme, status| match status {
                                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                                    _ => theme::tab_button_style(&c2),
                                },
                            )
                        },
                        {
                            let c2 = c;
                            button(
                                column![
                                    text("Clean Update").size(13),
                                    text("Remove old DLLs first, then install new release")
                                        .size(11)
                                        .color(c2.muted),
                                ]
                                .spacing(2),
                            )
                            .on_press(Message::DllCountWarningChoice {
                                repo_id: rid,
                                merge: false,
                            })
                            .padding([10, 16])
                            .width(Length::FillPortion(1))
                            .style(
                                move |_theme, status| match status {
                                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                                    _ => theme::tab_button_style(&c2),
                                },
                            )
                        },
                    ]
                    .spacing(8),
                ]
                .spacing(12)
                .into()
            }
            Dialog::InstanceSettings {
                is_new,
                profile_id,
                name,
                wow_dir,
                launch_method,
                like_turtles,
                clear_wdb,
                lutris_target,
                wine_command,
                wine_args,
                custom_command,
                custom_args,
            } => {
                let title_text = if *is_new {
                    "Add Instance"
                } else {
                    "Instance Settings"
                };
                let can_remove = !*is_new;
                let is_active_profile = *profile_id == self.active_profile_id;
                let remove_id = profile_id.clone();

                let method_buttons: Vec<Element<Message>> = [
                    ("Auto", "auto"),
                    ("Lutris", "lutris"),
                    ("Wine", "wine"),
                    ("Custom", "custom"),
                ]
                .iter()
                .map(|&(label, m)| {
                    let c2 = c;
                    let is_active = launch_method == m;
                    let m_str = String::from(m);
                    let btn = button(text(label).size(12))
                        .on_press(Message::UpdateInstanceField(InstanceField::LaunchMethod(
                            m_str,
                        )))
                        .padding([4, 10]);
                    if is_active {
                        btn.style(move |_t, _s| theme::tab_button_active_style(&c2))
                            .into()
                    } else {
                        btn.style(move |_t, s| match s {
                            button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                            _ => theme::tab_button_style(&c2),
                        })
                        .into()
                    }
                })
                .collect();

                // Conditional launch method fields
                let launch_fields: Element<Message> = match launch_method.as_str() {
                    "lutris" => column![
                        text("Lutris target").size(13).color(colors.text),
                        iced::widget::text_input("lutris:rungameid/2", lutris_target)
                            .on_input(|s| Message::UpdateInstanceField(
                                InstanceField::LutrisTarget(s)
                            ))
                            .padding([8, 12]),
                        text("Example: lutris:rungameid/2")
                            .size(11)
                            .color(colors.muted),
                    ]
                    .spacing(4)
                    .into(),
                    "wine" => column![
                        text("Wine command").size(13).color(colors.text),
                        iced::widget::text_input("wine", wine_command)
                            .on_input(|s| Message::UpdateInstanceField(InstanceField::WineCommand(
                                s
                            )))
                            .padding([8, 12]),
                        text("Wine arguments").size(13).color(colors.text),
                        iced::widget::text_input("--some-arg value", wine_args)
                            .on_input(|s| Message::UpdateInstanceField(InstanceField::WineArgs(s)))
                            .padding([8, 12]),
                    ]
                    .spacing(4)
                    .into(),
                    "custom" => column![
                        text("Custom command").size(13).color(colors.text),
                        iced::widget::text_input("command", custom_command)
                            .on_input(|s| Message::UpdateInstanceField(
                                InstanceField::CustomCommand(s)
                            ))
                            .padding([8, 12]),
                        text("Custom arguments").size(13).color(colors.text),
                        iced::widget::text_input("--flag value", custom_args)
                            .on_input(|s| Message::UpdateInstanceField(InstanceField::CustomArgs(
                                s
                            )))
                            .padding([8, 12]),
                        text("Tip: use {exe} in args to inject the detected game executable path.")
                            .size(11)
                            .color(colors.muted),
                    ]
                    .spacing(4)
                    .into(),
                    _ => {
                        // "auto"
                        text("Auto: launches VanillaFixes.exe if present, otherwise Wow.exe")
                            .size(12)
                            .color(colors.muted)
                            .into()
                    }
                };

                column![
                    row![
                        text(title_text).size(18).color(colors.title),
                        Space::new().width(Length::Fill),
                        close_button(&c),
                    ]
                    .align_y(iced::Alignment::Center),
                    text("Configure name, game path, and launch behavior for this instance.")
                        .size(12)
                        .color(colors.muted),
                    text("Instance name").size(13).color(colors.text),
                    iced::widget::text_input("My WoW Install", name)
                        .on_input(|s| Message::UpdateInstanceField(InstanceField::Name(s)))
                        .padding([8, 12]),
                    iced::widget::checkbox(*like_turtles)
                        .label("I like turtles!")
                        .on_toggle(|b| Message::UpdateInstanceField(InstanceField::LikeTurtles(b))),
                    text("WoW directory").size(13).color(colors.text),
                    row![
                        iced::widget::text_input("/path/to/WoW", wow_dir)
                            .on_input(|s| Message::UpdateInstanceField(InstanceField::WowDir(s)))
                            .width(Length::Fill)
                            .padding([8, 12]),
                        {
                            let c2 = c;
                            tip(
                                button(text("Browse").size(12))
                                    .on_press(Message::PickWowDirectory)
                                    .padding([8, 12])
                                    .style(move |_t, s| match s {
                                        button::Status::Hovered => {
                                            theme::tab_button_hovered_style(&c2)
                                        }
                                        _ => theme::tab_button_style(&c2),
                                    }),
                                "Pick the WoW installation folder",
                                iced::widget::tooltip::Position::Top,
                                colors,
                            )
                        },
                    ]
                    .spacing(6),
                    iced::widget::checkbox(*clear_wdb)
                        .label("Auto-clear WDB cache on launch")
                        .on_toggle(|b| Message::UpdateInstanceField(InstanceField::ClearWdb(b))),
                    text("Launch method").size(13).color(colors.text),
                    row(method_buttons).spacing(4),
                    launch_fields,
                    Space::new().height(4),
                    {
                        let mut footer_items: Vec<Element<Message>> = Vec::new();
                        if can_remove {
                            let c2 = c;
                            let remove_el: Element<Message> = if is_active_profile {
                                let dimmed_btn = button(text("Remove").size(13))
                                    .padding([6, 14])
                                    .style(move |_theme, _status| button::Style {
                                        background: None,
                                        text_color: iced::Color::from_rgba(1.0, 0.4, 0.4, 0.35),
                                        border: iced::Border {
                                            color: iced::Color::from_rgba(1.0, 0.4, 0.4, 0.25),
                                            width: 1.0,
                                            radius: 4.0.into(),
                                        },
                                        shadow: iced::Shadow::default(),
                                        snap: true,
                                    });
                                iced::widget::tooltip(
                                    dimmed_btn,
                                    container(
                                        text("Cannot remove the active instance")
                                            .size(13)
                                            .color(c2.text),
                                    )
                                    .padding([4, 8])
                                    .style(move |_theme| theme::tooltip_style(&c2)),
                                    iced::widget::tooltip::Position::Top,
                                )
                                .into()
                            } else {
                                let rm_btn = button(text("Remove").size(13).color(c.bad))
                                    .on_press(Message::RemoveProfile(remove_id))
                                    .padding([6, 14])
                                    .style(move |_theme, _status| {
                                        let mut s = theme::tab_button_style(&c2);
                                        s.border.color = c2.bad;
                                        s
                                    });
                                tip(
                                    rm_btn,
                                    "Delete this instance profile",
                                    iced::widget::tooltip::Position::Top,
                                    colors,
                                )
                            };
                            footer_items.push(remove_el);
                        }
                        footer_items.push(Space::new().width(Length::Fill).into());
                        footer_items.push(
                            button(text("Cancel").size(13))
                                .on_press(Message::CloseDialog)
                                .padding([6, 14])
                                .style(move |_theme, status| match status {
                                    button::Status::Hovered => theme::tab_button_hovered_style(&c),
                                    _ => theme::tab_button_style(&c),
                                })
                                .into(),
                        );
                        footer_items.push(tip(
                            button(text("Save").size(13))
                                .on_press(Message::SaveInstanceSettings)
                                .padding([6, 14])
                                .style(move |_theme, _status| theme::tab_button_active_style(&c)),
                            "Save instance settings",
                            iced::widget::tooltip::Position::Top,
                            colors,
                        ));
                        row(footer_items).spacing(8)
                    },
                ]
                .spacing(8)
                .into()
            }
            Dialog::DxvkConfig {
                config,
                show_preview,
            } => panels::dxvk_config::view(
                config,
                &self.wow_dir,
                *show_preview,
                &self.dxvk_preview_content,
                colors,
            ),
            Dialog::RadioSettings {
                auto_connect,
                auto_play,
                buffer_size,
                custom_buffer,
                persist_volume,
            } => panels::radio::view(
                auto_connect,
                auto_play,
                buffer_size,
                *custom_buffer,
                persist_volume,
                colors,
            ),
            Dialog::AvWarning { url, mode } => av_false_positive_warning(url, mode, colors),
            Dialog::AddonConflict {
                url,
                mode,
                conflicts,
            } => addon_conflict(url, mode, conflicts, colors),
        }
    }

    pub fn view_topbar(&self, colors: &ThemeColors) -> Element<'_, Message> {
        let c = *colors;

        let title = text("Wuddle")
            .size(44)
            .font(LIFECRAFT)
            .color(colors.title)
            .line_height(1.0);

        let view_tabs = row![
            self.view_tab_button(Tab::Home, colors),
            self.view_tab_button(Tab::Mods, colors),
            self.view_tab_button(Tab::Addons, colors),
            self.view_tab_button(Tab::Tweaks, colors),
        ]
        .spacing(8);

        let action_tabs = row![
            self.view_tab_button(Tab::Options, colors),
            self.view_tab_button(Tab::Logs, colors),
            self.view_tab_button(Tab::About, colors),
        ]
        .spacing(8);

        // Busy spinner \u{2014} always reserve its space so the left section never
        // changes width. Render an invisible placeholder when not spinning.
        let spinner_el: Element<Message> = if self.is_busy() {
            let tick = self.spinner_tick;
            let primary = colors.primary;
            canvas(SpinnerCanvas {
                tick,
                color: primary,
            })
            .width(26)
            .height(26)
            .into()
        } else {
            Space::new().width(26).height(26).into()
        };

        // Left section: title + spinner placeholder (fixed width, never shifts)
        let left_section = row![title, spinner_el]
            .spacing(12)
            .align_y(iced::Alignment::Center);

        // Right section: optional profile picker + action buttons
        let mut right_items: Vec<Element<Message>> = Vec::new();

        if self.profiles.len() > 1 {
            let display_labels: Vec<String> = self
                .profiles
                .iter()
                .map(|p| {
                    let dupes = self.profiles.iter().filter(|q| q.name == p.name).count();
                    if dupes > 1 {
                        format!("{} ({})", p.name, p.id)
                    } else {
                        p.name.clone()
                    }
                })
                .collect();

            let active_display = self
                .profiles
                .iter()
                .find(|p| p.id == self.active_profile_id)
                .map(|p| {
                    let dupes = self.profiles.iter().filter(|q| q.name == p.name).count();
                    if dupes > 1 {
                        format!("{} ({})", p.name, p.id)
                    } else {
                        p.name.clone()
                    }
                })
                .unwrap_or_else(|| "Default".to_string());

            let profile_picker: Element<Message> =
                iced::widget::pick_list(display_labels, Some(active_display), {
                    let profiles = self.profiles.clone();
                    move |display: String| {
                        let profile = profiles.iter().find(|p| {
                            let dupes = profiles.iter().filter(|q| q.name == p.name).count();
                            let label = if dupes > 1 {
                                format!("{} ({})", p.name, p.id)
                            } else {
                                p.name.clone()
                            };
                            label == display
                        });
                        Message::SwitchProfile(profile.map(|p| p.id.clone()).unwrap_or_default())
                    }
                })
                .text_size(13)
                .into();

            let divider = rule::vertical(1).style(move |_theme| theme::divider_style(&c));
            right_items.push(profile_picker);
            right_items.push(divider.into());
        }

        right_items.push(action_tabs.into());
        let right_section = row(right_items)
            .spacing(10)
            .align_y(iced::Alignment::Center);

        // Use a Stack so the tabs float in their own centered layer,
        // completely independent of the left/right content widths.
        // Both layers share the same height and vertical alignment so
        // tabs and buttons land on the same baseline.
        const BAR_H: f32 = 58.0;

        // Layer 0 (bottom): logo on left, controls on right
        let sides = container(
            row![
                left_section,
                Space::new().width(Length::Fill),
                right_section,
            ]
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(BAR_H)
        .align_y(iced::Alignment::Center)
        .padding([0, 12]);

        // Layer 1 (top): tabs centered, same height and padding
        let center = container(view_tabs)
            .width(Length::Fill)
            .height(BAR_H)
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center)
            .padding([0, 0]);

        let bar = stack![sides, center].width(Length::Fill).height(BAR_H);

        container(bar)
            .width(Length::Fill)
            .style(move |_theme| theme::topbar_style(&c))
            .into()
    }

    pub fn view_tab_button(&self, tab: Tab, colors: &ThemeColors) -> Element<'_, Message> {
        let is_active = self.active_tab == tab;
        let c = *colors;

        let is_icon = matches!(tab, Tab::Options | Tab::Logs);
        // About uses its Unicode \u{24d8} glyph \u{2014} compact width like SVG icon tabs
        let is_unicode_icon = tab == Tab::About;

        let content: Element<Message> = if is_icon {
            let icon_color = if is_active { c.primary_text } else { c.text };
            container(
                iced::widget::svg(tab_icon_svg(tab))
                    .width(17)
                    .height(17)
                    .style(move |_t, _s| iced::widget::svg::Style {
                        color: Some(icon_color),
                    }),
            )
            .width(Length::Fill)
            .center_x(Length::Fill)
            .into()
        } else if is_unicode_icon {
            let icon_color = if is_active { c.primary_text } else { c.text };
            container(
                text(tab.icon_label())
                    .size(17)
                    .color(icon_color)
                    .line_height(1.0),
            )
            .center_x(Length::Fill)
            .into()
        } else {
            let lbl = self.tab_label(tab);
            container(text(lbl).size(14))
                .width(Length::Fill)
                .center_x(Length::Fill)
                .into()
        };

        let btn = button(content)
            .on_press(Message::SetTab(tab))
            .padding([7, 0])
            .width(if is_icon || is_unicode_icon {
                Length::Fixed(32.0)
            } else {
                Length::Fixed(114.0)
            });

        let styled_btn: Element<Message> = if is_active {
            btn.style(move |_theme, _status| theme::tab_button_active_style(&c))
                .into()
        } else {
            btn.style(move |_theme, status| match status {
                button::Status::Hovered => theme::tab_button_hovered_style(&c),
                button::Status::Pressed => theme::tab_button_active_style(&c),
                _ => theme::tab_button_style(&c),
            })
            .into()
        };

        // Wrap icon tabs in a tooltip showing the tab name
        if is_icon || tab == Tab::About {
            iced::widget::tooltip(
                styled_btn,
                container(text(tab.tooltip()).size(13).color(c.text))
                    .padding([3, 8])
                    .style(move |_theme| theme::tooltip_style(&c)),
                iced::widget::tooltip::Position::Bottom,
            )
            .into()
        } else {
            styled_btn
        }
    }

    pub fn view_panel(&self, colors: &ThemeColors) -> Element<'_, Message> {
        let content: Element<Message> = match self.active_tab {
            Tab::Home => panels::home::view(self, colors),
            Tab::Mods => panels::projects::view(self, colors, "Mods"),
            Tab::Addons => panels::projects::view(self, colors, "Addons"),
            Tab::Tweaks => panels::tweaks::view(self, colors),
            Tab::Options => panels::options::view(self, colors),
            Tab::Logs => panels::logs::view(self, colors),
            Tab::About => panels::about::view(self, colors),
        };

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([8, 8])
            .into()
    }

    pub fn view_footer(&self, colors: &ThemeColors) -> Element<'_, Message> {
        let c = *colors;

        let hint: Element<Message> = if self.wow_dir.is_empty() {
            text("No WoW directory set. Go to Options to configure.")
                .size(12)
                .color(colors.warn)
                .into()
        } else {
            let active = self
                .profiles
                .iter()
                .find(|p| p.id == self.active_profile_id)
                .cloned()
                .unwrap_or_default();
            let (mode_label, tooltip_detail) = match active.launch_method.as_str() {
                "lutris" => {
                    let target = if active.lutris_target.trim().is_empty() {
                        "(no target set)".to_string()
                    } else {
                        active.lutris_target.clone()
                    };
                    (
                        "Launch Mode: Lutris".to_string(),
                        format!("Target: {}", target),
                    )
                }
                "wine" => {
                    let cmd = if active.wine_command.trim().is_empty() {
                        "wine".to_string()
                    } else {
                        active.wine_command.clone()
                    };
                    ("Launch Mode: Wine".to_string(), format!("Command: {}", cmd))
                }
                "custom" => {
                    let cmd = if active.custom_command.trim().is_empty() {
                        "(no command set)".to_string()
                    } else {
                        active.custom_command.clone()
                    };
                    (
                        "Launch Mode: Custom".to_string(),
                        format!("Command: {}", cmd),
                    )
                }
                _ => (
                    "Launch Mode: Auto".to_string(),
                    "Launches VanillaFixes.exe if present, otherwise Wow.exe".to_string(),
                ),
            };
            let tooltip_content =
                container(text(tooltip_detail).size(13).color(colors.text)).padding([6, 10]);
            iced::widget::tooltip(
                text(mode_label).size(12).color(colors.muted),
                tooltip_content,
                iced::widget::tooltip::Position::Top,
            )
            .style(move |_t| theme::tooltip_style(&c))
            .into()
        };

        let play_btn = button(container(text("PLAY").size(16)).center_x(Length::Shrink))
            .on_press(Message::LaunchGame)
            .padding([10, 36])
            .width(108)
            .style(move |_theme, status| match status {
                button::Status::Hovered => theme::play_button_hovered_style(&c),
                _ => theme::play_button_style(&c),
            });
        let bar = row![hint, Space::new().width(Length::Fill), play_btn,]
            .spacing(12)
            .padding([10, 12])
            .align_y(iced::Alignment::Center);

        container(bar)
            .width(Length::Fill)
            .style(move |_theme| theme::footer_style(&c))
            .into()
    }
}
