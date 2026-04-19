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

#[derive(Debug, Clone)]
pub enum Dialog {
    AddRepo { url: String, mode: String, is_addons: bool, advanced: bool },
    RemoveRepo { id: i64, name: String, remove_files: bool, files: Vec<(String, String)> },
    RemoveCollectionAddon {
        repo_id: i64,
        repo_name: String,
        addon_name: String,
        files: Vec<(String, String)>,
    },
    Changelog { title: String, items: Vec<iced::widget::markdown::Item>, loading: bool },
    DxvkConfig { config: DxvkConfig, show_preview: bool },
    DllCountWarning {
        repo_id: i64,
        repo_name: String,
        previous_count: usize,
        new_count: usize,
    },
    InstanceSettings {
        is_new: bool,
        profile_id: String,
        name: String,
        wow_dir: String,
        launch_method: String,  // "auto", "lutris", "wine", "custom"
        clear_wdb: bool,
        lutris_target: String,
        wine_command: String,
        wine_args: String,
        custom_command: String,
        custom_args: String,
    },
    AvWarning { url: String, mode: String },
    AddonConflict { url: String, mode: String, conflicts: Vec<wuddle_engine::AddonProbeConflict> },
}
