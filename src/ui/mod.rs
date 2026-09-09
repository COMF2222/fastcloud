pub mod connect;
pub mod data;
pub mod icons;
pub mod login;
pub mod player_bar;
pub mod queue;
pub mod route;
pub mod theme;
pub mod topbar;
pub mod views;
pub mod visualiser;
pub mod widgets;
pub mod winamp;

use crate::player::Player;
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct App {
    pub player: Arc<Player>,
    pub route: route::Route,
    history: Vec<route::Route>,
    future: Vec<route::Route>,
    pub theme: theme::Theme,
    pub theme_mode: crate::config::ThemeMode,
    pub accent: egui::Color32,

    pub settings: crate::config::Settings,
    pub demo: bool,
    pub search_query: String,
    pub library_tab: views::LibraryTab,
    pub new_playlist_name: String,
    /// Extra track ids dropped into demo playlists (id -> track ids).
    pub extra_tracks: HashMap<u64, Vec<u64>>,
    pub liked: HashSet<u64>,
    pub followed: HashSet<u64>,
    /// Multi-selected track ids (Ctrl/Cmd-click, Shift-range).
    pub selected: std::collections::BTreeSet<u64>,
    /// Range anchor for Shift-click (a track id in the current table).
    pub select_anchor: Option<u64>,
    /// One-frame carousel scroll targets by row salt (arrows page 80%).
    pub carousel: HashMap<String, f32>,
    pub recent_ids: Vec<u64>,
    pending_listen: Option<(u64, u64)>,
    last_pos_ms: u64,
    /// The track the last poll saw, so a change can be noticed once rather than
    /// every frame. Winamp's AUTO reloads a preset on that edge.
    last_track_id: Option<u64>,
    pub show_queue: bool,
    pub show_shortcuts: bool,
    /// Seek drag preview (0..=1) — shown while dragging, committed on release.
    pub seek_preview: Option<f32>,
    /// Volume drag preview (0..=1) — applied live, committed on release.
    pub volume_preview: Option<f32>,
    pub last_volume: f32,
    pub feed_banner_dismissed: bool,
    pub show_reposts: bool,
    pub filter: String,
    pub likes_view_grid: bool,
    pub playlists_mine_only: bool,
    pub recent_at: std::collections::HashMap<u64, u64>,
    pub toasts: Vec<(String, f64)>,
    last_error: Option<String>,
    pub media: Option<crate::desktop::media::MediaIntegration>,
    pub hotkeys: Option<crate::desktop::hotkeys::Hotkeys>,
    pub ipc_rx: Option<crossbeam_channel::Receiver<crate::cli::IpcMessage>>,
    /// Own the native tray icon for the lifetime of the UI. Dropping this
    /// handle removes the icon even though its command receiver stays alive.
    _tray: Option<crate::desktop::tray::Tray>,
    pub tray_rx: Option<std::sync::mpsc::Receiver<crate::desktop::tray::TrayCommand>>,
    pub quit_requested: bool,
    pub version_build: &'static str,
    /// Bounded artwork loader (RAM budget + disk cache, see `images.rs`).
    pub art: crate::images::ArtLoader,
    /// Real SoundCloud waveform samples, fetched lazily from `waveform_url`.
    pub waveforms: crate::waveforms::WaveformLoader,
    last_art_evict: f64,
    /// Tokio handle for background resolves (link open, stations).
    rt: tokio::runtime::Handle,
    /// Optimistic playlist edits: shown at once, written behind (see
    /// `crate::playlists`). SoundCloud has no snapshot id, so a failed write
    /// rolls the shown list back to what the server confirmed.
    pub playlist_edits: crate::playlists::Edits,
    /// Selected section on an artist profile.
    pub user_tab: views::UserTab,
    /// Selected category on the search results page.
    pub search_tab: views::SearchTab,
    /// The Winamp mini player, once `Ctrl+M` has opened it. Behind a mutex
    /// because egui's deferred-viewport callback is `Fn + Send + Sync` and
    /// outlives the frame that registered it, so it cannot borrow `App`.
    pub mini: Option<Arc<parking_lot::Mutex<winamp::MiniPlayer>>>,
    /// What the mini player shows and asks for; shared with its window.
    mini_shared: Option<Arc<std::sync::Mutex<winamp::Shared>>>,
    /// Whether the window is currently wearing the skin, so decorations and
    /// resizability are only changed when the mode actually flips.
    window_is_mini: Option<bool>,
    /// The big window's size, kept while the skin is up so closing the mini
    /// player restores it rather than leaving a 275×116 app.
    big_window_size: Option<egui::Vec2>,
    /// The big window's desktop position. Resizing the one native window into
    /// the skin moves it on some window managers, so restore both halves of
    /// its geometry when the skin closes.
    big_window_pos: Option<egui::Pos2>,
    /// The mini player's own desktop position.  Big and mini mode each keep
    /// their geometry, so switching modes does not make either window jump.
    mini_window_pos: Option<egui::Pos2>,
    /// Windows may apply an outer position before the asynchronous resize and
    /// then move the window again. Repeat the saved position for a few frames.
    restore_window_pos: Option<(egui::Pos2, u8)>,
    /// True only while a newly opened mini player waits for its first native
    /// resize. Scale changes are rendered live and must never reveal Home.
    mini_transitioning: bool,
    /// When the hidden resize began. Windows can occasionally ignore a
    /// resize; the timeout prevents the only app window staying hidden.
    mini_transition_started: Option<f64>,
    /// When the last resize was asked for, so a compositor that refuses one is
    /// not fought every frame.
    mini_fit_asked: f64,
    /// Set when the mini player's minimize button was pressed; sent to the
    /// window manager on the next frame (a `ViewportCommand` needs the
    /// context, which `poll_mini` does not have).
    minimize_requested: bool,
    /// Set when something asked for the window back (the tray, a second
    /// launch). Sent the same way as `minimize_requested`.
    raise_requested: bool,
    /// SoundCloud link waiting to be resolved (CLI, tray, cold boot).
    pub pending_link: Option<String>,
    link_tx: crossbeam_channel::Sender<LinkDone>,
    link_rx: crossbeam_channel::Receiver<LinkDone>,
    /// The live catalogue every view reads from (see [`crate::store`]).
    pub store: crate::store::Store,
    /// The OAuth session, when credentials were found. `None` until an
    /// application is registered (see [`connect`]).
    pub session: Option<Arc<crate::auth::Session>>,
    /// Where the app stands with SoundCloud: the connect screen draws this, and
    /// [`eframe::App::ui`] gates the interface on it.
    pub connection: connect::Connection,
    /// Whether this machine already has an application registered, even though
    /// no session came up — the network was down at launch, say. Read once at
    /// startup rather than per frame, because it is a keyring lookup.
    pub stored_app: bool,
    /// Steps of a running connect flow.
    connect_tx: crossbeam_channel::Sender<connect::ConnectStep>,
    connect_rx: crossbeam_channel::Receiver<connect::ConnectStep>,
    /// Set to stop a connect flow: the user pressed Cancel.
    connect_cancel: Option<tokio::sync::watch::Sender<bool>>,
    /// A sign-in or sign-out running in the background.
    auth_tx: crossbeam_channel::Sender<AuthDone>,
    auth_rx: crossbeam_channel::Receiver<AuthDone>,
    /// Like/follow writes running in the background.
    social_tx: crossbeam_channel::Sender<SocialDone>,
    social_rx: crossbeam_channel::Receiver<SocialDone>,
    /// True while the browser flow is out.
    pub signing_in: bool,
    pub auth_url: Option<String>,
    pub auth_error: Option<String>,
    auth_task: Option<tokio::task::JoinHandle<()>>,
    library_synced: (bool, bool),
    /// Drafts and progress for creator-only API features. Kept together so
    /// browsing listeners do not pay for scattered state throughout the UI.
    pub creator: CreatorState,
}

#[derive(Debug, Default)]
pub struct CreatorState {
    pub upload_path: String,
    pub upload_title: String,
    pub upload_artist: String,
    pub upload_description: String,
    pub upload_genre: String,
    pub upload_tags: String,
    pub upload_public: bool,
    pub uploading: bool,
    pub edit_track_id: Option<u64>,
    pub edit_title: String,
    pub edit_artist: String,
    pub edit_description: String,
    pub storefront_track_id: Option<u64>,
    pub storefront_title: String,
    pub storefront_kind: String,
    pub storefront_link: String,
    pub storefront_link_title: String,
    pub storefront_description: String,
    pub storefront_price: String,
    pub confirm_delete_track: Option<u64>,
    pub confirm_delete_playlist: Option<u64>,
}

/// Result of a background sign-in / sign-out.
pub enum AuthDone {
    Authorizing(String),
    SignedIn,
    SignedOut,
    Disconnected,
    /// An app-only token was minted, so public browsing works.
    AppTokenReady,
    Failed(String),
}

/// A finished like/follow write. `error` is `None` on success.
pub enum SocialDone {
    Like {
        track_id: u64,
        liked: bool,
        error: Option<String>,
    },
    Follow {
        user_id: u64,
        following: bool,
        error: Option<String>,
    },
    PlaylistCreated(Result<crate::api::models::Playlist, String>),
    PlaylistDeleted {
        id: u64,
        result: Result<(), String>,
    },
    TrackUploaded(Result<crate::api::models::Track, String>),
    TrackUpdated(Result<crate::api::models::Track, String>),
    TrackDeleted {
        id: u64,
        result: Result<(), String>,
    },
    StorefrontUpdated(Result<crate::api::models::Storefront, String>),
    Mutation {
        success: String,
        refresh: Vec<crate::store::Key>,
        error: Option<String>,
    },
}

/// Result of resolving a SoundCloud link in the background.
/// Each variant carries the raw link for the messages inbox.
///
/// The track is boxed: it is by far the biggest variant, and this travels
/// through a channel where every message would otherwise be that wide.
pub enum LinkDone {
    PlayTrack(Box<crate::api::models::Track>, String),
    OpenPlaylist(u64, String),
    OpenUser(u64, String),
    Failed(String),
}

