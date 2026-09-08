#![allow(dead_code)]

use crate::player::RepeatMode;
use clap::{Parser, Subcommand};

/// Raw `now-playing` field order (tab-separated, new fields appended only):
/// state, title, artist, position_ms, duration_ms, volume, shuffle,
/// repeat, art_url, liked, device. Mirrors fastpotify's `--raw` contract.
pub const NOW_PLAYING_RAW_FIELDS: &str = "state\ttitle\tartist\tposition_ms\tduration_ms\tvolume\tshuffle\trepeat\tart_url\tliked\tdevice";

#[derive(Parser, Debug)]
#[command(
    name = "fastcloud",
    version,
    about = "Native SoundCloud client",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Start in demo mode with sample content (no credentials needed)
    #[arg(long)]
    pub demo: bool,

    /// Override client_id (takes precedence over saved settings)
    #[arg(long)]
    pub client_id: Option<String>,

    /// Override client_secret (takes precedence over saved settings)
    #[arg(long)]
    pub client_secret: Option<String>,

    /// Open a SoundCloud link/URI in the running app (or on boot).
    pub link: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Send playback commands to a running instance
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    Stop,
    /// Seek relative seconds (negative goes back)
    Seek {
        seconds: i64,
    },
    /// Seek to absolute seconds
    SeekTo {
        seconds: u64,
    },
    /// Set volume (0-100)
    Volume {
        volume: u8,
    },
    /// Raise volume (default 10)
    VolumeUp {
        percent: Option<u8>,
    },
    /// Lower volume (default 10)
    VolumeDown {
        percent: Option<u8>,
    },
    /// Mute
    Mute,
    /// Toggle or set shuffle (on|off)
    Shuffle {
        state: Option<String>,
    },
    /// Cycle or set repeat (off|all|one)
    Repeat {
        mode: Option<String>,
    },
    /// Like/unlike the current track
    Like,
    /// Resolve a SoundCloud URL/URI and play it now
    PlayUrl {
        url: String,
    },
    /// Show now-playing (human-readable, or --raw tab-separated)
    NowPlaying {
        #[arg(long)]
        raw: bool,
    },
    /// List audio output devices (JSON with --raw)
    Devices {
        #[arg(long)]
        raw: bool,
    },
    /// Start SoundCloud account authorization in the browser
    SignIn,
    /// Bring the running window to front
    Show,
    /// Terminate running instance
    Quit,
}

/// Message sent to the running instance over the singleton socket.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum IpcMessage {
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    Stop,
    SeekBy(i64),
    SeekTo(u64),
    Volume(u8),
    VolumeBy(i16),
    Mute,
    Shuffle(Option<bool>),
    Repeat(Option<RepeatMode>),
    Like,
    PlayUrl(String),
    OpenLink(String),
    NowPlaying { raw: bool },
    Devices { raw: bool },
    SignIn,
    Show,
    Quit,
}

/// Reply read back by the CLI process (single `\n`-terminated JSON line).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum IpcReply {
    Ok,
    Text(String),
    Err(String),
}

impl From<Command> for IpcMessage {
    fn from(cmd: Command) -> Self {
        match cmd {
            Command::Play => IpcMessage::Play,
            Command::Pause => IpcMessage::Pause,
            Command::Toggle => IpcMessage::Toggle,
            Command::Next => IpcMessage::Next,
            Command::Prev => IpcMessage::Prev,
            Command::Stop => IpcMessage::Stop,
            Command::Seek { seconds } => IpcMessage::SeekBy(seconds * 1000),
            Command::SeekTo { seconds } => IpcMessage::SeekTo(seconds * 1000),
            Command::Volume { volume } => IpcMessage::Volume(volume.min(100)),
            Command::VolumeUp { percent } => {
                IpcMessage::VolumeBy(percent.unwrap_or(10).min(100) as i16)
            }
            Command::VolumeDown { percent } => {
                IpcMessage::VolumeBy(-(percent.unwrap_or(10).min(100) as i16))
            }
            Command::Mute => IpcMessage::Mute,
            Command::Shuffle { state } => IpcMessage::Shuffle(parse_on_off(state.as_deref())),
            Command::Repeat { mode } => IpcMessage::Repeat(mode.as_deref().and_then(parse_repeat)),
            Command::Like => IpcMessage::Like,
            Command::PlayUrl { url } => IpcMessage::PlayUrl(url),
            Command::NowPlaying { raw } => IpcMessage::NowPlaying { raw },
            Command::Devices { raw } => IpcMessage::Devices { raw },
            Command::SignIn => IpcMessage::SignIn,
            Command::Show => IpcMessage::Show,
            Command::Quit => IpcMessage::Quit,
        }
    }
}

