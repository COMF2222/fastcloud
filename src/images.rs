//! Bounded artwork loading: memory budget + disk cache.
//!
//! Modelled on fastpotify's `images.rs`: entries are Pending/Ready/Failed,
//! RAM is capped (`HELD_BYTES`, oldest-used-first eviction), single files
//! are capped (`MAX_ART_BYTES`), disk writes are atomic (`.part` + rename),
//! and eviction tells egui to forget the texture so GPU memory follows.

use anyhow::Context as _;
use eframe::egui;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
/// Source wallpapers may be full-resolution camera files. They have their own
/// limit because the single viewport texture is not part of the artwork LRU.
pub const MAX_WALLPAPER_SOURCE_BYTES: u64 = 40 * 1024 * 1024;
/// How often the UI asks the loader to evict (`App::logic`).
pub const EVICT_EVERY_SECS: f64 = 2.0;
/// Broken CDN URLs should not alternate between retry spinners and a missing
/// cover every eviction pass. Retry only after a useful backoff.
const FAILED_RETRY_AFTER: Duration = Duration::from_secs(120);
const MAX_FAILED_ENTRIES: usize = 128;
const WALLPAPER_SCHEME: &str = "fastcloud-wallpaper";
const ART_BUDGET_ID: &str = "fastcloud::art-budget";
const VISIBLE_ART_ID: &str = "fastcloud::visible-artwork";

/// Remember that a widget painted this URI in the current frame. The bytes
/// loader is not polled again once egui has a texture, so its own LRU clock
/// cannot tell whether a cached cover is still on screen.
pub fn mark_artwork_visible(ctx: &egui::Context, uri: &str) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|data| {
        let id = egui::Id::new(VISIBLE_ART_ID);
        let mut visible = data
            .get_temp::<HashMap<String, u64>>(id)
            .unwrap_or_default();
        visible.insert(uri.to_owned(), frame);
        if visible.len() > 512 {
            visible.retain(|_, seen| frame.saturating_sub(*seen) <= 2);
        }
        data.insert_temp(id, visible);
    });
}

/// Runtime artwork limits selected by the user-facing memory profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtBudget {
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
    pub ready_artworks: usize,
    pub card_pixels: u32,
    pub hero_pixels: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtStats {
    pub entries: usize,
    pub pending: usize,
    pub ready: usize,
    pub failed: usize,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
}

impl ArtBudget {
    pub const ECO: Self = Self {
        encoded_bytes: 4 * 1024 * 1024,
        decoded_bytes: 24 * 1024 * 1024,
        ready_artworks: 32,
        card_pixels: 160,
        hero_pixels: 320,
    };
    pub const BALANCED: Self = Self {
        encoded_bytes: 8 * 1024 * 1024,
        decoded_bytes: 32 * 1024 * 1024,
        ready_artworks: 48,
        card_pixels: 200,
        hero_pixels: 500,
    };
    pub const QUALITY: Self = Self {
        encoded_bytes: HELD_BYTES,
        decoded_bytes: MAX_DECODED_BYTES,
        ready_artworks: MAX_READY_ARTWORKS,
        card_pixels: 320,
        hero_pixels: 500,
    };
}

impl From<crate::config::MemoryProfile> for ArtBudget {
    fn from(profile: crate::config::MemoryProfile) -> Self {
        match profile {
            crate::config::MemoryProfile::Eco => Self::ECO,
            crate::config::MemoryProfile::Balanced => Self::BALANCED,
            crate::config::MemoryProfile::Quality => Self::QUALITY,
        }
    }
}

fn is_wallpaper_uri(uri: &str) -> bool {
    uri.starts_with("fastcloud-wallpaper://")
}

pub fn wallpaper_uri(source: &str, blur: u8) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("source", source)
        .append_pair("blur", &blur.min(40).to_string())
        .finish();
    format!("{WALLPAPER_SCHEME}://render?{query}")
}

fn parse_wallpaper_uri(uri: &str) -> anyhow::Result<(String, u8)> {
    let parsed = url::Url::parse(uri).context("parse Fastcloud wallpaper URI")?;
    if parsed.scheme() != WALLPAPER_SCHEME {
        anyhow::bail!("unsupported wallpaper URI scheme")
    }
    let mut source = None;
    let mut blur = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "source" => source = Some(value.into_owned()),
            "blur" => blur = value.parse::<u8>().ok(),
            _ => {}
        }
    }
    let source = source.context("wallpaper source is missing")?;
    let source_url = url::Url::parse(&source).context("parse wallpaper source URI")?;
    if !matches!(source_url.scheme(), "http" | "https" | "file") {
        anyhow::bail!("unsupported wallpaper source URI scheme")
    }
    Ok((source, blur.unwrap_or_default().min(40)))
}

