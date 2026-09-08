//! SoundCloud link parsing: `soundcloud.com` URLs (including `on.` short
//! links), plus `soundcloud:` / `fastcloud:` URIs. Mirrors fastpotify's
//! `link.rs` role: validate early (bad links exit 2), hand the rest to the
//! running instance or resolve via `/resolve` after boot.

#![allow(dead_code)]

use anyhow::{Context, Result};

/// A parsed open/play intent. IDs play/navigate without network;
/// [`ParsedLink::RemoteUrl`] needs `/resolve` first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedLink {
    TrackId(u64),
    PlaylistId(u64),
    UserId(u64),
    /// Any https SoundCloud URL (track, playlist, user, short link).
    RemoteUrl(String),
}

fn parse_id(segment: &str) -> Option<u64> {
    let digits: String = segment.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() > 20 {
        return None;
    }
    digits.parse().ok()
}

/// Parse anything the user may drop on us: full URLs, short links, URIs.
pub fn parse(raw: &str) -> Result<ParsedLink> {
    let raw = raw.trim();
    anyhow::ensure!(!raw.is_empty(), "empty link");

    // soundcloud:track(s):<id> / soundcloud:playlist:<id> / soundcloud:user:<id>
    // fastcloud:open?url=<encoded>
    if let Some(rest) = raw
        .strip_prefix("soundcloud:")
        .or_else(|| raw.strip_prefix("fastcloud:"))
    {
        let rest = rest.strip_prefix("open?url=").unwrap_or(rest);
        // URL-encoded full URL inside fastcloud:open?url=...
        if rest.starts_with("http") {
            let decoded = urlencoding_decode(rest);
            return parse(&decoded);
        }
        let mut parts = rest.split(':');
        let kind = parts.next().unwrap_or("").to_lowercase();
        // soundcloud://track/<id> form.
        let kind = kind.trim_start_matches('/').to_string();
        let id_part = parts.next().unwrap_or("");
        let id_part = id_part.trim_start_matches('/');
        match kind.as_str() {
            "track" | "tracks" => {
                let id = parse_id(id_part).context("bad track id")?;
                return Ok(ParsedLink::TrackId(id));
            }
            "playlist" | "playlists" | "album" | "albums" => {
                let id = parse_id(id_part).context("bad playlist id")?;
                return Ok(ParsedLink::PlaylistId(id));
            }
            "user" | "users" | "artist" | "artists" => {
                let id = parse_id(id_part).context("bad user id")?;
                return Ok(ParsedLink::UserId(id));
            }
            _ => {}
        }
        // Maybe the whole thing is just an id.
        if let Some(id) = parse_id(&kind) {
            return Ok(ParsedLink::TrackId(id));
        }
        anyhow::bail!("cannot open {raw:?}");
    }

    // Plain numeric id.
    if let Some(id) = parse_id(raw) {
        if id.to_string() == raw {
            return Ok(ParsedLink::TrackId(id));
        }
    }

    // https://soundcloud.com/<user>/<track|sets>/<slug> and variants.
    let lower = raw.to_lowercase();
    let url = if lower.starts_with("http://") || lower.starts_with("https://") {
        raw.to_string()
    } else if lower.starts_with("soundcloud.com/")
        || lower.starts_with("on.soundcloud.com/")
        || lower.starts_with("www.soundcloud.com/")
    {
        format!("https://{raw}")
    } else {
        anyhow::bail!("cannot open {raw:?}");
    };
    let parsed = url::Url::parse(&url).context("bad URL")?;
    let host = parsed.host_str().unwrap_or("").to_lowercase();
    let host = host.trim_start_matches("www.");
    if host != "soundcloud.com" && host != "on.soundcloud.com" {
        anyhow::bail!("cannot open {raw:?}: not a SoundCloud link");
    }
    if host == "on.soundcloud.com" {
        return Ok(ParsedLink::RemoteUrl(url));
    }
    let segs: Vec<&str> = parsed
        .path_segments()
        .map(|s| s.filter(|p| !p.is_empty()).collect())
        .unwrap_or_default();
    if segs.is_empty() {
        anyhow::bail!("cannot open {raw:?}");
    }
    // Reject app-internal paths, like fastpotify rejects /search etc.
    if matches!(
        segs[0].to_lowercase().as_str(),
        "search" | "stream" | "you" | "settings" | "upload" | "charts" | "discover" | "stations"
    ) {
        anyhow::bail!("cannot open {raw:?}");
    }
    Ok(ParsedLink::RemoteUrl(url))
}

fn urlencoding_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(b) => {
                    out.push(b as char);
                    i += 3;
                }
                Err(_) => {
                    out.push('%');
                    i += 1;
                }
            },
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris() {
        assert_eq!(
            parse("soundcloud:tracks:123").unwrap(),
            ParsedLink::TrackId(123)
        );
        assert_eq!(
            parse("soundcloud:track:123").unwrap(),
            ParsedLink::TrackId(123)
        );
        assert_eq!(
            parse("soundcloud:playlist:7").unwrap(),
            ParsedLink::PlaylistId(7)
        );
        assert_eq!(parse("soundcloud:user:9").unwrap(), ParsedLink::UserId(9));
        assert_eq!(parse("  42 ").unwrap(), ParsedLink::TrackId(42));
    }

    #[test]
    fn web_urls_need_resolve() {
        let link = parse("https://soundcloud.com/some-artist/some-track").unwrap();
        assert!(matches!(link, ParsedLink::RemoteUrl(_)));
        let link = parse("https://soundcloud.com/some-artist/sets/some-mix").unwrap();
        assert!(matches!(link, ParsedLink::RemoteUrl(_)));
        let link = parse("https://on.soundcloud.com/abcXYZ123").unwrap();
        assert!(matches!(link, ParsedLink::RemoteUrl(_)));
        let link = parse("soundcloud.com/some-artist/some-track").unwrap();
        assert!(matches!(link, ParsedLink::RemoteUrl(_)));
    }

    #[test]
    fn rejects_junk() {
        assert!(parse("https://example.com/x").is_err());
        assert!(parse("https://soundcloud.com/search?q=x").is_err());
        assert!(parse("not a link at all!!").is_err());
        assert!(parse("").is_err());
        assert!(parse("spotify:track:abc").is_err());
    }
}
