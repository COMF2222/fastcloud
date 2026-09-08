#![allow(dead_code)]

use crate::api::models::{Me, Track, UserLite};

/// Offline demo library used when `--demo` is passed (no credentials needed).
pub fn demo_tracks() -> Vec<Track> {
    let artist = UserLite {
        id: 1,
        username: "SoundCloud Demo".into(),
        permalink: Some("demo".into()),
        avatar_url: None,
    };
    let titles = [
        "Northern Lights",
        "Sunset Drive",
        "Rain on Glass",
        "Neon District",
        "Paper Planes",
        "Golden Hour",
        "Static Fields",
        "Low Tide",
        "Concrete Garden",
        "Afterglow",
    ];
    titles
        .iter()
        .enumerate()
        .map(|(i, title)| Track {
            id: 1000 + i as u64,
            title: (*title).into(),
            duration_ms: Some(120_000 + i as u64 * 13_000),
            full_duration_ms: Some(120_000 + i as u64 * 13_000),
            artwork: None,
            artwork_url: None,
            waveform_url: None,
            user: Some(artist.clone()),
            publisher_metadata: None,
            metadata_artist: None,
            playback_count: Some(12_345 * (i as u64 + 1)),
            favoritings_count: Some(100 * (i as u64 + 1)),
            likes_count: Some(100 * (i as u64 + 1)),
            comment_count: Some(12),
            streamable: true,
            downloadable: false,
            preview_start_ms: None,
            preview_end_ms: None,
            genre: Some("ambient".into()),
            description: Some("Demo track generated offline.".into()),
            created_at: Some("2025-01-01T00:00:00Z".into()),
            permalink_url: None,
            policy: None,
            access: None,
            monetization_model: None,
            feed_reposted: false,
        })
        .collect()
}

pub fn demo_me() -> Me {
    Me {
        id: 1,
        username: "Demo Listener".into(),
        permalink: Some("demo-listener".into()),
        avatar_url: None,
        followers_count: Some(0),
    }
}

/// Demo playlists for the library page and home tiles.
pub fn demo_playlists() -> Vec<crate::api::models::Playlist> {
    use crate::api::models::Playlist;
    let names = [
        (2001u64, "Neon Nights", 4u64),
        (2002, "Low Tide Radio", 3),
        (2003, "Concrete Garden Mix", 5),
    ];
    names
        .iter()
        .map(|(id, title, count)| Playlist {
            id: *id,
            title: (*title).into(),
            artwork: None,
            artwork_url: None,
            user: None,
            track_count: Some(*count),
            duration_ms: None,
            is_album: false,
            playlist_type: None,
            set_type: None,
            created_at: None,
            permalink_url: None,
            tracks: Vec::new(),
            feed_reposted: false,
        })
        .collect()
}

/// Demo artists for the "follow" rail and feed.
#[derive(Debug, Clone)]
pub struct DemoArtist {
    pub id: u64,
    pub name: String,
    pub followers: u64,
    pub tracks: u64,
    pub verified: bool,
}

pub fn demo_artists() -> Vec<DemoArtist> {
    vec![
        DemoArtist {
            id: 11,
            name: "auratoshi".into(),
            followers: 11_800,
            tracks: 23,
            verified: false,
        },
        DemoArtist {
            id: 12,
            name: "SALUKI".into(),
            followers: 1_200_000,
            tracks: 20,
            verified: true,
        },
        DemoArtist {
            id: 13,
            name: "eclipse media".into(),
            followers: 27_000,
            tracks: 620,
            verified: false,
        },
        DemoArtist {
            id: 14,
            name: "silver gloria".into(),
            followers: 84_000,
            tracks: 41,
            verified: false,
        },
        DemoArtist {
            id: 15,
            name: "WW CREW".into(),
            followers: 310_000,
            tracks: 112,
            verified: true,
        },
    ]
}

/// Feed activity for the Feed page (demo).
#[derive(Debug, Clone)]
pub struct FeedItem {
    pub kind: &'static str,
    pub user: String,
    pub user_id: u64,
    pub track_id: u64,
    pub when: &'static str,
}

