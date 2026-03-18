mod anchored_overlay;
mod panels;
mod service;
mod settings;
#[allow(dead_code)]
mod theme;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use iced::widget::{button, canvas, column, container, row, rule, stack, text, Space};
use iced::{Element, Font, Length, Subscription, Task, Theme};
use service::{PlanRow, RepoRow};
use theme::{ThemeColors, WuddleTheme};

const LIFECRAFT: Font = Font::with_name("LifeCraft");
const FRIZ: Font = Font::with_name("Friz Quadrata Std");

fn main() -> iced::Result {
    // Read settings early so we can set the default font
    let saved = settings::load_settings();
    let default_font = if saved.opt_friz_font { FRIZ } else { Font::DEFAULT };

    iced::application(App::new, App::update, App::view)
        .title("Wuddle")
        .theme(App::theme)
        .subscription(App::subscription)
        .font(include_bytes!("../assets/fonts/LifeCraft_Font.ttf"))
        .font(include_bytes!("../assets/fonts/FrizQuadrataStd-Regular.otf"))
        .default_font(default_font)
        .window_size((1100.0, 850.0))
        .run()
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Home,
    Mods,
    Addons,
    Tweaks,
    Options,
    Logs,
    About,
}

impl Tab {
    fn icon_label(self) -> &'static str {
        match self {
            Tab::Options => "\u{2699}",  // ⚙
            Tab::Logs => "\u{2630}",    // ☰
            Tab::About => "\u{24D8}",   // ⓘ
            _ => "",
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Mods => "Mods",
            Tab::Addons => "Addons",
            Tab::Tweaks => "Tweaks",
            Tab::Options => "Options",
            Tab::Logs => "Logs",
            Tab::About => "About",
        }
    }
}

impl Default for Tab {
    fn default() -> Self {
        Tab::Home
    }
}

