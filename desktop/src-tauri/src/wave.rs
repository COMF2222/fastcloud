use crate::api::{self, models::Track};
use chrono::{Local, Timelike};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, path::PathBuf, sync::Arc, time::Duration};

const MAX_LIKES: usize = 500;
const MAX_WAVE_TRACKS: usize = 40;

#[derive(Clone, Debug)]
pub struct TrackIdentity {
    id: u64,
    title: String,
    artist: Option<String>,
    performer_keys: Vec<String>,
    version: String,
    duration_ms: u64,
    audio_embedding: Option<Vec<i8>>,
}

fn words(value: &str) -> String {
    value.to_lowercase().replace('ё', "е").chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn performer_keys(value: &str) -> Vec<String> {
    let mut keys = vec![words(value)];
    for alias in value.split('|') {
        let key = words(alias);
        if key.chars().count() >= 4 && !keys.contains(&key) { keys.push(key); }
    }
    keys.retain(|key| !key.is_empty());
    keys
}

pub fn track_identity(track: &Track) -> TrackIdentity {
    let raw = track.title.trim();
    let known = words(track.artist());
    let anchored = [" - ", " – ", " — ", " | "].into_iter().find_map(|separator| {
        raw.match_indices(separator).find_map(|(index, _)| {
            let left = raw[..index].trim();
            let right = raw[index + separator.len()..].trim();
            if known.is_empty() || left.is_empty() || right.is_empty() { return None; }
            if words(left) == known { Some((track.artist(), right)) }
            else if words(right) == known { Some((track.artist(), left)) }
            else { None }
        })
    });
    let (title_artist, raw_title) = anchored.or_else(|| [" - ", " – ", " — "]
        .iter().find_map(|separator| raw.split_once(separator))
        .filter(|(artist, title)| !artist.trim().is_empty() && !title.trim().is_empty()
            && artist.chars().count() < 80))
        .map_or((None, raw), |(artist, title)| (Some(artist.trim().to_string()), title));
    let full = words(raw_title);
    let variants = ["remix", "live", "slowed", "sped up", "nightcore", "instrumental", "acoustic", "cover", "demo"];
    let is_variant = |value: &str| {
        let padded = format!(" {value} ");
        variants.iter().any(|term| padded.contains(&format!(" {term} ")))
    };
    let mut version_parts = Vec::new();
    let mut clean = raw_title.to_string();
    for (open, close) in [('(', ')'), ('[', ']')] {
        while let Some(start) = clean.find(open) {
            let Some(offset) = clean[start + 1..].find(close) else { break };
            let end = start + 1 + offset + 1;
            let qualifier = words(&clean[start + 1..end - 1]);
            if is_variant(&qualifier) { version_parts.push(qualifier); }
            clean.replace_range(start..end, " ");
        }
    }
    let version = if version_parts.is_empty() {
        variants.into_iter().filter(|term| {
            let padded = format!(" {full} ");
            padded.contains(&format!(" {term} "))
        }).collect::<Vec<_>>().join(" ")
    } else { version_parts.join(" ") };
    let mut title = words(&clean);
    for suffix in ["official audio", "official video", "official music video", "lyrics", "lyric video"] {
        title = title.trim_end_matches(suffix).trim().to_string();
    }
    if title.is_empty() { title = full; }
    let explicit = track.publisher_metadata.as_ref().and_then(|metadata| metadata.artist.as_deref())
        .or(track.metadata_artist.as_deref()).map(str::to_string).filter(|value| !value.trim().is_empty());
    let performer = title_artist.clone().or(explicit.clone())
        .unwrap_or_else(|| track.artist().to_string());
    TrackIdentity {
        id: track.id,
        title, performer_keys: performer_keys(&performer),
        artist: title_artist.or(explicit).map(|value| words(&value)), version,
        duration_ms: track.effective_duration_ms(),
        audio_embedding: None,
    }
}

pub fn same_recording(a: &TrackIdentity, b: &TrackIdentity) -> bool {
    if a.title.is_empty() || b.title.is_empty() || a.version != b.version { return false; }
    let same_title = a.title == b.title;
    let near_title = a.title.len().min(b.title.len()) >= 9
        && (a.title.contains(&b.title) || b.title.contains(&a.title));
    if !same_title && !near_title { return false; }
    let duration_close = a.duration_ms > 0 && b.duration_ms > 0
        && a.duration_ms.abs_diff(b.duration_ms) <= (a.duration_ms.min(b.duration_ms) / 20).clamp(6_000, 15_000);
    if duration_close && let (Some(left), Some(right)) = (&a.audio_embedding, &b.audio_embedding)
        && left.len() == right.len() && left.len() >= 256 {
        let dot: f64 = left.iter().zip(right).map(|(x, y)| f64::from(*x) * f64::from(*y)).sum();
        let norm = |v: &[i8]| v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
        if dot / (norm(left) * norm(right)).max(1.0) >= 0.985 { return true; }
    }
    if !same_title { return false; }
    if let (Some(left), Some(right)) = (&a.artist, &b.artist) {
        if left != right && !a.performer_keys.iter().any(|key| b.performer_keys.contains(key)) { return false; }
    }
    if a.duration_ms == 0 || b.duration_ms == 0 {
        return a.artist.is_some() && b.artist.is_some()
            && a.performer_keys.iter().any(|key| b.performer_keys.contains(key));
    }
    duration_close
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Signal {
    track: Option<Track>,
    plays: u32,
    engaged: u32,
    skips: u32,
    disliked: bool,
    dayparts: [u32; 4],
    engaged_dayparts: [u32; 4],
    skip_dayparts: [u32; 4],
    last_played: i64,
    audio_embedding: Option<Vec<i8>>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Profile {
    signals: HashMap<u64, Signal>,
    recent_starts: Vec<Track>,
    recommendations: Vec<Track>,
    start_counter: u32,
}

fn enriched_identity(track: &Track, profile: &Profile) -> TrackIdentity {
    let mut identity = track_identity(track);
    identity.audio_embedding = profile.signals.get(&identity.id)
        .and_then(|signal| signal.audio_embedding.clone());
    identity
}

#[derive(Clone)]
struct Observation {
    track: Track,
    position_ms: u64,
    duration_ms: u64,
    heard_ms: u64,
    daypart: usize,
    engagement_recorded: bool,
}

struct Inner {
    profile: Profile,
    current: Option<Observation>,
    embedding_in_flight: HashSet<u64>,
    embedding_failed_at: HashMap<u64, std::time::Instant>,
}

#[derive(Default)]
struct TextCache {
    vectors: HashMap<u64, Vec<f32>>,
    pending: HashSet<u64>,
}

#[derive(Default)]
struct StartState {
    recent: Vec<TrackIdentity>,
    recommendations: Vec<Track>,
}

pub struct Engine {
    path: PathBuf,
    inner: Mutex<Inner>,
    text_cache: Arc<Mutex<TextCache>>,
    starts: Mutex<StartState>,
    variation: std::sync::atomic::AtomicU32,
}

fn daypart(hour: u32) -> usize {
    match hour { 5..=10 => 0, 11..=16 => 1, 17..=22 => 2, _ => 3 }
}

fn current_daypart() -> usize { daypart(Local::now().hour()) }

impl Engine {
    pub fn new(path: PathBuf) -> Self {
        let mut profile = std::fs::read(&path).ok()
            .and_then(|bytes| serde_json::from_slice::<Profile>(&bytes).ok())
            .or_else(|| std::fs::read(path.with_extension("json.bak")).ok()
                .and_then(|bytes| serde_json::from_slice::<Profile>(&bytes).ok()))
            .unwrap_or_default();
        if profile.recent_starts.len() > 16 {
            profile.recent_starts.drain(..profile.recent_starts.len() - 16);
        }
        profile.recommendations.truncate(80);
        let starts = StartState {
            recent: profile.recent_starts.iter().map(track_identity).collect(),
            recommendations: profile.recommendations.clone(),
        };
        let variation = rand::random::<u32>().wrapping_add(profile.start_counter);
        Self { path, variation: std::sync::atomic::AtomicU32::new(variation),
            text_cache: Arc::new(Mutex::new(TextCache::default())), starts: Mutex::new(starts), inner: Mutex::new(Inner {
            profile, current: None,
            embedding_in_flight: HashSet::new(), embedding_failed_at: HashMap::new(),
        }) }
    }

    pub fn next_variation(&self, requested: u32) -> u32 {
        self.variation.fetch_add(1, std::sync::atomic::Ordering::Relaxed).wrapping_add(requested)
    }

    fn save(&self, profile: &Profile) {
        if let Some(parent) = self.path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                log::warn!("Could not create My Wave profile directory: {error}");
                return;
            }
        }
        if let Err(error) = self.save_inner(profile) {
            log::warn!("Could not save My Wave profile: {error}");
        }
    }

    fn save_inner(&self, profile: &Profile) -> Result<(), String> {
        use std::io::Write;
        let bytes = serde_json::to_vec(profile).map_err(|error| error.to_string())?;
        let temporary = self.path.with_extension("json.tmp");
        let backup = self.path.with_extension("json.bak");
        let mut file = std::fs::File::create(&temporary).map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        if self.path.exists() {
            if backup.exists() { std::fs::remove_file(&backup).map_err(|error| error.to_string())?; }
            std::fs::rename(&self.path, &backup).map_err(|error| error.to_string())?;
        }
        if let Err(error) = std::fs::rename(&temporary, &self.path) {
            if backup.exists() { let _ = std::fs::rename(&backup, &self.path); }
            return Err(error.to_string());
        }
        Ok(())
    }

    pub fn observe(&self, track: Option<&Track>, position_ms: u64, duration_ms: u64, playing: bool, loading: bool) {
        let mut inner = self.inner.lock();
        if inner.current.as_ref().map(|seen| seen.track.id) != track.map(|item| item.id) {
            if let Some(previous) = inner.current.take() {
                if finish(&mut inner.profile, previous) { self.save(&inner.profile); }
            }
            inner.current = track.cloned().map(|track| Observation {
                track, position_ms, duration_ms, heard_ms: 0, daypart: current_daypart(), engagement_recorded: false,
            });
        } else if let Some(seen) = inner.current.as_mut() {
            // Count actual playback progress; a seek cannot fake a listen.
            let delta = position_ms.saturating_sub(seen.position_ms);
            if playing && !loading && delta <= 3_000 { seen.heard_ms = seen.heard_ms.saturating_add(delta); }
            seen.position_ms = position_ms;
            seen.duration_ms = duration_ms.max(seen.duration_ms);
            if !seen.engagement_recorded && seen.heard_ms >= 30_000 {
                let track = seen.track.clone();
                let period = seen.daypart;
                seen.engagement_recorded = true;
                let signal = inner.profile.signals.entry(track.id).or_default();
                signal.track = Some(track);
                signal.engaged = signal.engaged.saturating_add(1);
                signal.engaged_dayparts[period] = signal.engaged_dayparts[period].saturating_add(1);
                signal.last_played = chrono::Utc::now().timestamp();
                self.save(&inner.profile);
            }
        }
    }

    pub fn dislike(&self, track: &Track, disliked: bool) {
        let mut inner = self.inner.lock();
        let signal = inner.profile.signals.entry(track.id).or_default();
        signal.track = Some(track.clone());
        signal.disliked = disliked;
        self.save(&inner.profile);
    }

    pub fn disliked(&self, id: u64) -> bool {
        self.inner.lock().profile.signals.get(&id).is_some_and(|s| s.disliked)
    }

    pub fn begin_audio_analysis(&self, track_id: u64) -> bool {
        let mut inner = self.inner.lock();
        if inner.profile.signals.get(&track_id).is_some_and(|signal| signal.audio_embedding.is_some())
            || inner.embedding_in_flight.contains(&track_id)
            || inner.embedding_failed_at.get(&track_id).is_some_and(|at| at.elapsed() < Duration::from_secs(600)) {
            return false;
        }
        inner.embedding_in_flight.insert(track_id);
        true
    }

    pub fn finish_audio_analysis(&self, track: &Track, embedding: Result<Vec<f32>, String>) {
        let mut inner = self.inner.lock();
        inner.embedding_in_flight.remove(&track.id);
        match embedding {
            Ok(values) if (256..=1024).contains(&values.len()) && values.iter().all(|v| v.is_finite()) => {
                let signal = inner.profile.signals.entry(track.id).or_default();
                signal.track = Some(track.clone());
                signal.audio_embedding = Some(values.into_iter()
                    .map(|value| (value.clamp(-1.0, 1.0) * 127.0).round() as i8).collect());
                self.save(&inner.profile);
            }
            Ok(_) => {
                log::warn!("CLAP returned an invalid embedding for track {}", track.id);
                inner.embedding_failed_at.insert(track.id, std::time::Instant::now());
            }
            Err(error) => {
                log::warn!("CLAP audio analysis failed for track {}: {error}", track.id);
                inner.embedding_failed_at.insert(track.id, std::time::Instant::now());
            }
        }
    }

    fn snapshot(&self) -> Profile { self.inner.lock().profile.clone() }
}

fn finish(profile: &mut Profile, seen: Observation) -> bool {
    let duration = seen.duration_ms.max(seen.track.effective_duration_ms());
    let completed = seen.heard_ms >= 180_000
        || duration > 0 && seen.heard_ms >= duration.saturating_mul(70) / 100;
    let skipped = seen.heard_ms >= 2_000 && seen.heard_ms < 45_000
        && (duration == 0 || seen.position_ms < duration.saturating_mul(40) / 100);
    if !completed && !skipped { return false; }
    let signal = profile.signals.entry(seen.track.id).or_default();
    signal.track = Some(seen.track);
    if completed {
        signal.plays = signal.plays.saturating_add(1);
        signal.dayparts[seen.daypart] = signal.dayparts[seen.daypart].saturating_add(1);
        signal.last_played = chrono::Utc::now().timestamp();
    } else {
        signal.skips = signal.skips.saturating_add(1);
        signal.skip_dayparts[seen.daypart] = signal.skip_dayparts[seen.daypart].saturating_add(1);
        signal.last_played = chrono::Utc::now().timestamp();
    }
    if profile.signals.len() > 4_000 {
        let oldest = profile.signals.iter().filter(|(_, s)| !s.disliked)
            .min_by_key(|(_, s)| s.last_played).map(|(id, _)| *id);
        if let Some(id) = oldest { profile.signals.remove(&id); }
    }
    true
}

#[derive(Default)]
struct FeatureStats {
    total: f64,
    count: u32,
    by_daypart: [f64; 4],
    daypart_count: [u32; 4],
}

impl FeatureStats {
    fn add(&mut self, signal: &Signal) {
        self.total += (f64::from(signal.plays) + 1.0).ln() * 2.2
            + (f64::from(signal.engaged) + 1.0).ln() * 0.6
            - (f64::from(signal.skips) + 1.0).ln() * 2.8
            - if signal.disliked { 5.0 } else { 0.0 };
        self.count += 1;
        for period in 0..4 {
            let local = (f64::from(signal.dayparts[period]) + 1.0).ln() * 3.0
                + (f64::from(signal.engaged_dayparts[period]) + 1.0).ln() * 0.9
                - (f64::from(signal.skip_dayparts[period]) + 1.0).ln() * 3.0;
            if local != 0.0 {
                self.by_daypart[period] += local;
                self.daypart_count[period] += 1;
            }
        }
    }

    fn score(&self, period: usize) -> f64 {
        self.total / (f64::from(self.count) + 2.0) * 2.0
            + self.by_daypart[period] / (f64::from(self.daypart_count[period]) + 1.5) * 2.0
    }
}

#[derive(Default)]
struct TasteModel {
    artists: HashMap<String, FeatureStats>,
    genres: HashMap<String, FeatureStats>,
    audio_global: Vec<f32>,
    audio_by_daypart: [Vec<f32>; 4],
    audio_liked: Vec<(Vec<f32>, f32, [u32; 4])>,
    audio_avoided: Vec<(Vec<f32>, f32)>,
}

fn normalized(value: &str) -> String { value.trim().to_lowercase() }

fn normalize_vector(vector: &mut Vec<f32>) {
    let length = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if length > 0.0001 {
        for value in vector { *value /= length; }
    } else {
        vector.clear();
    }
}

impl TasteModel {
    fn from_profile(profile: &Profile) -> Self {
        let mut model = Self::default();
        for signal in profile.signals.values() {
            let Some(track) = signal.track.as_ref() else { continue };
            let artist = normalized(track.artist());
            if !artist.is_empty() { model.artists.entry(artist).or_default().add(signal); }
            let genre = normalized(track.genre.as_deref().unwrap_or(""));
            if !genre.is_empty() { model.genres.entry(genre).or_default().add(signal); }
            if let Some(embedding) = signal.audio_embedding.as_ref() {
                if model.audio_global.is_empty() {
                    model.audio_global.resize(embedding.len(), 0.0);
                    for local in &mut model.audio_by_daypart { local.resize(embedding.len(), 0.0); }
                }
                if embedding.len() != model.audio_global.len() { continue; }
                let overall = signal.plays.min(8) as f32
                    + signal.engaged.min(8) as f32 * 0.35
                    - signal.skips.min(8) as f32 * 1.2
                    - if signal.disliked { 5.0 } else { 0.0 };
                let mut exemplar: Vec<f32> = embedding.iter().map(|&value| f32::from(value) / 127.0).collect();
                normalize_vector(&mut exemplar);
                if !exemplar.is_empty() {
                    if overall > 0.0 {
                        model.audio_liked.push((exemplar, overall.min(8.0), signal.dayparts));
                    } else if overall < 0.0 {
                        model.audio_avoided.push((exemplar, (-overall).min(8.0)));
                    }
                }
                for (target, &component) in model.audio_global.iter_mut().zip(embedding) {
                    *target += overall * f32::from(component) / 127.0;
                }
                for period in 0..4 {
                    let local = signal.dayparts[period].min(8) as f32 * 1.8
                        + signal.engaged_dayparts[period].min(8) as f32 * 0.35
                        - signal.skip_dayparts[period].min(8) as f32 * 1.5;
                    for (target, &component) in model.audio_by_daypart[period].iter_mut().zip(embedding) {
                        *target += local * f32::from(component) / 127.0;
                    }
                }
            }
        }
        normalize_vector(&mut model.audio_global);
        for local in &mut model.audio_by_daypart { normalize_vector(local); }
        model.audio_liked.sort_by(|a, b| b.1.total_cmp(&a.1));
        model.audio_liked.truncate(96);
        model.audio_avoided.sort_by(|a, b| b.1.total_cmp(&a.1));
        model.audio_avoided.truncate(48);
        model
    }

    fn score(&self, track: &Track, period: usize) -> f64 {
        let artist = self.artists.get(&normalized(track.artist())).map_or(0.0, |s| s.score(period));
        let genre = self.genres.get(&normalized(track.genre.as_deref().unwrap_or("")))
            .map_or(0.0, |s| s.score(period));
        (artist + genre * 0.7).clamp(-12.0, 12.0)
    }

    fn has_audio_taste(&self) -> bool { !self.audio_global.is_empty() }

    fn acoustic_score(&self, vector: &[f32], period: usize) -> f64 {
        if vector.len() != self.audio_global.len() { return 0.0; }
        let dot = |centroid: &[f32]| -> f64 { centroid.iter().zip(vector).map(|(a, b)| f64::from(a * b)).sum() };
        let liked = self.audio_liked.iter().map(|(sample, weight, dayparts)| {
            let local = 1.0 + (dayparts[period].min(5) as f64) * 0.12;
            dot(sample) * local * (0.8 + f64::from(*weight) * 0.025)
        }).fold(0.0_f64, f64::max);
        let avoided = self.audio_avoided.iter().map(|(sample, weight)|
            dot(sample) * (0.8 + f64::from(*weight) * 0.025))
            .fold(0.0_f64, f64::max);
        dot(&self.audio_global) * 2.0 + dot(&self.audio_by_daypart[period]) * 1.8
            + liked * 3.0 - avoided * 3.5
    }
}

fn seed_score(track: &Track, profile: &Profile, model: &TasteModel, period: usize) -> f64 {
    let own = profile.signals.get(&track.id).map_or(0.0, |s|
        s.plays.min(8) as f64 * 1.4 + s.dayparts[period].min(6) as f64 * 1.8
        - s.skips.min(5) as f64 * 2.5);
    let last = profile.signals.get(&track.id).map_or(0, |signal| signal.last_played);
    own + model.score(track, period) - recency_penalty(last, chrono::Utc::now().timestamp())
}

fn recency_penalty(last_played: i64, now: i64) -> f64 {
    if last_played <= 0 { return 0.0; }
    let elapsed = now.saturating_sub(last_played);
    if elapsed < 6 * 3600 { 12.0 } else if elapsed < 2 * 86_400 { 5.0 } else { 0.0 }
}

fn choose_seeds(likes: &[Track], profile: &Profile, model: &TasteModel, variation: u32, period: usize, avoid: &HashSet<u64>, avoid_identities: &[TrackIdentity]) -> Vec<Track> {
    let mut ranked: Vec<_> = likes.iter()
        .filter(|t| !t.is_blocked() && !t.is_snippet() && !avoid.contains(&t.id)
            && !avoid_identities.iter().any(|old| same_recording(old, &enriched_identity(t, profile)))
            && !profile.signals.get(&t.id).is_some_and(|s| s.disliked))
        .map(|t| (t, seed_score(t, profile, model, period)))
        .collect();
    if ranked.is_empty() {
        // Reuse a familiar seed to explore its related graph after every liked
        // track has appeared. The seed itself is filtered from the next queue.
        ranked = likes.iter().filter(|track| !track.is_blocked() && !track.is_snippet()
            && !profile.signals.get(&track.id).is_some_and(|s| s.disliked))
            .map(|track| (track, seed_score(track, profile, model, period))).collect();
    }
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    if ranked.is_empty() { return Vec::new(); }
    // Move by four seeds each refill. A tiny score jitter kept returning the
    // same related page, so the station could not grow after the first batch.
    let start = (variation as usize).wrapping_mul(4) % ranked.len().min(80);
    let mut seeds = Vec::new();
    let mut artists = HashSet::new();
    for offset in 0..ranked.len() {
        let (track, _) = ranked[(start + offset) % ranked.len()];
        if artists.insert(track.artist().to_lowercase())
            && !seeds.iter().any(|seed| same_recording(&track_identity(seed), &track_identity(track))) {
            seeds.push(track.clone());
        }
        if seeds.len() == 4 { return seeds; }
    }
    for offset in 0..ranked.len() {
        let (track, _) = ranked[(start + offset) % ranked.len()];
        if !seeds.iter().any(|seed| same_recording(&track_identity(seed), &track_identity(track))) {
            seeds.push(track.clone());
        }
        if seeds.len() == 4 { break; }
    }
    seeds
}

fn engaged_discoveries(profile: &Profile, likes: &HashSet<u64>) -> Vec<Track> {
    let now = chrono::Utc::now().timestamp();
    let mut candidates: Vec<_> = profile.signals.iter()
        .filter(|(id, signal)| !likes.contains(id) && !signal.disliked
            && (signal.plays > signal.skips
                || signal.engaged >= 2 && signal.engaged > signal.skips.saturating_mul(2)))
        .filter_map(|(_, signal)| signal.track.as_ref()
            .filter(|track| !track.is_blocked() && !track.is_snippet())
            .map(|track| (track.clone(), signal)))
        .collect();
    candidates.sort_by(|(_, a), (_, b)| {
        let score = |signal: &Signal| {
            signal.plays.min(8) as i64 * 3 + signal.engaged.min(8) as i64
                - signal.skips.min(8) as i64 * 4
                + if now - signal.last_played < 7 * 86_400 { 3 } else { 0 }
        };
        score(b).cmp(&score(a))
    });
    candidates.into_iter().take(80).map(|(track, _)| track).collect()
}

fn wave_seeds(likes: &[Track], profile: &Profile, model: &TasteModel, variation: u32,
    period: usize, avoid: &HashSet<u64>, avoid_identities: &[TrackIdentity]) -> Vec<Track> {
    let liked = choose_seeds(likes, profile, model, variation, period, avoid, avoid_identities);
    let liked_ids = likes.iter().map(|track| track.id).collect();
    let discoveries = engaged_discoveries(profile, &liked_ids);
    let discovered = choose_seeds(&discoveries, profile, model, variation.wrapping_add(1),
        period, avoid, avoid_identities);
    let mut seeds = Vec::<Track>::new();
    let mut append = |track: &Track, require_new_artist: bool| {
        let identity = track_identity(track);
        if seeds.iter().any(|old| old.id == track.id || same_recording(&track_identity(old), &identity)) { return; }
        if require_new_artist && seeds.iter().any(|old| track_identity(old).performer_keys.iter()
            .any(|key| identity.performer_keys.contains(key))) { return; }
        seeds.push(track.clone());
    };
    for track in liked.iter().take(2) { append(track, true); }
    for track in discovered.iter().take(2) { append(track, true); }
    for track in discovered.iter().chain(liked.iter()) {
        append(track, false);
    }
    seeds.truncate(4);
    seeds
}

async fn related(client: Arc<api::ApiClient>, seed_id: u64) -> Vec<Track> {
    let mut pager = api::endpoints::related_tracks(&client, seed_id).await;
    match tokio::time::timeout(Duration::from_secs(9), pager.next_page()).await {
        Ok(Ok(tracks)) => tracks,
        result => { log::warn!("My Wave seed {seed_id} failed: {result:?}"); Vec::new() }
    }
}

pub fn first_seed(engine: &Engine, liked_tracks: &[Track], variation: u32) -> Option<Track> {
    let mut profile = engine.snapshot();
    let likes: Vec<_> = liked_tracks.iter().take(MAX_LIKES).cloned().collect();
    if likes.is_empty() { return None; }
    for track in &likes {
        let signal = profile.signals.entry(track.id).or_default();
        signal.track = Some(track.clone());
        signal.plays = signal.plays.max(1);
    }
    let model = TasteModel::from_profile(&profile);
    let period = current_daypart();
    let liked_ids = likes.iter().map(|track| track.id).collect();
    let discoveries = engaged_discoveries(&profile, &liked_ids);
    let mut starts = engine.starts.lock();
    let mut candidates = Vec::<(Track, f64)>::new();
    let mut ids = HashSet::new();
    for (source, bonus) in [(&starts.recommendations, 24.0), (&discoveries, 12.0), (&likes, 0.0)] {
        for track in source {
            if !track.is_blocked() && !track.is_snippet()
                && !profile.signals.get(&track.id).is_some_and(|signal| signal.disliked)
                && ids.insert(track.id) {
                candidates.push((track.clone(), bonus));
            }
        }
    }
    // Prefer an opening recording that has not opened any recent station,
    // including after restarting. A tiny library still falls back to its songs.
    if candidates.iter().any(|(track, _)| !starts.recent.iter().any(|previous|
        previous.id == track.id || same_recording(previous, &track_identity(track)))) {
        candidates.retain(|(track, _)| !starts.recent.iter().any(|previous|
            previous.id == track.id || same_recording(previous, &track_identity(track))));
    }
    let selected = candidates.into_iter().max_by(|(left, left_bonus), (right, right_bonus)| {
        let score = |track: &Track, bonus: f64| {
            let identity = track_identity(track);
            let artist_uses = starts.recent.iter().filter(|previous| identity.performer_keys.iter()
                .any(|key| previous.performer_keys.contains(key))).count();
            let track_uses = starts.recent.iter().filter(|previous| previous.id == track.id).count();
            let variation = (track.id ^ u64::from(variation).wrapping_mul(0x9E37_79B9)) % 997;
            bonus + seed_score(track, &profile, &model, period)
                - artist_uses as f64 * 100.0 - track_uses as f64 * 50.0
                + variation as f64 / 997.0
        };
        score(left, *left_bonus).total_cmp(&score(right, *right_bonus))
    }).map(|(track, _)| track)?;
    starts.recent.push(track_identity(&selected));
    if starts.recent.len() > 16 { starts.recent.remove(0); }
    let mut inner = engine.inner.lock();
    inner.profile.recent_starts.push(selected.clone());
    if inner.profile.recent_starts.len() > 16 { inner.profile.recent_starts.remove(0); }
    inner.profile.start_counter = inner.profile.start_counter.wrapping_add(1);
    engine.save(&inner.profile);
    Some(selected)
}

fn remember_recommendations(engine: &Engine, tracks: &[Track], liked: &HashSet<u64>) {
    let recommendations: Vec<_> = tracks.iter().filter(|track| !liked.contains(&track.id))
        .take(80).cloned().collect();
    if !recommendations.is_empty() {
        engine.starts.lock().recommendations = recommendations.clone();
        let mut inner = engine.inner.lock();
        inner.profile.recommendations = recommendations;
        engine.save(&inner.profile);
    }
}

fn assemble(seeds: &[Track], related: Vec<Vec<Track>>, likes: &HashSet<u64>, known: &[TrackIdentity], avoid: &HashSet<u64>, profile: &Profile, model: &TasteModel, text_vectors: &HashMap<u64, Vec<f32>>, period: usize, session_artists: &[TrackIdentity]) -> Vec<Track> {
    let mut seen = likes.clone();
    seen.extend(avoid.iter().copied());
    if seeds.is_empty() { return Vec::new(); }
    let mut result = Vec::new();
    let mut pool = Vec::<(Track, f64)>::new();
    let mut artist_exposure = HashMap::<String, usize>::new();
    for previous in session_artists {
        for key in &previous.performer_keys {
            *artist_exposure.entry(key.clone()).or_default() += 1;
        }
    }
    for (seed, candidates) in seeds.iter().zip(related) {
        let source_score = seed_score(seed, profile, model, period);
        for (index, candidate) in candidates.into_iter().enumerate() {
            let identity = enriched_identity(&candidate, profile);
            if (candidate.is_blocked() || candidate.is_snippet()) || !seen.insert(candidate.id)
                || known.iter().any(|previous| same_recording(previous, &identity))
                || profile.signals.get(&candidate.id).is_some_and(|s| s.disliked) { continue; }
            let score = 8.0 + source_score * 0.35
                + model.score(&candidate, period) - index as f64 * 0.04
                + text_vectors.get(&candidate.id).map_or(0.0, |vector| model.acoustic_score(vector, period))
                + profile.signals.get(&candidate.id)
                    .and_then(|signal| signal.audio_embedding.as_ref())
                    .map_or(0.0, |vector| {
                        let values: Vec<f32> = vector.iter().map(|&value| f32::from(value) / 127.0).collect();
                        model.acoustic_score(&values, period)
                    })
                - profile.signals.get(&candidate.id).map_or(0.0, |s| {
                    s.skips as f64 * 3.0 + recency_penalty(s.last_played, chrono::Utc::now().timestamp())
                })
                - profile.recent_starts.iter().rev().take(8)
                    .filter(|previous| same_recording(&track_identity(previous), &identity)).count() as f64 * 12.0;
            pool.push((candidate, score));
        }
    }
    while result.len() < MAX_WAVE_TRACKS && !pool.is_empty() {
        let recent: Vec<_> = result.iter().rev().take(5).map(track_identity).collect();
        let adjusted = |t: &Track, score: f64| {
            let identity = track_identity(t);
            let exposure = identity.performer_keys.iter()
                .filter_map(|key| artist_exposure.get(key).copied()).max().unwrap_or(0);
            let recently_played = recent.iter().any(|previous| identity.performer_keys.iter()
                .any(|key| previous.performer_keys.contains(key)));
            let recent_penalty = if recently_played { 12.0 } else { 0.0 };
            // Historical exposure stops one related page dominating for hours.
            // A penalty, rather than a hard quota, still permits a niche library
            // where there genuinely are no other artists to play.
            score - recent_penalty - (exposure as f64 * 1.4).min(24.0)
        };
        let best = pool.iter().enumerate().max_by(|(_, (a, sa)), (_, (b, sb))|
            adjusted(a, *sa).total_cmp(&adjusted(b, *sb))).map(|(index, _)| index).unwrap();
        let chosen = pool.swap_remove(best).0;
        let identity = enriched_identity(&chosen, profile);
        for key in &identity.performer_keys {
            *artist_exposure.entry(key.clone()).or_default() += 1;
        }
        pool.retain(|(candidate, _)| !same_recording(&identity, &enriched_identity(candidate, profile)));
        result.push(chosen);
    }
    result
}

pub async fn generate(client: Arc<api::ApiClient>, engine: &Engine, liked_tracks: Vec<Track>, variation: u32, avoid: &HashSet<u64>, avoid_identities: &[TrackIdentity]) -> Result<Vec<Track>, String> {
    let likes: Vec<_> = liked_tracks.into_iter().take(MAX_LIKES).collect();
    if likes.is_empty() { return Err("Like a few tracks to start My Wave".into()); }
    let mut profile = engine.snapshot();
    // A SoundCloud like is positive feedback even before the track was played here.
    // This prior exists only for ranking; it does not fabricate listening history on disk.
    for track in &likes {
        let signal = profile.signals.entry(track.id).or_default();
        signal.track = Some(track.clone());
        signal.plays = signal.plays.max(1);
    }
    let period = current_daypart();
    let model = TasteModel::from_profile(&profile);
    let seeds = wave_seeds(&likes, &profile, &model, variation, period, avoid, avoid_identities);
    if seeds.is_empty() { return Err("No new playable tracks are available for My Wave".into()); }
    let (a, b, c, d) = tokio::join!(
        related(client.clone(), seeds[0].id),
        async { if let Some(t) = seeds.get(1) { related(client.clone(), t.id).await } else { Vec::new() } },
        async { if let Some(t) = seeds.get(2) { related(client.clone(), t.id).await } else { Vec::new() } },
        async { if let Some(t) = seeds.get(3) { related(client.clone(), t.id).await } else { Vec::new() } },
    );
    let related = vec![a, b, c, d];
    let mut text_vectors = HashMap::new();
    if model.has_audio_taste() && crate::clap::available() {
        let mut candidate_ids = Vec::new();
        let mut prompts = Vec::new();
        let mut unique = HashSet::new();
        {
            let mut cache = engine.text_cache.lock();
            for track in related.iter().flatten() {
                if !unique.insert(track.id) { continue; }
                if let Some(vector) = cache.vectors.get(&track.id) {
                    text_vectors.insert(track.id, vector.clone());
                } else if prompts.len() < 24 && cache.pending.insert(track.id) {
                    candidate_ids.push(track.id);
                    prompts.push(format!("A music track titled {} by {}. Genre: {}.",
                        track.title, track.artist(), track.genre.as_deref().unwrap_or("unknown")));
                }
            }
        }
        if !prompts.is_empty() {
            let cache = engine.text_cache.clone();
            let inference = tokio::task::spawn_blocking(move || {
                let result = crate::clap::embed_texts(&prompts);
                let mut cache = cache.lock();
                for id in &candidate_ids { cache.pending.remove(id); }
                match result {
                    Ok(vectors) if vectors.len() == candidate_ids.len() => {
                        for (id, vector) in candidate_ids.into_iter().zip(vectors) {
                            cache.vectors.insert(id, vector);
                        }
                        while cache.vectors.len() > 1024 {
                            if let Some(old) = cache.vectors.keys().next().copied() { cache.vectors.remove(&old); }
                        }
                    }
                    Ok(_) => log::warn!("CLAP text inference returned an unexpected vector count"),
                    Err(error) => log::warn!("CLAP text inference failed: {error}"),
                }
            });
            let _ = tokio::time::timeout(Duration::from_millis(700), inference).await;
            let cache = engine.text_cache.lock();
            for track in related.iter().flatten() {
                if let Some(vector) = cache.vectors.get(&track.id) {
                    text_vectors.insert(track.id, vector.clone());
                }
            }
        }
    }
    let ids = likes.iter().map(|t| t.id).collect();
    let mut known: Vec<_> = likes.iter().map(|track| enriched_identity(track, &profile)).collect();
    known.extend(avoid_identities.iter().cloned().map(|mut identity| {
        identity.audio_embedding = profile.signals.get(&identity.id)
            .and_then(|signal| signal.audio_embedding.clone());
        identity
    }));
    let mut tracks = assemble(&seeds, related, &ids, &known, avoid, &profile, &model, &text_vectors, period, avoid_identities);
    if tracks.len() < 4 {
        let mut seen: HashSet<_> = tracks.iter().map(|track| track.id).collect();
        let liked_ids: HashSet<_> = likes.iter().map(|track| track.id).collect();
        for track in engaged_discoveries(&profile, &liked_ids).iter().chain(likes.iter()) {
            if tracks.len() >= 4 { break; }
            if !avoid.contains(&track.id) && !track.is_blocked() && !track.is_snippet()
                && !tracks.iter().any(|existing| same_recording(
                    &enriched_identity(existing, &profile), &enriched_identity(track, &profile)))
                && !profile.signals.get(&track.id).is_some_and(|s| s.disliked) && seen.insert(track.id) {
                tracks.push(track.clone());
            }
        }
    }
    remember_recommendations(engine, &tracks, &ids);
    Ok(tracks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dayparts_cover_local_day() {
        assert_eq!([daypart(0), daypart(6), daypart(12), daypart(19), daypart(23)], [3, 0, 1, 2, 3]);
    }

    #[test]
    fn skip_listen_and_seek_are_distinct() {
        let track = crate::demo::demo_tracks().remove(0);
        let mut profile = Profile::default();
        let seen = |heard_ms, position_ms| Observation { track: track.clone(), position_ms, duration_ms: 120_000, heard_ms, daypart: 1, engagement_recorded: false };
        assert!(finish(&mut profile, seen(8_000, 8_000)));
        assert_eq!(profile.signals[&track.id].skips, 1);
        assert!(finish(&mut profile, seen(90_000, 90_000)));
        assert_eq!(profile.signals[&track.id].plays, 1);
        assert_eq!(profile.signals[&track.id].dayparts[1], 1);
        assert!(!finish(&mut profile, seen(0, 100_000)));
        assert!(finish(&mut profile, seen(31_000, 31_000)));
        assert_eq!(profile.signals[&track.id].plays, 1);
        assert_eq!(profile.signals[&track.id].skips, 2);
    }

    #[test]
    fn dislikes_are_excluded() {
        let demo = crate::demo::demo_tracks();
        let mut profile = Profile::default();
        profile.signals.insert(demo[1].id, Signal { disliked: true, ..Signal::default() });
        let likes = HashSet::from([demo[0].id]);
        let model = TasteModel::from_profile(&profile);
        let tracks = assemble(&demo[..1], vec![vec![demo[1].clone(), demo[2].clone()]], &likes, &[], &HashSet::new(), &profile, &model, &HashMap::new(), 1, &[]);
        assert_eq!(tracks.iter().map(|t| t.id).collect::<Vec<_>>(), vec![demo[2].id]);
    }

    #[test]
    fn time_of_day_changes_preferred_seed() {
        let demo = crate::demo::demo_tracks();
        let mut profile = Profile::default();
        profile.signals.insert(demo[0].id, Signal {
            track: Some(demo[0].clone()), dayparts: [6, 0, 0, 0], ..Signal::default()
        });
        profile.signals.insert(demo[1].id, Signal {
            track: Some(demo[1].clone()), dayparts: [0, 0, 0, 6], ..Signal::default()
        });
        let model = TasteModel::from_profile(&profile);
        assert_eq!(choose_seeds(&demo[..3], &profile, &model, 0, 0, &HashSet::new(), &[])[0].id, demo[0].id);
        assert_eq!(choose_seeds(&demo[..3], &profile, &model, 0, 3, &HashSet::new(), &[])[0].id, demo[1].id);
    }

    #[test]
    fn refill_avoids_queued_tracks_and_rotates_seeds() {
        let demo = crate::demo::demo_tracks();
        let profile = Profile::default();
        let model = TasteModel::default();
        let first = choose_seeds(&demo, &profile, &model, 0, 1, &HashSet::new(), &[]);
        let next = choose_seeds(&demo, &profile, &model, 1, 1, &HashSet::new(), &[]);
        assert_ne!(first[0].id, next[0].id);
        let avoid = HashSet::from([demo[1].id]);
        let likes = HashSet::from([first[0].id]);
        let tracks = assemble(&first[..1], vec![vec![demo[1].clone(), demo[2].clone()]], &likes, &[], &avoid, &profile, &model, &HashMap::new(), 1, &[]);
        assert!(!tracks.iter().any(|track| track.id == demo[1].id));
    }

    #[test]
    fn a_large_library_does_not_drown_out_time_preferences() {
        let demo = crate::demo::demo_tracks();
        let mut profile = Profile::default();
        for id in 1..=500 {
            let mut liked = demo[0].clone();
            liked.id = id;
            profile.signals.insert(id, Signal { track: Some(liked), plays: 1, ..Signal::default() });
        }
        let mut favorite = demo[1].clone();
        favorite.user = demo[1].user.clone();
        profile.signals.insert(10_000, Signal {
            track: Some(favorite.clone()), plays: 3, dayparts: [0, 0, 0, 5], ..Signal::default()
        });
        let model = TasteModel::from_profile(&profile);
        assert!(model.score(&favorite, 3) > model.score(&favorite, 0));
    }

    #[test]
    fn local_audio_embedding_favors_positive_sound_and_avoids_negative_sound() {
        let demo = crate::demo::demo_tracks();
        let mut profile = Profile::default();
        profile.signals.insert(demo[0].id, Signal {
            track: Some(demo[0].clone()), plays: 4, dayparts: [4, 0, 0, 0],
            audio_embedding: Some(vec![127, 0, 0, 0, 0].into_iter().cycle().take(256).collect()),
            ..Signal::default()
        });
        let model = TasteModel::from_profile(&profile);
        let mut similar = vec![0.0; 256];
        similar[0] = 1.0;
        let mut different = vec![0.0; 256];
        different[1] = 1.0;
        assert!(model.acoustic_score(&similar, 0) > model.acoustic_score(&different, 0));
    }

    #[test]
    fn duplicate_uploads_are_filtered_but_remixes_survive() {
        let demo = crate::demo::demo_tracks();
        let mut seed = demo[0].clone();
        seed.title = "Different song".into();
        let mut original = demo[1].clone();
        original.title = "Lil Peep - Star Shopping [Official Audio]".into();
        original.duration_ms = Some(181_000);
        original.full_duration_ms = None;
        let mut reupload = original.clone();
        reupload.id += 10_000;
        reupload.title = "Star Shopping".into();
        reupload.metadata_artist = Some("Lil Peep".into());
        reupload.duration_ms = Some(184_000);
        let mut remix = original.clone();
        remix.id += 20_000;
        remix.title = "Lil Peep - Star Shopping (slowed)".into();
        let likes = HashSet::from([seed.id]);
        let profile = Profile::default();
        let model = TasteModel::from_profile(&profile);
        let result = assemble(&[seed], vec![vec![original, reupload, remix.clone()]],
            &likes, &[], &HashSet::new(), &profile, &model, &HashMap::new(), 1, &[]);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|track| track.id == remix.id));
    }

    #[test]
    fn title_collision_with_different_artist_is_not_a_duplicate() {
        let mut first = crate::demo::demo_tracks().remove(0);
        first.title = "Blue Moon".into();
        first.metadata_artist = Some("Artist A".into());
        first.duration_ms = Some(180_000);
        first.full_duration_ms = None;
        let mut second = first.clone();
        second.metadata_artist = Some("Artist B".into());
        assert!(!same_recording(&track_identity(&first), &track_identity(&second)));
    }

    #[test]
    fn matching_audio_can_resolve_a_minor_title_difference() {
        let mut first = crate::demo::demo_tracks().remove(0);
        first.title = "Artist - Midnight Song".into();
        first.duration_ms = Some(180_000);
        first.full_duration_ms = None;
        let mut second = first.clone();
        second.id += 1000;
        second.title = "Artist - Midnight Song remastered".into();
        let mut profile = Profile::default();
        profile.signals.insert(first.id, Signal {
            audio_embedding: Some(vec![50; 512]), ..Signal::default()
        });
        profile.signals.insert(second.id, Signal {
            audio_embedding: Some(vec![50; 512]), ..Signal::default()
        });
        assert!(same_recording(&enriched_identity(&first, &profile), &enriched_identity(&second, &profile)));
        profile.signals.get_mut(&second.id).unwrap().audio_embedding = Some(
            (0..512).map(|index| if index % 2 == 0 { 50 } else { -50 }).collect());
        assert!(!same_recording(&enriched_identity(&first, &profile), &enriched_identity(&second, &profile)));
    }

    #[test]
    fn seed_search_continues_after_every_like_has_appeared() {
        let likes = crate::demo::demo_tracks();
        let seen = likes.iter().map(|track| track.id).collect();
        let model = TasteModel::default();
        let seeds = choose_seeds(&likes, &Profile::default(), &model, 3, 1, &seen, &[]);
        assert!(!seeds.is_empty());
    }

    #[test]
    fn long_session_prefers_another_artist_when_music_is_available() {
        let mut seed = crate::demo::demo_tracks().remove(0);
        seed.title = "Seed".into();
        seed.metadata_artist = Some("Seed artist".into());
        let mut dominant = seed.clone();
        dominant.id += 1000;
        dominant.title = "Dominant track".into();
        dominant.metadata_artist = Some("Artist A".into());
        let mut alternate = dominant.clone();
        alternate.id += 1;
        alternate.title = "Alternate track".into();
        alternate.metadata_artist = Some("Artist B".into());
        let history: Vec<_> = (0..12).map(|index| {
            let mut track = dominant.clone();
            track.id += 100 + index;
            track.title = format!("Older track {index}");
            track_identity(&track)
        }).collect();
        let model = TasteModel::default();
        let profile = Profile::default();
        let liked = HashSet::from([seed.id]);
        let result = assemble(&[seed.clone()], vec![vec![dominant.clone(), alternate.clone()]],
            &liked, &[], &HashSet::new(), &profile, &model, &HashMap::new(), 1, &history);
        assert_eq!(result[0].id, alternate.id);
        let only_dominant = assemble(&[seed], vec![vec![dominant.clone()]],
            &liked, &[], &HashSet::new(), &profile, &model, &HashMap::new(), 1, &history);
        assert_eq!(only_dominant[0].id, dominant.id);
    }

    #[test]
    fn alternate_artist_spellings_share_exposure() {
        let mut first = crate::demo::demo_tracks().remove(0);
        first.metadata_artist = Some("JESUS | ДЖИЗУС".into());
        let mut second = first.clone();
        second.metadata_artist = Some("Джизус".into());
        let one = track_identity(&first);
        let two = track_identity(&second);
        assert!(one.performer_keys.iter().any(|key| two.performer_keys.contains(key)));
    }

    #[test]
    fn engaged_discoveries_expand_the_related_search() {
        let tracks = crate::demo::demo_tracks();
        let liked = vec![tracks[0].clone(), tracks[1].clone()];
        let mut profile = Profile::default();
        profile.signals.insert(tracks[2].id, Signal {
            track: Some(tracks[2].clone()), plays: 2, engaged: 2, ..Signal::default()
        });
        profile.signals.insert(tracks[3].id, Signal {
            track: Some(tracks[3].clone()), plays: 1, skips: 3, ..Signal::default()
        });
        profile.signals.insert(tracks[4].id, Signal {
            track: Some(tracks[4].clone()), plays: 3, disliked: true, ..Signal::default()
        });
        let model = TasteModel::from_profile(&profile);
        let seeds = wave_seeds(&liked, &profile, &model, 0, 1, &HashSet::new(), &[]);
        assert!(seeds.iter().any(|track| track.id == tracks[2].id));
        assert!(!seeds.iter().any(|track| track.id == tracks[3].id || track.id == tracks[4].id));
    }

    #[test]
    fn first_seed_starts_without_waiting_for_related_api() {
        let directory = tempfile::tempdir().unwrap();
        let engine = Engine::new(directory.path().join("wave.json"));
        let likes = crate::demo::demo_tracks();
        let seed = first_seed(&engine, &likes, 0).unwrap();
        assert!(likes.iter().any(|track| track.id == seed.id));
        assert!(first_seed(&engine, &[], 0).is_none());
    }

    #[test]
    fn new_wave_rotates_starting_artists_instead_of_rotating_tracks_only() {
        let directory = tempfile::tempdir().unwrap();
        let engine = Engine::new(directory.path().join("wave.json"));
        let template = crate::demo::demo_tracks().remove(0);
        let likes: Vec<_> = (0..24).map(|index| {
            let mut track = template.clone();
            track.id += index;
            track.title = format!("Track {index}");
            track.metadata_artist = Some(if index < 20 {
                "Dominant artist".into()
            } else {
                format!("Artist {index}")
            });
            track
        }).collect();
        let started: Vec<_> = (0..5).map(|variation| {
            track_identity(&first_seed(&engine, &likes, variation).unwrap()).performer_keys
        }).collect();
        assert_eq!(started.iter().collect::<HashSet<_>>().len(), 5);
    }

    #[test]
    fn cached_recommendation_can_start_a_new_wave() {
        let directory = tempfile::tempdir().unwrap();
        let engine = Engine::new(directory.path().join("wave.json"));
        let tracks = crate::demo::demo_tracks();
        engine.starts.lock().recommendations = vec![tracks[2].clone()];
        let started = first_seed(&engine, &tracks[..2], 0).unwrap();
        assert_eq!(started.id, tracks[2].id);
    }

    #[test]
    fn first_track_does_not_repeat_after_restarting_the_application() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wave.json");
        let likes = crate::demo::demo_tracks();
        let started: Vec<_> = (0..likes.len()).map(|_| {
            let engine = Engine::new(path.clone());
            first_seed(&engine, &likes, 0).unwrap().id
        }).collect();
        assert_eq!(started.iter().collect::<HashSet<_>>().len(), likes.len());
    }

    #[test]
    fn cached_discoveries_survive_a_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wave.json");
        let tracks = crate::demo::demo_tracks();
        let engine = Engine::new(path.clone());
        remember_recommendations(&engine, &tracks[2..3], &HashSet::new());
        let restarted = Engine::new(path);
        assert_eq!(first_seed(&restarted, &tracks[..2], 0).unwrap().id, tracks[2].id);
    }

    #[test]
    fn a_tiny_library_can_still_start_when_every_song_has_been_used() {
        let directory = tempfile::tempdir().unwrap();
        let likes = crate::demo::demo_tracks();
        let engine = Engine::new(directory.path().join("wave.json"));
        assert!(first_seed(&engine, &likes[..1], 0).is_some());
        assert!(first_seed(&engine, &likes[..1], 0).is_some());
    }

    #[test]
    fn suffix_artist_is_the_same_recording_and_recent_penalty_fades() {
        let mut original = crate::demo::demo_tracks().remove(0);
        original.metadata_artist = Some("CUPSIZE".into());
        original.title = "оригами".into();
        let mut upload = original.clone();
        upload.id += 1;
        upload.title = "оригами - CUPSIZE".into();
        assert!(same_recording(&track_identity(&original), &track_identity(&upload)));
        assert_eq!(recency_penalty(0, 1_000_000), 0.0);
        assert_eq!(recency_penalty(999_000, 1_000_000), 12.0);
        assert_eq!(recency_penalty(900_000, 1_000_000), 5.0);
        assert_eq!(recency_penalty(1, 1_000_000), 0.0);
    }
}
