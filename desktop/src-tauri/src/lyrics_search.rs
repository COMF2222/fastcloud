//! Search an actual lyric passage, then corroborate the SoundCloud recording.
use crate::{
    api::models::Track,
    lyrics::{
        self, LyricsRecord, candidate_pairs, comparable, genius_title_keys, metadata_text,
        same_artist, version_tags,
    },
};
use scraper::Html;
use serde_json::Value;
use std::{
    collections::{HashSet, VecDeque},
    sync::OnceLock,
};

// Bounded session index, without credentials or persistent user data.
static RECENT: OnceLock<parking_lot::Mutex<VecDeque<LyricsRecord>>> = OnceLock::new();

pub fn remember(record: &LyricsRecord) {
    if record.instrumental
        || record.plain_lyrics.as_ref().map_or(0, String::len)
            + record.synced_lyrics.as_ref().map_or(0, String::len)
            > 128 * 1024
    {
        return;
    }
    let mut cache = RECENT.get_or_init(Default::default).lock();
    cache
        .retain(|old| old.track_name != record.track_name || old.artist_name != record.artist_name);
    cache.push_front(record.clone());
    cache.truncate(64);
}

#[derive(Clone, Debug)]
pub struct Match {
    pub record: LyricsRecord,
    pub excerpt: String,
    pub score: u16,
    artists: Vec<String>,
}

fn words(value: &str) -> Vec<String> {
    metadata_text(value)
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(comparable)
        .collect()
}

fn near_word(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    let left: Vec<_> = left.chars().collect();
    let right: Vec<_> = right.chars().collect();
    if left.len().min(right.len()) < 5 || left.len().abs_diff(right.len()) > 1 {
        return false;
    }
    let (mut a, mut b, mut edits) = (0, 0, 0);
    while a < left.len() && b < right.len() {
        if left[a] == right[b] {
            a += 1;
            b += 1;
            continue;
        }
        edits += 1;
        if edits > 1 {
            return false;
        }
        match left.len().cmp(&right.len()) {
            std::cmp::Ordering::Less => b += 1,
            std::cmp::Ordering::Greater => a += 1,
            std::cmp::Ordering::Equal => {
                a += 1;
                b += 1;
            }
        }
    }
    edits + usize::from(a < left.len() || b < right.len()) <= 1
}

// Ordered, consecutive words: common words scattered through a song do not
// prove a match. One typo is allowed only in a longer otherwise exact phrase.
fn fragment_score(query: &str, text: &str) -> u16 {
    let wanted = words(query);
    let words = words(text);
    if wanted.is_empty() || words.len() < wanted.len() {
        return 0;
    }
    words
        .windows(wanted.len())
        .map(|window| {
            let exact = window.iter().zip(&wanted).filter(|(a, b)| a == b).count();
            if exact == wanted.len() {
                return 1000;
            }
            if wanted.len() >= 4
                && exact + 1 == wanted.len()
                && window.iter().zip(&wanted).all(|(a, b)| near_word(a, b))
            {
                return 850;
            }
            0
        })
        .max()
        .unwrap_or(0)
}

fn excerpt(query: &str, text: &str) -> String {
    let words: Vec<_> = text.split_whitespace().collect();
    let size = self::words(query).len();
    let best = fragment_score(query, text);
    let start = (0..words.len())
        .find(|&start| {
            fragment_score(
                query,
                &words[start..words.len().min(start + size + 3)].join(" "),
            ) == best
        })
        .unwrap_or(0);
    words[start..words.len().min(start + size + 4)]
        .join(" ")
        .chars()
        .take(180)
        .collect()
}

fn record_match(query: &str, record: LyricsRecord) -> Option<Match> {
    if record.instrumental {
        return None;
    }
    let plain = record.plain_lyrics.as_deref().unwrap_or_default();
    let synced = record
        .synced_lyrics
        .as_deref()
        .unwrap_or_default()
        .lines()
        .map(|line| line.rsplit_once(']').map_or(line, |(_, text)| text))
        .collect::<Vec<_>>()
        .join(" ");
    let plain_score = fragment_score(query, plain);
    let synced_score = fragment_score(query, &synced);
    let (text, score) = if plain_score >= synced_score {
        (plain, plain_score)
    } else {
        (synced.as_str(), synced_score)
    };
    if score == 0 {
        return None;
    }
    Some(Match {
        excerpt: excerpt(query, text),
        score,
        artists: vec![record.artist_name.clone()],
        record,
    })
}

