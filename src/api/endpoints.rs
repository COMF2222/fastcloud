#![allow(dead_code)]

//! SoundCloud API endpoints, exactly as the published OpenAPI spec defines
//! them (see `docs/soundcloud-api.md`). Paths take **URNs**, not bare ids: the
//! spec's parameters are `track_urn`, `playlist_urn`, `user_urn`, and numeric
//! ids are only accepted on a few legacy routes.
//!
//! What the public API does **not** have, and what we do instead:
//! * personal recommendations (Daily Drops, Made for you, Weekly Wave) —
//!   built locally from [`recently_played`], [`related_tracks`] and likes,
//! * charts / trending — the store sorts a recent-window
//!   `/tracks` search by play count on our side,
//! * editorial "Curated by SoundCloud" selections — [`resolve`] against
//!   known permalinks,
//! * stations — a seed track plus [`related_tracks`] and autoplay,
//! * playlist snapshots/versioning — `PUT /playlists/{urn}` replaces the
//!   whole track list, so edits are optimistic locally and reconciled by
//!   re-reading the playlist.

use super::client::ApiClient;
use super::error::Result;
use super::models::{
    Collection, Comment, Me, Playlist, Storefront, StreamEntry, Track, User, WebProfile,
};
use super::paginated::Pager;
use serde_json::json;
use std::sync::Arc;

fn q(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// `soundcloud:tracks:123`, percent-encoded for a path segment.
pub fn track_urn(id: u64) -> String {
    urlencoding_encode(&format!("soundcloud:tracks:{id}"))
}

pub fn playlist_urn(id: u64) -> String {
    urlencoding_encode(&format!("soundcloud:playlists:{id}"))
}

pub fn user_urn(id: u64) -> String {
    urlencoding_encode(&format!("soundcloud:users:{id}"))
}

// ===== Tracks =====

/// Metadata accepted by `POST /tracks`. The audio file is streamed from disk;
/// optional empty fields are omitted so they do not overwrite server defaults.
#[derive(Debug, Clone, Default)]
pub struct TrackUpload<'a> {
    pub title: &'a str,
    pub artist: Option<&'a str>,
    pub description: Option<&'a str>,
    pub genre: Option<&'a str>,
    pub tags: Option<&'a str>,
    pub public: bool,
}

pub async fn upload_track(
    client: &Arc<ApiClient>,
    audio_path: &std::path::Path,
    upload: TrackUpload<'_>,
) -> Result<Track> {
    let mut fields = vec![
        ("track[title]".to_owned(), upload.title.to_owned()),
        (
            "track[sharing]".to_owned(),
            if upload.public { "public" } else { "private" }.to_owned(),
        ),
    ];
    for (name, value) in [
        ("track[artist]", upload.artist),
        ("track[description]", upload.description),
        ("track[genre]", upload.genre),
        ("track[tag_list]", upload.tags),
    ] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            fields.push((name.to_owned(), value.to_owned()));
        }
    }
    client
        .post_multipart_file("/tracks", &fields, "track[asset_data]", audio_path)
        .await
}

pub async fn track(client: &Arc<ApiClient>, id: u64) -> Result<Track> {
    let urn = track_urn(id);
    client.get(&format!("/tracks/{urn}"), &[]).await
}

/// Batch lookup: `GET /tracks?urns=` (the `ids` parameter is deprecated).
pub async fn tracks_by_urns(client: &Arc<ApiClient>, urns: &[String]) -> Result<Vec<Track>> {
    let joined = urns.join(",");
    client
        .get("/tracks", &[("urns", joined), ("limit", "200".into())])
        .await
}

pub async fn track_by_urn(client: &Arc<ApiClient>, urn: &str) -> Result<Track> {
    let encoded = urlencoding_encode(urn);
    client.get(&format!("/tracks/{encoded}"), &[]).await
}

/// Related tracks — the closest thing the API has to a station seed.
pub async fn related_tracks(client: &Arc<ApiClient>, id: u64) -> Pager<Track> {
    let urn = track_urn(id);
    Pager::new(
        client.clone(),
        &format!("/tracks/{urn}/related"),
        q(&[("access", "playable,preview")]),
    )
}

pub async fn track_comments(client: &Arc<ApiClient>, id: u64) -> Pager<Comment> {
    let urn = track_urn(id);
    Pager::new(
        client.clone(),
        &format!("/tracks/{urn}/comments"),
        q(&[("limit", "200")]),
    )
}

