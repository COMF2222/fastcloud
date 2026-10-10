//! Spotify metadata imports. Audio stays on SoundCloud; exports never leave
//! the computer. ZIP entries are read in memory, never extracted to disk.
use super::{AppState, Key, api, auth};
use api::models::Track;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

const FILE_LIMIT: u64 = 16 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 128 * 1024 * 1024;
const TRACK_LIMIT: usize = 50_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Playlist,
    Likes,
}

#[derive(Clone, Debug)]
struct Song {
    title: String,
    artists: Vec<String>,
    duration: Option<u64>,
    isrc: Option<String>,
}

impl Song {
    fn label(&self) -> String {
        format!("{} — {}", self.artists.join(", "), self.title)
    }
    fn key(&self) -> String {
        format!(
            "{}|{}|{:?}|{:?}",
            normalized(&self.title),
            self.artists
                .iter()
                .map(|a| normalized(a))
                .collect::<Vec<_>>()
                .join(";"),
            self.duration,
            self.isrc
        )
    }
}

#[derive(Debug)]
struct Collection {
    name: String,
    kind: Kind,
    songs: Vec<Song>,
    skipped: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    id: usize,
    name: String,
    kind: Kind,
    tracks: usize,
    skipped: usize,
}

#[derive(Deserialize)]
pub struct Selection {
    id: usize,
    kind: Kind,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    name: String,
    matched: usize,
    added: usize,
    already_liked: usize,
    not_found: usize,
    missing: Vec<String>,
    playlist_ids: Vec<u64>,
    completed: bool,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub running: bool,
    ready: bool,
    current: usize,
    total: usize,
    matched: usize,
    title: String,
    error: String,
    cancelled: bool,
    collections: Vec<Summary>,
    reports: Vec<Report>,
}

#[derive(Default)]
pub struct Import {
    view: Mutex<View>,
    plan: Mutex<Option<Vec<Collection>>>,
    cancel: AtomicBool,
    account_changes: AtomicUsize,
    finished: tokio::sync::Notify,
}

pub struct AccountChange<'a>(&'a Import);
impl Drop for AccountChange<'_> {
    fn drop(&mut self) {
        self.0.account_changes.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Import {
    pub async fn cancel_and_wait(&self) -> AccountChange<'_> {
        {
            // Serialize the gate and cancellation with start_import, so a new
            // import cannot clear cancellation while sign-in is waiting.
            let _view = self.view.lock();
            self.account_changes.fetch_add(1, Ordering::AcqRel);
            self.cancel.store(true, Ordering::Release);
        }
        let guard = AccountChange(self);
        loop {
            let wait = self.finished.notified();
            tokio::pin!(wait);
            wait.as_mut().enable();
            if !self.view.lock().running {
                return guard;
            }
            wait.await;
        }
    }
}

fn normalized(raw: &str) -> String {
    raw.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn recording_title(raw: &str) -> String {
    let value = normalized(raw);
    let value = [" feat ", " ft ", " featuring "]
        .iter()
        .filter_map(|marker| value.find(marker))
        .min()
        .map_or(value.as_str(), |end| &value[..end]);
    for suffix in [
        " official music video",
        " official audio",
        " official video",
        " lyric video",
        " lyrics",
    ] {
        if let Some(title) = value.strip_suffix(suffix) {
            return title.to_owned();
        }
    }
    value.to_owned()
}

fn guessed_kind(name: &str) -> Kind {
    if matches!(
        normalized(name).as_str(),
        "liked songs"
            | "liked tracks"
            | "liked songs spotify"
            | "saved tracks"
            | "yourlibrary"
            | "your library"
            | "любимые треки"
            | "любимые песни"
    ) {
        Kind::Likes
    } else {
        Kind::Playlist
    }
}

fn text(data: &[u8]) -> Result<String, String> {
    if data.starts_with(&[0xff, 0xfe]) || data.starts_with(&[0xfe, 0xff]) {
        if data.len() % 2 != 0 {
            return Err("Invalid UTF-16 export".into());
        }
        let little = data[0] == 0xff;
        let words: Vec<u16> = data[2..]
            .chunks_exact(2)
            .map(|v| {
                if little {
                    u16::from_le_bytes([v[0], v[1]])
                } else {
                    u16::from_be_bytes([v[0], v[1]])
                }
            })
            .collect();
        return String::from_utf16(&words).map_err(|_| "Invalid UTF-16 export".into());
    }
    std::str::from_utf8(data.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(data))
        .map(str::to_owned)
        .map_err(|_| "Export must use UTF-8 or UTF-16".into())
}

fn csv_collection(name: &str, raw: &str) -> Result<Collection, String> {
    let header = raw.lines().next().unwrap_or_default();
    let delimiter = if header.matches(';').count() > header.matches(',').count() {
        b';'
    } else {
        b','
    };
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .trim(csv::Trim::All)
        .from_reader(raw.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| e.to_string())?
        .iter()
        .map(normalized)
        .collect::<Vec<_>>();
    let index = |names: &[&str]| headers.iter().position(|h| names.contains(&h.as_str()));
    let title =
        index(&["track name", "trackname", "title"]).ok_or("CSV has no Track Name column")?;
    let artist = index(&[
        "artist name s",
        "artist names",
        "artist name",
        "artistname",
        "artists",
        "artist",
    ])
    .ok_or("CSV has no Artist Name(s) column")?;
    let duration = index(&["duration ms", "durationms", "track duration ms"]);
    let isrc = index(&["isrc"]);
    let uri = index(&["track uri", "trackuri", "uri"]);
    let mut result = Collection {
        name: name.to_owned(),
        kind: guessed_kind(name),
        songs: Vec::new(),
        skipped: 0,
    };
    for record in reader.records() {
        let record = record.map_err(|e| format!("Invalid CSV: {e}"))?;
        if uri.and_then(|i| record.get(i)).is_some_and(|u| {
            !u.is_empty()
                && !u.starts_with("spotify:track:")
                && !u.starts_with("https://open.spotify.com/track/")
        }) {
            result.skipped += 1;
            continue;
        }
        let title = record.get(title).unwrap_or_default().trim();
        let artists: Vec<String> = record
            .get(artist)
            .unwrap_or_default()
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        if title.is_empty() || artists.is_empty() {
            result.skipped += 1;
            continue;
        }
        result.songs.push(Song {
            title: title.to_owned(),
            artists,
            duration: duration
                .and_then(|i| record.get(i))
                .and_then(|v| v.parse().ok())
                .filter(|v| *v > 0),
            isrc: isrc
                .and_then(|i| record.get(i))
                .filter(|v| !v.is_empty())
                .map(str::to_owned),
        });
        if result.songs.len() > TRACK_LIMIT {
            return Err("Too many tracks (maximum 50,000)".into());
        }
    }
    Ok(result)
}

fn string_at<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    })
}

