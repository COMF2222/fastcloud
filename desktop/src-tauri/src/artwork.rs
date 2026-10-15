use base64::Engine as _;
use sha2::Digest as _;
use std::io::Read;
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

    pub fn directory(&self) -> &Path {
        &self.root
    }

    pub async fn image_source(
        &self,
        raw: String,
        relay: Option<(String, String)>,
    ) -> Result<String, String> {
        let url = url::Url::parse(&raw).map_err(|_| "Invalid image URL")?;
        let host = url.host_str().ok_or("Image host is missing")?;
        if url.scheme() != "https" || (host != "sndcdn.com" && !host.ends_with(".sndcdn.com")) {
            return Err("Only HTTPS SoundCloud artwork is allowed".into());
        }
        let path = cache_path(&self.root, url.as_str());
        let read_path = path.clone();
        if tokio::task::spawn_blocking(move || cached_image(&read_path))
            .await
            .unwrap_or(false)
        {
            return Ok(path.to_string_lossy().into_owned());
        }

        // Limit simultaneous downloads after checking the disk cache. This also
        // bounds the number of partially buffered responses during a fast scroll.
        let _permit = self
            .download_slots
            .acquire()
            .await
            .map_err(|error| error.to_string())?;
        let request = if let Some((server, token)) = relay {
            self.http
                .post(format!("{server}/v1/soundcloud/artwork"))
                .header(reqwest::header::AUTHORIZATION, format!("OAuth {token}"))
                .json(&serde_json::json!({"url": url.as_str()}))
        } else {
            self.http.get(url)
        };
        let mut response = request
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?;
        if response
            .content_length()
            .is_some_and(|size| size > MAX_IMAGE_BYTES as u64)
        {
            return Err("Image is too large".into());
        }
        let mut bytes =
            Vec::with_capacity(response.content_length().unwrap_or(0).min(64 * 1024) as usize);
        while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
            if chunk.len() > MAX_IMAGE_BYTES.saturating_sub(bytes.len()) {
                return Err("Image is too large".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let mime = image_mime(&bytes).ok_or("Unsupported image format")?;
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || {
            // File URLs keep compressed image bytes out of the JS query cache
            // and avoid base64/JSON copies in both processes. Retain the old
            // bounded fallback when the cache is unwritable or full.
            let source = if save_image(&path, &bytes).is_ok() {
                path.to_string_lossy().into_owned()
            } else {
                as_data_url(mime, &bytes)
            };
            if CACHE_WRITES.fetch_add(1, Ordering::Relaxed) % 8 == 7 {
                prune(&root, Some(&path));
            }
            source
        })
        .await
        .map_err(|error| error.to_string())
    }
}

fn cached_image(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(metadata) = file.metadata() else {
        return false;
    };
    if metadata.len() == 0 || metadata.len() > MAX_IMAGE_BYTES as u64 {
        return false;
    }
    let mut header = [0; 12];
    let Ok(size) = file.read(&mut header) else {
        return false;
    };
    image_mime(&header[..size]).is_some()
}

fn save_image(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    // Concurrent requests for the same cover must not share a partial file.
    let part = path.with_extension(format!("{}.part", rand::random::<u64>()));
    let result = std::fs::write(&part, bytes).and_then(|()| std::fs::rename(&part, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

fn cache_path(root: &Path, url: &str) -> PathBuf {
    let hash = sha2::Sha256::digest(url.as_bytes());
    let name = hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    root.join(format!("{name}.img"))
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn as_data_url(mime: &str, bytes: &[u8]) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn prune(root: &Path, protected: Option<&Path>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut files = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "img") {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            Some((path, metadata.len(), metadata.modified().ok()))
        })
        .collect::<Vec<_>>();
    let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
    if total <= MAX_CACHE_BYTES {
        return;
    }
    files.sort_by_key(|(_, _, modified)| *modified);
    for (path, size, _) in files {
        if total <= MAX_CACHE_BYTES {
            break;
        }
        if protected == Some(path.as_path()) {
            continue;
        }
        if std::fs::remove_file(path).is_ok() {
            total = total.saturating_sub(size)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_url_specific_and_mime_comes_from_image_bytes() {
        let root = Path::new("cache");
        assert_ne!(
            cache_path(root, "https://i1.sndcdn.com/a-t160x160.jpg"),
            cache_path(root, "https://i1.sndcdn.com/a-t500x500.jpg")
        );
        assert_eq!(image_mime(b"\xff\xd8\xff\x00"), Some("image/jpeg"));
        assert_eq!(image_mime(b"\x89PNG\r\n\x1a\n"), Some("image/png"));
        assert_eq!(image_mime(b"not an image"), None);
    }

    #[tokio::test]
    async fn cached_cover_returns_only_a_file_path_and_preserves_original_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let cache = ArtworkCache::new(temp.path().to_owned()).unwrap();
        let url = "https://i1.sndcdn.com/probe-original.jpg";
        let path = cache_path(temp.path(), url);
        let mut bytes = vec![0x42; 1024 * 1024];
        bytes[..3].copy_from_slice(&[0xff, 0xd8, 0xff]);
        save_image(&path, &bytes).unwrap();
        let source = cache.image_source(url.to_owned(), None).await.unwrap();
        assert_eq!(source, path.to_string_lossy());
        assert!(source.len() < 512);
        assert_eq!(std::fs::read(&source).unwrap(), bytes);
        assert!(
            tauri::utils::mime_type::MimeType::parse(&bytes, &source).starts_with("image/jpeg")
        );
    }

    #[test]
    fn cache_rejects_invalid_headers_and_oversized_files() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("invalid.img");
        std::fs::write(&path, b"not an image").unwrap();
        assert!(!cached_image(&path));
        std::fs::write(&path, b"\xff\xd8\xff").unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(MAX_IMAGE_BYTES as u64 + 1)
            .unwrap();
        assert!(!cached_image(&path));
    }

    #[tokio::test]
    async fn artwork_bridge_does_not_accept_local_or_foreign_addresses() {
        let temp = tempfile::tempdir().unwrap();
        let cache = ArtworkCache::new(temp.path().to_owned()).unwrap();
        for url in [
            "file:///secret",
            "http://i1.sndcdn.com/a.jpg",
            "https://example.invalid/a.jpg",
        ] {
            assert!(cache.image_source(url.to_owned(), None).await.is_err());
        }
    }

    #[test]
    fn quota_preserves_the_file_about_to_be_displayed() {
        let temp = tempfile::tempdir().unwrap();
        let current = temp.path().join("current.img");
        let old = temp.path().join("old.img");
        for path in [&current, &old] {
            std::fs::File::create(path)
                .unwrap()
                .set_len(MAX_CACHE_BYTES)
                .unwrap();
        }
        prune(temp.path(), Some(&current));
        assert!(current.is_file());
        assert!(!old.exists());
    }
}
