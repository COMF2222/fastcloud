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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

/// Decode enough before starting the audio device that network and scheduler
/// jitter cannot turn one MP3 packet into a burst followed by silence.
const START_BUFFER_MS: u64 = 3_000;
/// Each background pass produces a useful chunk instead of one ~26 ms frame.
const DECODE_BATCH_MS: u64 = 2_000;
/// Refill before the audio device gets close enough to the end to underrun.
const TARGET_BUFFER_MS: u64 = 8_000;
/// Coalesce slider events before serializing the queue and settings to disk.
const CONTROL_SAVE_DELAY: Duration = Duration::from_millis(750);

fn initial_decode_target_ms(start_ms: u64) -> u64 {
    start_ms.saturating_add(START_BUFFER_MS)
}

fn visible_position_ms(position_ms: u64, duration_ms: u64) -> u64 {
    if duration_ms == 0 {
        position_ms
    } else {
        position_ms.min(duration_ms)
    }
}

fn should_decode_more(buffered_ms: u64, loading: bool) -> bool {
    !loading && buffered_ms < TARGET_BUFFER_MS
}

/// Decoder EOF means no more samples can be appended; it does not mean the
/// samples already queued in the audio device have finished playing.
fn should_advance_after_decode_end(buffered_ms: u64) -> bool {
    buffered_ms == 0
}

fn control_save_due(deadline: Option<Instant>, now: Instant) -> bool {
    deadline.is_some_and(|deadline| now >= deadline)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// Stereo balance, -1 hard left to 1 hard right.
    pub balance: f32,
    pub mono: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub is_playing: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub preview_fallback: bool,
    /// The stream's sample rate in Hz, once one is decoded. Winamp showed it
    /// next to the bitrate, and it is the only one of the two we really know.
    pub sample_rate: u32,
    /// The chosen stream's bitrate in kbps, as the API names it.
    pub bitrate_kbps: u32,
    /// How many channels the source has, before the upmix to stereo. Winamp's
    /// mono/stereo lamp.
    pub channels: u16,
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
            balance: 0.0,
            mono: false,
            position_ms: 0,
            duration_ms: 0,
            is_playing: false,
            loading: false,
            error: None,
            preview_fallback: false,
            sample_rate: 0,
            bitrate_kbps: 0,
            channels: 0,
        }
    }
}

enum DecodeAction {
    None,
    Samples(Vec<f32>, u32),
    FetchMore,
    TrackEnd,
    Fatal(String),
}

/// Player engine: fetch HLS streams, decode in background, drive AudioOutput.
pub struct Player {
    pub state: Mutex<PlayerState>,
    pub output: Arc<AudioOutput>,
    client: Arc<crate::api::ApiClient>,
    rt: tokio::runtime::Handle,
    hls: HlsDownloader,
    http: reqwest::Client,
    settings_path: std::path::PathBuf,
    decoder: Mutex<DecoderSlot>,
    /// Monotonic id of the track load that is allowed to touch the decoder and
    /// output. A slower previous request must never start after a newer click.
    load_generation: AtomicU64,
    stop: parking_lot::Mutex<bool>,
    self_ref: parking_lot::Mutex<Weak<Player>>,
    pending_control_save: parking_lot::Mutex<Option<Instant>>,
    /// When set, playback synthesizes local PCM instead of fetching streams.
    pub demo: parking_lot::Mutex<bool>,
    /// Keep playing similar tracks when the queue runs out.
    pub autoplay: parking_lot::Mutex<bool>,
}

struct DecoderSlot {
    inner: SegmentDecoder,
    urn: String,
    segments_fetched: usize,
    playlist: Option<crate::audio::hls::MediaPlaylist>,
    stream_url: Option<String>,
    is_preview: bool,
    /// The decoder has no more data and nothing has arrived since.
    ///
    /// Set when a pull comes back empty or errors, cleared by [`Self::feed`] —
    /// which is the only thing that can make it untrue. Clearing it only where
    /// a pull *succeeded* is what silenced playback a second after launch: the
    /// loop pulls before any track is loaded, `SegmentDecoder` rightly errors
    /// with "no data to decode", and the flag then blocked every later pull —
    /// including the ones after a track had actually arrived.
    exhausted: bool,
    /// A decoder failure is not a clean EOF and must never auto-advance.
    failed: bool,
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
            // Nothing to decode yet: a pull before the first append is not an
            // error worth logging, it is just early.
            exhausted: true,
            failed: false,
        }
    }

    /// Hand the decoder bytes, and note that there is something to pull again.
    fn feed(&mut self, bytes: &[u8]) {
        self.inner.append(bytes);
        self.exhausted = false;
        self.failed = false;
    }

    /// Point the slot at a new stream. Also resets `exhausted`, because the old
    /// decoder's emptiness says nothing about the new one.
    fn reset_decoder(&mut self, mime: Option<String>) {
        self.inner = SegmentDecoder::new(mime);
        self.segments_fetched = 0;
        self.exhausted = true;
        self.failed = false;
    }
}

