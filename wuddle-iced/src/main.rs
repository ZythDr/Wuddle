mod anchored_overlay;
mod panels;
mod service;
mod radio;
mod settings;
#[allow(dead_code)]
mod theme;
mod tweaks;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;

use iced::widget::{button, canvas, checkbox, column, container, row, rule, scrollable, stack, text, Space};
use iced::{Element, Font, Length, Subscription, Task, Theme};
use service::{PlanRow, RepoRow};
use settings::UpdateChannel;
use theme::{ThemeColors, WuddleTheme};

const LIFECRAFT: Font = Font::with_name("LifeCraft");
const FRIZ: Font = Font::with_name("Friz Quadrata Std");
const NOTO: Font = Font::with_name("Noto Sans");

/// Returns a path to a temp copy of the app icon, suitable for desktop notifications.
/// Written once and cached for the process lifetime.
fn notification_icon_path() -> &'static str {
    static ICON_PATH: OnceLock<String> = OnceLock::new();
    ICON_PATH.get_or_init(|| {
        let icon_bytes = include_bytes!("../icons/128x128.png");
        let dir = std::env::temp_dir().join("wuddle");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("notification-icon.png");
        if !path.exists()
            || std::fs::metadata(&path)
                .map(|m| m.len())
                .unwrap_or(0)
                != icon_bytes.len() as u64
        {
            let _ = std::fs::write(&path, icon_bytes);
        }
        path.to_string_lossy().into_owned()
    })
}

fn main() -> iced::Result {
    // Read settings early so we can set the default font.
    // Noto Sans is the default UI font (matches Tauri's system-ui stack on Linux);
    // Friz Quadrata overrides it when the user opts in.
    let saved = settings::load_settings();
    let default_font = if saved.opt_friz_font { FRIZ } else { NOTO };

    iced::application(App::new, App::update, App::view)
        .title("Wuddle")
        .theme(App::theme)
        .subscription(App::subscription)
        .font(include_bytes!("../assets/fonts/LifeCraft_Font.ttf"))
        .font(include_bytes!("../assets/fonts/FrizQuadrataStd-Regular.otf"))
        .font(include_bytes!("../assets/fonts/NotoSans-Regular.ttf"))
        .font(include_bytes!("../assets/fonts/NotoSans-Bold.ttf"))
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

// ---------------------------------------------------------------------------
// DXVK config types
// ---------------------------------------------------------------------------

/// Three-state option: let DXVK auto-detect, force on, or force off.
#[derive(Debug, Clone, PartialEq)]
pub enum TriState {
    Auto,
    True,
    False,
}

/// How to handle d3d9.presentInterval (VSync override).
#[derive(Debug, Clone, PartialEq)]
pub enum PresentInterval {
    Default, // -1: do not override the in-game setting
    NoSync,  // 0: always no VSync
    Vsync,   // 1: always VSync
    Half,    // 2: half refresh rate (e.g. 30 fps on 60 Hz)
}

/// Anisotropic filtering level for d3d9.samplerAnisotropy.
#[derive(Debug, Clone, PartialEq)]
pub enum AnisotropyLevel {
    NoOverride, // -1: let the game / driver decide
    Off,        // 0: force disabled
    X2,
    X4,
    X8,
    X16,
}

/// Field mutation carried by SetDxvkField messages.
#[derive(Debug, Clone)]
pub enum DxvkField {
    MaxFrameRate(String),
    MaxFrameLatency(String),
    LatencySleep(TriState),
    EnableDialogMode(bool),
    DpiAware(bool),
    PresentInterval(PresentInterval),
    TearFree(TriState),
    SamplerAnisotropy(AnisotropyLevel),
    ClampNegativeLodBias(bool),
    NumCompilerThreads(String),
    EnableGpl(TriState),
    TrackPipelineLifetime(TriState),
    DeferSurfaceCreation(bool),
    LenientClear(bool),
    LogPath(String),
    Hud(String),
    EnableAsync(bool),
}

/// State held inside Dialog::DxvkConfig.
#[derive(Debug, Clone)]
pub struct DxvkConfig {
    pub max_frame_rate: String,       // d3d9.maxFrameRate
    pub max_frame_latency: String,    // d3d9.maxFrameLatency
    pub latency_sleep: TriState,      // dxvk.latencySleep
    pub enable_dialog_mode: bool,     // d3d9.enableDialogMode
    pub dpi_aware: bool,              // d3d9.dpiAware
    pub present_interval: PresentInterval, // d3d9.presentInterval
    pub tear_free: TriState,          // dxvk.tearFree
    pub sampler_anisotropy: AnisotropyLevel, // d3d9.samplerAnisotropy
    pub clamp_negative_lod_bias: bool, // d3d9.clampNegativeLodBias
    pub num_compiler_threads: String, // dxvk.numCompilerThreads
    pub enable_gpl: TriState,         // dxvk.enableGraphicsPipelineLibrary
    pub track_pipeline_lifetime: TriState, // dxvk.trackPipelineLifetime
    pub defer_surface_creation: bool, // d3d9.deferSurfaceCreation
    pub lenient_clear: bool,          // d3d9.lenientClear
    pub log_path: String,             // dxvk.logPath
    pub hud: String,                  // dxvk.hud
    pub enable_async: bool,           // dxvk.enableAsync (gplasync fork)
}

impl Default for DxvkConfig {
    fn default() -> Self {
        Self {
            max_frame_rate: "240".into(),
            max_frame_latency: "1".into(),
            latency_sleep: TriState::Auto,
            enable_dialog_mode: true,
            dpi_aware: false,
            present_interval: PresentInterval::Default,
            tear_free: TriState::Auto,
            sampler_anisotropy: AnisotropyLevel::X16,
            clamp_negative_lod_bias: false,
            num_compiler_threads: "0".into(),
            enable_gpl: TriState::Auto,
            track_pipeline_lifetime: TriState::Auto,
            defer_surface_creation: true,
            lenient_clear: true,
            log_path: ".".into(),
            hud: String::new(),
            enable_async: true,
        }
    }
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Dialog {
    AddRepo { url: String, mode: String, is_addons: bool, advanced: bool },
    RemoveRepo { id: i64, name: String, remove_files: bool, files: Vec<(String, String)> },
    Changelog { items: Vec<iced::widget::markdown::Item>, loading: bool },
    DxvkConfig { config: DxvkConfig, show_preview: bool },
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
    pub opt_xattr: bool,
    pub opt_clock12: bool,
    pub opt_friz_font: bool,
    // Radio
    pub radio_playing: bool,
    pub radio_volume: f32,
    pub radio_connecting: bool,
    pub radio_auto_connect: bool,
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

    // Repos whose updates are being ignored
    pub ignored_update_ids: HashSet<i64>,

    // Multi-DLL repos that are currently expanded in the project list
    pub expanded_repo_ids: HashSet<i64>,

    // Selectable log view
    pub log_editor_content: iced::widget::text_editor::Content,

    // Selectable DXVK config preview
    pub dxvk_preview_content: iced::widget::text_editor::Content,

    // README source-view toggle (formatted markdown ↔ selectable raw text)
    pub readme_source_view: bool,
    pub readme_editor_content: iced::widget::text_editor::Content,

    // Release channel
    pub update_channel: UpdateChannel,
}

impl Default for WuddleTheme {
    fn default() -> Self {
        WuddleTheme::Cata
    }
}

// ---------------------------------------------------------------------------
// Channel switching — writes current.json and exits so the launcher picks up
// the highest stable version. Returns false if no stable version is found
// locally (caller should open the releases page as a fallback).
// ---------------------------------------------------------------------------

fn switch_to_stable_channel() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false; };
    // Expect layout: <launcher_dir>/versions/<version>/<binary>
    let Some(launcher_dir) = exe.parent().and_then(|v| v.parent()).and_then(|v| v.parent()) else {
        return false;
    };
    let Ok(entries) = std::fs::read_dir(launcher_dir.join("versions")) else { return false; };

    let mut stables: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| {
            let l = n.to_lowercase();
            !l.contains("alpha") && !l.contains("beta") && !l.contains("pre") && !l.contains("rc")
        })
        .collect();

    if stables.is_empty() { return false; }

    stables.sort_by(|a, b| semver_parts(b).cmp(&semver_parts(a)));
    let best = &stables[0];
    let json = format!("{{\"current\":\"{}\"}}\n", best);
    if std::fs::write(launcher_dir.join("current.json"), json).is_err() { return false; }

    std::process::exit(0);
}

fn semver_parts(s: &str) -> Vec<u64> {
    s.trim_start_matches(|c: char| !c.is_ascii_digit())
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|seg| seg.parse::<u64>().ok())
        .collect()
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
    SetAutoCheckMinutes(String),
    ToggleDesktopNotify(bool),
    ToggleSymlinks(bool),
    ToggleXattr(bool),
    ToggleClock12(bool),
    ToggleFrizFont(bool),
    // Radio
    ToggleRadio,
    RadioStarted(Result<radio::RadioHandle, String>),
    SetRadioVolume(f32),
    ToggleRadioAutoConnect(bool),
    AutoConnectRadio,
    SetGithubTokenInput(String),

    // Tweaks
    ToggleTweak(TweakId, bool),

    // Logs
    SetLogFilter(LogFilter),
    SetLogSearch(String),
    ToggleLogWrap(bool),
    ToggleLogAutoScroll(bool),
    ToggleLogErrorFetch(bool),
    ToggleLogErrorMisc(bool),
    ClearLogs,

    // Dialogs
    OpenDialog(Dialog),
    CloseDialog,

    // Context menu
    ToggleMenu(i64),
    CloseMenu,
    ToggleAddNewMenu,

    // Engine data (Phase 2)
    ReposLoaded(Result<Vec<RepoRow>, String>),
    PlansLoaded(Result<Vec<PlanRow>, String>),
    SettingsLoaded(settings::AppSettings),

    // Operations (Phase 3)
    CheckUpdates,
    CheckUpdatesResult(Result<Vec<PlanRow>, String>),
    AddRepoSubmit,
    AddRepoResult(Result<i64, String>),
    InstallAfterAddResult(Result<String, String>),
    RemoveRepoConfirm(i64, bool),
    ToggleRemoveFiles(bool),
    RemoveRepoFilesLoaded(Result<Vec<(String, String)>, String>),
    RemoveRepoResult(Result<(), String>),
    ToggleRepoEnabled(i64, bool),
    ToggleRepoEnabledResult(Result<(), String>),
    ToggleRepoExpanded(i64),
    ToggleDllEnabled(i64, String, bool),
    ToggleDllEnabledResult(Result<(), String>),
    UpdateAll,
    UpdateAllResult(Result<Vec<service::UpdateOneResult>, String>),
    UpdateRepo(i64),
    UpdateRepoResult(Result<Option<PlanRow>, String>),
    ReinstallRepo(i64),
    ReinstallRepoResult(Result<PlanRow, String>),
    FetchBranches(i64),
    FetchBranchesResult((i64, Result<Vec<String>, String>)),
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
    ReadTweaksResult(Result<tweaks::ReadTweakValues, String>),
    ApplyTweaks,
    ApplyTweaksResult(Result<String, String>),
    RestoreTweaks,
    RestoreTweaksResult(Result<String, String>),
    ResetTweaksToDefault,

    ToggleIgnoreUpdates(i64),

    // About
    CheckSelfUpdate,
    CheckSelfUpdateResult(Result<String, String>),
    ShowChangelog,
    ChangelogLoaded(Result<String, String>),

    // Add-repo preview
    SetAddRepoUrl(String),
    FetchRepoPreview(String),
    FetchRepoPreviewResult(Result<service::RepoPreviewInfo, String>),
    ToggleAddRepoDir(String),
    PreviewRepoFile(String),
    PreviewRepoFileResult(Result<(String, String), String>),
    FetchDirContents(String, String),
    FetchDirContentsResult(Result<(String, Vec<service::RepoFileEntry>), String>),

    // Release notes (in-app)
    FetchReleaseNotes,
    FetchReleaseNotesResult(Result<Vec<service::ReleaseItem>, String>),
    ShowReadme,

    // Auto-check tick
    AutoCheckTick,

    // Spinner animation
    SpinnerTick,

    // Selectable log view
    LogEditorAction(iced::widget::text_editor::Action),

    // README source toggle
    ToggleReadmeSourceView,
    ReadmeEditorAction(iced::widget::text_editor::Action),

    // DXVK config dialog
    OpenDxvkConfig,
    SetDxvkField(DxvkField),
    SaveDxvkConfig,
    DxvkConfigSaved(Result<(), String>),
    ToggleDxvkPreview,
    DxvkPreviewEditorAction(iced::widget::text_editor::Action),

    // Release channel
    SetUpdateChannel(UpdateChannel),
    SwitchToStableChannel,
}