fn genius_hits(data: &Value, query: &str) -> Vec<Match> {
    let mut hits = Vec::new();
    if let Some(sections) = data.pointer("/response/sections").and_then(Value::as_array) {
        hits.extend(
            sections
                .iter()
                .filter(|section| section.get("type").and_then(Value::as_str) == Some("lyric"))
                .filter_map(|section| section.get("hits").and_then(Value::as_array))
                .flatten(),
        );
    }
    if let Some(direct) = data.pointer("/response/hits").and_then(Value::as_array) {
        hits.extend(
            direct
                .iter()
                .filter(|hit| hit.get("type").and_then(Value::as_str) == Some("lyric")),
        );
    }
    hits.into_iter()
        .take(20)
        .filter_map(|hit| {
            let song = hit.get("result")?;
            if song.get("instrumental").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            let url = reqwest::Url::parse(song.get("url")?.as_str()?).ok()?;
            if url.scheme() != "https"
                || url.host_str() != Some("genius.com")
                || !url.username().is_empty()
                || url.password().is_some()
                || url.port().is_some()
            {
                return None;
            }
            // The lyric index supplies matching passages, often wrapped in <em>.
            // Title highlights alone must never count as lyric evidence.
            let highlights = hit.get("highlights")?.as_array()?;
            let (score, text) = highlights
                .iter()
                .filter(|highlight| {
                    highlight
                        .get("property")
                        .and_then(Value::as_str)
                        .is_none_or(|property| property == "lyrics")
                })
                .filter_map(|highlight| {
                    let html = Html::parse_fragment(highlight.get("value")?.as_str()?);
                    let text = html.root_element().text().collect::<Vec<_>>().join("");
                    Some((fragment_score(query, &text), text))
                })
                .max_by_key(|(score, _)| *score)?;
            if score == 0 {
                return None;
            }
            let title = song.get("title")?.as_str()?.to_owned();
            let artist = song.pointer("/primary_artist/name")?.as_str()?.to_owned();
            let mut artists = vec![artist.clone()];
            for key in ["primary_artists", "featured_artists"] {
                if let Some(credits) = song.get(key).and_then(Value::as_array) {
                    artists.extend(
                        credits
                            .iter()
                            .filter_map(|credit| credit.get("name")?.as_str().map(str::to_owned)),
                    );
                }
            }
            Some(Match {
                excerpt: excerpt(query, &text),
                score,
                artists,
                record: LyricsRecord {
                    id: song.get("id").and_then(Value::as_u64).unwrap_or_default(),
                    track_name: title,
                    artist_name: artist,
                    album_name: None,
                    duration: None,
                    plain_lyrics: None,
                    synced_lyrics: None,
                    instrumental: false,
                    source: "Genius".into(),
                    source_url: Some(url.to_string()),
                },
            })
        })
        .collect()
}

async fn genius_search(client: &reqwest::Client, query: &str) -> Result<Vec<Match>, String> {
    genius_search_at(client, query, "https://genius.com/api/search/lyric").await
}

async fn genius_search_at(
    client: &reqwest::Client,
    query: &str,
    url: &str,
) -> Result<Vec<Match>, String> {
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, lyrics::GENIUS_USER_AGENT)
        .query(&[("q", query), ("per_page", "20")])
        .send()
        .await
        .map_err(|_| "Lyric search connection failed")?;
    if !response.status().is_success() {
        return Err(format!("Lyric search HTTP {}", response.status()));
    }
    let data = response
        .json::<Value>()
        .await
        .map_err(|_| "Invalid lyric search response")?;
    if data.pointer("/response/sections").is_none() && data.pointer("/response/hits").is_none() {
        return Err("Invalid lyric search response".into());
    }
    Ok(genius_hits(&data, query))
}

pub async fn search(query: &str) -> Result<Vec<Match>, String> {
    let query = metadata_text(query);
    if query.chars().count() < 4 || query.chars().count() > 200 {
        return Ok(Vec::new());
    }
    let records: Vec<_> = RECENT
        .get_or_init(Default::default)
        .lock()
        .iter()
        .cloned()
        .collect();
    let mut matches: Vec<_> = records
        .into_iter()
        .filter_map(|record| record_match(&query, record))
        .collect();
    let client = lyrics::client()?;
    let (genius, lrclib) = tokio::join!(
        genius_search(&client, &query),
        lyrics::search(query.clone())
    );
    let failure = genius.as_ref().err().cloned();
    if let Ok(found) = genius {
        matches.extend(found);
    }
    if let Ok(records) = lrclib {
        matches.extend(
            records
                .into_iter()
                .filter_map(|record| record_match(&query, record)),
        );
    }
    matches.sort_by_key(|item| std::cmp::Reverse(item.score));
    let mut seen = HashSet::new();
    matches.retain(|item| {
        seen.insert((
            comparable(&item.record.artist_name),
            comparable(&item.record.track_name),
        ))
    });
    matches.truncate(8);
    if matches.is_empty()
        && let Some(error) = failure
    {
        return Err(error);
    }
    Ok(matches)
}

