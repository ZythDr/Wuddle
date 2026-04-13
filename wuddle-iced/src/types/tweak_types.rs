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
