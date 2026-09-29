use serde::{Deserialize, Serialize};
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
    #[serde(default)] pub instrumental: bool,
    #[serde(default = "lrclib_source")] pub source: String,
}

fn lrclib_source() -> String { "LRCLIB".into() }

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(7))
        .user_agent("Fastcloud/0.1 (https://github.com/zxcloli666)")
        .build().map_err(|error| error.to_string())
}

async fn send_lrclib(request: impl Fn() -> reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
    for attempt in 0..3 {
        let response = request().send().await.map_err(|error| error.to_string())?;
        if matches!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE | reqwest::StatusCode::TOO_MANY_REQUESTS)
            && attempt < 2 {
            let seconds = response.headers().get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(1).clamp(1, 3);
            tokio::time::sleep(Duration::from_secs(seconds)).await;
            continue;
        }
        return Ok(response);
    }
    unreachable!("the final LRCLIB attempt always returns")
}

fn comparable(value: &str) -> String {
    value.chars().flat_map(char::to_lowercase).filter(|ch| ch.is_alphanumeric()).collect()
}

fn clean_artist(value: &str) -> String {
    value.chars().filter(|ch| ch.is_alphanumeric() || ch.is_whitespace() || *ch == '&')
        .collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn candidate_pairs(artist: &str, title: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for separator in [" - ", " – ", " — ", " | "] {
        if let Some((prefix, song)) = title.split_once(separator) {
            if !prefix.trim().is_empty() && !song.trim().is_empty() && prefix.len() < 70 {
                pairs.push((clean_artist(prefix), song.trim().to_string()));
                break;
            }
        }
    }
    let original = (clean_artist(artist), title.trim().to_string());
    if !pairs.iter().any(|pair| comparable(&pair.0) == comparable(&original.0)
        && comparable(&pair.1) == comparable(&original.1)) { pairs.push(original); }
    pairs
}

fn match_score(record: &LyricsRecord, artist: &str, title: &str, duration_ms: u64) -> i32 {
    let found_title = comparable(&record.track_name);
    let found_artist = comparable(&record.artist_name);
    let wanted_title = comparable(title);
    let wanted_artist = comparable(artist);
    if wanted_title.len() < 3 || wanted_artist.len() < 2 { return 0; }
    let title_points = if found_title == wanted_title { 60 }
        else if found_title.contains(&wanted_title) || wanted_title.contains(&found_title) { 42 }
        else { 0 };
    let artist_points = if found_artist == wanted_artist { 40 }
        else if found_artist.contains(&wanted_artist) || wanted_artist.contains(&found_artist) { 26 }
        else { 0 };
    if title_points == 0 || artist_points == 0 { return 0; }
    let duration_points = record.duration.map(|seconds| {
        let difference = (seconds * 1000.0 - duration_ms as f64).abs();
        if difference <= 2500.0 { 30 } else if difference <= 8000.0 { 20 }
        else if difference <= 20000.0 { 8 } else { 0 }
    }).unwrap_or(0);
    title_points + artist_points + duration_points
}

async fn exact(client: &reqwest::Client, artist: &str, title: &str, duration_ms: u64) -> Result<Option<LyricsRecord>, String> {
    let seconds = (duration_ms / 1000).to_string();
    let response = send_lrclib(|| client.get("https://lrclib.net/api/get")
        .query(&[("artist_name", artist), ("track_name", title), ("duration", seconds.as_str())])).await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND { return Ok(None); }
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS { return Err("LRCLIB rate limit".into()); }
    if !response.status().is_success() { return Err(format!("LRCLIB HTTP {}", response.status())); }
    response.json::<LyricsRecord>().await.map(Some).map_err(|error| error.to_string())
}

async fn search_structured(client: &reqwest::Client, artist: &str, title: &str, duration_ms: u64) -> Result<Option<LyricsRecord>, String> {
    let response = send_lrclib(|| client.get("https://lrclib.net/api/search")
        .query(&[("artist_name", artist), ("track_name", title)])).await?;
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS { return Err("LRCLIB rate limit".into()); }
    if !response.status().is_success() { return Err(format!("LRCLIB HTTP {}", response.status())); }
    let entries = response.json::<Vec<LyricsRecord>>().await.map_err(|error| error.to_string())?;
    let mut best = entries.into_iter().filter(|entry| entry.instrumental || entry.plain_lyrics.is_some() || entry.synced_lyrics.is_some())
        .map(|entry| (match_score(&entry, artist, title, duration_ms), entry))
        .filter(|(score, _)| *score >= 80)
        .max_by_key(|(score, entry)| (*score, entry.synced_lyrics.is_some() as i32))
        .map(|(_, entry)| entry);
    if let Some(entry) = best.as_mut() {
        if entry.plain_lyrics.is_some() && entry.duration.is_some_and(|seconds| (seconds * 1000.0 - duration_ms as f64).abs() > 8000.0) {
            entry.synced_lyrics = None;
        }
    }
    Ok(best)
}

async fn lyrics_ovh(client: &reqwest::Client, artist: &str, title: &str) -> Result<Option<LyricsRecord>, String> {
    let mut url = reqwest::Url::parse("https://api.lyrics.ovh/v1/").map_err(|error| error.to_string())?;
    url.path_segments_mut().map_err(|_| "Invalid lyrics URL")?.push(artist).push(title);
    let response = client.get(url).send().await.map_err(|error| error.to_string())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND { return Ok(None); }
    if !response.status().is_success() { return Err(format!("lyrics.ovh HTTP {}", response.status())); }
    let data = response.json::<serde_json::Value>().await.map_err(|error| error.to_string())?;
    let Some(plain) = data.get("lyrics").and_then(serde_json::Value::as_str).map(str::trim).filter(|text| !text.is_empty()) else { return Ok(None) };
    Ok(Some(LyricsRecord { id: 0, track_name: title.into(), artist_name: artist.into(), album_name: None,
        duration: None, plain_lyrics: Some(plain.into()), synced_lyrics: None, instrumental: false, source: "lyrics.ovh".into() }))
}

pub async fn lookup(artist: String, title: String, duration_ms: u64) -> Result<Option<LyricsRecord>, String> {
    if artist.trim().is_empty() || title.trim().is_empty() { return Ok(None); }
    let client = client()?;
    let pairs = candidate_pairs(&artist, &title);
    let mut failure = None;
    for (index, (performer, song)) in pairs.iter().enumerate() {
        if index > 0 { tokio::time::sleep(Duration::from_millis(250)).await; }
        match exact(&client, performer, song, duration_ms).await {
            Ok(Some(record)) => return Ok(Some(record)), Ok(None) => {},
            Err(error) => { failure = Some(error); break; },
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
        match search_structured(&client, performer, song, duration_ms).await {
            Ok(Some(record)) => return Ok(Some(record)), Ok(None) => {},
            Err(error) => { failure = Some(error); break; },
        }
    }
    for (performer, song) in &pairs {
        match lyrics_ovh(&client, performer, song).await {
            Ok(Some(record)) => return Ok(Some(record)),
            Ok(None) => {},
            Err(error) => { if failure.is_none() { failure = Some(error); } },
        }
    }
    match failure { Some(error) => Err(error), None => Ok(None) }
}

pub async fn search(query: String) -> Result<Vec<LyricsRecord>, String> {
    let query = query.trim();
    if query.chars().count() < 2 { return Ok(Vec::new()); }
    let client = client()?;
    let response = send_lrclib(|| client.get("https://lrclib.net/api/search")
        .query(&[("q", query)])).await?;
    if !response.status().is_success() { return Err(format!("LRCLIB HTTP {}", response.status())); }
    let mut entries = response.json::<Vec<LyricsRecord>>().await.map_err(|error| error.to_string())?;
    entries.truncate(20);
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_decorated_soundcloud_artist() {
        assert_eq!(candidate_pairs("☆LiL PEEP☆", "save that shit")[0], ("LiL PEEP".into(), "save that shit".into()));
        assert_eq!(candidate_pairs("Uploader", "Lil Peep - Star Shopping")[0], ("Lil Peep".into(), "Star Shopping".into()));
    }

    #[test]
    fn rejects_unrelated_song() {
        let record = LyricsRecord { id: 1, track_name: "Another Song".into(), artist_name: "Lil Peep".into(), album_name: None, duration: Some(141.0), plain_lyrics: Some("text".into()), synced_lyrics: None, instrumental: false, source: lrclib_source() };
        assert_eq!(match_score(&record, "Lil Peep", "Star Shopping", 141_000), 0);
    }
}