pub fn demo_feed() -> Vec<FeedItem> {
    vec![
        FeedItem {
            kind: "released",
            user: "SALUKI".into(),
            user_id: 12,
            track_id: 1001,
            when: "2h",
        },
        FeedItem {
            kind: "reposted",
            user: "auratoshi".into(),
            user_id: 11,
            track_id: 1003,
            when: "5h",
        },
        FeedItem {
            kind: "liked",
            user: "silver gloria".into(),
            user_id: 14,
            track_id: 1005,
            when: "1d",
        },
        FeedItem {
            kind: "released",
            user: "WW CREW".into(),
            user_id: 15,
            track_id: 1007,
            when: "2d",
        },
        FeedItem {
            kind: "liked",
            user: "eclipse media".into(),
            user_id: 13,
            track_id: 1000,
            when: "3d",
        },
    ]
}

/// A shelf on the home page, in soundcloud.com's own vocabulary.
///
/// SoundCloud's home is a stack of titled shelves, but the public API has no
/// editorial or recommendation endpoints (checked against the published
/// OpenAPI spec): there is no `/charts`, no `/trending`, no `/selections`,
/// and nothing for Daily Drops / Made for you / Weekly Wave. [`Source`]
/// records what each shelf will really be built from:
///
/// | shelf | real source |
/// |---|---|
/// | Recently played | `GET /me/recently-played/tracks` (≤25, no paging) |
/// | Daily Drops | `/tracks/{urn}/related` seeded by history |
/// | Mixed for you | `/me/likes/tracks` grouped locally |
/// | Trending by genre | `/tracks?genres=&created_at[from]=`, sorted here |
/// | Made for you | related tracks of top artists |
/// | Curated by SoundCloud | `/resolve` on known permalinks |
/// | Albums for you | playlists with `set_type=album` |
/// | Liked By | `/users/{urn}/likes/tracks` |
/// | Discover with Stations | seed track + `/related` + autoplay |
/// | Artists to watch | `/users/{urn}/related` |
#[derive(Debug, Clone)]
pub struct HomeShelf {
    /// Section heading, e.g. "Daily Drops".
    pub title: &'static str,
    /// The line under it, or empty.
    pub subtitle: &'static str,
    /// Unique carousel id (ScrollAreas with equal ids clash).
    pub salt: &'static str,
    pub kind: ShelfKind,
    /// Which API call fills this shelf in the real build.
    pub source: Source,
}

/// Where a shelf's content comes from once an account is connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `GET /me/recently-played/tracks`
    RecentlyPlayed,
    /// `GET /tracks/{track_urn}/related`, seeded from history or likes.
    RelatedToHistory,
    /// `GET /me/likes/tracks`
    MyLikes,
    /// `GET /tracks?genres=…&created_at[from]=…`, sorted by plays here.
    GenreWindow,
    /// `GET /users/{user_urn}/likes/tracks`
    UserLikes,
    /// `GET /users/{user_urn}/related`
    RelatedUsers,
    /// `GET /resolve?url=…` on curated permalinks.
    ResolvedPermalinks,
    /// `GET /me/playlists` filtered to albums.
    MyPlaylists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShelfKind {
    /// Track cards.
    Tracks,
    /// Playlist/mix cards.
    Playlists,
    /// Album cards (playlists flagged as albums on SoundCloud).
    Albums,
    /// Round artist avatars.
    Artists,
    /// Artist stations, i.e. a seed track plus autoplay.
    Stations,
    /// ``<name>'s Picks`` — an artist's likes.
    Picks,
    /// Genre chips ("Indie · Trending"), which open a filtered search.
    Genres,
}

/// The home page's shelves, in soundcloud.com's order.
pub fn home_shelves() -> Vec<HomeShelf> {
    vec![
        HomeShelf {
            title: "Daily Drops",
            subtitle: "New releases based on your taste. Updated every day",
            salt: "home-daily-drops",
            kind: ShelfKind::Tracks,
            source: Source::RelatedToHistory,
        },
        HomeShelf {
            title: "Your likes",
            subtitle: "",
            salt: "home-mixed",
            kind: ShelfKind::Tracks,
            source: Source::MyLikes,
        },
        HomeShelf {
            title: "Trending by genre",
            subtitle: "",
            salt: "home-genres",
            kind: ShelfKind::Genres,
            source: Source::GenreWindow,
        },
        HomeShelf {
            title: "Related tracks",
            subtitle: "",
            salt: "home-made-for-you",
            kind: ShelfKind::Tracks,
            source: Source::RelatedToHistory,
        },
        HomeShelf {
            title: "Curated by SoundCloud",
            subtitle: "",
            salt: "home-curated",
            kind: ShelfKind::Playlists,
            source: Source::ResolvedPermalinks,
        },
        HomeShelf {
            title: "Your playlists",
            subtitle: "",
            salt: "home-albums",
            kind: ShelfKind::Playlists,
            source: Source::MyPlaylists,
        },
        HomeShelf {
            title: "Liked By",
            subtitle: "",
            salt: "home-liked-by",
            kind: ShelfKind::Picks,
            source: Source::UserLikes,
        },
        HomeShelf {
            title: "Discover with Stations",
            subtitle: "",
            salt: "home-stations",
            kind: ShelfKind::Stations,
            source: Source::RelatedToHistory,
        },
        HomeShelf {
            title: "Artists to watch out for",
            subtitle: "",
            salt: "home-artists",
            kind: ShelfKind::Artists,
            source: Source::RelatedUsers,
        },
    ]
}