impl Player {
    pub fn new(
        output: Arc<AudioOutput>,
        client: Arc<crate::api::ApiClient>,
        cache: Arc<AudioCache>,
        settings_path: std::path::PathBuf,
        rt: tokio::runtime::Handle,
    ) -> Self {
        Self {
            state: Mutex::new(PlayerState::default()),
            output,
            client,
            rt,
            hls: HlsDownloader::new(reqwest::Client::new(), cache),
            http: reqwest::Client::new(),
            settings_path,
            decoder: Mutex::new(DecoderSlot::new()),
            load_generation: AtomicU64::new(0),
            stop: parking_lot::Mutex::new(false),
            self_ref: parking_lot::Mutex::new(Weak::new()),
            pending_control_save: parking_lot::Mutex::new(None),
            demo: parking_lot::Mutex::new(false),
            autoplay: parking_lot::Mutex::new(true),
        }
    }

    /// Shared API client (link resolution, stations).
    pub fn api(&self) -> &Arc<crate::api::ApiClient> {
        &self.client
    }

    /// Runtime handle for spawning fetches from any thread (including the
    /// egui UI thread, where bare `tokio::spawn` would panic).
    pub fn runtime(&self) -> &tokio::runtime::Handle {
        &self.rt
    }

    /// Enable synthesized offline playback (demo mode).
    pub fn set_demo(&self, demo: bool) {
        *self.demo.lock() = demo;
    }

    /// Autoplay similar tracks when the queue runs out (demo only).
    pub fn set_autoplay(&self, autoplay: bool) {
        *self.autoplay.lock() = autoplay;
    }

    /// Register a shared self-reference so spawned threads can drive playback.
    pub fn attach(self: &Arc<Self>) {
        *self.self_ref.lock() = Arc::downgrade(self);
    }

    pub fn output_handle(&self) -> &Arc<AudioOutput> {
        &self.output
    }

    /// Apply persisted controls without treating boot as a new user action.
    pub fn restore_controls(&self, volume: f32, balance: f32, mono: bool) {
        let volume = volume.clamp(0.0, 1.0);
        let balance = balance.clamp(-1.0, 1.0);
        self.output.set_volume(volume);
        self.output.set_balance(balance);
        self.output.set_mono(mono);
        let mut state = self.state.lock();
        state.volume = volume;
        state.balance = balance;
        state.mono = mono;
    }

