//! Bounded artwork loading: memory budget + disk cache.
//!
//! Modelled on fastpotify's `images.rs`: entries are Pending/Ready/Failed,
//! RAM is capped (`HELD_BYTES`, oldest-used-first eviction), single files
//! are capped (`MAX_ART_BYTES`), disk writes are atomic (`.part` + rename),
//! and eviction tells egui to forget the texture so GPU memory follows.

use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Budget for the original compressed files kept by the bytes loader.
pub const HELD_BYTES: usize = 16 * 1024 * 1024;
/// Approximate RGBA size of artwork kept decoded by egui. The renderer may
/// retain both a CPU image and a driver-side upload, so keeping this well below
/// the application's total memory target leaves room for both copies.
pub const MAX_DECODED_BYTES: usize = 48 * 1024 * 1024;
/// A second bound for many tiny avatars whose byte total alone is misleading.
pub const MAX_READY_ARTWORKS: usize = 64;
/// Largest single artwork file accepted (network or disk).
pub const MAX_ART_BYTES: u64 = 8 * 1024 * 1024;
/// How often the UI asks the loader to evict (`App::logic`).
pub const EVICT_EVERY_SECS: f64 = 2.0;

/// Ask SoundCloud's image CDN for artwork large enough for detail pages.
/// API objects often return the `large` (100px) variant even when a 500px
/// source exists, which looks visibly blurred once a card is enlarged.
fn high_resolution_url(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return url.to_owned();
    };
    if !parsed
        .host_str()
        .is_some_and(|host| host == "sndcdn.com" || host.ends_with(".sndcdn.com"))
    {
        return url.to_owned();
    }
    let path = parsed.path().to_owned();
    let Some(dot) = path.rfind('.') else {
        return url.to_owned();
    };
    let Some(dash) = path[..dot].rfind('-') else {
        return url.to_owned();
    };
    let size = &path[dash + 1..dot];
    let transform = size == "large"
        || size == "crop"
        || size
            .strip_prefix('t')
            .and_then(|value| value.split_once('x'))
            .is_some_and(|(width, height)| {
                width.chars().all(|c| c.is_ascii_digit())
                    && height.chars().all(|c| c.is_ascii_digit())
            });
    if !transform || size == "t500x500" {
        return url.to_owned();
    }
    let mut upgraded = path;
    upgraded.replace_range(dash + 1..dot, "t500x500");
    parsed.set_path(&upgraded);
    parsed.to_string()
}

enum Entry {
    Pending,
    Ready {
        bytes: Arc<[u8]>,
        decoded_bytes: usize,
        last_used: Instant,
    },
    Failed(String),
}

struct Shared {
    entries: Mutex<HashMap<String, Entry>>,
    http: reqwest::Client,
    runtime: tokio::runtime::Handle,
    cache_dir: PathBuf,
}

#[derive(Clone)]
pub struct ArtLoader {
    shared: Arc<Shared>,
}