fn json_song(value: &Value) -> Option<Song> {
    // A null track is a podcast/local file, not a track object to search.
    let track = if value.get("track").is_some_and(Value::is_object) {
        &value["track"]
    } else if value.get("item").is_some_and(Value::is_object) {
        &value["item"]
    } else {
        value
    };
    if track
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|v| v != "track")
    {
        return None;
    }
    if string_at(track, &["trackUri", "uri"]).is_some_and(|u| {
        !u.starts_with("spotify:track:") && !u.starts_with("https://open.spotify.com/track/")
    }) {
        return None;
    }
    let title = string_at(track, &["trackName", "name", "title", "track"])?;
    let artists = if let Some(values) = track.get("artists").and_then(Value::as_array) {
        values
            .iter()
            .filter_map(|v| v.as_str().or_else(|| v.get("name").and_then(Value::as_str)))
            .map(str::to_owned)
            .collect()
    } else {
        string_at(track, &["artistName", "artist"])?
            .split(';')
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    if artists.is_empty() {
        return None;
    }
    Some(Song {
        title: title.to_owned(),
        artists,
        duration: track
            .get("duration_ms")
            .or_else(|| track.get("durationMs"))
            .and_then(Value::as_u64)
            .filter(|d| *d > 0),
        isrc: string_at(track, &["isrc"])
            .or_else(|| track.pointer("/external_ids/isrc").and_then(Value::as_str))
            .map(str::to_owned),
    })
}

fn json_list(name: &str, kind: Kind, items: &[Value]) -> Result<Collection, String> {
    if items.len() > TRACK_LIMIT {
        return Err("Too many tracks (maximum 50,000)".into());
    }
    let songs: Vec<_> = items.iter().filter_map(json_song).collect();
    let skipped = items.len() - songs.len();
    Ok(Collection {
        name: name.chars().take(200).collect(),
        kind,
        songs,
        skipped,
    })
}

fn json_collections(name: &str, raw: &str) -> Result<Vec<Collection>, String> {
    let value: Value = serde_json::from_str(raw).map_err(|e| format!("Invalid JSON: {e}"))?;
    if let Some(playlists) = value.get("playlists").and_then(Value::as_array) {
        return playlists
            .iter()
            .map(|p| {
                let name = string_at(p, &["name", "title"]).ok_or("Playlist has no name")?;
                let items = p
                    .get("items")
                    .or_else(|| p.get("tracks"))
                    .and_then(Value::as_array)
                    .ok_or("Playlist has no items")?;
                json_list(name, Kind::Playlist, items)
            })
            .collect();
    }
    for key in ["likedSongs", "liked_tracks"] {
        if let Some(items) = value.get(key).and_then(Value::as_array) {
            return Ok(vec![json_list("Liked Songs", Kind::Likes, items)?]);
        }
    }
    if let Some(items) = value.get("tracks").and_then(Value::as_array) {
        let is_library = guessed_kind(name) == Kind::Likes
            || value.get("albums").is_some()
            || value.get("artists").is_some();
        let kind = if is_library {
            Kind::Likes
        } else {
            Kind::Playlist
        };
        return Ok(vec![json_list(
            string_at(&value, &["name", "title"]).unwrap_or(name),
            kind,
            items,
        )?]);
    }
    if let Some(items) = value.get("items").and_then(Value::as_array) {
        return Ok(vec![json_list(name, guessed_kind(name), items)?]);
    }
    Err(
        "Unsupported JSON. Choose Playlist*.json or YourLibrary.json from Spotify account data"
            .into(),
    )
}