impl App {
    /// Build the UI shell around an initialized player.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        player: Arc<Player>,
        settings: crate::config::Settings,
        demo: bool,
        media: Option<crate::desktop::media::MediaIntegration>,
        hotkeys: Option<crate::desktop::hotkeys::Hotkeys>,
        ipc_rx: Option<crossbeam_channel::Receiver<crate::cli::IpcMessage>>,
        tray: Option<crate::desktop::tray::Tray>,
        tray_rx: Option<std::sync::mpsc::Receiver<crate::desktop::tray::TrayCommand>>,
        art: crate::images::ArtLoader,
        rt: tokio::runtime::Handle,
        store: crate::store::Store,
        session: Option<Arc<crate::auth::Session>>,
    ) -> Self {
        let theme_mode = settings.theme;
        let theme = theme::Theme::from_mode(theme_mode, theme::ORANGE);
        // Seed a lively demo library on first run (persisted afterwards).
        let liked: HashSet<u64> = if demo {
            [1000, 1002, 1004].into_iter().collect()
        } else {
            HashSet::new()
        };
        let followed: HashSet<u64> = if demo {
            [12, 14].into_iter().collect()
        } else {
            HashSet::new()
        };
        let (link_tx, link_rx) = crossbeam_channel::unbounded();
        let (auth_tx, auth_rx) = crossbeam_channel::unbounded();
        let (social_tx, social_rx) = crossbeam_channel::unbounded();
        let (connect_tx, connect_rx) = crossbeam_channel::unbounded();
        // Demo mode is a deliberate choice (`--demo`); a live session means
        // there is an application to call with. Neither: the connect screen.
        let connection = if demo || session.is_some() {
            connect::Connection::Connected { username: None }
        } else {
            connect::Connection::Disconnected
        };
        // Credentials on disk but no session means something failed at launch
        // rather than that this user has never connected, and the screen offers
        // a retry instead of a fresh sign-in.
        let stored_app =
            session.is_none() && !demo && crate::auth::AppCredentials::discover().is_some();
        Self {
            player,
            route: route::Route::Home,
            history: Vec::new(),
            future: Vec::new(),
            theme,
            theme_mode,
            accent: theme::ORANGE,
            settings,
            demo,
            search_query: String::new(),
            library_tab: views::LibraryTab::Likes,
            new_playlist_name: String::new(),
            extra_tracks: HashMap::new(),
            liked,
            followed,
            selected: std::collections::BTreeSet::new(),
            select_anchor: None,
            carousel: HashMap::new(),
            recent_ids: Vec::new(),
            pending_listen: None,
            last_pos_ms: 0,
            last_track_id: None,
            show_queue: false,
            show_shortcuts: false,
            seek_preview: None,
            volume_preview: None,
            last_volume: 0.8,
            feed_banner_dismissed: false,
            show_reposts: true,
            filter: String::new(),
            likes_view_grid: true,
            playlists_mine_only: false,
            recent_at: std::collections::HashMap::new(),
            last_error: None,
            toasts: Vec::new(),
            media,
            hotkeys,
            ipc_rx,
            _tray: tray,
            tray_rx,
            quit_requested: false,
            version_build: concat!("v", env!("CARGO_PKG_VERSION")),
            art,
            waveforms: crate::waveforms::WaveformLoader::new(rt.clone()),
            last_art_evict: 0.0,
            rt,
            playlist_edits: crate::playlists::Edits::new(),
            user_tab: views::UserTab::default(),
            search_tab: views::SearchTab::default(),
            mini: None,
            mini_shared: None,
            window_is_mini: None,
            big_window_size: None,
            big_window_pos: None,
            mini_window_pos: None,
            restore_window_pos: None,
            mini_transitioning: false,
            mini_transition_started: None,
            mini_fit_asked: 0.0,
            minimize_requested: false,
            raise_requested: false,
            pending_link: None,
            link_tx,
            link_rx,
            store,
            session,
            connection,
            stored_app,
            connect_tx,
            connect_rx,
            connect_cancel: None,
            auth_tx,
            auth_rx,
            social_tx,
            social_rx,
            signing_in: false,
            auth_url: None,
            auth_error: None,
            auth_task: None,
            library_synced: (false, false),
            creator: CreatorState {
                upload_public: true,
                storefront_kind: "digital".to_owned(),
                ..CreatorState::default()
            },
        }
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.toasts.push((msg.into(), 0.0));
    }

    /// Toast, but only when the message changed since the last call
    /// (for repeating engine errors).
    pub fn toast_once(&mut self, msg: String) {
        if self.last_error.as_ref() != Some(&msg) {
            self.last_error = Some(msg.clone());
            self.toasts.push((msg, 0.0));
        }
    }

    pub fn clear_error_notice(&mut self) {
        self.last_error = None;
    }

    /// User-initiated navigation: pushes the current route to history.
    pub fn navigate(&mut self, route: route::Route) {
        if self.route == route {
            return;
        }
        // Typing in search updates the query without spamming history.
        let search_edit = matches!(&self.route, route::Route::Search(_))
            && matches!(&route, route::Route::Search(_));
        if !search_edit {
            self.history.push(self.route.clone());
            if self.history.len() > 100 {
                self.history.remove(0);
            }
        }
        self.future.clear();
        if let route::Route::Search(q) = &route {
            self.search_query = q.clone();
        }
        self.filter.clear();
        self.route = route;
    }

    pub fn can_go_back(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn go_back(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.future.push(self.route.clone());
            self.route = prev;
            if let route::Route::Search(q) = &self.route {
                self.search_query = q.clone();
            }
        }
    }

    pub fn go_forward(&mut self) {
        if let Some(next) = self.future.pop() {
            self.history.push(self.route.clone());
            self.route = next;
            if let route::Route::Search(q) = &self.route {
                self.search_query = q.clone();
            }
        }
    }

    // ===== Library state =====

    pub fn is_liked(&self, id: u64) -> bool {
        self.liked.contains(&id)
    }

    /// Toggle a like; returns the new state.
    ///
    /// Optimistic: the heart fills at once and the write goes out behind it,
    /// because a like that waits for a round trip feels broken. A rejected
    /// write puts the heart back and says why (see [`Self::poll_social`]).
    pub fn toggle_like(&mut self, id: u64) -> bool {
        let now = if self.liked.remove(&id) {
            false
        } else {
            self.liked.insert(id);
            true
        };
        self.persist_library();
        self.write_like(id, now);
        now
    }

    /// Toggle a follow; returns the new state. Optimistic, like the like.
    pub fn toggle_follow(&mut self, user_id: u64) -> bool {
        let now = if self.followed.remove(&user_id) {
            false
        } else {
            self.followed.insert(user_id);
            true
        };
        self.persist_library();
        self.write_follow(user_id, now);
        now
    }

    /// Account-backed follow state used by every follow button.
    pub fn is_following(&self, user_id: u64) -> bool {
        self.followed.contains(&user_id)
    }

    /// Send a like/unlike. Demo mode and public-only sessions keep it local:
    /// `/likes/tracks/{urn}` needs the account.
    fn write_like(&mut self, track_id: u64, liked: bool) {
        if self.demo || !self.signed_in() {
            return;
        }
        let client = self.player.api().clone();
        let tx = self.social_tx.clone();
        self.rt.spawn(async move {
            let result = if liked {
                crate::api::endpoints::like_track(&client, track_id).await
            } else {
                crate::api::endpoints::unlike_track(&client, track_id).await
            };
            let _ = tx.send(SocialDone::Like {
                track_id,
                liked,
                error: result.err().map(|e| e.to_string()),
            });
        });
    }

    fn write_follow(&mut self, user_id: u64, following: bool) {
        if self.demo || !self.signed_in() {
            return;
        }
        let client = self.player.api().clone();
        let tx = self.social_tx.clone();
        self.rt.spawn(async move {
            let result = if following {
                crate::api::endpoints::follow_user(&client, user_id).await
            } else {
                crate::api::endpoints::unfollow_user(&client, user_id).await
            };
            let _ = tx.send(SocialDone::Follow {
                user_id,
                following,
                error: result.err().map(|e| e.to_string()),
            });
        });
    }

    /// Apply finished like/follow writes: a success refreshes the affected
    /// list, a failure undoes the optimistic change.
    fn poll_social(&mut self) {
        while let Ok(done) = self.social_rx.try_recv() {
            match done {
                SocialDone::Like {
                    track_id,
                    liked,
                    error,
                } => match error {
                    None => self.store.invalidate(&crate::store::Key::Likes),
                    Some(why) => {
                        // Put the heart back where the server has it.
                        if liked {
                            self.liked.remove(&track_id);
                        } else {
                            self.liked.insert(track_id);
                        }
                        self.persist_library();
                        self.toast(format!("Like not saved: {why}"));
                    }
                },
                SocialDone::Follow {
                    user_id,
                    following,
                    error,
                } => match error {
                    None => {
                        self.library_synced.1 = false;
                        self.store.invalidate(&crate::store::Key::Following);
                    }
                    Some(why) => {
                        if following {
                            self.followed.remove(&user_id);
                        } else {
                            self.followed.insert(user_id);
                        }
                        self.persist_library();
                        self.toast(format!("Follow not saved: {why}"));
                    }
                },
                SocialDone::PlaylistCreated(result) => match result {
                    Ok(playlist) => {
                        self.store.invalidate(&crate::store::Key::MyPlaylists);
                        self.toast(format!("Created playlist {}", playlist.title));
                        self.navigate(crate::ui::route::Route::PlaylistDetail(playlist.id));
                    }
                    Err(why) => self.toast(format!("Playlist not created: {why}")),
                },
                SocialDone::PlaylistDeleted { id, result } => match result {
                    Ok(()) => {
                        self.creator.confirm_delete_playlist = None;
                        self.store.invalidate(&crate::store::Key::MyPlaylists);
                        self.store.invalidate(&crate::store::Key::Playlist(id));
                        self.toast("Playlist deleted from SoundCloud");
                        self.navigate(crate::ui::route::Route::Library);
                    }
                    Err(why) => self.toast(format!("Playlist not deleted: {why}")),
                },
                SocialDone::TrackUploaded(result) => {
                    self.creator.uploading = false;
                    match result {
                        Ok(track) => {
                            self.store.invalidate(&crate::store::Key::MyTracks);
                            self.creator.upload_path.clear();
                            self.creator.upload_title.clear();
                            self.creator.upload_description.clear();
                            self.creator.upload_genre.clear();
                            self.creator.upload_tags.clear();
                            self.toast(format!("Uploaded {}", track.title));
                            self.navigate(crate::ui::route::Route::TrackDetail(track.id));
                        }
                        Err(why) => self.toast(format!("Track not uploaded: {why}")),
                    }
                }
                SocialDone::TrackUpdated(result) => match result {
                    Ok(track) => {
                        self.store.invalidate(&crate::store::Key::Track(track.id));
                        self.store.invalidate(&crate::store::Key::MyTracks);
                        self.creator.edit_track_id = None;
                        self.toast("Track updated on SoundCloud");
                    }
                    Err(why) => self.toast(format!("Track not updated: {why}")),
                },
                SocialDone::TrackDeleted { id, result } => match result {
                    Ok(()) => {
                        self.store.invalidate(&crate::store::Key::Track(id));
                        self.store.invalidate(&crate::store::Key::MyTracks);
                        self.creator.confirm_delete_track = None;
                        self.toast("Track deleted from SoundCloud");
                        self.navigate(crate::ui::route::Route::Library);
                    }
                    Err(why) => self.toast(format!("Track not deleted: {why}")),
                },
                SocialDone::StorefrontUpdated(result) => match result {
                    Ok(storefront) => {
                        self.creator.storefront_track_id = None;
                        self.toast(format!("Storefront saved: {}", storefront.title));
                    }
                    Err(why) => self.toast(format!("Storefront not saved: {why}")),
                },
                SocialDone::Mutation {
                    success,
                    refresh,
                    error,
                } => match error {
                    Some(why) => self.toast(why),
                    None => {
                        for key in refresh {
                            self.store.invalidate(&key);
                        }
                        self.toast(success);
                    }
                },
            }
        }
    }

    /// Ctrl/Cmd-click: toggle one row, move the Shift-anchor to it.
    pub fn toggle_select(&mut self, id: u64) {
        if !self.selected.remove(&id) {
            self.selected.insert(id);
        }
        self.select_anchor = Some(id);
    }

    /// Shift-click: select anchor..=id in table order (`order` holds the
    /// table's ids top to bottom). Without an anchor, behaves as toggle.
    pub fn select_range(&mut self, order: &[u64], id: u64) {
        let Some(anchor) = self.select_anchor else {
            self.toggle_select(id);
            return;
        };
        match range_between(order, anchor, id) {
            Some((lo, hi)) => {
                self.selected.extend(order[lo..=hi].iter().copied());
                self.select_anchor = Some(id);
            }
            None => self.toggle_select(id),
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.select_anchor = None;
    }

    // ===== Winamp mini player =====

    /// Open or close the mini player (`Ctrl+M`, or the button in the top bar).
    ///
    /// The window is a deferred viewport, so it comes and goes with this flag
    /// rather than being created and destroyed. Opening it loads the skin from
    /// settings, falling back to the built-in one.
    pub fn toggle_mini(&mut self) {
        if self.mini.is_some() {
            self.close_mini();
            return;
        }
        // Opening the player from the main UI should reveal its controls.
        // Restoring at startup still uses the saved shade state.
        self.settings.winamp_shade = false;
        self.open_mini();
    }

    fn open_mini(&mut self) {
        let skin = self.load_skin();
        let player = winamp::MiniPlayer::new(skin, self.settings.winamp_scale, self.player.clone());
        {
            let mut shared = player.shared.lock().unwrap_or_else(|p| p.into_inner());
            shared.visualiser = self.settings.visualiser;
            // The windows come back the way they were left.
            shared.stack = self.window_stack();
            shared.on_top = self.settings.winamp_on_top;
            shared.eq = self.eq_settings();
            shared.has_preset = self.current_eq_preset().is_some();
        }
        self.mini_shared = Some(player.shared.clone());
        self.mini = Some(Arc::new(parking_lot::Mutex::new(player)));
        self.mini_transitioning = true;
        self.mini_transition_started = None;
        self.remember_window_mode(true);
    }

    /// Which windows are open and rolled up, from settings.
    fn window_stack(&self) -> winamp::Stack {
        let mut stack = winamp::Stack {
            equalizer: self.settings.winamp_eq_window,
            playlist: self.settings.winamp_pl_window,
            playlist_rows: self.settings.winamp_pl_rows,
            ..winamp::Stack::default()
        };
        stack.set_shade(winamp::Window::Main, self.settings.winamp_shade);
        stack.set_shade(winamp::Window::Equalizer, self.settings.winamp_eq_shade);
        stack.set_shade(winamp::Window::Playlist, self.settings.winamp_pl_shade);
        stack
    }

    /// Write a stack back to settings, and save.
    fn remember_window_stack(&mut self, stack: winamp::Stack) {
        self.settings.winamp_eq_window = stack.equalizer;
        self.settings.winamp_pl_window = stack.playlist;
        self.settings.winamp_pl_rows = stack.playlist_rows;
        self.settings.winamp_shade = stack.is_shade(winamp::Window::Main);
        self.settings.winamp_eq_shade = stack.is_shade(winamp::Window::Equalizer);
        self.settings.winamp_pl_shade = stack.is_shade(winamp::Window::Playlist);
        if let Err(e) = self.settings.save() {
            self.toast(format!("Failed to save: {e}"));
        }
    }

    /// The equaliser's settings as the skin wants them.
    fn eq_settings(&self) -> winamp::EqSettings {
        winamp::EqSettings {
            enabled: self.settings.eq_enabled,
            auto: self.settings.eq_auto,
            preamp_db: self.settings.eq_preamp_db,
            gains_db: self.settings.eq_gains_db,
        }
    }

    /// The playing track's saved preset, if it has one.
    fn current_eq_preset(&self) -> Option<crate::config::EqPreset> {
        let id = self.player.current_track()?.id;
        self.settings
            .eq_presets
            .iter()
            .find(|preset| preset.track_id == id)
            .copied()
    }

    /// Come back up in the mini player, because that is how it was left.
    ///
    /// The window is already built for the skin (see `main`), so the mode is
    /// noted as applied — asking for decorations and a resize on the first
    /// frame would fight a window manager that has just honoured them.
    pub fn restore_mini(&mut self) {
        self.open_mini();
        // `main` created the native window at the skin's size already.
        self.mini_transitioning = false;
        self.mini_transition_started = None;
        self.window_is_mini = Some(true);
    }

    pub fn close_mini(&mut self) {
        self.mini = None;
        self.mini_shared = None;
        self.mini_transitioning = false;
        self.mini_transition_started = None;
        self.remember_window_mode(false);
    }

    /// Persist which window the app is in, so it comes back the same way.
    fn remember_window_mode(&mut self, mini: bool) {
        if self.settings.winamp_window == mini {
            return;
        }
        self.settings.winamp_window = mini;
        if let Err(e) = self.settings.save() {
            self.toast(format!("Failed to save: {e}"));
        }
    }

    pub fn mini_open(&self) -> bool {
        self.mini.is_some()
    }

    /// The skin from settings, or the built-in one when there is none or it
    /// cannot be read. A bad path is not fatal: the mini player still opens.
    fn load_skin(&mut self) -> crate::skin::Skin {
        let Some(path) = self.settings.winamp_skin.clone() else {
            return crate::skin::stock::stock();
        };
        match std::fs::read(&path) {
            Ok(bytes) => {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Skin".to_owned());
                match crate::skin::Skin::from_wsz(name, bytes) {
                    Ok(mut skin) => {
                        skin.fill_missing_from(&crate::skin::stock::stock());
                        skin
                    }
                    Err(e) => {
                        self.toast(format!("Skin not loaded: {e}"));
                        crate::skin::stock::stock()
                    }
                }
            }
            Err(e) => {
                self.toast(format!("Skin not read: {e}"));
                crate::skin::stock::stock()
            }
        }
    }

    /// Install a classic Winamp skin and wear it immediately.
    ///
    /// The selected archive is copied into Fastcloud's data directory. The
    /// remembered skin therefore keeps working when the original download is
    /// moved, deleted, or selected from a temporary browser directory.
    pub fn apply_skin_file(&mut self, path: std::path::PathBuf) {
        // A `.wal` is a Winamp *5* skin: a different format entirely (XML
        // layout, PNG art), so say so rather than failing to find main.bmp.
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("wal"))
        {
            self.toast("Winamp 5 skins (.wal) are not supported — use a classic .wsz");
            return;
        }
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) => {
                self.toast(format!("Skin not read: {e}"));
                return;
            }
        };
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Skin".to_owned());
        match crate::skin::Skin::from_wsz(name.clone(), bytes) {
            Ok(mut skin) => {
                skin.fill_missing_from(&crate::skin::stock::stock());
                let installed_path = match crate::config::app_paths().and_then(|paths| {
                    let file_name = path
                        .file_name()
                        .map(std::ffi::OsStr::to_os_string)
                        .unwrap_or_else(|| std::ffi::OsString::from("skin.wsz"));
                    let installed = paths.skin_dir.join(file_name);
                    if path != installed {
                        std::fs::copy(&path, &installed)?;
                    }
                    Ok(installed)
                }) {
                    Ok(installed) => installed,
                    Err(e) => {
                        self.toast(format!("Skin not installed: {e}"));
                        return;
                    }
                };
                self.settings.winamp_skin = Some(installed_path);
                if let Err(e) = self.settings.save() {
                    self.toast(format!("Failed to save: {e}"));
                }
                if let Some(mini) = &self.mini {
                    mini.lock().set_skin(skin);
                } else {
                    // Dropping a skin with the window closed opens it.
                    let player = winamp::MiniPlayer::new(
                        skin,
                        self.settings.winamp_scale,
                        self.player.clone(),
                    );
                    {
                        let mut shared = player.shared.lock().unwrap_or_else(|p| p.into_inner());
                        shared.visualiser = self.settings.visualiser;
                        shared.stack = self.window_stack();
                        shared.on_top = self.settings.winamp_on_top;
                        shared.eq = self.eq_settings();
                        shared.has_preset = self.current_eq_preset().is_some();
                    }
                    self.mini_shared = Some(player.shared.clone());
                    self.mini = Some(Arc::new(parking_lot::Mutex::new(player)));
                    // Installing from the full-size window is the same native
                    // resize transition as Ctrl+M. Without this flag the mode
                    // switch hides the window, but `mini_ready_to_draw`
                    // considers it already ready and never reveals it again.
                    self.mini_transitioning = true;
                    self.mini_transition_started = None;
                    self.remember_window_mode(true);
                }
                self.toast(format!("Skin: {name}"));
            }
            Err(e) => self.toast(format!("Not a Winamp skin: {e}")),
        }
    }

    /// Adopt a dropped font file as the interface face.
    ///
    /// egui installs fonts once, when the context is created, so this stores
    /// the path and asks for a restart rather than pretending to hot-swap.
    pub fn apply_interface_font(&mut self, path: std::path::PathBuf) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "that file".to_owned());
        // Parse it now: a face that cannot be read should be refused while the
        // user is watching, not silently ignored on the next start.
        if crate::fonts::custom_face(&path).is_none() {
            self.toast(format!("{name} is not a font Fastcloud can read"));
            return;
        }
        self.settings.interface_font = Some(path);
        match self.settings.save() {
            Ok(()) => self.toast(format!("Interface font: {name} — restart to apply")),
            Err(e) => self.toast(format!("Failed to save: {e}")),
        }
    }

    /// Go back to the generated skin.
    pub fn use_stock_skin(&mut self) {
        self.settings.winamp_skin = None;
        if let Err(e) = self.settings.save() {
            self.toast(format!("Failed to save: {e}"));
        }
        if let Some(mini) = &self.mini {
            mini.lock().set_skin(crate::skin::stock::stock());
        }
        self.toast("Skin: Fastcloud");
    }

    /// Set the mini player's whole-pixel scale.
    pub fn set_mini_scale(&mut self, scale: u32) {
        let scale = scale.clamp(1, 4);
        self.settings.winamp_scale = scale;
        if let Err(e) = self.settings.save() {
            self.toast(format!("Failed to save: {e}"));
        }
        if let Some(mini) = &self.mini {
            mini.lock().scale = scale;
        }
    }

    /// Apply what the mini player asked for.
    ///
    /// Transport goes straight from that window to the player, so only what
    /// needs `App` — which windows are open, and anything persisted — comes
    /// through here.
    fn poll_mini(&mut self) {
        let Some(shared) = self.mini_shared.clone() else {
            return;
        };
        let commands = {
            let mut shared = shared.lock().unwrap_or_else(|p| p.into_inner());
            // Keep the window's trace, pin and equaliser in step with the app:
            // the skin reads these rather than being told.
            shared.visualiser = self.settings.visualiser;
            shared.on_top = self.settings.winamp_on_top;
            shared.eq = winamp::EqSettings {
                enabled: self.settings.eq_enabled,
                auto: self.settings.eq_auto,
                preamp_db: self.settings.eq_preamp_db,
                gains_db: self.settings.eq_gains_db,
            };
            std::mem::take(&mut shared.commands)
        };
        for command in commands {
            match command {
                winamp::Command::Close => self.close_mini(),
                winamp::Command::Minimize => self.minimize_requested = true,
                winamp::Command::ToggleShade(window) => self.toggle_shade(window),
                winamp::Command::ToggleWindow(window) => self.toggle_skin_window(window),
                winamp::Command::ResizePlaylist(rows) => self.resize_playlist(rows),
                winamp::Command::Volume(percent) => {
                    self.settings.volume = f32::from(percent.min(100)) / 100.0;
                }
                winamp::Command::Balance(percent) => {
                    self.settings.balance = f32::from(percent.clamp(-100, 100)) / 100.0;
                }
                winamp::Command::CycleVisualiser => {
                    self.settings.visualiser = self.settings.visualiser.next();
                    if let Err(e) = self.settings.save() {
                        self.toast(format!("Failed to save: {e}"));
                    }
                }
                winamp::Command::ToggleOnTop => {
                    self.settings.winamp_on_top = !self.settings.winamp_on_top;
                    // The level is applied by `apply_window_mode`, which only
                    // acts when something changed — so tell it something did.
                    self.forget_window_mode();
                    if let Err(e) = self.settings.save() {
                        self.toast(format!("Failed to save: {e}"));
                    }
                }
                winamp::Command::SetMono(mono) => {
                    self.settings.mono = mono;
                    self.player.set_mono(mono);
                    if let Err(e) = self.settings.save() {
                        self.toast(format!("Failed to save: {e}"));
                    }
                }
                winamp::Command::CycleScale => {
                    let next = match self.settings.winamp_scale {
                        0 | 1 | 4.. => 2,
                        scale => scale + 1,
                    };
                    self.set_mini_scale(next);
                }
                winamp::Command::SetScale(scale) => self.set_mini_scale(scale),
                winamp::Command::TrackInfoCurrent => {
                    let id = {
                        let state = self.player.state.lock();
                        state
                            .current
                            .and_then(|index| state.queue.get(index).map(|track| track.id))
                    };
                    if let Some(id) = id {
                        self.navigate(route::Route::TrackDetail(id));
                        self.close_mini();
                    }
                }
                winamp::Command::About => {
                    let skin = self
                        .mini
                        .as_ref()
                        .map(|m| m.lock().skin_name().to_owned())
                        .unwrap_or_default();
                    self.toast(format!("Fastcloud {} — skin: {skin}", self.version_build));
                }
                winamp::Command::Eq(eq) => self.apply_eq(eq),
                winamp::Command::SaveEqPreset => self.save_eq_preset(),
                winamp::Command::ClearEqPreset => self.clear_eq_preset(),
                winamp::Command::PlayAt(index) => {
                    self.player.skip_to(index);
                }
                winamp::Command::RemoveTracks(indices) => self.remove_from_queue(&indices),
                winamp::Command::CropQueue(keep) => self.crop_queue(&keep),
                winamp::Command::ClearQueue => self.clear_queue(),
                winamp::Command::SortQueue(by) => self.sort_queue(by),
                winamp::Command::ShuffleQueue => self.player.set_shuffle(true),
                winamp::Command::SaveQueue => {
                    // The dialog lives in the app's own interface, so this is
                    // one of the few things that has to leave the skin.
                    self.show_queue = true;
                    self.close_mini();
                    self.toast("Name the playlist in the queue panel");
                }
                winamp::Command::TrackInfo(index) => {
                    let id = self
                        .player
                        .state
                        .lock()
                        .queue
                        .get(index)
                        .map(|track| track.id);
                    if let Some(id) = id {
                        self.navigate(route::Route::TrackDetail(id));
                        self.close_mini();
                    }
                }
                winamp::Command::AddTracks => {
                    // Winamp's ADD opened a file picker. There are no files, so
                    // the nearest thing is the app's own search.
                    self.navigate(route::Route::Search(String::new()));
                    self.close_mini();
                }
            }
        }
    }

    /// Open or close the equaliser or the playlist.
    fn toggle_skin_window(&mut self, window: winamp::Window) {
        let Some(shared) = self.mini_shared.clone() else {
            return;
        };
        let stack = {
            let mut shared = shared.lock().unwrap_or_else(|p| p.into_inner());
            let open = shared.stack.is_open(window);
            shared.stack.set_open(window, !open);
            shared.stack
        };
        // The stack's height changed, so the window has to follow at once.
        self.mini_fit_asked = 0.0;
        self.remember_window_stack(stack);
    }

    /// The same, from the Settings dialog: the mini player may not be open, in
    /// which case only the setting changes.
    pub fn toggle_skin_window_setting(&mut self, window: winamp::Window) {
        if self.mini_shared.is_some() {
            self.toggle_skin_window(window);
            return;
        }
        let mut stack = self.window_stack();
        let open = stack.is_open(window);
        stack.set_open(window, !open);
        self.remember_window_stack(stack);
    }

    /// The playlist was dragged to a new number of rows.
    fn resize_playlist(&mut self, rows: u32) {
        let Some(shared) = self.mini_shared.clone() else {
            return;
        };
        let stack = {
            let mut shared = shared.lock().unwrap_or_else(|p| p.into_inner());
            shared.stack.playlist_rows = rows;
            shared.stack
        };
        self.mini_fit_asked = 0.0;
        self.remember_window_stack(stack);
    }

    /// Take the equaliser's new curve: to the player now, to disk after.
    fn apply_eq(&mut self, eq: winamp::EqSettings) {
        self.settings.eq_enabled = eq.enabled;
        self.settings.eq_auto = eq.auto;
        self.settings.eq_preamp_db = eq.preamp_db;
        self.settings.eq_gains_db = eq.gains_db;
        self.player.set_eq(eq.enabled, eq.gains_db);
        self.player.set_eq_preamp_db(eq.preamp_db);
        if let Err(e) = self.settings.save() {
            self.toast(format!("Failed to save: {e}"));
        }
    }

    /// Save the current curve as this track's own preset, which AUTO reloads.
    fn save_eq_preset(&mut self) {
        let Some(track) = self.player.current_track() else {
            self.toast("Nothing playing to save a preset for");
            return;
        };
        let preset = crate::config::EqPreset {
            track_id: track.id,
            preamp_db: self.settings.eq_preamp_db,
            gains_db: self.settings.eq_gains_db,
        };
        self.settings
            .eq_presets
            .retain(|other| other.track_id != track.id);
        self.settings.eq_presets.push(preset);
        // Winamp's own list was bounded; so is this, oldest first.
        while self.settings.eq_presets.len() > 500 {
            self.settings.eq_presets.remove(0);
        }
        self.set_has_preset(true);
        if let Err(e) = self.settings.save() {
            self.toast(format!("Failed to save: {e}"));
        } else {
            self.toast(format!("Preset saved for {}", track.title));
        }
    }

    fn clear_eq_preset(&mut self) {
        let Some(track) = self.player.current_track() else {
            return;
        };
        self.settings
            .eq_presets
            .retain(|other| other.track_id != track.id);
        self.set_has_preset(false);
        if let Err(e) = self.settings.save() {
            self.toast(format!("Failed to save: {e}"));
        }
    }

    fn set_has_preset(&mut self, has: bool) {
        if let Some(shared) = &self.mini_shared {
            shared.lock().unwrap_or_else(|p| p.into_inner()).has_preset = has;
        }
    }

    /// Winamp's AUTO: when a track starts, load its own preset if it has one.
    ///
    /// Called from `poll_player_state`, which is the one place that knows the
    /// track changed.
    fn autoload_eq_preset(&mut self) {
        if !self.settings.eq_auto {
            self.set_has_preset(self.current_eq_preset().is_some());
            return;
        }
        let Some(preset) = self.current_eq_preset() else {
            self.set_has_preset(false);
            return;
        };
        self.set_has_preset(true);
        if self.settings.eq_preamp_db == preset.preamp_db
            && self.settings.eq_gains_db == preset.gains_db
        {
            return;
        }
        self.apply_eq(winamp::EqSettings {
            enabled: self.settings.eq_enabled,
            auto: true,
            preamp_db: preset.preamp_db,
            gains_db: preset.gains_db,
        });
    }

    /// Remove queue entries by index.
    fn remove_from_queue(&mut self, indices: &[usize]) {
        // Back to front, so an earlier removal cannot shift a later index.
        let mut sorted: Vec<usize> = indices.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        for index in sorted.into_iter().rev() {
            self.player.remove_at(index);
        }
    }

    /// Keep only these entries, which is Winamp's "crop".
    fn crop_queue(&mut self, keep: &[usize]) {
        if keep.is_empty() {
            return;
        }
        let len = self.player.state.lock().queue.len();
        let keep: std::collections::BTreeSet<usize> = keep.iter().copied().collect();
        let drop: Vec<usize> = (0..len).filter(|i| !keep.contains(i)).collect();
        self.remove_from_queue(&drop);
    }

    fn clear_queue(&mut self) {
        self.player.clear_queue();
    }

    /// Sort the queue, keeping the playing track playing.
    fn sort_queue(&mut self, by: winamp::SortBy) {
        use winamp::SortBy;
        let mut state = self.player.state.lock();
        // The playing track is found again after the sort, so the queue can be
        // reordered under it without interrupting playback.
        let playing = state.current.and_then(|i| state.queue.get(i).map(|t| t.id));
        match by {
            SortBy::Title => state.queue.sort_by_key(|t| t.title.to_lowercase()),
            SortBy::Artist => state.queue.sort_by(|a, b| {
                a.artist()
                    .to_lowercase()
                    .cmp(&b.artist().to_lowercase())
                    .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            }),
            SortBy::Reverse => state.queue.reverse(),
        }
        state.order = (0..state.queue.len()).collect();
        state.current = playing.and_then(|id| state.queue.iter().position(|t| t.id == id));
    }

    /// Roll a skin window up to its title bar, or back down.
    ///
    /// This is a *state of that window*, not a different window: it keeps
    /// playing, keeps its transport and keeps its seek bar. Winamp called it
    /// windowshade.
    pub fn toggle_shade(&mut self, window: winamp::Window) {
        // Settings remembers it either way, so the choice survives a restart
        // even when the mini player is closed.
        if let Some(shared) = self.mini_shared.clone() {
            let stack = {
                let mut shared = shared.lock().unwrap_or_else(|p| p.into_inner());
                shared.stack.toggle_shade(window);
                shared.stack
            };
            // The height changed, so the window has to follow at once rather
            // than waiting out the resize throttle.
            self.mini_fit_asked = 0.0;
            self.remember_window_stack(stack);
            return;
        }
        let mut stack = self.window_stack();
        stack.toggle_shade(window);
        self.remember_window_stack(stack);
    }

    /// Make the next frame re-apply the window's mode, for a setting that
    /// changed without the mode itself changing (the window level).
    pub fn forget_window_mode(&mut self) {
        self.window_is_mini = None;
    }

    /// Draw the mini player *as* the window, and make the window fit it.
    ///
    /// The mini player is not a second window. Fastpotify makes it the one
    /// window the app has, and it is right: two windows meant the big one sat
    /// behind the skin with nothing to do, kept its taskbar button, and both
    /// fought over which was "the app". Here the same viewport is resized,
    /// undecorated and made unresizable while the skin is up, and put back
    /// when it goes away.
    fn show_mini(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let Some(mini) = self.mini.clone() else {
            return;
        };
        // Whole *physical* pixels: the classic look is exact art, so the skin
        // is measured against the desktop's scaling rather than in points.
        let (scale, height) = {
            let mini = mini.lock();
            (mini.scale, mini.height())
        };
        let wanted = mini_window_size(scale, height, ctx.pixels_per_point());
        self.fit_mini_window(ctx, wanted);

        let frame = egui::Frame::new()
            .fill(egui::Color32::TRANSPARENT)
            .inner_margin(0);
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            mini.lock().ui(ui);
        });
    }

    fn mini_target_size(&self, ctx: &egui::Context) -> Option<egui::Vec2> {
        let mini = self.mini.as_ref()?.lock();
        Some(mini_window_size(
            mini.scale,
            mini.height(),
            ctx.pixels_per_point(),
        ))
    }

    fn mini_ready_to_draw(&mut self, ctx: &egui::Context) -> bool {
        if !self.mini_transitioning {
            return true;
        }
        let now = ctx.input(|input| input.time);
        let elapsed = self
            .mini_transition_started
            .map(|started| (now - started).max(0.0))
            .unwrap_or(0.0);
        let ready = self.mini_target_size(ctx).is_some_and(|wanted| {
            mini_transition_should_reveal(ctx.viewport_rect().size(), wanted, elapsed)
        });
        if ready {
            self.mini_transitioning = false;
            self.mini_transition_started = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        } else {
            ctx.request_repaint();
        }
        ready
    }

    /// Ask the window manager for the skin's size, at most once a second.
    ///
    /// A compositor may refuse or round a resize, and asking again every frame
    /// would fight it; Wayland reports no window position at all, so the
    /// viewport's own rect is the only size to compare against.
    fn fit_mini_window(&mut self, ctx: &egui::Context, wanted: egui::Vec2) {
        let current = ctx.viewport_rect().size();
        if (current - wanted).abs().max_elem() < 1.0 {
            return;
        }
        let now = ctx.input(|i| i.time);
        if now - self.mini_fit_asked < 1.0 {
            return;
        }
        self.mini_fit_asked = now;
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(wanted));
        ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(wanted));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(wanted));
    }

    /// Turn the window into the skin, or back into the app.
    ///
    /// Decorations, resizability and the window level all change with the
    /// mode; the big window's size is remembered so closing the mini player
    /// puts it back where it was rather than at the skin's 275×116.
    fn apply_window_mode(&mut self, ctx: &egui::Context) {
        // Minimizing and raising are one-shot asks, not modes, so they are
        // handled before the early return below.
        if std::mem::take(&mut self.minimize_requested) {
            if self.tray_rx.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
        }
        if std::mem::take(&mut self.raise_requested) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            // The tray's "Show Fastcloud" on a minimized window: un-minimize
            // first, then take focus. Either alone leaves it where it was.
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        let restoring_position = if let Some((pos, frames)) = self.restore_window_pos.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
            if frames > 1 {
                self.restore_window_pos = Some((pos, frames - 1));
                ctx.request_repaint();
            }
            true
        } else {
            false
        };
        let mini = self.mini.is_some();
        let was = self.window_is_mini;

        // Native resize/move commands are asynchronous.  Cache geometry only
        // while a mode is stable; reading it during a transition can capture
        // the other mode's bounds and causes the jump reported on close.
        if was == Some(mini) && !restoring_position {
            let outer = ctx.input(|input| input.viewport().outer_rect);
            if mini {
                if !self.mini_transitioning {
                    self.mini_window_pos = outer.map(|rect| rect.min);
                }
            } else {
                let size = ctx.viewport_rect().size();
                if size.x > winamp::WIDTH && size.y > winamp::HEIGHT {
                    self.big_window_size = Some(size);
                }
                self.big_window_pos = outer.map(|rect| rect.min);
            }
        }
        if was == Some(mini) {
            return;
        }
        // Remember the big window before it shrinks — but only on the way in.
        //
        // A size test alone is not enough: at 4× the skin is 1100×464, which is
        // bigger than the 275×116 this used to compare against, so re-applying
        // the mode for any other reason (the always-on-top switch) would have
        // recorded the *skin's* size as the one to restore.
        if mini && was == Some(false) {
            // A native resize is asynchronous. Hide this one transition so a
            // large undecorated black frame never flashes around the skin.
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            if self.mini_transitioning {
                self.mini_transition_started = Some(ctx.input(|input| input.time));
            }
            // First entry can happen before a stable big-mode frame was
            // observed.  Keep this as a fallback, never overwrite a good
            // cached rectangle during the hidden resize.
            if self.big_window_size.is_none() {
                let size = ctx.viewport_rect().size();
                if size.x > winamp::WIDTH && size.y > winamp::HEIGHT {
                    self.big_window_size = Some(size);
                }
            }
            if self.big_window_pos.is_none() {
                self.big_window_pos =
                    ctx.input(|input| input.viewport().outer_rect.map(|rect| rect.min));
            }
        }
        self.window_is_mini = Some(mini);
        self.mini_fit_asked = 0.0;
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(!mini));
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(!mini));
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            if mini && self.settings.winamp_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            },
        ));
        if mini {
            if let Some(wanted) = self.mini_target_size(ctx) {
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(wanted));
                ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(wanted));
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(wanted));
            }
            if let Some(pos) = self.mini_window_pos {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
                self.restore_window_pos = Some((pos, 4));
                ctx.request_repaint();
            }
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            // Let the big window be any size again before restoring it.
            let free = egui::vec2(1.0, 1.0);
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(free));
            ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(egui::vec2(
                f32::INFINITY,
                f32::INFINITY,
            )));
            if let Some(size) = self.big_window_size {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            }
            if let Some(pos) = self.big_window_pos {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
                self.restore_window_pos = Some((pos, 4));
                ctx.request_repaint();
            }
        }
    }

    /// Horizontal card carousel with ‹ › arrows (soundcloud.com style).
    /// The card closure gets `App` back; the scroll target is applied for
    /// one frame so wheel/drag scrolling keeps working. Arrows are centred
    /// on the artwork (`art_size`), not on the row including its captions.
    pub fn carousel<R>(
        &mut self,
        ui: &mut egui::Ui,
        salt: &str,
        add_cards: impl FnOnce(&mut App, &mut egui::Ui) -> R,
    ) -> R {
        self.carousel_sized(ui, salt, Some(widgets::CARD_ART), add_cards)
    }

    /// [`App::carousel`] with an explicit artwork height for arrow centring.
    pub fn carousel_sized<R>(
        &mut self,
        ui: &mut egui::Ui,
        salt: &str,
        art_size: Option<f32>,
        add_cards: impl FnOnce(&mut App, &mut egui::Ui) -> R,
    ) -> R {
        let pending = self.carousel.remove(salt);
        let out = widgets::carousel_plain(ui, salt, pending, |ui| add_cards(self, ui));
        let mut target = None;
        widgets::carousel_arrows(
            ui,
            salt,
            out.view,
            out.offset_x,
            out.max_x,
            art_size,
            &mut |x| {
                target = Some(x);
            },
        );
        if let Some(x) = target {
            self.carousel.insert(salt.to_owned(), x);
        }
        out.inner
    }

    /// Create a real playlist on the signed-in SoundCloud account.
    pub fn create_playlist(&mut self, title: String) {
        self.create_playlist_with_tracks(title, Vec::new());
    }

    pub fn create_playlist_with_tracks(&mut self, title: String, track_ids: Vec<u64>) {
        if self.demo {
            let id = 9000 + self.settings.custom_playlists.len() as u64;
            self.settings
                .custom_playlists
                .push(crate::config::CustomPlaylist {
                    id,
                    title,
                    track_ids,
                });
            self.persist_library();
            self.navigate(crate::ui::route::Route::PlaylistDetail(id));
            return;
        }
        let client = self.player.api().clone();
        let tx = self.social_tx.clone();
        self.rt.spawn(async move {
            let result = crate::api::endpoints::create_playlist(&client, &title, true, &track_ids)
                .await
                .map_err(|error| error.to_string());
            let _ = tx.send(SocialDone::PlaylistCreated(result));
        });
    }

    pub fn set_track_reposted(&self, track_id: u64, reposted: bool) {
        let client = self.player.api().clone();
        let tx = self.social_tx.clone();
        self.rt.spawn(async move {
            let result = if reposted {
                crate::api::endpoints::repost_track(&client, track_id).await
            } else {
                crate::api::endpoints::unrepost_track(&client, track_id).await
            };
            let _ = tx.send(SocialDone::Mutation {
                success: if reposted {
                    "Track reposted"
                } else {
                    "Repost removed"
                }
                .to_owned(),
                refresh: vec![crate::store::Key::MyRepostedTracks],
                error: result
                    .err()
                    .map(|error| format!("Repost not saved: {error}")),
            });
        });
    }

    pub fn set_playlist_liked(&self, playlist_id: u64, liked: bool) {
        let client = self.player.api().clone();
        let tx = self.social_tx.clone();
        self.rt.spawn(async move {
            let result = if liked {
                crate::api::endpoints::like_playlist(&client, playlist_id).await
            } else {
                crate::api::endpoints::unlike_playlist(&client, playlist_id).await
            };
            let _ = tx.send(SocialDone::Mutation {
                success: if liked {
                    "Playlist liked"
                } else {
                    "Playlist unliked"
                }
                .to_owned(),
                refresh: vec![crate::store::Key::LikedPlaylists],
                error: result.err().map(|error| format!("Like not saved: {error}")),
            });
        });
    }

    pub fn set_playlist_reposted(&self, playlist_id: u64, reposted: bool) {
        let client = self.player.api().clone();
        let tx = self.social_tx.clone();
        self.rt.spawn(async move {
            let result = if reposted {
                crate::api::endpoints::repost_playlist(&client, playlist_id).await
            } else {
                crate::api::endpoints::unrepost_playlist(&client, playlist_id).await
            };
            let _ = tx.send(SocialDone::Mutation {
                success: if reposted {
                    "Playlist reposted"
                } else {
                    "Repost removed"
                }
                .to_owned(),
                refresh: vec![crate::store::Key::MyRepostedPlaylists],
                error: result
                    .err()
                    .map(|error| format!("Repost not saved: {error}")),
            });
        });
    }

    pub fn delete_playlist(&self, playlist_id: u64) {
        let client = self.player.api().clone();
        let tx = self.social_tx.clone();
        self.rt.spawn(async move {
            let result = crate::api::endpoints::delete_playlist(&client, playlist_id)
                .await
                .map_err(|error| error.to_string());
            let _ = tx.send(SocialDone::PlaylistDeleted {
                id: playlist_id,
                result,
            });
        });
    }

    pub fn upload_track(&mut self) {
        if self.creator.uploading {
            return;
        }
        let path = std::path::PathBuf::from(self.creator.upload_path.trim());
        if !path.is_file() {
            self.toast("Choose an audio file that exists");
            return;
        }
        let title = self.creator.upload_title.trim().to_owned();
        if title.is_empty() {
            self.toast("Track title is required");
            return;
        }
        self.creator.uploading = true;
        let artist = self.creator.upload_artist.trim().to_owned();
        let description = self.creator.upload_description.trim().to_owned();
        let genre = self.creator.upload_genre.trim().to_owned();
        let tags = self.creator.upload_tags.trim().to_owned();
        let public = self.creator.upload_public;
        let client = self.player.api().clone();
        let tx = self.social_tx.clone();
        self.rt.spawn(async move {
            let request = crate::api::endpoints::TrackUpload {
                title: &title,
                artist: (!artist.is_empty()).then_some(artist.as_str()),
                description: (!description.is_empty()).then_some(description.as_str()),
                genre: (!genre.is_empty()).then_some(genre.as_str()),
                tags: (!tags.is_empty()).then_some(tags.as_str()),
                public,
            };
            let result = crate::api::endpoints::upload_track(&client, &path, request)
                .await
                .map_err(|error| error.to_string());
            let _ = tx.send(SocialDone::TrackUploaded(result));
        });
    }

    pub fn begin_track_edit(&mut self, track: &crate::api::models::Track) {
        self.creator.edit_track_id = Some(track.id);
        self.creator.edit_title = track.title.clone();
        self.creator.edit_artist = track.artist().to_owned();
        self.creator.edit_description = track.description.clone().unwrap_or_default();
    }

    pub fn save_track_edit(&mut self, track_id: u64) {
        let title = self.creator.edit_title.trim().to_owned();
        if title.is_empty() {
            self.toast("Track title is required");
            return;
        }
        let artist = self.creator.edit_artist.trim().to_owned();
        let description = self.creator.edit_description.trim().to_owned();
        let client = self.player.api().clone();
        let tx = self.social_tx.clone();
        self.rt.spawn(async move {
            let result = crate::api::endpoints::update_track_metadata(
                &client,
                track_id,
                &title,
                Some(&description),
                Some(&artist),
            )
            .await
            .map_err(|error| error.to_string());
            let _ = tx.send(SocialDone::TrackUpdated(result));
        });
    }

    pub fn delete_track(&self, track_id: u64) {
        let client = self.player.api().clone();
        let tx = self.social_tx.clone();
        self.rt.spawn(async move {
            let result = crate::api::endpoints::delete_track(&client, track_id)
                .await
                .map_err(|error| error.to_string());
            let _ = tx.send(SocialDone::TrackDeleted {
                id: track_id,
                result,
            });
        });
    }

    pub fn save_storefront(&mut self, track_id: u64) {
        let title = self.creator.storefront_title.trim().to_owned();
        let kind = self.creator.storefront_kind.trim().to_owned();
        let link = self.creator.storefront_link.trim().to_owned();
        if title.is_empty() || kind.is_empty() || link.is_empty() {
            self.toast("Storefront title, type and link are required");
            return;
        }
        let link_title = self.creator.storefront_link_title.trim().to_owned();
        let description = self.creator.storefront_description.trim().to_owned();
        let price = self.creator.storefront_price.trim().to_owned();
        let client = self.player.api().clone();
        let tx = self.social_tx.clone();
        self.rt.spawn(async move {
            let request = crate::api::endpoints::StorefrontUpdate {
                title: &title,
                kind: &kind,
                link: &link,
                link_title: (!link_title.is_empty()).then_some(link_title.as_str()),
                description: (!description.is_empty()).then_some(description.as_str()),
                price: (!price.is_empty()).then_some(price.as_str()),
            };
            let result = crate::api::endpoints::update_track_storefront(&client, track_id, request)
                .await
                .map_err(|error| error.to_string());
            let _ = tx.send(SocialDone::StorefrontUpdated(result));
        });
    }

    /// Add a track to a playlist.
    ///
    /// The list on screen changes now; the write goes out behind it and rolls
    /// back if SoundCloud refuses (see `crate::playlists`). Demo mode and the
    /// local Liked list are pure bookkeeping and never hit the network.
    pub fn add_to_playlist(&mut self, playlist_id: u64, track_id: u64) -> String {
        self.add_tracks_to_playlist(playlist_id, &[track_id])
    }

    /// Start playback from an explicit UI action and remember the album/track
    /// context separately from the decoder queue. Autoplay calls the player
    /// directly, so related tracks cannot unexpectedly replace Recently Played.
    pub fn play_user_queue(
        &mut self,
        tracks: Vec<crate::api::models::Track>,
        index: usize,
        shuffle: bool,
    ) {
        self.player.play_queue(tracks, index, shuffle);
    }

    /// Add several tracks as one ordered playlist update. SoundCloud replaces
    /// the full track list on PUT, so batching prevents concurrent writes from
    /// dropping all but the last selected track.
    pub fn add_tracks_to_playlist(&mut self, playlist_id: u64, track_ids: &[u64]) -> String {
        if self.demo {
            if playlist_id == 0 {
                self.liked.extend(track_ids.iter().copied());
                self.persist_library();
                return "Liked Songs".to_owned();
            }
            if let Some(pl) = self
                .settings
                .custom_playlists
                .iter_mut()
                .find(|p| p.id == playlist_id)
            {
                for track_id in track_ids {
                    if !pl.track_ids.contains(track_id) {
                        pl.track_ids.push(*track_id);
                    }
                }
                let title = pl.title.clone();
                self.persist_library();
                return title;
            }
        }
        // A SoundCloud playlist: PUT replaces the whole list. If this
        // playlist is already open, edit its confirmed rows optimistically.
        // From a track's "add to playlist" menu those rows usually have not
        // loaded yet, so read the complete server list in the same task before
        // writing; never turn "add one" into "replace everything with one".
        let remote = self.tracks(crate::store::Key::PlaylistTracks(playlist_id));
        if let crate::store::Slot::Ready(tracks) = remote {
            let current: Vec<u64> = tracks.iter().map(|track| track.id).collect();
            for track_id in track_ids {
                self.playlist_edits.add(playlist_id, &current, *track_id);
            }
        } else {
            let client = self.player.api().clone();
            let tx = self.social_tx.clone();
            let track_ids = track_ids.to_vec();
            self.rt.spawn(async move {
                let result = async {
                    let mut ids: Vec<u64> =
                        crate::api::endpoints::playlist_tracks(&client, playlist_id)
                            .await
                            .collect_all()
                            .await?
                            .into_iter()
                            .map(|track| track.id)
                            .collect();
                    let original_len = ids.len();
                    for track_id in track_ids {
                        if !ids.contains(&track_id) {
                            ids.push(track_id);
                        }
                    }
                    if ids.len() != original_len {
                        crate::api::endpoints::update_playlist(
                            &client,
                            playlist_id,
                            None,
                            None,
                            &ids,
                        )
                        .await?;
                    }
                    Ok::<(), crate::api::error::ApiError>(())
                }
                .await;
                let _ = tx.send(SocialDone::Mutation {
                    success: "Track added to playlist".to_owned(),
                    refresh: vec![
                        crate::store::Key::PlaylistTracks(playlist_id),
                        crate::store::Key::Playlist(playlist_id),
                        crate::store::Key::MyPlaylists,
                    ],
                    error: result
                        .err()
                        .map(|error| format!("Playlist not saved: {error}")),
                });
            });
            return "playlist".to_owned();
        }
        "playlist".to_owned()
    }

    /// Remove a track from a playlist, optimistically.
    pub fn remove_from_playlist(&mut self, playlist_id: u64, track_id: u64) {
        if self.demo {
            if playlist_id == 0 {
                self.liked.remove(&track_id);
                self.persist_library();
                return;
            }
            if let Some(pl) = self
                .settings
                .custom_playlists
                .iter_mut()
                .find(|p| p.id == playlist_id)
            {
                pl.track_ids.retain(|id| *id != track_id);
                self.persist_library();
                return;
            }
        }
        let current = self.playlist_track_ids(playlist_id);
        if !self.demo {
            self.playlist_edits.remove(playlist_id, &current, track_id);
        }
        if let Some(extra) = self.extra_tracks.get_mut(&playlist_id) {
            extra.retain(|id| *id != track_id);
        }
    }

    /// Move a track inside a playlist, optimistically. `current` is the list
    /// as shown, so a local playlist and a SoundCloud one reorder the same.
    pub fn move_in_playlist(&mut self, playlist_id: u64, current: &[u64], from: usize, to: usize) {
        if self.demo
            && let Some(pl) = self
                .settings
                .custom_playlists
                .iter_mut()
                .find(|p| p.id == playlist_id)
        {
            if from < pl.track_ids.len() && to < pl.track_ids.len() {
                let id = pl.track_ids.remove(from);
                pl.track_ids.insert(to, id);
            }
            self.persist_library();
            return;
        }
        if !self.demo {
            self.playlist_edits.reorder(playlist_id, current, from, to);
        }
        if let Some(extra) = self.extra_tracks.get_mut(&playlist_id)
            && from < extra.len()
            && to < extra.len()
        {
            let id = extra.remove(from);
            extra.insert(to, id);
        }
    }

    /// The playlist's track ids as currently shown: the optimistic list when
    /// an edit is outstanding, else what was last read.
    pub fn playlist_track_ids(&self, playlist_id: u64) -> Vec<u64> {
        if let Some(shown) = self.playlist_edits.view(playlist_id) {
            return shown.to_vec();
        }
        if self.demo && playlist_id == 0 {
            let mut ids: Vec<u64> = self.liked.iter().copied().collect();
            ids.sort_unstable();
            return ids;
        }
        if self.demo
            && let Some(pl) = self
                .settings
                .custom_playlists
                .iter()
                .find(|p| p.id == playlist_id)
        {
            return pl.track_ids.clone();
        }
        let mut ids: Vec<u64> = self
            .tracks(crate::store::Key::PlaylistTracks(playlist_id))
            .rows()
            .iter()
            .map(|track| track.id)
            .collect();
        if let Some(extra) = self.extra_tracks.get(&playlist_id) {
            for id in extra {
                if !ids.contains(id) {
                    ids.push(*id);
                }
            }
        }
        ids
    }

    fn persist_library(&mut self) {
        if self.demo {
            return;
        }
        let mut liked: Vec<u64> = self.liked.iter().copied().collect();
        liked.sort_unstable();
        let mut followed: Vec<u64> = self.followed.iter().copied().collect();
        followed.sort_unstable();
        self.settings.liked_ids = liked;
        self.settings.followed_user_ids = followed;
        if let Err(e) = self.settings.save() {
            self.toasts
                .push((format!("Failed to save library: {e}"), 0.0));
        }
    }

    /// Bind OS media controls to the live window (Windows requires the HWND).
    #[cfg(target_os = "windows")]
    pub fn attach_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        if self.media.is_none() {
            let mut media = crate::desktop::media::MediaIntegration::new(hwnd);
            let position = self.player.state.lock().position_ms;
            media.update_playback(false, position);
            media.discard_pending_events();
            self.media = Some(media);
        }
    }

    // ===== Account =====

    /// Whether the account's own lists are reachable.
    pub fn signed_in(&self) -> bool {
        self.demo || self.store.signed_in()
    }

    /// Start the browser flow. Returns immediately; the result arrives on
    /// [`Self::poll_auth`].
    pub fn sign_in(&mut self) {
        let Some(session) = self.session.clone() else {
            self.start_connect();
            return;
        };
        if self.signing_in {
            return;
        }
        self.signing_in = true;
        self.auth_error = None;
        self.auth_url = None;
        self.toast("Complete SoundCloud sign-in in your browser");
        let tx = self.auth_tx.clone();
        self.auth_task = Some(self.rt.spawn(async move {
            let progress = tx.clone();
            let done = match session
                .sign_in_notifying(move |url| {
                    let _ = progress.send(AuthDone::Authorizing(url));
                })
                .await
            {
                Ok(()) => AuthDone::SignedIn,
                Err(e) => AuthDone::Failed(e.to_string()),
            };
            let _ = tx.send(done);
        }));
    }

    pub fn sign_in_controls(&mut self, ui: &mut egui::Ui) {
        if ui
            .add_enabled(
                !self.signing_in,
                egui::Button::new(if self.signing_in {
                    "Waiting for SoundCloud…"
                } else {
                    "Sign in with SoundCloud"
                }),
            )
            .clicked()
        {
            self.sign_in();
        }
        if let Some(url) = &self.auth_url {
            ui.hyperlink_to("Open SoundCloud sign-in in browser", url);
        }
        if let Some(error) = &self.auth_error {
            ui.label(error);
        }
        if self.signing_in && ui.button("Cancel sign-in").clicked() {
            if let Some(task) = self.auth_task.take() {
                task.abort();
            }
            self.signing_in = false;
            self.auth_url = None;
        }
    }

    pub fn sign_out(&mut self) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let tx = self.auth_tx.clone();
        self.rt.spawn(async move {
            let done = match session.sign_out().await {
                Ok(()) => AuthDone::SignedOut,
                Err(e) => AuthDone::Failed(e.to_string()),
            };
            let _ = tx.send(done);
        });
    }

    pub fn disconnect_account(&mut self) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let tx = self.auth_tx.clone();
        self.rt.spawn(async move {
            let done = match session.disconnect().await {
                Ok(()) => AuthDone::Disconnected,
                Err(error) => AuthDone::Failed(error.to_string()),
            };
            let _ = tx.send(done);
        });
    }

    // ===== Connecting an account =====

    /// Begin the device-code flow that registers the user's own SoundCloud
    /// application (see [`crate::auth::register`]).
    ///
    /// Returns at once: the steps arrive on [`Self::poll_connect`].
    pub fn start_connect(&mut self) {
        if self.connection.busy() {
            return;
        }
        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
        self.connect_cancel = Some(cancel_tx);
        self.connection = connect::Connection::Starting;
        let tx = self.connect_tx.clone();
        // Its own client: the API client carries an OAuth header we must not
        // send to the registration host.
        let http = reqwest::Client::builder()
            .user_agent(concat!("fastcloud/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        self.rt.spawn(connect::connect(http, cancel_rx, tx));
    }

    /// Connect, reusing an application this machine already registered.
    ///
    /// A stored registration only needs a token, not another browser sign-in —
    /// which is the difference between "the network was down at launch" and
    /// "this user has never connected". [`Self::start_connect`] is the latter.
    pub fn reconnect(&mut self) {
        if self.connection.busy() {
            return;
        }
        match crate::auth::AppCredentials::discover() {
            Some(creds) => {
                self.adopt_credentials(creds, "Reconnecting to SoundCloud", false);
            }
            None => self.start_connect(),
        }
    }

    /// Stop a running connect flow.
    pub fn cancel_connect(&mut self) {
        if let Some(cancel) = self.connect_cancel.take() {
            let _ = cancel.send(true);
        }
        self.connection = connect::Connection::Disconnected;
    }

    /// Take credentials into use without a restart.
    ///
    /// Everything that reads them goes through the one [`crate::api::ApiClient`]
    /// the player holds, so the client id is swapped in place and a fresh
    /// [`crate::auth::Session`] is built around it. Rebuilding the client itself
    /// would leave the player pointing at the old one.
    fn adopt_credentials(
        &mut self,
        creds: crate::auth::AppCredentials,
        what: &str,
        sign_in_after_registration: bool,
    ) {
        let was_demo = self.demo;
        let client = self.player.api().clone();
        client.set_client_id(creds.client_id.clone());
        // Whatever demo state we were in, there is a live API now.
        client.set_demo(false);
        self.demo = false;
        self.player.set_demo(false);
        self.stored_app = true;
        // The demo library seeded a queue of synthesized tracks. They have no
        // stream on SoundCloud, so leaving them queued would hand the real
        // player something it cannot load.
        if was_demo {
            self.clear_queue();
        }
        self.liked.clear();
        self.followed.clear();
        self.recent_ids.clear();
        self.store.set_signed_in(false);
        self.library_synced = (false, false);
        self.store.clear();

        let session = crate::auth::Session::new(creds, client);
        self.session = Some(session.clone());
        self.connection = connect::Connection::Registering;
        self.toast(what.to_owned());

        // A pairing token can only create the developer application. It
        // cannot read `/me`, likes, or playlists, so a fresh registration must
        // immediately continue into that new application's OAuth flow. This
        // keeps first launch as one guided action instead of hiding a second
        // sign-in button in Settings.
        if sign_in_after_registration {
            self.sign_in();
            return;
        }

        // Resume a signed-in session if the keyring has one, else mint an
        // app-only token so search and playback work before the user signs in.
        // Both are requests, so neither can be awaited here.
        let tx = self.auth_tx.clone();
        self.rt.spawn(async move {
            if session.resume().await == Some(crate::auth::Grant::User) {
                let _ = tx.send(AuthDone::SignedIn);
                return;
            }
            match session.access_token().await {
                Ok(_) => {
                    let _ = tx.send(AuthDone::AppTokenReady);
                }
                Err(e) => {
                    let _ = tx.send(AuthDone::Failed(e.to_string()));
                }
            }
        });
    }

    /// Apply the steps of a running connect flow.
    fn poll_connect(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        while let Ok(step) = self.connect_rx.try_recv() {
            changed = true;
            match step {
                connect::ConnectStep::Paired { code, url } => {
                    self.connection = connect::Connection::Pairing { code, url };
                }
                connect::ConnectStep::Registering => {
                    self.connection = connect::Connection::Registering;
                }
                connect::ConnectStep::Registered {
                    credentials,
                    existing,
                } => {
                    self.connect_cancel = None;
                    let credentials = *credentials;
                    // To the keyring first: a connection that works this run
                    // but not the next is worse than a visible failure.
                    if let Err(e) = credentials.save_to_keyring() {
                        self.connection = connect::Connection::Failed(format!(
                            "Connected, but the keys could not be stored: {e}"
                        ));
                        continue;
                    }
                    let what = if existing {
                        "Reconnected with your existing SoundCloud app"
                    } else {
                        "Connected to SoundCloud"
                    };
                    self.adopt_credentials(credentials, what, true);
                }
                connect::ConnectStep::NeedsArtistPro(message) => {
                    self.connect_cancel = None;
                    self.connection = connect::Connection::NeedsArtistPro(message);
                }
                connect::ConnectStep::Failed(message) => {
                    self.connect_cancel = None;
                    self.connection = connect::Connection::Failed(message);
                }
                connect::ConnectStep::Cancelled => {
                    self.connect_cancel = None;
                    self.connection = connect::Connection::Disconnected;
                }
            }
        }
        if changed {
            ctx.request_repaint();
        }
    }

    /// Apply a finished sign-in / sign-out, and keep the store's view of the
    /// session in step.
    ///
    /// The boot-time grant is set once in `main`; nothing here awaits, so the
    /// interface thread never blocks on the runtime.
    fn poll_auth(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        while let Ok(done) = self.auth_rx.try_recv() {
            if let AuthDone::Authorizing(url) = done {
                self.auth_url = Some(url);
                ctx.request_repaint();
                continue;
            }
            self.signing_in = false;
            changed = true;
            match done {
                AuthDone::Authorizing(_) => unreachable!(),
                AuthDone::SignedIn => {
                    self.auth_url = None;
                    self.auth_error = None;
                    self.library_synced = (false, false);
                    self.store.set_signed_in(true);
                    self.store.clear();
                    self.connection = connect::Connection::Connected { username: None };
                    self.toast("Signed in to SoundCloud");
                }
                AuthDone::SignedOut => {
                    self.store.set_signed_in(false);
                    self.liked.clear();
                    self.followed.clear();
                    self.toast("Signed out");
                }
                AuthDone::Disconnected => {
                    self.store.set_signed_in(false);
                    self.store.clear();
                    self.liked.clear();
                    self.followed.clear();
                    self.connection = connect::Connection::Connected { username: None };
                    self.toast("Fastcloud disconnected from this SoundCloud account");
                }
                // App-only access is live: public browsing works, and the store
                // may have cached "sign in for this" answers while it did not.
                AuthDone::AppTokenReady => {
                    self.store.set_signed_in(false);
                    self.store.clear();
                    self.connection = connect::Connection::Connected { username: None };
                }
                AuthDone::Failed(why) => {
                    self.auth_error = Some(why.clone());
                    if !self.connection.connected() {
                        self.connection = connect::Connection::Failed(why.clone());
                    }
                    self.toast(format!("Sign-in failed: {why}"));
                }
            }
        }
        if changed {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.quit_requested && self.tray_rx.is_some() {
            if ctx.input(|input| input.viewport().close_requested()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            } else if ctx.input(|input| input.viewport().minimized == Some(true)) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }
        self.handle_ipc_and_tray();
        self.handle_hotkeys();
        self.poll_player_state();
        self.poll_pending_link();
        self.poll_link_results();
        self.poll_connect(ctx);
        self.poll_auth(ctx);
        self.poll_social();
        self.poll_mini();
        self.apply_window_mode(ctx);
        // Lists that landed since the last frame; repaint so they appear now
        // rather than at the next 5 Hz tick.
        if self.store.poll() > 0 {
            ctx.request_repaint();
        }
        if !self.demo && self.store.signed_in() {
            if !self.library_synced.0
                && let crate::store::Slot::Ready(tracks) =
                    self.store.tracks(crate::store::Key::Likes)
            {
                self.liked.extend(tracks.iter().map(|t| t.id));
                self.library_synced.0 = true;
            }
            if !self.library_synced.1
                && let crate::store::Slot::Ready(users) =
                    self.store.users(crate::store::Key::Following)
            {
                self.followed = users.iter().map(|u| u.id).collect();
                self.library_synced.1 = true;
            }
        }
        // Optimistic playlist edits: send what is outstanding, apply what
        // came back. Failures put the list back and say why.
        let client = self.player.api().clone();
        self.playlist_edits.flush(&client, &self.rt);
        let (updated_playlists, playlist_errors) = self.playlist_edits.poll();
        for playlist_id in updated_playlists {
            self.store
                .invalidate(&crate::store::Key::PlaylistTracks(playlist_id));
            self.store
                .invalidate(&crate::store::Key::Playlist(playlist_id));
            self.extra_tracks.remove(&playlist_id);
        }
        for error in playlist_errors {
            self.toast(format!("Playlist not saved: {error}"));
        }
        // Bounded artwork memory: forget textures over budget periodically.
        // (No time-based eviction of visible art — that flashed the window
        // upstream in fastpotify#129.)
        let now = ctx.input(|i| i.time);
        if now - self.last_art_evict > crate::images::EVICT_EVERY_SECS {
            self.last_art_evict = now;
            self.art.evict(ctx);
        }
        // Repaint at ~5 Hz so IPC commands, hotkeys and the seek bar stay
        // responsive even when paused or hidden.
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_egui_hotkeys(&ctx);

        self.theme.apply(&ctx);

        // The mini player *is* the window while it is open: the app's own
        // interface is not drawn behind it, as Winamp had one window and
        // fastpotify keeps that.
        if self.mini.is_some() && self.mini_ready_to_draw(&ctx) {
            self.show_mini(&ctx, ui);
            self.show_toasts(&ctx);
            if self.quit_requested {
                self.player.shutdown();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }

        // Account pages only exist behind user OAuth. Public-profile fallbacks
        // cannot supply private playlists, feed or history and must never be
        // presented as though they were the signed-in user's library.
        if !self.signed_in() {
            login::show(self, ui);
            self.show_toasts(&ctx);
            if self.quit_requested {
                self.player.shutdown();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }

        player_bar::show(self, ui);
        // The panel itself carries the fill and the padding: a Frame *inside*
        // the panel paints only its own rect, which left the page colour
        // showing along the top bar's edges.
        egui::Panel::top(egui::Id::new("topbar"))
            .exact_size(topbar::BAR_HEIGHT)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(self.theme.bg).inner_margin(0))
            .show(ui, |ui| {
                let available = ui.available_width();
                let width = available.min(1220.0);
                let left = ui.max_rect().left() + ((available - width) / 2.0).max(0.0);
                let rect = egui::Rect::from_min_size(
                    egui::pos2(left, ui.max_rect().top()),
                    egui::vec2(width, ui.available_height()),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    topbar::show(self, ui);
                });
            });
        if self.show_queue {
            egui::Panel::right(egui::Id::new("queue"))
                .default_size(300.0)
                .min_size(240.0)
                .show(ui, |ui| {
                    queue::show(self, ui);
                });
        }
        egui::CentralPanel::default().show(ui, |ui| {
            // Centered content column like soundcloud.com (~1220px).
            let avail_w = ui.available_width();
            let content_w = avail_w.min(1220.0);
            let pad = ((avail_w - content_w) / 2.0).max(0.0);
            let x0 = ui.max_rect().left() + pad;
            let rect = egui::Rect::from_min_size(
                egui::pos2(x0, ui.max_rect().top()),
                egui::vec2(content_w, ui.available_height()),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                views::show(self, ui);
            });
        });

        self.show_toasts(&ctx);

        if self.show_shortcuts {
            egui::Window::new("Keyboard shortcuts")
                .id(egui::Id::new("shortcuts"))
                .collapsible(false)
                .show(&ctx, |ui| {
                    egui::Grid::new("shortcuts-grid")
                        .num_columns(2)
                        .spacing([16.0, 4.0])
                        .show(ui, |ui| {
                            for (keys, what) in [
                                ("Space", "Play / pause"),
                                ("Ctrl + Left / Right", "Next / previous track"),
                                ("Left / Right", "Seek 5 seconds"),
                                ("M", "Mute / unmute"),
                                ("Ctrl + F", "Search"),
                                ("Ctrl + M", "Winamp mini player"),
                                ("Ctrl + Shift + M", "Roll the mini player up"),
                                ("Esc", "Clear the selection"),
                                ("Mouse back / forward", "Page history"),
                                ("Drop a .wsz", "Wear that Winamp skin"),
                                ("Drop a .ttf / .otf", "Use that interface font"),
                                ("F1", "This dialog"),
                            ] {
                                ui.label(
                                    egui::RichText::new(keys).monospace().color(self.theme.text),
                                );
                                ui.label(egui::RichText::new(what).color(self.theme.text_dim));
                                ui.end_row();
                            }
                        });
                    ui.add_space(6.0);
                    if ui.button("Close").clicked() {
                        self.show_shortcuts = false;
                    }
                });
        }

        if self.quit_requested {
            self.player.shutdown();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// The window is see-through where the skin leaves it out; the app's own
    /// interface paints over eframe's ground.
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        if self.mini.is_some() {
            [0.0; 4]
        } else {
            visuals.panel_fill.to_normalized_gamma_f32()
        }
    }
}

impl App {
    fn handle_ipc_and_tray(&mut self) {
        let mut inbox: Vec<crate::cli::IpcMessage> = Vec::new();
        if let Some(rx) = &self.ipc_rx {
            while let Ok(msg) = rx.try_recv() {
                inbox.push(msg);
            }
        }
        if let Some(rx) = &self.tray_rx {
            while let Ok(cmd) = rx.try_recv() {
                inbox.push(cmd.to_ipc());
            }
        }
        if let Some(media) = &mut self.media {
            while let Ok(msg) = media.rx.try_recv() {
                inbox.push(msg);
            }
        }
        for msg in inbox {
            self.apply_ipc(msg);
        }
    }

    fn apply_ipc(&mut self, msg: crate::cli::IpcMessage) {
        use crate::cli::IpcMessage;
        match msg {
            IpcMessage::Play => self.player.play(),
            IpcMessage::Pause => self.player.pause(),
            IpcMessage::Toggle => self.player.play_pause(),
            IpcMessage::Next => {
                self.player.next();
            }
            IpcMessage::Prev => {
                self.player.prev();
            }
            IpcMessage::Stop => self.player.stop(),
            IpcMessage::SeekBy(ms) => {
                let pos = self.player.state.lock().position_ms as i64 + ms;
                self.player.seek_ms(pos.max(0) as u64);
            }
            IpcMessage::SeekTo(ms) => self.player.seek_ms(ms),
            IpcMessage::Volume(v) => {
                let v = (v.min(100) as f32) / 100.0;
                self.settings.volume = v;
                self.player.set_volume(v);
            }
            IpcMessage::VolumeBy(d) => {
                let cur = self.player.state.lock().volume;
                let next = (cur + d as f32 / 100.0).clamp(0.0, 1.0);
                self.settings.volume = next;
                self.player.set_volume(next);
            }
            IpcMessage::Mute => self.mute(),
            IpcMessage::Shuffle(state) => match state {
                Some(on) => self.player.set_shuffle(on),
                None => self.player.toggle_shuffle(),
            },
            IpcMessage::Repeat(mode) => match mode {
                Some(m) => self.player.set_repeat(m),
                None => self.player.cycle_repeat(),
            },
            IpcMessage::Like => {
                if let Some(t) = self.player.current_track() {
                    let now = self.toggle_like(t.id);
                    self.toast(if now {
                        format!("Liked {}", t.title)
                    } else {
                        format!("Removed {} from likes", t.title)
                    });
                }
            }
            IpcMessage::PlayUrl { .. } | IpcMessage::OpenLink(..) => {
                let url = match msg {
                    IpcMessage::PlayUrl(url) | IpcMessage::OpenLink(url) => url,
                    _ => unreachable!(),
                };
                // Same pipeline as a cold-boot link: resolve, then play/open.
                self.pending_link = Some(url);
            }
            // Answered by the singleton server, never forwarded here.
            IpcMessage::NowPlaying { .. } | IpcMessage::Devices { .. } => {}
            IpcMessage::SignIn => self.sign_in(),
            // The tray's "Show Fastcloud", and a second launch asking the
            // running one to come forward. Applied in `apply_window_mode`,
            // which is where this frame's viewport commands are sent.
            IpcMessage::Show => self.raise_requested = true,
            IpcMessage::Quit => self.quit_requested = true,
        }
    }

    fn mute(&mut self) {
        let vol = self.player.state.lock().volume;
        let next = if vol < 0.01 {
            if self.last_volume < 0.01 {
                0.8
            } else {
                self.last_volume
            }
        } else {
            self.last_volume = vol;
            0.0
        };
        self.volume_preview = None;
        self.settings.volume = next;
        self.player.set_volume(next);
    }

    /// Kick off background link resolution (CLI, tray, cold boot).
    fn poll_pending_link(&mut self) {
        let Some(raw) = self.pending_link.take() else {
            return;
        };
        let client = self.player.api().clone();
        let tx = self.link_tx.clone();
        self.rt.spawn(async move {
            let done = resolve_link(&client, &raw).await;
            let _ = tx.send(done);
        });
        while let Ok(done) = self.link_rx.try_recv() {
            self.apply_link(done);
        }
    }

    fn poll_link_results(&mut self) {
        while let Ok(done) = self.link_rx.try_recv() {
            self.apply_link(done);
        }
    }

    fn apply_link(&mut self, done: LinkDone) {
        match done {
            LinkDone::PlayTrack(t, link) => {
                let id = t.id;
                self.log_inbox(format!("{} — {}", t.title, t.artist()), link);
                self.play_user_queue(vec![*t], 0, false);
                self.navigate(crate::ui::route::Route::TrackDetail(id));
            }
            LinkDone::OpenPlaylist(id, link) => {
                self.log_inbox(format!("Playlist {id}"), link);
                self.navigate(crate::ui::route::Route::PlaylistDetail(id));
            }
            LinkDone::OpenUser(id, link) => {
                self.log_inbox(format!("Profile {id}"), link);
                self.navigate(crate::ui::route::Route::UserDetail(id));
            }
            LinkDone::Failed(e) => self.toast(format!("Cannot open link: {e}")),
        }
    }

    /// Log an opened link into the local messages inbox (newest last,
    /// capped; SoundCloud DMs aren't in the public API, so links shared
    /// into the app live here instead).
    fn log_inbox(&mut self, label: String, link: String) {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Dedupe on repeat opens: newest wins.
        self.settings
            .inbox
            .retain(|item| item.label != label && item.link != link);
        self.settings
            .inbox
            .push(crate::config::InboxItem { label, link, at });
        while self.settings.inbox.len() > 50 {
            self.settings.inbox.remove(0);
        }
        self.persist_library();
    }

    fn handle_hotkeys(&mut self) {
        let Some(hk) = &self.hotkeys else { return };
        let mut actions = Vec::new();
        hk.poll(&mut actions);
        for action in actions {
            self.apply_ipc(action.to_ipc());
        }
    }

    fn handle_egui_hotkeys(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.show_shortcuts = !self.show_shortcuts;
            return;
        }
        // Dropped files: a `.wsz` wears that skin (from either window, as
        // Winamp itself accepted), a font file becomes the interface face.
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        for path in dropped {
            let Some(extension) = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
            else {
                continue;
            };
            match extension.as_str() {
                "wsz" | "zip" | "wal" => self.apply_skin_file(path),
                "ttf" | "otf" | "ttc" | "otc" => self.apply_interface_font(path),
                _ => {}
            }
        }
        // Ctrl+M opens the Winamp mini player (Cmd+M on macOS), and with Shift
        // rolls it up to its title bar — the same two things its title bar's
        // buttons do.
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::M)) {
            if ctx.input(|i| i.modifiers.shift) {
                // Rolling up an unopened player would leave a rolled-up window
                // waiting for next time, which is not what the key looks like
                // it does — so open it first.
                if self.mini.is_none() {
                    self.toggle_mini();
                } else {
                    self.toggle_shade(winamp::Window::Main);
                }
            } else {
                self.toggle_mini();
            }
            return;
        }
        // The mouse's back/forward buttons walk the page history, like a
        // browser. Handled before the text-field guard: they are not typing.
        let (mouse_back, mouse_forward) = ctx.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Extra1),
                i.pointer.button_pressed(egui::PointerButton::Extra2),
            )
        });
        if mouse_back && self.can_go_back() {
            self.go_back();
        }
        if mouse_forward && self.can_go_forward() {
            self.go_forward();
        }
        // Escape clears a multi-selection first, before anything else.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.selected.is_empty() {
            self.clear_selection();
            return;
        }
        // Don't steal keys while typing in search or other text fields.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let next = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowRight));
        let prev = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowLeft));
        let space = ctx.input(|i| i.key_pressed(egui::Key::Space));
        let search = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F));
        let mute = ctx.input(|i| i.key_pressed(egui::Key::M));
        let fwd5 = ctx.input(|i| {
            i.key_pressed(egui::Key::ArrowRight) && !i.modifiers.ctrl && !i.modifiers.alt
        });
        let back5 = ctx.input(|i| {
            i.key_pressed(egui::Key::ArrowLeft) && !i.modifiers.ctrl && !i.modifiers.alt
        });
        if space {
            self.player.play_pause();
        }
        if next {
            self.player.next();
        }
        if prev {
            self.player.prev();
        }
        if search {
            self.navigate(crate::ui::route::Route::Search(String::new()));
        }
        if mute {
            let next = if self.settings.volume < 0.01 {
                0.8
            } else {
                0.0
            };
            self.settings.volume = next;
            self.player.set_volume(next);
        }
        if fwd5 || back5 {
            let pos = self.player.state.lock().position_ms;
            let target = if fwd5 {
                pos.saturating_add(5_000)
            } else {
                pos.saturating_sub(5_000)
            };
            self.player.seek_ms(target);
        }
    }

    fn poll_player_state(&mut self) {
        let st = self.player.state.lock();
        let current = st.current.and_then(|i| st.queue.get(i).cloned());
        let pos = st.position_ms;
        let playing = st.is_playing;
        let loading = st.loading;
        drop(st);
        // The track changed: AUTO loads its saved preset, and the PRESETS menu
        // has to know whether there is one to forget. Done on the edge rather
        // than every poll, because applying a curve saves settings to disk.
        let track_id = current.as_ref().map(|t| t.id);
        if track_id != self.last_track_id {
            self.last_track_id = track_id;
            self.autoload_eq_preset();
        }
        if let Some(track) = current.clone() {
            // Recently played: a track counts after 30 s (or half of a
            // shorter track); paused time and seeks do not count.
            match (Some(track.id), self.pending_listen) {
                (id, Some((pid, _))) if id != Some(pid) => {
                    self.pending_listen = id.map(|id| (id, 0));
                }
                (Some(id), None) if self.recent_ids.first() != Some(&id) => {
                    self.pending_listen = Some((id, 0));
                }
                _ => {}
            }
            if playing
                && !loading
                && let Some((pid, ms)) = self.pending_listen
                && pid == track.id
            {
                let delta = pos.saturating_sub(self.last_pos_ms);
                if delta <= 1500 {
                    let ms = ms + delta;
                    let threshold = counts_after(track.effective_duration_ms());
                    if ms >= threshold {
                        self.recent_ids.retain(|&id| id != pid);
                        self.recent_ids.insert(0, pid);
                        self.recent_ids.truncate(20);
                        self.recent_at.insert(pid, now_secs());
                        self.pending_listen = None;
                    } else {
                        self.pending_listen = Some((pid, ms));
                    }
                } else {
                    // Seek jump: seeking does not count.
                    self.pending_listen = Some((pid, 0));
                }
            }
            self.last_pos_ms = pos;
            if let Some(media) = &mut self.media {
                media.update_metadata(
                    &track.title,
                    track.artist(),
                    track.artwork_url(),
                    track.effective_duration_ms() as i64,
                );
                media.update_playback(playing, pos);
            }
        }
    }

    fn show_toasts(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        self.toasts
            .retain(|(_, born)| *born == 0.0 || now - born < 4.0);
        // Stamp new toasts with the current time.
        for (_, born) in self.toasts.iter_mut() {
            if *born == 0.0 {
                *born = now;
            }
        }
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -96.0])
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(self.theme.surface)
                    .show(ui, |ui| {
                        for (msg, _) in self.toasts.iter() {
                            ui.label(egui::RichText::new(msg).color(self.theme.text));
                        }
                    });
            });
    }
}

