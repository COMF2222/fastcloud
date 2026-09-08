//! Where a view's rows come from.
//!
//! Every list has one entry point here, so a view never has to know whether
//! it is drawing the live catalogue or the offline demo library:
//!
//! ```text
//! for track in app.tracks(Key::Likes).rows() { … }
//! ```
//!
//! * **Connected** (credentials found, see [`crate::auth`]) — the row comes
//!   from [`crate::store`], which fetches once and caches.
//! * **Demo** (no credentials, or `--demo`) — the row comes from
//!   [`crate::demo`], synthesized locally, with no network at all.
//!
//! [`placeholder`] draws the three non-ready states, so "loading", "sign in
//! for this" and "that failed, here is why" look the same everywhere instead
//! of being reinvented per view.

use super::App;
use super::theme::{Metrics, Type};
use crate::api::models::{Playlist, Track, User};
use crate::store::{Key, Slot};
use eframe::egui;

impl App {
    /// Tracks for a list.
    pub fn tracks(&self, key: Key) -> Slot<Vec<Track>> {
        if self.demo {
            return Slot::Ready(demo_tracks(&key));
        }
        self.store.tracks(key)
    }

    /// Playlists for a list.
    pub fn playlists(&self, key: Key) -> Slot<Vec<Playlist>> {
        if self.demo {
            return Slot::Ready(demo_playlists(&key));
        }
        self.store.playlists(key)
    }

    /// Users for a list.
    pub fn users(&self, key: Key) -> Slot<Vec<User>> {
        if self.demo {
            return Slot::Ready(demo_users(&key));
        }
        self.store.users(key)
    }

    /// One track by id.
    pub fn track(&self, id: u64) -> Slot<Track> {
        if self.demo {
            return match crate::demo::demo_tracks().into_iter().find(|t| t.id == id) {
                Some(track) => Slot::Ready(track),
                None => Slot::Failed("that track is not in the demo library".to_owned()),
            };
        }
        self.store.track(id)
    }

    /// One playlist by id.
    pub fn playlist(&self, id: u64) -> Slot<Playlist> {
        if self.demo {
            return match crate::demo::demo_playlists()
                .into_iter()
                .find(|p| p.id == id)
            {
                Some(playlist) => Slot::Ready(playlist),
                None => Slot::Failed("that playlist is not in the demo library".to_owned()),
            };
        }
        self.store.playlist(id)
    }

    /// One profile by id.
    pub fn user(&self, id: u64) -> Slot<User> {
        if self.demo {
            return match demo_users(&Key::User(id)).into_iter().next() {
                Some(user) => Slot::Ready(user),
                None => Slot::Failed("that profile is not in the demo library".to_owned()),
            };
        }
        self.store.user(id)
    }

    /// The signed-in account, when there is one.
    pub fn account(&self) -> Option<crate::api::models::Me> {
        if self.demo {
            return Some(crate::demo::demo_me());
        }
        self.store.me().ready()
    }

    /// The name to show for whoever is using the app.
    pub fn display_name(&self) -> String {
        self.account()
            .map(|me| me.username)
            .unwrap_or_else(|| if self.demo { "Demo Listener" } else { "Guest" }.to_owned())
    }

    /// Tracks the player should queue for a station seeded on `track_id`:
    /// the seed first, then what SoundCloud considers related.
    pub fn station_queue(&self, seed: &Track) -> Vec<Track> {
        let mut queue = vec![seed.clone()];
        queue.extend(self.tracks(Key::Related(seed.id)).rows().iter().cloned());
        queue
    }

    /// A track to seed "based on what you play" shelves with: the last thing
    /// played, else the first like, else the first track in view.
    pub fn seed_track_id(&self) -> Option<u64> {
        if let Some(id) = self.recent_ids.first().copied() {
            return Some(id);
        }
        if let Some(track) = self.tracks(Key::History).rows().first() {
            return Some(track.id);
        }
        if let Some(id) = self.liked.iter().next().copied() {
            return Some(id);
        }
        self.tracks(Key::Likes).rows().first().map(|t| t.id)
    }

    /// An artist to seed "X's Picks" and "Artists to watch out for" with.
    pub fn seed_user_id(&self) -> Option<u64> {
        if let Some(id) = self.followed.iter().next().copied() {
            return Some(id);
        }
        if let Some(user) = self.users(Key::Following).rows().first() {
            return Some(user.id);
        }
        // Fall back to whoever made the seed track.
        let seed = self.seed_track_id()?;
        self.track(seed).ready()?.user.map(|u| u.id)
    }

