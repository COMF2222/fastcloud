#![allow(dead_code)]

use anyhow::Result;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Content-addressed on-disk cache for HLS segments and full previews.
///
/// Layout:
///
/// ```text
/// <root>/<track_urn>/<hash>.seg  - individual segments
/// <root>/lock                    - lightweight admission lock
/// ```
pub struct AudioCache {
    root: PathBuf,
    max_bytes: u64,
}

impl AudioCache {
    pub fn new(root: PathBuf, max_bytes: u64) -> Result<Self> {
        std::fs::create_dir_all(&root)?;
        Ok(Self { root, max_bytes })
    }

    pub fn disabled() -> Self {
        Self {
            root: PathBuf::new(),
            max_bytes: 0,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.max_bytes > 0
    }

    fn hash_of(url: &str) -> String {
        use sha2::{Digest, Sha256};
        // CDN signatures expire and change on every resolve. The underlying
        // media path does not, so excluding query/fragment turns the disk
        // cache into a cache across launches instead of a pile of duplicates.
        let stable = url::Url::parse(url)
            .map(|mut parsed| {
                parsed.set_query(None);
                parsed.set_fragment(None);
                parsed.to_string()
            })
            .unwrap_or_else(|_| url.to_owned());
        let d = Sha256::digest(stable.as_bytes());
        d.iter().take(16).map(|b| format!("{b:02x}")).collect()
    }

    fn track_dir(&self, track_urn: &str) -> PathBuf {
        let safe = track_urn.replace([':', '/', '\\'], "_");
        self.root.join(safe)
    }

    pub fn get_segment(&self, track_urn: &str, url: &str) -> Option<Vec<u8>> {
        if !self.is_enabled() {
            return None;
        }
        let path = self
            .track_dir(track_urn)
            .join(format!("{}.seg", Self::hash_of(url)));
        let bytes = std::fs::read(&path)
            .ok()
            .filter(|bytes| !bytes.is_empty())?;
        if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
            let _ = file.set_modified(SystemTime::now());
        }
        Some(bytes)
    }

    pub fn put_segment(&self, track_urn: &str, url: &str, data: &[u8]) {
        if !self.is_enabled() {
            return;
        }
        let dir = self.track_dir(track_urn);
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        if data.is_empty() {
            return;
        }
        let path = dir.join(format!("{}.seg", Self::hash_of(url)));
        let temporary = path.with_extension("seg.part");
        let written = std::fs::File::create(&temporary).and_then(|mut file| {
            file.write_all(data)?;
            file.sync_all()
        });
        if written.is_ok() {
            let _ = std::fs::rename(&temporary, path);
        } else {
            let _ = std::fs::remove_file(temporary);
        }
    }

    /// Remove all cached segments for a track.
    pub fn evict_track(&self, track_urn: &str) {
        if !self.is_enabled() {
            return;
        }
        let _ = std::fs::remove_dir_all(self.track_dir(track_urn));
    }

    /// Evict LRU segments until total size is below `max_bytes`.
    pub fn enforce_quota(&self) {
        if !self.is_enabled() {
            return;
        }
        let Ok(entries) = self.collect_entries() else {
            return;
        };
        let mut total: u64 = entries.iter().map(|(_, len)| len).sum();
        if total <= self.max_bytes {
            return;
        }
        for (path, len) in entries {
            if total <= self.max_bytes {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(len);
            }
        }
    }

    fn collect_entries(&self) -> std::io::Result<Vec<(PathBuf, u64)>> {
        let mut out = Vec::new();
        for dir in std::fs::read_dir(&self.root)? {
            let dir = dir?.path();
            if !dir.is_dir() {
                continue;
            }
            for f in std::fs::read_dir(&dir)? {
                let f = f?.path();
                let Ok(meta) = std::fs::metadata(&f) else {
                    continue;
                };
                let Ok(modified) = meta.modified() else {
                    continue;
                };
                out.push((f, meta.len(), modified));
            }
        }
        out.sort_by_key(|(_, _, m)| *m);
        Ok(out.into_iter().map(|(p, l, _)| (p, l)).collect())
    }

    pub fn total_size(&self) -> u64 {
        self.collect_entries()
            .map(|e| e.iter().map(|(_, l)| l).sum())
            .unwrap_or(0)
    }
}

/// Run the quota enforcer periodically.
pub async fn quota_task(cache: std::sync::Arc<AudioCache>, interval: Duration) {
    loop {
        tokio::time::sleep(interval).await;
        let cache = cache.clone();
        if let Err(error) = tokio::task::spawn_blocking(move || cache.enforce_quota()).await {
            log::warn!("audio cache quota task failed: {error}");
        }
    }
}

#[allow(dead_code)]
fn _assert_send() {
    fn is_send<T: Send>() {}
    is_send::<SystemTime>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiring_cdn_signatures_share_one_cache_key() {
        let first = AudioCache::hash_of("https://cdn.example/media/42?a=old&token=one");
        let second = AudioCache::hash_of("https://cdn.example/media/42?a=new&token=two#x");
        assert_eq!(first, second);
    }

    #[test]
    fn empty_or_partial_writes_never_become_cache_hits() {
        let dir = tempfile::tempdir().unwrap();
        let cache = AudioCache::new(dir.path().to_path_buf(), 1024).unwrap();
        cache.put_segment("track", "https://cdn.example/empty", &[]);
        assert!(
            cache
                .get_segment("track", "https://cdn.example/empty")
                .is_none()
        );
    }
}
