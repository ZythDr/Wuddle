// WoW.exe patching based on vanilla-tweaks by brndd
// https://github.com/brndd/vanilla-tweaks
// Copyright (c) 2022 brndd — MIT License

use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakOptions {
    pub fov: Option<f32>,
    pub farclip: Option<f32>,
    pub frilldistance: Option<f32>,
    pub nameplate_distance: Option<f32>,
    pub sound_channels: Option<u32>,
    pub max_camera_distance: Option<f32>,
    pub quickloot: bool,
    pub sound_in_background: bool,
    pub large_address_aware: bool,
    pub camera_skip_fix: bool,
}

// ---- Offsets (from vanilla-tweaks) ----

const FOV_OFFSET: usize = 0x4089B4;
const FARCLIP_OFFSET: usize = 0x40FED8;
const FRILLDISTANCE_OFFSET: usize = 0x467958;
const NAMEPLATE_DISTANCE_OFFSET: usize = 0x40C448;
const SOUNDCHANNEL_OFFSET: usize = 0x435D38;
const MAX_CAMERA_DISTANCE_OFFSET: usize = 0x4089A4;
const QUICKLOOT_OFFSET_1: usize = 0x0C1ECF;
const QUICKLOOT_OFFSET_2: usize = 0x0C2B25;
const SOUND_IN_BACKGROUND_OFFSET: usize = 0x3A4869;
const CHARACTERISTICS_OFFSET: usize = 0x126;

// Camera skip fix patches
const CAMERA_PATCHES: [(usize, &[u8]); 5] = [
    (
        0x02CCD0,
        &[
            0x55, 0x8B, 0x05, 0x48, 0x4E, 0x88, 0x00, 0x8B, 0x0D, 0x44, 0x4E, 0x88, 0x00, 0xE9,
            0x33, 0x90, 0x32, 0x00, 0x83, 0xC0, 0x32, 0x83, 0xC1, 0x32, 0x3B, 0x0D, 0xA8, 0xEB,
            0xC4, 0x00, 0x7E, 0x03, 0x83, 0xE9, 0x01, 0x3B, 0x05, 0xAC, 0xEB, 0xC4, 0x00, 0x7E,
            0x03, 0x83, 0xE8, 0x01, 0x83, 0xE9, 0x32, 0x83, 0xE8, 0x32, 0x89, 0x05, 0x48, 0x4E,
            0x88, 0x00, 0x89, 0x0D, 0x44, 0x4E, 0x88, 0x00, 0x5D, 0xEB, 0x0D,
        ],
    ),
    (0x02D326, &[0xE9, 0xB1, 0x8A, 0x32, 0x00]),
    (
        0x02D334,
        &[0x8B, 0x35, 0x48, 0x4E, 0x88, 0x00],
    ),
    (
        0x355D15,
        &[
            0x83, 0xF8, 0x32, 0x7D, 0x03, 0x83, 0xC0, 0x01, 0x83, 0xF9, 0x32, 0x7D, 0x03, 0x83,
            0xC1, 0x01, 0xE9, 0xB8, 0x6F, 0xCD, 0xFF,
        ],
    ),
    (
        0x355DDC,
        &[
            0x8D, 0x4D, 0xF0, 0x51, 0xFF, 0x35, 0x00, 0x4E, 0x88, 0x00, 0xFF, 0x15, 0x50, 0xF6,
            0x7F, 0x00, 0x8B, 0x45, 0xF0, 0x8B, 0x15, 0x44, 0x4E, 0x88, 0x00, 0xE9, 0x35, 0x75,
            0xCD, 0xFF,
        ],
    ),
];

fn write_f32(buf: &mut [u8], offset: usize, val: f32) -> Result<(), String> {
    let end = offset + 4;
    if end > buf.len() {
        return Err(format!("Offset 0x{offset:X} out of range"));
    }
    buf[offset..end].copy_from_slice(&val.to_le_bytes());
    Ok(())
}