// ---------------------------------------------------------------------------
// Filters & log types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Updates,
    Errors,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    #[default]
    Name,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortDir {
    #[default]
    Asc,
    Desc,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFilter {
    #[default]
    All,
    Info,
    Errors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Error,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub level: LogLevel,
    pub text: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TweakId {
    Fov,
    Farclip,
    Frilldistance,
    NameplateDist,
    CameraSkip,
    MaxCameraDist,
    SoundBg,
    SoundChannels,
    Quickloot,
    LargeAddress,
}

#[derive(Debug, Clone)]
pub struct TweakState {
    pub fov: bool,
    pub farclip: bool,
    pub frilldistance: bool,
    pub nameplate_dist: bool,
    pub camera_skip: bool,
    pub max_camera_dist: bool,
    pub sound_bg: bool,
    pub sound_channels: bool,
    pub quickloot: bool,
    pub large_address: bool,
}

impl Default for TweakState {
    fn default() -> Self {
        Self {
            fov: true,
            farclip: true,
            frilldistance: true,
            nameplate_dist: true,
            camera_skip: true,
            max_camera_dist: true,
            sound_bg: true,
            sound_channels: true,
            quickloot: true,
            large_address: true,
        }
    }
}

impl TweakState {
    pub fn set(&mut self, id: TweakId, val: bool) {
        match id {
            TweakId::Fov => self.fov = val,
            TweakId::Farclip => self.farclip = val,
            TweakId::Frilldistance => self.frilldistance = val,
            TweakId::NameplateDist => self.nameplate_dist = val,
            TweakId::CameraSkip => self.camera_skip = val,
            TweakId::MaxCameraDist => self.max_camera_dist = val,
            TweakId::SoundBg => self.sound_bg = val,
            TweakId::SoundChannels => self.sound_channels = val,
            TweakId::Quickloot => self.quickloot = val,
            TweakId::LargeAddress => self.large_address = val,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TweakValues {
    pub fov: f32,
    pub farclip: f32,
    pub frilldistance: f32,
    pub nameplate_dist: f32,
    pub max_camera_dist: f32,
    pub sound_channels: u32,
}

impl Default for TweakValues {
    fn default() -> Self {
        Self {
            fov: 1.925,
            farclip: 1000.0,
            frilldistance: 300.0,
            nameplate_dist: 41.0,
            max_camera_dist: 50.0,
            sound_channels: 64,
        }
    }
}

#[derive(Debug, Clone)]
pub enum InstanceField {
    Name(String),
    WowDir(String),
    LaunchMethod(String),
    LikeTurtles(bool),
    ClearWdb(bool),
    LutrisTarget(String),
    WineCommand(String),
    WineArgs(String),
    CustomCommand(String),
    CustomArgs(String),
}

#[derive(Debug, Clone)]
pub enum Dialog {
    AddRepo { url: String, mode: String },
    RemoveRepo { id: i64, name: String },
    InstanceSettings {
        is_new: bool,
        profile_id: String,
        name: String,
        wow_dir: String,
        launch_method: String,  // "auto", "lutris", "wine", "custom"
        like_turtles: bool,
        clear_wdb: bool,
        lutris_target: String,
        wine_command: String,
        wine_args: String,
        custom_command: String,
        custom_args: String,
    },
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

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
    pub opt_clock12: bool,
    pub opt_friz_font: bool,

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

    // Dialog overlay
    pub dialog: Option<Dialog>,

    // Context menu: which repo's menu is open
    pub open_menu: Option<i64>,

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

    // Tweak values
    pub tweak_values: TweakValues,

    // About
    pub latest_version: Option<String>,
    pub update_message: Option<String>,

    // Auto-check
    pub auto_check_minutes: u32,

    // Profiles/instances
    pub profiles: Vec<settings::ProfileConfig>,

    // Spinner animation tick (0..36, one full rotation = 36 ticks @ 80ms each)
    pub spinner_tick: usize,
}

impl Default for WuddleTheme {
    fn default() -> Self {
        WuddleTheme::Cata
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    SetTab(Tab),
    SetTheme(WuddleTheme),

    // Projects
    SetFilter(Filter),
    SetProjectSearch(String),
    ToggleSort(SortKey),

    // Options toggles
    ToggleAutoCheck(bool),
    ToggleDesktopNotify(bool),
    ToggleSymlinks(bool),
    ToggleClock12(bool),
    ToggleFrizFont(bool),
    SetGithubTokenInput(String),

    // Tweaks
    ToggleTweak(TweakId, bool),

    // Logs
    SetLogFilter(LogFilter),
    SetLogSearch(String),
    ToggleLogWrap(bool),
    ToggleLogAutoScroll(bool),
    ClearLogs,

    // Dialogs
    OpenDialog(Dialog),
    CloseDialog,

    // Context menu
    ToggleMenu(i64),
    CloseMenu,

    // Engine data (Phase 2)
    ReposLoaded(Result<Vec<RepoRow>, String>),
    PlansLoaded(Result<Vec<PlanRow>, String>),
    SettingsLoaded(settings::AppSettings),

    // Operations (Phase 3)
    CheckUpdates,
    CheckUpdatesResult(Result<Vec<PlanRow>, String>),
    AddRepoSubmit,
    AddRepoResult(Result<i64, String>),
    RemoveRepoConfirm(i64),
    RemoveRepoResult(Result<(), String>),
    ToggleRepoEnabled(i64, bool),
    ToggleRepoEnabledResult(Result<(), String>),
    UpdateAll,
    UpdateAllResult(Result<Vec<PlanRow>, String>),
    UpdateRepo(i64),
    UpdateRepoResult(Result<Option<PlanRow>, String>),
    ReinstallRepo(i64),
    ReinstallRepoResult(Result<PlanRow, String>),
    FetchBranches(i64),
    FetchBranchesResult(Result<(i64, Vec<String>), String>),
    SetRepoBranch(i64, String),
    SetRepoBranchResult(Result<i64, String>),
    RefreshRepos,
    SaveSettings,

    // Shared actions
    OpenUrl(String),
    OpenDirectory(String),
    CopyToClipboard(String),
    LaunchGame,
    LaunchGameResult(Result<String, String>),

    // GitHub token
    SaveGithubToken,
    SaveGithubTokenResult(Result<(), String>),
    ForgetGithubToken,
    ForgetGithubTokenResult(Result<(), String>),

    // Instance settings
    SaveInstanceSettings,
    UpdateInstanceField(InstanceField),
    SwitchProfile(String),
    RemoveProfile(String),
    RemoveProfileResult(Result<String, String>),

    // File dialog
    PickWowDirectory,
    WowDirectoryPicked(Option<PathBuf>),

    // Tweaks
    SetTweakFov(f32),
    SetTweakFarclip(f32),
    SetTweakFrilldistance(f32),
    SetTweakNameplateDist(f32),
    SetTweakMaxCameraDist(String),
    SetTweakSoundChannels(String),
    ReadTweaks,
    ReadTweaksResult(Result<TweakValues, String>),
    ApplyTweaks,
    ApplyTweaksResult(Result<String, String>),
    RestoreTweaks,
    RestoreTweaksResult(Result<String, String>),
    ResetTweaksToDefault,

    // About
    CheckSelfUpdate,
    CheckSelfUpdateResult(Result<String, String>),

    // Auto-check tick
    AutoCheckTick,

    // Spinner animation
    SpinnerTick,
}

impl App {
    fn new() -> (Self, Task<Message>) {
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
            opt_clock12: false,
            opt_friz_font: false,
            github_token_input: String::new(),
            tweaks: TweakState::default(),
            log_lines: vec![
                LogLine { level: LogLevel::Info, text: "Wuddle v3.0.0-alpha.1 started".into(), timestamp: chrono_now() },
                LogLine { level: LogLevel::Info, text: "Ready.".into(), timestamp: chrono_now() },
            ],
            log_filter: LogFilter::default(),
            log_search: String::new(),
            log_wrap: false,
            log_autoscroll: true,
            dialog: None,
            open_menu: None,
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
            tweak_values: TweakValues::default(),
            latest_version: None,
            update_message: None,
            auto_check_minutes: 15,
            profiles: vec![settings::ProfileConfig::default()],
            spinner_tick: 0,
        };

        // Sync GitHub token from keychain/env at startup
        service::sync_github_token();

        // Load settings synchronously (fast, local JSON), then kick off async repo load
        let task = Task::perform(
            async { settings::load_settings() },
            Message::SettingsLoaded,
        );

        (app, task)
    }

    fn log(&mut self, level: LogLevel, msg: &str) {
        self.log_lines.push(LogLine {
            level,
            text: msg.to_string(),
            timestamp: chrono_now(),
        });
    }

    fn refresh_repos_task(&self) -> Task<Message> {
        let db = self.db_path.clone();
        let wow = if self.wow_dir.is_empty() {
            None
        } else {
            Some(self.wow_dir.clone())
        };
        Task::perform(service::list_repos(db, wow), Message::ReposLoaded)
    }

    fn check_updates_task(&self) -> Task<Message> {
        let db = self.db_path.clone();
        let wow = if self.wow_dir.is_empty() {
            None
        } else {
            Some(self.wow_dir.clone())
        };
        Task::perform(
            service::check_updates(db, wow, wuddle_engine::CheckMode::Force),
            Message::CheckUpdatesResult,
        )
    }

    fn save_settings(&self) {
        let s = settings::AppSettings {
            wow_dir: self.wow_dir.clone(),
            theme: self.wuddle_theme.key().to_string(),
            active_profile_id: self.active_profile_id.clone(),
            opt_auto_check: self.opt_auto_check,
            opt_desktop_notify: self.opt_desktop_notify,
            opt_symlinks: self.opt_symlinks,
            opt_clock12: self.opt_clock12,
            opt_friz_font: self.opt_friz_font,
            log_wrap: self.log_wrap,
            log_autoscroll: self.log_autoscroll,
            auto_check_minutes: self.auto_check_minutes,
            profiles: self.profiles.clone(),
        };
        let _ = settings::save_settings(&s);
    }

    fn theme(&self) -> Theme {
        self.wuddle_theme.to_iced_theme()
    }

    fn is_busy(&self) -> bool {
        self.loading || self.checking_updates || self.updating_all || !self.updating_repo_ids.is_empty()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = Vec::new();

        if self.opt_auto_check {
            let mins = self.auto_check_minutes.max(1) as u64;
            subs.push(
                iced::time::every(std::time::Duration::from_secs(mins * 60))
                    .map(|_| Message::AutoCheckTick),
            );
        }

        if self.is_busy() {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(80))
                    .map(|_| Message::SpinnerTick),
            );
        }

        Subscription::batch(subs)
    }

    fn colors(&self) -> ThemeColors {
        let mut c = self.wuddle_theme.colors();
        c.body_font = self.body_font();
        c
    }

    pub fn body_font(&self) -> Font {
        if self.opt_friz_font { FRIZ } else { Font::DEFAULT }
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
        self.plans.iter().filter(|p| {
            p.has_update && self.repos.iter().any(|r| r.id == p.repo_id && is_mod(r))
        }).count()
    }

    pub fn addon_update_count(&self) -> usize {
        self.plans.iter().filter(|p| {
            p.has_update && self.repos.iter().any(|r| r.id == p.repo_id && !is_mod(r))
        }).count()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetTab(tab) => self.active_tab = tab,
            Message::SetTheme(theme) => {
                self.wuddle_theme = theme;
                self.save_settings();
            }

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

            // Options
            Message::ToggleAutoCheck(b) => { self.opt_auto_check = b; self.save_settings(); }
            Message::ToggleDesktopNotify(b) => { self.opt_desktop_notify = b; self.save_settings(); }
            Message::ToggleSymlinks(b) => { self.opt_symlinks = b; self.save_settings(); }
            Message::ToggleClock12(b) => { self.opt_clock12 = b; self.save_settings(); }
            Message::ToggleFrizFont(b) => {
                self.opt_friz_font = b;
                self.save_settings();
                self.log(LogLevel::Info, "Friz Quadrata font setting saved. Restart Wuddle to apply.");
            }
            Message::SetGithubTokenInput(s) => self.github_token_input = s,

            // Tweaks
            Message::ToggleTweak(id, val) => self.tweaks.set(id, val),

            // Logs
            Message::SetLogFilter(f) => self.log_filter = f,
            Message::SetLogSearch(s) => self.log_search = s,
            Message::ToggleLogWrap(b) => { self.log_wrap = b; self.save_settings(); }
            Message::ToggleLogAutoScroll(b) => { self.log_autoscroll = b; self.save_settings(); }
            Message::ClearLogs => self.log_lines.clear(),

            // Dialogs
            Message::OpenDialog(d) => { self.open_menu = None; self.dialog = Some(d); }
            Message::CloseDialog => self.dialog = None,

            // Context menu
            Message::ToggleMenu(id) => {
                if self.open_menu == Some(id) {
                    self.open_menu = None;
                } else {
                    self.open_menu = Some(id);
                }
            }
            Message::CloseMenu => self.open_menu = None,

            // --- Phase 2: Data loading ---
            Message::SettingsLoaded(s) => {
                self.wuddle_theme = WuddleTheme::from_key(&s.theme);
                self.active_profile_id = s.active_profile_id.clone();
                self.opt_auto_check = s.opt_auto_check;
                self.opt_desktop_notify = s.opt_desktop_notify;
                self.opt_symlinks = s.opt_symlinks;
                self.opt_clock12 = s.opt_clock12;
                self.opt_friz_font = s.opt_friz_font;
                self.log_wrap = s.log_wrap;
                self.log_autoscroll = s.log_autoscroll;
                self.auto_check_minutes = s.auto_check_minutes.max(1);
                self.profiles = if s.profiles.is_empty() {
                    vec![settings::ProfileConfig::default()]
                } else {
                    s.profiles
                };
                // Set wow_dir from active profile (or legacy setting)
                let active_profile = self.profiles.iter().find(|p| p.id == self.active_profile_id);
                self.wow_dir = active_profile
                    .map(|p| p.wow_dir.clone())
                    .unwrap_or(s.wow_dir);
                self.db_path = settings::profile_db_path(&self.active_profile_id).ok();
                self.log(LogLevel::Info, &format!("Profile: {}", self.active_profile_id));
                return self.refresh_repos_task();
            }
            Message::ReposLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(repos) => {
                        let count = repos.len();
                        let mod_count = repos.iter().filter(|r| is_mod(r)).count();
                        let addon_count = count - mod_count;
                        self.repos = repos;
                        self.log(LogLevel::Info, &format!("Loaded {} repos ({} mods, {} addons).", count, mod_count, addon_count));
                        // Fetch branches for addon_git repos that aren't cached yet
                        let fetch_tasks: Vec<Task<Message>> = self
                            .repos
                            .iter()
                            .filter(|r| r.mode == "addon_git" && !self.branches.contains_key(&r.id))
                            .map(|r| {
                                let db = self.db_path.clone();
                                Task::perform(
                                    service::list_repo_branches(db, r.id),
                                    Message::FetchBranchesResult,
                                )
                            })
                            .collect();
                        if !fetch_tasks.is_empty() {
                            return Task::batch(fetch_tasks);
                        }
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.log(LogLevel::Error, &format!("Failed to load repos: {}", e));
                    }
                }
            }
            Message::PlansLoaded(result) => {
                match result {
                    Ok(plans) => self.plans = plans,
                    Err(e) => self.log(LogLevel::Error, &format!("Plans error: {}", e)),
                }
            }

            // --- Phase 3: Operations ---
            Message::CheckUpdates => {
                self.checking_updates = true;
                self.log(LogLevel::Info, "Checking for updates...");
                return self.check_updates_task();
            }
            Message::CheckUpdatesResult(result) => {
                self.checking_updates = false;
                match result {
                    Ok(plans) => {
                        let update_count = plans.iter().filter(|p| p.has_update).count();
                        self.log(LogLevel::Info, &format!("Update check complete. {} updates available.", update_count));
                        self.plans = plans;
                        self.last_checked = Some(chrono_now());
                        self.cached_plans.insert(
                            self.active_profile_id.clone(),
                            (self.plans.clone(), self.last_checked.clone()),
                        );
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.log(LogLevel::Error, &format!("Update check failed: {}", e));
                    }
                }
            }
            Message::AddRepoSubmit => {
                if let Some(Dialog::AddRepo { ref url, ref mode }) = self.dialog {
                    let url = url.clone();
                    let mode = mode.clone();
                    let db = self.db_path.clone();
                    self.dialog = None;
                    self.log(LogLevel::Info, &format!("Adding repo: {}", url));
                    return Task::perform(
                        service::add_repo(db, url, mode),
                        Message::AddRepoResult,
                    );
                }
            }
            Message::AddRepoResult(result) => {
                match result {
                    Ok(id) => {
                        self.log(LogLevel::Info, &format!("Repo added (id={}).", id));
                        return self.refresh_repos_task();
                    }
                    Err(e) => {
                        self.log(LogLevel::Error, &format!("Add repo failed: {}", e));
                        self.error = Some(e);
                    }
                }
            }
            Message::RemoveRepoConfirm(id) => {
                let db = self.db_path.clone();
                let wow = if self.wow_dir.is_empty() { None } else { Some(self.wow_dir.clone()) };
                self.dialog = None;
                self.log(LogLevel::Info, &format!("Removing repo id={}...", id));
                return Task::perform(
                    service::remove_repo(db, id, wow, false),
                    Message::RemoveRepoResult,
                );
            }
            Message::RemoveRepoResult(result) => {
                match result {
                    Ok(()) => {
                        self.log(LogLevel::Info, "Repo removed.");
                        return self.refresh_repos_task();
                    }
                    Err(e) => self.log(LogLevel::Error, &format!("Remove failed: {}", e)),
                }
            }
            Message::ToggleRepoEnabled(id, enabled) => {
                let db = self.db_path.clone();
                return Task::perform(
                    service::set_repo_enabled(db, id, enabled),
                    Message::ToggleRepoEnabledResult,
                );
            }
            Message::ToggleRepoEnabledResult(result) => {
                match result {
                    Ok(()) => return self.refresh_repos_task(),
                    Err(e) => self.log(LogLevel::Error, &format!("Enable/disable failed: {}", e)),
                }
            }
            Message::UpdateAll => {
                if self.wow_dir.is_empty() {
                    self.log(LogLevel::Error, "Set a WoW directory in Options first.");
                } else {
                    self.updating_all = true;
                    self.log(LogLevel::Info, "Updating all repos...");
                    let db = self.db_path.clone();
                    let wow = self.wow_dir.clone();
                    let opts = self.install_options();
                    return Task::perform(
                        service::update_all(db, wow, opts),
                        Message::UpdateAllResult,
                    );
                }
            }
            Message::UpdateAllResult(result) => {
                self.updating_all = false;
                match result {
                    Ok(plans) => {
                        let applied = plans.iter().filter(|p| !p.has_update).count();
                        self.log(LogLevel::Info, &format!("Updated {} repos.", applied));
                        self.plans = plans;
                        self.cached_plans.insert(
                            self.active_profile_id.clone(),
                            (self.plans.clone(), self.last_checked.clone()),
                        );
                        return self.refresh_repos_task();
                    }
                    Err(e) => self.log(LogLevel::Error, &format!("Update all failed: {}", e)),
                }
            }
            Message::UpdateRepo(id) => {
                self.open_menu = None;
                if self.wow_dir.is_empty() {
                    self.log(LogLevel::Error, "Set a WoW directory in Options first.");
                } else {
                    self.updating_repo_ids.insert(id);
                    let db = self.db_path.clone();
                    let wow = self.wow_dir.clone();
                    let opts = self.install_options();
                    return Task::perform(
                        service::update_repo(db, id, wow, opts),
                        Message::UpdateRepoResult,
                    );
                }
            }
            Message::UpdateRepoResult(result) => {
                match result {
                    Ok(Some(plan)) => {
                        self.updating_repo_ids.remove(&plan.repo_id);
                        self.log(LogLevel::Info, &format!("Updated {}/{}.", plan.owner, plan.name));
                    }
                    Ok(None) => self.log(LogLevel::Info, "Repo already up to date."),
                    Err(e) => self.log(LogLevel::Error, &format!("Update failed: {}", e)),
                }
                return self.refresh_repos_task();
            }
            Message::ReinstallRepo(id) => {
                self.open_menu = None;
                if self.wow_dir.is_empty() {
                    self.log(LogLevel::Error, "Set a WoW directory in Options first.");
                } else {
                    self.dialog = None;
                    self.log(LogLevel::Info, &format!("Reinstalling repo id={}...", id));
                    let db = self.db_path.clone();
                    let wow = self.wow_dir.clone();
                    let opts = self.install_options();
                    return Task::perform(
                        service::reinstall_repo(db, id, wow, opts),
                        Message::ReinstallRepoResult,
                    );
                }
            }
            Message::ReinstallRepoResult(result) => {
                match result {
                    Ok(plan) => {
                        self.log(LogLevel::Info, &format!("Reinstalled {}/{}.", plan.owner, plan.name));
                        return self.refresh_repos_task();
                    }
                    Err(e) => self.log(LogLevel::Error, &format!("Reinstall failed: {}", e)),
                }
            }
            Message::FetchBranches(repo_id) => {
                let db = self.db_path.clone();
                return Task::perform(
                    service::list_repo_branches(db, repo_id),
                    Message::FetchBranchesResult,
                );
            }
            Message::FetchBranchesResult(result) => {
                match result {
                    Ok((repo_id, branch_list)) => {
                        self.branches.insert(repo_id, branch_list);
                    }
                    Err(e) => self.log(LogLevel::Error, &format!("Failed to fetch branches: {}", e)),
                }
            }
            Message::SetRepoBranch(repo_id, branch) => {
                let db = self.db_path.clone();
                self.log(LogLevel::Info, &format!("Setting branch to '{}' for repo id={}...", branch, repo_id));
                return Task::perform(
                    service::set_repo_branch(db, repo_id, branch),
                    Message::SetRepoBranchResult,
                );
            }
            Message::SetRepoBranchResult(result) => {
                match result {
                    Ok(repo_id) => {
                        self.log(LogLevel::Info, "Branch updated. Refreshing repos...");
                        // Clear cached branches so they get re-fetched
                        self.branches.remove(&repo_id);
                        return self.refresh_repos_task();
                    }
                    Err(e) => self.log(LogLevel::Error, &format!("Set branch failed: {}", e)),
                }
            }
            Message::RefreshRepos => {
                self.loading = true;
                return self.refresh_repos_task();
            }
            Message::SaveSettings => {
                self.save_settings();
            }

            // --- Shared actions ---
            Message::OpenUrl(url) => {
                if let Err(e) = open::that(&url) {
                    self.log(LogLevel::Error, &format!("Failed to open URL: {}", e));
                }
            }
            Message::OpenDirectory(path) => {
                if let Err(e) = open::that(&path) {
                    self.log(LogLevel::Error, &format!("Failed to open directory: {}", e));
                }
            }
            Message::CopyToClipboard(text_val) => {
                match copy_to_clipboard(&text_val) {
                    Ok(()) => self.log(LogLevel::Info, "Copied to clipboard."),
                    Err(e) => self.log(LogLevel::Error, &format!("Clipboard error: {}", e)),
                }
            }
            Message::LaunchGame => {
                if self.wow_dir.is_empty() {
                    self.log(LogLevel::Error, "Set a WoW directory in Options first.");
                } else {
                    self.log(LogLevel::Info, "Launching game...");
                    let wow = self.wow_dir.clone();
                    return Task::perform(
                        service::launch_game(wow),
                        Message::LaunchGameResult,
                    );
                }
            }
            Message::LaunchGameResult(result) => {
                match result {
                    Ok(msg) => self.log(LogLevel::Info, &msg),
                    Err(e) => self.log(LogLevel::Error, &format!("Launch failed: {}", e)),
                }
            }

            // --- GitHub token ---
            Message::SaveGithubToken => {
                let token = self.github_token_input.clone();
                if token.trim().is_empty() {
                    self.log(LogLevel::Error, "Token is empty.");
                } else {
                    self.log(LogLevel::Info, "Saving GitHub token...");
                    return Task::perform(
                        service::save_github_token(token),
                        Message::SaveGithubTokenResult,
                    );
                }
            }
            Message::SaveGithubTokenResult(result) => {
                match result {
                    Ok(()) => {
                        self.github_token_input.clear();
                        self.log(LogLevel::Info, "GitHub token saved.");
                    }
                    Err(e) => self.log(LogLevel::Error, &format!("Save token failed: {}", e)),
                }
            }
            Message::ForgetGithubToken => {
                self.log(LogLevel::Info, "Clearing GitHub token...");
                return Task::perform(
                    service::clear_github_token(),
                    Message::ForgetGithubTokenResult,
                );
            }
            Message::ForgetGithubTokenResult(result) => {
                match result {
                    Ok(()) => self.log(LogLevel::Info, "GitHub token cleared."),
                    Err(e) => self.log(LogLevel::Error, &format!("Clear token failed: {}", e)),
                }
            }

            // --- Instance settings ---
            Message::UpdateInstanceField(field) => {
                if let Some(Dialog::InstanceSettings {
                    ref mut name, ref mut wow_dir, ref mut launch_method,
                    ref mut like_turtles, ref mut clear_wdb,
                    ref mut lutris_target, ref mut wine_command, ref mut wine_args,
                    ref mut custom_command, ref mut custom_args, ..
                }) = self.dialog {
                    match field {
                        InstanceField::Name(v) => *name = v,
                        InstanceField::WowDir(v) => *wow_dir = v,
                        InstanceField::LaunchMethod(v) => *launch_method = v,
                        InstanceField::LikeTurtles(v) => *like_turtles = v,
                        InstanceField::ClearWdb(v) => *clear_wdb = v,
                        InstanceField::LutrisTarget(v) => *lutris_target = v,
                        InstanceField::WineCommand(v) => *wine_command = v,
                        InstanceField::WineArgs(v) => *wine_args = v,
                        InstanceField::CustomCommand(v) => *custom_command = v,
                        InstanceField::CustomArgs(v) => *custom_args = v,
                    }
                }
            }
            Message::SaveInstanceSettings => {
                if let Some(Dialog::InstanceSettings {
                    is_new, profile_id: dialog_profile_id, name, wow_dir, launch_method,
                    like_turtles, clear_wdb,
                    lutris_target, wine_command, wine_args,
                    custom_command, custom_args,
                }) = self.dialog.take() {
                    let profile_name = if name.trim().is_empty() { String::from("Default") } else { name.trim().to_string() };
                    let dir = wow_dir.trim().to_string();
                    let profile_id = if is_new {
                        // Generate a new ID from the name
                        profile_name.to_lowercase().replace(' ', "-")
                    } else if !dialog_profile_id.is_empty() {
                        dialog_profile_id
                    } else {
                        // Fallback: find existing profile ID or use active
                        self.profiles.iter()
                            .find(|p| p.name == profile_name)
                            .map(|p| p.id.clone())
                            .unwrap_or_else(|| self.active_profile_id.clone())
                    };

                    let config = settings::ProfileConfig {
                        id: profile_id.clone(),
                        name: profile_name.clone(),
                        wow_dir: dir.clone(),
                        launch_method,
                        like_turtles,
                        clear_wdb,
                        lutris_target,
                        wine_command,
                        wine_args,
                        custom_command,
                        custom_args,
                        working_dir: String::new(),
                        env_text: String::new(),
                    };

                    // Update or add profile
                    if let Some(existing) = self.profiles.iter_mut().find(|p| p.id == profile_id) {
                        *existing = config;
                    } else {
                        self.profiles.push(config);
                    }

                    // Update active profile state
                    if profile_id == self.active_profile_id || is_new {
                        self.wow_dir = dir.clone(); // always update, even if empty
                        if is_new {
                            self.active_profile_id = profile_id.clone();
                            self.db_path = settings::profile_db_path(&profile_id).ok();
                        }
                        // Clear stale data from previous profile
                        self.repos.clear();
                        self.plans.clear();
                        self.branches.clear();
                        self.last_checked = None;
                        self.loading = true;
                    }

                    self.log(LogLevel::Info, &format!("Instance '{}' saved. WoW dir: {}", profile_name, dir));
                    self.save_settings();
                    return self.refresh_repos_task();
                }
            }

            // --- File dialog ---
            Message::SwitchProfile(profile_id) => {
                if let Some(p) = self.profiles.iter().find(|p| p.id == profile_id) {
                    let pid = p.id.clone();
                    let pname = p.name.clone();
                    let pdir = p.wow_dir.clone();
                    if pid != self.active_profile_id {
                        self.active_profile_id = pid.clone();
                        self.db_path = settings::profile_db_path(&pid).ok();
                        self.wow_dir = pdir;
                        // Restore cached plans for the new profile (or clear if never checked)
                        self.repos.clear();
                        self.branches.clear();
                        if let Some((plans, last_checked)) = self.cached_plans.get(&pid) {
                            self.plans = plans.clone();
                            self.last_checked = last_checked.clone();
                        } else {
                            self.plans.clear();
                            self.last_checked = None;
                        }
                        self.loading = true;
                        self.log(LogLevel::Info, &format!("Switched to profile: {} ({})", pname, pid));
                        self.save_settings();
                        return self.refresh_repos_task();
                    }
                }
            }

            Message::RemoveProfile(profile_id) => {
                if profile_id == self.active_profile_id {
                    self.log(LogLevel::Error, "Cannot remove the active profile.");
                    return Task::none();
                }
                let db = settings::profile_db_path(&profile_id).ok();
                self.profiles.retain(|p| p.id != profile_id);
                self.dialog = None;
                self.log(LogLevel::Info, &format!("Removed profile: {}", profile_id));
                self.save_settings();
                // Delete the profile's SQLite database in the background
                return Task::perform(
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
                );
            }
            Message::RemoveProfileResult(result) => {
                match result {
                    Ok(id) => self.log(LogLevel::Info, &format!("Deleted database for profile: {}", id)),
                    Err(e) => self.log(LogLevel::Error, &format!("Failed to delete profile db: {}", e)),
                }
            }

            // --- File dialog ---
            Message::PickWowDirectory => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Select WoW Directory")
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    Message::WowDirectoryPicked,
                );
            }
            Message::WowDirectoryPicked(opt) => {
                if let Some(path) = opt {
                    let dir = path.to_string_lossy().to_string();
                    self.log(LogLevel::Info, &format!("WoW directory set: {}", dir));
                    // Update dialog if instance settings is open
                    if let Some(Dialog::InstanceSettings { ref mut wow_dir, .. }) = self.dialog {
                        *wow_dir = dir;
                    } else {
                        self.wow_dir = dir;
                        self.save_settings();
                        return self.refresh_repos_task();
                    }
                }
            }

            // --- Tweak value setters ---
            Message::SetTweakFov(v) => self.tweak_values.fov = v,
            Message::SetTweakFarclip(v) => self.tweak_values.farclip = v,
            Message::SetTweakFrilldistance(v) => self.tweak_values.frilldistance = v,
            Message::SetTweakNameplateDist(v) => self.tweak_values.nameplate_dist = v,
            Message::SetTweakMaxCameraDist(s) => {
                if let Ok(v) = s.parse::<f32>() {
                    self.tweak_values.max_camera_dist = v.clamp(10.0, 200.0);
                }
            }
            Message::SetTweakSoundChannels(s) => {
                if let Ok(v) = s.parse::<u32>() {
                    self.tweak_values.sound_channels = v.clamp(1, 999);
                }
            }
            Message::ResetTweaksToDefault => {
                self.tweak_values = TweakValues::default();
                self.log(LogLevel::Info, "Tweak values reset to defaults.");
            }

            // Tweak read/apply/restore are placeholders until tweaks.rs is ported
            Message::ReadTweaks => {
                self.log(LogLevel::Info, "Reading current tweak values... (not yet implemented)");
            }
            Message::ReadTweaksResult(_result) => {}
            Message::ApplyTweaks => {
                self.log(LogLevel::Info, "Applying tweaks... (not yet implemented)");
            }
            Message::ApplyTweaksResult(result) => {
                match result {
                    Ok(msg) => self.log(LogLevel::Info, &msg),
                    Err(e) => self.log(LogLevel::Error, &format!("Apply tweaks failed: {}", e)),
                }
            }
            Message::RestoreTweaks => {
                self.log(LogLevel::Info, "Restoring tweaks... (not yet implemented)");
            }
            Message::RestoreTweaksResult(result) => {
                match result {
                    Ok(msg) => self.log(LogLevel::Info, &msg),
                    Err(e) => self.log(LogLevel::Error, &format!("Restore tweaks failed: {}", e)),
                }
            }

            // --- About ---
            Message::CheckSelfUpdate => {
                self.log(LogLevel::Info, "Checking for Wuddle updates...");
                // Placeholder — will fetch latest release from GitHub API
                self.latest_version = Some("3.0.0-alpha.1".to_string());
                self.update_message = Some("Up to date".to_string());
                self.log(LogLevel::Info, "Version check complete.");
            }
            Message::CheckSelfUpdateResult(result) => {
                match result {
                    Ok(ver) => {
                        self.latest_version = Some(ver);
                        self.update_message = Some("Up to date".to_string());
                    }
                    Err(e) => self.log(LogLevel::Error, &format!("Version check failed: {}", e)),
                }
            }

            // --- Spinner tick ---
            Message::SpinnerTick => {
                self.spinner_tick = (self.spinner_tick + 1) % 36;
            }

            // --- Auto-check tick ---
            Message::AutoCheckTick => {
                if self.opt_auto_check && !self.checking_updates {
                    self.checking_updates = true;
                    self.log(LogLevel::Info, "Auto-checking for updates...");
                    return self.check_updates_task();
                }
            }
        }
        Task::none()
    }

    fn install_options(&self) -> wuddle_engine::InstallOptions {
        wuddle_engine::InstallOptions {
            use_symlinks: self.opt_symlinks,
            set_xattr_comment: true,
            replace_addon_conflicts: false,
            cache_keep_versions: 2,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let colors = self.colors();
        let bg_start = colors.bg_grad_start;
        let bg_mid = colors.bg_grad_mid;
        let bg_end = colors.bg_grad_end;

        let topbar = self.view_topbar(&colors);
        let topbar_border = rule::horizontal(1).style(move |_theme| {
            theme::topbar_rule_style(&colors)
        });
        let body = self.view_panel(&colors);
        let footer = self.view_footer(&colors);

        let main_layout = container(
            column![topbar, topbar_border, body, footer]
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Gradient(
                iced::Gradient::Linear(
                    iced::gradient::Linear::new(iced::Radians(std::f32::consts::PI))
                        .add_stop(0.0, bg_start)
                        .add_stop(0.35, bg_mid)
                        .add_stop(1.0, bg_end),
                ),
            )),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            text_color: None,
            snap: true,
        });

        let main_content: Element<Message> = main_layout.into();

        // Determine which overlays to add
        if self.dialog.is_some() {
            let dialog = self.dialog.as_ref().unwrap();
            let c = colors;
            let (dialog_max_w, dialog_pad) = match dialog {
                Dialog::AddRepo { .. } => (600, 24),
                Dialog::InstanceSettings { .. } => (600, 24),
                _ => (480, 24),
            };
            let dialog_box = container(self.view_dialog(dialog, &c))
                .max_width(dialog_max_w)
                .padding(dialog_pad)
                .style(move |_theme| theme::dialog_style(&c));

            let scrim = iced::widget::mouse_area(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|_theme| theme::scrim_style()),
            )
            .on_press(Message::CloseDialog);

            let centered_dialog = container(dialog_box)
                .center_x(Length::Fill)
                .center_y(Length::Fill);

            stack![main_content, scrim, centered_dialog]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            main_content
        }
    }

    fn view_dialog<'a>(&'a self, dialog: &'a Dialog, colors: &ThemeColors) -> Element<'a, Message> {
        let c = *colors;
        match dialog {
            Dialog::AddRepo { url, mode } => {
                let mode_buttons: Vec<Element<Message>> = ["auto", "addon", "dll", "mixed", "raw"]
                    .iter()
                    .map(|&m| {
                        let c2 = c;
                        let m_str = String::from(m);
                        let is_active = mode == m;
                        let url_c = url.clone();
                        let btn = button(text(m).size(12))
                            .on_press(Message::OpenDialog(Dialog::AddRepo {
                                url: url_c,
                                mode: m_str,
                            }))
                            .padding([4, 10]);
                        if is_active {
                            btn.style(move |_t, _s| theme::tab_button_active_style(&c2)).into()
                        } else {
                            btn.style(move |_t, s| match s {
                                button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                                _ => theme::tab_button_style(&c2),
                            }).into()
                        }
                    })
                    .collect();

                let mode_clone = mode.clone();
                column![
                    row![
                        text("Add Repository").size(18).color(colors.title),
                        Space::new().width(Length::Fill),
                        close_button(&c),
                    ].align_y(iced::Alignment::Center),
                    text("Paste a GitHub, GitLab, or Codeberg URL. Wuddle will auto-detect the install mode unless you override it below.")
                        .size(12)
                        .color(colors.muted),
                    text("Repository URL").size(13).color(colors.text),
                    iced::widget::text_input("https://github.com/owner/repo", url)
                        .on_input(move |s| Message::OpenDialog(Dialog::AddRepo {
                            url: s,
                            mode: mode_clone.clone(),
                        }))
                        .on_submit(Message::AddRepoSubmit)
                        .padding([8, 12]),
                    text("Install mode").size(13).color(colors.text),
                    row(mode_buttons).spacing(4),
                    text("auto = detect from repo structure, addon = Interface/AddOns, dll = single DLL, mixed = DLL + data, raw = copy files directly")
                        .size(11)
                        .color(colors.muted),
                    Space::new().height(8),
                    row![
                        Space::new().width(Length::Fill),
                        button(text("Cancel").size(13))
                            .on_press(Message::CloseDialog)
                            .padding([6, 14])
                            .style(move |_theme, status| match status {
                                button::Status::Hovered => theme::tab_button_hovered_style(&c),
                                _ => theme::tab_button_style(&c),
                            }),
                        button(text("Add Repository").size(13))
                            .on_press(Message::AddRepoSubmit)
                            .padding([6, 14])
                            .style(move |_theme, _status| theme::tab_button_active_style(&c)),
                    ]
                    .spacing(8),
                ]
                .spacing(8)
                .into()
            }
            Dialog::RemoveRepo { id, name } => {
                let rid = *id;
                column![
                    row![
                        text("Remove Repository").size(18).color(colors.title),
                        Space::new().width(Length::Fill),
                        close_button(&c),
                    ].align_y(iced::Alignment::Center),
                    text(format!("Remove \"{}\"? This will untrack it from Wuddle.", name))
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
                        button(text("Remove").size(13).color(c.bad))
                            .on_press(Message::RemoveRepoConfirm(rid))
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
            Dialog::InstanceSettings {
                is_new, profile_id, name, wow_dir, launch_method,
                like_turtles, clear_wdb,
                lutris_target, wine_command, wine_args,
                custom_command, custom_args,
            } => {
                let title_text = if *is_new { "Add Instance" } else { "Instance Settings" };
                let can_remove = !*is_new && *profile_id != self.active_profile_id;
                let remove_id = profile_id.clone();

                let method_buttons: Vec<Element<Message>> = [
                    ("Auto", "auto"), ("Lutris", "lutris"), ("Wine", "wine"), ("Custom", "custom"),
                ]
                    .iter()
                    .map(|&(label, m)| {
                        let c2 = c;
                        let is_active = launch_method == m;
                        let m_str = String::from(m);
                        let btn = button(text(label).size(12))
                            .on_press(Message::UpdateInstanceField(InstanceField::LaunchMethod(m_str)))
                            .padding([4, 10]);
                        if is_active {
                            btn.style(move |_t, _s| theme::tab_button_active_style(&c2)).into()
                        } else {
                            btn.style(move |_t, s| match s {
                                button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                                _ => theme::tab_button_style(&c2),
                            }).into()
                        }
                    })
                    .collect();

                // Conditional launch method fields
                let launch_fields: Element<Message> = match launch_method.as_str() {
                    "lutris" => column![
                        text("Lutris target").size(13).color(colors.text),
                        iced::widget::text_input("lutris:rungameid/2", lutris_target)
                            .on_input(|s| Message::UpdateInstanceField(InstanceField::LutrisTarget(s)))
                            .padding([8, 12]),
                        text("Example: lutris:rungameid/2").size(11).color(colors.muted),
                    ].spacing(4).into(),
                    "wine" => column![
                        text("Wine command").size(13).color(colors.text),
                        iced::widget::text_input("wine", wine_command)
                            .on_input(|s| Message::UpdateInstanceField(InstanceField::WineCommand(s)))
                            .padding([8, 12]),
                        text("Wine arguments").size(13).color(colors.text),
                        iced::widget::text_input("--some-arg value", wine_args)
                            .on_input(|s| Message::UpdateInstanceField(InstanceField::WineArgs(s)))
                            .padding([8, 12]),
                    ].spacing(4).into(),
                    "custom" => column![
                        text("Custom command").size(13).color(colors.text),
                        iced::widget::text_input("command", custom_command)
                            .on_input(|s| Message::UpdateInstanceField(InstanceField::CustomCommand(s)))
                            .padding([8, 12]),
                        text("Custom arguments").size(13).color(colors.text),
                        iced::widget::text_input("--flag value", custom_args)
                            .on_input(|s| Message::UpdateInstanceField(InstanceField::CustomArgs(s)))
                            .padding([8, 12]),
                        text("Tip: use {exe} in args to inject the detected game executable path.")
                            .size(11).color(colors.muted),
                    ].spacing(4).into(),
                    _ => {  // "auto"
                        text("Auto: launches VanillaFixes.exe if present, otherwise Wow.exe")
                            .size(12).color(colors.muted).into()
                    }
                };

                column![
                    row![
                        text(title_text).size(18).color(colors.title),
                        Space::new().width(Length::Fill),
                        close_button(&c),
                    ].align_y(iced::Alignment::Center),
                    text("Configure name, game path, and launch behavior for this instance.")
                        .size(12).color(colors.muted),
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
                        button(text("Browse").size(12))
                            .on_press(Message::PickWowDirectory)
                            .padding([8, 12])
                            .style(move |_t, s| match s {
                                button::Status::Hovered => theme::tab_button_hovered_style(&c),
                                _ => theme::tab_button_style(&c),
                            }),
                    ].spacing(6),
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
                            footer_items.push(
                                button(text("Remove").size(13).color(c.bad))
                                    .on_press(Message::RemoveProfile(remove_id))
                                    .padding([6, 14])
                                    .style(move |_theme, _status| {
                                        let mut s = theme::tab_button_style(&c2);
                                        s.border.color = c2.bad;
                                        s
                                    })
                                    .into(),
                            );
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
                        footer_items.push(
                            button(text("Save").size(13))
                                .on_press(Message::SaveInstanceSettings)
                                .padding([6, 14])
                                .style(move |_theme, _status| theme::tab_button_active_style(&c))
                                .into(),
                        );
                        row(footer_items).spacing(8)
                    },
                ]
                .spacing(8)
                .into()
            }
        }
    }

    // -----------------------------------------------------------------------
    // Topbar
    // -----------------------------------------------------------------------

    fn view_topbar(&self, colors: &ThemeColors) -> Element<'_, Message> {
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

        // Busy spinner — always reserve its space so the left section never
        // changes width. Render an invisible placeholder when not spinning.
        let spinner_el: Element<Message> = if self.is_busy() {
            let tick = self.spinner_tick;
            let primary = colors.primary;
            canvas(SpinnerCanvas { tick, color: primary })
                .width(26)
                .height(26)
                .into()
        } else {
            Space::new().width(26).height(26).into()
        };

        // Left section: title + spinner placeholder (fixed width, never shifts)
        let left_section = row![title, spinner_el]
            .spacing(12)
            .align_y(iced::Alignment::End);

        // Right section: optional profile picker + action buttons
        let mut right_items: Vec<Element<Message>> = Vec::new();

        if self.profiles.len() > 1 {
            let display_labels: Vec<String> = self.profiles.iter().map(|p| {
                let dupes = self.profiles.iter().filter(|q| q.name == p.name).count();
                if dupes > 1 { format!("{} ({})", p.name, p.id) } else { p.name.clone() }
            }).collect();

            let active_display = self.profiles.iter()
                .find(|p| p.id == self.active_profile_id)
                .map(|p| {
                    let dupes = self.profiles.iter().filter(|q| q.name == p.name).count();
                    if dupes > 1 { format!("{} ({})", p.name, p.id) } else { p.name.clone() }
                })
                .unwrap_or_else(|| "Default".to_string());

            let profile_picker: Element<Message> = iced::widget::pick_list(
                display_labels,
                Some(active_display),
                {
                    let profiles = self.profiles.clone();
                    move |display: String| {
                        let profile = profiles.iter().find(|p| {
                            let dupes = profiles.iter().filter(|q| q.name == p.name).count();
                            let label = if dupes > 1 { format!("{} ({})", p.name, p.id) } else { p.name.clone() };
                            label == display
                        });
                        Message::SwitchProfile(profile.map(|p| p.id.clone()).unwrap_or_default())
                    }
                },
            )
            .text_size(13)
            .into();

            let divider = rule::vertical(1).style(move |_theme| theme::divider_style(&c));
            right_items.push(profile_picker);
            right_items.push(divider.into());
        }

        right_items.push(action_tabs.into());
        let right_section = row(right_items).spacing(10).align_y(iced::Alignment::Center);

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

    fn view_tab_button(&self, tab: Tab, colors: &ThemeColors) -> Element<'_, Message> {
        let is_active = self.active_tab == tab;
        let c = *colors;

        let is_icon = matches!(tab, Tab::Options | Tab::Logs | Tab::About);

        let lbl = self.tab_label(tab);
        let label = text(lbl).size(if is_icon { 15 } else { 14 });
        // All fixed-width buttons: center content inside
        let content: Element<Message> = container(label)
            .width(Length::Fill)
            .center_x(Length::Fill)
            .into();
        let btn = button(content)
            .on_press(Message::SetTab(tab))
            .padding([7, 0])
            .width(if is_icon { Length::Fixed(32.0) } else { Length::Fixed(114.0) });

        if is_active {
            btn.style(move |_theme, _status| {
                theme::tab_button_active_style(&c)
            })
            .into()
        } else {
            btn.style(move |_theme, status| match status {
                button::Status::Hovered => theme::tab_button_hovered_style(&c),
                button::Status::Pressed => theme::tab_button_active_style(&c),
                _ => theme::tab_button_style(&c),
            })
            .into()
        }
    }

    // -----------------------------------------------------------------------
    // Panel body
    // -----------------------------------------------------------------------

    fn view_panel(&self, colors: &ThemeColors) -> Element<'_, Message> {
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

    // -----------------------------------------------------------------------
    // Footer
    // -----------------------------------------------------------------------

    fn view_footer(&self, colors: &ThemeColors) -> Element<'_, Message> {
        let c = *colors;

        let hint = if self.wow_dir.is_empty() {
            text("No WoW directory set. Go to Options to configure.").size(12).color(colors.warn)
        } else {
            text("Launch target: VanillaFixes.exe if installed, otherwise Wow.exe.").size(12).color(colors.muted)
        };

        let play_btn = button(
            container(text("PLAY").size(16))
                .center_x(Length::Shrink),
        )
        .on_press(Message::LaunchGame)
        .padding([10, 36])
        .width(108)
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::play_button_hovered_style(&c),
            _ => theme::play_button_style(&c),
        });

        let bar = row![
            hint,
            Space::new().width(Length::Fill),
            play_btn,
        ]
        .spacing(12)
        .padding([10, 12])
        .align_y(iced::Alignment::Center);

        container(bar)
            .width(Length::Fill)
            .style(move |_theme| theme::footer_style(&c))
            .into()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A repo is a "mod" if it's NOT an addon (addon or addon_git mode).
