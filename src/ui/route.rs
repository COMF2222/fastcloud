#![allow(dead_code)]

use crate::api::models::{Track, User};

/// Where the main view is pointing.
#[derive(Debug, Clone)]
pub enum Route {
    Home,
    Likes,
    Search(String),
    TrackDetail(u64),
    PlaylistDetail(u64),
    UserDetail(u64),
    Recent,
    Following,
    Settings,
}

/// A track row payload for drag & drop into playlists.
#[derive(Clone)]
pub struct TrackDragPayload {
    pub track_id: u64,
}

pub fn track_row_id(track: &Track) -> egui::Id {
    egui::Id::new(("track", track.id))
}

pub fn user_row_id(user: &User) -> egui::Id {
    egui::Id::new(("user", user.id))
}