// ---------------------------------------------------------------------------
// Wraps a code block element with a "Copy" button overlaid at the top-right corner.
fn with_copy_button(block: Element<'_, Message>, code: String) -> Element<'_, Message> {
    let copy_btn = container(
        button(text("Copy").size(11))
            .on_press(Message::CopyToClipboard(code))
            .padding([2, 8])
            .style(|_theme, status| match status {
                button::Status::Hovered => button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.15))),
                    text_color: iced::Color::WHITE,
                    border: iced::Border { radius: 3.0.into(), ..Default::default() },
                    ..Default::default()
                },
                _ => button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.07))),
                    text_color: iced::Color::from_rgb8(0xb0, 0xc4, 0xde),
                    border: iced::Border { radius: 3.0.into(), ..Default::default() },
                    ..Default::default()
                },
            }),
    )
    .width(Length::Fill)
    .align_x(iced::Alignment::End)
    .padding(iced::Padding { top: 4.0, right: 6.0, bottom: 0.0, left: 0.0 });

    iced::widget::stack![block, copy_btn].into()
}

// ---------------------------------------------------------------------------
// Lazy-initialized syntect state for syntax highlighting
// ---------------------------------------------------------------------------

fn syntax_set() -> &'static syntect::parsing::SyntaxSet {
    static SS: OnceLock<syntect::parsing::SyntaxSet> = OnceLock::new();
    SS.get_or_init(syntect::parsing::SyntaxSet::load_defaults_newlines)
}

fn highlight_theme() -> &'static syntect::highlighting::Theme {
    static TS: OnceLock<syntect::highlighting::ThemeSet> = OnceLock::new();
    let ts = TS.get_or_init(syntect::highlighting::ThemeSet::load_defaults);
    &ts.themes["base16-ocean.dark"]
}

// ---------------------------------------------------------------------------
// Custom markdown viewer: cached images + bold headings + syntax highlighting
// ---------------------------------------------------------------------------

struct ImageViewer<'a> {
    cache: &'a std::collections::HashMap<String, Vec<u8>>,
    raw_base_url: &'a str,
}