fn parse_entry(name: &str, data: &[u8]) -> Result<Vec<Collection>, String> {
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("Spotify");
    let raw = text(data)?;
    match path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "csv" => Ok(vec![csv_collection(stem, &raw)?]),
        "json" => json_collections(stem, &raw),
        _ => Err("Choose a Spotify CSV, JSON or ZIP export".into()),
    }
}

fn parse_zip(data: &[u8]) -> Result<Vec<Collection>, String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(data)).map_err(|e| format!("Invalid ZIP: {e}"))?;
    if archive.len() > 2000 {
        return Err("ZIP has too many entries".into());
    }
    let mut collections = Vec::new();
    let mut bytes = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_owned();
        let lower = name.to_ascii_lowercase();
        let stem = Path::new(&lower)
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or_default();
        if entry.is_dir()
            || !(lower.ends_with(".csv")
                || lower.ends_with(".json")
                    && (stem.starts_with("playlist")
                        || stem == "yourlibrary"
                        || stem == "your_library"))
        {
            continue;
        }
        bytes += entry.size();
        if entry.size() > FILE_LIMIT || bytes > TOTAL_LIMIT {
            return Err("Export is too large (16 MiB per file, 128 MiB total)".into());
        }
        let mut data = Vec::new();
        (&mut entry)
            .take(FILE_LIMIT + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() as u64 > FILE_LIMIT {
            return Err("ZIP entry exceeds 16 MiB".into());
        }
        collections.extend(parse_entry(&name, &data).map_err(|e| format!("{name}: {e}"))?);
    }
    validate_plan(&collections)?;
    Ok(collections)
}

fn validate_plan(collections: &[Collection]) -> Result<(), String> {
    if collections.is_empty() {
        return Err("No playlists or liked tracks found in this export".into());
    }
    if collections.len() > 1000
        || collections.iter().map(|c| c.songs.len()).sum::<usize>() > TRACK_LIMIT
    {
        return Err("Export exceeds 1,000 playlists or 50,000 tracks".into());
    }
    Ok(())
}

fn load_files(paths: &[String]) -> Result<Vec<Collection>, String> {
    if paths.is_empty() || paths.len() > 100 {
        return Err("Choose between 1 and 100 export files".into());
    }
    let mut collections = Vec::new();
    let mut bytes = 0;
    for path in paths {
        let path = Path::new(path);
        let zip = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
        let limit = if zip { TOTAL_LIMIT } else { FILE_LIMIT };
        let file = std::fs::File::open(path).map_err(|e| format!("Could not open export: {e}"))?;
        if file.metadata().map_err(|e| e.to_string())?.len() > limit {
            return Err("Export file is too large".into());
        }
        let mut data = Vec::new();
        file.take(limit + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        bytes += data.len() as u64;
        if data.len() as u64 > limit || bytes > TOTAL_LIMIT {
            return Err("Exports exceed the size limit".into());
        }
        collections.extend(if zip {
            parse_zip(&data)?
        } else {
            parse_entry(
                path.file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or_default(),
                &data,
            )?
        });
        validate_plan(&collections)?;
    }
    Ok(collections)
}

/// Strong metadata checks take precedence over search rank. Unrequested remix,
/// live and speed variants are rejected even when the artist matches.
fn match_score(song: &Song, track: &Track) -> Option<u32> {
    if track.is_blocked() || track.is_snippet() || !track.streamable {
        return None;
    }
    let source = normalized(&song.title);
    let title = normalized(&track.title);
    let has = |value: &str, word: &str| value.split_whitespace().any(|v| v == word);
    for word in [
        "remix",
        "cover",
        "live",
        "instrumental",
        "karaoke",
        "acoustic",
        "slowed",
        "sped",
        "nightcore",
        "reverb",
        "edit",
        "remaster",
        "remastered",
    ] {
        if has(&source, word) != has(&title, word) {
            return None;
        }
    }
    if song.duration.is_some_and(|duration| duration > 0) && track.effective_duration_ms() == 0 {
        return None;
    }
    let duration = song
        .duration
        .zip(Some(track.effective_duration_ms()).filter(|d| *d > 0));
    if duration.is_some_and(|(a, b)| a.abs_diff(b) > (a / 20).clamp(3000, 10_000)) {
        return None;
    }
    let isrc = song
        .isrc
        .as_ref()
        .zip(
            track
                .publisher_metadata
                .as_ref()
                .and_then(|m| m.isrc.as_ref()),
        )
        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b));
    if isrc {
        return Some(100);
    }
    let artist = normalized(track.artist());
    let source_base = recording_title(&song.title);
    let title_base = recording_title(&track.title);
    let title_match = title_base == source_base
        || song
            .artists
            .iter()
            .any(|a| title_base == format!("{} {source_base}", normalized(a)) || title_base == format!("{source_base} {}", normalized(a)));
    if !title_match {
        return None;
    }
    let artist_match = song.artists.iter().any(|a| {
        let a = normalized(a);
        let mut credits = track.artist().to_lowercase();
        for separator in [" & ", ", ", ";", " / ", " feat. ", " ft. ", " featuring "] {
            credits = credits.replace(separator, ";");
        }
        !a.is_empty()
            && (artist == a
                || credits.split(';').any(|credit| normalized(credit) == a)
                || title.starts_with(&format!("{a} ")) || title.ends_with(&format!(" {a}")))
    });
    if !artist_match {
        return None;
    }
    Some(
        70 + if title == source { 10 } else { 0 }
            + if duration.is_some() { 5 } else { 0 }
            + if !track.is_snippet() { 3 } else { 0 },
    )
}