/// Genres for the "Trending by genre" shelf, as soundcloud.com lists them.
pub fn demo_genres() -> Vec<&'static str> {
    vec![
        "All Genres",
        "Indie",
        "Electronic",
        "Trap",
        "Alternative Rock",
        "Hip-Hop & Rap",
        "House",
        "Metal",
    ]
}

/// Albums for the "Albums for you" shelf: `(id, title, artist, year)`.
pub fn demo_albums() -> Vec<(u64, &'static str, &'static str, u32)> {
    vec![
        (3001, "Northern Lights", "SoundCloud Demo", 2025),
        (3002, "Concrete Garden", "eclipse media", 2024),
        (3003, "Low Tide", "silver gloria", 2023),
        (3004, "Static Fields", "WW CREW", 2022),
    ]
}

/// The demo synthesiser's rate. Matches SoundCloud's HLS streams, so the
/// visualiser's constants (`crate::vis::SAMPLE_RATE`) hold in demo mode too.
pub const DEMO_RATE: u32 = 44_100;

/// How much demo audio is synthesized at a time.
///
/// A whole track as `f32` stereo is 44100 × 2 × 4 = 353 KB a second — over
/// 100 MB for a four-minute pad, all of it resident. The player only ever
/// needs the next few seconds ([`crate::player::Player::run`] tops the buffer
/// up below 8 s), so a window is synthesized on demand instead. Ten seconds
/// keeps the resident buffer under ~18 s (≈6 MB) while topping up twice a
/// minute at most.
const WINDOW_SECS: f32 = 10.0;

/// The most one window may hold. Pinned by a test: the point of windowing is
/// the bound, so it should fail loudly if the window grows.
const WINDOW_BUDGET_BYTES: usize = 4 * 1024 * 1024;

/// Synthesize `WINDOW_SECS` of the track starting at `from_ms`.
///
/// The waveform is a pure function of the track and the absolute time, so
/// consecutive windows join seamlessly and a seek lands on the same audio it
/// would have had.
pub fn pcm_window(track: &crate::api::models::Track, from_ms: u64) -> (Vec<f32>, u32) {
    let total = track_seconds(track);
    let start = (from_ms as f32 / 1000.0).min(total);
    let span = WINDOW_SECS.min((total - start).max(0.0));
    (synth(track.id, total, start, span), DEMO_RATE)
}

/// The whole track at once. Only tests and the boot preload use this.
pub fn demo_pcm_for(track: &crate::api::models::Track) -> Vec<f32> {
    let total = track_seconds(track);
    synth(track.id, total, 0.0, total)
}

fn track_seconds(track: &crate::api::models::Track) -> f32 {
    (track.effective_duration_ms() as f32 / 1000.0).clamp(2.0, 400.0)
}

/// A four-note pad, with the fade in and out placed against `total` so a
/// window taken from the middle sounds like the middle.
fn synth(track_id: u64, total: f32, start: f32, span: f32) -> Vec<f32> {
    let rate = DEMO_RATE as f32;
    let frames = (rate * span.max(0.0)) as usize;
    let base = 220.0 * 2f32.powf((track_id % 5) as f32 / 12.0 * 3.0);
    let chord = [1.0, 1.25, 1.5, 2.0];
    let mut out = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let t = start + i as f32 / rate;
        let mut sample = 0.0f32;
        for (ci, &ratio) in chord.iter().enumerate() {
            let f = base * ratio;
            let lfo = 0.5 + 0.5 * (2.0 * std::f32::consts::PI * (0.1 + 0.03 * ci as f32) * t).sin();
            sample += lfo * (2.0 * std::f32::consts::PI * f * t).sin() / chord.len() as f32;
        }
        let fade = (t / 2.0).min(1.0).min((total - t) / 2.0).clamp(0.0, 1.0);
        let v = sample * fade * 0.5;
        out.push(v);
        out.push(v);
    }
    out
}