/// This matches the Tauri version's `isAddonRepo` inverse logic.
pub fn is_mod(repo: &RepoRow) -> bool {
    !matches!(repo.mode.as_str(), "addon" | "addon_git")
}

/// Returns the font for project names: Bold when using default font, Regular when using Friz
/// (Friz Quadrata only ships Regular weight; requesting Bold causes a fallback to system font).
pub fn name_font(colors: &ThemeColors) -> Font {
    if colors.body_font.family == iced::font::Family::SansSerif {
        Font { weight: iced::font::Weight::Bold, ..Font::DEFAULT }
    } else {
        colors.body_font
    }
}

fn close_button<'a>(colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    button(text("\u{2715}").size(16).color(c.muted)) // ✕
        .on_press(Message::CloseDialog)
        .padding([4, 8])
        .style(move |_theme, status| match status {
            button::Status::Hovered => {
                let mut s = theme::tab_button_hovered_style(&c);
                s.text_color = c.text;
                s
            }
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

/// Build the context menu content for a repo row (used inline in the row itself).
pub fn inline_context_menu<'a>(app: &App, repo: &RepoRow, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    let rid = repo.id;
    let has_update = app.plans.iter().any(|p| p.repo_id == rid && p.has_update);
    let enabled = repo.enabled;
    let is_mod_val = is_mod(repo);
    let name = format!("{}/{}", repo.owner, repo.name);

    let mut items: Vec<Element<Message>> = Vec::new();

    if has_update {
        items.push(ctx_menu_item("\u{2193} Update", Message::UpdateRepo(rid), &c));
    }
    items.push(ctx_menu_item("Reinstall / Repair", Message::ReinstallRepo(rid), &c));
    if is_mod_val {
        let label = if enabled { "Disable" } else { "Enable" };
        items.push(ctx_menu_item(label, Message::ToggleRepoEnabled(rid, !enabled), &c));
    }
    // Remove (danger)
    let c3 = c;
    items.push(
        button(text("Remove").size(12).color(c.bad))
            .on_press(Message::OpenDialog(Dialog::RemoveRepo { id: rid, name }))
            .padding([6, 12])
            .width(Length::Fill)
            .style(move |_theme, status| {
                let mut s = match status {
                    button::Status::Hovered => theme::tab_button_hovered_style(&c3),
                    _ => button::Style {
                        background: None,
                        text_color: c3.bad,
                        border: iced::Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: true,
                    },
                };
                s.border.color = c3.bad;
                s
            })
            .into(),
    );

    container(column(items).spacing(2))
        .padding(6)
        .width(170)
        .style(move |_theme| theme::context_menu_style(&c))
        .into()
}

fn ctx_menu_item<'a>(label: &str, msg: Message, colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    button(text(String::from(label)).size(12))
        .on_press(msg)
        .padding([6, 12])
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

/// Clipboard helper — uses system commands on Linux for reliable Wayland support.
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    // Try wl-copy (Wayland) — most reliable on modern Linux
    if let Ok(mut child) = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
        return Ok(());
    }
    // Try xclip (X11)
    if let Ok(mut child) = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
        return Ok(());
    }
    // Try xsel
    if let Ok(mut child) = Command::new("xsel")
        .args(["--clipboard", "--input"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
        return Ok(());
    }
    Err("No clipboard tool found (wl-copy, xclip, xsel)".to_string())
}