/// A play counts towards Recently Played after 30 seconds, or halfway
/// through a shorter track (at least 1 s).
pub fn counts_after(duration_ms: u64) -> u64 {
    30_000.min(duration_ms / 2).max(1_000)
}

/// Inclusive index span between two ids in table order (either direction).
/// `None` when either id is missing — the caller falls back to toggle.
pub fn range_between(order: &[u64], anchor: u64, id: u64) -> Option<(usize, usize)> {
    let mut found = (None, None);
    for (i, tid) in order.iter().enumerate() {
        if *tid == anchor {
            found.0 = Some(i);
        }
        if *tid == id {
            found.1 = Some(i);
        }
    }
    match found {
        (Some(a), Some(b)) => Some((a.min(b), a.max(b))),
        _ => None,
    }
}

/// Resolve a link to something playable/navigable (background task).
async fn resolve_link(client: &std::sync::Arc<crate::api::ApiClient>, raw: &str) -> LinkDone {
    use crate::api::endpoints;
    let link = match crate::link::parse(raw) {
        Ok(l) => l,
        Err(e) => return LinkDone::Failed(e.to_string()),
    };
    // Direct IDs need no /resolve round trip.
    let raw_owned = raw.to_string();
    let (kind, id) = match &link {
        crate::link::ParsedLink::TrackId(id) => ("track", *id),
        crate::link::ParsedLink::PlaylistId(id) => return LinkDone::OpenPlaylist(*id, raw_owned),
        crate::link::ParsedLink::UserId(id) => return LinkDone::OpenUser(*id, raw_owned),
        crate::link::ParsedLink::RemoteUrl(url) => match endpoints::resolve(client, url).await {
            Ok(v) => {
                let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                let id = v.get("id").and_then(|i| i.as_u64()).unwrap_or(0);
                if id == 0 {
                    return LinkDone::Failed("unsupported link".into());
                }
                match kind {
                    "playlist" => return LinkDone::OpenPlaylist(id, raw_owned),
                    "user" => return LinkDone::OpenUser(id, raw_owned),
                    _ => ("track", id),
                }
            }
            Err(e) => return LinkDone::Failed(e.to_string()),
        },
    };
    let _ = kind;
    match endpoints::track(client, id).await {
        Ok(t) => LinkDone::PlayTrack(Box::new(t), raw_owned),
        Err(e) => LinkDone::Failed(e.to_string()),
    }
}