impl<'a> iced::widget::markdown::Viewer<'a, Message> for ImageViewer<'a> {
    fn on_link_click(url: iced::widget::markdown::Uri) -> Message {
        if let Some(text) = url.strip_prefix("wuddle-copy://") {
            Message::CopyToClipboard(text.to_string())
        } else {
            Message::OpenUrl(url)
        }
    }

    fn paragraph(
        &self,
        settings: iced::widget::markdown::Settings,
        text: &iced::widget::markdown::Text,
    ) -> Element<'a, Message> {
        // Clone spans and inject copy links into inline-code spans (identified by highlight background)
        let raw_spans = text.spans(settings.style);
        let has_code = raw_spans.iter().any(|s| s.highlight.is_some() && s.link.is_none());
        if !has_code {
            return iced::widget::markdown::paragraph(settings, text, Self::on_link_click);
        }
        let patched: Vec<iced::widget::text::Span<'static, iced::widget::markdown::Uri>> =
            raw_spans.iter().cloned().map(|mut s| {
                if s.highlight.is_some() && s.link.is_none() {
                    let copy_text = s.text.as_ref().trim().to_string();
                    s.link = Some(format!("wuddle-copy://{copy_text}"));
                    // Subtle underline hint so user knows it's clickable
                    s.underline = true;
                }
                s
            }).collect();
        iced::widget::rich_text(patched)
            .size(settings.text_size)
            .on_link_click(Self::on_link_click)
            .into()
    }

    fn heading(
        &self,
        settings: iced::widget::markdown::Settings,
        level: &'a iced::widget::markdown::HeadingLevel,
        text: &'a iced::widget::markdown::Text,
        index: usize,
    ) -> Element<'a, Message> {
        // Render headings with bold weight
        let bold_settings = iced::widget::markdown::Settings {
            style: iced::widget::markdown::Style {
                font: iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..settings.style.font
                },
                ..settings.style
            },
            ..settings
        };
        iced::widget::markdown::heading(bold_settings, level, text, index, Self::on_link_click)
    }

    fn image(
        &self,
        _settings: iced::widget::markdown::Settings,
        url: &'a iced::widget::markdown::Uri,
        _title: &'a str,
        _alt: &iced::widget::markdown::Text,
    ) -> Element<'a, Message> {
        // Try original URL, then resolved absolute URL
        let bytes = self.cache.get(url.as_str())
            .or_else(|| {
                let abs = service::resolve_image_url(url, self.raw_base_url);
                self.cache.get(abs.as_str())
            });
        if let Some(bytes) = bytes {
            container(
                iced::widget::image(
                    iced::widget::image::Handle::from_bytes(bytes.clone())
                )
                .width(Length::Fill)
            )
            .width(Length::Fill)
            .padding([4, 0])
            .into()
        } else {
            // Show a subtle placeholder for unfetched images
            container(
                text(format!("[image: {}]", url.split('/').last().unwrap_or(url)))
                    .size(11)
                    .color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.25))
            )
            .padding([2, 0])
            .into()
        }
    }

    fn code_block(
        &self,
        settings: iced::widget::markdown::Settings,
        language: Option<&'a str>,
        code: &'a str,
        lines: &'a [iced::widget::markdown::Text],
    ) -> Element<'a, Message> {
        use syntect::easy::HighlightLines;
        use syntect::util::LinesWithEndings;

        // Only attempt syntect highlighting when a language hint is given and recognized
        if let Some(lang_str) = language {
            let ps = syntax_set();
            let syntax = ps.find_syntax_by_token(lang_str)
                .or_else(|| ps.find_syntax_by_extension(lang_str));

            if let Some(syntax) = syntax {
                let theme = highlight_theme();
                let mut h = HighlightLines::new(syntax, theme);
                let code_font = settings.style.code_block_font;
                let code_size = settings.code_size;

                let line_elements: Vec<Element<'a, Message>> = LinesWithEndings::from(code)
                    .filter_map(|line| {
                        let tokens = h.highlight_line(line, ps).ok()?;
                        let spans: Vec<iced::widget::text::Span<'static, iced::widget::markdown::Uri>> = tokens
                            .iter()
                            .filter(|(_, s)| !s.is_empty())
                            .map(|(style, token)| {
                                iced::widget::span(token.to_string())
                                    .color(iced::Color::from_rgb(
                                        style.foreground.r as f32 / 255.0,
                                        style.foreground.g as f32 / 255.0,
                                        style.foreground.b as f32 / 255.0,
                                    ))
                                    .font(code_font)
                            })
                            .collect();
                        Some(
                            iced::widget::rich_text(spans)
                                .size(code_size)
                                .into(),
                        )
                    })
                    .collect();

                let bg = iced::Color::from_rgb8(0x14, 0x18, 0x24);
                let border_color = iced::Color::from_rgb8(0x2a, 0x2f, 0x3d);
                let code_owned = code.to_string();

                let inner = container(
                    iced::widget::scrollable(
                        container(column(line_elements))
                            .padding(settings.code_size),
                    )
                    .direction(iced::widget::scrollable::Direction::Horizontal(
                        iced::widget::scrollable::Scrollbar::default()
                            .width(settings.code_size / 2)
                            .scroller_width(settings.code_size / 2),
                    )),
                )
                .width(Length::Fill)
                .padding(settings.code_size / 4)
                .style(move |_t| container::Style {
                    background: Some(iced::Background::Color(bg)),
                    border: iced::Border {
                        color: border_color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                });

                return with_copy_button(inner.into(), code_owned);
            }
        }

        // Fall back to default unstyled code block, also with copy button
        let code_owned = code.to_string();
        let fallback = iced::widget::markdown::code_block(settings, lines, Self::on_link_click);
        with_copy_button(fallback, code_owned)
    }
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
            opt_xattr: true,
            opt_clock12: false,
            opt_friz_font: false,
            radio_playing: false,
            radio_volume: 0.25,
            radio_connecting: false,
            radio_auto_connect: false,
            radio_error: None,
            radio_handle: None,
            github_token_input: String::new(),
            tweaks: TweakState::default(),
            log_lines: vec![
                LogLine { level: LogLevel::Info, text: concat!("Wuddle v", env!("CARGO_PKG_VERSION"), " started").into(), timestamp: chrono_now() },
                LogLine { level: LogLevel::Info, text: "Ready.".into(), timestamp: chrono_now() },
            ],
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
            tweak_values: TweakValues::default(),
            latest_version: None,
            update_message: None,
            auto_check_minutes: 60,
            profiles: vec![settings::ProfileConfig::default()],
            spinner_tick: 0,
            add_repo_preview: None,
            add_repo_preview_loading: false,
            add_repo_expanded_dirs: HashSet::new(),
            add_repo_dir_contents: HashMap::new(),
            add_repo_file_preview: None,
            add_repo_release_notes: None,
            add_repo_show_releases: false,
            ignored_update_ids: HashSet::new(),
            expanded_repo_ids: HashSet::new(),
            log_editor_content: iced::widget::text_editor::Content::with_text(
                concat!("[INFO] Wuddle v", env!("CARGO_PKG_VERSION"), " started\n[INFO] Ready.")
            ),
            readme_source_view: false,
            readme_editor_content: iced::widget::text_editor::Content::new(),
            dxvk_preview_content: iced::widget::text_editor::Content::new(),
            update_channel: UpdateChannel::Beta,
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
            timestamp: chrono_now_fmt(self.opt_clock12),
        });
        self.rebuild_log_content();
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
            opt_xattr: self.opt_xattr,
            radio_auto_connect: self.radio_auto_connect,
            radio_volume: self.radio_volume,
            opt_clock12: self.opt_clock12,
            opt_friz_font: self.opt_friz_font,
            log_wrap: self.log_wrap,
            log_autoscroll: self.log_autoscroll,
            auto_check_minutes: self.auto_check_minutes,
            profiles: self.profiles.clone(),
            ignored_update_ids: self.ignored_update_ids.iter().cloned().collect(),
            update_channel: self.update_channel,
        };
        let _ = settings::save_settings(&s);
    }

    fn theme(&self) -> Theme {
        self.wuddle_theme.to_iced_theme()
    }

    fn rebuild_log_content(&mut self) {
        let search = self.log_search.to_ascii_lowercase();
        let text: String = self.log_lines
            .iter()
            .filter(|line| match self.log_filter {
                LogFilter::All => true,
                LogFilter::Info => matches!(line.level, LogLevel::Info),
                LogFilter::Errors => {
                    if !matches!(line.level, LogLevel::Error) { return false; }
                    let fetch = panels::logs::is_fetch_error(&line.text);
                    (fetch && self.log_error_fetch) || (!fetch && self.log_error_misc)
                }
            })
            .filter(|line| search.is_empty() || line.text.to_ascii_lowercase().contains(&search))
            .map(|line| {
                let prefix = match line.level {
                    LogLevel::Info => "[INFO]",
                    LogLevel::Error => "[ERROR]",
                };
                format!("[{}] {} {}", line.timestamp, prefix, line.text)
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.log_editor_content = iced::widget::text_editor::Content::with_text(&text);
        if self.log_autoscroll {
            self.log_editor_content.perform(
                iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
            );
        }
    }

    fn is_busy(&self) -> bool {
        self.loading
            || self.checking_updates
            || self.updating_all
            || !self.updating_repo_ids.is_empty()
            || self.add_repo_preview_loading
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

        // Hourly self-update check for unauthenticated users; authenticated users get
        // checked on launch and on every About-tab navigation.
        if wuddle_engine::github_token().is_none() {
            subs.push(
                iced::time::every(std::time::Duration::from_secs(3600))
                    .map(|_| Message::CheckSelfUpdate),
            );
        }

        if self.is_busy() {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(80))
                    .map(|_| Message::SpinnerTick),
            );
        }

        if self.dialog.is_some() {
            subs.push(iced::event::listen_with(|event, _status, _window| {
                match event {
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                        ..
                    }) => Some(Message::CloseDialog),
                    _ => None,
                }
            }));
        }

        Subscription::batch(subs)
    }

    fn colors(&self) -> ThemeColors {
        let mut c = self.wuddle_theme.colors();
        c.body_font = self.body_font();
        c
    }

    pub fn body_font(&self) -> Font {
        if self.opt_friz_font { FRIZ } else { NOTO }
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
            p.has_update && !self.ignored_update_ids.contains(&p.repo_id)
                && self.repos.iter().any(|r| r.id == p.repo_id && is_mod(r))
        }).count()
    }

    pub fn addon_update_count(&self) -> usize {
        self.plans.iter().filter(|p| {
            p.has_update && !self.ignored_update_ids.contains(&p.repo_id)
                && self.repos.iter().any(|r| r.id == p.repo_id && !is_mod(r))
        }).count()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetTab(tab) => {
                self.active_tab = tab;
                // Fire self-update check whenever the About tab becomes active
                if tab == Tab::About {
                    return Task::perform(service::check_self_update(self.update_channel == UpdateChannel::Beta), Message::CheckSelfUpdateResult);
                }
            }
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
            Message::SetAutoCheckMinutes(s) => {
                if let Ok(n) = s.parse::<u32>() {
                    self.auto_check_minutes = n.max(1);
                } else if s.is_empty() {
                    self.auto_check_minutes = 1;
                }
                self.save_settings();
            }
            Message::ToggleDesktopNotify(b) => { self.opt_desktop_notify = b; self.save_settings(); }
            Message::ToggleSymlinks(b) => { self.opt_symlinks = b; self.save_settings(); }
            Message::ToggleXattr(b) => { self.opt_xattr = b; self.save_settings(); }
            Message::ToggleClock12(b) => { self.opt_clock12 = b; self.save_settings(); }
            Message::ToggleFrizFont(b) => {
                self.opt_friz_font = b;
                self.save_settings();
                self.log(LogLevel::Info, "Friz Quadrata font setting saved. Restart Wuddle to apply.");
            }
            Message::ToggleRadio => {
                if self.radio_connecting {
                    return Task::none();
                }
                if self.radio_playing {
                    // User pressed Stop — fade out, keep handle for instant resume
                    self.radio_playing = false;
                    self.log(LogLevel::Info, "Radio: stopped.");
                    if let Some(h) = &self.radio_handle { h.fade_out(); }
                    return Task::none();
                } else if let Some(h) = &self.radio_handle {
                    // Pre-connected — fade in instantly
                    self.radio_playing = true;
                    self.radio_error = None;
                    h.fade_in(self.radio_volume);
                    self.log(LogLevel::Info, "Radio: playing (instant resume).");
                    return Task::none();
                } else {
                    // Need to connect — start silent, fade in after connect
                    self.radio_playing = true;
                    self.radio_connecting = true;
                    self.radio_error = None;
                    self.log(LogLevel::Info, "Radio: connecting…");
                    let (tx, rx) = tokio::sync::oneshot::channel::<Result<radio::RadioHandle, String>>();
                    std::thread::spawn(move || { let _ = tx.send(radio::start(0.0)); });
                    return Task::perform(
                        async move { rx.await.unwrap_or_else(|_| Err("Thread died".to_string())) },
                        Message::RadioStarted,
                    );
                }
            }
            Message::RadioStarted(Ok(handle)) => {
                self.radio_connecting = false;
                if self.radio_playing {
                    handle.fade_in(self.radio_volume);
                    self.log(LogLevel::Info, "Radio: connected and playing.");
                } else {
                    self.log(LogLevel::Info, "Radio: pre-loaded (muted).");
                }
                self.radio_handle = Some(handle);
            }
            Message::RadioStarted(Err(e)) => {
                self.radio_connecting = false;
                self.radio_playing = false;
                self.log(LogLevel::Error, &format!("Radio: connection failed — {e}"));
                self.radio_error = Some(e);
            }
            Message::SetRadioVolume(v) => {
                self.radio_volume = v;
                if self.radio_playing {
                    if let Some(handle) = &self.radio_handle {
                        handle.set_volume(v);
                    }
                }
            }
            Message::ToggleRadioAutoConnect(b) => {
                self.radio_auto_connect = b;
                self.save_settings();
                if b && self.radio_handle.is_none() && !self.radio_connecting {
                    // Immediately start silent connection
                    return Task::done(Message::AutoConnectRadio);
                } else if !b && !self.radio_playing {
                    // Disconnect the silent stream
                    if let Some(h) = self.radio_handle.take() { h.stop(); }
                }
            }
            Message::AutoConnectRadio => {
                let like_turtles = self.profiles.iter()
                    .find(|p| p.id == self.active_profile_id)
                    .map(|p| p.like_turtles)
                    .unwrap_or(true);
                if like_turtles && self.radio_auto_connect && self.radio_handle.is_none() && !self.radio_connecting {
                    self.radio_connecting = true;
                    self.log(LogLevel::Info, "Radio: pre-loading in background…");
                    let (tx, rx) = tokio::sync::oneshot::channel::<Result<radio::RadioHandle, String>>();
                    std::thread::spawn(move || { let _ = tx.send(radio::start(0.0)); });
                    return Task::perform(
                        async move { rx.await.unwrap_or_else(|_| Err("Thread died".to_string())) },
                        Message::RadioStarted,
                    );
                }
            }
            Message::SetGithubTokenInput(s) => self.github_token_input = s,

            // Tweaks
            Message::ToggleTweak(id, val) => self.tweaks.set(id, val),

            // Logs
            Message::SetLogFilter(f) => { self.log_filter = f; self.rebuild_log_content(); }
            Message::SetLogSearch(s) => { self.log_search = s; self.rebuild_log_content(); }
            Message::ToggleLogWrap(b) => { self.log_wrap = b; self.save_settings(); }
            Message::ToggleLogAutoScroll(b) => { self.log_autoscroll = b; self.save_settings(); }
            Message::ToggleLogErrorFetch(b) => { self.log_error_fetch = b; self.rebuild_log_content(); }
            Message::ToggleLogErrorMisc(b) => { self.log_error_misc = b; self.rebuild_log_content(); }
            Message::ClearLogs => { self.log_lines.clear(); self.rebuild_log_content(); }
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
                    iced::widget::operation::focus(iced::widget::Id::new("add_repo_url"))
                } else {
                    Task::none()
                };
                self.dialog = Some(d);
                return fetch_task;
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
            Message::SettingsLoaded(s) => {
                self.wuddle_theme = WuddleTheme::from_key(&s.theme);
                self.active_profile_id = s.active_profile_id.clone();
                self.opt_auto_check = s.opt_auto_check;
                self.opt_desktop_notify = s.opt_desktop_notify;
                self.opt_symlinks = s.opt_symlinks;
                self.opt_xattr = s.opt_xattr;
                self.radio_auto_connect = s.radio_auto_connect;
                self.radio_volume = s.radio_volume;
                self.opt_clock12 = s.opt_clock12;
                self.opt_friz_font = s.opt_friz_font;
                self.log_wrap = s.log_wrap;
                self.log_autoscroll = s.log_autoscroll;
                self.auto_check_minutes = s.auto_check_minutes.max(1);
                self.ignored_update_ids = s.ignored_update_ids.into_iter().collect();
                self.update_channel = s.update_channel;
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
                let repos_task = self.refresh_repos_task();
                if self.radio_auto_connect {
                    return Task::batch([repos_task, Task::done(Message::AutoConnectRadio)]);
                }
                return repos_task;
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
                        let mut tasks: Vec<Task<Message>> = self
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
                        // Auto-check on launch if the option is enabled
                        if self.opt_auto_check && !self.repos.is_empty() && !self.checking_updates {
                            self.checking_updates = true;
                            self.log(LogLevel::Info, "Auto-checking for updates on launch...");
                            tasks.push(self.check_updates_task());
                        }
                        // Always fire self-update check on launch
                        tasks.push(Task::perform(service::check_self_update(self.update_channel == UpdateChannel::Beta), Message::CheckSelfUpdateResult));
                        if !tasks.is_empty() {
                            return Task::batch(tasks);
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
                        let update_count = plans.iter().filter(|p| p.has_update && !self.ignored_update_ids.contains(&p.repo_id)).count();
                        for p in &plans {
                            if let Some(err) = &p.error {
                                // Suppress -16 (GIT_EAUTH): deleted/private repos the user
                                // has acknowledged; they generate noise on every check.
                                if !is_silenced_git_error(err) {
                                    self.log(LogLevel::Error, &format!("{}/{} - {}", p.owner, p.name, simplify_git_error(err)));
                                }
                            }
                        }
                        self.log(LogLevel::Info, &format!("Update check complete. {} updates available.", update_count));
                        self.plans = plans;
                        self.last_checked = Some(chrono_now_fmt(self.opt_clock12));
                        self.cached_plans.insert(
                            self.active_profile_id.clone(),
                            (self.plans.clone(), self.last_checked.clone()),
                        );
                        if self.opt_desktop_notify && update_count > 0 {
                            let _ = notify_rust::Notification::new()
                                .appname("Wuddle")
                                .summary("Wuddle")
                                .body(&format!("{} update{} available", update_count, if update_count == 1 { "" } else { "s" }))
                                .icon(notification_icon_path())
                                .show();
                        }
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.log(LogLevel::Error, &format!("Update check failed: {}", e));
                    }
                }
            }
            Message::AddRepoSubmit => {
                if let Some(Dialog::AddRepo { ref url, ref mode, .. }) = self.dialog {
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
                        // Auto-install mirrors Tauri: update first, reinstall if update
                        // returns None (engine has nothing to fetch but files aren't on disk).
                        // Do NOT run refresh_repos_task concurrently — prune_missing_repos
                        // inside list_repos would delete the newly-added addon_git repo
                        // (no worktree yet) before install can run, causing "Query returned
                        // no rows". InstallAfterAddResult triggers the refresh after install.
                        if !self.wow_dir.is_empty() {
                            let db = self.db_path.clone();
                            let wow = self.wow_dir.clone();
                            let opts = self.install_options();
                            self.log(LogLevel::Info, "Installing…");
                            self.updating_repo_ids.insert(id);
                            return Task::perform(
                                service::install_new_repo(db, id, wow, opts),
                                Message::InstallAfterAddResult,
                            );
                        }
                        return self.refresh_repos_task();
                    }
                    Err(e) => {
                        self.log(LogLevel::Error, &format!("Add repo failed: {}", e));
                        self.error = Some(e);
                    }
                }
            }
            Message::InstallAfterAddResult(result) => {
                match result {
                    Ok(msg) => self.log(LogLevel::Info, &msg),
                    Err(e) => self.log(LogLevel::Error, &format!("Install failed: {}", e)),
                }
                self.updating_repo_ids.clear();
                return self.refresh_repos_task();
            }
            Message::RemoveRepoConfirm(id, remove_files) => {
                let db = self.db_path.clone();
                let wow = if self.wow_dir.is_empty() { None } else { Some(self.wow_dir.clone()) };
                self.dialog = None;
                self.log(LogLevel::Info, &format!("Removing repo id={} (remove_files={})...", id, remove_files));
                return Task::perform(
                    service::remove_repo(db, id, wow, remove_files),
                    Message::RemoveRepoResult,
                );
            }
            Message::ToggleRemoveFiles(val) => {
                if let Some(Dialog::RemoveRepo { ref mut remove_files, .. }) = self.dialog {
                    *remove_files = val;
                }
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
            Message::ToggleIgnoreUpdates(id) => {
                if self.ignored_update_ids.contains(&id) {
                    self.ignored_update_ids.remove(&id);
                } else {
                    self.ignored_update_ids.insert(id);
                }
                self.save_settings();
            }
            Message::ToggleRepoEnabled(id, enabled) => {
                let db = self.db_path.clone();
                let wow = self.wow_dir.clone();
                return Task::perform(
                    service::set_repo_enabled(db, id, enabled, wow),
                    Message::ToggleRepoEnabledResult,
                );
            }
            Message::ToggleRepoEnabledResult(result) => {
                match result {
                    Ok(()) => return self.refresh_repos_task(),
                    Err(e) => self.log(LogLevel::Error, &format!("Enable/disable failed: {}", e)),
                }
            }
            Message::ToggleRepoExpanded(id) => {
                if self.expanded_repo_ids.contains(&id) {
                    self.expanded_repo_ids.remove(&id);
                } else {
                    self.expanded_repo_ids.insert(id);
                }
            }
            Message::ToggleDllEnabled(_repo_id, dll_name, enabled) => {
                let db = self.db_path.clone();
                let wow = self.wow_dir.clone();
                return Task::perform(
                    service::set_dll_enabled(db, wow, dll_name, enabled),
                    Message::ToggleDllEnabledResult,
                );
            }
            Message::ToggleDllEnabledResult(result) => {
                match result {
                    Ok(()) => return self.refresh_repos_task(),
                    Err(e) => self.log(LogLevel::Error, &format!("DLL enable/disable failed: {}", e)),
                }
            }
            Message::UpdateAll => {
                if self.wow_dir.is_empty() {
                    self.log(LogLevel::Error, "Set a WoW directory in Options first.");
                } else {
                    // Only update repos that have a pending update, are not ignored, and are enabled.
                    let ignored = &self.ignored_update_ids;
                    let repos = &self.repos;
                    let ids_to_update: Vec<i64> = self.plans.iter()
                        .filter(|p| {
                            p.has_update
                                && !ignored.contains(&p.repo_id)
                                && repos.iter().any(|r| r.id == p.repo_id && r.enabled)
                        })
                        .map(|p| p.repo_id)
                        .collect();

                    if ids_to_update.is_empty() {
                        self.log(LogLevel::Info, "Nothing to update.");
                    } else {
                        self.updating_all = true;
                        self.log(LogLevel::Info, &format!("Updating {} repo(s)...", ids_to_update.len()));
                        let db = self.db_path.clone();
                        let wow = self.wow_dir.clone();
                        let opts = self.install_options();
                        return Task::perform(
                            service::update_all(db, wow, ids_to_update, opts),
                            Message::UpdateAllResult,
                        );
                    }
                }
            }
            Message::UpdateAllResult(result) => {
                self.updating_all = false;
                match result {
                    Ok(results) => {
                        let mut applied = 0usize;
                        let mut failed = 0usize;
                        for r in &results {
                            for line in &r.log_lines {
                                let level = if r.error.is_some() { LogLevel::Error } else { LogLevel::Info };
                                self.log(level, line);
                            }
                            if r.error.is_some() {
                                failed += 1;
                            } else if r.plan.is_some() {
                                applied += 1;
                            }
                        }
                        if failed > 0 {
                            self.log(LogLevel::Error, &format!("Done. Updated {} repo(s); {} failed.", applied, failed));
                        } else {
                            self.log(LogLevel::Info, &format!("Done. Updated {} repo(s).", applied));
                        }
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
            Message::FetchBranchesResult((repo_id, result)) => {
                match result {
                    Ok(branch_list) => {
                        self.branches.insert(repo_id, branch_list);
                    }
                    Err(e) => {
                        let repo_name = self.repos.iter()
                            .find(|r| r.id == repo_id)
                            .map(|r| format!("{}/{}", r.owner, r.name))
                            .unwrap_or_else(|| format!("repo#{}", repo_id));
                        if !is_silenced_git_error(&e) {
                            self.log(LogLevel::Error, &format!("Failed to fetch branches for {}: {}", repo_name, simplify_git_error(&e)));
                        }
                    }
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
                    Err(e) => self.log(LogLevel::Error, &format!("Set branch failed: {}", simplify_git_error(&e))),
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
                    let active = self.profiles.iter()
                        .find(|p| p.id == self.active_profile_id)
                        .cloned()
                        .unwrap_or_default();
                    let cfg = service::LaunchConfig {
                        method: active.launch_method,
                        lutris_target: active.lutris_target,
                        wine_command: active.wine_command,
                        wine_args: active.wine_args,
                        custom_command: active.custom_command,
                        custom_args: active.custom_args,
                        clear_wdb: active.clear_wdb,
                    };
                    self.log(LogLevel::Info, &format!(
                        "Launching game (method: {})...", cfg.method
                    ));
                    let wow = self.wow_dir.clone();
                    return Task::perform(
                        service::launch_game(wow, cfg),
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

            // --- DXVK config dialog ---
            Message::OpenDxvkConfig => {
                self.open_menu = None;
                self.add_new_menu_open = false;
                self.dialog = Some(Dialog::DxvkConfig { config: DxvkConfig::default(), show_preview: false });
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
                if let Some(Dialog::DxvkConfig { ref config, show_preview: true }) = self.dialog {
                    let text = panels::dxvk_config::generate_conf(config);
                    self.dxvk_preview_content = iced::widget::text_editor::Content::with_text(&text);
                }
            }
            Message::SaveDxvkConfig => {
                if let Some(Dialog::DxvkConfig { ref config, .. }) = self.dialog {
                    let content = panels::dxvk_config::generate_conf(config);
                    let path = std::path::Path::new(&self.wow_dir).join("dxvk.conf");
                    return Task::perform(
                        service::save_dxvk_conf(path, content),
                        Message::DxvkConfigSaved,
                    );
                }
            }
            Message::DxvkConfigSaved(result) => {
                match result {
                    Ok(()) => {
                        let path = std::path::Path::new(&self.wow_dir).join("dxvk.conf");
                        self.log(LogLevel::Info, &format!("Saved dxvk.conf → {}", path.display()));
                        self.dialog = None;
                    }
                    Err(e) => self.log(LogLevel::Error, &format!("Failed to save dxvk.conf: {}", e)),
                }
            }
            Message::ToggleDxvkPreview => {
                if let Some(Dialog::DxvkConfig { ref mut show_preview, ref config }) = self.dialog {
                    *show_preview = !*show_preview;
                    if *show_preview {
                        let text = panels::dxvk_config::generate_conf(config);
                        self.dxvk_preview_content = iced::widget::text_editor::Content::with_text(&text);
                    }
                }
            }
            Message::DxvkPreviewEditorAction(action) => {
                if !action.is_edit() {
                    self.dxvk_preview_content.perform(action);
                }
            }

            // Tweak read/apply/restore
            Message::ReadTweaks => {
                if self.wow_dir.is_empty() {
                    self.log(LogLevel::Error, "Set a WoW directory in Options first.");
                } else {
                    self.log(LogLevel::Info, "Reading tweak values from WoW.exe...");
                    let wow = self.wow_dir.clone();
                    return Task::perform(service::read_tweaks(wow), Message::ReadTweaksResult);
                }
            }
            Message::ReadTweaksResult(result) => {
                match result {
                    Ok(vals) => {
                        self.tweak_values.fov = vals.fov;
                        self.tweak_values.farclip = vals.farclip;
                        self.tweak_values.frilldistance = vals.frilldistance;
                        self.tweak_values.nameplate_dist = vals.nameplate_distance;
                        self.tweak_values.max_camera_dist = vals.max_camera_distance;
                        self.tweak_values.sound_channels = vals.sound_channels;
                        self.tweaks.quickloot = vals.quickloot;
                        self.tweaks.sound_bg = vals.sound_in_background;
                        self.tweaks.large_address = vals.large_address_aware;
                        self.tweaks.camera_skip = vals.camera_skip_fix;
                        self.log(LogLevel::Info, "Tweak values read from WoW.exe.");
                    }
                    Err(e) => self.log(LogLevel::Error, &format!("Read tweaks failed: {}", e)),
                }
            }
            Message::ApplyTweaks => {
                if self.wow_dir.is_empty() {
                    self.log(LogLevel::Error, "Set a WoW directory in Options first.");
                } else {
                    self.log(LogLevel::Info, "Applying tweaks to WoW.exe...");
                    let wow = self.wow_dir.clone();
                    let tv = &self.tweak_values;
                    let ts = &self.tweaks;
                    let opts = tweaks::TweakOptions {
                        fov:               if ts.fov { Some(tv.fov) } else { None },
                        farclip:           if ts.farclip { Some(tv.farclip) } else { None },
                        frilldistance:     if ts.frilldistance { Some(tv.frilldistance) } else { None },
                        nameplate_distance:if ts.nameplate_dist { Some(tv.nameplate_dist) } else { None },
                        sound_channels:    if ts.sound_channels { Some(tv.sound_channels) } else { None },
                        max_camera_distance: if ts.max_camera_dist { Some(tv.max_camera_dist) } else { None },
                        quickloot:          ts.quickloot,
                        sound_in_background:ts.sound_bg,
                        large_address_aware:ts.large_address,
                        camera_skip_fix:    ts.camera_skip,
                    };
                    return Task::perform(service::apply_tweaks(wow, opts), Message::ApplyTweaksResult);
                }
            }
            Message::ApplyTweaksResult(result) => {
                match result {
                    Ok(msg) => self.log(LogLevel::Info, &msg),
                    Err(e) => self.log(LogLevel::Error, &format!("Apply tweaks failed: {}", e)),
                }
            }
            Message::RestoreTweaks => {
                if self.wow_dir.is_empty() {
                    self.log(LogLevel::Error, "Set a WoW directory in Options first.");
                } else {
                    self.log(LogLevel::Info, "Restoring WoW.exe from backup...");
                    let wow = self.wow_dir.clone();
                    return Task::perform(service::restore_tweaks(wow), Message::RestoreTweaksResult);
                }
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
                return Task::perform(service::check_self_update(self.update_channel == UpdateChannel::Beta), Message::CheckSelfUpdateResult);
            }
            Message::CheckSelfUpdateResult(result) => {
                match result {
                    Ok(latest) => {
                        let current = env!("CARGO_PKG_VERSION");
                        let msg = if latest == current || latest.trim_start_matches('v') == current {
                            "Up to date".to_string()
                        } else {
                            format!("Update available: v{}", latest.trim_start_matches('v'))
                        };
                        self.latest_version = Some(latest.trim_start_matches('v').to_string());
                        self.update_message = Some(msg.clone());
                        self.log(LogLevel::Info, &format!("Version check: {}", msg));
                    }
                    Err(e) => self.log(LogLevel::Error, &format!("Version check failed: {}", e)),
                }
            }
            Message::ShowChangelog => {
                self.dialog = Some(Dialog::Changelog { items: Vec::new(), loading: true });
                return Task::perform(service::fetch_changelog(), Message::ChangelogLoaded);
            }
            Message::ChangelogLoaded(result) => {
                if let Some(Dialog::Changelog { ref mut items, ref mut loading }) = self.dialog {
                    *loading = false;
                    let text = result.unwrap_or_else(|e| format!("Failed to load changelog: {}", e));
                    *items = iced::widget::markdown::Content::parse(&text).items().to_vec();
                }
            }

            // --- Add-repo preview ---
            Message::SetAddRepoUrl(url) => {
                // Update dialog URL field
                if let Some(Dialog::AddRepo { url: ref mut u, .. }) = self.dialog {
                    *u = url.clone();
                }
                // Trigger preview fetch if the URL resolves to a known forge
                let trimmed = url.trim().to_string();
                if service::parse_forge_url(&trimmed).is_some() {
                    self.add_repo_preview_loading = true;
                    return Task::perform(
                        service::fetch_repo_preview(trimmed),
                        Message::FetchRepoPreviewResult,
                    );
                } else {
                    self.add_repo_preview = None;
                    self.add_repo_preview_loading = false;
                }
            }
            Message::FetchRepoPreview(url) => {
                self.add_repo_preview_loading = true;
                return Task::perform(
                    service::fetch_repo_preview(url),
                    Message::FetchRepoPreviewResult,
                );
            }
            Message::FetchRepoPreviewResult(result) => {
                self.add_repo_preview_loading = false;
                match result {
                    Ok(info) => {
                        // Build selectable source content from raw readme text
                        self.readme_editor_content = iced::widget::text_editor::Content::with_text(&info.readme_text);
                        self.readme_source_view = false;
                        self.add_repo_preview = Some(info);
                        // Reset all per-preview state when a new preview loads
                        self.add_repo_release_notes = None;
                        self.add_repo_show_releases = false;
                        self.add_repo_file_preview = None;
                        self.add_repo_expanded_dirs.clear();
                        self.add_repo_dir_contents.clear();
                    }
                    Err(_) => self.add_repo_preview = None,
                }
            }

            Message::ToggleAddRepoDir(path) => {
                if self.add_repo_expanded_dirs.contains(&path) {
                    self.add_repo_expanded_dirs.remove(&path);
                } else {
                    self.add_repo_expanded_dirs.insert(path.clone());
                    // Lazily fetch this directory's contents if not yet loaded
                    if !self.add_repo_dir_contents.contains_key(&path) {
                        if let Some(ref preview) = self.add_repo_preview {
                            let forge_url = preview.forge_url.clone();
                            return Task::perform(
                                service::fetch_dir_contents(forge_url, path),
                                Message::FetchDirContentsResult,
                            );
                        }
                    }
                }
            }
            Message::FetchDirContents(forge_url, path) => {
                return Task::perform(
                    service::fetch_dir_contents(forge_url, path),
                    Message::FetchDirContentsResult,
                );
            }
            Message::FetchDirContentsResult(result) => {
                if let Ok((dir_path, entries)) = result {
                    let mut sorted = entries;
                    sorted.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
                    self.add_repo_dir_contents.insert(dir_path, sorted);
                }
            }

            Message::FetchReleaseNotes => {
                if self.add_repo_release_notes.is_some() {
                    // Already fetched — just switch view
                    self.add_repo_show_releases = true;
                } else if let Some(ref preview) = self.add_repo_preview {
                    let url = preview.forge_url.clone();
                    self.add_repo_show_releases = true;
                    return Task::perform(
                        service::fetch_releases(url),
                        Message::FetchReleaseNotesResult,
                    );
                }
            }
            Message::FetchReleaseNotesResult(result) => {
                match result {
                    Ok(releases) => self.add_repo_release_notes = Some(releases),
                    Err(e) => {
                        self.add_repo_show_releases = false;
                        self.log(LogLevel::Error, &format!("Failed to fetch releases: {}", e));
                    }
                }
            }
            Message::ShowReadme => {
                self.add_repo_show_releases = false;
                self.add_repo_file_preview = None;
            }

            Message::PreviewRepoFile(path) => {
                if let Some(ref preview) = self.add_repo_preview {
                    let raw_base = preview.raw_base_url.clone();
                    return Task::perform(
                        service::fetch_raw_file(raw_base, path),
                        Message::PreviewRepoFileResult,
                    );
                }
            }
            Message::PreviewRepoFileResult(result) => {
                match result {
                    Ok((path, content)) => self.add_repo_file_preview = Some((path, content)),
                    Err(e) => self.add_repo_file_preview = Some(("Error".to_string(), e)),
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
            Message::SetUpdateChannel(ch) => {
                self.update_channel = ch;
                self.save_settings();
            }

            Message::SwitchToStableChannel => {
                if !switch_to_stable_channel() {
                    let _ = open::that("https://github.com/ZythDr/Wuddle/releases");
                }
            }
        }
        Task::none()
    }

    fn install_options(&self) -> wuddle_engine::InstallOptions {
        wuddle_engine::InstallOptions {
            use_symlinks: self.opt_symlinks,
            set_xattr_comment: self.opt_xattr,
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

            // Two-card layout for AddRepo with a loaded preview
            let has_two_cards = matches!(dialog, Dialog::AddRepo { .. })
                && self.add_repo_preview.is_some();

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
            let dialog_blocker = iced::widget::mouse_area(dialog_box)
                .on_press(Message::CloseMenu);

            let scrim = iced::widget::mouse_area(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|_theme| theme::scrim_style()),
            )
            .on_press(Message::CloseDialog);

            // 40px margin on all sides ≈ 90% of a typical window
            let centered_dialog = container(dialog_blocker)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .padding(40);

            // Use opaque() to make the entire overlay absorb ALL mouse events,
            // preventing any interaction with main_content while the dialog is open.
            let overlay = iced::widget::opaque(
                stack![scrim, centered_dialog]
                    .width(Length::Fill)
                    .height(Length::Fill),
            );

            stack![main_content, overlay]
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
            Dialog::AddRepo { url, mode, is_addons, advanced } => {
                let is_addons = *is_addons;
                let advanced = *advanced;
                let title = if is_addons { "Add an addon repo" } else { "Add a mod repo" };
                let subtitle = if is_addons {
                    "Paste a Git repository URL below. Wuddle will automatically download and install the addon for you."
                } else {
                    "Quick-add from the mods listed, or add your own Git repo URL below."
                };
                let url_label = if is_addons { "Addon Repo URL" } else { "Repo URL" };
                let placeholder = if is_addons {
                    "(e.g. https://github.com/pepopo978/BigWigs)"
                } else {
                    "(e.g. https://gitea.com/avitasia/nampower)"
                };

                // --- URL input with inline clear (✕) button ---
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
                        .padding(iced::Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 0.0 }),
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
                    }.into_iter().map(String::from).collect();

                    let url_cb = url.clone();
                    let mode_cb = mode.clone();
                    let url_pl = url.clone();

                    let advanced_section: Element<Message> = if advanced {
                        let picked = Some(mode.clone());
                        row![
                            iced::widget::checkbox(advanced)
                                .label("Advanced")
                                .on_toggle(move |val| Message::OpenDialog(Dialog::AddRepo {
                                    url: url_cb.clone(), mode: mode_cb.clone(),
                                    is_addons, advanced: val,
                                })),
                            text("Mode").size(12).color(c.muted),
                            iced::widget::pick_list(mode_list, picked, move |m: String| {
                                Message::OpenDialog(Dialog::AddRepo {
                                    url: url_pl.clone(), mode: m,
                                    is_addons, advanced: true,
                                })
                            }).text_size(12).padding([4, 8]),
                        ]
                        .spacing(6)
                        .align_y(iced::Alignment::Center)
                        .into()
                    } else {
                        let url_c2 = url.clone();
                        let mode_c2 = mode.clone();
                        iced::widget::checkbox(advanced)
                            .label("Advanced")
                            .on_toggle(move |val| Message::OpenDialog(Dialog::AddRepo {
                                url: url_c2.clone(), mode: mode_c2.clone(),
                                is_addons, advanced: val,
                            }))
                            .into()
                    };

                    // Forge link button: "Open on [forge icon]" (shown when preview is loaded)
                    let forge_link: Option<Element<Message>> = self.add_repo_preview.as_ref().map(|p| {
                        let furl = p.forge_url.clone();
                        let icon_handle = forge_svg_handle(&p.forge, &p.forge_url);
                        let icon_color = c.text;
                        button(
                            row![
                                text("Open on").size(12).color(c.text)
                                    .line_height(iced::widget::text::LineHeight::Relative(1.0)),
                                iced::widget::svg(icon_handle)
                                    .width(14)
                                    .height(14)
                                    .style(move |_t, _s| iced::widget::svg::Style {
                                        color: Some(icon_color),
                                    }),
                            ]
                            .spacing(5)
                            .align_y(iced::Alignment::Center)
                        )
                        .on_press(Message::OpenUrl(furl))
                        .padding([6, 10])
                        .style(move |_t, s| match s {
                            button::Status::Hovered => theme::tab_button_hovered_style(&c),
                            _ => theme::tab_button_style(&c),
                        })
                        .into()
                    });

                    // Release Notes / README toggle button (shown when preview is loaded)
                    let release_notes: Option<Element<Message>> = self.add_repo_preview.as_ref().map(|_p| {
                        let (label, msg) = if self.add_repo_show_releases {
                            ("README", Message::ShowReadme)
                        } else {
                            ("Release Notes", Message::FetchReleaseNotes)
                        };
                        button(text(label).size(12))
                            .on_press(msg)
                            .padding([6, 10])
                            .style(move |_t, s| match s {
                                button::Status::Hovered => theme::tab_button_hovered_style(&c),
                                _ => theme::tab_button_style(&c),
                            })
                            .into()
                    });

                    let mut footer_row: Vec<Element<Message>> = Vec::new();
                    if let Some(fl) = forge_link { footer_row.push(fl); }
                    if let Some(rn) = release_notes { footer_row.push(rn); }
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
                            .into()
                    );
                    footer_row.push(
                        button(text(if is_addons { "Add addon" } else { "Add mod" }).size(13))
                            .on_press(Message::AddRepoSubmit)
                            .padding([6, 14])
                            .style(move |_t, _s| theme::tab_button_active_style(&c))
                            .into()
                    );
                    row(footer_row).spacing(8).align_y(iced::Alignment::Center).into()
                };

                if let Some(ref preview) = self.add_repo_preview {
                    // =========================================================
                    // TWO-CARD LAYOUT: floating side panel + main form card
                    // =========================================================
                    let current_theme = self.theme();
                    let c_sp = c;
                    let c_form = c;
                    let c_divider = c;

                    // --- SIDE PANEL CARD (About + Files) ---
                    let mut sidebar_col: Vec<Element<Message>> = Vec::new();

                    // About section header
                    sidebar_col.push(text("About").size(12).color(colors.muted).into());
                    sidebar_col.push(text(&preview.name).size(15).color(colors.text).into());
                    if !preview.description.is_empty() {
                        sidebar_col.push(
                            text(&preview.description).size(12).color(colors.text_soft).into()
                        );
                    }

                    // Stats
                    sidebar_col.push(
                        row![
                            text("\u{2b50}").size(12),  // ⭐
                            text(format!("{} star{}", preview.stars, if preview.stars == 1 { "" } else { "s" }))
                                .size(13).color(colors.text_soft),
                        ].spacing(4).into()
                    );
                    if preview.forks > 0 {
                        let forks_url = format!("{}/forks", preview.forge_url);
                        let c_fk = c;
                        let fork_count = preview.forks;
                        sidebar_col.push(
                            row![
                                text("\u{1f374}").size(12),  // 🍴
                                button(
                                    iced::widget::rich_text::<(), _, _, _>([
                                        iced::widget::span(format!(
                                            "{} fork{}",
                                            fork_count,
                                            if fork_count == 1 { "" } else { "s" }
                                        ))
                                        .underline(true)
                                        .color(c_fk.link)
                                        .size(13.0_f32),
                                    ])
                                )
                                .on_press(Message::OpenUrl(forks_url))
                                .padding(0)
                                .style(move |_t, _s| button::Style {
                                    background: None,
                                    text_color: c_fk.link,
                                    border: iced::Border::default(),
                                    shadow: iced::Shadow::default(),
                                    snap: true,
                                }),
                            ].spacing(4).align_y(iced::Alignment::Center).into()
                        );
                    }
                    if !preview.language.is_empty() {
                        sidebar_col.push(
                            row![
                                text("\u{1f4bb}").size(12),  // 💻
                                text(&preview.language).size(12).color(colors.text_soft),
                            ].spacing(4).into()
                        );
                    }
                    if !preview.license.is_empty() {
                        sidebar_col.push(
                            row![
                                text("\u{1f4cb}").size(12),  // 📋
                                text(&preview.license).size(12).color(colors.text_soft),
                            ].spacing(4).into()
                        );
                    }

                    // Files section
                    if !preview.files.is_empty() {
                        sidebar_col.push(
                            rule::horizontal(1)
                                .style(move |_t| theme::update_line_style(&c_divider))
                                .into()
                        );
                        sidebar_col.push(text("Files").size(12).color(colors.muted).into());

                        let mut sorted_files = preview.files.clone();
                        sorted_files.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));

                        let mut file_rows: Vec<Element<Message>> = Vec::new();
                        for f in sorted_files.iter().take(60) {
                            let c_tree = c;
                            let path = f.path.clone();
                            if f.is_dir {
                                let expanded = self.add_repo_expanded_dirs.contains(&f.path);
                                let folder_icon = if expanded { "\u{1f4c2}" } else { "\u{1f4c1}" }; // 📂 / 📁
                                file_rows.push(
                                    button(
                                        text(format!("{} {}", folder_icon, f.name))
                                            .size(12).color(colors.text)
                                    )
                                    .on_press(Message::ToggleAddRepoDir(path.clone()))
                                    .padding([2, 4])
                                    .style(move |_t, status| match status {
                                        button::Status::Hovered => button::Style {
                                            background: Some(iced::Background::Color(
                                                iced::Color::from_rgba(1.0, 1.0, 1.0, 0.07)
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
                                    .into()
                                );
                                if expanded {
                                    if let Some(children) = self.add_repo_dir_contents.get(&f.path) {
                                        for child in children.iter().take(40) {
                                            let c_ch = c;
                                            let child_path = child.path.clone();
                                            let child_icon = if child.is_dir { "\u{1f4c1}" } else { "\u{1f4c4}" };
                                            let child_msg = if child.is_dir {
                                                Message::ToggleAddRepoDir(child_path.clone())
                                            } else {
                                                Message::PreviewRepoFile(child_path.clone())
                                            };
                                            let child_color = if child.is_dir { colors.text } else { colors.text_soft };
                                            file_rows.push(
                                                button(
                                                    row![
                                                        Space::new().width(14),
                                                        text(format!("{} {}", child_icon, child.name))
                                                            .size(11).color(child_color),
                                                    ]
                                                )
                                                .on_press(child_msg)
                                                .padding([1, 4])
                                                .style(move |_t, status| match status {
                                                    button::Status::Hovered => button::Style {
                                                        background: Some(iced::Background::Color(
                                                            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.07)
                                                        )),
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
                                                .into()
                                            );
                                        }
                                    } else {
                                        file_rows.push(
                                            row![
                                                Space::new().width(18),
                                                text("Loading…").size(11).color(colors.muted),
                                            ].into()
                                        );
                                    }
                                }
                            } else {
                                file_rows.push(
                                    button(
                                        text(format!("\u{1f4c4} {}", f.name))
                                            .size(12).color(colors.text_soft)
                                    )
                                    .on_press(Message::PreviewRepoFile(path))
                                    .padding([2, 4])
                                    .style(move |_t, status| match status {
                                        button::Status::Hovered => button::Style {
                                            background: Some(iced::Background::Color(
                                                iced::Color::from_rgba(1.0, 1.0, 1.0, 0.07)
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
                                    .into()
                                );
                            }
                        }

                        // Files fill remaining sidebar height
                        sidebar_col.push(
                            iced::widget::scrollable(column(file_rows).spacing(1).width(Length::Fill))
                                .width(Length::Fill)
                                .height(Length::Fill)
                                .direction(theme::vscroll())
                                .style(move |t, s| theme::scrollable_style(&c_sp)(t, s))
                                .into()
                        );
                    }

                    let sidebar_card = container(
                        column(sidebar_col).spacing(6).width(Length::Fill).height(Length::Fill)
                    )
                    .width(280)
                    .height(Length::Fill)
                    .padding([16, 14])
                    .style(move |_theme| theme::dialog_style(&c_sp));

                    // --- MAIN FORM CARD ---
                    let content_label: Element<Message> = if let Some((ref fname, _)) = self.add_repo_file_preview {
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
                        text("Release Notes").size(12).color(colors.muted).into()
                    } else {
                        text("README").size(12).color(colors.muted).into()
                    };

                    let scrollable_content: Element<Message> = if let Some((_, ref content)) = self.add_repo_file_preview {
                        // Show file content (plain text, monospace)
                        let inner_content = container(
                            iced::widget::scrollable(
                                text(content.as_str()).size(12).font(Font::MONOSPACE)
                                    .color(colors.text)
                            )
                            .height(Length::Fill)
                            .direction(theme::vscroll())
                            .style(move |t, s| theme::scrollable_style(&c_form)(t, s))
                        )
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .padding(8)
                        .style(move |_t| container::Style {
                            background: Some(iced::Background::Color(
                                iced::Color::from_rgba(0.0, 0.0, 0.0, 0.38)
                            )),
                            border: iced::Border {
                                color: c_form.border,
                                width: 1.0,
                                radius: iced::border::Radius::from(6),
                            },
                            ..Default::default()
                        });
                        inner_content.into()
                    } else if self.add_repo_show_releases {
                        // Show release notes or loading indicator
                        if let Some(ref releases) = self.add_repo_release_notes {
                            if releases.is_empty() {
                                container(
                                    text("No releases found.").size(13).color(colors.muted)
                                )
                                .padding([8, 0])
                                .into()
                            } else {
                                let c_rl = c_form;
                                let rn_theme = self.theme();
                                let mut rn_style = iced::widget::markdown::Style::from(&rn_theme);
                                rn_style.link_color = c_rl.link;
                                let rn_settings = iced::widget::markdown::Settings::with_text_size(
                                    12,
                                    rn_style,
                                );
                                let release_cards: Vec<Element<Message>> = releases.iter().map(|r| {
                                    let date = r.published_at.get(..10).unwrap_or(&r.published_at);
                                    let mut col_items: Vec<Element<Message>> = vec![
                                        row![
                                            text(&r.name).size(14).color(colors.text),
                                            Space::new().width(Length::Fill),
                                            text(date).size(11).color(colors.muted),
                                        ].align_y(iced::Alignment::Center).into(),
                                    ];
                                    if r.tag_name != r.name && !r.tag_name.is_empty() {
                                        col_items.push(
                                            text(&r.tag_name).size(11).color(colors.muted).into()
                                        );
                                    }
                                    if r.prerelease {
                                        col_items.push(badge_tag("pre-release", iced::Color::from_rgb8(0xfd, 0xe6, 0x8a), iced::Color::from_rgb8(0xd4, 0x82, 0x1a)));
                                    }
                                    if !r.items.is_empty() {
                                        col_items.push(
                                            iced::widget::markdown::view(&r.items, rn_settings)
                                                .map(Message::OpenUrl)
                                                .into()
                                        );
                                    }
                                    container(column(col_items).spacing(3))
                                        .width(Length::Fill)
                                        .padding([8, 12])
                                        .style(move |_t| theme::card_style(&c_rl))
                                        .into()
                                }).collect();
                                iced::widget::scrollable(
                                    column(release_cards).spacing(6).width(Length::Fill)
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
                                    canvas(SpinnerCanvas { tick, color: primary }).width(18).height(18),
                                    text("Resolving...").size(13).color(colors.muted),
                                ]
                                .spacing(8)
                                .align_y(iced::Alignment::Center)
                            )
                            .padding([8, 0])
                            .into()
                        }
                    } else {
                        // Show README (or placeholder if empty)
                        let readme_is_source = self.readme_source_view;
                        let inner_scrollable: Element<Message> = if preview.readme_items.is_empty() {
                            container(
                                column![
                                    text("\u{1f4c4}").size(32),
                                    text("No README found for this repository.")
                                        .size(13).color(colors.muted),
                                ]
                                .spacing(8)
                                .align_x(iced::Alignment::Center)
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
                                    background: iced::Background::Color(iced::Color::from_rgb8(0x0d, 0x11, 0x1a)),
                                    border: iced::Border::default(),
                                    placeholder: iced::Color::from_rgb8(0x4a, 0x55, 0x68),
                                    value: iced::Color::from_rgb8(0xdb, 0xe7, 0xff),
                                    selection: iced::Color { a: 0.35, ..iced::Color::from_rgb8(0x4a, 0x90, 0xd9) },
                                })
                                .into()
                        } else {
                            let viewer = ImageViewer {
                                cache: &preview.image_cache,
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
                                border: iced::Border { color: iced::Color::from_rgb8(0x2a, 0x2f, 0x3d), width: 1.0, radius: 3.0.into() },
                            };
                            let mut md_settings = iced::widget::markdown::Settings::with_text_size(
                                13,
                                md_style,
                            );
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
                        // Source toggle button overlaid at top-right of readme area
                        let source_label = if readme_is_source { "Formatted" } else { "Source" };
                        let source_btn = {
                            let c2 = c_form;
                            button(text(source_label).size(11))
                                .on_press(Message::ToggleReadmeSourceView)
                                .padding([3, 8])
                                .style(move |_theme, status| match status {
                                    button::Status::Hovered => theme::tab_button_hovered_style(&c2),
                                    _ => theme::tab_button_style(&c2),
                                })
                        };
                        let readme_area = column![
                            container(source_btn)
                                .width(Length::Fill)
                                .align_x(iced::Alignment::End)
                                .padding(iced::Padding { top: 4.0, right: 4.0, bottom: 2.0, left: 4.0 }),
                            inner_scrollable,
                        ]
                        .width(Length::Fill)
                        .height(Length::Fill);
                        container(readme_area)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .padding(8)
                        .style(move |_t| container::Style {
                            background: Some(iced::Background::Color(
                                iced::Color::from_rgba(0.0, 0.0, 0.0, 0.38)
                            )),
                            border: iced::Border {
                                color: c_form.border,
                                width: 1.0,
                                radius: iced::border::Radius::from(6),
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
                            ].align_y(iced::Alignment::Center),
                            text(subtitle).size(12).color(colors.text_soft),
                            rule::horizontal(1).style(move |_t| theme::update_line_style(&c_form)),
                            text(url_label).size(12).color(colors.text),
                            url_row,
                            content_label,
                            // Scrollable content fills remaining space
                            scrollable_content,
                            rule::horizontal(1).style(move |_t| theme::update_line_style(&c_form)),
                            footer,
                        ]
                        .spacing(6)
                        .height(Length::Fill)
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
                    // SINGLE-CARD LAYOUT: no preview — show Quick Add or loading
                    // =========================================================
                    let body_content: Element<Message> = if self.add_repo_preview_loading {
                        let tick = self.spinner_tick;
                        let primary = colors.primary;
                        container(
                            row![
                                canvas(SpinnerCanvas { tick, color: primary }).width(18).height(18),
                                text("Resolving...").size(13).color(colors.muted),
                            ]
                            .spacing(8)
                            .align_y(iced::Alignment::Center)
                        )
                        .padding([12, 0])
                        .into()
                    } else if !is_addons && url.trim().is_empty() {
                        // Quick Add preset list (mods tab only, when no URL entered)
                        build_quick_add_presets(&self.repos, colors)
                    } else {
                        Space::new().height(0).into()
                    };

                    let body_section: Element<Message> = if self.add_repo_preview_loading || (!is_addons && url.trim().is_empty()) {
                        let section_label: Element<Message> = if is_addons { Space::new().height(0).into() } else {
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
                        ].align_y(iced::Alignment::Center),
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
                    .height(if !is_addons && url.trim().is_empty() { Length::Fill } else { Length::Shrink })
                    .into()
                }
            }
            Dialog::Changelog { items, loading } => {
                let body: Element<Message> = if *loading {
                    container(text("Loading changelog…").size(13).color(colors.muted))
                        .center_x(Length::Fill)
                        .center_y(Length::Fill)
                        .width(Length::Fill)
                        .height(Length::Fixed(300.0))
                        .into()
                } else {
                    let mut cl_style = iced::widget::markdown::Style::from(&self.theme());
                    cl_style.link_color = c.link;
                    let md_settings = iced::widget::markdown::Settings::with_text_size(
                        13,
                        cl_style,
                    );
                    iced::widget::scrollable(
                        iced::widget::markdown::view(items, md_settings)
                            .map(Message::OpenUrl),
                    )
                    .height(Length::Fixed(480.0))
                    .direction(theme::vscroll())
                    .style(move |t, s| theme::scrollable_style(&c)(t, s))
                    .into()
                };
                column![
                    row![
                        text("Changelog").size(18).color(colors.title),
                        Space::new().width(Length::Fill),
                        close_button(&c),
                    ].align_y(iced::Alignment::Center),
                    body,
                ]
                .spacing(12)
                .width(Length::Fixed(700.0))
                .into()
            }
            Dialog::RemoveRepo { id, name, remove_files, files } => {
                let rid = *id;
                let rf = *remove_files;

                // File tree preview
                let file_rows: Vec<Element<Message>> = files.iter().map(|(path, kind)| {
                    let icon = match kind.as_str() {
                        "dll"   => "\u{2699}",  // ⚙
                        "addon" => "\u{1f4c1}", // 📁
                        _       => "\u{1f4c4}", // 📄
                    };
                    let color = if rf { colors.warn } else { colors.text_soft };
                    container(
                        text(format!("{} {}", icon, path))
                            .size(12)
                            .color(color)
                    )
                    .padding([2, 6])
                    .into()
                }).collect();

                let file_tree: Element<Message> = if files.is_empty() {
                    text("No tracked files found.").size(12).color(colors.muted).into()
                } else {
                    scrollable(
                        column(file_rows).spacing(0).width(Length::Fill)
                    )
                    .height(iced::Length::Fixed(160.0))
                    .direction(theme::vscroll_overlay())
                    .style(move |t, s| theme::scrollable_style(&c)(t, s))
                    .into()
                };

                let file_section: Element<Message> = container(file_tree)
                    .width(Length::Fill)
                    .padding([6, 0])
                    .style(move |_t| container::Style {
                        background: Some(iced::Background::Color(
                            iced::Color { a: 0.5, ..c.card }
                        )),
                        border: iced::Border {
                            color: iced::Color { a: 0.15, ..c.border },
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
                        "⚠ Installed files will be permanently deleted from your WoW directory."
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
                        button(text("Remove").size(13).color(c.bad))
                            .on_press(Message::RemoveRepoConfirm(rid, rf))
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
                let can_remove = !*is_new;
                let is_active_profile = *profile_id == self.active_profile_id;
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
                                    container(text("Cannot remove the active instance").size(11).color(c2.text))
                                        .padding([4, 8])
                                        .style(move |_theme| theme::tooltip_style(&c2)),
                                    iced::widget::tooltip::Position::Top,
                                )
                                .into()
                            } else {
                                button(text("Remove").size(13).color(c.bad))
                                    .on_press(Message::RemoveProfile(remove_id))
                                    .padding([6, 14])
                                    .style(move |_theme, _status| {
                                        let mut s = theme::tab_button_style(&c2);
                                        s.border.color = c2.bad;
                                        s
                                    })
                                    .into()
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
            Dialog::DxvkConfig { config, show_preview } => {
                panels::dxvk_config::view(config, &self.wow_dir, *show_preview, &self.dxvk_preview_content, colors)
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
            .align_y(iced::Alignment::Center);

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

        let is_icon = matches!(tab, Tab::Options | Tab::Logs);
        // About uses its Unicode ⓘ glyph — compact width like SVG icon tabs
        let is_unicode_icon = tab == Tab::About;

        let content: Element<Message> = if is_icon {
            let icon_color = if is_active { c.primary_text } else { c.text };
            container(
                iced::widget::svg(tab_icon_svg(tab))
                    .width(17)
                    .height(17)
                    .style(move |_t, _s| iced::widget::svg::Style { color: Some(icon_color) })
            )
            .width(Length::Fill)
            .center_x(Length::Fill)
            .into()
        } else if is_unicode_icon {
            let icon_color = if is_active { c.primary_text } else { c.text };
            container(
                text(tab.icon_label()).size(17).color(icon_color).line_height(1.0),
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
            .width(if is_icon || is_unicode_icon { Length::Fixed(32.0) } else { Length::Fixed(114.0) });

        let styled_btn: Element<Message> = if is_active {
            btn.style(move |_theme, _status| theme::tab_button_active_style(&c)).into()
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
                container(text(tab.tooltip()).size(11).color(c.text))
                    .padding([3, 8])
                    .style(move |_theme| theme::tooltip_style(&c)),
                iced::widget::tooltip::Position::Bottom,
            )
            .into()
        } else {
            styled_btn
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

        let hint: Element<Message> = if self.wow_dir.is_empty() {
            text("No WoW directory set. Go to Options to configure.")
                .size(12).color(colors.warn).into()
        } else {
            let active = self.profiles.iter()
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
                    ("Launch Mode: Lutris".to_string(), format!("Target: {}", target))
                }
                "wine" => {
                    let cmd = if active.wine_command.trim().is_empty() { "wine".to_string() } else { active.wine_command.clone() };
                    ("Launch Mode: Wine".to_string(), format!("Command: {}", cmd))
                }
                "custom" => {
                    let cmd = if active.custom_command.trim().is_empty() { "(no command set)".to_string() } else { active.custom_command.clone() };
                    ("Launch Mode: Custom".to_string(), format!("Command: {}", cmd))
                }
                _ => (
                    "Launch Mode: Auto".to_string(),
                    "Launches VanillaFixes.exe if present, otherwise Wow.exe".to_string(),
                ),
            };
            let tooltip_content = container(
                text(tooltip_detail).size(11).color(colors.text)
            )
            .padding([6, 10]);
            iced::widget::tooltip(
                text(mode_label).size(12).color(colors.muted),
                tooltip_content,
                iced::widget::tooltip::Position::Top,
            )
            .style(move |_t| theme::tooltip_style(&c))
            .into()
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

/// Returns the font for project names: Bold when using Noto Sans, Regular when using Friz
/// (Friz Quadrata only ships Regular weight; requesting Bold causes a fallback to system font).
pub fn name_font(colors: &ThemeColors) -> Font {
    if colors.body_font == FRIZ {
        FRIZ
    } else {
        Font { weight: iced::font::Weight::Bold, ..colors.body_font }
    }
}

/// Quick Add preset data (mirrors wuddle-gui/src/presets.js)
struct Preset {
    name: &'static str,
    url: &'static str,
    description: &'static str,
    categories: &'static [&'static str],
    recommended: bool,
    warning: Option<&'static str>,
    companion_links: &'static [(&'static str, &'static str)],
    expanded_notes: &'static [&'static str],
}

static QUICK_ADD_PRESETS: &[Preset] = &[
    Preset {
        name: "VanillaFixes",
        url: "https://github.com/hannesmann/vanillafixes",
        description: "A client modification for World of Warcraft 1.6.1-1.12.1 to eliminate stutter and animation lag. VanillaFixes also acts as a launcher (start game via VanillaFixes.exe instead of Wow.exe) and DLL mod loader which loads DLL files listed in dlls.txt found in the WoW install directory.",
        categories: &["Performance"],
        recommended: true,
        warning: Some("VanillaFixes may trigger antivirus false-positive alerts on Windows."),
        companion_links: &[],
        expanded_notes: &[],
    },
    Preset {
        name: "Interact",
        url: "https://github.com/lookino/Interact",
        description: "Legacy WoW client mod for 1.12 that brings Dragonflight-style interact key support to Vanilla, reducing click friction and improving moment-to-moment gameplay.",
        categories: &["QoL"],
        recommended: false,
        warning: None,
        companion_links: &[],
        expanded_notes: &[],
    },
    Preset {
        name: "UnitXP_SP3",
        url: "https://codeberg.org/konaka/UnitXP_SP3",
        description: "Adds optional camera offset, proper nameplates (showing only with LoS), improved tab-targeting keybind behavior, LoS and distance checks in Lua, screenshot format options, network tweaks, background notifications, and additional QoL features.",
        categories: &["QoL", "API"],
        recommended: true,
        warning: Some("UnitXP_SP3 may trigger antivirus false-positive alerts on Windows."),
        companion_links: &[],
        expanded_notes: &[],
    },
    Preset {
        name: "nampower",
        url: "https://gitea.com/avitasia/nampower",
        description: "Addresses a 1.12 client casting limitation where follow-up casts wait on round-trip completion feedback. The result is reduced cast downtime and better effective DPS, especially on higher-latency connections.",
        categories: &["API"],
        recommended: true,
        warning: None,
        companion_links: &[("nampowersettings", "https://gitea.com/avitasia/nampowersettings")],
        expanded_notes: &[],
    },
    Preset {
        name: "SuperWoW",
        url: "https://github.com/balakethelock/SuperWoW",
        description: "Client mod for WoW 1.12.1 that fixes engine/client bugs and expands the Lua API used by addons. Some addons require SuperWoW directly, and many others gain improved functionality when it is present.",
        categories: &["QoL", "API"],
        recommended: true,
        warning: Some("Known issue: SuperWoW will trigger antivirus false-positive alerts on Windows."),
        companion_links: &[
            ("SuperAPI", "https://github.com/balakethelock/SuperAPI"),
            ("SuperAPI_Castlib", "https://github.com/balakethelock/SuperAPI_Castlib"),
        ],
        expanded_notes: &[
            "SuperAPI improves compatibility with the default interface and adds a minimap icon for persistent mod settings.",
            "It exposes settings like autoloot, clickthrough corpses, GUID in combat log/events, adjustable FoV, enable background sound, uncapped sound channels, and targeting circle style.",
            "SuperAPI_Castlib adds default-style nameplate castbars. If you're using pfUI/shaguplates, you do not need this module.",
        ],
    },
    Preset {
        name: "DXVK (GPLAsync fork)",
        url: "https://gitlab.com/Ph42oN/dxvk-gplasync",
        description: "DXVK can massively improve performance in old Direct3D titles (including WoW 1.12) by using Vulkan. This fork includes Async + GPL options aimed at further reducing stutters. Async/GPL behavior is controlled through dxvk.conf, so users can keep default behavior if they prefer.",
        categories: &["Performance"],
        recommended: true,
        warning: None,
        companion_links: &[],
        expanded_notes: &[],
    },
    Preset {
        name: "perf_boost",
        url: "https://gitea.com/avitasia/perf_boost",
        description: "Performance-focused DLL for WoW 1.12.1 intended to improve FPS in crowded areas and raids. Uses advanced render-distance controls.",
        categories: &["Performance"],
        recommended: false,
        warning: None,
        companion_links: &[("PerfBoostSettings", "https://gitea.com/avitasia/PerfBoostSettings")],
        expanded_notes: &[],
    },
    Preset {
        name: "VanillaHelpers",
        url: "https://github.com/isfir/VanillaHelpers",
        description: "Utility library for WoW 1.12 adding file read/write helpers, minimap blip customization, larger allocator capacity, higher-resolution texture/skin support, and character morph-related functionality.",
        categories: &["API", "Performance"],
        recommended: true,
        warning: None,
        companion_links: &[],
        expanded_notes: &[],
    },
];

/// Build the Quick Add preset card list (shown when URL input is empty in mods dialog).
fn build_quick_add_presets<'a>(repos: &[RepoRow], colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;

    let cards: Vec<Element<Message>> = QUICK_ADD_PRESETS.iter().map(|preset| {
        let already_installed = repos.iter().any(|r| {
            r.url.trim_end_matches('/') == preset.url.trim_end_matches('/')
        });

        // Title link — clicking it fills the URL input (underlined blue link)
        let preset_url = preset.url.to_string();
        let title_btn = button(
            iced::widget::rich_text::<(), _, _, _>([
                iced::widget::span(preset.name)
                    .underline(true)
                    .color(c.link)
                    .size(14.0_f32),
            ])
        )
        .on_press(Message::SetAddRepoUrl(preset_url.clone()))
        .padding(0)
        .style(move |_t, _s| button::Style {
            background: None,
            text_color: c.link,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: true,
        });

        // Category/flag tags — colors match Tauri CSS variables exactly
        let mut tags: Vec<Element<Message>> = Vec::new();
        if preset.recommended {
            tags.push(badge_tag(
                "Recommended",
                iced::Color::from_rgb8(0x34, 0xd3, 0x99),
                iced::Color::from_rgb8(0x10, 0xb9, 0x81),
            ));
        }
        if preset.warning.is_some() {
            tags.push(badge_tag(
                "AV false-positive",
                iced::Color::from_rgb8(0xfc, 0xa5, 0xa5),
                iced::Color::from_rgb8(0xef, 0x44, 0x44),
            ));
        }
        for cat in preset.categories {
            let (text_col, base_col) = match *cat {
                "Performance" => (
                    iced::Color::from_rgb8(0xc4, 0xb5, 0xfd),
                    iced::Color::from_rgb8(0xa8, 0x55, 0xf7),
                ),
                "QoL" => (
                    iced::Color::from_rgb8(0x93, 0xc5, 0xfd),
                    iced::Color::from_rgb8(0x3b, 0x82, 0xf6),
                ),
                "API" => (
                    iced::Color::from_rgb8(0xfd, 0xe6, 0x8a),
                    iced::Color::from_rgb8(0xfa, 0xcc, 0x15),
                ),
                _ => (c.muted, c.muted),
            };
            tags.push(badge_tag(cat, text_col, base_col));
        }

        let tags_row = row(tags).spacing(4).align_y(iced::Alignment::Center);

        // Description + optional expanded bullet notes + optional warning
        let mut desc_col: Vec<Element<Message>> = vec![
            text(preset.description).size(12).color(c.text_soft).into(),
        ];
        // Expanded bullet notes (e.g. SuperWoW)
        for note in preset.expanded_notes {
            desc_col.push(
                row![
                    text("\u{2022}").size(11).color(c.text_soft),
                    text(*note).size(11).color(c.text_soft),
                ]
                .spacing(4)
                .into()
            );
        }
        if let Some(warn) = preset.warning {
            desc_col.push(
                text(warn).size(11).color(iced::Color::from_rgb8(0xfc, 0xa5, 0xa5)).into()
            );
        }
        // Companion links (blue underlined)
        if !preset.companion_links.is_empty() {
            let companions: Vec<Element<Message>> = preset.companion_links.iter().map(|(label, lurl)| {
                let l = lurl.to_string();
                button(
                    iced::widget::rich_text::<(), _, _, _>([
                        iced::widget::span(*label)
                            .underline(true)
                            .color(c.link)
                            .size(11.0_f32),
                    ])
                )
                .on_press(Message::OpenUrl(l))
                .padding(0)
                .style(move |_t, _s| button::Style {
                    background: None,
                    text_color: c.link,
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: true,
                })
                .into()
            }).collect();
            desc_col.push(
                row![
                    text("Companion addons:").size(11).color(c.muted),
                    row(companions).spacing(8),
                ].spacing(4).into()
            );
        }

        // Action button (Installed badge or Add button) — bottom-right of card
        let action_btn: Element<Message> = if already_installed {
            container(
                text("Installed").size(12).color(iced::Color::from_rgb8(0x34, 0xd3, 0x99))
            )
            .padding([4, 10])
            .style(move |_t| container::Style {
                background: Some(iced::Background::Color(
                    iced::Color::from_rgba8(0x10, 0xb9, 0x81, 0.15)
                )),
                border: iced::Border {
                    color: iced::Color::from_rgba8(0x10, 0xb9, 0x81, 0.4),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            })
            .into()
        } else {
            let pu = preset.url.to_string();
            button(text("Add").size(12))
                .on_press(Message::SetAddRepoUrl(pu))
                .padding([4, 14])
                .style(move |_t, _s| theme::tab_button_active_style(&c))
                .into()
        };

        // Assemble card — action button at bottom-right
        let card_content = column![
            row![title_btn, tags_row].spacing(8).align_y(iced::Alignment::Center),
            column(desc_col).spacing(3),
            row![Space::new().width(Length::Fill), action_btn]
                .align_y(iced::Alignment::Center),
        ]
        .spacing(6);

        container(card_content)
            .width(Length::Fill)
            .padding([10, 14])
            .style(move |_t| theme::card_style(&c))
            .into()
    }).collect();

    column(cards).spacing(6).width(Length::Fill).into()
}

/// Small colored tag badge used in preset cards.
/// `text_color` is the label text color; `base_color` controls the background/border tint.
fn badge_tag<'a>(label: &'static str, text_color: iced::Color, base_color: iced::Color) -> Element<'a, Message> {
    container(
        text(label).size(10).color(text_color)
    )
    .padding([2, 6])
    .style(move |_t| container::Style {
        background: Some(iced::Background::Color(
            iced::Color::from_rgba(base_color.r, base_color.g, base_color.b, 0.18)
        )),
        border: iced::Border {
            color: iced::Color::from_rgba(base_color.r, base_color.g, base_color.b, 0.45),
            width: 1.0,
            radius: 5.0.into(),
        },
        ..Default::default()
    })
    .into()
}

/// Build an SVG handle for a forge icon.
/// `forge_url` is used to distinguish Codeberg from other Gitea-based instances.
pub(crate) fn forge_svg_handle(forge: &str, forge_url: &str) -> iced::widget::svg::Handle {
    let svg: &str = match forge {
        "github" => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">"#,
            r#"<path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 "#,
            r#"11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61"#,
            r#"-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 "#,
            r#"1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776"#,
            r#".417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22"#,
            r#"-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 "#,
            r#"1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 "#,
            r#"3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 "#,
            r#"2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 "#,
            r#"12.297c0-6.627-5.373-12-12-12"/></svg>"#,
        ),
        "gitlab" => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">"#,
            r#"<path d="M23.955 13.587l-1.342-4.135-2.664-8.189c-.135-.423-.73-.423-.867 0L16.42 "#,
            r#"9.452H7.582L4.918 1.263c-.135-.423-.731-.423-.867 0L1.386 9.452.044 13.587c-.121"#,
            r#".374.014.784.33 1.016L12 22.047l11.625-8.444c.317-.232.452-.642.33-1.016"/></svg>"#,
        ),
        _ => "",  // resolved below based on URL
    };

    // For gitea-typed forges, distinguish Codeberg from generic Gitea/Forgejo
    let svg_owned: String;
    let resolved_svg = if svg.is_empty() {
        if forge_url.contains("codeberg") {
            // Codeberg: official Simple Icons path (CC0)
            svg_owned = concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">"#,
                r#"<path d="M11.999.747A11.974 11.974 0 000 12.75c0 2.254.635 4.465 1.833 6.376L11.837 "#,
                r#"6.19c.072-.092.251-.092.323 0l4.178 5.402h-2.992l.065.239h3.113l.882 1.138h-3.674"#,
                r#"l.103.374h3.86l.777 1.003h-4.358l.135.483h4.593l.695.894h-5.038l.165.589h5.326"#,
                r#"l.609.785h-5.717l.182.65h6.038l.562.727h-6.397l.183.65h6.717A12.003 12.003 0 0024"#,
                r#" 12.75 11.977 11.977 0 0011.999.747zm3.654 19.104.182.65h5.326c.173-.204.353-.433"#,
                r#".513-.65zm.385 1.377.18.65h3.563c.233-.198.485-.428.712-.65zm.383 1.377.182.648h"#,
                r#"1.203c.356-.204.685-.412 1.042-.648z"/>"#,
                r#"</svg>"#,
            ).to_string();
        } else {
            // Gitea / Forgejo: tea cup with handle (stroke-based)
            svg_owned = concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" "#,
                r#"stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">"#,
                r#"<path d="M5 9h14v7a3 3 0 0 1-3 3H8a3 3 0 0 1-3-3V9z"/>"#,
                r#"<path d="M5 9V7a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v2"/>"#,
                r#"<path d="M19 11.5h1a2 2 0 0 1 0 4h-1"/>"#,
                r#"</svg>"#,
            ).to_string();
        }
        svg_owned.as_str()
    } else {
        svg
    };

    iced::widget::svg::Handle::from_memory(resolved_svg.as_bytes().to_vec())
}

/// SVG icons for the Options / Logs / About tab buttons, matching the Tauri version exactly.
fn tab_icon_svg(tab: Tab) -> iced::widget::svg::Handle {
    let svg: &'static str = match tab {
        Tab::Options => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" "#,
            r#"stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">"#,
            r#"<path d="M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6z"/>"#,
            r#"<path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06"#,
            r#"a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09"#,
            r#"A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83"#,
            r#"l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09"#,
            r#"A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0"#,
            r#"l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09"#,
            r#"a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83"#,
            r#"l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09"#,
            r#"a1.65 1.65 0 0 0-1.51 1z"/></svg>"#,
        ),
        Tab::Logs => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" "#,
            r#"stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">"#,
            r#"<path d="M5 4.5A1.5 1.5 0 0 1 6.5 3h9l4.5 4.5V19.5A1.5 1.5 0 0 1 18.5 21h-12"#,
            r#"A1.5 1.5 0 0 1 5 19.5v-15Zm10 .5v3h3"/>"#,
            r#"<path d="M8 11h8M8 14h8M8 17h6"/>"#,
            r#"</svg>"#,
        ),
        Tab::About => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">"#,
            r#"<path fill-rule="evenodd" d="M12 2a10 10 0 0 1 0 20a10 10 0 0 1 0-20z "#,
            r#"M12 6.8a1.2 1.2 0 0 1 0 2.4a1.2 1.2 0 0 1 0-2.4z "#,
            r#"M10.5 11h3v7h-3z"/></svg>"#,
        ),
        _ => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"></svg>"#,
    };
    iced::widget::svg::Handle::from_memory(svg.as_bytes().to_vec())
}

fn close_button<'a>(colors: &ThemeColors) -> Element<'a, Message> {
    let c = *colors;
    button(text("\u{2715}").size(14).color(c.bad)) // ✕ in red
        .on_press(Message::CloseDialog)
        .padding([4, 8])
        .style(move |_theme, status| match status {
            button::Status::Hovered => button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    c.bad.r, c.bad.g, c.bad.b, 0.15,
                ))),
                text_color: c.bad,
                border: iced::Border {
                    color: iced::Color::from_rgba(c.bad.r, c.bad.g, c.bad.b, 0.4),
                    width: 1.0,
                    radius: iced::border::Radius::from(4),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            },
            _ => button::Style {
                background: None,
                text_color: c.bad,
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
    let update_ignored = app.ignored_update_ids.contains(&rid);
    let name = format!("{}/{}", repo.owner, repo.name);

    let mut items: Vec<Element<Message>> = Vec::new();

    if has_update && !update_ignored {
        items.push(ctx_menu_item("\u{2193} Update", Message::UpdateRepo(rid), &c));
    }
    items.push(ctx_menu_item("Reinstall / Repair", Message::ReinstallRepo(rid), &c));
    if panels::projects::is_dxvk_repo(&repo.name) {
        items.push(ctx_menu_item("\u{2699} Configure DXVK\u{2026}", Message::OpenDxvkConfig, &c));
    }
    if is_mod_val {
        let label = if enabled { "Disable" } else { "Enable" };
        items.push(ctx_menu_item(label, Message::ToggleRepoEnabled(rid, !enabled), &c));
    }
    let ignore_label = if update_ignored { "Unignore Updates" } else { "Ignore Updates" };
    items.push(ctx_menu_item(ignore_label, Message::ToggleIgnoreUpdates(rid), &c));
    // Remove (danger)
    let c3 = c;
    items.push(
        button(text("Remove").size(12).color(c.bad))
            .on_press(Message::OpenDialog(Dialog::RemoveRepo { id: rid, name, remove_files: false, files: Vec::new() }))
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

/// Clipboard helper.
///
/// On Linux (Wayland/X11) the clipboard is "owned" by a process that must keep serving requests
/// until another app takes ownership. We spawn a background thread that holds `Clipboard` alive
/// via `wait_until()` so clipboard managers have time to read and cache the content.
///
/// On Windows/macOS the OS retains clipboard content after the handle is closed, so a simple
/// `set_text` is sufficient.
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use arboard::SetExtLinux;
        let text_owned = text.to_string();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        std::thread::spawn(move || {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set().wait_until(deadline).text(text_owned);
            }
        });
        return Ok(());
    }

    #[cfg(not(target_os = "linux"))]
    {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if cb.set_text(text).is_ok() {
                return Ok(());
            }
        }
        Err("Clipboard unavailable".to_string())
    }
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

/// Returns true for error codes the user has chosen to silence (e.g. -16 = GIT_EAUTH,
/// produced by deleted or private repositories).  Callers should skip logging these.
fn is_silenced_git_error(raw: &str) -> bool {
    // "code=Auth (-16)" or "code=Something (-16)" anywhere in the raw error string.
    raw.contains("(-16)")
}

/// Converts a verbose libgit2/network error chain into a short, human-readable message,
/// appending the numeric error code if one is found (e.g. "… (Error Code -16)").
///
/// Raw errors look like:
///   "list remote branches URL (last tried URL): connect remote URL (auth failed: … class=Http (34); code=Auth (-16))"
/// Which becomes: "Repository not found or requires authentication (Error Code -16)"
fn simplify_git_error(raw: &str) -> String {
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

fn chrono_now_fmt(use_12h: bool) -> String {
    // Simple time string without pulling in chrono crate
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h24 = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    let s = secs % 60;
    if use_12h {
        let ampm = if h24 < 12 { "AM" } else { "PM" };
        let h12 = match h24 % 12 { 0 => 12, h => h };
        format!("{:02}:{:02}:{:02} {}", h12, mins, s, ampm)
    } else {
        format!("{:02}:{:02}:{:02}", h24, mins, s)
    }
}

fn chrono_now() -> String {
    chrono_now_fmt(false)
}
