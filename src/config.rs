#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub const APP_ID: &str = "fastcloud";

static SETTINGS_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn settings_write_lock() -> &'static Mutex<()> {
    SETTINGS_WRITE_LOCK.get_or_init(|| Mutex::new(()))
}

/// User-editable settings persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeMode,
    pub volume: f32,
    /// Stereo balance, -1 hard left to 1 hard right. Only the mini player's
    /// second slider sets it; the app's own interface has no control.
    #[serde(default)]
    pub balance: f32,
    /// Fold stereo output to the same signal in both channels.
    #[serde(default)]
    pub mono: bool,
    pub eq_gains_db: [f32; 10],
    pub eq_enabled: bool,
    /// The equaliser's preamp in dB, ±12 as Winamp's slider is.
    #[serde(default)]
    pub eq_preamp_db: f32,
    /// Winamp's AUTO: reload each track's own preset when it starts.
    #[serde(default)]
    pub eq_auto: bool,
    pub last_track_urn: Option<String>,
    pub last_position_ms: Option<u64>,
    pub client_id: Option<String>,
    /// SoundCloud profile used for public library fallback when SoundCloud's
    /// user OAuth flow is unavailable. Public likes, tracks and playlists can
    /// still be read with the app token.
    #[serde(default)]
    pub soundcloud_profile_url: Option<String>,
    pub show_track_numbers: bool,
    /// Liked track ids (synced to SoundCloud when online).
    #[serde(default)]
    pub liked_ids: Vec<u64>,
    /// Followed user ids.
    #[serde(default)]
    pub followed_user_ids: Vec<u64>,
    /// Locally created playlists.
    #[serde(default)]
    pub custom_playlists: Vec<CustomPlaylist>,
    /// Per-track equaliser presets, by track id. Winamp kept these in
    /// `winamp.q1`; this is the same idea, and what AUTO reloads.
    #[serde(default)]
    pub eq_presets: Vec<EqPreset>,
    /// Opened SoundCloud links (local messages inbox, newest last).
    #[serde(default)]
    pub inbox: Vec<InboxItem>,
    /// Keep playing similar tracks when the queue runs out.
    #[serde(default = "default_true")]
    pub autoplay: bool,
    /// One-line track rows without artwork (Appearance → Compact rows).
    #[serde(default)]
    pub compact_rows: bool,
    /// Which trace the player bar's little display shows.
    #[serde(default)]
    pub visualiser: crate::ui::visualiser::Mode,
    /// Path of the `.wsz` the Winamp mini player wears; the built-in skin
    /// when unset or unreadable.
    #[serde(default)]
    pub winamp_skin: Option<PathBuf>,
    /// Whether the app is *in* the mini player.
    ///
    /// The mini player is not a second window: it is what the one window
    /// looks like, so this survives a restart the way Winamp's own mode did.
    #[serde(default)]
    pub winamp_window: bool,
    /// Whether the mini player is rolled up to its title bar ("windowshade").
    ///
    /// Winamp's own shade mode: 275×14 of title, time, transport and a seek
    /// bar. Still the mini player, just without the interface — so this is a
    /// second flag rather than a third window mode.
    #[serde(default)]
    pub winamp_shade: bool,
    /// Whether the equaliser window is open, and rolled up.
    #[serde(default)]
    pub winamp_eq_window: bool,
    #[serde(default)]
    pub winamp_eq_shade: bool,
    /// Whether the playlist window is open, rolled up, and how many rows it
    /// shows. Winamp's playlist is the one window that resizes.
    #[serde(default)]
    pub winamp_pl_window: bool,
    #[serde(default)]
    pub winamp_pl_shade: bool,
    #[serde(default = "default_pl_rows")]
    pub winamp_pl_rows: u32,
    /// Keep the mini player above other windows, as Winamp's "always on top".
    #[serde(default)]
    pub winamp_on_top: bool,
    /// A font file to use for the interface instead of the bundled Inter.
    ///
    /// soundcloud.com's own face (Söhne) is a commercial licence and cannot be
    /// shipped with an MIT project, so a user who owns it can point at the
    /// file here and get the site's exact letterforms. Unset, unreadable or
    /// unparsable falls back to Inter (see [`crate::fonts`]).
    #[serde(default)]
    pub interface_font: Option<PathBuf>,
    /// The mini player's whole-pixel scale, 1–4.
    #[serde(default = "default_scale")]
    pub winamp_scale: u32,
    /// Persisted queue (tracks are small; capped on save).
    #[serde(default)]
    pub last_queue: Vec<crate::api::models::Track>,
    /// Index into `last_queue` that was current.
    #[serde(default)]
    pub last_queue_idx: Option<usize>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            volume: 0.8,
            balance: 0.0,
            mono: false,
            eq_gains_db: [0.0; 10],
            eq_enabled: false,
            eq_preamp_db: 0.0,
            eq_auto: false,
            last_track_urn: None,
            last_position_ms: None,
            client_id: None,
            soundcloud_profile_url: None,
            show_track_numbers: false,
            liked_ids: Vec::new(),
            followed_user_ids: Vec::new(),
            custom_playlists: Vec::new(),
            eq_presets: Vec::new(),
            inbox: Vec::new(),
            autoplay: true,
            compact_rows: false,
            visualiser: Default::default(),
            winamp_skin: None,
            winamp_window: false,
            winamp_shade: false,
            winamp_eq_window: false,
            winamp_eq_shade: false,
            winamp_pl_window: false,
            winamp_pl_shade: false,
            winamp_pl_rows: default_pl_rows(),
            winamp_on_top: false,
            interface_font: None,
            winamp_scale: default_scale(),
            last_queue: Vec::new(),
            last_queue_idx: None,
        }
    }
}