async fn find_song(context: &Context, song: &Song) -> Result<Option<u64>, String> {
    let query = format!(
        "{} {}",
        song.artists.first().map(String::as_str).unwrap_or_default(),
        song.title
    );
    let mut pager = api::endpoints::search_import_tracks(&context.client, &query).await;
    let mut best = None;
    for _ in 0..2 {
        context.check()?;
        if pager.is_exhausted() {
            break;
        }
        let candidates = pager
            .next_page()
            .await
            .map_err(|e| format!("SoundCloud search failed: {e}"))?;
        for track in candidates {
            if let Some(score) = match_score(song, &track) {
                if best.is_none_or(|(_, previous)| score > previous) {
                    best = Some((track.id, score));
                }
            }
        }
        if best.is_some_and(|(_, score)| score >= 88) {
            break;
        }
    }
    Ok(best.map(|(id, _)| id))
}

struct Context {
    job: Arc<Import>,
    client: Arc<api::ApiClient>,
    original_session: Arc<auth::Session>,
    active_session: Arc<Mutex<Option<Arc<auth::Session>>>>,
    store: super::Store,
    confirmed_likes: Mutex<Vec<u64>>,
}

impl Context {
    fn check(&self) -> Result<(), String> {
        if self.job.cancel.load(Ordering::Acquire)
            || !self.store.signed_in()
            || !self
                .active_session
                .lock()
                .as_ref()
                .is_some_and(|s| Arc::ptr_eq(s, &self.original_session))
        {
            return Err("cancelled".into());
        }
        Ok(())
    }
}

async fn run(context: &Context, collections: Vec<Collection>) -> Result<(), String> {
    let mut cache = HashMap::<String, Option<u64>>::new();
    let mut liked = HashSet::new();
    if collections.iter().any(|c| c.kind == Kind::Likes) {
        let mut pager = api::endpoints::my_liked_tracks(&context.client).await;
        while !pager.is_exhausted() {
            context.check()?;
            let page = pager
                .next_page()
                .await
                .map_err(|e| format!("Could not read existing likes: {e}"))?;
            liked.extend(page.iter().map(|t| t.id));
        }
    }
    for collection in collections {
        context.check()?;
        let report_index = {
            let mut view = context.job.view.lock();
            let index = view.reports.len();
            view.reports.push(Report {
                name: collection.name.clone(),
                ..Report::default()
            });
            index
        };
        let mut ids = Vec::new();
        let mut seen = HashSet::new();
        for song in collection.songs {
            context.check()?;
            context.job.view.lock().title = song.label();
            let key = song.key();
            let id = if let Some(id) = cache.get(&key) {
                *id
            } else {
                let id = find_song(context, &song).await?;
                cache.insert(key, id);
                tokio::time::sleep(Duration::from_millis(150)).await;
                id
            };
            context.check()?;
            if let Some(id) = id {
                {
                    let mut view = context.job.view.lock();
                    view.matched += 1;
                    view.reports[report_index].matched += 1;
                }
                if seen.insert(id) {
                    ids.push(id);
                    if collection.kind == Kind::Likes {
                        if liked.insert(id) {
                            api::endpoints::like_track(&context.client, id)
                                .await
                                .map_err(|e| format!("Could not add like: {e}"))?;
                            context.store.invalidate(&Key::Likes);
                            context.confirmed_likes.lock().push(id);
                            context.job.view.lock().reports[report_index].added += 1;
                        } else {
                            context.job.view.lock().reports[report_index].already_liked += 1;
                        }
                    }
                }
            } else {
                let mut view = context.job.view.lock();
                let report = &mut view.reports[report_index];
                report.not_found += 1;
                if report.missing.len() < 200 {
                    report.missing.push(song.label());
                }
            }
            context.job.view.lock().current += 1;
        }
        context.check()?;
        if collection.kind == Kind::Playlist && !ids.is_empty() {
            save_playlist_parts(context, &collection.name, &ids, report_index).await?;
        }
        context.job.view.lock().reports[report_index].completed = true;
    }
    Ok(())
}

