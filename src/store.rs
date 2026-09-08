//! The live catalogue: every list the views draw, fetched once and cached.
//!
//! ## Why a store rather than fetches inside views
//!
//! egui redraws a view many times a second, and a view cannot await. Calling
//! the API from inside one would either block the frame or fire a request per
//! frame. So every list has a [`Key`], and asking for one is synchronous:
//!
//! ```ignore
//! match app.store.tracks(Key::Likes) {
//!     Slot::Ready(tracks) => track_rows(app, ui, tracks),
//!     Slot::Loading => spinner(ui),
//!     Slot::Failed(why) => ui.label(why),
//! }
//! ```
//!
//! The first ask spawns the fetch and answers `Loading`; the reply lands in
//! the map and the next frame sees `Ready`. Nothing in the UI is async, and a
//! list is fetched once however many views want it.
//!
//! ## Bounds
//!
//! The store is a cache, not a database: [`MAX_ENTRIES`] lists are held,
//! oldest-used first out. Library collections follow every SoundCloud page;
//! discovery shelves intentionally hold at most [`PAGE`] rows. A stale entry
//! ([`FRESH_FOR`]) is re-fetched the next time it is asked for, so the feed
//! and history do not sit still for a whole session.
//!
//! ## What the public API cannot do
//!
//! SoundCloud publishes no charts, no editorial selections and no personal
//! recommendations. The shelves that need them are assembled here from what
//! does exist — related tracks, a genre window sorted by plays, an artist's
//! likes — which is what [`crate::demo::Source`] documents per shelf.

use crate::api::models::{Comment, Playlist, Track, User, WebProfile};
use crate::api::{ApiClient, endpoints};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Rows fetched per list. SoundCloud allows up to 200; 50 is its default page
/// and more than any shelf shows.
pub const PAGE: usize = 50;

/// Lists held before the oldest-used are dropped.
pub const MAX_ENTRIES: usize = 64;

/// How long a fetched list is used without asking again.
pub const FRESH_FOR: Duration = Duration::from_secs(300);

/// A list the UI can ask for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// `GET /me` — the signed-in account.
    Me,
    /// `GET /me/likes/tracks`
    Likes,
    /// `GET /me/playlists`
    MyPlaylists,
    /// `GET /me/likes/playlists`
    LikedPlaylists,
    /// `GET /me/followings`
    Following,
    /// `GET /me/followers`
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "kept for complete account API coverage; not a Library tab"
        )
    )]
    Followers,
    /// `GET /me/followings/tracks`
    FollowingTracks,
    /// `GET /me/tracks`
    MyTracks,
    /// `GET /me/reposts/tracks`
    MyRepostedTracks,
    /// `GET /me/reposts/playlists`
    MyRepostedPlaylists,
    /// `GET /me/feed`
    Feed,
    /// `GET /me/recently-played/tracks`
    History,
    /// `GET /tracks?q=`
    SearchTracks(String),
    /// `GET /playlists?q=`
    SearchPlaylists(String),
    /// `GET /users?q=`
    SearchUsers(String),
    /// `GET /tracks/{urn}`
    Track(u64),
    /// `GET /tracks/{urn}/related` — the closest thing to a station.
    Related(u64),
    /// `GET /tracks/{urn}/comments`
    #[allow(dead_code)]
    Comments(u64),
    /// `GET /tracks/{urn}/favoriters`
    TrackFavoriters(u64),
    /// `GET /tracks/{urn}/reposters`
    TrackReposters(u64),
    /// `GET /playlists/{urn}/tracks`
    PlaylistTracks(u64),
    /// `GET /playlists/{urn}/reposters`
    PlaylistReposters(u64),
    /// `GET /playlists/{urn}`
    Playlist(u64),
    /// `GET /users/{urn}`
    User(u64),
    /// `GET /users/{urn}/tracks`
    UserTracks(u64),
    /// `GET /users/{urn}/likes/tracks` — an artist's picks.
    UserLikes(u64),
    #[allow(dead_code)]
    UserLikedPlaylists(u64),
    UserRepostedTracks(u64),
    UserRepostedPlaylists(u64),
    #[allow(dead_code)]
    UserFollowers(u64),
    #[allow(dead_code)]
    UserFollowings(u64),
    UserWebProfiles(u64),
    /// `GET /users/{urn}/playlists`
    UserPlaylists(u64),
    /// `GET /users/{urn}/related` — artists to watch out for.
    RelatedUsers(u64),
    /// A genre window sorted by plays here, since there is no charts route.
    Genre(String),
}