impl ArtLoader {
    pub fn new(cache_dir: PathBuf, runtime: tokio::runtime::Handle) -> Self {
        let _ = std::fs::create_dir_all(&cache_dir);
        let http = reqwest::Client::builder()
            .user_agent(concat!("fastcloud/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            shared: Arc::new(Shared {
                entries: Mutex::new(HashMap::new()),
                http,
                runtime,
                cache_dir,
            }),
        }
    }

    fn cache_path(&self, url: &str) -> PathBuf {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(url.as_bytes());
        let mut name = String::with_capacity(64);
        for byte in hasher.finalize() {
            use std::fmt::Write;
            let _ = write!(name, "{byte:02x}");
        }
        self.shared.cache_dir.join(name)
    }

    fn start(&self, ctx: egui::Context, url: String) {
        {
            let mut entries = self
                .shared
                .entries
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            entries.insert(url.clone(), Entry::Pending);
        }
        let me = self.clone();
        self.shared.runtime.spawn(async move {
            let result = me.fetch_bytes(&url).await;
            {
                let mut entries = me.shared.entries.lock().unwrap_or_else(|p| p.into_inner());
                match result {
                    Ok(bytes) => {
                        let decoded_bytes = decoded_size(&bytes);
                        entries.insert(
                            url,
                            Entry::Ready {
                                bytes: bytes.into(),
                                decoded_bytes,
                                last_used: Instant::now(),
                            },
                        );
                    }
                    Err(e) => {
                        entries.insert(url, Entry::Failed(e.to_string()));
                    }
                }
            }
            // Enforce the real decoded-image budget as soon as a download
            // lands. Waiting for the periodic UI pass allowed a fast scroll to
            // upload hundreds of covers before the first eviction.
            me.evict(&ctx);
            ctx.request_repaint();
        });
    }

    async fn fetch_bytes(&self, url: &str) -> anyhow::Result<Vec<u8>> {
        let url = high_resolution_url(url);
        // Disk cache first (blocking IO off the async runtime).
        let path = self.cache_path(&url);
        let disk_hit = tokio::task::spawn_blocking({
            let path = path.clone();
            move || std::fs::read(&path).ok()
        })
        .await
        .unwrap_or(None);
        if let Some(bytes) = disk_hit {
            if !bytes.is_empty() && bytes.len() as u64 <= MAX_ART_BYTES {
                return Ok(bytes);
            }
        }
        let resp = self.shared.http.get(&url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("artwork fetch {}: {}", resp.status(), url);
        }
        if let Some(len) = resp.content_length() {
            if len > MAX_ART_BYTES {
                anyhow::bail!("artwork too large ({len} bytes): {url}");
            }
        }
        let bytes = resp.bytes().await?.to_vec();
        if bytes.len() as u64 > MAX_ART_BYTES {
            anyhow::bail!("artwork too large ({} bytes): {url}", bytes.len());
        }
        // Atomic disk write: temp file + rename.
        let part = path.with_extension("part");
        let owned = bytes.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if std::fs::write(&part, &owned).is_ok() {
                let _ = std::fs::rename(&part, &path);
            }
        })
        .await;
        Ok(bytes)
    }

    fn lock_entries(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.shared
            .entries
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    /// Drop failed entries and the oldest-used images over any memory budget.
    /// Returns the number of entries forgotten.
    pub fn evict(&self, ctx: &egui::Context) -> usize {
        let evicted: Vec<String> = {
            let mut entries = self.lock_entries();
            let mut out = Vec::new();
            // Failed entries never recover on their own; drop them so a
            // later frame retries against a possibly fixed network/URL.
            entries.retain(|url, entry| {
                if matches!(entry, Entry::Failed(_)) {
                    out.push(url.clone());
                    false
                } else {
                    true
                }
            });
            let mut encoded_total: usize = entries
                .values()
                .map(|e| match e {
                    Entry::Ready { bytes, .. } => bytes.len(),
                    _ => 0,
                })
                .sum();
            let mut decoded_total: usize = entries
                .values()
                .map(|e| match e {
                    Entry::Ready { decoded_bytes, .. } => *decoded_bytes,
                    _ => 0,
                })
                .sum();
            let mut ready_count = entries
                .values()
                .filter(|entry| matches!(entry, Entry::Ready { .. }))
                .count();
            if encoded_total > HELD_BYTES
                || decoded_total > MAX_DECODED_BYTES
                || ready_count > MAX_READY_ARTWORKS
            {
                let mut by_age: Vec<(String, Instant)> = entries
                    .iter()
                    .filter_map(|(url, e)| match e {
                        Entry::Ready { last_used, .. } => Some((url.clone(), *last_used)),
                        _ => None,
                    })
                    .collect();
                by_age.sort_by_key(|(_, t)| *t);
                for (url, _) in by_age {
                    if encoded_total <= HELD_BYTES
                        && decoded_total <= MAX_DECODED_BYTES
                        && ready_count <= MAX_READY_ARTWORKS
                    {
                        break;
                    }
                    if let Some(Entry::Ready {
                        bytes,
                        decoded_bytes,
                        ..
                    }) = entries.remove(&url)
                    {
                        encoded_total = encoded_total.saturating_sub(bytes.len());
                        decoded_total = decoded_total.saturating_sub(decoded_bytes);
                        ready_count = ready_count.saturating_sub(1);
                        out.push(url);
                    }
                }
            }
            out
        };
        let n = evicted.len();
        for url in evicted {
            ctx.forget_image(&url);
        }
        n
    }

    pub fn byte_size(&self) -> usize {
        self.lock_entries()
            .values()
            .map(|e| match e {
                Entry::Ready { bytes, .. } => bytes.len(),
                _ => 0,
            })
            .sum()
    }

    /// Estimated bytes occupied after image decoding (RGBA8).
    pub fn decoded_byte_size(&self) -> usize {
        self.lock_entries()
            .values()
            .map(|entry| match entry {
                Entry::Ready { decoded_bytes, .. } => *decoded_bytes,
                _ => 0,
            })
            .sum()
    }

    /// Delete the on-disk artwork cache. Returns bytes freed.
    pub fn clear_disk_cache(&self) -> u64 {
        let mut freed = 0u64;
        if let Ok(files) = std::fs::read_dir(&self.shared.cache_dir) {
            for file in files.flatten() {
                let size = file.metadata().map(|m| m.len()).unwrap_or(0);
                if std::fs::remove_file(file.path()).is_ok() {
                    freed += size;
                }
            }
        }
        freed
    }
}

impl egui::load::BytesLoader for ArtLoader {
    fn id(&self) -> &str {
        "fastcloud::ArtLoader"
    }