async fn save_playlist_parts(
    context: &Context,
    name: &str,
    ids: &[u64],
    report_index: usize,
) -> Result<(), String> {
    // SoundCloud caps playlists at 500 tracks. Keep completed parts in the
    // partial report if a later write fails; never retry a playlist POST blindly.
    for (part, chunk) in ids.chunks(500).enumerate() {
        context.check()?;
        let name = if ids.len() > 500 {
            format!("{name} - part {}", part + 1)
        } else {
            name.to_owned()
        };
        let playlist = api::endpoints::create_playlist(&context.client, &name, false, chunk)
            .await
            .map_err(|e| format!("Could not create playlist '{name}': {e}"))?;
        context.store.invalidate(&Key::MyPlaylists);
        let mut view = context.job.view.lock();
        view.reports[report_index].added += chunk.len();
        view.reports[report_index].playlist_ids.push(playlist.id);
    }
    Ok(())
}

#[tauri::command]
pub fn spotify_import_status(state: tauri::State<'_, AppState>) -> View {
    state.spotify.view.lock().clone()
}

#[tauri::command]
pub async fn preview_spotify_import(
    state: tauri::State<'_, AppState>,
    paths: Vec<String>,
) -> Result<View, String> {
    if state.spotify.view.lock().running {
        return Err("Import is already running".into());
    }
    let collections = tokio::task::spawn_blocking(move || load_files(&paths))
        .await
        .map_err(|e| e.to_string())??;
    let summaries = collections
        .iter()
        .enumerate()
        .map(|(id, c)| Summary {
            id,
            name: c.name.clone(),
            kind: c.kind,
            tracks: c.songs.len(),
            skipped: c.skipped,
        })
        .collect();
    let mut view = state.spotify.view.lock();
    if view.running {
        return Err("Import is already running".into());
    }
    if state.spotify.account_changes.load(Ordering::Acquire) > 0 {
        return Err("Wait for the SoundCloud account connection to finish".into());
    }
    *state.spotify.plan.lock() = Some(collections);
    *view = View {
        ready: true,
        collections: summaries,
        ..View::default()
    };
    Ok(view.clone())
}

#[tauri::command]
pub fn start_spotify_import(
    state: tauri::State<'_, AppState>,
    selections: Vec<Selection>,
) -> Result<(), String> {
    if !state.store.signed_in() || state.client.is_demo() {
        return Err("Sign in to SoundCloud first".into());
    }
    let original_session = state
        .session
        .lock()
        .clone()
        .ok_or("No SoundCloud session")?;
    let mut view = state.spotify.view.lock();
    if view.running {
        return Err("Import is already running".into());
    }
    if state.spotify.account_changes.load(Ordering::Acquire) > 0 {
        return Err("Wait for the SoundCloud account connection to finish".into());
    }
    let mut plan = state.spotify.plan.lock();
    let choices = selections
        .iter()
        .map(|s| (s.id, s.kind))
        .collect::<HashMap<_, _>>();
    let source = plan.as_ref().ok_or("Choose an export file first")?;
    if choices.is_empty()
        || choices.len() != selections.len()
        || choices.keys().any(|i| *i >= source.len())
    {
        return Err("Choose valid collections to import".into());
    }
    let collections = plan
        .take()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .filter_map(|(i, mut c)| {
            choices.get(&i).map(|kind| {
                c.kind = *kind;
                c
            })
        })
        .collect::<Vec<_>>();
    let total = collections.iter().map(|c| c.songs.len()).sum();
    *view = View {
        running: true,
        total,
        collections: view.collections.clone(),
        ..View::default()
    };
    state.spotify.cancel.store(false, Ordering::Release);
    let client = Arc::new(api::ApiClient::new(
        state.settings.lock().client_id.clone(),
        false,
    ));
    let session = original_session.for_client(client.clone());
    let context = Context {
        job: state.spotify.clone(),
        client,
        original_session,
        active_session: state.session.clone(),
        store: state.store.clone(),
        confirmed_likes: Mutex::new(Vec::new()),
    };
    let settings = state.settings.clone();
    state.rt.spawn(async move {
        // Hold the isolated session strongly until the client has finished.
        let _session = session;
        let mut result = run(&context, collections).await;
        if context
            .active_session
            .lock()
            .as_ref()
            .is_some_and(|s| Arc::ptr_eq(s, &context.original_session))
        {
            let ids = context.confirmed_likes.lock();
            if !ids.is_empty() {
                let mut settings = settings.lock();
                let mut known = settings.liked_ids.iter().copied().collect::<HashSet<_>>();
                settings
                    .liked_ids
                    .extend(ids.iter().copied().filter(|id| known.insert(*id)));
                if let Err(error) = settings.save() {
                    if result.is_ok() {
                        result = Err(format!(
                            "Import completed, but local settings could not be saved: {error}"
                        ));
                    }
                }
            }
        }
        let mut view = context.job.view.lock();
        view.running = false;
        view.title.clear();
        if let Err(error) = result {
            if error == "cancelled" {
                view.cancelled = true;
            } else {
                view.error = error;
            }
        }
        drop(view);
        context.job.finished.notify_waiters();
    });
    Ok(())
}

#[tauri::command]
pub fn cancel_spotify_import(state: tauri::State<'_, AppState>) {
    state.spotify.cancel.store(true, Ordering::Release);
}

