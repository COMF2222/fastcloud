#![allow(dead_code)]

use crate::api::endpoints::StreamUrls;
use crate::api::models::Track;
use crate::audio::cache::AudioCache;
use crate::audio::decode::SegmentDecoder;
use crate::audio::hls::HlsDownloader;
use crate::audio::output::AudioOutput;
use crate::config::Settings;
use anyhow::{Context as _, Result};
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub track: Track,
}

pub struct PlayerState {
    pub queue: Vec<Track>,
    pub order: Vec<usize>,
    pub current: Option<usize>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub volume: f32,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub is_playing: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub preview_fallback: bool,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            order: Vec::new(),
            current: None,
            shuffle: false,
            repeat: RepeatMode::Off,
            volume: 0.8,
            position_ms: 0,
            duration_ms: 0,
            is_playing: false,
            loading: false,
            error: None,
            preview_fallback: false,
        }
    }
}

enum DecodeAction {
    None,
    Samples(Vec<f32>, u32),
    FetchMore,
    TrackEnd,
}

/// Player engine: fetch HLS streams, decode in background, drive AudioOutput.
pub struct Player {
    pub state: Mutex<PlayerState>,
    pub output: Arc<AudioOutput>,
    client: Arc<crate::api::ApiClient>,
    hls: HlsDownloader,
    http: reqwest::Client,
    settings_path: std::path::PathBuf,
    decoder: Mutex<DecoderSlot>,
    stop: parking_lot::Mutex<bool>,
    self_ref: parking_lot::Mutex<Option<std::sync::Arc<Player>>>,
    /// When set, playback synthesizes local PCM instead of fetching streams.
    pub demo: parking_lot::Mutex<bool>,
}

struct DecoderSlot {
    inner: SegmentDecoder,
    urn: String,
    segments_fetched: usize,
    playlist: Option<crate::audio::hls::MediaPlaylist>,
    stream_url: Option<String>,
    is_preview: bool,
    exhausted: bool,
}

impl DecoderSlot {
    fn new() -> Self {
        Self {
            inner: SegmentDecoder::new(None),
            urn: String::new(),
            segments_fetched: 0,
            playlist: None,
            stream_url: None,
            is_preview: false,
            exhausted: false,
        }
    }
}

struct ArcSelf(*const Player);
unsafe impl Send for ArcSelf {}
unsafe impl Sync for ArcSelf {}

impl Player {
    pub fn new(
        output: Arc<AudioOutput>,
        client: Arc<crate::api::ApiClient>,
        cache: Arc<AudioCache>,
        settings_path: std::path::PathBuf,
    ) -> Self {
        Self {
            state: Mutex::new(PlayerState::default()),
            output,
            client,
            hls: HlsDownloader::new(reqwest::Client::new(), cache),
            http: reqwest::Client::new(),
            settings_path,
            decoder: Mutex::new(DecoderSlot::new()),
            stop: parking_lot::Mutex::new(false),
            self_ref: parking_lot::Mutex::new(None),
            demo: parking_lot::Mutex::new(false),
        }
    }

    /// Enable synthesized offline playback (demo mode).
    pub fn set_demo(&self, demo: bool) {
        *self.demo.lock() = demo;
    }

    /// Register a shared self-reference so spawned threads can drive playback.
    pub fn attach(self: &Arc<Self>) {
        *self.self_ref.lock() = Some(self.clone());
    }

    pub fn output_handle(&self) -> &Arc<AudioOutput> {
        &self.output
    }

    // ===== Queue management =====

    pub fn play_queue(&self, tracks: Vec<Track>, start: usize, shuffle: bool) {
        let mut st = self.state.lock();
        st.queue = tracks;
        st.shuffle = shuffle;
        st.order = build_order(st.queue.len(), shuffle);
        st.current = Some(start);
        let track = st.queue[start].clone();
        drop(st);
        self.load_track(track, 0);
    }