pub fn apply_tweaks(wow_dir: &Path, opts: &TweakOptions) -> Result<String, String> {
    let exe_path = wow_dir.join("WoW.exe");
    if !exe_path.exists() {
        return Err("WoW.exe not found in the specified directory.".into());
    }

    let backup_path = wow_dir.join("WoW.exe.bak");
    if !backup_path.exists() {
        fs::copy(&exe_path, &backup_path)
            .map_err(|e| format!("Failed to create backup: {e}"))?;
    }

    // Always start from the clean backup so unchecked tweaks revert to original values
    // and re-applying with different settings works without a manual restore first.
    let mut buf = fs::read(&backup_path).map_err(|e| format!("Failed to read WoW.exe.bak: {e}"))?;

    let mut applied: Vec<&str> = Vec::new();

    // FoV
    if let Some(val) = opts.fov {
        write_f32(&mut buf, FOV_OFFSET, val)?;
        applied.push("FoV");
    }

    // Farclip
    if let Some(val) = opts.farclip {
        write_f32(&mut buf, FARCLIP_OFFSET, val)?;
        applied.push("Farclip");
    }

    // Frilldistance
    if let Some(val) = opts.frilldistance {
        write_f32(&mut buf, FRILLDISTANCE_OFFSET, val)?;
        applied.push("Frilldistance");
    }

    // Nameplate distance
    if let Some(val) = opts.nameplate_distance {
        write_f32(&mut buf, NAMEPLATE_DISTANCE_OFFSET, val)?;
        applied.push("Nameplate distance");
    }

    // Sound channels
    if let Some(val) = opts.sound_channels {
        let clamped = val.clamp(1, 999);
        let s = clamped.to_string();
        let cstring =
            CString::new(s).map_err(|e| format!("Invalid sound channel value: {e}"))?;
        let bytes = cstring.to_bytes_with_nul();
        if bytes.len() <= 4 {
            let end = SOUNDCHANNEL_OFFSET + bytes.len();
            if end > buf.len() {
                return Err("Sound channel offset out of range".into());
            }
            buf[SOUNDCHANNEL_OFFSET..end].copy_from_slice(bytes);
            applied.push("Sound channels");
        }
    }

    // Max camera distance
    if let Some(val) = opts.max_camera_distance {
        write_f32(&mut buf, MAX_CAMERA_DISTANCE_OFFSET, val)?;
        applied.push("Max camera distance");
    }

    // Quickloot
    if opts.quickloot {
        if QUICKLOOT_OFFSET_1 >= buf.len() || QUICKLOOT_OFFSET_2 >= buf.len() {
            return Err("Quickloot offset out of range".into());
        }
        buf[QUICKLOOT_OFFSET_1] = 0x75;
        buf[QUICKLOOT_OFFSET_2] = 0x75;
        applied.push("Quickloot");
    }

    // Sound in background
    if opts.sound_in_background {
        if SOUND_IN_BACKGROUND_OFFSET >= buf.len() {
            return Err("Sound in background offset out of range".into());
        }
        buf[SOUND_IN_BACKGROUND_OFFSET] = 0x27;
        applied.push("Sound in background");
    }

    // Large Address Aware
    if opts.large_address_aware {
        if CHARACTERISTICS_OFFSET + 2 > buf.len() {
            return Err("PE header offset out of range".into());
        }
        let mut chars = u16::from_le_bytes(
            buf[CHARACTERISTICS_OFFSET..CHARACTERISTICS_OFFSET + 2]
                .try_into()
                .unwrap(),
        );
        chars |= 0x20;
        buf[CHARACTERISTICS_OFFSET..CHARACTERISTICS_OFFSET + 2]
            .copy_from_slice(&chars.to_le_bytes());
        applied.push("Large Address Aware");
    }

    // Camera skip fix
    if opts.camera_skip_fix {
        for (offset, patch) in &CAMERA_PATCHES {
            let end = offset + patch.len();
            if end > buf.len() {
                return Err(format!("Camera patch offset 0x{offset:X} out of range"));
            }
            buf[*offset..end].copy_from_slice(patch);
        }
        applied.push("Camera skip fix");
    }

    fs::write(&exe_path, &buf).map_err(|e| format!("Failed to write patched WoW.exe: {e}"))?;

    if applied.is_empty() {
        Ok("No tweaks were selected.".into())
    } else {
        Ok(format!("Applied {} tweak(s): {}", applied.len(), applied.join(", ")))
    }
}