fn default_true() -> bool {
    true
}

/// 2× is the classic window at a size a modern screen can read.
fn default_scale() -> u32 {
    2
}

/// Eight rows is enough for the playlist to be worth opening.
fn default_pl_rows() -> u32 {
    crate::skin::layout::pl::DEFAULT_ROWS
}

/// A user-created playlist persisted in settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPlaylist {
    pub id: u64,
    pub title: String,
    pub track_ids: Vec<u64>,
}

/// One track's equaliser settings, which Winamp's AUTO reloads when that track
/// comes on.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct EqPreset {
    pub track_id: u64,
    pub preamp_db: f32,
    pub gains_db: [f32; 10],
}

/// A SoundCloud link opened/shared into the app (local messages inbox).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxItem {
    pub label: String,
    pub link: String,
    /// Unix seconds when it arrived.
    pub at: u64,
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
        Self::load_from(&path)
    }

    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            let backup = path.with_extension("json.bak");
            if backup.exists() {
                let raw = std::fs::read_to_string(backup)?;
                return Ok(serde_json::from_str(&raw)?);
            }
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)?;
        match serde_json::from_str(&raw) {
            Ok(settings) => Ok(settings),
            Err(primary) => {
                let backup = path.with_extension("json.bak");
                if !backup.exists() {
                    return Err(primary.into());
                }
                log::warn!("settings file is invalid; recovering {}", backup.display());
                let raw = std::fs::read_to_string(backup)?;
                Ok(serde_json::from_str(&raw)?)
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = settings_path()?;
        self.save_to_preserving_session(&path)
    }

    fn save_to_preserving_session(&self, path: &std::path::Path) -> Result<()> {
        let _guard = settings_write_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut next = self.clone();
        if let Ok(current) = Self::load_from(path) {
            // Player and UI intentionally own different snapshots. A UI-only
            // save must not roll back the newer playback session on disk.
            next.last_track_urn = current.last_track_urn;
            next.last_position_ms = current.last_position_ms;
            next.last_queue = current.last_queue;
            next.last_queue_idx = current.last_queue_idx;
        }
        write_settings(path, &next)
    }
}

/// Update playback-owned settings under the same lock as UI saves.
pub(crate) fn update_settings(
    path: &std::path::Path,
    update: impl FnOnce(&mut Settings),
) -> Result<()> {
    let _guard = settings_write_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut settings = Settings::load_from(path).unwrap_or_default();
    update(&mut settings);
    write_settings(path, &settings)
}

