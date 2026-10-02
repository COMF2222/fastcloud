use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsRecord {
    pub id: u64,
    pub track_name: String,
    pub artist_name: String,
    pub album_name: Option<String>,
    pub duration: Option<f64>,
    pub plain_lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(default = "lrclib_source")]
    pub source: String,
    #[serde(default)]
    pub source_url: Option<String>,
}

fn lrclib_source() -> String {
    "LRCLIB".into()
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .user_agent("Fastcloud/0.2 (https://github.com/COMF2222/fastcloud)")
        .build()
        .map_err(|_| "Could not start lyrics search".into())
}

async fn send_lrclib(
    request: impl Fn() -> reqwest::RequestBuilder,
) -> Result<reqwest::Response, String> {
    for attempt in 0..2 {
        let response = request()
            .send()
            .await
            .map_err(|_| "LRCLIB connection failed")?;
        if matches!(
            response.status(),
            reqwest::StatusCode::SERVICE_UNAVAILABLE | reqwest::StatusCode::TOO_MANY_REQUESTS
        ) && attempt == 0
        {
            let seconds = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(1)
                .clamp(1, 3);
            tokio::time::sleep(Duration::from_secs(seconds)).await;
            continue;
        }
        return Ok(response);
    }
    Err("LRCLIB unavailable".into())
}

fn comparable(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

fn promotional(value: &str) -> bool {
    let value = value.trim().to_lowercase();
    [
        "official",
        "audio",
        "video",
        "lyrics",
        "lyric video",
        "visualizer",
        "visualiser",
        "music video",
        "hq",
        "hd",
        "remaster",
    ]
    .iter()
    .any(|prefix| {
        value == *prefix
            || value
                .strip_prefix(prefix)
                .is_some_and(|tail| tail.starts_with(' '))
    }) || value.starts_with("prod.")
        || value.starts_with("prod ")
}

fn clean_title(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        let close = match ch {
            '(' => ')',
            '[' => ']',
            '{' => '}',
            _ => {
                output.push(ch);
                continue;
            }
        };
        let mut part = String::new();
        let mut ended = false;
        for next in chars.by_ref() {
            if next == close {
                ended = true;
                break;
            }
            part.push(next);
        }
        if !ended || !promotional(&part) || !version_tags(&part).is_empty() {
            output.push(ch);
            output.push_str(&part);
            if ended {
                output.push(close);
            }
        }
    }
    let mut output = output.split_whitespace().collect::<Vec<_>>().join(" ");
    for suffix in [
        "official audio",
        "official video",
        "official music video",
        "lyric video",
    ] {
        if output.to_lowercase().ends_with(suffix) {
            output.truncate(output.len() - suffix.len());
            output = output
                .trim_end_matches([' ', '-', '–', '—', '|'])
                .to_owned();
        }
    }
    output.trim().to_owned()
}