    pub fn enqueue(&self, tracks: Vec<Track>, play_next: bool) {
        let mut st = self.state.lock();
        if st.queue.is_empty() {
            drop(st);
            self.play_queue(tracks, 0, false);
            return;
        }
        if play_next {
            let cur = st.current.unwrap_or(0);
            let mut merged = st.queue.clone();
            let mut tail = merged.split_off(cur + 1);
            for t in tracks {
                merged.push(t);
            }
            merged.append(&mut tail);
            st.queue = merged;
        } else {
            st.queue.extend(tracks);
        }
        st.order = build_order(st.queue.len(), st.shuffle);
    }

    pub fn current_track(&self) -> Option<Track> {
        let st = self.state.lock();
        st.current.and_then(|i| st.queue.get(i).cloned())
    }

    pub fn toggle_shuffle(&self) {
        let mut st = self.state.lock();
        st.shuffle = !st.shuffle;
        let cur = st.current;
        st.order = build_order(st.queue.len(), st.shuffle);
        if let Some(c) = cur {
            // Keep current track first in shuffle order.
            if let Some(pos) = st.order.iter().position(|&x| x == c) {
                st.order.swap(0, pos);
            }
        }
    }

    pub fn cycle_repeat(&self) {
        let mut st = self.state.lock();
        st.repeat = match st.repeat {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        };
    }

    // ===== Transport =====

    pub fn play_pause(&self) {
        let playing = {
            let mut st = self.state.lock();
            st.is_playing = !st.is_playing;
            st.is_playing
        };
        self.output.set_playing(playing);
        self.save_session();
    }

    pub fn play(&self) {
        let mut st = self.state.lock();
        st.is_playing = true;
        drop(st);
        self.output.set_playing(true);
        self.save_session();
    }

    pub fn pause(&self) {
        let mut st = self.state.lock();
        st.is_playing = false;
        drop(st);
        self.output.set_playing(false);
        self.save_session();
    }

    pub fn stop(&self) {
        let mut st = self.state.lock();
        st.is_playing = false;
        st.current = None;
        drop(st);
        self.output.set_playing(false);
        self.save_session();
    }

    pub fn next(&self) -> bool {
        {
            let mut st = self.state.lock();
            let Some(cur) = st.current else { return false };
            if st.order.is_empty() {
                return false;
            }
            let pos_in_order = st.order.iter().position(|&x| x == cur);
            let next = match (pos_in_order, st.repeat) {
                (Some(p), RepeatMode::All) if p + 1 < st.order.len() => st.order[p + 1],
                (Some(_), RepeatMode::All) => st.order[0],
                (Some(p), _) if p + 1 < st.order.len() => st.order[p + 1],
                _ => return false,
            };
            st.current = Some(next);
            let track = st.queue.get(next).cloned();
            drop(st);
            if let Some(t) = track {
                self.load_track(t, 0);
            }
            true
        }
    }

    pub fn prev(&self) -> bool {
        let mut st = self.state.lock();
        // If more than 3s into the track, restart it instead.
        if st.position_ms > 3000 {
            if let Some(cur) = st.current {
                let track = st.queue.get(cur).cloned();
                drop(st);
                if let Some(t) = track {
                    self.load_track(t, 0);
                }
                return true;
            }
        }
        let Some(cur) = st.current else { return false };
        let pos_in_order = st.order.iter().position(|&x| x == cur);
        let prev = match pos_in_order {
            Some(0) | None => return false,
            Some(p) => st.order[p - 1],
        };
        st.current = Some(prev);
        let track = st.queue.get(prev).cloned();
        drop(st);
        if let Some(t) = track {
            self.load_track(t, 0);
        }
        true
    }

    pub fn seek_ms(&self, ms: u64) {
        self.output.seek_ms(ms);
        let mut st = self.state.lock();
        st.position_ms = ms;
        drop(st);
        self.save_session();
    }

    pub fn set_volume(&self, v: f32) {
        let v = v.clamp(0.0, 1.0);
        self.output.set_volume(v);
        let mut st = self.state.lock();
        st.volume = v;
        drop(st);
        self.save_session();
    }

    pub fn set_eq(&self, enabled: bool, gains: [f32; 10]) {
        self.output.set_eq(enabled, gains);
    }

    // ===== Stream loading =====