pub fn track_score(found: &Match, track: &Track) -> u16 {
    if track.is_blocked() {
        return 0;
    }
    let keys = genius_title_keys(&found.record.track_name);
    let mut score = 0;
    for (performer, title) in candidate_pairs(track.artist(), &track.title) {
        if version_tags(&title) != version_tags(&found.record.track_name)
            || !genius_title_keys(&title)
                .iter()
                .any(|key| keys.contains(key))
            || !found
                .artists
                .iter()
                .any(|artist| same_artist(artist, &performer))
        {
            continue;
        }
        if let Some(seconds) = found
            .record
            .duration
            .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
            && track.effective_duration_ms() > 1000
            && (seconds * 1000.0 - track.effective_duration_ms() as f64).abs() > 12_000.0
        {
            continue;
        }
        score = 100 + u16::from(same_artist(track.artist(), &performer)) * 10;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(title: &str, artist: &str, text: &str) -> LyricsRecord {
        LyricsRecord {
            id: 1,
            track_name: title.into(),
            artist_name: artist.into(),
            album_name: None,
            duration: Some(180.0),
            plain_lyrics: Some(text.into()),
            synced_lyrics: None,
            instrumental: false,
            source: "LRCLIB".into(),
            source_url: None,
        }
    }

    fn hit(text: &str) -> Value {
        serde_json::json!({"response":{"sections":[{"type":"lyric","hits":[{
            "type":"lyric", "highlights":[{"property":"lyrics","value":text}],
            "result":{"id":1,"title":"Song","primary_artist":{"name":"Main & Guest"},
                "primary_artists":[{"name":"Main"},{"name":"Guest"}],"url":"https://genius.com/main-song-lyrics"}
        }]}]}})
    }

    #[test]
    fn fragments_match_both_languages_and_normalize_punctuation() {
        assert_eq!(
            fragment_score("моё тело, живёт", "Мое тело живет — и помнит"),
            1000
        );
        assert_eq!(
            fragment_score("I’m finding my way", "I'm finding my way home"),
            1000
        );
        assert_eq!(
            fragment_score("finding my way home", "finding some other way back home"),
            0
        );
        assert_eq!(
            fragment_score("моё тело живёт", "моё тело ... когда-то давно ... живёт"),
            0
        );
    }

    #[test]
    fn exact_passages_rank_above_one_typo_and_unrelated_words() {
        assert_eq!(
            fragment_score("finding my way home", "findign my way home"),
            0
        );
        assert_eq!(
            fragment_score("finding my way home", "findin my way home"),
            850
        );
        assert_eq!(
            fragment_score("finding my way home", "finding your way home"),
            0
        );
        assert!(
            fragment_score("finding my way home", "finding my way home")
                > fragment_score("finding my way home", "findin my way home")
        );
    }

    #[test]
    fn metadata_alone_does_not_prove_a_lyric_match_and_lrc_tags_do_not_interrupt_it() {
        assert!(
            record_match(
                "finding my way",
                record("finding my way", "Main", "Different words")
            )
            .is_none()
        );
        let mut synced = record("Song", "Main", "");
        synced.synced_lyrics = Some("[00:01.20]finding my\n[00:02.30]way home".into());
        assert_eq!(record_match("finding my way", synced).unwrap().score, 1000);
    }

    #[test]
    fn lyric_hits_require_real_passages_and_trusted_source_urls() {
        assert_eq!(
            genius_hits(&hit("<em>finding</em> my way home"), "finding my way").len(),
            1
        );
        let mut title_only = hit("finding my way home");
        title_only["response"]["sections"][0]["hits"][0]["highlights"][0]["property"] =
            Value::String("title".into());
        assert!(genius_hits(&title_only, "finding my way").is_empty());
        let mut unsafe_url = hit("finding my way home");
        unsafe_url["response"]["sections"][0]["hits"][0]["result"]["url"] =
            Value::String("https://genius.com.evil.test/song".into());
        assert!(genius_hits(&unsafe_url, "finding my way").is_empty());
        assert!(genius_hits(&hit("different passage"), "finding my way").is_empty());
    }

    #[test]
    fn track_resolution_rejects_other_artists_versions_and_durations() {
        let found = record_match(
            "finding my way",
            record("Song", "Main", "finding my way home"),
        )
        .unwrap();
        let track = |title: &str, artist: &str, duration: u64| {
            serde_json::from_value::<Track>(serde_json::json!({
            "id":1,"title":title,"metadata_artist":artist,"duration":duration}))
            .unwrap()
        };
        assert!(track_score(&found, &track("Song (Official Audio)", "Main", 180_000)) > 0);
        assert!(track_score(&found, &track("Main - Song", "Uploader", 180_000)) > 0);
        assert_eq!(track_score(&found, &track("Song", "Wrong", 180_000)), 0);
        assert_eq!(
            track_score(&found, &track("Song (Remix)", "Main", 180_000)),
            0
        );
        assert_eq!(track_score(&found, &track("Song", "Main", 90_000)), 0);
        let joint = genius_hits(&hit("finding my way home"), "finding my way").remove(0);
        assert!(track_score(&joint, &track("Song", "Guest", 180_000)) > 0);
    }

    #[tokio::test]
    async fn provider_failure_is_not_reported_as_no_results() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for (status, body, expected) in [
            ("403 Forbidden", "denied", "HTTP 403"),
            ("200 OK", "{bad", "Invalid lyric"),
            ("200 OK", "{}", "Invalid lyric"),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 2048];
                socket.read(&mut request).await.unwrap();
                socket.write_all(response.as_bytes()).await.unwrap();
            });
            let error = genius_search_at(
                &lyrics::client().unwrap(),
                "finding my way",
                &format!("http://{address}/api/search/lyric"),
            )
            .await
            .unwrap_err();
            assert!(error.contains(expected), "{error}");
            server.await.unwrap();
        }
    }
}
