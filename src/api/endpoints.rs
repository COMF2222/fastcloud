#![allow(dead_code)]

use super::client::ApiClient;
use super::error::Result;
use super::models::{Comment, LikeEntry, Me, Playlist, StreamEntry, Track, User};
use super::paginated::Pager;
use serde_json::json;
use std::sync::Arc;

fn q(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// ===== Tracks =====

pub async fn track(client: &Arc<ApiClient>, id: u64) -> Result<Track> {
    client.get(&format!("/tracks/{id}"), &[]).await
}

pub async fn track_by_urn(client: &Arc<ApiClient>, urn: &str) -> Result<Track> {
    let encoded = urlencoding_encode(urn);
    client.get(&format!("/tracks?urns={encoded}"), &[]).await
}

pub async fn related_tracks(client: &Arc<ApiClient>, id: u64) -> Pager<Track> {
    Pager::new(client.clone(), &format!("/tracks/{id}/related"), q(&[]))
}

pub async fn track_comments(client: &Arc<ApiClient>, id: u64) -> Pager<Comment> {
    Pager::new(client.clone(), &format!("/tracks/{id}/comments"), q(&[]))
}

pub async fn repost_track(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    client
        .post::<serde_json::Value>(&format!("/tracks/{id}/reposts"), &[], None)
        .await
        .map(|_| ())
}

pub async fn unrepost_track(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    client.delete(&format!("/tracks/{id}/reposts"), &[]).await
}

/// Resolve any permalink URL to its API entity.
pub async fn resolve(client: &Arc<ApiClient>, url: &str) -> Result<serde_json::Value> {
    let enc = urlencoding_encode(url);
    client.get("/resolve", &[("url", format!("/{enc}"))]).await
}

// ===== Likes =====

pub async fn like_track(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    client
        .post::<serde_json::Value>(&format!("/tracks/{id}/likers"), &[], Some(json!({})))
        .await
        .map(|_| ())
}

pub async fn unlike_track(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    client.delete(&format!("/tracks/{id}/likers"), &[]).await
}

pub async fn user_likes(client: &Arc<ApiClient>, user_id: u64) -> Pager<LikeEntry> {
    Pager::new(client.clone(), &format!("/users/{user_id}/likes"), q(&[]))
}

// ===== Playlists =====

pub async fn playlist(client: &Arc<ApiClient>, id: u64) -> Result<Playlist> {
    client.get(&format!("/playlists/{id}"), &[]).await
}

pub async fn user_playlists(client: &Arc<ApiClient>, user_id: u64) -> Pager<Playlist> {
    Pager::new(
        client.clone(),
        &format!("/users/{user_id}/playlists"),
        q(&[("show_tracks", "false")]),
    )
}

/// Update a playlist: PUT with the full ordered track list.
pub async fn update_playlist(
    client: &Arc<ApiClient>,
    id: u64,
    title: Option<&str>,
    is_public: Option<bool>,
    track_ids: &[u64],
) -> Result<Playlist> {
    let body = json!({
        "playlist": {
            "title": title,
            "public": is_public,
            "tracks": track_ids.iter().map(|id| json!({"id": id})).collect::<Vec<_>>(),
        }
    });
    client
        .put(&format!("/playlists/{id}"), &[], Some(body))
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
            "public": is_public,
            "tracks": track_ids.iter().map(|id| json!({"id": id})).collect::<Vec<_>>(),
        }
    });
    client.post("/playlists", &[], Some(body)).await
}

pub async fn delete_playlist(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    client.delete(&format!("/playlists/{id}"), &[]).await
}

// ===== Users / social =====

pub async fn user(client: &Arc<ApiClient>, id: u64) -> Result<User> {
    client.get(&format!("/users/{id}"), &[]).await
}

pub async fn user_tracks(client: &Arc<ApiClient>, user_id: u64) -> Pager<Track> {
    Pager::new(client.clone(), &format!("/users/{user_id}/tracks"), q(&[]))
}

pub async fn follow_user(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    client
        .post::<serde_json::Value>(&format!("/users/{id}/followings"), &[], Some(json!({})))
        .await
        .map(|_| ())
}

pub async fn unfollow_user(client: &Arc<ApiClient>, id: u64) -> Result<()> {
    client.delete(&format!("/users/{id}/followings"), &[]).await
}

pub async fn user_followings(client: &Arc<ApiClient>, user_id: u64) -> Pager<User> {
    Pager::new(
        client.clone(),
        &format!("/users/{user_id}/followings"),
        q(&[]),
    )
}

pub async fn user_followers(client: &Arc<ApiClient>, user_id: u64) -> Pager<User> {
    Pager::new(
        client.clone(),
        &format!("/users/{user_id}/followers"),
        q(&[]),
    )
}

// ===== Feed / charts / recent =====

pub async fn me(client: &Arc<ApiClient>) -> Result<Me> {
    client.get_me().await
}

pub async fn feed(client: &Arc<ApiClient>, user_id: u64) -> Pager<StreamEntry> {
    Pager::new(client.clone(), &format!("/users/{user_id}/stream"), q(&[]))
}

pub async fn recent_tracks(client: &Arc<ApiClient>, user_id: u64) -> Pager<Track> {
    Pager::new(
        client.clone(),
        &format!("/users/{user_id}/tracks/recent"),
        q(&[]),
    )
}

// ===== Search =====

pub async fn search_tracks(client: &Arc<ApiClient>, query: &str) -> Pager<Track> {
    Pager::new(client.clone(), "/search/tracks", q(&[("q", query)]))
}

pub async fn search_playlists(client: &Arc<ApiClient>, query: &str) -> Pager<Playlist> {
    Pager::new(client.clone(), "/search/playlists", q(&[("q", query)]))
}

pub async fn search_users(client: &Arc<ApiClient>, query: &str) -> Pager<User> {
    Pager::new(client.clone(), "/search/users", q(&[("q", query)]))
}

// ===== Streams =====

#[derive(Debug, serde::Deserialize)]
pub struct StreamUrls {
    #[serde(default, rename = "hls_mp3_128_url")]
    pub hls_mp3_128: Option<String>,
    #[serde(default, rename = "hls_opus_64_url")]
    pub hls_opus_64: Option<String>,
    #[serde(default, rename = "hls_aac_160_url")]
    pub hls_aac_160: Option<String>,
    #[serde(default, rename = "http_mp3_128_url")]
    pub http_mp3_128: Option<String>,
    #[serde(default, rename = "preview_mp3_128_url")]
    pub preview_mp3_128: Option<String>,
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
    fn encode_urn() {
        assert_eq!(
            urlencoding_encode("soundcloud:tracks:123"),
            "soundcloud%3Atracks%3A123"
        );
    }
}