/// Legacy helper: the 30 s stub pad for a track id.
pub fn demo_pcm(track_id: u64) -> Vec<f32> {
    let mut t = demo_track_stub();
    t.id = track_id;
    demo_pcm_for(&t)
}

fn demo_track_stub() -> crate::api::models::Track {
    use crate::api::models::*;
    Track {
        id: 0,
        title: String::new(),
        duration_ms: Some(30_000),
        full_duration_ms: Some(30_000),
        artwork: None,
        artwork_url: None,
        waveform_url: None,
        user: None,
        publisher_metadata: None,
        metadata_artist: None,
        playback_count: None,
        favoritings_count: None,
        likes_count: None,
        comment_count: None,
        streamable: false,
        downloadable: false,
        preview_start_ms: None,
        preview_end_ms: None,
        genre: None,
        description: None,
        created_at: None,
        permalink_url: None,
        policy: None,
        access: None,
        monetization_model: None,
        feed_reposted: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_demo_tracks() {
        let tracks = demo_tracks();
        assert_eq!(tracks.len(), 10);
        assert!(tracks[0].streamable);
    }

    #[test]
    fn demo_pcm_is_stereo_and_bounded() {
        let pcm = demo_pcm(1000);
        assert_eq!(pcm.len() % 2, 0);
        assert!(pcm.iter().all(|&v| v.abs() <= 1.0));
    }

    #[test]
    fn demo_pcm_has_energy() {
        let pcm = demo_pcm(1002);
        let peak = pcm.iter().fold(0.0f32, |a, &v| a.max(v.abs()));
        assert!(peak > 0.1, "demo audio must be audible: {peak}");
    }

    /// A window must be bounded whatever the track length: synthesizing a
    /// whole four-minute pad as f32 stereo is over 80 MB resident, which is
    /// what made demo mode look like a memory leak.
    #[test]
    fn a_window_is_bounded_and_joins_the_next_one() {
        let track = &demo_tracks()[4];
        let total_ms = track.effective_duration_ms();
        assert!(total_ms > 60_000, "the demo tracks are minutes long");

        let (first, rate) = pcm_window(track, 0);
        assert_eq!(rate, DEMO_RATE);
        let window_frames = (WINDOW_SECS * DEMO_RATE as f32) as usize;
        assert_eq!(first.len(), window_frames * 2);
        assert!(
            first.len() * std::mem::size_of::<f32>() <= WINDOW_BUDGET_BYTES,
            "a window must stay inside its budget, got {} bytes",
            first.len() * std::mem::size_of::<f32>()
        );
        // The whole track would be an order of magnitude more.
        assert!(
            demo_pcm_for(track).len() > first.len() * 8,
            "windowing is not actually saving anything"
        );

        // The next window starts exactly where this one ended.
        let (second, _) = pcm_window(track, (WINDOW_SECS * 1000.0) as u64);
        let whole = demo_pcm_for(track);
        let joint = window_frames * 2;
        assert_eq!(first, whole[..joint], "the first window drifted");
        assert_eq!(
            second[..64],
            whole[joint..joint + 64],
            "the windows do not join"
        );
    }

    /// The last window is short rather than running past the end.
    #[test]
    fn the_final_window_stops_at_the_track_end() {
        let track = &demo_tracks()[0];
        let total_ms = track.effective_duration_ms();
        let (tail, _) = pcm_window(track, total_ms - 1_000);
        let frames = tail.len() / 2;
        assert!(frames > 0);
        assert!(
            frames <= DEMO_RATE as usize + 16,
            "the tail overran: {frames} frames"
        );
        // Past the end there is nothing left to play.
        let (past, _) = pcm_window(track, total_ms + 5_000);
        assert!(past.is_empty());
    }

    /// A window taken from the middle must not fade in: the envelope is
    /// placed against the whole track, not the window.
    #[test]
    fn a_middle_window_is_at_full_level() {
        let track = &demo_tracks()[2];
        let (middle, _) = pcm_window(track, 60_000);
        let peak = middle.iter().fold(0.0f32, |a, &v| a.max(v.abs()));
        assert!(peak > 0.2, "the middle of the track faded in: {peak}");
    }
}