    /// Which list a home shelf draws.
    ///
    /// The API publishes no recommendations, so each shelf is grounded in
    /// something that does exist — see [`crate::demo::Source`] for the mapping
    /// and [`crate::store`] for how it is fetched. `None` means the shelf has
    /// nothing to seed from yet (a fresh account with no plays and no likes).
    pub fn shelf_key(&self, source: crate::demo::Source) -> Option<Key> {
        use crate::demo::Source;
        Some(match source {
            Source::RecentlyPlayed => Key::History,
            Source::RelatedToHistory => Key::Related(self.seed_track_id()?),
            Source::MyLikes => Key::Likes,
            Source::GenreWindow => Key::Genre(self.shelf_genre().to_owned()),
            Source::UserLikes => Key::UserLikes(self.seed_user_id()?),
            Source::RelatedUsers => Key::RelatedUsers(self.seed_user_id()?),
            // No editorial route exists; a playlist search on the genre the
            // user listens to most is the closest honest substitute.
            Source::ResolvedPermalinks => return None,
            Source::MyPlaylists => Key::MyPlaylists,
        })
    }

    /// The genre the shelves lean on: whatever the seed track is tagged, else
    /// nothing (which the API reads as "any genre").
    pub fn shelf_genre(&self) -> String {
        self.seed_track_id()
            .and_then(|id| self.track(id).ready())
            .and_then(|track| track.genre)
            .filter(|genre| !genre.trim().is_empty())
            .unwrap_or_default()
    }
}

/// Draw a non-ready list's state, and say whether the caller should stop.
///
/// Returns `true` when there is nothing to lay out, so a view reads:
///
/// ```ignore
/// let slot = app.tracks(Key::Likes);
/// if placeholder(app, ui, &slot, "Tap the heart on a track to save it here.") {
///     return;
/// }
/// ```
pub fn placeholder<T>(app: &App, ui: &mut egui::Ui, slot: &Slot<Vec<T>>, empty: &str) -> bool {
    match slot {
        Slot::Ready(rows) if rows.is_empty() => {
            ui.add_space(Metrics::SP_1);
            ui.label(Type::BODY.rich(empty, app.theme.text_dim));
            true
        }
        Slot::Ready(_) => false,
        Slot::Loading => {
            loading_row(app, ui);
            true
        }
        Slot::Failed(why) => {
            failed_row(app, ui, why);
            true
        }
        Slot::Unavailable => {
            sign_in_row(app, ui);
            true
        }
    }
}

/// The same three states for a single item rather than a list.
pub fn placeholder_one<T>(app: &App, ui: &mut egui::Ui, slot: &Slot<T>) -> bool {
    match slot {
        Slot::Ready(_) => false,
        Slot::Loading => {
            loading_row(app, ui);
            true
        }
        Slot::Failed(why) => {
            failed_row(app, ui, why);
            true
        }
        Slot::Unavailable => {
            sign_in_row(app, ui);
            true
        }
    }
}

fn loading_row(app: &App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = Metrics::SP_1;
        super::icons::spinner(ui, 16.0, app.theme.text_dim);
        ui.label(Type::CAPTION.rich("Loading…", app.theme.text_dim));
    });
}

fn failed_row(app: &App, ui: &mut egui::Ui, why: &str) {
    ui.add_space(Metrics::SP_1);
    ui.label(Type::BODY.rich("That didn't load.", app.theme.text));
    ui.label(Type::CAPTION.rich(why, app.theme.text_dim));
}

/// The one thing an app-only token cannot do is read the account, so say so
/// and point at the button that fixes it.
fn sign_in_row(app: &App, ui: &mut egui::Ui) {
    ui.add_space(Metrics::SP_1);
    ui.label(Type::BODY.rich("This account data is unavailable.", app.theme.text_dim));
}

// ===========================================================================
// The demo library, mapped onto the same keys
// ===========================================================================

/// What the demo library answers for a key.
///
/// The shapes match the live ones (a station is a seed plus neighbours, picks
/// are one artist's likes) so a view laid out against demo data does not have
/// to change when an account is connected.
fn demo_tracks(key: &Key) -> Vec<Track> {
    let all = crate::demo::demo_tracks();
    match key {
        Key::Track(id) => all.into_iter().filter(|t| t.id == *id).collect(),
        // A little rotation so shelves are not all the same order.
        Key::Related(id) | Key::UserLikes(id) | Key::UserTracks(id) => rotate(all, *id as usize),
        Key::Genre(genre) => {
            let mut tracks = if genre.is_empty() || genre == "All Genres" {
                all
            } else {
                rotate(all, genre.len())
            };
            tracks.sort_by_key(|t| std::cmp::Reverse(t.playback_count.unwrap_or(0)));
            tracks
        }
        Key::SearchTracks(q) => {
            let needle = q.trim().to_lowercase();
            if needle.is_empty() {
                return all;
            }
            all.into_iter()
                .filter(|t| {
                    t.title.to_lowercase().contains(&needle)
                        || t.artist().to_lowercase().contains(&needle)
                })
                .collect()
        }
        _ => all,
    }
}

