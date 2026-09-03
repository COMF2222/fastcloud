#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const APP_ID: &str = "fastcloud";

/// User-editable settings persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeMode,
    pub volume: f32,
    pub eq_gains_db: [f32; 10],
    pub eq_enabled: bool,
    pub last_track_urn: Option<String>,
    pub last_position_ms: Option<u64>,
    pub client_id: Option<String>,
    pub show_track_numbers: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

impl Settings {
    pub fn load() -> Result<Self> {
        let path = settings_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self) -> Result<()> {
        let path = settings_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(path, raw)?;
        Ok(())
    }
}

pub struct AppPaths {
    pub root: PathBuf,
    pub cache: PathBuf,
    pub audio_cache: PathBuf,
    pub cover_cache: PathBuf,
}

pub fn app_paths() -> Result<AppPaths> {
    let base = directories::ProjectDirs::from("", "", APP_ID).context("resolve app data dir")?;
    let cache = base.cache_dir().to_path_buf();
    let audio_cache = cache.join("audio");
    let cover_cache = cache.join("covers");
    std::fs::create_dir_all(&audio_cache)?;
    std::fs::create_dir_all(&cover_cache)?;
    Ok(AppPaths {
        root: base.data_dir().to_path_buf(),
        cache,
        audio_cache,
        cover_cache,
    })
}

pub fn settings_path() -> Result<PathBuf> {
    Ok(app_paths()?.root.join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_roundtrip() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.volume, s.volume);
        assert_eq!(back.eq_gains_db.len(), 10);
        assert!(!back.eq_enabled);
    }

    #[test]
    fn parse_theme() {
        let json = r#"{"theme":"Light"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.theme, ThemeMode::Light);
    }
}
