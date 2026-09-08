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
        MediaControlEvent::Seek(dir) => {
            // Undetermined amount: one 10 s step, like the desktop default.
            IpcMessage::SeekBy(match dir {
                souvlaki::SeekDirection::Forward => 10_000,
                souvlaki::SeekDirection::Backward => -10_000,
            })
        }
        MediaControlEvent::SeekBy(dir, dur) => {
            let ms = dur.as_millis().min(i64::MAX as u128) as i64;
            IpcMessage::SeekBy(match dir {
                souvlaki::SeekDirection::Forward => ms,
                souvlaki::SeekDirection::Backward => -ms,
            })
        }
        MediaControlEvent::SetPosition(MediaPosition(pos)) => {
            IpcMessage::SeekTo(pos.as_millis().min(u64::MAX as u128) as u64)
        }
        MediaControlEvent::SetVolume(v) => IpcMessage::Volume((v.clamp(0.0, 1.0) * 100.0) as u8),
        MediaControlEvent::OpenUri(uri) => IpcMessage::OpenLink(uri),
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

    /// Forget callbacks emitted while the OS media session was being attached.
    ///
    /// Windows may restore the previous SMTC transport state as an initial
    /// callback. That state belongs to the old process and must not turn a
    /// freshly launched, deliberately paused Fastcloud session into playback.
    pub fn discard_pending_events(&mut self) {
        while self.rx.try_recv().is_ok() {}
    }

    pub fn detach(&mut self) {
        if let Some(c) = &mut self.controls {
            let _ = c.detach();
        }
    }
}