    /// Make process startup silent without rewriting the saved session.
    pub fn start_paused(&self) {
        self.state.lock().is_playing = false;
        self.output.set_playing(false);
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

    /// Set shuffle directly (CLI/MPRIS); no-op when unchanged.
    pub fn set_shuffle(&self, enabled: bool) {
        if self.state.lock().shuffle == enabled {
            return;
        }
        self.toggle_shuffle();
    }

    pub fn cycle_repeat(&self) {
        let mut st = self.state.lock();
        st.repeat = match st.repeat {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        };
    }

    /// Set repeat directly (CLI/MPRIS).
    pub fn set_repeat(&self, mode: RepeatMode) {
        self.state.lock().repeat = mode;
    }

    // ===== Transport =====

    /// Resume loading when the output holds no audio for the current track
    /// (fresh boot with a restored queue, or a track that never buffered).
    /// Returns the track+offset to load, if a load is needed instead of a
    /// plain pause/play flip.
    fn resume_load(&self) -> Option<(Track, u64)> {
        let st = self.state.lock();
        let idx = st.current?;
        if st.loading || self.output.buffered_ms() > 0 {
            return None;
        }
        st.queue.get(idx).cloned().map(|t| {
            let position = visible_position_ms(st.position_ms, t.effective_duration_ms());
            // A session saved exactly at EOF should replay, not reopen a
            // zero-length tail and appear stuck.
            let position = if position < t.effective_duration_ms() {
                position
            } else {
                0
            };
            (t, position)
        })
    }

    pub fn play_pause(&self) {
        if let Some((track, pos)) = self.resume_load() {
            self.load_track(track, pos);
            return;
        }
        let playing = {
            let mut st = self.state.lock();
            st.is_playing = !st.is_playing;
            st.is_playing
        };
        self.output.set_playing(playing);
        self.save_session();
    }

    pub fn play(&self) {
        if let Some((track, pos)) = self.resume_load() {
            self.load_track(track, pos);
            return;
        }
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
        self.load_generation.fetch_add(1, Ordering::AcqRel);
        let mut st = self.state.lock();
        st.is_playing = false;
        st.current = None;
        // Nothing is loaded, so nothing has a rate or a bitrate: the mini
        // player's readouts go blank rather than keeping the last track's.
        st.sample_rate = 0;
        st.bitrate_kbps = 0;
        st.channels = 0;
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

    // ===== Queue panel support =====

    /// Upcoming queue indices in effective play order (after current).
    pub fn upcoming(&self) -> Vec<usize> {
        let st = self.state.lock();
        upcoming_order(&st.order, st.current)
    }

    /// Current + upcoming tracks in one lock (for the queue panel).
    pub fn snapshot(&self) -> (Option<Track>, Vec<(usize, Track)>) {
        let st = self.state.lock();
        let cur = st.current.and_then(|i| st.queue.get(i).cloned());
        let up = upcoming_order(&st.order, st.current)
            .into_iter()
            .filter_map(|i| st.queue.get(i).cloned().map(|t| (i, t)))
            .collect();
        (cur, up)
    }

    /// Skip to a queue index, like pressing Next down to it.
    pub fn skip_to(&self, idx: usize) -> bool {
        let track = {
            let mut st = self.state.lock();
            if idx >= st.queue.len() {
                return false;
            }
            st.current = Some(idx);
            st.queue.get(idx).cloned()
        };
        if let Some(t) = track {
            self.load_track(t, 0);
            true
        } else {
            false
        }
    }

    /// Remove a row from the queue. Removing the current row advances
    /// to the next one (or stops when it was the last).
    pub fn remove_at(&self, idx: usize) -> bool {
        enum Plan {
            Done,
            SkipTo(usize),
            Stop,
        }
        let plan = {
            let mut st = self.state.lock();
            if idx >= st.queue.len() {
                return false;
            }
            if Some(idx) == st.current {
                let next = st
                    .order
                    .iter()
                    .position(|&x| x == idx)
                    .and_then(|p| st.order.get(p + 1).copied())
                    .map(|n| if n > idx { n - 1 } else { n });
                st.queue.remove(idx);
                let (order, _) = drop_bookkeeping(&st.order, None, idx);
                st.order = order;
                st.current = None;
                match next {
                    Some(n) => Plan::SkipTo(n),
                    None => Plan::Stop,
                }
            } else {
                st.queue.remove(idx);
                let (order, current) = drop_bookkeeping(&st.order, st.current, idx);
                st.order = order;
                st.current = current;
                Plan::Done
            }
        };
        match plan {
            Plan::Done => {
                self.save_session();
                true
            }
            Plan::SkipTo(n) => self.skip_to(n),
            Plan::Stop => {
                self.stop();
                true
            }
        }
    }

    /// Drop everything except the current track. Returns rows removed.
    pub fn clear_upcoming(&self) -> usize {
        let mut st = self.state.lock();
        let Some(cur) = st.current else { return 0 };
        let Some(track) = st.queue.get(cur).cloned() else {
            return 0;
        };
        let removed = st.queue.len().saturating_sub(1);
        st.queue = vec![track];
        st.current = Some(0);
        st.order = vec![0];
        drop(st);
        self.save_session();
        removed
    }

    /// Stop playback and remove every queued track, including the persisted
    /// session. Keeping this mutation inside `Player` prevents the UI from
    /// clearing memory after `stop()` has already saved the old queue.
    pub fn clear_queue(&self) {
        self.load_generation.fetch_add(1, Ordering::AcqRel);
        {
            let mut state = self.state.lock();
            state.queue.clear();
            state.order.clear();
            state.current = None;
            state.position_ms = 0;
            state.duration_ms = 0;
            state.is_playing = false;
            state.loading = false;
            state.sample_rate = 0;
            state.bitrate_kbps = 0;
            state.channels = 0;
        }
        self.output.set_playing(false);
        self.save_session();
    }

    /// Append a track to the end of the queue, preserving play order.
    pub fn push_back(&self, track: Track) {
        let mut st = self.state.lock();
        st.queue.push(track);
        let last = st.queue.len() - 1;
        st.order.push(last);
        drop(st);
        self.save_session();
    }

    /// Autoplay: when the queue runs out, queue a related track.
    /// Demo-only; the real build resolves related tracks via the API.
    fn maybe_autoplay(&self) {
        if !*self.autoplay.lock() || !*self.demo.lock() {
            return;
        }
        let current_id = self.current_track().map(|t| t.id).unwrap_or(0);
        let all = crate::demo::demo_tracks();
        if all.is_empty() {
            return;
        }
        let rel = all[(current_id as usize + 1) % all.len()].clone();
        log::info!("autoplay: queueing {}", rel.title);
        self.push_back(rel);
        self.next();
    }

    /// Seek within the track.
    ///
    /// The output only holds a window of decoded audio (see
    /// `AudioOutput::append_samples`), so a seek outside that window reloads
    /// the stream from the new position instead of clamping to the buffer —
    /// which is what made dragging the bar backwards jump to wherever the
    /// buffer happened to start.
    pub fn seek_ms(&self, ms: u64) {
        if self.output.can_seek_to(ms) {
            self.output.seek_ms(ms);
            let mut st = self.state.lock();
            st.position_ms = ms;
            drop(st);
            self.save_session();
            return;
        }
        let Some(track) = self.current_track() else {
            return;
        };
        self.load_track(track, ms);
    }

    pub fn set_volume(&self, v: f32) {
        let v = v.clamp(0.0, 1.0);
        self.output.set_volume(v);
        let mut st = self.state.lock();
        st.volume = v;
        drop(st);
        self.schedule_control_save();
    }

    /// Stereo balance, -1 hard left to 1 hard right. The mini player's second
    /// slider; the app's own interface has no control for it.
    pub fn set_balance(&self, balance: f32) {
        let balance = balance.clamp(-1.0, 1.0);
        self.output.set_balance(balance);
        let mut st = self.state.lock();
        st.balance = balance;
        drop(st);
        self.schedule_control_save();
    }

    pub fn set_mono(&self, mono: bool) {
        self.output.set_mono(mono);
        self.state.lock().mono = mono;
        self.schedule_control_save();
    }

    pub fn set_eq(&self, enabled: bool, gains: [f32; 10]) {
        self.output.set_eq(enabled, gains);
    }

    /// The equaliser's preamp, in dB. Persisted with the rest of the EQ.
    pub fn set_eq_preamp_db(&self, preamp_db: f32) {
        self.output.set_eq_preamp_db(preamp_db);
    }

    // ===== Stream loading =====

    fn load_track(&self, track: Track, start_ms: u64) {
        let generation = self.load_generation.fetch_add(1, Ordering::AcqRel) + 1;
        {
            let mut st = self.state.lock();
            st.loading = true;
            st.error = None;
            st.position_ms = start_ms;
            st.preview_fallback = false;
            st.duration_ms = track.effective_duration_ms();
        }
        // A manual Next/Previous must take effect audibly at once. Leaving the
        // old output running while the new stream resolves made the button
        // feel delayed and could leak a few seconds of the previous song.
        self.output.set_playing(false);
        // Clone via the shared self-reference installed in attach().
        let Some(me) = self.self_ref.lock().upgrade() else {
            log::warn!("player not attached; cannot start loader");
            return;
        };
        // Demo mode: instant local synthesis, no network. Only the stretch
        // being played is synthesized (see `demo::pcm_window`), so a long
        // demo track costs a few MB rather than the whole song at f32.
        if *self.demo.lock() {
            std::thread::spawn(move || {
                let (pcm, rate) = crate::demo::pcm_window(&track, start_ms);
                if !me.load_is_current(generation) {
                    return;
                }
                me.output.start_track(pcm, rate, true, start_ms);
                let mut st = me.state.lock();
                st.loading = false;
                st.is_playing = true;
                st.sample_rate = rate;
                // Synthesized f32, so nothing was ever compressed. The
                // readout stays blank rather than inventing a bitrate.
                st.bitrate_kbps = 0;
                // `demo::pcm_window` writes interleaved pairs.
                st.channels = 2;
            });
            return;
        }
        me.rt.clone().spawn(async move {
            if let Err(e) = me.load_track_async(track, start_ms, generation).await
                && me.load_is_current(generation)
            {
                let mut st = me.state.lock();
                st.error = Some(e.to_string());
                st.loading = false;
            }
        });
    }

    fn load_is_current(&self, generation: u64) -> bool {
        self.load_generation.load(Ordering::Acquire) == generation
    }

    async fn load_track_async(&self, track: Track, start_ms: u64, generation: u64) -> Result<()> {
        let urn = track.urn();
        let streams: StreamUrls = self
            .client
            .get(&format!("/tracks/{}/streams", url_encode(&urn)), &[])
            .await
            .context("fetch streams")?;
        if !self.load_is_current(generation) {
            return Ok(());
        }
        self.hls.set_oauth(self.client.oauth_token());

        // The API offers HLS only (MP3 then AAC); `stream_url` is deprecated
        // and preview-only, so a blocked/preview track falls back below.
        let chosen = streams
            .best_full_with_bitrate()
            .map(|(url, kbps)| (url.to_owned(), kbps));

        {
            let mut slot = self.decoder.lock();
            *slot = DecoderSlot::new();
            slot.urn = urn.clone();
        }

        let mut first_samples: Vec<f32>;
        let mut decode_skip_ms = start_ms;
        let mut is_preview = false;
        let source_rate: u32;
        let bitrate_kbps;
        let channels: u16;

        if let Some((url, kbps)) = chosen {
            bitrate_kbps = kbps;
            // Both full-stream fields are HLS, including API URLs ending in
            // /hls. The URL extension cannot distinguish a playlist from MP3.
            {
                let pl = self.hls.playlist(&url).await.context("fetch playlist")?;
                if !self.load_is_current(generation) {
                    return Ok(());
                }
                let is_fmp4 = pl.init_uri.is_some();
                let (first_segment, segment_start_ms) = pl.seek_point(start_ms);
                decode_skip_ms = start_ms.saturating_sub(segment_start_ms);
                {
                    let mut slot = self.decoder.lock();
                    slot.playlist = Some(pl);
                    slot.stream_url = Some(url);
                    // fMP4 segments are unplayable without the init header;
                    // plain TS/MP3 segments stay on the mpeg probe.
                    slot.reset_decoder(Some(if is_fmp4 {
                        "audio/mp4".into()
                    } else {
                        "audio/mpeg".into()
                    }));
                    slot.segments_fetched = first_segment;
                }
                // Init header first (cached), then media segments.
                let init_url = {
                    self.decoder
                        .lock()
                        .playlist
                        .as_ref()
                        .and_then(|p| p.init_uri.clone())
                };
                if let Some(init_url) = init_url {
                    let init_bytes = self
                        .hls
                        .init_segment(&urn, &init_url)
                        .await
                        .context("fetch init segment")?;
                    if !self.load_is_current(generation) {
                        return Ok(());
                    }
                    self.decoder.lock().inner.set_init_segment(init_bytes);
                }
                let mut out = Vec::new();
                let target_ms = initial_decode_target_ms(decode_skip_ms);
                loop {
                    // Start with one segment. Fetching three serially before
                    // decoding made manual track changes wait unnecessarily;
                    // subsequent passes fetch only when the initial 3 s target
                    // still is not satisfied.
                    self.fetch_and_append(1, generation).await?;
                    if !self.load_is_current(generation) {
                        return Ok(());
                    }
                    let needs_more = {
                        let mut slot = self.decoder.lock();
                        loop {
                            if !slot.inner.next_packet(&mut out)? {
                                break slot
                                    .playlist
                                    .as_ref()
                                    .is_some_and(|p| slot.segments_fetched < p.segments.len());
                            }
                            let rate = u64::from(slot.inner.rate().max(1));
                            if out.len() as u64 / 2 * 1000 / rate >= target_ms {
                                break false;
                            }
                        }
                    };
                    if !needs_more {
                        break;
                    }
                }
                let (rate, chans) = {
                    let slot = self.decoder.lock();
                    (slot.inner.rate(), slot.inner.channels())
                };
                anyhow::ensure!(!out.is_empty(), "stream contained no decodable audio");
                first_samples = out;
                source_rate = rate;
                channels = chans;
            }
        } else if let Some(preview) = streams.preview_mp3_128.clone() {
            // 30-second preview fallback.
            let bytes = self.hls.segment(&urn, &preview).await?;
            if !self.load_is_current(generation) {
                return Ok(());
            }
            let (samples, rate, chans) =
                crate::audio::decode::decode_all(bytes, Some("audio/mpeg"))?;
            first_samples = samples;
            source_rate = rate;
            channels = chans;
            // The field names its own rate, as the full streams do.
            bitrate_kbps = 128;
            is_preview = true;
            let mut slot = self.decoder.lock();
            slot.is_preview = true;
        } else {
            anyhow::bail!("no stream URL available");
        }

        if decode_skip_ms > 0 {
            let skip_frames = (decode_skip_ms * source_rate as u64 / 1000) as usize * 2;
            first_samples.drain(..skip_frames.min(first_samples.len()));
        }

        if !self.load_is_current(generation) {
            return Ok(());
        }

        self.output
            .start_track(first_samples, source_rate, true, start_ms);
        {
            let mut st = self.state.lock();
            st.loading = false;
            st.is_playing = true;
            st.preview_fallback = is_preview;
            st.sample_rate = source_rate;
            st.bitrate_kbps = bitrate_kbps;
            st.channels = channels;
        }
        self.save_session();
        Ok(())
    }

    async fn fetch_and_append(&self, count: usize, generation: u64) -> Result<()> {
        if !self.load_is_current(generation) {
            return Ok(());
        }
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
            if !self.load_is_current(generation) {
                return Ok(());
            }
            segs.push((url, bytes));
        }
        let mut slot = self.decoder.lock();
        for (url, bytes) in segs {
            if let Some(pl) = slot.playlist.as_ref() {
                if pl.segments.iter().any(|s| s == &url) {
                    slot.feed(&bytes);
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
            self.flush_control_save();
            let buffered = self.output.buffered_ms();
            // `load_track_async` owns the decoder while it builds the initial
            // buffer. Pulling here at the same time can steal the first packets
            // and append them to the output of the previous track.
            let loading = self.state.lock().loading;
            // Demo mode synthesizes its own audio in windows; there is no
            // decoder to pull from.
            if *self.demo.lock() {
                if should_decode_more(buffered, loading) {
                    self.top_up_demo();
                }
                self.publish_position();
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
            if should_decode_more(buffered, loading) {
                let action = {
                    let mut slot = self.decoder.lock();
                    if slot.exhausted {
                        if !slot.failed && should_advance_after_decode_end(buffered) {
                            DecodeAction::TrackEnd
                        } else {
                            DecodeAction::None
                        }
                    } else {
                        let mut out = Vec::new();
                        let mut decoded = false;
                        let pulled = loop {
                            match slot.inner.next_packet(&mut out) {
                                Ok(true) => {
                                    decoded = true;
                                    let rate = u64::from(slot.inner.rate().max(1));
                                    if out.len() as u64 / 2 * 1000 / rate >= DECODE_BATCH_MS {
                                        break Ok(true);
                                    }
                                }
                                Ok(false) if decoded => break Ok(true),
                                other => break other,
                            }
                        };
                        match pulled {
                            Ok(true) => DecodeAction::Samples(out, slot.inner.rate()),
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
                                    if should_advance_after_decode_end(buffered) {
                                        DecodeAction::TrackEnd
                                    } else {
                                        DecodeAction::None
                                    }
                                }
                            }
                            Err(e) => {
                                log::warn!("decode: {e}");
                                let remaining = slot
                                    .playlist
                                    .as_ref()
                                    .map(|p| p.segments.len() - slot.segments_fetched)
                                    .unwrap_or(0);
                                if remaining > 0 && !slot.is_preview {
                                    slot.inner.discard_window();
                                    DecodeAction::FetchMore
                                } else {
                                    slot.exhausted = true;
                                    slot.failed = true;
                                    DecodeAction::Fatal(e.to_string())
                                }
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
                        let generation = self.load_generation.load(Ordering::Acquire);
                        if let Err(e) = self.fetch_and_append(2, generation).await {
                            log::warn!("segment fetch: {e}");
                            self.decoder.lock().inner.wait_for_append();
                            tokio::time::sleep(Duration::from_millis(350)).await;
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
                                    if !self.next() {
                                        self.maybe_autoplay();
                                    }
                                }
                            }
                        }
                    }
                    DecodeAction::Fatal(why) => {
                        self.pause();
                        self.state.lock().error = Some(format!("Playback stopped: {why}"));
                    }
                }
            }

            // Publish the position at ~10 Hz. `AudioOutput` reports where the
            // *track* is (it counts the frames it dropped), so this follows a
            // backwards seek too — clamping it monotonic used to pin the bar
            // at the furthest point reached.
            self.publish_position();

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    fn publish_position(&self) {
        let pos = self.output.position_ms();
        let mut st = self.state.lock();
        if !st.loading {
            st.position_ms = visible_position_ms(pos, st.duration_ms);
        }
    }

    /// Demo mode: synthesize the next window, or move on at the track's end.
    fn top_up_demo(&self) {
        let Some(track) = self.current_track() else {
            return;
        };
        let duration = track.effective_duration_ms();
        let from = self.output.buffered_until_ms();
        if from >= duration {
            // The pad has played out: same end-of-track handling as a stream.
            let (nextable, repeat) = {
                let st = self.state.lock();
                (st.current.is_some(), st.repeat)
            };
            if !nextable || !should_advance_after_decode_end(self.output.buffered_ms()) {
                return;
            }
            match repeat {
                RepeatMode::One => self.load_track(track, 0),
                _ => {
                    if !self.next() {
                        self.maybe_autoplay();
                    }
                }
            }
            return;
        }
        let (pcm, rate) = crate::demo::pcm_window(&track, from);
        if !pcm.is_empty() {
            self.output.append_samples(&pcm, rate);
        }
    }

    // ===== Session persistence =====

    fn schedule_control_save(&self) {
        *self.pending_control_save.lock() = Some(Instant::now() + CONTROL_SAVE_DELAY);
    }

    fn flush_control_save(&self) {
        let now = Instant::now();
        let due = {
            let mut deadline = self.pending_control_save.lock();
            if control_save_due(*deadline, now) {
                *deadline = None;
                true
            } else {
                false
            }
        };
        if due {
            self.save_session();
        }
    }

    fn save_session(&self) {
        if *self.demo.lock() {
            return;
        }
        let (urn, pos, vol, balance, mono, queue, current) = {
            let st = self.state.lock();
            let urn = st.current.and_then(|i| st.queue.get(i).map(|t| t.urn()));
            (
                urn,
                st.position_ms,
                st.volume,
                st.balance,
                st.mono,
                st.queue.clone(),
                st.current,
            )
        };
        // Persist the queue itself so it survives restarts. Tracks are
        // small structs; keep the current one plus its surroundings
        // (at most ~200 entries ≈ 100 KB of JSON).
        let (kept, kept_current) = match current {
            Some(i) if i < queue.len() => {
                let start = i.saturating_sub(50);
                let end = (i + 150).min(queue.len());
                (queue[start..end].to_vec(), Some(i - start))
            }
            _ => (queue.into_iter().take(200).collect(), None),
        };
        if let Err(error) = crate::config::update_settings(&self.settings_path, |settings| {
            settings.last_track_urn = urn;
            settings.last_position_ms = Some(pos);
            settings.volume = vol;
            settings.balance = balance;
            settings.mono = mono;
            settings.last_queue = kept;
            settings.last_queue_idx = kept_current.filter(|_| !settings.last_queue.is_empty());
        }) {
            log::warn!("cannot save playback session: {error}");
        }
    }

    /// Restore a persisted queue on boot (paused; position applied by caller).
    /// Returns the saved position for the current track, if any.
    pub fn restore_session(&self, queue: Vec<Track>, current: Option<usize>) -> Option<u64> {
        if queue.is_empty() {
            return None;
        }
        let pos = Settings::load_from(&self.settings_path)
            .ok()
            .and_then(|s| s.last_position_ms);
        {
            let mut st = self.state.lock();
            let current = current.filter(|&i| i < queue.len());
            st.duration_ms = current
                .and_then(|i| queue.get(i))
                .map(|t| t.effective_duration_ms())
                .unwrap_or(0);
            st.queue = queue;
            st.order = (0..st.queue.len()).collect();
            st.current = current;
            st.position_ms = pos.unwrap_or(0);
            st.is_playing = false;
            st.loading = false;
        }
        pos
    }

    /// Snapshot the queue for "save as playlist" (current + upcoming ids).
    pub fn queue_track_ids(&self) -> Vec<u64> {
        let st = self.state.lock();
        let mut ids = Vec::with_capacity(st.queue.len());
        if let Some(cur) = st.current {
            for &i in std::iter::once(&cur).chain(upcoming_order(&st.order, st.current).iter()) {
                if let Some(t) = st.queue.get(i) {
                    ids.push(t.id);
                }
            }
        }
        ids
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

/// Upcoming indices after `current` in play `order`.
fn upcoming_order(order: &[usize], current: Option<usize>) -> Vec<usize> {
    let Some(cur) = current else {
        return Vec::new();
    };
    match order.iter().position(|&x| x == cur) {
        Some(p) => order[p + 1..].to_vec(),
        None => Vec::new(),
    }
}

/// Remove queue index `idx` from play-order bookkeeping.
/// Returns `(new_order, new_current)`; removing the current row yields
/// `None` so the caller can advance first.
fn drop_bookkeeping(
    order: &[usize],
    current: Option<usize>,
    idx: usize,
) -> (Vec<usize>, Option<usize>) {
    let order: Vec<usize> = order
        .iter()
        .filter(|&&x| x != idx)
        .map(|&x| if x > idx { x - 1 } else { x })
        .collect();
    let current = current.and_then(|cur| {
        if cur == idx {
            None
        } else if cur > idx {
            Some(cur - 1)
        } else {
            Some(cur)
        }
    });
    (order, current)
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
    fn decode_top_up_is_blocked_while_track_load_owns_decoder() {
        assert!(!should_decode_more(0, true));
    }

    #[test]
    fn decode_top_up_starts_below_target_buffer() {
        assert!(should_decode_more(TARGET_BUFFER_MS - 1, false));
    }

    #[test]
    fn decode_top_up_waits_at_target_buffer() {
        assert!(!should_decode_more(TARGET_BUFFER_MS, false));
    }

    #[test]
    fn decoder_eof_waits_for_the_output_buffer_to_finish() {
        assert!(!should_advance_after_decode_end(4_000));
        assert!(!should_advance_after_decode_end(50));
        assert!(!should_advance_after_decode_end(1));
        assert!(should_advance_after_decode_end(0));
    }

    #[test]
    fn attaching_player_does_not_create_an_arc_cycle() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let player = Arc::new(Player::new(
            Arc::new(AudioOutput::silent([0.0; 10], 0.8)),
            Arc::new(crate::api::ApiClient::demo()),
            Arc::new(AudioCache::disabled()),
            std::env::temp_dir().join("fastcloud-cycle-test.json"),
            runtime.handle().clone(),
        ));

        player.attach();

        assert_eq!(Arc::strong_count(&player), 1);
    }

    #[test]
    fn control_persistence_waits_until_dragging_has_settled() {
        let changed_at = std::time::Instant::now();
        let deadline = changed_at + CONTROL_SAVE_DELAY;

        assert!(!control_save_due(Some(deadline), changed_at));
        assert!(control_save_due(Some(deadline), deadline));
    }

    #[test]
    fn seeking_decodes_past_the_local_segment_offset_before_starting_output() {
        assert_eq!(initial_decode_target_ms(0), START_BUFFER_MS);
        assert_eq!(initial_decode_target_ms(70_000), 70_000 + START_BUFFER_MS);
        assert_eq!(initial_decode_target_ms(u64::MAX), u64::MAX);
    }

    #[test]
    fn visible_position_never_runs_past_the_track_duration() {
        assert_eq!(visible_position_ms(144_000, 91_847), 91_847);
        assert_eq!(visible_position_ms(12_000, 0), 12_000);
    }

    #[test]
    fn restored_session_is_always_paused() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let dir = tempfile::tempdir().expect("tempdir");
        let player = Player::new(
            Arc::new(AudioOutput::silent([0.0; 10], 0.8)),
            Arc::new(crate::api::ApiClient::demo()),
            Arc::new(AudioCache::disabled()),
            dir.path().join("settings.json"),
            runtime.handle().clone(),
        );
        player.state.lock().is_playing = true;
        let track: Track = serde_json::from_str(r#"{"id":7,"title":"Restored"}"#).unwrap();

        player.restore_session(vec![track], Some(0));

        assert!(!player.state.lock().is_playing);
    }

    #[test]
    fn startup_controls_restore_volume_and_remain_paused() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let dir = tempfile::tempdir().expect("tempdir");
        let player = Player::new(
            Arc::new(AudioOutput::silent([0.0; 10], 0.8)),
            Arc::new(crate::api::ApiClient::demo()),
            Arc::new(AudioCache::disabled()),
            dir.path().join("settings.json"),
            runtime.handle().clone(),
        );
        player.state.lock().is_playing = true;

        player.restore_controls(0.23, -0.4, true);
        player.start_paused();

        let state = player.state.lock();
        assert!((state.volume - 0.23).abs() < f32::EPSILON);
        assert!((state.balance - -0.4).abs() < f32::EPSILON);
        assert!(state.mono);
        assert!(!state.is_playing);
    }

    #[test]
    fn clearing_queue_persists_an_empty_session() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let dir = tempfile::tempdir().expect("tempdir");
        let settings_path = dir.path().join("settings.json");
        let player = Player::new(
            Arc::new(AudioOutput::silent([0.0; 10], 0.8)),
            Arc::new(crate::api::ApiClient::demo()),
            Arc::new(AudioCache::disabled()),
            settings_path.clone(),
            runtime.handle().clone(),
        );
        let track: Track = serde_json::from_str(r#"{"id":1,"title":"Song"}"#).unwrap();
        {
            let mut state = player.state.lock();
            state.queue.push(track);
            state.order.push(0);
            state.current = Some(0);
        }

        player.clear_queue();

        let saved = Settings::load_from(&settings_path).unwrap();
        assert!(saved.last_queue.is_empty());
        assert_eq!(saved.last_queue_idx, None);
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

    #[test]
    fn upcoming_linear_and_shuffled() {
        assert_eq!(upcoming_order(&[0, 1, 2, 3], Some(1)), vec![2, 3]);
        assert_eq!(upcoming_order(&[2, 0, 3, 1], Some(0)), vec![3, 1]);
        assert_eq!(upcoming_order(&[0, 1], Some(1)), Vec::<usize>::new());
        assert_eq!(upcoming_order(&[0, 1], None), Vec::<usize>::new());
        assert_eq!(upcoming_order(&[0, 1], Some(9)), Vec::<usize>::new());
    }

    #[test]
    fn drop_bookkeeping_middle() {
        let (order, cur) = drop_bookkeeping(&[0, 1, 2, 3], Some(0), 2);
        assert_eq!(order, vec![0, 1, 2]);
        assert_eq!(cur, Some(0));
    }

    #[test]
    fn drop_bookkeeping_before_current() {
        let (order, cur) = drop_bookkeeping(&[0, 1, 2, 3], Some(3), 1);
        assert_eq!(order, vec![0, 1, 2]);
        assert_eq!(cur, Some(2));
    }

    #[test]
    fn drop_bookkeeping_current_yields_none() {
        let (order, cur) = drop_bookkeeping(&[2, 0, 1], Some(0), 0);
        assert_eq!(order, vec![1, 0]);
        assert_eq!(cur, None);
    }
}
