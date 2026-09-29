use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use std::{
    sync::{Arc, mpsc},
    time::Duration,
};
use tauri::Manager;

use crate::player::Player;

pub fn spawn(app: tauri::AppHandle, player: Arc<Player>, hwnd: Option<usize>) {
    let started = std::thread::Builder::new()
        .name("fastcloud-media".into())
        .spawn(move || {
            let (tx, rx) = mpsc::channel();
            let config = PlatformConfig {
                dbus_name: "fastcloud",
                display_name: "Fastcloud",
                hwnd: hwnd.map(|value| value as *mut std::ffi::c_void),
            };
            let Ok(mut controls) = MediaControls::new(config) else {
                return;
            };
            if let Err(error) = controls.attach(move |event| {
                let _ = tx.send(event);
            }) {
                log::warn!("media controls unavailable: {error}");
                return;
            }
            while rx.try_recv().is_ok() {} // Discard stale OS state on launch.
            let mut last_track = None;
            let mut last_playing = None;
            loop {
                while let Ok(event) = rx.try_recv() {
                    match event {
                        MediaControlEvent::Play => player.play(),
                        MediaControlEvent::Pause => player.pause(),
                        MediaControlEvent::Toggle => player.play_pause(),
                        MediaControlEvent::Next => {
                            player.next();
                        }
                        MediaControlEvent::Previous => {
                            player.prev();
                        }
                        MediaControlEvent::Stop => player.stop(),
                        MediaControlEvent::Seek(direction) => {
                            let position = player.state.lock().position_ms;
                            player.seek_ms(match direction {
                                SeekDirection::Forward => position.saturating_add(10_000),
                                SeekDirection::Backward => position.saturating_sub(10_000),
                            });
                        }
                        MediaControlEvent::SeekBy(direction, amount) => {
                            let position = player.state.lock().position_ms;
                            let amount = amount.as_millis().min(u64::MAX as u128) as u64;
                            player.seek_ms(match direction {
                                SeekDirection::Forward => position.saturating_add(amount),
                                SeekDirection::Backward => position.saturating_sub(amount),
                            });
                        }
                        MediaControlEvent::SetPosition(MediaPosition(position)) => {
                            player.seek_ms(position.as_millis().min(u64::MAX as u128) as u64)
                        }
                        MediaControlEvent::SetVolume(volume) => {
                            player.set_volume(volume.clamp(0.0, 1.0) as f32)
                        }
                        MediaControlEvent::Raise => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        MediaControlEvent::Quit => {
                            app.exit(0);
                            return;
                        }
                        MediaControlEvent::OpenUri(_) => {}
                    }
                }
                let (track, playing, position, length) = {
                    let state = player.state.lock();
                    (
                        state
                            .current
                            .and_then(|index| state.queue.get(index))
                            .cloned(),
                        state.is_playing,
                        state.position_ms,
                        state.duration_ms,
                    )
                };
                if let Some(track) = track {
                    if last_track != Some(track.id) {
                        let _ = controls.set_metadata(MediaMetadata {
                            title: Some(&track.title),
                            artist: Some(track.artist()),
                            album: None,
                            cover_url: track.artwork_url(),
                            duration: Some(Duration::from_millis(length)),
                        });
                        last_track = Some(track.id);
                    }
                    if last_playing != Some(playing) || playing {
                        let progress = Some(MediaPosition(Duration::from_millis(position)));
                        let playback = if playing {
                            MediaPlayback::Playing { progress }
                        } else {
                            MediaPlayback::Paused { progress }
                        };
                        let _ = controls.set_playback(playback);
                        last_playing = Some(playing);
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
    if let Err(error) = started {
        log::warn!("cannot start media integration: {error}");
    }
}