pub async fn track_favoriters(client: &Arc<ApiClient>, id: u64) -> Pager<User> {
    let urn = track_urn(id);
    Pager::new(
        client.clone(),
        &format!("/tracks/{urn}/favoriters"),
        q(&[("limit", "200")]),
    )
}

pub async fn track_reposters(client: &Arc<ApiClient>, id: u64) -> Pager<User> {
    let urn = track_urn(id);
    Pager::new(
        client.clone(),
        &format!("/tracks/{urn}/reposters"),
        q(&[("limit", "200")]),
    )
}

pub async fn update_track_metadata(
    client: &Arc<ApiClient>,
    id: u64,
    title: &str,
    description: Option<&str>,
    artist: Option<&str>,
) -> Result<Track> {
    let urn = track_urn(id);
    let mut track = json!({ "title": title });
    if let Some(description) = description {
        track["description"] = json!(description);
    }
    if let Some(artist) = artist {
        track["metadata_artist"] = json!(artist);
    }
    client
        .put(
            &format!("/tracks/{urn}"),
            &[],
            Some(json!({ "track": track })),
        )
        .await
}

pub async fn delete_track(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    let urn = track_urn(id);
    client.delete(&format!("/tracks/{urn}"), &[]).await
}

#[derive(Debug, Clone)]
pub struct StorefrontUpdate<'a> {
    pub title: &'a str,
    pub kind: &'a str,
    pub link: &'a str,
    pub link_title: Option<&'a str>,
    pub description: Option<&'a str>,
    pub price: Option<&'a str>,
}

pub async fn update_track_storefront(
    client: &Arc<ApiClient>,
    id: u64,
    update: StorefrontUpdate<'_>,
) -> Result<Storefront> {
    let urn = track_urn(id);
    let mut body = json!({
        "title": update.title,
        "type": update.kind,
        "link": update.link,
    });
    for (name, value) in [
        ("link_title", update.link_title),
        ("description", update.description),
        ("price", update.price),
    ] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            body[name] = json!(value);
        }
    }
    client
        .put(&format!("/tracks/{urn}/storefront"), &[], Some(body))
        .await
}

pub async fn track_preview_url(
    client: &Arc<ApiClient>,
    id: u64,
    secret_token: Option<&str>,
) -> Result<String> {
    let urn = track_urn(id);
    let query = secret_token
        .map(|token| vec![("secret_token".to_owned(), token.to_owned())])
        .unwrap_or_default();
    client
        .redirected_url(&format!("/tracks/{urn}/preview"), &query)
        .await
}

/// Post a comment, optionally pinned to a timestamp in milliseconds.
pub async fn post_comment(
    client: &Arc<ApiClient>,
    id: u64,
    body: &str,
    timestamp_ms: Option<u64>,
) -> Result<Comment> {
    let urn = track_urn(id);
    let mut comment = json!({ "body": body });
    if let Some(ms) = timestamp_ms {
        comment["timestamp"] = json!(ms);
    }
    client
        .post(
            &format!("/tracks/{urn}/comments"),
            &[],
            Some(json!({ "comment": comment })),
        )
        .await
}

pub async fn repost_track(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    let urn = track_urn(id);
    client
        .post::<serde_json::Value>(&format!("/reposts/tracks/{urn}"), &[], None)
        .await
        .map(|_| ())
}

pub async fn unrepost_track(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    let urn = track_urn(id);
    client.delete(&format!("/reposts/tracks/{urn}"), &[]).await
}

/// Resolve any permalink URL to its API entity.
pub async fn resolve(client: &Arc<ApiClient>, url: &str) -> Result<serde_json::Value> {
    client.get("/resolve", &[("url", url.to_string())]).await
}

/// Invalidate the current SoundCloud web session while keeping the app grant.
pub async fn sign_out(client: &Arc<ApiClient>) -> Result<()> {
    client
        .post::<serde_json::Value>("/sign-out", &[], None)
        .await
        .map(|_| ())
}

/// Revoke this application's OAuth access for the current SoundCloud account.
pub async fn disconnect(client: &Arc<ApiClient>) -> Result<()> {
    client
        .post::<serde_json::Value>("/disconnect", &[], None)
        .await
        .map(|_| ())
}

