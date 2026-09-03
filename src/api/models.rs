#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// SoundCloud track URN, e.g. `soundcloud:tracks:123456`.
pub type TrackUrn = String;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Artwork {
    #[serde(rename = "150x150")]
    pub t150: Option<String>,
    #[serde(rename = "500x500")]
    pub t500: Option<String>,
}

impl Artwork {
    pub fn best(&self) -> Option<&str> {
        self.t500.as_deref().or(self.t150.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub permalink: Option<String>,
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub followers_count: u64,
    #[serde(default)]
    pub followings_count: u64,
    #[serde(default)]
    pub track_count: u64,
    #[serde(default)]
    pub public_playlists_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: u64,
    pub title: String,
    /// Duration in milliseconds (API field `duration`).
    #[serde(default, rename = "duration")]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub full_duration_ms: Option<u64>,
    pub artwork: Option<Artwork>,
    pub user: Option<UserLite>,
    #[serde(default)]
    pub playback_count: Option<u64>,
    #[serde(default)]
    pub favoritings_count: Option<u64>,
    #[serde(default)]
    pub likes_count: Option<u64>,
    #[serde(default)]
    pub comment_count: Option<u64>,
    #[serde(default)]
    pub streamable: bool,
    #[serde(default)]
    pub downloadable: bool,
    #[serde(default)]
    pub preview_start_ms: Option<u64>,
    #[serde(default)]
    pub preview_end_ms: Option<u64>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub permalink_url: Option<String>,
}

impl Track {
    pub fn urn(&self) -> String {
        format!("soundcloud:tracks:{}", self.id)
    }
    pub fn artist(&self) -> &str {
        self.user
            .as_ref()
            .map(|u| u.username.as_str())
            .unwrap_or("")
    }
    pub fn effective_duration_ms(&self) -> u64 {
        self.full_duration_ms.or(self.duration_ms).unwrap_or(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLite {
    pub id: u64,
    pub username: String,
    pub permalink: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: u64,
    pub title: String,
    pub artwork: Option<Artwork>,
    pub user: Option<UserLite>,
    #[serde(default)]
    pub track_count: Option<u64>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub is_album: bool,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub permalink_url: Option<String>,
}

impl Playlist {
    pub fn urn(&self) -> String {
        format!("soundcloud:playlists:{}", self.id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    pub created_at: Option<String>,
    pub user: Option<UserLite>,
    #[serde(default)]
    pub timestamp_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Me {
    pub id: u64,
    pub username: String,
    pub permalink: Option<String>,
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub followers_count: Option<u64>,
}

/// Standard SoundCloud collection response (linked partitioning).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection<T> {
    pub collection: Vec<T>,
    #[serde(default)]
    pub next_href: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LikeEntry {
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub target: LikeTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LikeTarget {
    Track(Box<Track>),
    Playlist(Box<Playlist>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEntry {
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub target: StreamTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StreamTarget {
    Playlist(Box<Playlist>),
    Track(Box<Track>),
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACK_JSON: &str = r#"{
        "id": 123,
        "title": "Song",
        "duration": 210000,
        "streamable": true,
        "user": {"id": 5, "username": "Artist"}
    }"#;

    #[test]
    fn parse_minimal_track() {
        let t: Track = serde_json::from_str(TRACK_JSON).unwrap();
        assert_eq!(t.id, 123);
        assert_eq!(t.artist(), "Artist");
        assert_eq!(t.effective_duration_ms(), 210000);
        assert_eq!(t.urn(), "soundcloud:tracks:123");
    }

    #[test]
    fn parse_collection() {
        let json = r#"{"collection":[{"id":1,"title":"a"}],"next_href":"https://x"}"#;
        let c: Collection<Track> = serde_json::from_str(json).unwrap();
        assert_eq!(c.collection.len(), 1);
        assert!(c.next_href.is_some());
    }

    #[test]
    fn parse_like_entry() {
        let json = r#"{"created_at":"2024-01-01","id":9,"title":"liked"}"#;
        let e: LikeEntry = serde_json::from_str(json).unwrap();
        assert!(matches!(e.target, LikeTarget::Track(_)));
    }
}