fn demo_playlists(key: &Key) -> Vec<Playlist> {
    let all = crate::demo::demo_playlists();
    match key {
        Key::Playlist(id) => all.into_iter().filter(|p| p.id == *id).collect(),
        Key::MyPlaylists => all.into_iter().filter(|p| !p.is_album()).collect(),
        Key::LikedPlaylists => all,
        Key::SearchPlaylists(q) => {
            let needle = q.trim().to_lowercase();
            if needle.is_empty() {
                return all;
            }
            all.into_iter()
                .filter(|p| p.title.to_lowercase().contains(&needle))
                .collect()
        }
        _ => all,
    }
}

fn demo_users(key: &Key) -> Vec<User> {
    let all: Vec<User> = crate::demo::demo_artists()
        .into_iter()
        .map(|artist| User {
            id: artist.id,
            username: artist.name,
            permalink: None,
            avatar_url: None,
            followers_count: artist.followers,
            followings_count: 0,
            track_count: 0,
            public_playlists_count: None,
        })
        .collect();
    match key {
        Key::User(id) => all.into_iter().filter(|u| u.id == *id).collect(),
        Key::SearchUsers(q) => {
            let needle = q.trim().to_lowercase();
            if needle.is_empty() {
                return all;
            }
            all.into_iter()
                .filter(|u| u.username.to_lowercase().contains(&needle))
                .collect()
        }
        _ => all,
    }
}

/// Rotate a list so different shelves lead with different rows.
fn rotate<T>(mut items: Vec<T>, by: usize) -> Vec<T> {
    let len = items.len();
    if len > 1 {
        items.rotate_left(by % len);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_maps_onto_the_demo_library() {
        let all = crate::demo::demo_tracks();
        assert!(!all.is_empty());
        // A track key answers exactly that track.
        let one = demo_tracks(&Key::Track(all[0].id));
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].id, all[0].id);
        assert!(demo_tracks(&Key::Track(u64::MAX)).is_empty());
        // Search filters on title and artist, and an empty query matches all.
        let hits = demo_tracks(&Key::SearchTracks(all[0].title.clone()));
        assert!(hits.iter().any(|t| t.id == all[0].id));
        assert_eq!(
            demo_tracks(&Key::SearchTracks("  ".into())).len(),
            all.len()
        );
        assert!(demo_tracks(&Key::SearchTracks("zzzz-nothing".into())).is_empty());
    }

    /// The genre shelf is sorted by plays in both paths — that is the whole
    /// substitute for the charts the API does not publish.
    #[test]
    fn the_demo_genre_shelf_is_sorted_by_plays() {
        let tracks = demo_tracks(&Key::Genre("Indie".into()));
        let plays: Vec<u64> = tracks
            .iter()
            .map(|t| t.playback_count.unwrap_or(0))
            .collect();
        let mut sorted = plays.clone();
        sorted.sort_unstable_by(|a, b| b.cmp(a));
        assert_eq!(plays, sorted);
    }

    #[test]
    fn related_shelves_do_not_all_lead_with_the_same_track() {
        let a = demo_tracks(&Key::Related(1));
        let b = demo_tracks(&Key::Related(4));
        assert_eq!(a.len(), b.len());
        assert_ne!(a[0].id, b[0].id);
    }

    #[test]
    fn playlists_and_albums_are_separated() {
        let mine = demo_playlists(&Key::MyPlaylists);
        assert!(mine.iter().all(|p| !p.is_album()));
        let all = demo_playlists(&Key::Feed);
        assert!(all.len() >= mine.len());
    }

    #[test]
    fn demo_artists_become_users() {
        let users = demo_users(&Key::RelatedUsers(1));
        assert!(!users.is_empty());
        assert!(users.iter().all(|u| u.followers_count > 0));
        let one = demo_users(&Key::User(users[0].id));
        assert_eq!(one.len(), 1);
    }

    #[test]
    fn rotation_is_bounded_and_keeps_every_row() {
        let items = vec![1, 2, 3, 4];
        assert_eq!(rotate(items.clone(), 0), vec![1, 2, 3, 4]);
        assert_eq!(rotate(items.clone(), 1), vec![2, 3, 4, 1]);
        // Past the end wraps instead of panicking.
        assert_eq!(rotate(items.clone(), 9), rotate(items.clone(), 1));
        assert_eq!(rotate(Vec::<i32>::new(), 3), Vec::<i32>::new());
        assert_eq!(rotate(vec![7], 5), vec![7]);
    }
}
