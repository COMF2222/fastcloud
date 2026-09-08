// Probe every path the app asks for on startup, with this machine's stored
// credentials, to find what actually answers 404 or 500. Not part of the app.
use chrono::Utc;
#[path = "../src/audio/cache.rs"]
mod cache;
#[path = "../src/audio/decode.rs"]
mod decode;
#[path = "../src/audio/hls.rs"]
mod hls;
#[path = "../src/api/models.rs"]
mod models;

type Probe = (&'static str, &'static str, Vec<(&'static str, String)>);

fn secret(key: &str) -> Option<String> {
    keyring::Entry::new("com.fastcloud.tokens", key)
        .ok()?
        .get_password()
        .ok()
}

#[tokio::main]
async fn main() {
    let id = secret("client_id").expect("client_id in keyring");
    let sec = secret("client_secret").expect("client_secret in keyring");
    println!(
        "Redirect URI: {}",
        secret("redirect_uri").unwrap_or_else(|| "<missing>".into())
    );
    let http = reqwest::Client::new();

    if std::env::var_os("FASTCLOUD_PROBE_AUTHORIZE").is_some() {
        let redirect = secret("redirect_uri").expect("redirect_uri in keyring");
        let mut url = reqwest::Url::parse("https://secure.soundcloud.com/authorize").unwrap();
        url.query_pairs_mut()
            .append_pair("client_id", &id)
            .append_pair("redirect_uri", &redirect)
            .append_pair("response_type", "code")
            .append_pair(
                "code_challenge",
                "0123456789012345678901234567890123456789012",
            )
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", "probe");
        let probe = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
            .get(url)
            .send()
            .await
            .unwrap();
        println!("Authorize status: {}", probe.status());
        println!(
            "Authorize location: {}",
            probe
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| reqwest::Url::parse(value).ok())
                .map(|url| format!(
                    "{}://{}{}",
                    url.scheme(),
                    url.host_str().unwrap_or(""),
                    url.path()
                ))
                .unwrap_or_else(|| "<none>".into())
        );
        let body = probe.text().await.unwrap_or_default();
        let title = body
            .split("<title>")
            .nth(1)
            .and_then(|rest| rest.split("</title>").next())
            .unwrap_or("<none>");
        println!("Authorize title: {title}");
        return;
    }

    let saved = secret("oauth").and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    let grant = saved
        .as_ref()
        .and_then(|saved| saved.get("grant"))
        .and_then(|grant| grant.as_str())
        .unwrap_or("unknown");
    println!("Stored grant: {grant}");
    let use_user = saved
        .as_ref()
        .and_then(|saved| saved.get("expires_at"))
        .and_then(|value| value.as_i64())
        .is_some_and(|expires| expires > Utc::now().timestamp() + 120);
    let token = if use_user {
        saved
            .as_ref()
            .and_then(|saved| saved.get("access_token"))
            .and_then(|token| token.as_str())
            .expect("stored user token")
            .to_owned()
    } else {
        let basic = {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(format!("{id}:{sec}"))
        };
        let body: serde_json::Value = http
            .post("https://secure.soundcloud.com/oauth/token")
            .header("Accept", "application/json; charset=utf-8")
            .header("Authorization", format!("Basic {basic}"))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await
            .expect("token request")
            .json()
            .await
            .unwrap_or_default();
        body.get("access_token")
            .and_then(|token| token.as_str())
            .expect("an app token")
            .to_owned()
    };

    let from = (Utc::now() - chrono::Duration::days(30))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let partition = ("linked_partitioning", "true".to_owned());

    let registration_me = http
        .get("https://api-reg.soundcloud.com/me")
        .header("Accept", "application/json; charset=utf-8")
        .header("Authorization", format!("OAuth {token}"))
        .send()
        .await
        .expect("registration profile request");
    let registration_status = registration_me.status();
    let registration_body = registration_me.text().await.unwrap_or_default();
    println!(
        "api-reg /me: {registration_status} {}",
        &registration_body[..registration_body.len().min(500)]
    );

    // Exactly what `store::fetch` asks for, key by key.
    let probes: Vec<Probe> = vec![
        ("Me", "/me", vec![]),
        (
            "MyLikes",
            "/me/likes/tracks",
            vec![("access", "playable,preview".into()), partition.clone()],
        ),
        (
            "MyPlaylists",
            "/me/playlists",
            vec![("show_tracks", "false".into()), partition.clone()],
        ),
        (
            "MyLikedPlaylists",
            "/me/likes/playlists",
            vec![("limit", "200".into()), partition.clone()],
        ),
        ("Following", "/me/followings", vec![]),
        (
            "Feed",
            "/me/feed",
            vec![("access", "playable,preview".into()), partition.clone()],
        ),
        ("History", "/me/recently-played/tracks", vec![]),
        (
            "SearchTracks",
            "/tracks",
            vec![
                ("q", "house".into()),
                ("access", "playable,preview".into()),
                partition.clone(),
            ],
        ),
        (
            "Genre",
            "/tracks",
            vec![
                ("genres", "House".into()),
                ("created_at[from]", from.clone()),
                ("access", "playable,preview".into()),
                ("limit", "50".into()),
                partition.clone(),
            ],
        ),
        (
            "SearchPlaylists",
            "/playlists",
            vec![
                ("q", "house".into()),
                ("show_tracks", "false".into()),
                partition.clone(),
            ],
        ),
        (
            "SearchUsers",
            "/users",
            vec![("q", "enchanted".into()), partition.clone()],
        ),
        (
            "ResolveEnchanted",
            "/resolve",
            vec![("url", "https://soundcloud.com/dead-lock-497964652".into())],
        ),
        (
            "Related(293)",
            "/tracks/soundcloud%3Atracks%3A293/related",
            vec![("access", "playable,preview".into()), partition.clone()],
        ),
        ("Track(293)", "/tracks/soundcloud%3Atracks%3A293", vec![]),
        (
            "Streams(293)",
            "/tracks/soundcloud%3Atracks%3A293/streams",
            vec![],
        ),
        (
            "Comments(293)",
            "/tracks/soundcloud%3Atracks%3A293/comments",
            vec![partition.clone()],
        ),
        (
            "User(enchanted)",
            "/users/soundcloud%3Ausers%3A1641534089",
            vec![],
        ),
        (
            "UserTracks(enchanted)",
            "/users/soundcloud%3Ausers%3A1641534089/tracks",
            vec![("access", "playable,preview".into()), partition.clone()],
        ),
        (
            "RelatedUsers(enchanted)",
            "/users/soundcloud%3Ausers%3A1641534089/related",
            vec![partition.clone()],
        ),
        (
            "UserLikes(enchanted)",
            "/users/soundcloud%3Ausers%3A1641534089/likes/tracks",
            vec![("access", "playable,preview".into()), partition.clone()],
        ),
        (
            "UserPlaylists(enchanted)",
            "/users/soundcloud%3Ausers%3A1641534089/playlists",
            vec![("show_tracks", "false".into()), partition.clone()],
        ),
    ];

    for (name, path, query) in probes {
        let r = http
            .get(format!("https://api.soundcloud.com{path}"))
            .header("Accept", "application/json; charset=utf-8")
            .header("Authorization", format!("OAuth {token}"))
            .query(&query)
            .send()
            .await;
        match r {
            Ok(r) => {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                let shape = if text.trim_start().starts_with('[') {
                    "ARRAY"
                } else if text.contains("\"collection\"") {
                    "collection"
                } else {
                    "object"
                };
                let note = if status.is_success() {
                    String::new()
                } else {
                    format!("  {}", &text[..160.min(text.len())])
                };
                println!("{status:>7}  {shape:<11} {name:<20} {path}{note}");
                if name == "ResolveEnchanted" && status.is_success() {
                    let user: models::User = serde_json::from_str(&text).expect("resolved user");
                    println!(
                        "  resolved user: id={} username={:?} permalink={:?} avatar={:?}",
                        user.id, user.username, user.permalink, user.avatar_url
                    );
                }
                if name == "MyLikedPlaylists" && status.is_success() {
                    let playlists: models::Collection<models::Playlist> =
                        serde_json::from_str(&text).expect("liked playlists");
                    println!("  model: Ok({})", playlists.collection.len());
                    for playlist in playlists.collection.iter().take(10) {
                        println!(
                            "  liked playlist: id={} title={:?} album={} owner={:?}",
                            playlist.id,
                            playlist.title,
                            playlist.is_album(),
                            playlist.user.as_ref().map(|user| user.username.as_str())
                        );
                    }
                }
                if name == "Streams(293)" && status.is_success() {
                    let streams: serde_json::Value = serde_json::from_str(&text).expect("streams");
                    if let Some(url) = streams.get("hls_mp3_128_url").and_then(|v| v.as_str()) {
                        let dir = tempfile::tempdir().expect("cache dir");
                        let cache = std::sync::Arc::new(
                            cache::AudioCache::new(dir.path().to_owned(), 4 * 1024 * 1024)
                                .expect("cache"),
                        );
                        let downloader = hls::HlsDownloader::new(http.clone(), cache);
                        downloader.set_oauth(Some(token.clone()));
                        let playlist = downloader.playlist(url).await.expect("HLS playlist");
                        let segment = playlist.segments.first().expect("media segment");
                        let bytes = downloader
                            .segment("soundcloud:tracks:293", segment)
                            .await
                            .expect("segment");
                        let (samples, rate, channels) =
                            decode::decode_all(bytes, Some("audio/mpeg")).expect("decode");
                        assert!(!samples.is_empty());
                        assert!(samples.iter().all(|s| s.is_finite()));
                        println!(
                            "  playback: {} samples, {rate} Hz, {channels} channels",
                            samples.len()
                        );
                    }
                }
                if name == "UserLikes(enchanted)" && status.is_success() {
                    let liked: models::Collection<models::Track> =
                        serde_json::from_str(&text).expect("liked tracks");
                    let track = liked.collection.first().expect("at least one public like");
                    println!(
                        "  first liked: id={} title={:?} artist={:?}",
                        track.id,
                        track.title,
                        track.artist()
                    );
                    let stream_response = http
                        .get(format!(
                            "https://api.soundcloud.com/tracks/soundcloud%3Atracks%3A{}/streams",
                            track.id
                        ))
                        .header("Accept", "application/json; charset=utf-8")
                        .header("Authorization", format!("OAuth {token}"))
                        .send()
                        .await
                        .expect("liked track streams")
                        .error_for_status()
                        .expect("playable liked track");
                    let streams: serde_json::Value = stream_response.json().await.expect("streams");
                    let (url, mime) = streams
                        .get("hls_aac_160_url")
                        .and_then(|v| v.as_str())
                        .map(|url| (url, "audio/mp4"))
                        .or_else(|| {
                            streams
                                .get("hls_mp3_128_url")
                                .and_then(|v| v.as_str())
                                .map(|url| (url, "audio/mpeg"))
                        })
                        .expect("liked track HLS");
                    let dir = tempfile::tempdir().expect("cache dir");
                    let cache = std::sync::Arc::new(
                        cache::AudioCache::new(dir.path().to_owned(), 16 * 1024 * 1024)
                            .expect("cache"),
                    );
                    let downloader = hls::HlsDownloader::new(http.clone(), cache);
                    downloader.set_oauth(Some(token.clone()));
                    let playlist = downloader.playlist(url).await.expect("liked HLS playlist");
                    let manifest_ms = playlist
                        .segment_durations
                        .iter()
                        .sum::<f64>()
                        .mul_add(1000.0, 0.0)
                        .round() as u64;
                    println!(
                        "  liked manifest: {} segments, {manifest_ms} ms; API duration {} ms; waveform={:?}",
                        playlist.segments.len(),
                        track.effective_duration_ms(),
                        serde_json::from_str::<serde_json::Value>(&text)
                            .ok()
                            .and_then(|value| value
                                .get("collection")?
                                .get(0)?
                                .get("waveform_url")?
                                .as_str()
                                .map(str::to_owned))
                    );
                    let mut decoder = decode::SegmentDecoder::new(Some(mime.into()));
                    if let Some(init) = playlist.init_uri.as_deref() {
                        let bytes = downloader
                            .init_segment(&track.urn(), init)
                            .await
                            .expect("liked init segment");
                        decoder.set_init_segment(bytes);
                    }
                    for segment in playlist.segments.iter().take(3) {
                        let bytes = downloader
                            .segment(&track.urn(), segment)
                            .await
                            .expect("liked segment");
                        decoder.append(&bytes);
                    }
                    let mut pcm = Vec::new();
                    let mut packets = 0;
                    while decoder.next_packet(&mut pcm).expect("decode liked track") {
                        packets += 1;
                    }
                    println!(
                        "  liked playback: {packets} packets, {} ms buffered, {} Hz, {} channels",
                        decoder.decoded_ms(),
                        decoder.rate(),
                        decoder.channels()
                    );
                    assert!(decoder.decoded_ms() >= 5_000);
                    assert!(pcm.iter().all(|sample| sample.is_finite()));
                }
                if status.is_success() && shape == "collection" {
                    let parsed = if name.contains("Playlist") {
                        serde_json::from_str::<models::Collection<models::Playlist>>(&text).map(
                            |c| {
                                if name == "SearchPlaylists" {
                                    for playlist in c.collection.iter().take(5) {
                                        println!(
                                            "  playlist: id={} title={:?} tracks={:?}",
                                            playlist.id, playlist.title, playlist.track_count
                                        );
                                    }
                                }
                                c.collection.len()
                            },
                        )
                    } else if name == "SearchUsers"
                        || name == "Following"
                        || name.starts_with("RelatedUsers")
                    {
                        serde_json::from_str::<models::Collection<models::User>>(&text).map(|c| {
                            if name == "SearchUsers" {
                                for user in c.collection.iter().take(10) {
                                    println!(
                                        "  user: id={} username={:?} permalink={:?}",
                                        user.id, user.username, user.permalink
                                    );
                                }
                            }
                            c.collection.len()
                        })
                    } else if name == "Feed" {
                        serde_json::from_str::<models::Collection<models::StreamEntry>>(&text)
                            .map(|c| c.collection.len())
                    } else if name.starts_with("Comments") {
                        serde_json::from_str::<models::Collection<models::Comment>>(&text)
                            .map(|c| c.collection.len())
                    } else {
                        serde_json::from_str::<models::Collection<models::Track>>(&text)
                            .map(|c| c.collection.len())
                    };
                    println!("  model: {parsed:?}");
                }
            }
            Err(e) => println!("    ERR  {name}: {e}"),
        }
    }
}