fn write_settings(path: &std::path::Path, settings: &Settings) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_vec_pretty(settings)?;
    let temporary = path.with_extension("json.tmp");
    let backup = path.with_extension("json.bak");
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(&raw)?;
    file.sync_all()?;
    drop(file);

    if backup.exists() {
        std::fs::remove_file(&backup)?;
    }
    if path.exists() {
        std::fs::rename(path, &backup)?;
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        if backup.exists() {
            let _ = std::fs::rename(&backup, path);
        }
        return Err(error.into());
    }
    if backup.exists() {
        std::fs::remove_file(backup)?;
    }
    Ok(())
}

pub struct AppPaths {
    pub root: PathBuf,
    pub cache: PathBuf,
    pub audio_cache: PathBuf,
    pub cover_cache: PathBuf,
    pub skin_dir: PathBuf,
}

pub fn app_paths() -> Result<AppPaths> {
    let base = directories::ProjectDirs::from("", "", APP_ID).context("resolve app data dir")?;
    let cache = base.cache_dir().to_path_buf();
    let audio_cache = cache.join("audio");
    let cover_cache = cache.join("covers");
    let skin_dir = base.data_dir().join("skins");
    std::fs::create_dir_all(&audio_cache)?;
    std::fs::create_dir_all(&cover_cache)?;
    std::fs::create_dir_all(&skin_dir)?;
    Ok(AppPaths {
        root: base.data_dir().to_path_buf(),
        cache,
        audio_cache,
        cover_cache,
        skin_dir,
    })
}

pub fn settings_path() -> Result<PathBuf> {
    Ok(app_paths()?.root.join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_install_is_audible_and_matches_missing_settings_fields() {
        let fresh = Settings::default();
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(fresh.volume, 0.8);
        assert_eq!(
            serde_json::to_value(&fresh).unwrap(),
            serde_json::to_value(old).unwrap()
        );
        let muted: Settings = serde_json::from_str(r#"{"volume":0}"#).unwrap();
        assert_eq!(muted.volume, 0.0);
    }

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

    /// A settings file written before the mini player grew its three states
    /// must still load: every field it does not name takes its default, and
    /// the defaults are the old behaviour.
    #[test]
    fn an_older_settings_file_still_loads() {
        let json = r#"{"theme":"Dark","volume":0.5,"winamp_window":true}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.volume, 0.5);
        assert!(s.winamp_window, "it was left in the mini player");
        assert!(!s.winamp_shade, "…and not rolled up");
        assert!(!s.winamp_on_top);
        assert_eq!(s.balance, 0.0, "centred");
        assert_eq!(s.winamp_scale, 2, "the default scale, not zero");
    }

    /// The window's own three flags survive a round trip, since they are what
    /// brings it back the way it was left.
    #[test]
    fn the_window_mode_roundtrips() {
        let s = Settings {
            winamp_window: true,
            winamp_shade: true,
            winamp_on_top: true,
            balance: -0.4,
            ..Settings::default()
        };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert!(back.winamp_window && back.winamp_shade && back.winamp_on_top);
        assert!((back.balance - -0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn ui_save_preserves_the_players_newer_session() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let player_state = Settings {
            last_track_urn: Some("soundcloud:tracks:42".into()),
            last_position_ms: Some(12_345),
            last_queue_idx: Some(0),
            ..Settings::default()
        };
        write_settings(&path, &player_state).unwrap();
        let stale_ui = Settings {
            theme: ThemeMode::Light,
            ..Settings::default()
        };

        stale_ui.save_to_preserving_session(&path).unwrap();

        let saved = Settings::load_from(&path).unwrap();
        assert_eq!(saved.theme, ThemeMode::Light);
        assert_eq!(saved.last_track_urn, player_state.last_track_urn);
        assert_eq!(saved.last_position_ms, player_state.last_position_ms);
        assert_eq!(saved.last_queue_idx, player_state.last_queue_idx);
    }

    #[test]
    fn load_recovers_the_backup_when_the_primary_file_is_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let backup = path.with_extension("json.bak");
        let expected = Settings {
            volume: 0.37,
            theme: ThemeMode::Light,
            ..Settings::default()
        };
        std::fs::write(&path, b"{not json").unwrap();
        std::fs::write(&backup, serde_json::to_vec(&expected).unwrap()).unwrap();

        let recovered = Settings::load_from(&path).unwrap();

        assert_eq!(recovered.theme, ThemeMode::Light);
        assert!((recovered.volume - 0.37).abs() < f32::EPSILON);
    }
}
