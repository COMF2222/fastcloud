use base64::Engine as _;
use sha2::Digest as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_CACHE_BYTES: u64 = 128 * 1024 * 1024;
static CACHE_WRITES: AtomicUsize = AtomicUsize::new(0);

pub struct ArtworkCache {
    root: PathBuf,
    http: reqwest::Client,
    download_slots: tokio::sync::Semaphore,
}

impl ArtworkCache {
    pub fn new(root: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            download_slots: tokio::sync::Semaphore::new(6),
        })
    }

    pub async fn data_url(&self, raw: String, relay: Option<(String, String)>) -> Result<String, String> {
        let url = url::Url::parse(&raw).map_err(|_| "Invalid image URL")?;
        let host = url.host_str().ok_or("Image host is missing")?;
        if url.scheme() != "https" || (host != "sndcdn.com" && !host.ends_with(".sndcdn.com")) {
            return Err("Only HTTPS SoundCloud artwork is allowed".into());
        }
        let path = cache_path(&self.root, url.as_str());
        let read_path = path.clone();
        if let Ok(Some(bytes)) = tokio::task::spawn_blocking(move || {
            let size = std::fs::metadata(&read_path).ok()?.len();
            if size == 0 || size > MAX_IMAGE_BYTES as u64 { return None }
            std::fs::read(read_path).ok()
        }).await {
            if let Some(mime) = image_mime(&bytes) {
                return Ok(as_data_url(mime, &bytes));
            }
        }

        // Limit simultaneous downloads after checking the disk cache. This also
        // bounds the number of partially buffered responses during a fast scroll.
        let _permit = self.download_slots.acquire().await.map_err(|error| error.to_string())?;
        let request = if let Some((server, token)) = relay {
            self.http.post(format!("{server}/v1/soundcloud/artwork"))
                .header(reqwest::header::AUTHORIZATION, format!("OAuth {token}"))
                .json(&serde_json::json!({"url": url.as_str()}))
        } else { self.http.get(url) };
        let mut response = request.send().await.map_err(|error| error.to_string())?
            .error_for_status().map_err(|error| error.to_string())?;
        if response.content_length().is_some_and(|size| size > MAX_IMAGE_BYTES as u64) {
            return Err("Image is too large".into());
        }
        let mut bytes = Vec::with_capacity(response.content_length().unwrap_or(0).min(64 * 1024) as usize);
        while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
            if chunk.len() > MAX_IMAGE_BYTES.saturating_sub(bytes.len()) {
                return Err("Image is too large".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let mime = image_mime(&bytes).ok_or("Unsupported image format")?;
        let data = as_data_url(mime, &bytes);
        let root = self.root.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let part = path.with_extension("part");
            if std::fs::write(&part, bytes).is_ok() {
                if std::fs::rename(&part, &path).is_err() {
                    let _ = std::fs::remove_file(part);
                }
            }
            if CACHE_WRITES.fetch_add(1, Ordering::Relaxed) % 8 == 7 {
                prune(&root);
            }
        }).await;
        Ok(data)
    }
}

fn cache_path(root: &Path, url: &str) -> PathBuf {
    let hash = sha2::Sha256::digest(url.as_bytes());
    let name = hash.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    root.join(format!("{name}.img"))
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) { Some("image/jpeg") }
    else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { Some("image/png") }
    else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") { Some("image/gif") }
    else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" { Some("image/webp") }
    else { None }
}

fn as_data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn prune(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    let mut files = entries.flatten().filter_map(|entry| {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "img") { return None }
        let metadata = entry.metadata().ok()?;
        Some((path, metadata.len(), metadata.modified().ok()))
    }).collect::<Vec<_>>();
    let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
    if total <= MAX_CACHE_BYTES { return }
    files.sort_by_key(|(_, _, modified)| *modified);
    for (path, size, _) in files {
        if total <= MAX_CACHE_BYTES { break }
        if std::fs::remove_file(path).is_ok() { total = total.saturating_sub(size) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_url_specific_and_mime_comes_from_image_bytes() {
        let root = Path::new("cache");
        assert_ne!(cache_path(root, "https://i1.sndcdn.com/a-t160x160.jpg"), cache_path(root, "https://i1.sndcdn.com/a-t500x500.jpg"));
        assert_eq!(image_mime(b"\xff\xd8\xff\x00"), Some("image/jpeg"));
        assert_eq!(image_mime(b"\x89PNG\r\n\x1a\n"), Some("image/png"));
        assert_eq!(image_mime(b"not an image"), None);
    }
}