// ===== Likes =====

pub async fn like_track(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    let urn = track_urn(id);
    client
        .post::<serde_json::Value>(&format!("/likes/tracks/{urn}"), &[], None)
        .await
        .map(|_| ())
}

pub async fn unlike_track(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    let urn = track_urn(id);
    client.delete(&format!("/likes/tracks/{urn}"), &[]).await
}

pub async fn like_playlist(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    let urn = playlist_urn(id);
    client
        .post::<serde_json::Value>(&format!("/likes/playlists/{urn}"), &[], None)
        .await
        .map(|_| ())
}

pub async fn unlike_playlist(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    let urn = playlist_urn(id);
    client.delete(&format!("/likes/playlists/{urn}"), &[]).await
}

pub async fn my_liked_tracks(client: &Arc<ApiClient>) -> Pager<Track> {
    Pager::new(
        client.clone(),
        "/me/likes/tracks",
        q(&[("access", "playable,preview,blocked"), ("limit", "200")]),
    )
}

pub async fn my_liked_playlists(client: &Arc<ApiClient>) -> Pager<Playlist> {
    Pager::new(
        client.clone(),
        "/me/likes/playlists",
        q(&[("limit", "200")]),
    )
}

/// Another listener's likes — the "X's Picks" shelf.
pub async fn user_liked_tracks(client: &Arc<ApiClient>, user_id: u64) -> Pager<Track> {
    let urn = user_urn(user_id);
    Pager::new(
        client.clone(),
        &format!("/users/{urn}/likes/tracks"),
        q(&[("access", "playable,preview"), ("limit", "200")]),
    )
}

/// Legacy alias retained because it remains in the published OpenAPI spec.
pub async fn user_favorites(client: &Arc<ApiClient>, user_id: u64) -> Pager<Track> {
    let urn = user_urn(user_id);
    Pager::new(
        client.clone(),
        &format!("/users/{urn}/favorites"),
        q(&[("access", "playable,preview")]),
    )
}

pub async fn user_liked_playlists(client: &Arc<ApiClient>, user_id: u64) -> Pager<Playlist> {
    let urn = user_urn(user_id);
    Pager::new(
        client.clone(),
        &format!("/users/{urn}/likes/playlists"),
        q(&[("limit", "200")]),
    )
}

// ===== Playlists =====

pub async fn playlist(client: &Arc<ApiClient>, id: u64) -> Result<Playlist> {
    let urn = playlist_urn(id);
    client
        .get(
            &format!("/playlists/{urn}"),
            &[("show_tracks", "true".into())],
        )
        .await
}

pub async fn playlist_tracks(client: &Arc<ApiClient>, id: u64) -> Pager<Track> {
    let urn = playlist_urn(id);
    Pager::new(
        client.clone(),
        &format!("/playlists/{urn}/tracks"),
        q(&[("access", "playable,preview"), ("limit", "200")]),
    )
}

pub async fn my_playlists(client: &Arc<ApiClient>) -> Pager<Playlist> {
    Pager::new(
        client.clone(),
        "/me/playlists",
        q(&[("show_tracks", "true"), ("limit", "200")]),
    )
}

pub async fn user_playlists(client: &Arc<ApiClient>, user_id: u64) -> Pager<Playlist> {
    let urn = user_urn(user_id);
    Pager::new(
        client.clone(),
        &format!("/users/{urn}/playlists"),
        q(&[("show_tracks", "true"), ("limit", "200")]),
    )
}

/// Update a playlist. The spec has no incremental "add track" call and no
/// snapshot id: `tracks` **replaces** the whole ordered list, so the caller
/// sends the full set it wants and the UI holds an optimistic copy until
/// this returns (see `ui::App::playlist_edit`).
pub async fn update_playlist(
    client: &Arc<ApiClient>,
    id: u64,
    title: Option<&str>,
    is_public: Option<bool>,
    track_ids: &[u64],
) -> Result<Playlist> {
    let urn = playlist_urn(id);
    let mut playlist = json!({
        "tracks": track_ids
            .iter()
            .map(|id| json!({ "urn": format!("soundcloud:tracks:{id}") }))
            .collect::<Vec<_>>(),
    });
    if let Some(title) = title {
        playlist["title"] = json!(title);
    }
    if let Some(public) = is_public {
        playlist["sharing"] = json!(if public { "public" } else { "private" });
    }
    client
        .put(
            &format!("/playlists/{urn}"),
            &[],
            Some(json!({ "playlist": playlist })),
        )
        .await
}