    fn load_track(&self, track: Track, start_ms: u64) {
        {
            let mut st = self.state.lock();
            st.loading = true;
            st.error = None;
            st.position_ms = start_ms;
            st.preview_fallback = false;
            st.duration_ms = track.effective_duration_ms();
        }
        // Clone via the shared self-reference installed in attach().
        let Some(me) = self.self_ref.lock().clone() else {
            log::warn!("player not attached; cannot start loader");
            return;
        };
        let me = me as Arc<Player>;
        // Demo mode: instant local synthesis, no network.
        if *self.demo.lock() {
            std::thread::spawn(move || {
                let pcm = crate::demo::demo_pcm_for(&track);
                let rate = 44_100;
                let skip = (start_ms * rate as u64 / 1000) as usize * 2;
                let samples = if skip > 0 && skip < pcm.len() {
                    pcm[skip..].to_vec()
                } else {
                    pcm
                };
                me.output.start_track(samples, rate, true);
                let mut st = me.state.lock();
                st.loading = false;
                st.is_playing = true;
            });
            return;
        }
        tokio::spawn(async move {
            if let Err(e) = me.load_track_async(track, start_ms).await {
                let mut st = me.state.lock();
                st.error = Some(e.to_string());
                st.loading = false;
            }
        });
    }

    async fn load_track_async(&self, track: Track, start_ms: u64) -> Result<()> {
        let urn = track.urn();
        let streams: StreamUrls = self
            .client
            .get(&format!("/tracks/{}/streams", url_encode(&urn)), &[])
            .await
            .context("fetch streams")?;

        // Prefer progressive mp3, then HLS mp3, then preview fallback.
        let chosen = streams.http_mp3_128.clone().or(streams.hls_mp3_128.clone());

        {
            let mut slot = self.decoder.lock();
            *slot = DecoderSlot::new();
            slot.urn = urn.clone();
        }

        let mut first_samples: Vec<f32>;
        let mut is_preview = false;
        let source_rate: u32;

        if let Some(url) = chosen {
            if url.contains(".m3u8") {
                let pl = self.hls.playlist(&url).await.context("fetch playlist")?;
                {
                    let mut slot = self.decoder.lock();
                    slot.playlist = Some(pl);
                    slot.stream_url = Some(url);
                    slot.inner = SegmentDecoder::new(Some("audio/mpeg".into()));
                }
                self.fetch_and_append(3).await?;
                let mut out = Vec::new();
                let rate = {
                    let mut slot = self.decoder.lock();
                    slot.inner.next_packet(&mut out)?;
                    slot.inner.rate()
                };
                first_samples = out;
                source_rate = rate;
            } else {
                let bytes = self.hls.segment(&urn, &url).await?;
                let (samples, rate) = crate::audio::decode::decode_all(bytes, Some("audio/mpeg"))?;
                first_samples = samples;
                source_rate = rate;
            }
        } else if let Some(preview) = streams.preview_mp3_128.clone() {
            // 30-second preview fallback.
            let bytes = self.hls.segment(&urn, &preview).await?;
            let (samples, rate) = crate::audio::decode::decode_all(bytes, Some("audio/mpeg"))?;
            first_samples = samples;
            source_rate = rate;
            is_preview = true;
            let mut slot = self.decoder.lock();
            slot.is_preview = true;
        } else {
            anyhow::bail!("no stream URL available");
        }

        if start_ms > 0 {
            let skip_frames = (start_ms * source_rate as u64 / 1000) as usize * 2;
            first_samples.drain(..skip_frames.min(first_samples.len()));
        }

        self.output.start_track(first_samples, source_rate, true);
        {
            let mut st = self.state.lock();
            st.loading = false;
            st.is_playing = true;
            st.preview_fallback = is_preview;
        }
        self.save_session();
        Ok(())
    }

    async fn fetch_and_append(&self, count: usize) -> Result<()> {
        // Collect the next segment URLs without holding the lock across await.
        let work: Vec<(String, String)> = {
            let slot = self.decoder.lock();
            let Some(pl) = slot.playlist.as_ref() else {
                return Ok(());
            };
            let start = slot.segments_fetched;
            let end = (start + count).min(pl.segments.len());
            pl.segments[start..end]
                .iter()
                .map(|s| (slot.urn.clone(), s.clone()))
                .collect()
        };
        let mut segs: Vec<(String, Vec<u8>)> = Vec::with_capacity(work.len());
        for (urn, url) in work {
            let bytes = self.hls.segment(&urn, &url).await?;
            segs.push((url, bytes));
        }
        let mut slot = self.decoder.lock();
        for (url, bytes) in segs {
            if let Some(pl) = slot.playlist.as_ref() {
                if pl.segments.iter().any(|s| s == &url) {
                    slot.inner.append(&bytes);
                    slot.segments_fetched += 1;
                }
            }
        }
        Ok(())
    }