/// Ask SoundCloud's image CDN for artwork large enough for detail pages.
/// API objects often return the `large` (100px) variant even when a 500px
/// source exists, which looks visibly blurred once a card is enlarged.
fn soundcloud_artwork_url(url: &str, target_pixels: u32) -> Cow<'_, str> {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return Cow::Borrowed(url);
    };
    if !parsed
        .host_str()
        .is_some_and(|host| host == "sndcdn.com" || host.ends_with(".sndcdn.com"))
    {
        return Cow::Borrowed(url);
    }
    let path = parsed.path().to_owned();
    let Some(dot) = path.rfind('.') else {
        return Cow::Borrowed(url);
    };
    let Some(dash) = path[..dot].rfind('-') else {
        return Cow::Borrowed(url);
    };
    let size = &path[dash + 1..dot];
    let transform = size == "large"
        || size == "crop"
        || size.strip_prefix('t').is_some_and(|value| {
            value.split_once('x').is_some_and(|(width, height)| {
                width.chars().all(|c| c.is_ascii_digit())
                    && height.chars().all(|c| c.is_ascii_digit())
            })
        });
    let target_pixels = match target_pixels {
        0..=50 => 50,
        51..=100 => 100,
        101..=160 => 160,
        161..=200 => 200,
        201..=320 => 320,
        _ => 500,
    };
    let target = format!("t{target_pixels}x{target_pixels}");
    if !transform || size == target {
        return Cow::Borrowed(url);
    }
    let mut upgraded = path;
    upgraded.replace_range(dash + 1..dot, &target);
    parsed.set_path(&upgraded);
    Cow::Owned(parsed.to_string())
}

/// Resolve the smallest SoundCloud CDN variant useful for this UI slot while
/// respecting the active memory profile's card/hero cap.
pub fn artwork_url_for_ui<'a>(
    ctx: &egui::Context,
    url: &'a str,
    logical_size: f32,
) -> Cow<'a, str> {
    let budget = ctx
        .data(|data| data.get_temp::<ArtBudget>(egui::Id::new(ART_BUDGET_ID)))
        .unwrap_or(ArtBudget::BALANCED);
    let cap = if logical_size >= 240.0 {
        budget.hero_pixels
    } else {
        budget.card_pixels
    };
    let requested = (logical_size.max(1.0) * ctx.pixels_per_point()).ceil() as u32;
    soundcloud_artwork_url(url, requested.min(cap))
}

enum Entry {
    Pending,
    Ready {
        bytes: Arc<[u8]>,
        decoded_bytes: usize,
        dimensions: (u32, u32),
        dominant_rgb: Option<[u8; 3]>,
        last_used: Instant,
    },
    Failed {
        message: String,
        failed_at: Instant,
    },
}