pub async fn create_playlist(
    client: &Arc<ApiClient>,
    title: &str,
    is_public: bool,
    track_ids: &[u64],
) -> Result<Playlist> {
    let body = json!({
        "playlist": {
            "title": title,
            "sharing": if is_public { "public" } else { "private" },
            "tracks": track_ids
                .iter()
                .map(|id| json!({ "urn": format!("soundcloud:tracks:{id}") }))
                .collect::<Vec<_>>(),
        }
    });
    client.post("/playlists", &[], Some(body)).await
}

pub async fn delete_playlist(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    let urn = playlist_urn(id);
    client.delete(&format!("/playlists/{urn}"), &[]).await
}

pub async fn playlist_reposters(client: &Arc<ApiClient>, id: u64) -> Pager<User> {
    let urn = playlist_urn(id);
    Pager::new(
        client.clone(),
        &format!("/playlists/{urn}/reposters"),
        q(&[("limit", "200")]),
    )
}

pub async fn repost_playlist(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    let urn = playlist_urn(id);
    client
        .post::<serde_json::Value>(&format!("/reposts/playlists/{urn}"), &[], None)
        .await
        .map(|_| ())
}

pub async fn unrepost_playlist(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    let urn = playlist_urn(id);
    client
        .delete(&format!("/reposts/playlists/{urn}"), &[])
        .await
}

// ===== Users / social =====

pub async fn user(client: &Arc<ApiClient>, id: u64) -> Result<User> {
    let urn = user_urn(id);
    client.get(&format!("/users/{urn}"), &[]).await
}

/// Deprecated relationship detail routes remain wrapped for complete API
/// coverage, while the UI normally uses the non-deprecated user endpoint.
pub async fn my_following(client: &Arc<ApiClient>, id: u64) -> Result<User> {
    let urn = user_urn(id);
    client.get(&format!("/me/followings/{urn}"), &[]).await
}

pub async fn my_follower(client: &Arc<ApiClient>, id: u64) -> Result<User> {
    let urn = user_urn(id);
    client.get(&format!("/me/followers/{urn}"), &[]).await
}

pub async fn user_following(
    client: &Arc<ApiClient>,
    user_id: u64,
    following_id: u64,
) -> Result<User> {
    let user = user_urn(user_id);
    let following = user_urn(following_id);
    client
        .get(&format!("/users/{user}/followings/{following}"), &[])
        .await
}

pub async fn user_tracks(client: &Arc<ApiClient>, user_id: u64) -> Pager<Track> {
    let urn = user_urn(user_id);
    Pager::new(
        client.clone(),
        &format!("/users/{urn}/tracks"),
        q(&[("access", "playable,preview"), ("limit", "200")]),
    )
}

/// Artists related to a listener — the "Artists to watch out for" shelf.
pub async fn related_users(client: &Arc<ApiClient>, user_id: u64) -> Pager<User> {
    let urn = user_urn(user_id);
    Pager::new(client.clone(), &format!("/users/{urn}/related"), q(&[]))
}

pub async fn follow_user(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    let urn = user_urn(id);
    client
        .put::<serde_json::Value>(&format!("/me/followings/{urn}"), &[], None)
        .await
        .map(|_| ())
}

pub async fn unfollow_user(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    let urn = user_urn(id);
    client.delete(&format!("/me/followings/{urn}"), &[]).await
}

pub async fn my_followings(client: &Arc<ApiClient>) -> Pager<User> {
    Pager::new(client.clone(), "/me/followings", q(&[("limit", "200")]))
}

pub async fn user_followings(client: &Arc<ApiClient>, user_id: u64) -> Pager<User> {
    let urn = user_urn(user_id);
    Pager::new(
        client.clone(),
        &format!("/users/{urn}/followings"),
        q(&[("limit", "200")]),
    )
}

pub async fn user_followers(client: &Arc<ApiClient>, user_id: u64) -> Pager<User> {
    let urn = user_urn(user_id);
    Pager::new(
        client.clone(),
        &format!("/users/{urn}/followers"),
        q(&[("limit", "200")]),
    )
}

