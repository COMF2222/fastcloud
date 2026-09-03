#![allow(dead_code)]

use anyhow::Result;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Content-addressed on-disk cache for HLS segments and full previews.
///
/// Layout:
///   <root>/<track_urn>/<hash>.seg      — individual segments
///   <root>/lock                        — lightweight admission lock
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
        let d = Sha256::digest(url.as_bytes());
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
        std::fs::read(
            self.track_dir(track_urn)
                .join(format!("{}.seg", Self::hash_of(url))),
        )
        .ok()
    }

    pub fn put_segment(&self, track_urn: &str, url: &str, data: &[u8]) {
        if !self.is_enabled() {
            return;
        }
        let dir = self.track_dir(track_urn);
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let _ = std::fs::write(dir.join(format!("{}.seg", Self::hash_of(url))), data);
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
        cache.enforce_quota();
    }
}

#[allow(dead_code)]
fn _assert_send() {
    fn is_send<T: Send>() {}
    is_send::<SystemTime>();
}