fn version_tags(title: &str) -> Vec<String> {
    let lower = title.to_lowercase();
    let mut tags = lower
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| {
            matches!(
                *word,
                "remix"
                    | "slowed"
                    | "sped"
                    | "nightcore"
                    | "live"
                    | "acoustic"
                    | "cover"
                    | "instrumental"
                    | "karaoke"
                    | "edit"
                    | "extended"
            )
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    tags.sort();
    tags.dedup();
    tags
}

fn candidate_pairs(artist: &str, title: &str) -> Vec<(String, String)> {
    let title = clean_title(title);
    let mut pairs = Vec::new();
    for separator in [" - ", " – ", " — ", " | "] {
        if let Some((prefix, song)) = title.split_once(separator) {
            if !prefix.trim().is_empty() && !song.trim().is_empty() && prefix.chars().count() < 70 {
                pairs.push((prefix.trim().to_owned(), song.trim().to_owned()));
                break;
            }
        }
    }
    if !artist.trim().is_empty() && !title.is_empty() {
        let original = (artist.trim().to_owned(), title);
        if !pairs.iter().any(|pair| {
            comparable(&pair.0) == comparable(&original.0)
                && comparable(&pair.1) == comparable(&original.1)
        }) {
            pairs.push(original);
        }
    }
    pairs
}

fn duration_difference(record: &LyricsRecord, duration_ms: u64) -> Option<f64> {
    record
        .duration
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .filter(|_| duration_ms > 1000)
        .map(|seconds| (seconds * 1000.0 - duration_ms as f64).abs())
}

fn match_score(
    record: &LyricsRecord,
    artist: &str,
    title: &str,
    duration_ms: u64,
    album: Option<&str>,
) -> u32 {
    let wanted_title = comparable(&clean_title(title));
    let wanted_artist = comparable(artist);
    if wanted_title.is_empty()
        || wanted_artist.is_empty()
        || comparable(&clean_title(&record.track_name)) != wanted_title
        || comparable(&record.artist_name) != wanted_artist
        || version_tags(&record.track_name) != version_tags(title)
    {
        return 0;
    }
    let difference = duration_difference(record, duration_ms);
    if difference.is_some_and(|value| value > 20_000.0) {
        return 0;
    }
    100 + difference.map_or(0, |value| {
        if value <= 3000.0 {
            20
        } else if value <= 8000.0 {
            10
        } else {
            0
        }
    }) + u32::from(album.is_some_and(|wanted| {
        record
            .album_name
            .as_deref()
            .is_some_and(|found| comparable(found) == comparable(wanted))
    })) * 5
}

fn usable(record: &LyricsRecord) -> bool {
    record.instrumental
        || record
            .plain_lyrics
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || record
            .synced_lyrics
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
}

fn select_record(
    records: Vec<LyricsRecord>,
    artist: &str,
    title: &str,
    duration_ms: u64,
    album: Option<&str>,
) -> Option<LyricsRecord> {
    records
        .into_iter()
        .map(|mut record| {
            // Reject timestamps for a different edit before ranking candidates.
            if !duration_difference(&record, duration_ms).is_some_and(|value| value <= 3000.0) {
                record.synced_lyrics = None;
            }
            record
        })
        .filter(usable)
        .map(|entry| {
            (
                match_score(&entry, artist, title, duration_ms, album),
                entry,
            )
        })
        .filter(|(score, _)| *score > 0)
        .max_by_key(|(score, entry)| (*score, entry.synced_lyrics.is_some()))
        .map(|(_, entry)| entry)
}

async fn exact(
    client: &reqwest::Client,
    artist: &str,
    title: &str,
    duration_ms: u64,
    album: Option<&str>,
) -> Result<Option<LyricsRecord>, String> {
    let seconds = (duration_ms / 1000).to_string();
    let response = send_lrclib(|| {
        client.get("https://lrclib.net/api/get").query(&[
            ("artist_name", artist),
            ("track_name", title),
            ("duration", seconds.as_str()),
            ("album_name", album.unwrap_or("")),
        ])
    })
    .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!("LRCLIB HTTP {}", response.status()));
    }
    let record = response
        .json::<LyricsRecord>()
        .await
        .map_err(|_| "Invalid LRCLIB response")?;
    Ok(select_record(
        vec![record],
        artist,
        title,
        duration_ms,
        album,
    ))
}

async fn search_structured(
    client: &reqwest::Client,
    artist: &str,
    title: &str,
    duration_ms: u64,
    album: Option<&str>,
) -> Result<Option<LyricsRecord>, String> {
    let response = send_lrclib(|| {
        client
            .get("https://lrclib.net/api/search")
            .query(&[("artist_name", artist), ("track_name", title)])
    })
    .await?;
    if !response.status().is_success() {
        return Err(format!("LRCLIB HTTP {}", response.status()));
    }
    let entries = response
        .json::<Vec<LyricsRecord>>()
        .await
        .map_err(|_| "Invalid LRCLIB response")?;
    Ok(select_record(entries, artist, title, duration_ms, album))
}

static MB_REQUEST: tokio::sync::Mutex<Option<tokio::time::Instant>> =
    tokio::sync::Mutex::const_new(None);

fn valid_isrc(raw: &str) -> Option<String> {
    let value = raw.replace(['-', ' '], "").to_uppercase();
    let bytes = value.as_bytes();
    (bytes.len() == 12
        && bytes[..2].iter().all(u8::is_ascii_alphabetic)
        && bytes[2..5].iter().all(u8::is_ascii_alphanumeric)
        && bytes[5..].iter().all(u8::is_ascii_digit))
    .then_some(value)
}