pub async fn user_web_profiles(client: &Arc<ApiClient>, user_id: u64) -> Result<Vec<WebProfile>> {
    let urn = user_urn(user_id);
    client.get(&format!("/users/{urn}/web-profiles"), &[]).await
}

pub async fn user_reposted_tracks(client: &Arc<ApiClient>, user_id: u64) -> Pager<Track> {
    let urn = user_urn(user_id);
    Pager::new(
        client.clone(),
        &format!("/users/{urn}/reposts/tracks"),
        q(&[("limit", "200")]),
    )
}

pub async fn user_reposted_playlists(client: &Arc<ApiClient>, user_id: u64) -> Pager<Playlist> {
    let urn = user_urn(user_id);
    Pager::new(
        client.clone(),
        &format!("/users/{urn}/reposts/playlists"),
        q(&[("limit", "200")]),
    )
}

// ===== Feed / history =====

pub async fn me(client: &Arc<ApiClient>) -> Result<Me> {
    client.get_me().await
}

pub async fn my_tracks(client: &Arc<ApiClient>) -> Pager<Track> {
    Pager::new(client.clone(), "/me/tracks", q(&[("limit", "200")]))
}

pub async fn my_followers(client: &Arc<ApiClient>) -> Pager<User> {
    Pager::new(client.clone(), "/me/followers", q(&[("limit", "200")]))
}

pub async fn my_followings_tracks(client: &Arc<ApiClient>) -> Pager<Track> {
    Pager::new(
        client.clone(),
        "/me/followings/tracks",
        q(&[("access", "playable,preview,blocked"), ("limit", "200")]),
    )
}

pub async fn my_reposted_tracks(client: &Arc<ApiClient>) -> Pager<Track> {
    Pager::new(client.clone(), "/me/reposts/tracks", q(&[("limit", "200")]))
}

pub async fn my_reposted_playlists(client: &Arc<ApiClient>) -> Pager<Playlist> {
    Pager::new(
        client.clone(),
        "/me/reposts/playlists",
        q(&[("limit", "200")]),
    )
}

pub async fn feed_tracks(client: &Arc<ApiClient>) -> Pager<StreamEntry> {
    Pager::new(
        client.clone(),
        "/me/feed/tracks",
        q(&[("access", "playable,preview,blocked"), ("limit", "200")]),
    )
}

/// Activity of the people you follow.
pub async fn feed(client: &Arc<ApiClient>) -> Pager<StreamEntry> {
    Pager::new(
        client.clone(),
        "/me/feed",
        q(&[("access", "playable,preview")]),
    )
}

/// Legacy activity routes retained because they still exist in the published
/// spec. New UI code uses `/me/feed` and `/me/feed/tracks`.
pub async fn activities(client: &Arc<ApiClient>) -> Pager<StreamEntry> {
    Pager::new(client.clone(), "/me/activities", q(&[]))
}

pub async fn own_activities(client: &Arc<ApiClient>) -> Pager<StreamEntry> {
    Pager::new(client.clone(), "/me/activities/all/own", q(&[]))
}

pub async fn track_activities(client: &Arc<ApiClient>) -> Pager<StreamEntry> {
    Pager::new(client.clone(), "/me/activities/tracks", q(&[]))
}

/// Play history. The endpoint answers at most 25 tracks and takes no
/// `limit` and no pagination, so this is a plain request, not a pager.
pub async fn recently_played(client: &Arc<ApiClient>) -> Result<Vec<Track>> {
    let tracks: Collection<Track> = client
        .get(
            "/me/recently-played/tracks",
            &[("access", "playable,preview,blocked".into())],
        )
        .await?;
    Ok(tracks.collection)
}

// ===== Search =====

pub async fn search_tracks(client: &Arc<ApiClient>, query: &str) -> Pager<Track> {
    Pager::new(
        client.clone(),
        "/tracks",
        q(&[("q", query), ("access", "playable,preview")]),
    )
}

pub async fn search_playlists(client: &Arc<ApiClient>, query: &str) -> Pager<Playlist> {
    // Unlike track discovery, the playlist search needs a non-empty query.
    let query = if query.trim().is_empty() {
        "music"
    } else {
        query.trim()
    };
    Pager::new(
        client.clone(),
        "/playlists",
        q(&[("q", query), ("show_tracks", "true")]),
    )
}

pub async fn search_users(client: &Arc<ApiClient>, query: &str) -> Pager<User> {
    Pager::new(client.clone(), "/users", q(&[("q", query)]))
}

