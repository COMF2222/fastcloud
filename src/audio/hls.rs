#![allow(dead_code)]

use anyhow::{Context, Result};

/// A minimal M3U8 (HLS) playlist parser: media segments + optional init segment.
#[derive(Debug, Clone, Default)]
pub struct MediaPlaylist {
    /// Leading `#EXT-X-MAP` init segment URI (fMP4).
    pub init_uri: Option<String>,
    /// Segment URIs in play order.
    pub segments: Vec<String>,
    /// Target duration in seconds (used for buffering heuristics).
    pub target_duration: Option<f64>,
}

pub fn parse(content: &str, base_url: &str) -> Result<MediaPlaylist> {
    let mut pl = MediaPlaylist::default();
    let mut next_is_segment = false;
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
            next_is_segment = true;
        } else if !line.starts_with('#') && next_is_segment {
            pl.segments.push(absolute(line, base_url)?);
            next_is_segment = false;
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
    pub cache: std::sync::Arc<super::cache::AudioCache>,
}

impl HlsDownloader {
    pub fn new(http: reqwest::Client, cache: std::sync::Arc<super::cache::AudioCache>) -> Self {
        Self { http, cache }
    }

    /// Fetch and parse a media playlist.
    pub async fn playlist(&self, url: &str) -> Result<MediaPlaylist> {
        let text = self.http.get(url).send().await?.text().await?;
        parse(&text, url)
    }

    /// Download one segment, using the disk cache when possible.
    pub async fn segment(&self, track_urn: &str, url: &str) -> Result<Vec<u8>> {
        if let Some(hit) = self.cache.get_segment(track_urn, url) {
            return Ok(hit);
        }
        let resp = self.http.get(url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("segment fetch {status}: {url}");
        }
        let bytes = resp.bytes().await?.to_vec();
        self.cache.put_segment(track_urn, url, &bytes);
        Ok(bytes)
    }

    /// Download the fMP4 init segment (cached).
    pub async fn init_segment(&self, track_urn: &str, url: &str) -> Result<Vec<u8>> {
        if let Some(hit) = self.cache.get_segment(track_urn, url) {
            return Ok(hit);
        }
        let resp = self.http.get(url).send().await?;
        let bytes = resp.bytes().await?.to_vec();
        self.cache.put_segment(track_urn, url, &bytes);
        Ok(bytes)
    }
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
        assert_eq!(pl.target_duration, Some(10.0));
    }

    #[test]
    fn parse_no_map() {
        let pl = parse("#EXTM3U\n#EXTINF:5.0,\na.mp4\n", "https://x/y.m3u8").unwrap();
        assert!(pl.init_uri.is_none());
        assert_eq!(pl.segments, vec!["https://x/a.mp4"]);
    }
}