/// Relative time like "5 minutes ago", "20 days ago", "5 years ago".
pub fn fmt_relative(now_secs: u64, at_secs: u64) -> String {
    let d = now_secs.saturating_sub(at_secs);
    const MIN: u64 = 60;
    const HOUR: u64 = 3_600;
    const DAY: u64 = 86_400;
    const MONTH: u64 = 2_592_000;
    const YEAR: u64 = 31_536_000;
    let (n, unit) = if d < MIN {
        return "just now".to_owned();
    } else if d < HOUR {
        (d / MIN, "minute")
    } else if d < DAY {
        (d / HOUR, "hour")
    } else if d < MONTH {
        (d / DAY, "day")
    } else if d < YEAR {
        (d / MONTH, "month")
    } else {
        (d / YEAR, "year")
    };
    if n <= 1 {
        if unit == "hour" {
            "an hour ago".to_owned()
        } else {
            format!("a {unit} ago")
        }
    } else {
        format!("{n} {unit}s ago")
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// What the mini player's window should measure, in points.
///
/// The skin is art at whole pixels, so it is sized in *physical* pixels and
/// divided back out by the desktop's scaling — asking for 275 points on a
/// 150 % display would land every sprite on a half-pixel.
fn mini_window_size(scale: u32, height: f32, pixels_per_point: f32) -> egui::Vec2 {
    let ppp = pixels_per_point.max(0.1);
    let scale = scale.clamp(1, 4) as f32;
    egui::vec2(winamp::WIDTH * scale / ppp, height * scale / ppp)
}

const MINI_REVEAL_TIMEOUT_SECS: f64 = 0.75;

fn mini_transition_should_reveal(
    current: egui::Vec2,
    target: egui::Vec2,
    elapsed_secs: f64,
) -> bool {
    (current - target).abs().max_elem() < 1.0 || elapsed_secs >= MINI_REVEAL_TIMEOUT_SECS
}

#[cfg(test)]
mod tests {
    use super::{
        counts_after, fmt_relative, mini_transition_should_reveal, mini_window_size, range_between,
    };

    #[test]
    fn history_threshold() {
        assert_eq!(counts_after(240_000), 30_000, "a four minute song");
        assert_eq!(counts_after(40_000), 20_000, "a forty second song");
        assert!(counts_after(0) >= 1_000);
    }

    #[test]
    fn range_selection_spans_either_direction() {
        let order = vec![10, 20, 30, 40, 50];
        assert_eq!(range_between(&order, 20, 40), Some((1, 3)));
        assert_eq!(range_between(&order, 40, 20), Some((1, 3)));
        assert_eq!(range_between(&order, 30, 30), Some((2, 2)));
        assert_eq!(range_between(&order, 20, 99), None);
        assert_eq!(range_between(&[], 1, 2), None);
    }

    #[test]
    fn relative_time() {
        let now = 1_700_000_000;
        assert_eq!(fmt_relative(now, now), "just now");
        assert_eq!(fmt_relative(now, now - 90), "a minute ago");
        assert_eq!(fmt_relative(now, now - 5 * 60), "5 minutes ago");
        assert_eq!(fmt_relative(now, now - 3_600), "an hour ago");
        assert_eq!(fmt_relative(now, now - 20 * 86_400), "20 days ago");
        assert_eq!(fmt_relative(now, now - 5 * 31_536_000), "5 years ago");
    }

    /// The skin is art at whole pixels, so the window it asks for is measured
    /// in *physical* pixels and divided back out by the desktop's scaling. Get
    /// this wrong on a 150 % display and every sprite lands on a half-pixel.
    #[test]
    fn the_mini_window_is_measured_in_physical_pixels() {
        use crate::ui::winamp;
        // 1:1 display: the window is exactly the skin.
        let plain = mini_window_size(1, winamp::HEIGHT, 1.0);
        assert_eq!(plain, egui::vec2(275.0, 116.0));
        // 2× skin on a 2× display is the same number of points.
        assert_eq!(mini_window_size(2, winamp::HEIGHT, 2.0), plain);
        // 150 %: 275 physical pixels are 183.33 points.
        let scaled = mini_window_size(1, winamp::HEIGHT, 1.5);
        assert!((scaled.x - 275.0 / 1.5).abs() < 0.01);
        // Rolled up, only the height changes.
        let rolled = mini_window_size(2, winamp::SHADE_HEIGHT, 1.0);
        assert_eq!(rolled.x, 550.0);
        assert_eq!(rolled.y, 28.0);
        // A nonsense scaling factor must not divide by zero.
        assert!(mini_window_size(1, winamp::HEIGHT, 0.0).x.is_finite());
    }

    #[test]
    fn a_stuck_mini_resize_reveals_the_window_after_timeout() {
        let current = egui::vec2(1180.0, 760.0);
        let target = egui::vec2(550.0, 232.0);

        assert!(!mini_transition_should_reveal(current, target, 0.1));
        assert!(mini_transition_should_reveal(current, target, 1.0));
    }
}