/// "Trending by genre", assembled here because the API has no charts:
/// a recent window of one genre, which the caller sorts by play count.
///
/// `days` bounds the window; the spec wants `created_at[from]` as
/// `yyyy-mm-dd hh:mm:ss`.
pub async fn tracks_by_genre(client: &Arc<ApiClient>, genre: &str, days: i64) -> Pager<Track> {
    let from = chrono::Utc::now() - chrono::Duration::days(days.max(1));
    Pager::new(
        client.clone(),
        "/tracks",
        q(&[
            ("genres", genre),
            (
                "created_at[from]",
                &from.format("%Y-%m-%d %H:%M:%S").to_string(),
            ),
            ("access", "playable,preview"),
            ("limit", "50"),
        ]),
    )
}

// ===== Streams =====

/// Playable URLs for a track. The spec defines exactly these three fields;
/// there is no progressive MP3 (`stream_url` is deprecated and preview-only).
#[derive(Debug, serde::Deserialize)]
pub struct StreamUrls {
    #[serde(default, rename = "hls_mp3_128_url")]
    pub hls_mp3_128: Option<String>,
    #[serde(default, rename = "hls_aac_160_url")]
    pub hls_aac_160: Option<String>,
    #[serde(default, rename = "preview_mp3_128_url")]
    pub preview_mp3_128: Option<String>,
}

impl StreamUrls {
    /// The best full-quality stream. SoundCloud is retiring MP3 playback and
    /// explicitly recommends the higher-bitrate AAC rendition.
    pub fn best_full(&self) -> Option<&str> {
        self.best_full_with_bitrate().map(|(url, _)| url)
    }

    /// The same choice, with the bitrate the field name declares. The API
    /// names its own rates, so this is read off rather than guessed — it is
    /// what the mini player's `kbps` readout shows.
    pub fn best_full_with_bitrate(&self) -> Option<(&str, u32)> {
        self.hls_aac_160
            .as_deref()
            .map(|url| (url, 160))
            .or_else(|| self.hls_mp3_128.as_deref().map(|url| (url, 128)))
    }
}

pub async fn track_streams(client: &Arc<ApiClient>, urn: &str) -> Result<StreamUrls> {
    let enc = urlencoding_encode(urn);
    client.get(&format!("/tracks/{enc}/streams"), &[]).await
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urns_are_percent_encoded_for_paths() {
        assert_eq!(track_urn(123), "soundcloud%3Atracks%3A123");
        assert_eq!(playlist_urn(7), "soundcloud%3Aplaylists%3A7");
        assert_eq!(user_urn(9), "soundcloud%3Ausers%3A9");
    }

    #[test]
    fn stream_choice_prefers_higher_quality_aac() {
        let both = StreamUrls {
            hls_mp3_128: Some("mp3".into()),
            hls_aac_160: Some("aac".into()),
            preview_mp3_128: Some("preview".into()),
        };
        assert_eq!(both.best_full(), Some("aac"));
        let aac_only = StreamUrls {
            hls_mp3_128: None,
            hls_aac_160: Some("aac".into()),
            preview_mp3_128: None,
        };
        assert_eq!(aac_only.best_full(), Some("aac"));
        let none = StreamUrls {
            hls_mp3_128: None,
            hls_aac_160: None,
            preview_mp3_128: Some("preview".into()),
        };
        assert_eq!(none.best_full(), None);
    }

    /// The bitrate comes from the field the URL was taken from, so the two
    /// can never disagree.
    #[test]
    fn the_bitrate_follows_the_chosen_stream() {
        let both = StreamUrls {
            hls_mp3_128: Some("mp3".into()),
            hls_aac_160: Some("aac".into()),
            preview_mp3_128: None,
        };
        assert_eq!(both.best_full_with_bitrate(), Some(("aac", 160)));
        let aac_only = StreamUrls {
            hls_mp3_128: None,
            hls_aac_160: Some("aac".into()),
            preview_mp3_128: None,
        };
        assert_eq!(aac_only.best_full_with_bitrate(), Some(("aac", 160)));
        let none = StreamUrls {
            hls_mp3_128: None,
            hls_aac_160: None,
            preview_mp3_128: Some("preview".into()),
        };
        assert_eq!(none.best_full_with_bitrate(), None);
    }
}