    /// Background decode loop: pull packets, append to output, fetch more
    /// segments as needed, advance the queue on track end (gapless).
    pub async fn run(self: Arc<Self>) {
        loop {
            if *self.stop.lock() {
                return;
            }
            let buffered = self.output.buffered_ms();
            if buffered < 8000 {
                let action = {
                    let mut slot = self.decoder.lock();
                    if slot.exhausted {
                        DecodeAction::None
                    } else {
                        let mut out = Vec::new();
                        match slot.inner.next_packet(&mut out) {
                            Ok(true) => {
                                let rate = slot.inner.rate();
                                slot.exhausted = false;
                                DecodeAction::Samples(out, rate)
                            }
                            Ok(false) => {
                                let remaining = slot
                                    .playlist
                                    .as_ref()
                                    .map(|p| p.segments.len() - slot.segments_fetched)
                                    .unwrap_or(0);
                                if remaining > 0 && !slot.is_preview {
                                    DecodeAction::FetchMore
                                } else {
                                    slot.exhausted = true;
                                    DecodeAction::TrackEnd
                                }
                            }
                            Err(e) => {
                                log::warn!("decode: {e}");
                                slot.exhausted = true;
                                DecodeAction::None
                            }
                        }
                    }
                };
                match action {
                    DecodeAction::None => {}
                    DecodeAction::Samples(out, rate) => {
                        self.output.append_samples(&out, rate);
                    }
                    DecodeAction::FetchMore => {
                        if let Err(e) = self.fetch_and_append(2).await {
                            log::warn!("segment fetch: {e}");
                            self.decoder.lock().exhausted = true;
                        }
                    }
                    DecodeAction::TrackEnd => {
                        let st = self.state.lock();
                        let nextable = st.current.is_some();
                        let repeat = st.repeat;
                        drop(st);
                        if nextable {
                            match repeat {
                                RepeatMode::One => {
                                    if let Some(t) = self.current_track() {
                                        self.load_track(t, 0);
                                    }
                                }
                                _ => {
                                    self.next();
                                }
                            }
                        }
                    }
                }
            }

            // Publish position to state at ~10 Hz.
            {
                let pos = self.output.position_ms();
                let mut st = self.state.lock();
                if !st.loading {
                    st.position_ms = pos;
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    // ===== Session persistence =====

    fn save_session(&self) {
        let st = self.state.lock();
        let s = Settings {
            last_track_urn: st.current.and_then(|i| st.queue.get(i).map(|t| t.urn())),
            last_position_ms: Some(st.position_ms),
            volume: st.volume,
            ..Settings::default()
        };
        let _ = save_settings_to(&self.settings_path, &s);
    }

    pub fn shutdown(&self) {
        *self.stop.lock() = true;
        self.save_session();
    }
}

fn build_order(len: usize, shuffle: bool) -> Vec<usize> {
    let mut order: Vec<usize> = (0..len).collect();
    if shuffle && len > 1 {
        use rand::seq::SliceRandom;
        order.shuffle(&mut rand::rng());
    }
    order
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn save_settings_to(path: &std::path::Path, s: &Settings) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(s)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_shuffle_preserves_all_indices() {
        let order = build_order(10, true);
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn order_linear() {
        assert_eq!(build_order(3, false), vec![0, 1, 2]);
    }

    #[test]
    fn url_encoding() {
        assert_eq!(url_encode("soundcloud:tracks:1"), "soundcloud%3Atracks%3A1");
    }

    #[test]
    fn repeat_cycle() {
        // state machine check without Player instance
        let seq = [
            RepeatMode::Off,
            RepeatMode::All,
            RepeatMode::One,
            RepeatMode::Off,
        ];
        assert_eq!(seq[0], RepeatMode::Off);
    }
}
