use crate::api::models::Track;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicTrack {
    pub id: u64, pub title: String, pub artist: String, pub duration_ms: u64,
    pub genre: String, pub artwork_url: Option<String>, pub permalink_url: Option<String>,
    pub isrc: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Feedback {
    pub disliked: bool, pub updated_at: i64, pub device: String, pub track: PublicTrack,
}

fn public_url(raw: Option<&str>, artwork: bool) -> Option<String> {
    let mut url = reqwest::Url::parse(raw?).ok()?;
    let host = url.host_str()?;
    let allowed = if artwork { host == "sndcdn.com" || host.ends_with(".sndcdn.com") }
        else { matches!(host, "soundcloud.com" | "www.soundcloud.com") };
    if url.scheme() != "https" || !allowed || !url.username().is_empty() || url.password().is_some() || url.port().is_some_and(|port| port != 443) { return None; }
    url.set_query(None); url.set_fragment(None);
    let value = url.to_string();
    (value.chars().count() <= 1024).then_some(value)
}

impl PublicTrack {
    pub fn from_track(track: &Track) -> Self {
        let text = |value: &str, max| value.chars().filter(|ch| !ch.is_control()).take(max).collect();
        Self { id: track.id, title: text(&track.title, 512), artist: text(track.artist(), 256),
            duration_ms: track.effective_duration_ms().min(86_400_000), genre: text(track.genre.as_deref().unwrap_or(""),128),
            artwork_url: public_url(track.artwork_url(),true), permalink_url: public_url(track.permalink_url.as_deref(),false),
            isrc: track.publisher_metadata.as_ref().and_then(|metadata| metadata.isrc.as_ref())
                .map(|code| code.chars().filter(char::is_ascii_alphanumeric).flat_map(char::to_uppercase).collect::<String>())
                .filter(|code| code.len() == 12) }
    }

    pub fn as_track(&self) -> Option<Track> {
        serde_json::from_value(serde_json::json!({"id":self.id,"title":self.title,"metadata_artist":self.artist,
            "duration":self.duration_ms,"genre":self.genre,"artwork_url":self.artwork_url,
            "permalink_url":self.permalink_url,"publisher_metadata":{"artist":self.artist,"isrc":self.isrc},"streamable":true})).ok()
    }
}

impl Feedback {
    pub fn rank(&self) -> (i64, &str) { (self.updated_at, &self.device) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_metadata_excludes_tokens_and_preserves_recording_credit() {
        let mut track = crate::demo::demo_tracks().remove(0);
        track.description = Some("private text".into());
        track.artwork_url = Some("https://i1.sndcdn.com/image.jpg?oauth_token=private".into());
        track.artwork = None;
        track.permalink_url = Some("https://soundcloud.com/artist/song?secret_token=private".into());
        let public = PublicTrack::from_track(&track);
        let serialized = serde_json::to_string(&public).unwrap();
        assert!(!serialized.contains("private"));
        let restored = public.as_track().unwrap();
        assert_eq!(restored.id,track.id);
        assert_eq!(restored.artist(),track.artist());
        assert_eq!(restored.effective_duration_ms(),track.effective_duration_ms());
    }
}
