#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub dark_theme: bool,
    pub notifications_enabled: bool,
    pub minimize_to_tray: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            dark_theme: false,
            notifications_enabled: true,
            minimize_to_tray: true,
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new() -> Result<Self, String> {
        let dir = dirs::config_dir()
            .ok_or_else(|| "Cannot resolve config dir".to_string())?
            .join("yandex-messenger-native");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(Self {
            path: dir.join("settings.json"),
        })
    }

    pub fn load(&self) -> AppSettings {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|v| serde_json::from_str::<AppSettings>(&v).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, settings: &AppSettings) -> Result<(), String> {
        let serialized = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
        fs::write(&self.path, serialized).map_err(|e| e.to_string())
    }
}
