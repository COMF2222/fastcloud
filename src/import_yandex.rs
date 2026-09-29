//! One-shot import of liked Yandex Music tracks into a SoundCloud playlist.

use std::{collections::HashSet, sync::Arc};

use anyhow::{Context as _, Result};
use crossbeam_channel::Sender;
use serde::Deserialize;

use crate::api::{ApiClient, endpoints};

#[derive(Debug)]
pub enum Event {
    Progress {
        current: usize,
        total: usize,
        matched: usize,
        title: String,
    },
    Finished {
        track_ids: Vec<u64>,
        not_found: usize,
    },
    Failed(String),
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    result: StatusResult,
}

#[derive(Debug, Deserialize)]
struct StatusResult {
    account: Account,
}

#[derive(Debug, Deserialize)]
struct Account {
    uid: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct LikesResponse {
    result: LikesResult,
}

#[derive(Debug, Deserialize)]
struct LikesResult {
    library: LikesLibrary,
}

#[derive(Debug, Deserialize)]
struct LikesLibrary {
    tracks: Vec<LikeRef>,
}

#[derive(Debug, Deserialize)]
struct LikeRef {
    id: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct TrackResponse {
    result: Vec<YandexTrack>,
}

#[derive(Debug, Deserialize)]
struct YandexTrack {
    title: String,
    #[serde(default, rename = "durationMs")]
    duration_ms: Option<u64>,
    #[serde(default)]
    artists: Vec<YandexArtist>,
}

#[derive(Debug, Deserialize)]
struct YandexArtist {
    name: String,
}

pub async fn import_likes(token: String, soundcloud: Arc<ApiClient>, tx: Sender<Event>) {
    if let Err(error) = run(&token, soundcloud, &tx).await {
        let _ = tx.send(Event::Failed(error.to_string()));
    }
}

async fn run(token: &str, soundcloud: Arc<ApiClient>, tx: &Sender<Event>) -> Result<()> {
    let http = reqwest::Client::builder()
        .user_agent(concat!("fastcloud/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let authorization = format!("OAuth {}", token.trim());
    let status: StatusResponse = http
        .get("https://api.music.yandex.net/account/status")
        .header(reqwest::header::AUTHORIZATION, &authorization)
        .send()
        .await?
        .error_for_status()
        .context("Yandex Music authorization failed")?
        .json()
        .await?;
    let uid = json_scalar(&status.result.account.uid).context("Yandex user id is missing")?;
    let likes: LikesResponse = http
        .get(format!(
            "https://api.music.yandex.net/users/{uid}/likes/tracks"
        ))
        .header(reqwest::header::AUTHORIZATION, &authorization)
        .send()
        .await?
        .error_for_status()
        .context("cannot load Yandex Music likes")?
        .json()
        .await?;
    let ids: Vec<_> = likes
        .result
        .library
        .tracks
        .iter()
        .filter_map(|item| json_scalar(&item.id))
        .collect();
    let total = ids.len();
    let mut matched_ids = Vec::new();
    let mut matched_seen = HashSet::new();
    let mut matched = 0usize;
    let mut processed = 0usize;
    for chunk in ids.chunks(50) {
        let response: TrackResponse = http
            .get("https://api.music.yandex.net/tracks")
            .query(&[("trackIds", chunk.join(","))])
            .header(reqwest::header::AUTHORIZATION, &authorization)
            .send()
            .await?
            .error_for_status()
            .context("cannot load Yandex Music track metadata")?
            .json()
            .await?;
        for track in response.result {
            let artist = track
                .artists
                .first()
                .map(|artist| artist.name.as_str())
                .unwrap_or_default();
            let query = format!("{artist} {}", track.title);
            let mut candidates = endpoints::search_tracks(&soundcloud, &query).await;
            let page = candidates.next_page().await.unwrap_or_default();
            if let Some(found) = best_match(&track, &page) {
                matched += 1;
                if matched_seen.insert(found.id) {
                    matched_ids.push(found.id);
                }
            }
            processed += 1;
            let _ = tx.send(Event::Progress {
                current: processed,
                total,
                matched,
                title: query,
            });
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        }
    }
    let not_found = total.saturating_sub(matched);
    let _ = tx.send(Event::Finished {
        track_ids: matched_ids,
        not_found,
    });
    Ok(())
}

fn json_scalar(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn best_match<'a>(
    source: &YandexTrack,
    candidates: &'a [crate::api::models::Track],
) -> Option<&'a crate::api::models::Track> {
    let title = normalize(&source.title);
    let artist = source
        .artists
        .first()
        .map(|artist| normalize(&artist.name))
        .unwrap_or_default();
    candidates
        .iter()
        .map(|candidate| {
            let candidate_title = normalize(&candidate.title);
            let candidate_artist = normalize(candidate.artist());
            let title_score = if candidate_title == title {
                6
            } else if candidate_title.contains(&title) || title.contains(&candidate_title) {
                3
            } else {
                0
            };
            let artist_score = if !artist.is_empty() && candidate_artist == artist {
                4
            } else if !artist.is_empty()
                && (candidate_artist.contains(&artist) || artist.contains(&candidate_artist))
            {
                2
            } else {
                0
            };
            let duration_score = source.duration_ms.map_or(0, |duration| {
                (candidate.effective_duration_ms().abs_diff(duration) <= 6_000) as i32 * 2
            });
            (candidate, title_score + artist_score + duration_score)
        })
        .filter(|(_, score)| *score >= 6)
        .max_by_key(|(_, score)| *score)
        .map(|(candidate, _)| candidate)
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(title: &str, artist: &str, duration_ms: u64) -> YandexTrack {
        YandexTrack {
            title: title.to_owned(),
            duration_ms: Some(duration_ms),
            artists: vec![YandexArtist {
                name: artist.to_owned(),
            }],
        }
    }

    fn candidate(
        id: u64,
        title: &str,
        artist: &str,
        duration_ms: u64,
    ) -> crate::api::models::Track {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "title": title,
            "duration": duration_ms,
            "metadata_artist": artist
        }))
        .unwrap()
    }

    #[test]
    fn best_match_prefers_the_same_title_artist_and_duration() {
        let source = source("Midnight (Radio Edit)", "Alice", 180_000);
        let candidates = [
            candidate(1, "Midnight", "Someone", 180_000),
            candidate(2, "Midnight (Radio Edit)", "Alice", 181_000),
        ];

        let result = best_match(&source, &candidates).unwrap();

        assert_eq!(result.id, 2);
    }

    #[test]
    fn best_match_rejects_an_unrelated_first_search_result() {
        let source = source("Midnight", "Alice", 180_000);
        let candidates = [candidate(1, "Sunrise", "Bob", 240_000)];

        let result = best_match(&source, &candidates);

        assert!(result.is_none());
    }
}
