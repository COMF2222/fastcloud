#![allow(dead_code)]

use crate::cli::IpcMessage;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use std::sync::mpsc;

/// OS media keys + now-playing metadata (SMTC on Windows, MPRIS on Linux, Now Playing on macOS).
pub struct MediaIntegration {
    controls: Option<MediaControls>,
    pub rx: mpsc::Receiver<IpcMessage>,
}

fn event_to_message(event: MediaControlEvent) -> Option<IpcMessage> {
    let msg = match event {
        MediaControlEvent::Play => IpcMessage::Play,
        MediaControlEvent::Pause => IpcMessage::Pause,
        MediaControlEvent::Toggle => IpcMessage::Toggle,
        MediaControlEvent::Next => IpcMessage::Next,
        MediaControlEvent::Previous => IpcMessage::Prev,
        MediaControlEvent::Stop => IpcMessage::Stop,
        MediaControlEvent::Seek(_) | MediaControlEvent::SeekBy(_, _) => IpcMessage::Toggle,
        MediaControlEvent::SetPosition(_) => IpcMessage::Toggle,
        MediaControlEvent::SetVolume(_) | MediaControlEvent::OpenUri(_) => return None,
        MediaControlEvent::Raise => IpcMessage::Show,
        MediaControlEvent::Quit => IpcMessage::Quit,
    };
    Some(msg)
}

impl MediaIntegration {
    #[cfg(target_os = "windows")]
    pub fn new(hwnd: *mut std::ffi::c_void) -> Self {
        let (tx, rx) = mpsc::channel();
        let config = PlatformConfig {
            dbus_name: "fastcloud",
            display_name: "Fastcloud",
            hwnd: Some(hwnd),
        };
        let controls = Self::attach(config, tx);
        Self { controls, rx }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let config = PlatformConfig {
            dbus_name: "fastcloud",
            display_name: "Fastcloud",
            hwnd: None,
        };
        let controls = Self::attach(config, tx);
        Self { controls, rx }
    }

    fn attach(config: PlatformConfig<'_>, tx: mpsc::Sender<IpcMessage>) -> Option<MediaControls> {
        match MediaControls::new(config) {
            Ok(mut c) => {
                let _ = c.attach(move |event| {
                    if let Some(msg) = event_to_message(event) {
                        let _ = tx.send(msg);
                    }
                });
                Some(c)
            }
            Err(e) => {
                log::warn!("media controls unavailable: {e}");
                None
            }
        }
    }

    pub fn update_metadata(
        &mut self,
        title: &str,
        artist: &str,
        cover_url: Option<&str>,
        duration_ms: i64,
    ) {
        let Some(controls) = &mut self.controls else {
            return;
        };
        let _ = controls.set_metadata(MediaMetadata {
            title: Some(title),
            artist: Some(artist),
            album: None,
            cover_url,
            duration: Some(std::time::Duration::from_millis(duration_ms.max(0) as u64)),
        });
    }

    pub fn update_playback(&mut self, playing: bool, position_ms: u64) {
        let Some(controls) = &mut self.controls else {
            return;
        };
        let progress = Some(MediaPosition(std::time::Duration::from_millis(position_ms)));
        let status = if playing {
            MediaPlayback::Playing { progress }
        } else {
            MediaPlayback::Paused { progress }
        };
        let _ = controls.set_playback(status);
    }

    pub fn detach(&mut self) {
        if let Some(c) = &mut self.controls {
            let _ = c.detach();
        }
    }
}
