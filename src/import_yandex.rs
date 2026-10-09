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
    result: Vec<Option<YandexTrack>>,
}

#[derive(Debug, Deserialize)]
struct YandexTrack {
    #[serde(default)]
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

fn http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("fastcloud/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}

pub async fn check_token(token: &str) -> Result<usize> {
    let http = http_client()?;
    Ok(load_liked_ids(&http, token).await?.len())
}

async fn load_liked_ids(http: &reqwest::Client, token: &str) -> Result<Vec<String>> {
    load_liked_ids_at(http, token, "https://api.music.yandex.net").await
}

async fn load_liked_ids_at(http: &reqwest::Client, token: &str, base: &str) -> Result<Vec<String>> {
    anyhow::ensure!(!token.trim().is_empty(), "Paste a Yandex Music OAuth token");
    let authorization = format!("OAuth {}", token.trim());
    let status: StatusResponse = http
        .get(format!("{base}/account/status"))
        .header(reqwest::header::AUTHORIZATION, &authorization)
        .send()
        .await?
        .error_for_status()
        .context("Yandex Music authorization failed")?
        .json()
        .await?;
    let uid = json_scalar(&status.result.account.uid).context("Yandex user id is missing")?;
    let likes: LikesResponse = http
        .get(format!("{base}/users/{uid}/likes/tracks"))
        .header(reqwest::header::AUTHORIZATION, &authorization)
        .send()
        .await?
        .error_for_status()
        .context("cannot load Yandex Music likes")?
        .json()
        .await?;
    let mut seen = HashSet::new();
    Ok(likes
        .result
        .library
        .tracks
        .iter()
        .filter_map(|item| json_scalar(&item.id))
        .filter(|id| seen.insert(id.clone()))
        .collect())
}

async fn run(token: &str, soundcloud: Arc<ApiClient>, tx: &Sender<Event>) -> Result<()> {
    run_at(
        token,
        soundcloud,
        tx,
        "https://api.music.yandex.net",
        std::time::Duration::from_millis(120),
    )
    .await
}

async fn run_at(
    token: &str,
    soundcloud: Arc<ApiClient>,
    tx: &Sender<Event>,
    base: &str,
    delay: std::time::Duration,
) -> Result<()> {
    let http = http_client()?;
    let ids = load_liked_ids_at(&http, token, base).await?;
    let total = ids.len();
    let mut matched_ids = Vec::new();
    let mut matched_seen = HashSet::new();
    let mut matched = 0usize;
    let mut processed = 0usize;
    for chunk in ids.chunks(50) {
        let response = load_tracks(&http, token, base, chunk).await?;
        let missing_metadata = chunk.len().saturating_sub(response.result.len());
        for track in response.result {
            let Some(track) = track.filter(|track| !track.title.trim().is_empty()) else {
                processed += 1;
                let _ = tx.send(Event::Progress {
                    current: processed,
                    total,
                    matched,
                    title: String::new(),
                });
                continue;
            };
            let artist = track
                .artists
                .first()
                .map(|artist| artist.name.as_str())
                .unwrap_or_default();
            let query = format!("{artist} {}", track.title);
            let mut candidates = endpoints::search_tracks(&soundcloud, &query).await;
            // A failed request is not a missing song. Keep the error visible;
            // do not produce an apparently successful empty import.
            let page = candidates
                .next_page()
                .await
                .context("SoundCloud search failed; import stopped")?;
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
            tokio::time::sleep(delay).await;
        }
        processed += missing_metadata;
        let _ = tx.send(Event::Progress {
            current: processed,
            total,
            matched,
            title: String::new(),
        });
    }
    let not_found = total.saturating_sub(matched);
    let _ = tx.send(Event::Finished {
        track_ids: matched_ids,
        not_found,
    });
    Ok(())
}

async fn load_tracks(
    http: &reqwest::Client,
    token: &str,
    base: &str,
    ids: &[String],
) -> Result<TrackResponse> {
    // Yandex's batch endpoint takes form data, rather than a long query URL.
    http.post(format!("{base}/tracks"))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("OAuth {}", token.trim()),
        )
        .form(&[("track-ids", ids.join(","))])
        .send()
        .await?
        .error_for_status()
        .context("cannot load Yandex Music track metadata")?
        .json()
        .await
        .context("cannot read Yandex Music track metadata")
}

pub const PLAYLIST_LIMIT: usize = 500;

pub fn playlist_name(part: usize, total: usize) -> String {
    if total > PLAYLIST_LIMIT {
        format!("Yandex Music likes - part {}", part + 1)
    } else {
        "Yandex Music likes".into()
    }
}

#[derive(Debug, Default)]
pub struct SavedPlaylists {
    pub added: usize,
    pub parts: usize,
    pub error: Option<String>,
}

/// Confirm each part before counting it as imported. Preserve earlier parts on
/// failure; retrying a playlist POST could create duplicate playlists.
pub async fn save_playlists(
    client: &Arc<ApiClient>,
    ids: &[u64],
    mut check: impl FnMut() -> Result<()>,
    mut created: impl FnMut(),
) -> SavedPlaylists {
    let mut report = SavedPlaylists::default();
    for (part, chunk) in ids.chunks(PLAYLIST_LIMIT).enumerate() {
        let name = playlist_name(part, ids.len());
        let result: Result<()> = async {
            check()?;
            let playlist = endpoints::create_playlist(client, &name, false, chunk).await?;
            created();
            let saved = endpoints::playlist_tracks(client, playlist.id)
                .await
                .collect_all()
                .await?;
            let saved_ids: Vec<u64> = saved.iter().map(|track| track.id).collect();
            if saved_ids != chunk {
                // If creation silently omitted tracks, repair this same playlist
                // using an idempotent replacement, never another create request.
                check()?;
                endpoints::update_playlist(client, playlist.id, None, None, chunk).await?;
                created();
                let saved = endpoints::playlist_tracks(client, playlist.id)
                    .await
                    .collect_all()
                    .await?;
                anyhow::ensure!(
                    saved.iter().map(|track| track.id).eq(chunk.iter().copied()),
                    "SoundCloud did not save all tracks in playlist {}",
                    playlist.id
                );
            }
            Ok(())
        }
        .await;
        if let Err(error) = result {
            report.error = Some(format!("Could not save '{name}': {error}"));
            break;
        }
        report.added += chunk.len();
        report.parts += 1;
    }
    report
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

    #[derive(Clone, Copy, Default)]
    struct Failure {
        second_part: bool,
        empty_create: bool,
        empty_update: bool,
        search: bool,
    }

    type Requests = Arc<parking_lot::Mutex<Vec<(String, String, serde_json::Value)>>>;

    async fn fixture(failure: Failure) -> (String, Requests, tokio::task::JoinHandle<()>) {
        use serde_json::json;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests: Requests = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let task = tokio::spawn(async move {
            let mut playlists = std::collections::HashMap::<u64, Vec<u64>>::new();
            let mut creates = 0;
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut data = Vec::new();
                let mut buffer = [0; 8192];
                let end = loop {
                    let n = socket.read(&mut buffer).await.unwrap();
                    if n == 0 {
                        break None;
                    }
                    data.extend_from_slice(&buffer[..n]);
                    if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        break Some(end + 4);
                    }
                };
                let Some(end) = end else {
                    continue;
                };
                let header = String::from_utf8_lossy(&data[..end]).to_string();
                let length = header
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                while data.len() < end + length {
                    let n = socket.read(&mut buffer).await.unwrap();
                    assert!(n > 0);
                    data.extend_from_slice(&buffer[..n]);
                }
                let mut first = header.lines().next().unwrap().split_whitespace();
                let method = first.next().unwrap().to_owned();
                let path = first.next().unwrap().to_owned();
                let url = url::Url::parse(&format!("http://fixture{path}")).unwrap();
                let body: serde_json::Value = if length == 0 {
                    serde_json::Value::Null
                } else if header.contains("application/x-www-form-urlencoded") {
                    json!(
                        url::form_urlencoded::parse(&data[end..])
                            .into_owned()
                            .collect::<std::collections::HashMap<_, _>>()
                    )
                } else {
                    serde_json::from_slice(&data[end..]).unwrap()
                };
                recorded
                    .lock()
                    .push((method.clone(), path.clone(), body.clone()));
                let mut status = "200 OK";
                let response = if url.path() == "/account/status" {
                    json!({"result":{"account":{"uid":42}}})
                } else if url.path() == "/users/42/likes/tracks" {
                    json!({"result":{"library":{"tracks":[{"id":1},{"id":"2"},{"id":3},{"id":1}]}}})
                } else if url.path() == "/tracks" && method == "POST" {
                    assert!(
                        header
                            .to_ascii_lowercase()
                            .contains("authorization: oauth test-yandex")
                    );
                    assert!(url.query().is_none());
                    assert!(body["track-ids"].as_str().is_some());
                    json!({"result":[{"title":"One","durationMs":180000,"artists":[{"name":"Alice"}]},null]})
                } else if url.path().ends_with("/tracks")
                    && url.query_pairs().any(|(k, _)| k == "q")
                {
                    if failure.search {
                        status = "400 Bad Request";
                        json!({"error":"search failed"})
                    } else {
                        json!({"collection":[candidate(1,"One","Alice",180000)],"next_href":null})
                    }
                } else if url.path().ends_with("/playlists") && method == "POST" {
                    creates += 1;
                    if failure.second_part && creates == 2 {
                        status = "400 Bad Request";
                        json!({"error":"playlist failed"})
                    } else {
                        let tracks = body["playlist"]["tracks"].as_array().unwrap();
                        assert!(!tracks.is_empty() && tracks.len() <= 500);
                        let ids = tracks
                            .iter()
                            .map(|t| {
                                t["urn"]
                                    .as_str()
                                    .unwrap()
                                    .rsplit(':')
                                    .next()
                                    .unwrap()
                                    .parse()
                                    .unwrap()
                            })
                            .collect();
                        playlists.insert(
                            creates,
                            if failure.empty_create {
                                Vec::new()
                            } else {
                                ids
                            },
                        );
                        json!({"id":creates,"title":"Imported","tracks":[]})
                    }
                } else if method == "PUT" {
                    let id: u64 = url
                        .path()
                        .replace("%3A", ":")
                        .rsplit(':')
                        .next()
                        .unwrap()
                        .parse()
                        .unwrap();
                    let ids = body["playlist"]["tracks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|t| {
                            t["urn"]
                                .as_str()
                                .unwrap()
                                .rsplit(':')
                                .next()
                                .unwrap()
                                .parse()
                                .unwrap()
                        })
                        .collect();
                    playlists.insert(
                        id,
                        if failure.empty_update {
                            Vec::new()
                        } else {
                            ids
                        },
                    );
                    json!({"id":id,"title":"Imported","tracks":[]})
                } else if url.path().ends_with("/tracks") {
                    let id: u64 = url
                        .path()
                        .replace("%3A", ":")
                        .split('/')
                        .nth(5)
                        .unwrap()
                        .rsplit(':')
                        .next()
                        .unwrap()
                        .parse()
                        .unwrap();
                    let ids = &playlists[&id];
                    let offset = url
                        .query_pairs()
                        .find(|(k, _)| k == "offset")
                        .map(|(_, v)| v.parse().unwrap())
                        .unwrap_or(0);
                    let collection: Vec<_> = ids
                        .iter()
                        .skip(offset)
                        .take(100)
                        .map(|id| json!({"id":id,"title":"Song"}))
                        .collect();
                    let next = (offset + 100 < ids.len()).then(|| format!("https://api.soundcloud.com/playlists/soundcloud:playlists:{id}/tracks?offset={}",offset+100));
                    json!({"collection":collection,"next_href":next})
                } else {
                    panic!("Unexpected request: {method} {path}");
                };
                let response = response.to_string();
                socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{response}",response.len()).as_bytes()).await.unwrap();
            }
        });
        (base, requests, task)
    }

    async fn client(base: String) -> (Arc<ApiClient>, Arc<crate::auth::Session>) {
        let client = Arc::new(ApiClient::new(None, false));
        let session = crate::auth::Session::with_test_user(
            crate::auth::AppCredentials {
                client_id: "test".into(),
                client_secret: String::new(),
                redirect_uri: "http://127.0.0.1/callback".into(),
                server_url: Some(base),
            },
            client.clone(),
        )
        .await;
        (client, session)
    }

    #[tokio::test]
    async fn thousand_likes_are_saved_in_order_across_verified_parts() {
        let (base, requests, task) = fixture(Failure::default()).await;
        let (client, _session) = client(base).await;
        let ids: Vec<u64> = (1..=1003).collect();
        let report = save_playlists(&client, &ids, || Ok(()), || {}).await;
        task.abort();
        assert!(report.error.is_none(), "{:?}", report.error);
        assert_eq!((report.added, report.parts), (1003, 3));
        let requests = requests.lock();
        let writes: Vec<_> = requests.iter().filter(|(m, _, _)| m == "POST").collect();
        assert_eq!(
            writes
                .iter()
                .map(|(_, _, b)| b["playlist"]["tracks"].as_array().unwrap().len())
                .collect::<Vec<_>>(),
            [500, 500, 3]
        );
        let imported: Vec<u64> = writes
            .iter()
            .flat_map(|(_, _, b)| b["playlist"]["tracks"].as_array().unwrap())
            .map(|t| {
                t["urn"]
                    .as_str()
                    .unwrap()
                    .rsplit(':')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap()
            })
            .collect();
        assert_eq!(imported, ids);
        assert_eq!(
            writes[2].2["playlist"]["title"],
            "Yandex Music likes - part 3"
        );
        assert_eq!(writes[0].2["playlist"]["sharing"], "private");
    }

    #[tokio::test]
    async fn later_write_failure_preserves_confirmed_parts_and_is_not_retried() {
        let (base, requests, task) = fixture(Failure {
            second_part: true,
            ..Default::default()
        })
        .await;
        let (client, _session) = client(base).await;
        let report =
            save_playlists(&client, &(1..=1003).collect::<Vec<_>>(), || Ok(()), || {}).await;
        task.abort();
        assert_eq!((report.added, report.parts), (500, 1));
        assert!(report.error.unwrap().contains("part 2"));
        assert_eq!(
            requests
                .lock()
                .iter()
                .filter(|(m, _, _)| m == "POST")
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn silent_empty_creation_is_repaired_without_duplicate_playlist() {
        let (base, requests, task) = fixture(Failure {
            empty_create: true,
            ..Default::default()
        })
        .await;
        let (client, _session) = client(base).await;
        let report = save_playlists(&client, &[1, 2, 3], || Ok(()), || {}).await;
        task.abort();
        assert_eq!((report.added, report.parts), (3, 1));
        assert!(report.error.is_none());
        assert_eq!(
            requests
                .lock()
                .iter()
                .filter(|(m, _, _)| m == "POST")
                .count(),
            1
        );
        assert_eq!(
            requests
                .lock()
                .iter()
                .filter(|(m, _, _)| m == "PUT")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn playlist_that_remains_empty_is_never_reported_as_imported() {
        let (base, _, task) = fixture(Failure {
            empty_create: true,
            empty_update: true,
            ..Default::default()
        })
        .await;
        let (client, _session) = client(base).await;
        let report = save_playlists(&client, &[1, 2, 3], || Ok(()), || {}).await;
        task.abort();
        assert_eq!((report.added, report.parts), (0, 0));
        assert!(report.error.unwrap().contains("did not save all tracks"));
    }

    #[tokio::test]
    async fn yandex_batch_uses_form_data_and_unavailable_metadata_finishes_progress() {
        let (base, requests, task) = fixture(Failure::default()).await;
        let (client, _session) = client(base.clone()).await;
        let (tx, rx) = crossbeam_channel::unbounded();
        run_at("test-yandex", client, &tx, &base, std::time::Duration::ZERO)
            .await
            .unwrap();
        task.abort();
        let events: Vec<_> = rx.try_iter().collect();
        assert!(
            matches!(events.last(), Some(Event::Finished {track_ids,not_found:2}) if track_ids == &[1])
        );
        assert!(events.iter().any(|e| matches!(
            e,
            Event::Progress {
                current: 3,
                total: 3,
                ..
            }
        )));
        let requests = requests.lock();
        let (_, _, body) = requests
            .iter()
            .find(|(m, p, _)| m == "POST" && p == "/tracks")
            .unwrap();
        assert_eq!(body["track-ids"], "1,2,3");
    }

    #[tokio::test]
    async fn search_failure_does_not_masquerade_as_no_matching_tracks() {
        let (base, _, task) = fixture(Failure {
            search: true,
            ..Default::default()
        })
        .await;
        let (client, _session) = client(base.clone()).await;
        let (tx, rx) = crossbeam_channel::unbounded();
        let error = run_at("test-yandex", client, &tx, &base, std::time::Duration::ZERO)
            .await
            .unwrap_err();
        task.abort();
        assert!(error.to_string().contains("SoundCloud search failed"));
        assert!(!rx.try_iter().any(|e| matches!(e, Event::Finished { .. })));
    }

    #[tokio::test]
    async fn account_change_stops_writes_before_creating_another_part() {
        let (base, requests, task) = fixture(Failure::default()).await;
        let (client, _session) = client(base).await;
        let mut checks = 0;
        let report = save_playlists(
            &client,
            &(1..=501).collect::<Vec<_>>(),
            || {
                checks += 1;
                anyhow::ensure!(checks == 1, "account changed");
                Ok(())
            },
            || {},
        )
        .await;
        task.abort();
        assert_eq!((report.added, report.parts), (500, 1));
        assert!(report.error.unwrap().contains("account changed"));
        assert_eq!(
            requests
                .lock()
                .iter()
                .filter(|(m, _, _)| m == "POST")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn no_matches_do_not_create_an_empty_playlist() {
        let (base, requests, task) = fixture(Failure::default()).await;
        let (client, _session) = client(base).await;
        let report = save_playlists(&client, &[], || Ok(()), || {}).await;
        task.abort();
        assert_eq!((report.added, report.parts), (0, 0));
        assert!(report.error.is_none());
        assert!(requests.lock().is_empty());
    }
}