impl Key {
    /// Whether this list is about the signed-in account, so an app-only token
    /// cannot read it. Asking anyway would only earn a 401.
    pub fn needs_user(&self) -> bool {
        matches!(
            self,
            Key::Me
                | Key::Likes
                | Key::MyPlaylists
                | Key::LikedPlaylists
                | Key::Following
                | Key::Followers
                | Key::FollowingTracks
                | Key::MyTracks
                | Key::MyRepostedTracks
                | Key::MyRepostedPlaylists
                | Key::Feed
                | Key::History
        )
    }
}

/// What a list holds. The variants a view has to handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Loading,
    Ready,
    Failed,
}

/// One cached list.
struct Entry {
    state: State,
    fetched_at: Instant,
    last_used: Instant,
    tracks: Vec<Track>,
    comments: Vec<Comment>,
    playlists: Vec<Playlist>,
    users: Vec<User>,
    web_profiles: Vec<WebProfile>,
    me: Option<crate::api::models::Me>,
    error: Option<String>,
}

impl Entry {
    fn loading() -> Self {
        let now = Instant::now();
        Self {
            state: State::Loading,
            fetched_at: now,
            last_used: now,
            tracks: Vec::new(),
            comments: Vec::new(),
            playlists: Vec::new(),
            users: Vec::new(),
            web_profiles: Vec::new(),
            me: None,
            error: None,
        }
    }

    /// Ready and young enough to use as is.
    fn fresh(&self) -> bool {
        match self.state {
            State::Loading => true,
            State::Ready => self.fetched_at.elapsed() < FRESH_FOR,
            // A failure is not cached: the next ask retries (the network may
            // have come back), but not within a second of the last try.
            State::Failed => self.fetched_at.elapsed() < Duration::from_secs(5),
        }
    }
}

/// What one fetch produced.
enum Payload {
    Tracks(Vec<Track>),
    Comments(Vec<Comment>),
    Playlists(Vec<Playlist>),
    Feed {
        tracks: Vec<Track>,
        playlists: Vec<Playlist>,
    },
    Users(Vec<User>),
    WebProfiles(Vec<WebProfile>),
    Me(Box<crate::api::models::Me>),
}

struct Done {
    key: Key,
    result: Result<Payload, String>,
}

/// The catalogue.
///
/// Cheap to clone (everything is behind one `Arc`), so background tasks can
/// hold it while the UI does.
#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

struct Inner {
    client: Arc<ApiClient>,
    rt: tokio::runtime::Handle,
    entries: Mutex<HashMap<Key, Entry>>,
    tx: crossbeam_channel::Sender<Done>,
    rx: crossbeam_channel::Receiver<Done>,
    /// False while only an app-only token is held, so account lists are not
    /// requested just to be refused.
    signed_in: std::sync::atomic::AtomicBool,
}

