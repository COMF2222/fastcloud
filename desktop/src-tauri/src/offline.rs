use crate::api::{self, models::Track};
use crate::audio::{cache::AudioCache, hls::HlsDownloader};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineEntry {
    pub track: Track,
    pub bytes: u64,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capacity { pub used_bytes: u64, pub free_bytes: u64, pub limit_bytes: u64 }

pub fn capacity(limit_mb: u64) -> Result<Capacity> {
    Ok(Capacity { used_bytes: list()?.iter().map(|entry| entry.bytes).sum(),
        free_bytes: fs2::available_space(root()?)?, limit_bytes: limit_mb.saturating_mul(1024 * 1024) })
}

pub fn estimated_bytes(track: &Track) -> u64 {
    // 192 kbps plus manifest/container headroom; an estimate, not a promise.
    track.effective_duration_ms().max(30_000).saturating_mul(24) * 115 / 100 + 65_536
}

pub fn set_pinned(track_id: u64, pinned: bool) -> Result<()> {
    let _mutation = mutation_guard()?;
    set_pinned_at(&root()?, track_id, pinned)
}

fn set_pinned_at(base: &Path, track_id: u64, pinned: bool) -> Result<()> {
    let path = base.join(track_id.to_string()).join("track.json");
    let mut entry: OfflineEntry = serde_json::from_slice(&std::fs::read(&path)?)?;
    entry.pinned = pinned;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, serde_json::to_vec(&entry)?)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

pub fn clear_unpinned(current_track: Option<u64>) -> Result<()> {
    let _mutation = mutation_guard()?;
    clear_unpinned_at(&root()?, current_track)?;
    Ok(())
}

pub fn root() -> Result<PathBuf> {
    let path = crate::config::app_paths()?.root.join("offline");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn list() -> Result<Vec<OfflineEntry>> { list_at(&root()?) }

fn list_at(base: &Path) -> Result<Vec<OfflineEntry>> {
    let mut entries = Vec::new();
    for folder in std::fs::read_dir(base)? {
        let path = folder?.path();
        if !path.is_dir() || !path.join("playlist.m3u8").exists() {
            continue;
        }
        if let Ok(bytes) = std::fs::read(path.join("track.json"))
            && let Ok(entry) = serde_json::from_slice::<OfflineEntry>(&bytes)
        {
            if path.file_name().and_then(|name| name.to_str()).and_then(|name| name.parse::<u64>().ok()) == Some(entry.track.id) { entries.push(entry); }
        }
    }
    let order = read_like_order(base)?;
    sort_by_like_order(&mut entries, &order);
    Ok(entries)
}

fn read_like_order(base: &Path) -> Result<Vec<u64>> {
    let path = base.join("like-order.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_slice(&std::fs::read(path)?).unwrap_or_default())
}

pub fn set_like_order(track_ids: Vec<u64>) -> Result<()> {
    ensure!(track_ids.len() <= 100_000, "Too many liked tracks");
    let mut seen = HashSet::new();
    let order: Vec<u64> = track_ids.into_iter().filter(|id| seen.insert(*id)).collect();
    let path = root()?.join("like-order.json");
    let bytes = serde_json::to_vec(&order)?;
    if std::fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

fn sort_by_like_order(entries: &mut [OfflineEntry], order: &[u64]) {
    let positions: HashMap<u64, usize> = order.iter().enumerate().map(|(index, id)| (*id, index)).collect();
    entries.sort_by(|a, b| {
        positions.get(&a.track.id).copied().unwrap_or(usize::MAX)
            .cmp(&positions.get(&b.track.id).copied().unwrap_or(usize::MAX))
            .then_with(|| a.track.title.to_lowercase().cmp(&b.track.title.to_lowercase()))
    });
}

static DOWNLOAD_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn mutation_guard() -> Result<tokio::sync::MutexGuard<'static, ()>> {
    DOWNLOAD_LOCK.try_lock().context("Wait for the offline download or cleanup to finish")
}

pub async fn download(client: Arc<api::ApiClient>, track: Track, limit_mb: u64) -> Result<OfflineEntry> {
    let _download = DOWNLOAD_LOCK.lock().await;
    let root = root()?;
    let target = root.join(track.id.to_string());
    if target.join("playlist.m3u8").exists() {
        let bytes = std::fs::read(target.join("track.json"))?;
        return Ok(serde_json::from_slice(&bytes)?);
    }
    let budget = capacity(limit_mb)?;
    let estimate = estimated_bytes(&track);
    ensure!(budget.free_bytes > estimate.saturating_add(64 * 1024 * 1024), "Not enough free disk space for this download");
    ensure!(budget.limit_bytes == 0 || budget.used_bytes.saturating_add(estimate) <= budget.limit_bytes, "Offline storage limit reached; clear unpinned downloads or increase the limit");
    let streams = client.playback_streams(&track.urn()).await?;
    let stream = streams
        .best_full()
        .context("Full track is unavailable for offline download")?;
    let downloader = HlsDownloader::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?,
        Arc::new(AudioCache::disabled()),
    );
    downloader.set_oauth(client.oauth_token());
    let playlist = downloader
        .playlist(stream)
        .await
        .context("Download track playlist")?;
    ensure!(
        !playlist.segments.is_empty() && playlist.segments.len() <= 1000,
        "Invalid or oversized playlist"
    );
    let temporary = root.join(format!(".partial-{}-{}", track.id, rand::random::<u64>()));
    std::fs::create_dir(&temporary)?;
    let max_bytes = if budget.limit_bytes == 0 { 2 * 1024 * 1024 * 1024 }
        else { budget.limit_bytes.saturating_sub(budget.used_bytes).min(2 * 1024 * 1024 * 1024) };
    let result = download_into(&downloader, &playlist, &track, &temporary, max_bytes).await;
    match result {
        Ok(entry) => {
            if target.exists() {
                std::fs::remove_dir_all(&temporary)?;
                return Ok(entry);
            }
            std::fs::rename(&temporary, &target)?;
            Ok(entry)
        }
        Err(error) => {
            let _ = std::fs::remove_dir_all(&temporary);
            Err(error)
        }
    }
}

async fn download_into(
    downloader: &HlsDownloader,
    playlist: &crate::audio::hls::MediaPlaylist,
    track: &Track,
    directory: &Path,
    max_bytes: u64,
) -> Result<OfflineEntry> {
    let mut manifest = String::from("#EXTM3U\n#EXT-X-VERSION:3\n");
    let mut total = 0u64;
    if let Some(url) = &playlist.init_uri {
        let bytes = downloader.init_segment(&track.urn(), url).await?;
        total += bytes.len() as u64;
        ensure!(total <= max_bytes, "Offline storage limit reached");
        ensure!(fs2::available_space(directory)? > (bytes.len() as u64).saturating_add(64 * 1024 * 1024), "Not enough free disk space");
        std::fs::write(directory.join("init.bin"), bytes)?;
        manifest.push_str("#EXT-X-MAP:URI=\"init.bin\"\n");
    }
    for (index, url) in playlist.segments.iter().enumerate() {
        let bytes = downloader.segment(&track.urn(), url).await?;
        total += bytes.len() as u64;
        ensure!(total <= max_bytes, "Offline storage limit reached");
        ensure!(fs2::available_space(directory)? > (bytes.len() as u64).saturating_add(64 * 1024 * 1024), "Not enough free disk space");
        let name = format!("segment-{index:04}.bin");
        std::fs::write(directory.join(&name), bytes)?;
        let duration = playlist
            .segment_durations
            .get(index)
            .copied()
            .unwrap_or(10.0);
        manifest.push_str(&format!("#EXTINF:{duration:.3},\n{name}\n"));
    }
    manifest.push_str("#EXT-X-ENDLIST\n");
    let entry = OfflineEntry {
        track: track.clone(),
        bytes: total,
        pinned: false,
    };
    std::fs::write(directory.join("track.json"), serde_json::to_vec(&entry)?)?;
    std::fs::write(directory.join("playlist.m3u8"), manifest)?;
    Ok(entry)
}

pub fn remove(track_id: u64) -> Result<()> {
    let _mutation = mutation_guard()?;
    remove_at(&root()?, track_id)
}

fn remove_at(base: &Path, track_id: u64) -> Result<()> {
    let folder = base.join(track_id.to_string());
    if folder.exists() {
        std::fs::remove_dir_all(folder)?;
    }
    Ok(())
}

fn clear_unpinned_at(base: &Path, current_track: Option<u64>) -> Result<()> {
    for entry in list_at(base)? {
        if !entry.pinned && Some(entry.track.id) != current_track { remove_at(base, entry.track.id)?; }
    }
    Ok(())
}

pub fn clear_all() -> Result<()> {
    let _mutation = mutation_guard()?;
    let base = root()?;
    for entry in std::fs::read_dir(&base)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() { continue; }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.parse::<u64>().is_ok() || name.starts_with(".partial-") {
            std::fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_guard_prevents_cleanup_during_download() {
        let download = DOWNLOAD_LOCK.try_lock().unwrap();
        assert!(mutation_guard().is_err());
        drop(download);
        assert!(mutation_guard().is_ok());
    }

    #[test]
    fn cleanup_preserves_pins_current_audio_and_unrelated_files() {
        let folder = tempfile::tempdir().unwrap();
        for mut track in crate::demo::demo_tracks().into_iter().take(3) {
            let directory = folder.path().join(track.id.to_string());
            std::fs::create_dir(&directory).unwrap();
            let id = track.id;
            track.title = "Fixture".into();
            std::fs::write(directory.join("playlist.m3u8"), "fixture").unwrap();
            // Legacy metadata has no pin field and must remain readable.
            std::fs::write(directory.join("track.json"), serde_json::to_vec(&serde_json::json!({"track":track,"bytes":123})).unwrap()).unwrap();
            assert!(directory.ends_with(id.to_string()));
        }
        std::fs::write(folder.path().join("unrelated.txt"), "keep").unwrap();
        set_pinned_at(folder.path(), 1000, true).unwrap();
        clear_unpinned_at(folder.path(), Some(1001)).unwrap();
        let entries = list_at(folder.path()).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.track.id == 1000 && entry.pinned));
        assert!(entries.iter().any(|entry| entry.track.id == 1001));
        assert!(!folder.path().join("1002").exists());
        assert!(folder.path().join("unrelated.txt").exists());
    }

    #[test]
    fn downloads_follow_like_order_before_other_saved_tracks() {
        let tracks = crate::demo::demo_tracks();
        let mut entries: Vec<_> = tracks.into_iter().take(3).map(|track| OfflineEntry { track, bytes: 1, pinned: false }).collect();
        sort_by_like_order(&mut entries, &[1002, 1000]);
        assert_eq!(entries.iter().map(|entry| entry.track.id).collect::<Vec<_>>(), vec![1002, 1000, 1001]);
    }
}
