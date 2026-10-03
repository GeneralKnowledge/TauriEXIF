use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub preserve_orientation: bool,
    pub preserve_color_profile: bool,
    pub preserve_resolution: bool,
    pub save_as_copy: bool,
    pub theme_mode: ThemeMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preserve_orientation: true,
            preserve_color_profile: true,
            preserve_resolution: true,
            save_as_copy: true,
            theme_mode: ThemeMode::System,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SettingsFile {
    version: u32,
    settings: Settings,
}

const SCHEMA_VERSION: u32 = 1;

fn settings_path() -> Result<PathBuf, String> {
    let dir = dirs::config_dir()
        .ok_or_else(|| "Could not resolve config directory".to_string())?
        .join("tauri-exif");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

pub fn load_settings() -> Settings {
    let Ok(path) = settings_path() else {
        return Settings::default();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return Settings::default();
    };
    match serde_json::from_str::<SettingsFile>(&raw) {
        Ok(file) => file.settings,
        Err(_) => Settings::default(),
    }
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let path = settings_path()?;
    let file = SettingsFile {
        version: SCHEMA_VERSION,
        settings: settings.clone(),
    };
    let raw = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}
