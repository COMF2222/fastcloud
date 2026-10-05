use crate::{api, config, player::Player};
use anyhow::{Context, Result, ensure};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Instant};

pub const PORTABLE: &[&str] = &[
    "theme", "language", "music_taste", "theme_presets", "quick_access", "accent_rgb", "panel_rgb",
    "panel_opacity", "panel_blur", "heading_opacity", "text_rgb", "muted_text_rgb", "interface_text_scale",
    "interface_scale", "background_opacity", "background_dim", "background_blur", "background_overlay",
    "lyrics_scale", "lyrics_blur_past", "lyrics_auto_scroll", "compact_rows", "show_track_numbers",
    "reduced_motion", "autoplay", "normalization", "crossfade_ms", "gapless",
];

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Cloud {
    pub preferences: BTreeMap<String, Value>,
    pub folders: BTreeMap<String, Value>,
    pub smart_playlists: BTreeMap<String, Value>,
    pub liked_at: BTreeMap<String, Value>,
    pub daily: Vec<Value>, pub tracks: Vec<Value>, pub totals: Value,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listen {
    pub day: String, pub track_id: u64, pub title: String, pub artist: String, pub genre: String,
    pub ms: u64, pub plays: u32, pub last_played: i64,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Cache {
    device: String,
    cloud: Cloud,
    dirty_preferences: BTreeMap<String, Value>,
    dirty_folders: BTreeMap<String, Value>,
    dirty_smart: BTreeMap<String, Value>,
    dirty_likes: BTreeMap<String, Value>,
    stats: BTreeMap<String, Listen>,
    synced_stats: BTreeMap<String, (u64, u32)>,
    seeded_likes: bool,
}

struct Inner { account: u64, epoch: u64, cache: Cache, previous: Option<(u64, u64, Instant, bool)>, session_ms: u64, counted: bool, status: String }
pub struct Hub { root: PathBuf, settings_path: PathBuf, inner: Mutex<Inner>, sync_gate: tokio::sync::Mutex<()> }

pub fn portable(settings: &config::Settings) -> BTreeMap<String, Value> {
    let all = serde_json::to_value(settings).unwrap_or_default();
    PORTABLE.iter().map(|&key| {
        let value = if !key.ends_with("_blur") && key != "crossfade_ms" && let Some(value) = all[key].as_f64() { json!((value * 1_000_000.0).round() / 1_000_000.0) } else { all[key].clone() };
        (key.into(),value)
    }).collect()
}

pub fn apply_preferences(settings: &config::Settings, values: &BTreeMap<String, Value>) -> Result<config::Settings> {
    ensure!(values.keys().all(|key| PORTABLE.contains(&key.as_str())), "Invalid portable settings fields");
    let mut all = serde_json::to_value(settings)?;
    for (key, value) in values { all[key] = value.clone(); }
    let mut next: config::Settings = serde_json::from_value(all)?;
    ensure!((0.9..=1.25).contains(&next.interface_text_scale) && (0.9..=1.15).contains(&next.interface_scale), "Invalid interface size");
    ensure!(next.crossfade_ms <= 8000 && next.theme_presets.len() <= 20 && next.quick_access.len() <= 100, "Invalid portable settings limits");
    for preset in &next.theme_presets { crate::appearance::validate(&serde_json::from_value(preset.clone())?)?; }
    crate::appearance::capture(&next,"Backup".into())?;
    next.music_taste.normalize();
    Ok(next)
}

impl Hub {
    pub fn seed_preferences(&self, settings: &config::Settings) {
        let mut inner = self.inner.lock();
        if inner.cache.cloud.preferences.is_empty() { inner.cache.cloud.preferences = portable(settings); let _ = self.save(&inner); }
    }
    pub fn new(root: PathBuf, settings_path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&root)?;
        let account: u64 = std::fs::read_to_string(root.join("account.txt")).ok().and_then(|s| s.parse().ok()).unwrap_or(0);
        let cache = Self::load_cache(&root,account);
        Ok(Self { root, settings_path, inner: Mutex::new(Inner { account, epoch: 0, cache, previous: None, session_ms: 0, counted: false, status: "offline".into() }), sync_gate: tokio::sync::Mutex::new(()) })
    }

    fn load_cache(root: &std::path::Path, account: u64) -> Cache {
        let mut cache: Cache = std::fs::read(root.join(format!("{account}.json"))).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default();
        if cache.device.len() != 32 || !cache.device.chars().all(|c| c.is_ascii_hexdigit()) { cache.device = format!("{:032x}",rand::random::<u128>()); }
        cache
    }

    fn save(&self, inner: &Inner) -> Result<()> {
        let path = self.root.join(format!("{}.json", inner.account));
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec(&inner.cache)?)?;
        std::fs::rename(temp, path)?;
        Ok(())
    }

    pub fn preferences_changed(&self, settings: &config::Settings) {
        let next = portable(settings);
        let mut inner = self.inner.lock();
        for (key, value) in next {
            if inner.cache.cloud.preferences.get(&key) != Some(&value) {
                inner.cache.cloud.preferences.insert(key.clone(), value.clone());
                inner.cache.dirty_preferences.insert(key, value);
            }
        }
        if let Err(error) = self.save(&inner) { log::warn!("personal preferences cache: {error}"); }
    }

    pub fn observe(&self, track: Option<&api::models::Track>, position: u64, playing: bool, loading: bool, speed: f32) {
        let now = Instant::now();
        let mut inner = self.inner.lock();
        let Some(track) = track else { inner.previous = None; return; };
        let active = playing && !loading;
        let mut elapsed_ms = 0;
        if let Some((id, previous, at, was_active)) = inner.previous {
            if id == track.id && active && was_active && position >= previous {
                let wall = now.duration_since(at).as_millis().min(5000) as u64;
                let advance = position - previous;
                // Count actual moving audio, never time spent paused, loading,
                // stalled or jumping ahead with the seek slider.
                if advance <= (wall as f64 * f64::from(speed.max(0.5)) * 1.3) as u64 + 250 {
                    elapsed_ms = wall.min((advance as f64 / f64::from(speed.max(0.5))) as u64);
                }
            } else if id != track.id || (position <= 1500 && previous >= track.effective_duration_ms().saturating_sub(10000)) { inner.session_ms = 0; inner.counted = false; }
        }
        inner.previous = Some((track.id, position, now, active));
        if elapsed_ms == 0 { return; }
        inner.session_ms += elapsed_ms;
        let count = !inner.counted && inner.session_ms >= 30_000.min(track.effective_duration_ms() / 2).max(1000);
        if count { inner.counted = true; }
        let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let key = format!("{day}:{}", track.id);
        let row = inner.cache.stats.entry(key).or_insert_with(|| Listen { day, track_id: track.id, title: track.title.chars().filter(|c| !c.is_control()).take(512).collect(), artist: track.artist().chars().filter(|c| !c.is_control()).take(256).collect(), genre: track.genre.clone().unwrap_or_default().chars().filter(|c| !c.is_control()).take(128).collect(), ms: 0, plays: 0, last_played: 0 });
        row.ms = row.ms.saturating_add(elapsed_ms).min(86_400_000);
        row.plays = row.plays.saturating_add(u32::from(count)).min(2880);
        row.last_played = chrono::Utc::now().timestamp();
        if row.ms / 15_000 != row.ms.saturating_sub(elapsed_ms) / 15_000 {
            if let Err(error) = self.save(&inner) { log::warn!("listening cache: {error}"); }
        }
    }

    pub fn liked(&self, id: u64) {
        let mut inner = self.inner.lock();
        let value = json!(chrono::Utc::now().timestamp());
        inner.cache.cloud.liked_at.insert(id.to_string(), value.clone());
        inner.cache.dirty_likes.insert(id.to_string(), value);
        let _ = self.save(&inner);
    }

    pub fn collections(&self) -> Value {
        let inner = self.inner.lock();
        json!({"folders": inner.cache.cloud.folders, "smartPlaylists": inner.cache.cloud.smart_playlists, "syncStatus": inner.status})
    }

    pub fn change_collection(&self, section: &str, id: String, value: Value) -> Result<()> {
        ensure!(!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || "_-".contains(c)), "Invalid collection id");
        if !value.is_null() {
            let name = value["name"].as_str().context("Collection needs a name")?;
            ensure!(!name.trim().is_empty() && name.chars().count() <= 128, "Invalid collection name");
            validate_collection(section, &value)?;
        }
        let mut inner = self.inner.lock();
        ensure!(section == "folders" || section == "smartPlaylists", "Unknown collection type");
        let previous = inner.cache.clone();
        if section == "folders" {
            ensure!(value.is_null() || inner.cache.cloud.folders.contains_key(&id) || inner.cache.cloud.folders.len() < 100, "Too many folders");
            if value.is_null() { inner.cache.cloud.folders.remove(&id); } else { inner.cache.cloud.folders.insert(id.clone(), value.clone()); }
            inner.cache.dirty_folders.insert(id, value);
        } else {
            ensure!(value.is_null() || inner.cache.cloud.smart_playlists.contains_key(&id) || inner.cache.cloud.smart_playlists.len() < 50, "Too many smart playlists");
            if value.is_null() { inner.cache.cloud.smart_playlists.remove(&id); } else { inner.cache.cloud.smart_playlists.insert(id.clone(), value.clone()); }
            inner.cache.dirty_smart.insert(id, value);
        }
        if let Err(error) = self.save(&inner) { inner.cache = previous; return Err(error); }
        Ok(())
    }

    pub fn smart_tracks(&self, id: &str, likes: &[api::models::Track]) -> Result<Vec<api::models::Track>> {
        let mut inner = self.inner.lock();
        let rule: Smart = serde_json::from_value(inner.cache.cloud.smart_playlists.get(id).cloned().context("Smart playlist not found")?)?;
        for track in likes {
            if !inner.cache.cloud.liked_at.contains_key(&track.id.to_string()) {
                let at = if inner.cache.seeded_likes { chrono::Utc::now().timestamp() } else { 0 };
                inner.cache.cloud.liked_at.insert(track.id.to_string(), json!(at));
                inner.cache.dirty_likes.insert(track.id.to_string(), json!(at));
            }
        }
        inner.cache.seeded_likes = true;
        let last: BTreeMap<u64, i64> = inner.cache.cloud.tracks.iter().filter_map(|row| Some((row["trackId"].as_u64()?, row["lastPlayed"].as_i64()?))).chain(inner.cache.stats.values().map(|row| (row.track_id, row.last_played))).fold(BTreeMap::new(), |mut map, (id, at)| { map.entry(id).and_modify(|old| *old = (*old).max(at)).or_insert(at); map });
        let now = chrono::Utc::now().timestamp();
        let mut tracks: Vec<_> = likes.iter().filter(|track| smart_matches(&rule, track, inner.cache.cloud.liked_at.get(&track.id.to_string()).and_then(Value::as_i64).unwrap_or(0), last.get(&track.id).copied().unwrap_or(0), now)).cloned().collect();
        tracks.truncate(rule.limit.clamp(1, 500));
        self.save(&inner)?;
        Ok(tracks)
    }

    pub fn statistics(&self) -> Value {
        let inner = self.inner.lock();
        let mut daily: BTreeMap<String, (u64, u64)> = inner.cache.cloud.daily.iter().filter_map(|row| Some((row["day"].as_str()?.into(), (row["ms"].as_u64()?, row["plays"].as_u64()?)))).collect();
        let mut tracks: BTreeMap<u64, Value> = inner.cache.cloud.tracks.iter().filter_map(|row| Some((row["trackId"].as_u64()?, row.clone()))).collect();
        let mut total_ms = inner.cache.cloud.totals["ms"].as_u64().unwrap_or(0);
        let mut plays = inner.cache.cloud.totals["plays"].as_u64().unwrap_or(0);
        for (key, row) in &inner.cache.stats {
            let synced = inner.cache.synced_stats.get(key).copied().unwrap_or_default();
            let delta_ms = row.ms.saturating_sub(synced.0); let delta_plays = u64::from(row.plays.saturating_sub(synced.1));
            total_ms += delta_ms; plays += delta_plays;
            let day = daily.entry(row.day.clone()).or_default(); day.0 += delta_ms; day.1 += delta_plays;
            let track = tracks.entry(row.track_id).or_insert_with(|| json!({"trackId": row.track_id, "title": row.title, "artist": row.artist, "genre": row.genre, "ms": 0, "plays": 0, "lastPlayed": 0}));
            track["ms"] = json!(track["ms"].as_u64().unwrap_or(0) + delta_ms);
            track["plays"] = json!(track["plays"].as_u64().unwrap_or(0) + delta_plays);
            track["lastPlayed"] = json!(track["lastPlayed"].as_i64().unwrap_or(0).max(row.last_played));
        }
        let mut tracks: Vec<_> = tracks.into_values().collect(); tracks.sort_by_key(|row| std::cmp::Reverse(row["ms"].as_u64().unwrap_or(0)));
        json!({"totals": {"ms": total_ms, "plays": plays}, "daily": daily.into_iter().map(|(day,(ms,plays))| json!({"day": day,"ms": ms,"plays": plays})).collect::<Vec<_>>(), "tracks": tracks, "syncStatus": inner.status})
    }

    pub async fn sync(&self, client: &api::ApiClient, settings: &Arc<Mutex<config::Settings>>, player: Option<&Arc<Player>>) -> Result<()> {
        let _gate = self.sync_gate.lock().await;
        let epoch = self.inner.lock().epoch;
        let (server, token) = client.relay_credentials().await?.context("Sign in through the Fastcloud server to sync")?;
        let http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build()?;
        let identity: Value = http.get(format!("{server}/v1/session")).header("Authorization", format!("OAuth {token}")).send().await?.error_for_status()?.json().await?;
        let account = identity["user_id"].as_u64().context("Invalid account identity")?;
        let current_preferences = portable(&settings.lock());
        {
            let mut inner = self.inner.lock();
            if inner.epoch != epoch { return Ok(()); }
            if inner.account != account {
                self.save(&inner)?;
                let first = inner.account == 0;
                let migrated = if first && !self.root.join(format!("{account}.json")).exists() { Some(inner.cache.clone()) } else { None };
                inner.cache = migrated.unwrap_or_else(|| Self::load_cache(&self.root,account));
                inner.account = account; inner.previous = None; inner.session_ms = 0; inner.counted = false;
                std::fs::write(self.root.join("account.txt"), account.to_string())?;
                if first && inner.cache.cloud.preferences.is_empty() { inner.cache.cloud.preferences = current_preferences; }
            }
            self.save(&inner)?;
        }
        let url = format!("{server}/v1/me/personal");
        let remote: Cloud = http.get(&url).header("Authorization", format!("OAuth {token}")).send().await?.error_for_status()?.json().await?;
        let payload = {
            let mut inner = self.inner.lock();
            if inner.epoch != epoch { return Ok(()); }
            if remote.preferences.is_empty() && inner.cache.dirty_preferences.is_empty() {
                inner.cache.dirty_preferences = if inner.cache.cloud.preferences.is_empty() { portable(&config::Settings::default()) } else { inner.cache.cloud.preferences.clone() };
            }
            let stats: Vec<_> = inner.cache.stats.iter().filter(|(key,row)| inner.cache.synced_stats.get(*key) != Some(&(row.ms, row.plays))).take(100).map(|(_,row)| row.clone()).collect();
            json!({"preferences": inner.cache.dirty_preferences, "folders": inner.cache.dirty_folders.iter().take(100).collect::<BTreeMap<_,_>>(), "smartPlaylists": inner.cache.dirty_smart.iter().take(100).collect::<BTreeMap<_,_>>(), "likedAt": inner.cache.dirty_likes.iter().take(100).collect::<BTreeMap<_,_>>(), "device": inner.cache.device, "stats": stats})
        };
        let has_changes = ["preferences","folders","smartPlaylists","likedAt"].iter().any(|key| payload[key].as_object().is_some_and(|map| !map.is_empty())) || payload["stats"].as_array().is_some_and(|rows| !rows.is_empty());
        let remote = if has_changes { http.post(&url).header("Authorization", format!("OAuth {token}")).json(&payload).send().await?.error_for_status()?.json::<Cloud>().await? } else { remote };
        let mut local = settings.lock();
        let preferences = {
            let mut inner = self.inner.lock();
            if inner.account != account || inner.epoch != epoch { return Ok(()); }
            for (section, pending) in [("preferences", &mut inner.cache.dirty_preferences)] {
                pending.retain(|key,value| payload[section].get(key) != Some(value));
            }
            inner.cache.dirty_folders.retain(|key,value| payload["folders"].get(key) != Some(value));
            inner.cache.dirty_smart.retain(|key,value| payload["smartPlaylists"].get(key) != Some(value));
            inner.cache.dirty_likes.retain(|key,value| payload["likedAt"].get(key) != Some(value));
            for row in payload["stats"].as_array().into_iter().flatten() {
                let row: Listen = serde_json::from_value(row.clone())?;
                inner.cache.synced_stats.insert(format!("{}:{}", row.day, row.track_id), (row.ms,row.plays));
            }
            inner.cache.cloud = remote;
            let pending = inner.cache.dirty_preferences.clone(); inner.cache.cloud.preferences.extend(pending);
            let pending = inner.cache.dirty_folders.clone(); for (id,value) in pending { if value.is_null() { inner.cache.cloud.folders.remove(&id); } else { inner.cache.cloud.folders.insert(id,value); } }
            let pending = inner.cache.dirty_smart.clone(); for (id,value) in pending { if value.is_null() { inner.cache.cloud.smart_playlists.remove(&id); } else { inner.cache.cloud.smart_playlists.insert(id,value); } }
            inner.status = "synced".into(); self.save(&inner)?;
            inner.cache.cloud.preferences.clone()
        };
        let next = apply_preferences(&local, &preferences)?;
        if portable(&local) != portable(&next) { next.save_to(&self.settings_path)?; *local = next; }
        if let Some(player) = player { player.set_autoplay(local.autoplay); player.set_audio_preferences(local.normalization, local.crossfade_ms, local.gapless); }
        Ok(())
    }
    pub fn sync_failed(&self) { self.inner.lock().status = "offline".into(); }

    pub fn flush(&self) {
        if let Err(error) = self.save(&self.inner.lock()) { log::warn!("personal data cache: {error}"); }
    }

    pub fn detach_account(&self) -> Result<()> {
        let mut inner = self.inner.lock();
        self.save(&inner)?;
        inner.epoch = inner.epoch.wrapping_add(1);
        inner.account = 0;
        inner.cache = Cache { device: format!("{:032x}", rand::random::<u128>()), ..Default::default() };
        inner.cache.cloud.preferences = portable(&config::Settings::default());
        inner.previous = None; inner.session_ms = 0; inner.counted = false; inner.status = "offline".into();
        std::fs::write(self.root.join("account.txt"), "0")?;
        self.save(&inner)
    }

    pub fn import_collections(&self, folders: BTreeMap<String,Value>, smart: BTreeMap<String,Value>) -> Result<()> {
        let mut inner = self.inner.lock();
        let mut next = inner.cache.clone();
        next.cloud.folders.extend(folders.clone()); next.cloud.smart_playlists.extend(smart.clone());
        ensure!(next.cloud.folders.len() <= 100 && next.cloud.smart_playlists.len() <= 50, "Too many saved collections");
        next.dirty_folders.extend(folders); next.dirty_smart.extend(smart);
        let previous = std::mem::replace(&mut inner.cache,next);
        if let Err(error) = self.save(&inner) { inner.cache = previous; return Err(error); }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Backup { pub version: u8, pub preferences: BTreeMap<String,Value>, pub folders: BTreeMap<String,Value>, pub smart_playlists: BTreeMap<String,Value> }

pub fn read_backup(path: &std::path::Path) -> Result<Backup> {
    ensure!(std::fs::metadata(path)?.len() <= 2_097_152, "Settings backup is too large");
    let backup: Backup = serde_json::from_slice(&std::fs::read(path)?)?;
    ensure!(backup.version == 1 && backup.folders.len() <= 100 && backup.smart_playlists.len() <= 50, "Invalid settings backup");
    apply_preferences(&config::Settings::default(),&backup.preferences)?;
    for (section,items) in [("folders",&backup.folders),("smartPlaylists",&backup.smart_playlists)] {
        for (id,value) in items { ensure!(!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || "_-".contains(c)), "Invalid collection id"); validate_collection(section,value)?; }
    }
    Ok(backup)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Folder { name: String, playlist_ids: Vec<u64>, pinned: bool, order: u32 }
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Smart { name: String, genre: String, added_days: u32, unplayed_days: u32, limit: usize }

pub fn validate_collection(section: &str, value: &Value) -> Result<()> {
    if section == "folders" {
        let folder: Folder = serde_json::from_value(value.clone())?;
        ensure!(folder.playlist_ids.len() <= 1000 && folder.playlist_ids.iter().all(|id| *id > 0 && *id < (1u64 << 53)) && folder.order <= 10000, "Invalid folder limits");
    } else {
        let rule: Smart = serde_json::from_value(value.clone())?;
        ensure!(rule.genre.chars().count() <= 48 && rule.added_days <= 365 && rule.unplayed_days <= 365 && (1..=500).contains(&rule.limit), "Invalid smart playlist limits");
    }
    let name = value["name"].as_str().context("Missing collection name")?;
    ensure!(!name.trim().is_empty() && name.chars().count() <= 128 && !name.chars().any(char::is_control), "Invalid collection name");
    Ok(())
}

fn smart_matches(rule: &Smart, track: &api::models::Track, added: i64, played: i64, now: i64) -> bool {
    let genre_key = |value: &str| value.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
    (rule.genre.trim().is_empty() || genre_key(track.genre.as_deref().unwrap_or("")).contains(&genre_key(&rule.genre)))
        && (rule.added_days == 0 || (added > 0 && added >= now - i64::from(rule.added_days) * 86400))
        && (rule.unplayed_days == 0 || played < now - i64::from(rule.unplayed_days) * 86400)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_backup_preserves_device_settings_and_excludes_credentials() {
        let settings = config::Settings { client_id: Some("secret".into()), background_image: Some("C:/private.png".into()), volume: 0.3, ..Default::default() };
        let mut values = portable(&settings); values.insert("theme".into(), json!("Light"));
        let next = apply_preferences(&settings,&values).unwrap();
        assert_eq!(next.volume,0.3); assert_eq!(next.client_id.as_deref(),Some("secret"));
        assert!(!serde_json::to_string(&values).unwrap().contains("private"));
        values.insert("client_id".into(),json!("bad")); assert!(apply_preferences(&settings,&values).is_err());
    }
    #[test]
    fn smart_rules_exclude_unknown_like_dates_and_recently_played_tracks() {
        let track = crate::demo::demo_tracks().remove(0);
        let rule = Smart { name:"Fixture".into(),genre:"Ambient".into(),added_days:7,unplayed_days:30,limit:50 };
        let now = 1800000000;
        assert!(smart_matches(&rule,&track,now,0,now));
        assert!(!smart_matches(&rule,&track,0,0,now));
        assert!(!smart_matches(&rule,&track,now,now-60,now));
    }
    #[test]
    fn listening_counts_moving_audio_but_not_pause_seek_or_loading() {
        let dir = tempfile::tempdir().unwrap(); let hub = Hub::new(dir.path().to_owned(),dir.path().join("settings.json")).unwrap();
        let track = crate::demo::demo_tracks().remove(0);
        hub.observe(Some(&track),0,true,false,1.0);
        hub.inner.lock().previous = Some((track.id,0,Instant::now()-std::time::Duration::from_secs(1),true));
        hub.observe(Some(&track),1000,true,false,1.0);
        assert_eq!(hub.statistics()["totals"]["ms"],1000);
        hub.inner.lock().previous = Some((track.id,1000,Instant::now()-std::time::Duration::from_secs(1),true));
        hub.observe(Some(&track),90000,true,false,1.0);
        hub.observe(Some(&track),91000,false,false,1.0);
        hub.observe(Some(&track),92000,true,true,1.0);
        assert_eq!(hub.statistics()["totals"]["ms"],1000);
    }
    #[test]
    fn unknown_backup_fields_are_rejected_before_mutation() {
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("backup.json");
        std::fs::write(&path,serde_json::to_vec(&json!({"version":1,"preferences":{"client_id":"secret"},"folders":{},"smartPlaylists":{}})).unwrap()).unwrap();
        assert!(read_backup(&path).is_err());
        let original = config::Settings::default(); let mut prefs = portable(&original); prefs.insert("interface_text_scale".into(),json!(100));
        assert!(apply_preferences(&original,&prefs).is_err());
    }
    #[test]
    fn account_caches_and_device_counters_survive_restart_without_leaking_on_sign_out() {
        let dir = tempfile::tempdir().unwrap(); let settings_path = dir.path().join("settings.json");
        let hub = Hub::new(dir.path().to_owned(),settings_path.clone()).unwrap();
        hub.inner.lock().account = 123;
        let device = hub.inner.lock().cache.device.clone();
        hub.change_collection("folders","fixture".into(),json!({"name":"Road","playlistIds":[12],"pinned":true,"order":0})).unwrap();
        std::fs::write(dir.path().join("account.txt"),"123").unwrap(); hub.flush();
        let restored = Hub::new(dir.path().to_owned(),settings_path).unwrap();
        assert_eq!(restored.inner.lock().cache.device,device);
        assert_eq!(restored.collections()["folders"]["fixture"]["name"],"Road");
        restored.detach_account().unwrap(); assert_eq!(restored.collections()["folders"],json!({}));
        assert_eq!(Hub::load_cache(dir.path(),123).cloud.folders["fixture"]["name"],"Road");
    }
    #[tokio::test]
    async fn server_preferences_are_applied_without_uploading_device_paths_or_credentials() {
        use tokio::io::{AsyncReadExt,AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let broker = format!("http://{}",listener.local_addr().unwrap());
        let client = Arc::new(api::ApiClient::new(None,false));
        let _session = crate::auth::Session::with_test_user(crate::auth::AppCredentials { client_id:"fixture".into(),client_secret:String::new(),redirect_uri:"http://127.0.0.1:41317/callback".into(),server_url:Some(broker) },client.clone()).await;
        let server = tokio::spawn(async move {
            for (path,body) in [("GET /v1/session ",json!({"user_id":123,"admin":false})),("GET /v1/me/personal ",json!({"preferences":{"theme":"Light","language":"Russian"},"folders":{},"smartPlaylists":{},"likedAt":{},"daily":[],"tracks":[],"totals":{"ms":0,"plays":0}}))] {
                let (mut socket,_) = listener.accept().await.unwrap(); let mut bytes = vec![0;8192]; let len = socket.read(&mut bytes).await.unwrap();
                let request = String::from_utf8_lossy(&bytes[..len]); assert!(request.starts_with(path)); assert!(request.to_lowercase().contains("authorization: oauth test-user-token")); assert!(!request.contains("private.png"));
                let body = serde_json::to_string(&body).unwrap(); let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()); socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let dir = tempfile::tempdir().unwrap(); let hub = Hub::new(dir.path().to_owned(),dir.path().join("settings.json")).unwrap();
        let settings = Arc::new(Mutex::new(config::Settings { client_id:Some("secret".into()),background_image:Some("C:/private.png".into()),volume:0.3,..Default::default() }));
        hub.seed_preferences(&settings.lock()); hub.sync(&client,&settings,None).await.unwrap();
        assert_eq!(settings.lock().theme,config::ThemeMode::Light); assert_eq!(settings.lock().volume,0.3); assert_eq!(settings.lock().client_id.as_deref(),Some("secret"));
        tokio::time::timeout(std::time::Duration::from_secs(5),server).await.unwrap().unwrap();
    }
}