pub fn parse_on_off(s: Option<&str>) -> Option<bool> {
    match s.map(str::to_lowercase)?.as_str() {
        "on" | "true" | "1" | "yes" => Some(true),
        "off" | "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

pub fn parse_repeat(s: &str) -> Option<RepeatMode> {
    match s.to_lowercase().as_str() {
        "off" => Some(RepeatMode::Off),
        "all" | "context" => Some(RepeatMode::All),
        "one" | "track" => Some(RepeatMode::One),
        _ => None,
    }
}

pub fn repeat_name(mode: RepeatMode) -> &'static str {
    match mode {
        RepeatMode::Off => "off",
        RepeatMode::All => "all",
        RepeatMode::One => "one",
    }
}

/// Snapshot the singleton server answers queries from (no App access).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NowPlayingInfo {
    pub state: String,
    pub title: String,
    pub artist: String,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: u8,
    pub shuffle: String,
    pub repeat: String,
    pub art_url: String,
    pub liked: String,
    pub device: String,
}

impl NowPlayingInfo {
    pub fn raw(&self) -> String {
        [
            self.state.clone(),
            self.title.clone(),
            self.artist.clone(),
            self.position_ms.to_string(),
            self.duration_ms.to_string(),
            self.volume.to_string(),
            self.shuffle.clone(),
            self.repeat.clone(),
            self.art_url.clone(),
            self.liked.clone(),
            self.device.clone(),
        ]
        .join("\t")
    }

    pub fn human(&self) -> String {
        if self.state == "stopped" {
            return "Nothing playing".to_owned();
        }
        let icon = if self.state == "playing" {
            "▶"
        } else {
            "⏸"
        };
        format!(
            "{icon} {} — {} [{} / {}]",
            self.title,
            self.artist,
            crate::util::fmt_duration_ms(self.position_ms),
            crate::util::fmt_duration_ms(self.duration_ms),
        )
    }
}

/// One audio output edge for `devices [--raw]`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub active: bool,
}

pub fn format_devices_human(devices: &[DeviceInfo]) -> String {
    if devices.is_empty() {
        return "No devices".to_owned();
    }
    devices
        .iter()
        .map(|d| {
            format!(
                "{} {}\t{}\t{}",
                if d.active { "*" } else { " " },
                d.id,
                d.name,
                d.kind
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Send a command and wait for the reply. `Err` = no running instance.
pub fn run_remote_command(cmd: Command) -> anyhow::Result<IpcReply> {
    crate::desktop::single_instance::send_request(&IpcMessage::from(cmd))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_play() {
        let cli = Cli::try_parse_from(["fastcloud", "play"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Play)));
    }

    #[test]
    fn verbs_map() {
        assert!(matches!(
            IpcMessage::from(Command::Seek { seconds: -15 }),
            IpcMessage::SeekBy(-15000)
        ));
        assert!(matches!(
            IpcMessage::from(Command::VolumeUp { percent: None }),
            IpcMessage::VolumeBy(10)
        ));
        assert!(matches!(
            IpcMessage::from(Command::Repeat {
                mode: Some("one".into())
            }),
            IpcMessage::Repeat(Some(RepeatMode::One))
        ));
        assert!(matches!(
            IpcMessage::from(Command::Shuffle {
                state: Some("off".into())
            }),
            IpcMessage::Shuffle(Some(false))
        ));
    }

    #[test]
    fn raw_field_order_is_stable() {
        let info = NowPlayingInfo {
            state: "playing".into(),
            title: "T".into(),
            artist: "A".into(),
            position_ms: 1000,
            duration_ms: 2000,
            volume: 80,
            shuffle: "off".into(),
            repeat: "off".into(),
            art_url: "http://x".into(),
            liked: "yes".into(),
            device: "local".into(),
        };
        assert_eq!(
            info.raw(),
            "playing\tT\tA\t1000\t2000\t80\toff\toff\thttp://x\tyes\tlocal"
        );
        assert!(info.human().contains("▶"));
    }
}