impl Store {
    pub fn new(client: Arc<ApiClient>, rt: tokio::runtime::Handle) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            inner: Arc::new(Inner {
                client,
                rt,
                entries: Mutex::new(HashMap::new()),
                tx,
                rx,
                signed_in: std::sync::atomic::AtomicBool::new(false),
            }),
        }
    }

    /// Note whether the account's own lists are reachable.
    pub fn set_signed_in(&self, signed_in: bool) {
        let was = self
            .inner
            .signed_in
            .swap(signed_in, std::sync::atomic::Ordering::SeqCst);
        if was != signed_in {
            // Signing in or out changes what every account list would answer.
            self.forget_account_lists();
        }
    }

    pub fn signed_in(&self) -> bool {
        self.inner
            .signed_in
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Apply whatever finished since the last frame. Call once per frame.
    /// Returns the number of lists that landed, so the caller can repaint.
    pub fn poll(&self) -> usize {
        let mut landed = 0;
        while let Ok(done) = self.inner.rx.try_recv() {
            let mut entries = self.inner.entries.lock();
            let entry = entries.entry(done.key).or_insert_with(Entry::loading);
            entry.fetched_at = Instant::now();
            match done.result {
                Ok(Payload::Tracks(tracks)) => {
                    entry.tracks = tracks;
                    entry.state = State::Ready;
                }
                Ok(Payload::Comments(comments)) => {
                    entry.comments = comments;
                    entry.state = State::Ready;
                }
                Ok(Payload::Playlists(playlists)) => {
                    entry.playlists = playlists;
                    entry.state = State::Ready;
                }
                Ok(Payload::Feed { tracks, playlists }) => {
                    entry.tracks = tracks;
                    entry.playlists = playlists;
                    entry.state = State::Ready;
                }
                Ok(Payload::Users(users)) => {
                    entry.users = users;
                    entry.state = State::Ready;
                }
                Ok(Payload::WebProfiles(profiles)) => {
                    entry.web_profiles = profiles;
                    entry.state = State::Ready;
                }
                Ok(Payload::Me(me)) => {
                    entry.me = Some(*me);
                    entry.state = State::Ready;
                }
                Err(error) => {
                    entry.state = State::Failed;
                    entry.error = Some(error);
                }
            }
            landed += 1;
            drop(entries);
        }
        if landed > 0 {
            self.evict();
        }
        landed
    }

    /// A list's tracks, fetching if needed. `Loading` on the first ask.
    pub fn tracks(&self, key: Key) -> Slot<Vec<Track>> {
        self.slot(key, |entry| entry.tracks.clone())
    }

    #[allow(dead_code)]
    pub fn comments(&self, key: Key) -> Slot<Vec<Comment>> {
        self.slot(key, |entry| entry.comments.clone())
    }

    pub fn playlists(&self, key: Key) -> Slot<Vec<Playlist>> {
        self.slot(key, |entry| entry.playlists.clone())
    }

    pub fn users(&self, key: Key) -> Slot<Vec<User>> {
        self.slot(key, |entry| entry.users.clone())
    }

    pub fn web_profiles(&self, key: Key) -> Slot<Vec<WebProfile>> {
        self.slot(key, |entry| entry.web_profiles.clone())
    }

    /// One track. `Key::Track(id)` holds it as a one-row list.
    pub fn track(&self, id: u64) -> Slot<Track> {
        match self.slot(Key::Track(id), |entry| entry.tracks.clone()) {
            Slot::Ready(tracks) => match tracks.into_iter().next() {
                Some(track) => Slot::Ready(track),
                None => Slot::Failed("that track is not available".to_owned()),
            },
            Slot::Loading => Slot::Loading,
            Slot::Failed(e) => Slot::Failed(e),
            Slot::Unavailable => Slot::Unavailable,
        }
    }

    pub fn playlist(&self, id: u64) -> Slot<Playlist> {
        match self.slot(Key::Playlist(id), |entry| entry.playlists.clone()) {
            Slot::Ready(lists) => match lists.into_iter().next() {
                Some(playlist) => Slot::Ready(playlist),
                None => Slot::Failed("that playlist is not available".to_owned()),
            },
            Slot::Loading => Slot::Loading,
            Slot::Failed(e) => Slot::Failed(e),
            Slot::Unavailable => Slot::Unavailable,
        }
    }

    pub fn user(&self, id: u64) -> Slot<User> {
        match self.slot(Key::User(id), |entry| entry.users.clone()) {
            Slot::Ready(users) => match users.into_iter().next() {
                Some(user) => Slot::Ready(user),
                None => Slot::Failed("that profile is not available".to_owned()),
            },
            Slot::Loading => Slot::Loading,
            Slot::Failed(e) => Slot::Failed(e),
            Slot::Unavailable => Slot::Unavailable,
        }
    }

    pub fn me(&self) -> Slot<crate::api::models::Me> {
        match self.slot(Key::Me, |entry| entry.me.clone()) {
            Slot::Ready(Some(me)) => Slot::Ready(me),
            Slot::Ready(None) => Slot::Loading,
            Slot::Loading => Slot::Loading,
            Slot::Failed(e) => Slot::Failed(e),
            Slot::Unavailable => Slot::Unavailable,
        }
    }

    /// Drop a list so the next ask refetches it (after an edit, say).
    pub fn invalidate(&self, key: &Key) {
        self.inner.entries.lock().remove(key);
    }

    /// Drop every account list — used when the session changes.
    pub fn forget_account_lists(&self) {
        self.inner.entries.lock().retain(|key, _| !key.needs_user());
    }

    pub fn clear(&self) {
        self.inner.entries.lock().clear();
    }

    /// How many lists are held (for Settings' storage line).
    pub fn len(&self) -> usize {
        self.inner.entries.lock().len()
    }

    fn slot<T>(&self, key: Key, take: impl Fn(&Entry) -> T) -> Slot<T> {
        if key.needs_user() && !self.signed_in() {
            return Slot::Unavailable;
        }
        let mut entries = self.inner.entries.lock();
        if let Some(entry) = entries.get_mut(&key) {
            entry.last_used = Instant::now();
            if entry.fresh() {
                return match entry.state {
                    State::Loading => Slot::Loading,
                    State::Ready => Slot::Ready(take(entry)),
                    State::Failed => Slot::Failed(
                        entry
                            .error
                            .clone()
                            .unwrap_or_else(|| "the request failed".to_owned()),
                    ),
                };
            }
            // Stale: keep showing what we have while the refetch runs.
            let stale = (entry.state == State::Ready).then(|| take(entry));
            entry.state = State::Loading;
            entry.fetched_at = Instant::now();
            drop(entries);
            self.spawn(key);
            return match stale {
                Some(value) => Slot::Ready(value),
                None => Slot::Loading,
            };
        }
        entries.insert(key.clone(), Entry::loading());
        drop(entries);
        self.spawn(key);
        Slot::Loading
    }

    /// Start the fetch for a key on the runtime.
    fn spawn(&self, key: Key) {
        let store = self.clone();
        let client = self.inner.client.clone();
        let tx = self.inner.tx.clone();
        self.inner.rt.spawn(async move {
            let result = fetch(&client, &key).await.map_err(|e| {
                log::warn!("catalogue {key:?}: {e}");
                e.user_message()
            });
            let _ = tx.send(Done { key, result });
            drop(store);
        });
    }

    /// Keep at most [`MAX_ENTRIES`], oldest-used out first.
    fn evict(&self) {
        let mut entries = self.inner.entries.lock();
        if entries.len() <= MAX_ENTRIES {
            return;
        }
        let mut by_age: Vec<(Key, Instant)> = entries
            .iter()
            .map(|(key, entry)| (key.clone(), entry.last_used))
            .collect();
        by_age.sort_by_key(|(_, used)| *used);
        let over = entries.len() - MAX_ENTRIES;
        for (key, _) in by_age.into_iter().take(over) {
            entries.remove(&key);
        }
    }
}