struct Shared {
    entries: Mutex<HashMap<String, Entry>>,
    budget: Mutex<ArtBudget>,
    download_slots: Arc<tokio::sync::Semaphore>,
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
                budget: Mutex::new(ArtBudget::BALANCED),
                download_slots: Arc::new(tokio::sync::Semaphore::new(6)),
                http,
                runtime,
                cache_dir,
            }),
        }
    }

    pub fn set_profile(&self, profile: crate::config::MemoryProfile) {
        *self
            .shared
            .budget
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = profile.into();
    }

    pub fn budget(&self) -> ArtBudget {
        *self
            .shared
            .budget
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Make the profile visible to widgets without coupling every card to App.
    pub fn install_context_budget(&self, ctx: &egui::Context) {
        ctx.data_mut(|data| data.insert_temp(egui::Id::new(ART_BUDGET_ID), self.budget()));
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
            let _permit = me
                .shared
                .download_slots
                .clone()
                .acquire_owned()
                .await
                .expect("artwork semaphore is never closed");
            let result = me.fetch_bytes(&url).await;
            {
                let mut entries = me.shared.entries.lock().unwrap_or_else(|p| p.into_inner());
                match result {
                    Ok(bytes) => {
                        let dimensions = image_dimensions(&bytes).unwrap_or((1024, 1024));
                        let decoded_bytes = decoded_size(dimensions);
                        let dominant_rgb = dominant_rgb(&bytes);
                        entries.insert(
                            url,
                            Entry::Ready {
                                bytes: bytes.into(),
                                decoded_bytes,
                                dimensions,
                                dominant_rgb,
                                last_used: Instant::now(),
                            },
                        );
                    }
                    Err(e) => {
                        entries.insert(
                            url,
                            Entry::Failed {
                                message: e.to_string(),
                                failed_at: Instant::now(),
                            },
                        );
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
        if is_wallpaper_uri(url) {
            let (source, blur) = parse_wallpaper_uri(url)?;
            let bytes = self
                .fetch_source_bytes_capped(&source, MAX_WALLPAPER_SOURCE_BYTES)
                .await?;
            return tokio::task::spawn_blocking(move || process_wallpaper(bytes, blur))
                .await
                .context("wallpaper blur worker stopped")?;
        }
        self.fetch_source_bytes(url).await
    }

    async fn fetch_source_bytes(&self, url: &str) -> anyhow::Result<Vec<u8>> {
        self.fetch_source_bytes_capped(url, MAX_ART_BYTES).await
    }

    async fn fetch_source_bytes_capped(
        &self,
        url: &str,
        max_bytes: u64,
    ) -> anyhow::Result<Vec<u8>> {
        if url.starts_with("file://") {
            let parsed = url::Url::parse(url).context("parse wallpaper file URI")?;
            let path = parsed
                .to_file_path()
                .map_err(|_| anyhow::anyhow!("invalid wallpaper file URI"))?;
            let bytes = tokio::task::spawn_blocking(move || std::fs::read(path)).await??;
            if bytes.len() as u64 > max_bytes {
                anyhow::bail!("wallpaper too large ({} bytes)", bytes.len());
            }
            return Ok(bytes);
        }
        let budget = self.budget();
        let url = soundcloud_artwork_url(url, budget.hero_pixels);
        // Disk cache first (blocking IO off the async runtime).
        let path = self.cache_path(url.as_ref());
        let disk_hit = tokio::task::spawn_blocking({
            let path = path.clone();
            move || std::fs::read(&path).ok()
        })
        .await
        .unwrap_or(None);
        if let Some(bytes) = disk_hit
            && !bytes.is_empty()
            && bytes.len() as u64 <= max_bytes
        {
            return Ok(bytes);
        }
        let resp = self.shared.http.get(url.as_ref()).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("artwork fetch {}: {}", resp.status(), url);
        }
        if let Some(len) = resp.content_length()
            && len > max_bytes
        {
            anyhow::bail!("artwork too large ({len} bytes): {url}");
        }
        let bytes = resp.bytes().await?.to_vec();
        if bytes.len() as u64 > max_bytes {
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

    /// Bound failed URLs and drop the oldest-used images over memory budgets.
    /// Returns the number of entries forgotten.
    pub fn evict(&self, ctx: &egui::Context) -> usize {
        let frame = ctx.cumulative_frame_nr();
        let visible = ctx.data(|data| {
            data.get_temp::<HashMap<String, u64>>(egui::Id::new(VISIBLE_ART_ID))
                .unwrap_or_default()
        });
        let budget = self.budget();
        let evicted: Vec<String> = {
            let mut entries = self.lock_entries();
            let mut out = Vec::new();
            let mut failed: Vec<_> = entries
                .iter()
                .filter_map(|(url, entry)| match entry {
                    Entry::Failed { failed_at, .. } => Some((url.clone(), *failed_at)),
                    _ => None,
                })
                .collect();
            if failed.len() > MAX_FAILED_ENTRIES {
                failed.sort_by_key(|(_, failed_at)| *failed_at);
                for (url, _) in failed.into_iter().take(
                    entries
                        .values()
                        .filter(|entry| matches!(entry, Entry::Failed { .. }))
                        .count()
                        - MAX_FAILED_ENTRIES,
                ) {
                    entries.remove(&url);
                    out.push(url);
                }
            }
            let mut encoded_total: usize = entries
                .iter()
                .filter(|(url, _)| !is_wallpaper_uri(url))
                .map(|(_, e)| match e {
                    Entry::Ready { bytes, .. } => bytes.len(),
                    _ => 0,
                })
                .sum();
            let mut decoded_total: usize = entries
                .iter()
                .filter(|(url, _)| !is_wallpaper_uri(url))
                .map(|(_, e)| match e {
                    Entry::Ready { decoded_bytes, .. } => *decoded_bytes,
                    _ => 0,
                })
                .sum();
            let mut ready_count = entries
                .iter()
                .filter(|(url, entry)| {
                    !is_wallpaper_uri(url) && matches!(entry, Entry::Ready { .. })
                })
                .count();
            if encoded_total > budget.encoded_bytes
                || decoded_total > budget.decoded_bytes
                || ready_count > budget.ready_artworks
            {
                let mut by_age: Vec<(String, Instant)> = entries
                    .iter()
                    .filter_map(|(url, e)| match e {
                        Entry::Ready { last_used, .. }
                            if !is_wallpaper_uri(url)
                                && !visible
                                    .get(url)
                                    .is_some_and(|seen| frame.saturating_sub(*seen) <= 1) =>
                        {
                            Some((url.clone(), *last_used))
                        }
                        _ => None,
                    })
                    .collect();
                by_age.sort_by_key(|(_, t)| *t);
                for (url, _) in by_age {
                    if encoded_total <= budget.encoded_bytes
                        && decoded_total <= budget.decoded_bytes
                        && ready_count <= budget.ready_artworks
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

    pub fn stats(&self) -> ArtStats {
        let entries = self.lock_entries();
        let mut stats = ArtStats {
            entries: entries.len(),
            pending: 0,
            ready: 0,
            failed: 0,
            encoded_bytes: 0,
            decoded_bytes: 0,
        };
        for entry in entries.values() {
            match entry {
                Entry::Pending => stats.pending += 1,
                Entry::Ready {
                    bytes,
                    decoded_bytes,
                    ..
                } => {
                    stats.ready += 1;
                    stats.encoded_bytes += bytes.len();
                    stats.decoded_bytes += decoded_bytes;
                }
                Entry::Failed { .. } => stats.failed += 1,
            }
        }
        stats
    }

    /// Encoded image dimensions once the loader has decoded its header.
    pub fn dimensions(&self, uri: &str) -> Option<(u32, u32)> {
        let entries = self.lock_entries();
        match entries.get(uri) {
            Some(Entry::Ready { dimensions, .. }) => Some(*dimensions),
            _ => None,
        }
    }

    /// Average artwork colour, computed off the UI thread when the bytes land.
    pub fn dominant_color_for_ui(
        &self,
        ctx: &egui::Context,
        uri: &str,
        logical_size: f32,
    ) -> Option<egui::Color32> {
        let resolved = artwork_url_for_ui(ctx, uri, logical_size);
        let entries = self.lock_entries();
        match entries.get(resolved.as_ref()) {
            Some(Entry::Ready {
                dominant_rgb: Some([r, g, b]),
                ..
            }) => Some(egui::Color32::from_rgb(*r, *g, *b)),
            _ => None,
        }
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
        if !(uri.starts_with("http://")
            || uri.starts_with("https://")
            || uri.starts_with("file://"))
            && !uri.starts_with("fastcloud-wallpaper://")
        {
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
                Some(Entry::Failed { message, failed_at }) => {
                    if failed_at.elapsed() >= FAILED_RETRY_AFTER {
                        Next::Start
                    } else {
                        Next::Failed(message.clone())
                    }
                }
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

fn process_wallpaper(bytes: Vec<u8>, radius: u8) -> anyhow::Result<Vec<u8>> {
    let image = image::load_from_memory(&bytes).context("decode wallpaper")?;
    let source = (image.width().max(1), image.height().max(1));
    let effective_blur = wallpaper_blur(radius, source);

    // Keep the source bytes untouched when no processing is needed. The old
    // path upscaled every small image to 1080p and JPEG-encoded it again,
    // making block edges more obvious without creating any real detail.
    if effective_blur == 0.0
        && wallpaper_dimensions(source) == source
        && bytes.len() as u64 <= MAX_WALLPAPER_SOURCE_BYTES
    {
        return Ok(bytes);
    }

    let (target_width, target_height) = wallpaper_dimensions(source);
    let mut processed = if (target_width, target_height) == source {
        image
    } else {
        image.resize_exact(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        )
    };
    if effective_blur > 0.0 {
        processed = processed.blur(effective_blur);
    }
    let mut output = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 95)
        .encode_image(&processed)
        .context("encode prepared wallpaper")?;
    if output.len() as u64 > MAX_WALLPAPER_SOURCE_BYTES {
        anyhow::bail!("prepared wallpaper too large ({} bytes)", output.len());
    }
    Ok(output)
}

/// Downscale only oversized wallpaper sources. Upscaling belongs to the GPU
/// at the final viewport size; doing it here wastes memory and magnifies JPEG
/// artefacts before the image reaches the screen.
fn wallpaper_dimensions((width, height): (u32, u32)) -> (u32, u32) {
    const MAX_PIXELS: f32 = 3_840.0 * 2_160.0;
    let pixels = width.max(1) as f32 * height.max(1) as f32;
    if pixels <= MAX_PIXELS {
        return (width.max(1), height.max(1));
    }
    let scale = (MAX_PIXELS / pixels).sqrt();
    (
        (width as f32 * scale).round().max(1.0) as u32,
        (height as f32 * scale).round().max(1.0) as u32,
    )
}

fn wallpaper_blur(user_radius: u8, _source: (u32, u32)) -> f32 {
    f32::from(user_radius.min(40))
}

/// Read dimensions from the encoded image header without decoding its pixels.
fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
}

fn dominant_rgb(bytes: &[u8]) -> Option<[u8; 3]> {
    let sample = image::load_from_memory(bytes)
        .ok()?
        .thumbnail_exact(1, 1)
        .to_rgb8();
    let pixel = sample.get_pixel(0, 0).0;
    Some(pixel)
}

fn decoded_size((width, height): (u32, u32)) -> usize {
    (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4)
}

/// UV rectangle for CSS-like `background-size: cover` without distortion.
pub fn cover_uv(source: (u32, u32), target: egui::Vec2) -> egui::Rect {
    let source_aspect = source.0.max(1) as f32 / source.1.max(1) as f32;
    let target_aspect = target.x.max(1.0) / target.y.max(1.0);
    if source_aspect > target_aspect {
        let visible = (target_aspect / source_aspect).clamp(0.0, 1.0);
        let edge = (1.0 - visible) * 0.5;
        egui::Rect::from_min_max(egui::pos2(edge, 0.0), egui::pos2(1.0 - edge, 1.0))
    } else {
        let visible = (source_aspect / target_aspect).clamp(0.0, 1.0);
        let edge = (1.0 - visible) * 0.5;
        egui::Rect::from_min_max(egui::pos2(0.0, edge), egui::pos2(1.0, 1.0 - edge))
    }
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
    fn soundcloud_thumbnails_use_the_requested_variant_without_losing_the_query() {
        assert_eq!(
            soundcloud_artwork_url(
                "https://i1.sndcdn.com/artworks-abc-large.jpg?token=keep",
                500
            ),
            "https://i1.sndcdn.com/artworks-abc-t500x500.jpg?token=keep"
        );
        assert_eq!(
            soundcloud_artwork_url("https://i1.sndcdn.com/avatars-abc-t200x200.jpg", 160),
            "https://i1.sndcdn.com/avatars-abc-t160x160.jpg"
        );
        assert_eq!(
            soundcloud_artwork_url("https://example.com/image-large.jpg", 500),
            "https://example.com/image-large.jpg"
        );
    }

    #[test]
    fn memory_profiles_have_monotonic_budgets() {
        const {
            assert!(ArtBudget::ECO.encoded_bytes < ArtBudget::BALANCED.encoded_bytes);
            assert!(ArtBudget::BALANCED.encoded_bytes < ArtBudget::QUALITY.encoded_bytes);
            assert!(ArtBudget::ECO.decoded_bytes < ArtBudget::BALANCED.decoded_bytes);
            assert!(ArtBudget::BALANCED.decoded_bytes < ArtBudget::QUALITY.decoded_bytes);
            assert!(ArtBudget::ECO.ready_artworks < ArtBudget::BALANCED.ready_artworks);
            assert!(ArtBudget::BALANCED.ready_artworks < ArtBudget::QUALITY.ready_artworks);
        }
    }

    #[test]
    fn wallpaper_uri_round_trips_a_local_path_and_blur_radius() {
        let source = "file:///C:/Music & Art/wallpaper.png";
        let uri = wallpaper_uri(source, 18);

        let (decoded_source, decoded_blur) = parse_wallpaper_uri(&uri).unwrap();

        assert_eq!((decoded_source.as_str(), decoded_blur), (source, 18));
    }

    #[test]
    fn cover_uv_crops_without_stretching() {
        let wide = cover_uv((1920, 1080), egui::vec2(1000.0, 1000.0));
        assert!(wide.min.x > 0.0);
        assert_eq!((wide.min.y, wide.max.y), (0.0, 1.0));

        let tall = cover_uv((1080, 1920), egui::vec2(1600.0, 900.0));
        assert!(tall.min.y > 0.0);
        assert_eq!((tall.min.x, tall.max.x), (0.0, 1.0));
    }

    #[test]
    fn wallpaper_preparation_never_upscales_a_small_source() {
        assert_eq!(wallpaper_dimensions((640, 480)), (640, 480));
    }

    #[test]
    fn low_resolution_wallpaper_stays_crisp_when_blur_is_zero() {
        assert_eq!(wallpaper_blur(0, (640, 480)), 0.0);
    }

    #[test]
    fn large_wallpaper_keeps_the_users_blur_choice() {
        assert_eq!(wallpaper_blur(12, (3840, 2160)), 12.0);
    }

    #[test]
    fn donor_wallpaper_blur_supports_forty_pixels() {
        let uri = wallpaper_uri("file:///C:/wallpaper.jpg", 40);

        assert_eq!(parse_wallpaper_uri(&uri).unwrap().1, 40);
    }

    #[test]
    fn over_budget_evicts_oldest_first() {
        let loader = loader();
        loader.set_profile(crate::config::MemoryProfile::Quality);
        {
            let mut entries = loader.lock_entries();
            // Each compressed file is 8 MiB but expands to 40 MiB of RGBA.
            let big = |age_ms: u64| Entry::Ready {
                bytes: Arc::from(vec![0u8; 8 * 1024 * 1024]),
                decoded_bytes: 40 * 1024 * 1024,
                dimensions: (3200, 3200),
                dominant_rgb: None,
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
    fn visible_artwork_is_not_evicted_while_scrolling() {
        let loader = loader();
        loader.set_profile(crate::config::MemoryProfile::Eco);
        {
            let mut entries = loader.lock_entries();
            for (name, age_ms) in [("visible", 9_000), ("offscreen", 5_000)] {
                entries.insert(
                    name.into(),
                    Entry::Ready {
                        bytes: Arc::from(vec![0u8; 3 * 1024 * 1024]),
                        decoded_bytes: 16 * 1024 * 1024,
                        dimensions: (2_000, 2_000),
                        dominant_rgb: None,
                        last_used: Instant::now() - std::time::Duration::from_millis(age_ms),
                    },
                );
            }
        }
        let ctx = egui::Context::default();
        mark_artwork_visible(&ctx, "visible");

        loader.evict(&ctx);

        let entries = loader.lock_entries();
        assert!(entries.contains_key("visible"));
        assert!(!entries.contains_key("offscreen"));
    }

    #[test]
    fn periodic_eviction_keeps_recent_failures_stable() {
        let loader = loader();
        loader.lock_entries().insert(
            "broken-cover".into(),
            Entry::Failed {
                message: "404".into(),
                failed_at: Instant::now(),
            },
        );

        assert_eq!(loader.evict(&egui::Context::default()), 0);
        assert!(loader.lock_entries().contains_key("broken-cover"));
    }

    #[test]
    fn decoded_budget_counts_rgba_pixels_not_compressed_length() {
        let loader = loader();
        loader.set_profile(crate::config::MemoryProfile::Quality);
        {
            let mut entries = loader.lock_entries();
            let image = |age_ms: u64| Entry::Ready {
                bytes: Arc::from(vec![0u8; 32]),
                decoded_bytes: 20 * 1024 * 1024,
                dimensions: (2560, 2048),
                dominant_rgb: None,
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

    #[test]
    fn decoded_size_does_not_charge_every_small_cover_as_four_megabytes() {
        assert_eq!(decoded_size((500, 500)), 1_000_000);
        assert_eq!(decoded_size((100, 100)), 40_000);
    }

    #[test]
    fn wallpaper_does_not_compete_with_track_artwork_budget() {
        let loader = loader();
        let wallpaper = wallpaper_uri("file:///C:/wallpaper.jpg", 8);
        {
            let mut entries = loader.lock_entries();
            entries.insert(
                wallpaper.clone(),
                Entry::Ready {
                    bytes: Arc::from(vec![0u8; 1024]),
                    decoded_bytes: 32 * 1024 * 1024,
                    dimensions: (3840, 2160),
                    dominant_rgb: None,
                    last_used: Instant::now() - std::time::Duration::from_secs(30),
                },
            );
            entries.insert(
                "track-art".into(),
                Entry::Ready {
                    bytes: Arc::from(vec![0u8; 1024]),
                    decoded_bytes: 24 * 1024 * 1024,
                    dimensions: (2500, 2500),
                    dominant_rgb: None,
                    last_used: Instant::now(),
                },
            );
        }

        loader.evict(&egui::Context::default());

        assert!(loader.lock_entries().contains_key(&wallpaper));
    }
}
