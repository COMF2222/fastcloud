#![allow(dead_code)]

use anyhow::{Context, Result};

/// A minimal M3U8 (HLS) playlist parser: media segments + optional init segment.
#[derive(Debug, Clone, Default)]
pub struct MediaPlaylist {
    /// Leading `#EXT-X-MAP` init segment URI (fMP4).
    pub init_uri: Option<String>,
    /// Segment URIs in play order.
    pub segments: Vec<String>,
    /// Exact `#EXTINF` duration for each segment, in seconds.
    pub segment_durations: Vec<f64>,
    /// Target duration in seconds (used for buffering heuristics).
    pub target_duration: Option<f64>,
}

impl MediaPlaylist {
    /// Pick the segment containing `target_ms` and return its absolute start.
    /// Seeking can then download only that segment onward instead of replaying
    /// the entire stream from byte zero.
    pub fn seek_point(&self, target_ms: u64) -> (usize, u64) {
        if self.segments.is_empty() {
            return (0, 0);
        }
        let fallback_ms = self
            .target_duration
            .map(|seconds| (seconds * 1000.0).round() as u64)
            .filter(|duration| *duration > 0)
            .unwrap_or(10_000);
        let mut start_ms = 0u64;
        for index in 0..self.segments.len() {
            let duration_ms = self
                .segment_durations
                .get(index)
                .map(|seconds| (seconds * 1000.0).round() as u64)
                .filter(|duration| *duration > 0)
                .unwrap_or(fallback_ms);
            if target_ms < start_ms.saturating_add(duration_ms) {
                return (index, start_ms);
            }
            start_ms = start_ms.saturating_add(duration_ms);
        }
        let last = self.segments.len() - 1;
        let last_duration = self
            .segment_durations
            .get(last)
            .map(|seconds| (seconds * 1000.0).round() as u64)
            .filter(|duration| *duration > 0)
            .unwrap_or(fallback_ms);
        (last, start_ms.saturating_sub(last_duration))
    }
}

pub fn parse(content: &str, base_url: &str) -> Result<MediaPlaylist> {
    anyhow::ensure!(
        content.trim_start().starts_with("#EXTM3U"),
        "invalid HLS playlist"
    );
    let mut pl = MediaPlaylist::default();
    let mut next_duration = None;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("#EXT-X-TARGETDURATION:") {
            pl.target_duration = line.split(':').nth(1).and_then(|v| v.trim().parse().ok());
        } else if line.starts_with("#EXT-X-MAP:") {
            let uri = attribute_value(line, "URI").context("#EXT-X-MAP without URI")?;
            pl.init_uri = Some(absolute(&uri, base_url)?);
        } else if line.starts_with("#EXTINF:") {
            next_duration = line
                .strip_prefix("#EXTINF:")
                .and_then(|value| value.split(',').next())
                .and_then(|value| value.trim().parse::<f64>().ok());
        } else if !line.starts_with('#') && next_duration.is_some() {
            pl.segments.push(absolute(line, base_url)?);
            pl.segment_durations
                .push(next_duration.take().unwrap_or_default());
        }
    }
    Ok(pl)
}