/// What a view gets when it asks for a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot<T> {
    /// The request is on the wire.
    Loading,
    Ready(T),
    Failed(String),
    /// This list needs a signed-in account and there is none.
    Unavailable,
}

impl<T> Slot<T> {
    pub fn ready(self) -> Option<T> {
        match self {
            Slot::Ready(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Slot::Loading)
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            Slot::Failed(e) => Some(e),
            _ => None,
        }
    }
}

impl<T> Slot<Vec<T>> {
    /// The rows, or an empty slice while loading/failed — for views that have
    /// their own empty state and do not care why.
    pub fn rows(&self) -> &[T] {
        match self {
            Slot::Ready(rows) => rows,
            _ => &[],
        }
    }
}

/// Run one key's request. Personal library collections follow every page so
/// counts and server ordering match SoundCloud; discovery shelves stay small.
async fn fetch(client: &Arc<ApiClient>, key: &Key) -> crate::api::error::Result<Payload> {
    use Key::*;
    Ok(match key {
        Me => Payload::Me(Box::new(endpoints::me(client).await?)),
        Likes => Payload::Tracks(
            endpoints::my_liked_tracks(client)
                .await
                .collect_all()
                .await?,
        ),
        MyPlaylists => {
            Payload::Playlists(endpoints::my_playlists(client).await.collect_all().await?)
        }
        LikedPlaylists => Payload::Playlists(
            endpoints::my_liked_playlists(client)
                .await
                .collect_all()
                .await?,
        ),
        Following => Payload::Users(endpoints::my_followings(client).await.collect_all().await?),
        Followers => Payload::Users(endpoints::my_followers(client).await.collect_all().await?),
        FollowingTracks => Payload::Tracks(
            endpoints::my_followings_tracks(client)
                .await
                .collect_all()
                .await?,
        ),
        MyTracks => Payload::Tracks(endpoints::my_tracks(client).await.collect_all().await?),
        MyRepostedTracks => Payload::Tracks(
            endpoints::my_reposted_tracks(client)
                .await
                .collect_all()
                .await?,
        ),
        MyRepostedPlaylists => Payload::Playlists(
            endpoints::my_reposted_playlists(client)
                .await
                .collect_all()
                .await?,
        ),
        Feed => {
            let (tracks, playlists) = feed_entries(client).await?;
            Payload::Feed { tracks, playlists }
        }
        History => Payload::Tracks(endpoints::recently_played(client).await?),
        SearchTracks(q) => Payload::Tracks(page(endpoints::search_tracks(client, q).await).await?),
        SearchPlaylists(q) => {
            Payload::Playlists(page(endpoints::search_playlists(client, q).await).await?)
        }
        SearchUsers(q) => Payload::Users(page(endpoints::search_users(client, q).await).await?),
        Track(id) => Payload::Tracks(vec![endpoints::track(client, *id).await?]),
        Related(id) => Payload::Tracks(page(endpoints::related_tracks(client, *id).await).await?),
        Comments(id) => Payload::Comments(
            endpoints::track_comments(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        TrackFavoriters(id) => Payload::Users(
            endpoints::track_favoriters(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        TrackReposters(id) => Payload::Users(
            endpoints::track_reposters(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        PlaylistTracks(id) => Payload::Tracks(
            endpoints::playlist_tracks(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        PlaylistReposters(id) => Payload::Users(
            endpoints::playlist_reposters(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        Playlist(id) => Payload::Playlists(vec![endpoints::playlist(client, *id).await?]),
        User(id) => Payload::Users(vec![endpoints::user(client, *id).await?]),
        UserTracks(id) => Payload::Tracks(
            endpoints::user_tracks(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        UserLikes(id) => Payload::Tracks(
            endpoints::user_liked_tracks(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        UserLikedPlaylists(id) => Payload::Playlists(
            endpoints::user_liked_playlists(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        UserRepostedTracks(id) => Payload::Tracks(
            endpoints::user_reposted_tracks(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        UserRepostedPlaylists(id) => Payload::Playlists(
            endpoints::user_reposted_playlists(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        UserFollowers(id) => Payload::Users(
            endpoints::user_followers(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        UserFollowings(id) => Payload::Users(
            endpoints::user_followings(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        UserWebProfiles(id) => {
            Payload::WebProfiles(endpoints::user_web_profiles(client, *id).await?)
        }
        UserPlaylists(id) => Payload::Playlists(
            endpoints::user_playlists(client, *id)
                .await
                .collect_all()
                .await?,
        ),
        RelatedUsers(id) => {
            Payload::Users(page(endpoints::related_users(client, *id).await).await?)
        }
        Genre(genre) => Payload::Tracks(trending(client, genre).await?),
    })
}

/// One page of a pager.
async fn page<T: serde::de::DeserializeOwned + Send + 'static>(
    mut pager: crate::api::paginated::Pager<T>,
) -> crate::api::error::Result<Vec<T>> {
    let mut rows = pager.next_page().await?;
    rows.truncate(PAGE);
    Ok(rows)
}

/// The feed as a track list: the entries are tracks and playlists mixed, and
/// a playlist's own tracks are not in the payload, so only tracks are kept.
async fn feed_entries(
    client: &Arc<ApiClient>,
) -> crate::api::error::Result<(Vec<Track>, Vec<Playlist>)> {
    use crate::api::models::StreamTarget;
    let entries = page(endpoints::feed(client).await).await?;
    let mut tracks = Vec::new();
    let mut playlists = Vec::new();
    for entry in entries {
        let reposted = entry
            .activity_type
            .as_deref()
            .is_some_and(|kind| kind.to_ascii_lowercase().contains("repost"));
        match entry.target {
            StreamTarget::Track(track) => {
                let mut track = *track;
                track.feed_reposted = reposted;
                tracks.push(track);
            }
            StreamTarget::Playlist(playlist) => {
                let mut playlist = *playlist;
                playlist.feed_reposted = reposted;
                playlists.push(playlist);
            }
        }
    }
    Ok((tracks, playlists))
}

/// "Trending by genre", assembled here: the API publishes no charts, so a
/// recent window of the genre is sorted by play count on our side. That is
/// what [`crate::demo::Source::GenreWindow`] documents.
async fn trending(client: &Arc<ApiClient>, genre: &str) -> crate::api::error::Result<Vec<Track>> {
    const WINDOW_DAYS: i64 = 30;
    let mut tracks = page(endpoints::tracks_by_genre(client, genre, WINDOW_DAYS).await).await?;
    tracks.sort_by_key(|track| std::cmp::Reverse(track.playback_count.unwrap_or(0)));
    Ok(tracks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        let handle = rt.handle().clone();
        std::mem::forget(rt);
        Store::new(Arc::new(ApiClient::demo()), handle)
    }

    /// Account lists must not be requested with an app-only token: the answer
    /// would be a 401 and the user would see an error instead of a prompt.
    #[test]
    fn account_lists_need_a_signed_in_session() {
        let store = store();
        assert!(!store.signed_in());
        for key in [
            Key::Me,
            Key::Likes,
            Key::MyPlaylists,
            Key::LikedPlaylists,
            Key::Following,
            Key::Followers,
            Key::FollowingTracks,
            Key::MyTracks,
            Key::MyRepostedTracks,
            Key::MyRepostedPlaylists,
            Key::Feed,
            Key::History,
        ] {
            assert!(key.needs_user(), "{key:?} should need an account");
            assert!(
                matches!(store.tracks(key.clone()), Slot::Unavailable),
                "{key:?} was requested without a session"
            );
        }
        assert_eq!(store.len(), 0, "nothing should have been fetched");
        // Public lists are asked for either way.
        assert!(!Key::SearchTracks("x".into()).needs_user());
        assert!(!Key::Related(1).needs_user());
        assert!(!Key::Genre("Indie".into()).needs_user());
    }

    /// The first ask starts one fetch and answers `Loading`; asking again in
    /// the same frame must not start a second.
    #[test]
    fn a_list_is_fetched_once() {
        let store = store();
        let key = Key::SearchTracks("hello".into());
        assert!(store.tracks(key.clone()).is_loading());
        assert_eq!(store.len(), 1);
        assert!(store.tracks(key.clone()).is_loading());
        assert_eq!(store.len(), 1, "a second ask started another fetch");
    }

    #[test]
    fn a_landed_list_is_ready_and_a_failure_carries_why() {
        let store = store();
        let key = Key::SearchTracks("q".into());
        assert!(store.tracks(key.clone()).is_loading());
        store
            .inner
            .tx
            .send(Done {
                key: key.clone(),
                result: Ok(Payload::Tracks(vec![track(1), track(2)])),
            })
            .unwrap();
        assert_eq!(store.poll(), 1);
        let rows = store.tracks(key.clone()).ready().expect("ready");
        assert_eq!(rows.len(), 2);

        let bad = Key::SearchTracks("bad".into());
        assert!(store.tracks(bad.clone()).is_loading());
        store
            .inner
            .tx
            .send(Done {
                key: bad.clone(),
                result: Err("HTTP 500".into()),
            })
            .unwrap();
        store.poll();
        assert_eq!(store.tracks(bad).error(), Some("HTTP 500"));
    }

    /// A stale list keeps showing while it refetches: blanking the page every
    /// five minutes would be worse than slightly old rows.
    #[test]
    fn a_stale_list_is_shown_while_it_refreshes() {
        let store = store();
        let key = Key::Genre("Indie".into());
        store.tracks(key.clone());
        store
            .inner
            .tx
            .send(Done {
                key: key.clone(),
                result: Ok(Payload::Tracks(vec![track(9)])),
            })
            .unwrap();
        store.poll();
        // Age it past the freshness window.
        {
            let mut entries = store.inner.entries.lock();
            let entry = entries.get_mut(&key).unwrap();
            entry.fetched_at = Instant::now() - FRESH_FOR - Duration::from_secs(1);
        }
        let rows = store.tracks(key.clone()).ready().expect("still shown");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            store.inner.entries.lock().get(&key).unwrap().state,
            State::Loading,
            "a refetch should be running"
        );
    }

    #[test]
    fn signing_in_or_out_forgets_only_the_account_lists() {
        let store = store();
        store.set_signed_in(true);
        store.tracks(Key::Likes);
        store.tracks(Key::Genre("Indie".into()));
        assert_eq!(store.len(), 2);
        store.set_signed_in(false);
        assert_eq!(store.len(), 1, "only the account list should go");
        assert!(store.tracks(Key::Genre("Indie".into())).is_loading());
    }

    #[test]
    fn the_cache_is_bounded() {
        let store = store();
        for i in 0..(MAX_ENTRIES as u64 + 10) {
            store.tracks(Key::Related(i));
            store
                .inner
                .tx
                .send(Done {
                    key: Key::Related(i),
                    result: Ok(Payload::Tracks(vec![track(i)])),
                })
                .unwrap();
            store.poll();
        }
        assert!(
            store.len() <= MAX_ENTRIES,
            "held {} lists, budget is {MAX_ENTRIES}",
            store.len()
        );
        // The newest survived.
        assert!(
            store
                .inner
                .entries
                .lock()
                .contains_key(&Key::Related(MAX_ENTRIES as u64 + 9))
        );
    }

    #[test]
    fn invalidating_forces_a_refetch() {
        let store = store();
        let key = Key::PlaylistTracks(5);
        store.tracks(key.clone());
        store
            .inner
            .tx
            .send(Done {
                key: key.clone(),
                result: Ok(Payload::Tracks(vec![track(1)])),
            })
            .unwrap();
        store.poll();
        assert!(store.tracks(key.clone()).ready().is_some());
        store.invalidate(&key);
        assert!(store.tracks(key).is_loading());
    }

    /// `rows()` lets a view lay out without matching, and must never panic on
    /// a loading or failed slot.
    #[test]
    fn rows_are_empty_until_ready() {
        let loading: Slot<Vec<Track>> = Slot::Loading;
        assert!(loading.rows().is_empty());
        let failed: Slot<Vec<Track>> = Slot::Failed("nope".into());
        assert!(failed.rows().is_empty());
        let ready = Slot::Ready(vec![track(1)]);
        assert_eq!(ready.rows().len(), 1);
    }

    fn track(id: u64) -> Track {
        Track {
            id,
            title: format!("Track {id}"),
            duration_ms: Some(180_000),
            full_duration_ms: Some(180_000),
            artwork: None,
            artwork_url: None,
            waveform_url: None,
            user: None,
            publisher_metadata: None,
            metadata_artist: None,
            playback_count: Some(id * 10),
            favoritings_count: None,
            likes_count: None,
            comment_count: None,
            streamable: true,
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

    /// Trending is our own sort: the API has no charts route, so the window
    /// must come back ordered by plays.
    #[test]
    fn trending_sorts_by_plays() {
        let mut tracks = [track(1), track(5), track(3)];
        tracks.sort_by_key(|t| std::cmp::Reverse(t.playback_count.unwrap_or(0)));
        assert_eq!(
            tracks.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![5, 3, 1]
        );
    }
}