fn recording_identity(data: &Value, title: &str, duration_ms: u64) -> Option<(String, String)> {
    let mut identities = Vec::new();
    for recording in data.get("recordings")?.as_array()?.iter().take(25) {
        let Some(song) = recording.get("title").and_then(Value::as_str) else {
            continue;
        };
        let Some(credits) = recording.get("artist-credit").and_then(Value::as_array) else {
            continue;
        };
        let artist = credits
            .iter()
            .filter_map(|credit| {
                credit
                    .get("name")
                    .or_else(|| credit.get("artist")?.get("name"))
                    .and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join(" & ");
        if artist.is_empty() || version_tags(song) != version_tags(title) {
            continue;
        }
        if duration_ms > 1000
            && recording
                .get("length")
                .and_then(Value::as_u64)
                .is_some_and(|length| length.abs_diff(duration_ms) > 8000)
        {
            continue;
        }
        if !identities.iter().any(|(a, t): &(String, String)| {
            comparable(a) == comparable(&artist) && comparable(t) == comparable(song)
        }) {
            identities.push((artist, song.to_owned()));
        }
    }
    // A reused ISRC must not silently choose between different recordings.
    (identities.len() == 1).then(|| identities.remove(0))
}

async fn identify_isrc(
    client: &reqwest::Client,
    isrc: &str,
    title: &str,
    duration_ms: u64,
) -> Option<(String, String)> {
    let code = valid_isrc(isrc)?;
    {
        let mut last = MB_REQUEST.lock().await;
        if let Some(previous) = *last {
            tokio::time::sleep_until(previous + Duration::from_secs(1)).await;
        }
        *last = Some(tokio::time::Instant::now());
    }
    let response = client
        .get(format!("https://musicbrainz.org/ws/2/isrc/{code}"))
        .query(&[("fmt", "json"), ("inc", "artist-credits")])
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    recording_identity(&response.json::<Value>().await.ok()?, title, duration_ms)
}

async fn lyrics_ovh(
    client: &reqwest::Client,
    artist: &str,
    title: &str,
) -> Result<Option<LyricsRecord>, String> {
    let mut url =
        reqwest::Url::parse("https://api.lyrics.ovh/v1/").map_err(|_| "Invalid lyrics URL")?;
    url.path_segments_mut()
        .map_err(|_| "Invalid lyrics URL")?
        .push(artist)
        .push(title);
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| "lyrics.ovh connection failed")?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!("lyrics.ovh HTTP {}", response.status()));
    }
    let data = response
        .json::<Value>()
        .await
        .map_err(|_| "Invalid lyrics.ovh response")?;
    let Some(plain) = data
        .get("lyrics")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    else {
        return Ok(None);
    };
    Ok(Some(LyricsRecord {
        id: 0,
        track_name: title.into(),
        artist_name: artist.into(),
        album_name: None,
        duration: None,
        plain_lyrics: Some(plain.into()),
        synced_lyrics: None,
        instrumental: false,
        source: "lyrics.ovh".into(),
        source_url: Some("https://lyrics.ovh/".into()),
    }))
}

fn append_lyrics(element: ElementRef<'_>, output: &mut String) {
    let value = element.value();
    if value.attr("data-exclude-from-selection") == Some("true")
        || value
            .attr("class")
            .is_some_and(|class| class.contains("LyricsHeader"))
        || matches!(value.name(), "script" | "style")
    {
        return;
    }
    if value.name() == "br" {
        output.push('\n');
        return;
    }
    for child in element.children() {
        if let scraper::Node::Text(text) = child.value() {
            output.push_str(&text.text);
        } else if let Some(child) = ElementRef::wrap(child) {
            append_lyrics(child, output);
        }
    }
}

fn genius_text(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("div[data-lyrics-container=\"true\"]").ok()?;
    let mut output = String::new();
    for container in document.select(&selector) {
        if !output.is_empty() {
            output.push('\n');
        }
        append_lyrics(container, &mut output);
    }
    let output = output.trim();
    (!output.is_empty() && output.len() <= 64_000).then(|| output.to_owned())
}