/// Canvas-drawn spinner: a rotating arc, matching Tauri's CSS border-top spinner.
struct SpinnerCanvas {
    tick: usize,
    color: iced::Color,
}

impl<Message> canvas::Program<Message> for SpinnerCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = frame.center();
        let radius = bounds.width.min(bounds.height) / 2.0 - 2.0;
        let stroke_width = 3.0;

        // Background circle (faint)
        let bg_circle = canvas::Path::circle(center, radius);
        frame.stroke(
            &bg_circle,
            canvas::Stroke::default()
                .with_color(iced::Color { a: 0.18, ..self.color })
                .with_width(stroke_width),
        );

        // Rotating arc (270° sweep, starting angle rotates with tick)
        let start_angle = (self.tick as f32) * (std::f32::consts::TAU / 36.0);
        let sweep = std::f32::consts::FRAC_PI_2 * 3.0; // 270 degrees
        let arc = canvas::Path::new(|b| {
            b.arc(canvas::path::Arc {
                center,
                radius,
                start_angle: iced::Radians(start_angle),
                end_angle: iced::Radians(start_angle + sweep),
            });
        });
        frame.stroke(
            &arc,
            canvas::Stroke::default()
                .with_color(self.color)
                .with_width(stroke_width)
                .with_line_cap(canvas::LineCap::Round),
        );

        vec![frame.into_geometry()]
    }
}

fn chrono_now() -> String {
    // Simple time string without pulling in chrono crate
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, s)
}
