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
}

pub fn root() -> Result<PathBuf> {
    let path = crate::config::app_paths()?.root.join("offline");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn list() -> Result<Vec<OfflineEntry>> {
    let mut entries = Vec::new();
    for folder in std::fs::read_dir(root()?)? {
        let path = folder?.path();
        if !path.is_dir() || !path.join("playlist.m3u8").exists() {
            continue;
        }
        if let Ok(bytes) = std::fs::read(path.join("track.json"))
            && let Ok(entry) = serde_json::from_slice::<OfflineEntry>(&bytes)
        {
            entries.push(entry);
        }
    }
    let order = like_order()?;
    sort_by_like_order(&mut entries, &order);
    Ok(entries)
}

fn like_order() -> Result<Vec<u64>> {
    let path = root()?.join("like-order.json");
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

pub async fn download(client: Arc<api::ApiClient>, track: Track) -> Result<OfflineEntry> {
    let root = root()?;
    let target = root.join(track.id.to_string());
    if target.join("playlist.m3u8").exists() {
        let bytes = std::fs::read(target.join("track.json"))?;
        return Ok(serde_json::from_slice(&bytes)?);
    }
    let streams = api::endpoints::track_streams(&client, &track.urn()).await?;
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
    let result = download_into(&downloader, &playlist, &track, &temporary).await;
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
) -> Result<OfflineEntry> {
    let mut manifest = String::from("#EXTM3U\n#EXT-X-VERSION:3\n");
    let mut total = 0u64;
    if let Some(url) = &playlist.init_uri {
        let bytes = downloader.init_segment(&track.urn(), url).await?;
        total += bytes.len() as u64;
        ensure!(total <= 2 * 1024 * 1024 * 1024, "Track is too large");
        std::fs::write(directory.join("init.bin"), bytes)?;
        manifest.push_str("#EXT-X-MAP:URI=\"init.bin\"\n");
    }
    for (index, url) in playlist.segments.iter().enumerate() {
        let bytes = downloader.segment(&track.urn(), url).await?;
        total += bytes.len() as u64;
        ensure!(total <= 2 * 1024 * 1024 * 1024, "Track is too large");
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
    };
    std::fs::write(directory.join("track.json"), serde_json::to_vec(&entry)?)?;
    std::fs::write(directory.join("playlist.m3u8"), manifest)?;
    Ok(entry)
}

pub fn remove(track_id: u64) -> Result<()> {
    let folder = root()?.join(track_id.to_string());
    if folder.exists() {
        std::fs::remove_dir_all(folder)?;
    }
    Ok(())
}

pub fn clear_all() -> Result<()> {
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
    fn downloads_follow_like_order_before_other_saved_tracks() {
        let tracks = crate::demo::demo_tracks();
        let mut entries: Vec<_> = tracks.into_iter().take(3).map(|track| OfflineEntry { track, bytes: 1 }).collect();
        sort_by_like_order(&mut entries, &[1002, 1000]);
        assert_eq!(entries.iter().map(|entry| entry.track.id).collect::<Vec<_>>(), vec![1002, 1000, 1001]);
    }
}