pub fn restore_backup(wow_dir: &Path) -> Result<String, String> {
    let exe_path = wow_dir.join("WoW.exe");
    let backup_path = wow_dir.join("WoW.exe.bak");
    if !backup_path.exists() {
        return Err("No backup file (WoW.exe.bak) found.".into());
    }
    fs::copy(&backup_path, &exe_path)
        .map_err(|e| format!("Failed to restore backup: {e}"))?;
    Ok("Restored WoW.exe from backup.".into())
}

#[allow(dead_code)]
pub fn has_backup(wow_dir: &Path) -> bool {
    wow_dir.join("WoW.exe.bak").exists()
}

// ---- Read current values from WoW.exe ----

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadTweakValues {
    pub fov: f32,
    pub farclip: f32,
    pub frilldistance: f32,
    pub nameplate_distance: f32,
    pub sound_channels: u32,
    pub max_camera_distance: f32,
    pub quickloot: bool,
    pub sound_in_background: bool,
    pub large_address_aware: bool,
    pub camera_skip_fix: bool,
}

fn read_f32(buf: &[u8], offset: usize) -> Result<f32, String> {
    let end = offset + 4;
    if end > buf.len() {
        return Err(format!("Offset 0x{offset:X} out of range"));
    }
    Ok(f32::from_le_bytes(buf[offset..end].try_into().unwrap()))
}

pub fn read_tweaks(wow_dir: &Path) -> Result<ReadTweakValues, String> {
    let exe_path = wow_dir.join("WoW.exe");
    if !exe_path.exists() {
        return Err("WoW.exe not found in the specified directory.".into());
    }

    let buf = fs::read(&exe_path).map_err(|e| format!("Failed to read WoW.exe: {e}"))?;

    let fov = read_f32(&buf, FOV_OFFSET)?;
    let farclip = read_f32(&buf, FARCLIP_OFFSET)?;
    let frilldistance = read_f32(&buf, FRILLDISTANCE_OFFSET)?;
    let nameplate_distance = read_f32(&buf, NAMEPLATE_DISTANCE_OFFSET)?;
    let max_camera_distance = read_f32(&buf, MAX_CAMERA_DISTANCE_OFFSET)?;

    // Sound channels: null-terminated ASCII string at offset, parse as u32
    let sound_channels = {
        let start = SOUNDCHANNEL_OFFSET;
        let mut end = start;
        while end < buf.len() && end < start + 4 && buf[end] != 0 {
            end += 1;
        }
        let s = std::str::from_utf8(&buf[start..end]).unwrap_or("12");
        s.parse::<u32>().unwrap_or(12)
    };

    let quickloot = QUICKLOOT_OFFSET_1 < buf.len() && buf[QUICKLOOT_OFFSET_1] == 0x75;
    let sound_in_background =
        SOUND_IN_BACKGROUND_OFFSET < buf.len() && buf[SOUND_IN_BACKGROUND_OFFSET] == 0x27;

    let large_address_aware = if CHARACTERISTICS_OFFSET + 2 <= buf.len() {
        let chars = u16::from_le_bytes(
            buf[CHARACTERISTICS_OFFSET..CHARACTERISTICS_OFFSET + 2]
                .try_into()
                .unwrap(),
        );
        chars & 0x20 != 0
    } else {
        false
    };

    // Check first byte of first camera patch
    let camera_skip_fix = {
        let (offset, patch) = &CAMERA_PATCHES[0];
        *offset < buf.len() && buf[*offset] == patch[0]
    };

    Ok(ReadTweakValues {
        fov,
        farclip,
        frilldistance,
        nameplate_distance,
        sound_channels,
        max_camera_distance,
        quickloot,
        sound_in_background,
        large_address_aware,
        camera_skip_fix,
    })
}