fn attribute_value(line: &str, key: &str) -> Option<String> {
    // #EXT-X-MAP:URI="init.mp4",BYTERANGE="..."
    let needle = format!("{key}=\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn absolute(uri: &str, base: &str) -> Result<String> {
    if uri.starts_with("http://") || uri.starts_with("https://") {
        Ok(uri.to_string())
    } else if let Ok(base_url) = url::Url::parse(base) {
        Ok(base_url.join(uri)?.to_string())
    } else {
        anyhow::bail!("cannot resolve relative URI {uri:?} against {base:?}")
    }
}

/// Streaming HLS segment fetcher with disk cache support.
pub struct HlsDownloader {
    http: reqwest::Client,
    oauth: parking_lot::RwLock<Option<String>>,
    pub cache: std::sync::Arc<super::cache::AudioCache>,
}

impl HlsDownloader {
    pub fn new(http: reqwest::Client, cache: std::sync::Arc<super::cache::AudioCache>) -> Self {
        Self {
            http,
            cache,
            oauth: parking_lot::RwLock::new(None),
        }
    }

    pub fn set_oauth(&self, token: Option<String>) {
        *self.oauth.write() = token;
    }

    fn request(&self, url: &str) -> reqwest::RequestBuilder {
        let mut request = self.http.get(url);
        // Stream URLs now start on the API host and redirect to a CDN.
        // Authorize that first hop only; reqwest strips it on cross-host redirects.
        if needs_oauth(url)
            && let Some(token) = self.oauth.read().as_ref()
        {
            request = request.header(reqwest::header::AUTHORIZATION, format!("OAuth {token}"));
        }
        request
    }

    /// Fetch and parse a media playlist.
    pub async fn playlist(&self, url: &str) -> Result<MediaPlaylist> {
        let text = self
            .request(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        parse(&text, url)
    }

    /// Download one segment, using the disk cache when possible.
    pub async fn segment(&self, track_urn: &str, url: &str) -> Result<Vec<u8>> {
        let cached = tokio::task::spawn_blocking({
            let cache = self.cache.clone();
            let track_urn = track_urn.to_owned();
            let url = url.to_owned();
            move || cache.get_segment(&track_urn, &url)
        })
        .await
        .unwrap_or(None);
        if let Some(hit) = cached {
            return Ok(hit);
        }
        let resp = self.request(url).send().await?.error_for_status()?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("segment fetch {status}: {url}");
        }
        let bytes = resp.bytes().await?.to_vec();
        self.cache_segment(track_urn, url, &bytes).await;
        Ok(bytes)
    }

    /// Download the fMP4 init segment (cached).
    pub async fn init_segment(&self, track_urn: &str, url: &str) -> Result<Vec<u8>> {
        let cached = tokio::task::spawn_blocking({
            let cache = self.cache.clone();
            let track_urn = track_urn.to_owned();
            let url = url.to_owned();
            move || cache.get_segment(&track_urn, &url)
        })
        .await
        .unwrap_or(None);
        if let Some(hit) = cached {
            return Ok(hit);
        }
        let resp = self.request(url).send().await?.error_for_status()?;
        let bytes = resp.bytes().await?.to_vec();
        self.cache_segment(track_urn, url, &bytes).await;
        Ok(bytes)
    }

    async fn cache_segment(&self, track_urn: &str, url: &str, bytes: &[u8]) {
        let cache = self.cache.clone();
        let track_urn = track_urn.to_owned();
        let url = url.to_owned();
        let bytes = bytes.to_vec();
        let _ =
            tokio::task::spawn_blocking(move || cache.put_segment(&track_urn, &url, &bytes)).await;
    }
}

fn needs_oauth(url: &str) -> bool {
    url::Url::parse(url)
        .is_ok_and(|url| url.scheme() == "https" && url.host_str() == Some("api.soundcloud.com"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PL: &str = "#EXTM3U\n\
        #EXT-X-TARGETDURATION:10\n\
        #EXT-X-MAP:URI=\"init.mp4\"\n\
        #EXTINF:9.9,\n\
        seg1.ts\n\
        #EXTINF:9.9,\n\
        https://cdn.example/seg2.ts\n\
        #EXT-X-ENDLIST\n";

    #[test]
    fn parse_absolute_and_relative() {
        let pl = parse(PL, "https://media.example/hls/playlist.m3u8").unwrap();
        assert_eq!(
            pl.init_uri.as_deref(),
            Some("https://media.example/hls/init.mp4")
        );
        assert_eq!(pl.segments.len(), 2);
        assert_eq!(pl.segments[0], "https://media.example/hls/seg1.ts");
        assert_eq!(pl.segments[1], "https://cdn.example/seg2.ts");
        assert_eq!(pl.segment_durations, vec![9.9, 9.9]);
        assert_eq!(pl.target_duration, Some(10.0));
    }

    #[test]
    fn parse_no_map() {
        let pl = parse("#EXTM3U\n#EXTINF:5.0,\na.mp4\n", "https://x/y.m3u8").unwrap();
        assert!(pl.init_uri.is_none());
        assert_eq!(pl.segments, vec!["https://x/a.mp4"]);
    }

    #[test]
    fn seek_starts_at_the_segment_containing_the_target() {
        let pl = parse(PL, "https://media.example/hls/playlist.m3u8").unwrap();
        assert_eq!(pl.seek_point(0), (0, 0));
        assert_eq!(pl.seek_point(9_899), (0, 0));
        assert_eq!(pl.seek_point(9_900), (1, 9_900));
        assert_eq!(pl.seek_point(15_000), (1, 9_900));
    }
}