async fn genius(
    client: &reqwest::Client,
    artist: &str,
    title: &str,
) -> Result<Option<LyricsRecord>, String> {
    let response = client
        .get("https://genius.com/api/search/song")
        .query(&[("q", format!("{artist} {title}"))])
        .send()
        .await
        .map_err(|_| "Genius connection failed")?;
    if !response.status().is_success() {
        return Err(format!("Genius HTTP {}", response.status()));
    }
    let data = response
        .json::<Value>()
        .await
        .map_err(|_| "Invalid Genius response")?;
    let Some(sections) = data.pointer("/response/sections").and_then(Value::as_array) else {
        return Ok(None);
    };
    for hit in sections
        .iter()
        .filter(|section| section.get("type").and_then(Value::as_str) == Some("song"))
        .filter_map(|section| section.get("hits").and_then(Value::as_array))
        .flatten()
        .take(10)
    {
        let Some(song) = hit.get("result") else {
            continue;
        };
        let found_title = song
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let found_artist = song
            .pointer("/primary_artist/name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if comparable(&clean_title(found_title)) != comparable(&clean_title(title))
            || comparable(found_artist) != comparable(artist)
            || version_tags(found_title) != version_tags(title)
            || song.get("lyrics_state").and_then(Value::as_str) != Some("complete")
            || song.get("instrumental").and_then(Value::as_bool) == Some(true)
        {
            continue;
        }
        let Some(url) = song
            .get("url")
            .and_then(Value::as_str)
            .and_then(|url| reqwest::Url::parse(url).ok())
        else {
            continue;
        };
        if url.scheme() != "https" || url.host_str() != Some("genius.com") {
            continue;
        }
        let mut response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|_| "Genius page unavailable")?;
        if !response.status().is_success() {
            return Err(format!("Genius HTTP {}", response.status()));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Genius page unavailable")?
        {
            if bytes.len() + chunk.len() > 2_000_000 {
                return Err("Genius page is too large".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let text = String::from_utf8(bytes).map_err(|_| "Invalid Genius page")?;
        if let Some(plain) = genius_text(&text) {
            return Ok(Some(LyricsRecord {
                id: song.get("id").and_then(Value::as_u64).unwrap_or(0),
                track_name: found_title.into(),
                artist_name: found_artist.into(),
                album_name: None,
                duration: None,
                plain_lyrics: Some(plain),
                synced_lyrics: None,
                instrumental: false,
                source: "Genius".into(),
                source_url: Some(url.to_string()),
            }));
        }
    }
    Ok(None)
}

async fn lookup_sources(
    artist: &str,
    title: &str,
    duration_ms: u64,
    album: Option<&str>,
    isrc: Option<&str>,
) -> Result<Option<LyricsRecord>, String> {
    let client = client()?;
    let mut pairs = candidate_pairs(artist, title);
    if let Some(code) = isrc {
        if let Some(identity) = identify_isrc(&client, code, title, duration_ms).await {
            pairs = vec![identity];
        }
    }
    if pairs.is_empty() {
        return Ok(None);
    }
    let mut answered = false;
    for (performer, song) in &pairs {
        match exact(&client, performer, song, duration_ms, album).await {
            Ok(Some(record)) => return Ok(Some(record)),
            Ok(None) => answered = true,
            Err(_) => break,
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
        match search_structured(&client, performer, song, duration_ms, album).await {
            Ok(Some(record)) => return Ok(Some(record)),
            Ok(None) => answered = true,
            Err(_) => break,
        }
    }
    // This provider cannot distinguish recording versions; never use it for edited audio.
    for (performer, song) in pairs
        .iter()
        .filter(|(_, song)| version_tags(song).is_empty())
    {
        match lyrics_ovh(&client, performer, song).await {
            Ok(Some(record)) => return Ok(Some(record)),
            Ok(None) => answered = true,
            Err(_) => {}
        }
    }
    for (performer, song) in &pairs {
        match genius(&client, performer, song).await {
            Ok(Some(record)) => return Ok(Some(record)),
            Ok(None) => answered = true,
            Err(_) => {}
        }
    }
    if answered {
        Ok(None)
    } else {
        Err("Lyrics sources are temporarily unavailable".into())
    }
}

pub async fn lookup(
    artist: String,
    title: String,
    duration_ms: u64,
    album: Option<String>,
    isrc: Option<String>,
) -> Result<Option<LyricsRecord>, String> {
    tokio::time::timeout(
        Duration::from_secs(40),
        lookup_sources(
            &artist,
            &title,
            duration_ms,
            album.as_deref(),
            isrc.as_deref(),
        ),
    )
    .await
    .map_err(|_| "Lyrics search timed out; try again")?
}

pub async fn search(query: String) -> Result<Vec<LyricsRecord>, String> {
    let query = query.trim();
    if query.chars().count() < 2 {
        return Ok(Vec::new());
    }
    let client = client()?;
    let response = send_lrclib(|| {
        client
            .get("https://lrclib.net/api/search")
            .query(&[("q", query)])
    })
    .await?;
    if !response.status().is_success() {
        return Err(format!("LRCLIB HTTP {}", response.status()));
    }
    let mut entries = response
        .json::<Vec<LyricsRecord>>()
        .await
        .map_err(|_| "Invalid LRCLIB response")?;
    entries.retain(usable);
    entries.truncate(20);
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(title: &str, artist: &str, seconds: f64) -> LyricsRecord {
        LyricsRecord {
            id: 1,
            track_name: title.into(),
            artist_name: artist.into(),
            album_name: None,
            duration: Some(seconds),
            plain_lyrics: Some("Synthetic test text".into()),
            synced_lyrics: Some("[00:00.00]Synthetic test text".into()),
            instrumental: false,
            source: lrclib_source(),
            source_url: None,
        }
    }
    #[test]
    fn uploader_prefix_and_promotional_labels_are_removed() {
        assert_eq!(
            clean_title("we think too much (prod. nedarb)"),
            "we think too much"
        );
        assert_eq!(
            candidate_pairs("Uploader", "Lil Peep - Star Shopping (Official Audio)")[0],
            ("Lil Peep".into(), "Star Shopping".into())
        );
    }
    #[test]
    fn related_titles_and_artists_are_not_the_same_song() {
        assert_eq!(
            match_score(
                &record("Love Me Again", "Artist", 180.0),
                "Artist",
                "Love Me",
                180_000,
                None
            ),
            0
        );
        assert_eq!(
            match_score(
                &record("Song", "Artist Tribute", 180.0),
                "Artist",
                "Song",
                180_000,
                None
            ),
            0
        );
    }
    #[test]
    fn remixes_and_speed_changes_are_preserved() {
        assert_eq!(
            clean_title("Song (Official Remix)"),
            "Song (Official Remix)"
        );
        assert_eq!(
            clean_title("Song (slowed + reverb) [Official Audio]"),
            "Song (slowed + reverb)"
        );
        assert_eq!(
            match_score(
                &record("Song", "Artist", 180.0),
                "Artist",
                "Song (Remix)",
                180_000,
                None
            ),
            0
        );
    }
    #[test]
    fn wrong_duration_is_rejected_and_other_edit_timestamps_are_removed() {
        assert!(
            select_record(
                vec![record("Song", "Artist", 260.0)],
                "Artist",
                "Song",
                180_000,
                None
            )
            .is_none()
        );
        let found = select_record(
            vec![record("Song", "Artist", 190.0)],
            "Artist",
            "Song",
            180_000,
            None,
        )
        .unwrap();
        assert!(found.synced_lyrics.is_none());
        assert!(found.plain_lyrics.is_some());
    }
    #[test]
    fn matching_duration_wins_over_another_album() {
        let found = select_record(
            vec![
                record("Song", "Artist", 180.0),
                record("Song", "Artist", 190.0),
            ],
            "Artist",
            "Song",
            180_000,
            None,
        )
        .unwrap();
        assert!(found.synced_lyrics.is_some());
    }
    #[test]
    fn isrc_validation_and_reused_identifier() {
        assert!(valid_isrc("bad/code").is_none());
        assert_eq!(
            valid_isrc("GB-AYE-92-00070").as_deref(),
            Some("GBAYE9200070")
        );
        let data = serde_json::json!({"recordings":[{"title":"One","length":180000,"artist-credit":[{"name":"Artist"}]},{"title":"Two","length":180000,"artist-credit":[{"name":"Artist"}]}]});
        assert!(recording_identity(&data, "upload", 180_000).is_none());
    }
    #[test]
    fn isrc_fixes_an_arbitrary_uploader_title() {
        let data = serde_json::json!({"recordings":[{"title":"Song","length":180000,"artist-credit":[{"name":"Artist"}]}]});
        assert_eq!(
            recording_identity(&data, "upload 42", 180_000),
            Some(("Artist".into(), "Song".into()))
        );
    }
    #[test]
    fn genius_preserves_lines_and_entities_and_skips_controls() {
        let html = r#"<div data-lyrics-container="true"><div class="LyricsHeader_test">Header</div>[Verse]<br>One &amp; <a href="/">two</a><br><span data-exclude-from-selection="true">Controls</span>Three</div><div data-lyrics-container="true">Four<br>Five</div>"#;
        assert_eq!(
            genius_text(html).as_deref(),
            Some("[Verse]\nOne & two\nThree\nFour\nFive")
        );
        assert!(genius_text("<html><body>Verify your browser</body></html>").is_none());
    }
}