    fn load(&self, ctx: &egui::Context, uri: &str) -> egui::load::BytesLoadResult {
        use egui::load::{Bytes, BytesPoll, LoadError};
        if !(uri.starts_with("http://") || uri.starts_with("https://")) {
            return Err(LoadError::NotSupported);
        }
        enum Next {
            Start,
            Pending,
            Ready(Arc<[u8]>),
            Failed(String),
        }
        let next = {
            let mut entries = self.lock_entries();
            match entries.get_mut(uri) {
                Some(Entry::Ready {
                    bytes, last_used, ..
                }) => {
                    *last_used = Instant::now();
                    Next::Ready(bytes.clone())
                }
                Some(Entry::Pending) => Next::Pending,
                Some(Entry::Failed(msg)) => Next::Failed(msg.clone()),
                None => Next::Start,
            }
        };
        match next {
            Next::Ready(bytes) => Ok(BytesPoll::Ready {
                size: None,
                bytes: Bytes::Shared(bytes),
                mime: None,
            }),
            Next::Pending => Ok(BytesPoll::Pending { size: None }),
            // Surface the reason; a later evict() retries the URL.
            Next::Failed(msg) => Err(LoadError::Loading(msg)),
            Next::Start => {
                self.start(ctx.clone(), uri.to_string());
                Ok(BytesPoll::Pending { size: None })
            }
        }
    }

    fn forget(&self, uri: &str) {
        self.lock_entries().remove(uri);
    }

    fn forget_all(&self) {
        self.lock_entries().clear();
    }

    fn byte_size(&self) -> usize {
        self.byte_size()
    }
}

/// Read dimensions from the encoded image header without decoding its pixels.
fn decoded_size(bytes: &[u8]) -> usize {
    image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
        .map(|(width, height)| {
            (width as usize)
                .saturating_mul(height as usize)
                .saturating_mul(4)
        })
        // If a supported loader can still decode an unusual image, reserve a
        // conservative slot instead of letting it bypass the memory budget.
        .unwrap_or(4 * 1024 * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loader() -> ArtLoader {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        // Leak the runtime handle's owner is unnecessary: Handle keeps the
        // runtime alive only while it exists; tests never spawn fetches.
        let handle = rt.handle().clone();
        std::mem::forget(rt);
        ArtLoader::new(std::env::temp_dir(), handle)
    }

    #[test]
    fn cache_path_is_stable_hex() {
        let loader = loader();
        let a = loader.cache_path("https://example.com/a.jpg");
        let b = loader.cache_path("https://example.com/a.jpg");
        assert_eq!(a, b);
        assert_eq!(a.file_name().unwrap().len(), 64);
    }

    #[test]
    fn soundcloud_thumbnails_are_upgraded_without_losing_the_query() {
        assert_eq!(
            high_resolution_url("https://i1.sndcdn.com/artworks-abc-large.jpg?token=keep"),
            "https://i1.sndcdn.com/artworks-abc-t500x500.jpg?token=keep"
        );
        assert_eq!(
            high_resolution_url("https://i1.sndcdn.com/avatars-abc-t200x200.jpg"),
            "https://i1.sndcdn.com/avatars-abc-t500x500.jpg"
        );
        assert_eq!(
            high_resolution_url("https://example.com/image-large.jpg"),
            "https://example.com/image-large.jpg"
        );
    }

    #[test]
    fn over_budget_evicts_oldest_first() {
        let loader = loader();
        {
            let mut entries = loader.lock_entries();
            // Each compressed file is 8 MiB but expands to 40 MiB of RGBA.
            let big = |age_ms: u64| Entry::Ready {
                bytes: Arc::from(vec![0u8; 8 * 1024 * 1024]),
                decoded_bytes: 40 * 1024 * 1024,
                last_used: Instant::now() - std::time::Duration::from_millis(age_ms),
            };
            entries.insert("old".into(), big(9000));
            entries.insert("mid".into(), big(5000));
            entries.insert("new".into(), big(1000));
        }
        assert!(loader.byte_size() > HELD_BYTES);
        let ctx = egui::Context::default();
        // 3 × 40 MiB = 120 MiB: "old" and "mid" go under both budgets.
        let n = loader.evict(&ctx);
        assert_eq!(n, 2);
        let entries = loader.lock_entries();
        assert!(!entries.contains_key("old"));
        assert!(!entries.contains_key("mid"));
        assert!(entries.contains_key("new"));
    }

    #[test]
    fn decoded_budget_counts_rgba_pixels_not_compressed_length() {
        let loader = loader();
        {
            let mut entries = loader.lock_entries();
            let image = |age_ms: u64| Entry::Ready {
                bytes: Arc::from(vec![0u8; 32]),
                decoded_bytes: 20 * 1024 * 1024,
                last_used: Instant::now() - std::time::Duration::from_millis(age_ms),
            };
            entries.insert("old".into(), image(3000));
            entries.insert("mid".into(), image(2000));
            entries.insert("new".into(), image(1000));
        }

        assert_eq!(loader.byte_size(), 96);
        assert!(loader.decoded_byte_size() > MAX_DECODED_BYTES);
        assert_eq!(loader.evict(&egui::Context::default()), 1);
        let entries = loader.lock_entries();
        assert!(!entries.contains_key("old"));
        assert!(entries.contains_key("mid"));
        assert!(entries.contains_key("new"));
    }
}