#[tauri::command]
pub fn open_spotify_export() -> Result<(), String> {
    webbrowser::open("https://exportify.net/").map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn song(title: &str) -> Song {
        Song {
            title: title.into(),
            artists: vec!["Artist".into()],
            duration: Some(180_000),
            isrc: None,
        }
    }
    fn track(id: u64, title: &str) -> Track {
        serde_json::from_value(json!({"id":id,"title":title,"duration":180000,"streamable":true,"publisher_metadata":{"artist":"Artist"}})).unwrap()
    }

    #[test]
    fn csv_preserves_quoted_titles_newlines_and_large_collections() {
        let csv = "Track URI,ISRC,Track Name,Artist Name(s),Duration (ms)\r\nspotify:track:1,USABC1234567,\"Song, with comma\",\"Artist;Guest\",180000\r\nspotify:track:2,,\"Two\nlines\",Artist,200000\r\nspotify:local:3,,Local,Artist,10000\r\n";
        let parsed = parse_entry("Liked Songs.csv", format!("\u{feff}{csv}").as_bytes()).unwrap();
        assert_eq!(parsed[0].kind, Kind::Likes);
        assert_eq!(parsed[0].songs[0].title, "Song, with comma");
        assert_eq!(parsed[0].songs[0].artists, ["Artist", "Guest"]);
        assert_eq!(parsed[0].songs[1].title, "Two\nlines");
        assert_eq!(parsed[0].skipped, 1);
        let large = format!(
            "Track Name,Artist Name(s)\n{}",
            (0..1200)
                .map(|i| format!("Song {i},Artist\n"))
                .collect::<String>()
        );
        assert_eq!(
            csv_collection("Long playlist", &large).unwrap().songs.len(),
            1200
        );
    }

    #[test]
    fn official_json_imports_library_and_playlists_but_not_podcasts_or_history() {
        let library = json!({"tracks":[{"artist":"Artist","track":"Song","album":"Album","uri":"spotify:track:1"}],"albums":[],"artists":[]});
        assert_eq!(
            json_collections("YourLibrary", &library.to_string()).unwrap()[0].kind,
            Kind::Likes
        );
        let playlists = json!({"playlists":[{"name":"My playlist","items":[{"track":{"trackName":"Song","artistName":"Artist","trackUri":"spotify:track:1"}}, {"track":null,"episode":{"episodeName":"Podcast"}}, {"localTrack":{"trackName":"Local","artistName":"Artist"}}]}]});
        let parsed = json_collections("Playlist1", &playlists.to_string()).unwrap();
        assert_eq!(
            (
                parsed[0].name.as_str(),
                parsed[0].songs.len(),
                parsed[0].skipped
            ),
            ("My playlist", 1, 2)
        );
        assert!(
            json_collections(
                "StreamingHistory0",
                r#"[{"trackName":"Song","artistName":"Artist","msPlayed":90000}]"#
            )
            .is_err()
        );
        assert!(json_collections("Playlist1", "{").is_err());
    }

    #[test]
    fn zip_imports_nested_exports_without_extracting_account_details() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        writer
            .start_file("exports/My playlist.csv", options)
            .unwrap();
        writer
            .write_all(b"Track Name,Artist Name(s)\nSong,Artist\n")
            .unwrap();
        writer.start_file("Account/Userdata.json", options).unwrap();
        writer
            .write_all(b"this deliberately invalid personal data must never be parsed")
            .unwrap();
        writer
            .start_file("exports/YourLibrary.json", options)
            .unwrap();
        writer
            .write_all(
                br#"{"tracks":[{"track":"Other","artist":"Artist","uri":"spotify:track:2"}]}"#,
            )
            .unwrap();
        let data = writer.finish().unwrap().into_inner();
        let parsed = parse_zip(&data).unwrap();
        assert_eq!(
            parsed
                .iter()
                .map(|c| (c.name.as_str(), c.kind, c.songs.len()))
                .collect::<Vec<_>>(),
            [
                ("My playlist", Kind::Playlist, 1),
                ("YourLibrary", Kind::Likes, 1)
            ]
        );
    }

    #[test]
    fn zip_rejects_large_expanded_entry_before_allocating_it() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "Big.csv",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        writer
            .write_all(&vec![b'a'; FILE_LIMIT as usize + 1])
            .unwrap();
        let data = writer.finish().unwrap().into_inner();
        assert!(parse_zip(&data).unwrap_err().contains("too large"));
    }

    #[test]
    fn matching_rejects_other_artists_versions_and_duration_mismatch() {
        let source = song("Song");
        assert!(match_score(&source, &track(1, "Artist - Song (Official Audio)")).is_some());
        for title in [
            "Song (Remix)",
            "Song live",
            "Song cover",
            "Song slowed + reverb",
            "Song instrumental",
            "Different song",
        ] {
            assert!(match_score(&source, &track(1, title)).is_none(), "{title}");
        }
        let mut other = track(1, "Song");
        other.publisher_metadata.as_mut().unwrap().artist = Some("Other artist".into());
        assert!(match_score(&source, &other).is_none());
        other.publisher_metadata.as_mut().unwrap().artist = Some("Artist".into());
        other.duration_ms = Some(250_000);
        assert!(match_score(&source, &other).is_none());
        assert!(match_score(&song("Song (feat. Guest)"), &track(1, "Song")).is_some());
    }

    #[test]
    fn matching_prefers_full_track_over_a_preview() {
        let source = song("Song");
        let full = track(1, "Song");
        let mut preview = track(2, "Song");
        preview.access = Some("preview".into());
        assert!(match_score(&source, &full).is_some());
        assert!(match_score(&source, &preview).is_none());
    }

    type Requests = Arc<Mutex<Vec<(String, String, Value)>>>;
    async fn fixture(fail_search: bool) -> (String, Requests, tokio::task::JoinHandle<()>) {
        fixture_errors(fail_search, false).await
    }

    async fn fixture_errors(
        fail_search: bool,
        fail_second_part: bool,
    ) -> (String, Requests, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests: Requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let handle = tokio::spawn(async move {
            loop {
                let (mut connection, _) = listener.accept().await.unwrap();
                let mut data = Vec::new();
                let mut buffer = [0; 4096];
                let header_end = loop {
                    let n = connection.read(&mut buffer).await.unwrap();
                    if n == 0 {
                        break None;
                    }
                    data.extend_from_slice(&buffer[..n]);
                    if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        break Some(end + 4);
                    }
                };
                let Some(end) = header_end else { continue };
                let header = String::from_utf8_lossy(&data[..end]).to_string();
                let length = header
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|s| s.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                while data.len() < end + length {
                    let n = connection.read(&mut buffer).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(&buffer[..n]);
                }
                let mut first = header.lines().next().unwrap().split_whitespace();
                let method = first.next().unwrap().to_owned();
                let path = first.next().unwrap().to_owned();
                assert!(
                    header
                        .to_ascii_lowercase()
                        .contains("authorization: oauth test-user-token")
                );
                let body = if length > 0 {
                    serde_json::from_slice(&data[end..]).unwrap()
                } else {
                    Value::Null
                };
                recorded.lock().push((method.clone(), path.clone(), body));
                let mut status = "200 OK";
                let response = if path.contains("/me/likes/tracks") {
                    if path.contains("cursor=next") {
                        json!({"collection":[track(2,"Two")],"next_href":null})
                    } else {
                        json!({"collection":[],"next_href":"https://api.soundcloud.com/me/likes/tracks?cursor=next"})
                    }
                } else if method == "GET" && path.contains("/tracks?") {
                    if fail_search {
                        status = "500 Internal Server Error";
                        json!({"error":"fixture failure"})
                    } else {
                        let url = url::Url::parse(&format!("http://test{path}")).unwrap();
                        let q = url
                            .query_pairs()
                            .find(|(k, _)| k == "q")
                            .unwrap()
                            .1
                            .to_string();
                        let (id, title) = if q.ends_with("One") {
                            (1, "One")
                        } else if q.ends_with("Two") {
                            (2, "Two")
                        } else {
                            (3, "Missing cover")
                        };
                        json!({"collection":[track(id,title)],"next_href":null})
                    }
                } else if method == "POST" && path.ends_with("/playlists") {
                    let count = recorded
                        .lock()
                        .iter()
                        .filter(|(m, p, _)| m == "POST" && p.ends_with("/playlists"))
                        .count();
                    if fail_second_part && count == 2 {
                        status = "500 Internal Server Error";
                        json!({"error":"fixture failure"})
                    } else {
                        json!({"id":90 + count,"title":"Imported","tracks":[]})
                    }
                } else if method == "POST" && path.contains("/likes/tracks/") {
                    Value::Null
                } else {
                    panic!("Unexpected fixture request: {method} {path}")
                };
                let response = response.to_string();
                connection.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{response}",response.len()).as_bytes()).await.unwrap();
            }
        });
        (format!("http://{address}"), requests, handle)
    }

    async fn context(broker: String) -> (Context, Arc<auth::Session>) {
        let foreground = Arc::new(api::ApiClient::new(None, false));
        let session = auth::Session::with_test_user(
            auth::AppCredentials {
                client_id: "test".into(),
                client_secret: "test".into(),
                redirect_uri: "http://127.0.0.1/callback".into(),
                server_url: Some(broker),
            },
            foreground,
        )
        .await;
        let client = Arc::new(api::ApiClient::new(None, false));
        let isolated = session.for_client(client.clone());
        let store = super::super::Store::new(client.clone(), tokio::runtime::Handle::current());
        store.set_signed_in(true);
        let context = Context {
            job: Arc::new(Import::default()),
            client,
            original_session: session.clone(),
            active_session: Arc::new(Mutex::new(Some(session))),
            store,
            confirmed_likes: Mutex::new(Vec::new()),
        };
        (context, isolated)
    }

    #[tokio::test]
    async fn import_preserves_order_privacy_and_dedupes_likes_across_all_pages() {
        let (broker, requests, server) = fixture(false).await;
        let (context, _session) = context(broker).await;
        let plan = vec![
            Collection {
                name: "Road trip".into(),
                kind: Kind::Playlist,
                songs: vec![song("Two"), song("One"), song("Two"), song("Missing")],
                skipped: 0,
            },
            Collection {
                name: "Liked Songs".into(),
                kind: Kind::Likes,
                songs: vec![song("One"), song("Two"), song("One")],
                skipped: 0,
            },
        ];
        run(&context, plan).await.unwrap();
        let data = requests.lock();
        let playlist = &data
            .iter()
            .find(|(method, path, _)| method == "POST" && path.ends_with("/playlists"))
            .unwrap()
            .2;
        assert_eq!(
            playlist["playlist"],
            json!({"title":"Road trip","sharing":"private","tracks":[{"urn":"soundcloud:tracks:2"},{"urn":"soundcloud:tracks:1"}]})
        );
        assert_eq!(
            data.iter()
                .filter(|(m, p, _)| m == "POST" && p.contains("/likes/tracks/"))
                .count(),
            1
        );
        assert!(data.iter().any(|(_, p, _)| p.contains("cursor=next")));
        assert_eq!(
            data.iter()
                .filter(|(m, p, _)| m == "GET" && p.contains("/tracks?") && !p.contains("/likes/"))
                .count(),
            3
        );
        let view = context.job.view.lock();
        assert_eq!(
            (
                view.current,
                view.matched,
                view.reports[0].added,
                view.reports[0].not_found,
                view.reports[1].added,
                view.reports[1].already_liked
            ),
            (7, 6, 2, 1, 1, 1)
        );
        server.abort();
    }

    #[tokio::test]
    async fn search_failure_stops_import_instead_of_reporting_a_missing_song() {
        let (broker, requests, server) = fixture(true).await;
        let (context, _session) = context(broker).await;
        let plan = vec![Collection {
            name: "Test".into(),
            kind: Kind::Playlist,
            songs: vec![song("One")],
            skipped: 0,
        }];
        assert!(
            run(&context, plan)
                .await
                .unwrap_err()
                .contains("search failed")
        );
        assert_eq!(context.job.view.lock().reports[0].not_found, 0);
        assert!(
            !requests
                .lock()
                .iter()
                .any(|(method, _, _)| method == "POST")
        );
        server.abort();
    }

    #[tokio::test]
    async fn cancellation_and_account_change_prevent_further_requests() {
        let (broker, requests, server) = fixture(false).await;
        let (context, _session) = context(broker).await;
        context.job.cancel.store(true, Ordering::Release);
        assert_eq!(
            run(
                &context,
                vec![Collection {
                    name: "Test".into(),
                    kind: Kind::Playlist,
                    songs: vec![song("One")],
                    skipped: 0
                }]
            )
            .await
            .unwrap_err(),
            "cancelled"
        );
        context.job.cancel.store(false, Ordering::Release);
        context.active_session.lock().take();
        assert!(context.check().is_err());
        assert!(requests.lock().is_empty());
        server.abort();
    }

    #[tokio::test]
    async fn large_playlist_keeps_all_tracks_and_reports_completed_parts_on_error() {
        for fail_second in [false, true] {
            let (broker, requests, server) = fixture_errors(false, fail_second).await;
            let (context, _session) = context(broker).await;
            context.job.view.lock().reports.push(Report::default());
            let result =
                save_playlist_parts(&context, "Long playlist", &(1..=501).collect::<Vec<_>>(), 0)
                    .await;
            assert_eq!(result.is_err(), fail_second);
            let requests = requests.lock();
            assert_eq!(requests.len(), 2); // Failed mutation is never retried.
            assert_eq!(requests[0].2["playlist"]["title"], "Long playlist - part 1");
            assert_eq!(requests[0].2["playlist"]["sharing"], "private");
            assert_eq!(
                requests[0].2["playlist"]["tracks"]
                    .as_array()
                    .unwrap()
                    .len(),
                500
            );
            assert_eq!(
                requests[0].2["playlist"]["tracks"][499]["urn"],
                "soundcloud:tracks:500"
            );
            assert_eq!(
                requests[1].2["playlist"]["tracks"],
                json!([{"urn":"soundcloud:tracks:501"}])
            );
            let view = context.job.view.lock();
            assert_eq!(view.reports[0].added, if fail_second { 500 } else { 501 });
            assert_eq!(
                view.reports[0].playlist_ids.len(),
                if fail_second { 1 } else { 2 }
            );
            server.abort();
        }
    }

    #[tokio::test]
    async fn account_change_gate_stays_closed_until_authentication_finishes() {
        let import = Arc::new(Import::default());
        import.view.lock().running = true;
        let background = import.clone();
        let waiter = tokio::spawn(async move {
            let guard = background.cancel_and_wait().await;
            assert_eq!(background.account_changes.load(Ordering::Acquire), 1);
            drop(guard);
        });
        tokio::task::yield_now().await;
        assert!(import.cancel.load(Ordering::Acquire));
        assert_eq!(import.account_changes.load(Ordering::Acquire), 1);
        import.view.lock().running = false;
        import.finished.notify_waiters();
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(import.account_changes.load(Ordering::Acquire), 0);
    }
}
