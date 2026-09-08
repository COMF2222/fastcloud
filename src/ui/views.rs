use super::App;
use super::data;
use super::route::Route;
use super::theme::{Metrics, Type};
use super::widgets;
use crate::api::models::Track;
use crate::store::{Key, Slot};
use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryTab {
    #[default]
    Overview,
    Likes,
    Playlists,
    Albums,
    #[allow(dead_code)]
    Uploads,
    Stations,
    Following,
    History,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserTab {
    #[default]
    All,
    PopularTracks,
    Tracks,
    Albums,
    Playlists,
    Reposts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchTab {
    #[default]
    All,
    Tracks,
    People,
    Albums,
    Playlists,
}

impl LibraryTab {
    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Likes => "Likes",
            Self::Playlists => "Playlists",
            Self::Albums => "Albums",
            Self::Uploads => "Tracks",
            Self::Stations => "Stations",
            Self::Following => "Following",
            Self::History => "History",
        }
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    // One outer scroll for the whole page; inner lists expand into it.
    egui::ScrollArea::vertical()
        .id_salt("page")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(
                    Metrics::SP_075 as i8,
                    Metrics::SP_15 as i8,
                ))
                .show(ui, |ui| show_inner(app, ui));
        });
}

fn show_inner(app: &mut App, ui: &mut egui::Ui) {
    match app.route.clone() {
        Route::Home => home(app, ui),
        Route::Feed => feed(app, ui),
        Route::Library => library(app, ui),
        Route::Likes => {
            app.library_tab = LibraryTab::Likes;
            library(app, ui);
        }
        Route::Recent => recent(app, ui),
        Route::Following => {
            app.library_tab = LibraryTab::Following;
            library(app, ui);
        }
        Route::Search(_) => search(app, ui),
        Route::TrackDetail(id) => track_detail(app, ui, id),
        Route::PlaylistDetail(id) => playlist_detail(app, ui, id),
        Route::UserDetail(id) => user_detail(app, ui, id),
        Route::Settings => settings(app, ui),
    }
}

// ===== Home (soundcloud.com style) =====

fn home(app: &mut App, ui: &mut egui::Ui) {
    two_columns(app, ui, |app, ui| {
        ui.add_space(Metrics::SP_HALF);

        use crate::demo::{HomeShelf, ShelfKind, Source};
        let likes = HomeShelf {
            title: "Your Likes",
            subtitle: "Saved on your SoundCloud account",
            salt: "home-likes",
            kind: ShelfKind::Tracks,
            source: Source::MyLikes,
        };
        shelf_header(app, ui, likes.title, likes.subtitle);
        home_shelf(app, ui, &likes, Key::Likes);

        let recent = recent_tracks(app, 25);
        if recent.is_empty() {
            widgets::section_header(app, ui, "Recently Played");
            ui.label(Type::BODY.rich("Press play on anything to get started.", app.theme.text_dim));
        } else {
            recently_played_panel(app, ui, &recent);
        }

        let related_seed = recent
            .first()
            .map(|track| track.id)
            .or_else(|| app.tracks(Key::Likes).rows().first().map(|track| track.id));
        if let Some(seed) = related_seed {
            use crate::demo::{HomeShelf, ShelfKind, Source};
            let shelf = HomeShelf {
                title: "More of what you like",
                subtitle: "Related to music you've played and liked",
                salt: "home-related",
                kind: ShelfKind::Tracks,
                source: Source::RecentlyPlayed,
            };
            shelf_header(app, ui, shelf.title, shelf.subtitle);
            home_shelf(app, ui, &shelf, Key::Related(seed));
        }

        // …then every editorial shelf, in the site's order. A shelf with
        // nothing to seed from is skipped rather than shown empty.
        let shelves = [
            (
                HomeShelf {
                    title: "From people you follow",
                    subtitle: "Recent tracks from your SoundCloud subscriptions",
                    salt: "home-following-tracks",
                    kind: ShelfKind::Tracks,
                    source: Source::RecentlyPlayed,
                },
                Key::FollowingTracks,
            ),
            (
                HomeShelf {
                    title: "People you follow",
                    subtitle: "Your SoundCloud following list",
                    salt: "home-following",
                    kind: ShelfKind::Artists,
                    source: Source::RelatedUsers,
                },
                Key::Following,
            ),
        ];
        for (shelf, key) in shelves {
            shelf_header(app, ui, shelf.title, shelf.subtitle);
            home_shelf(app, ui, &shelf, key);
        }
        shelf_header(
            app,
            ui,
            "Your playlists",
            "Your own and saved SoundCloud playlists",
        );
        home_account_playlists(app, ui, false, "home-playlists");
        shelf_header(
            app,
            ui,
            "Your albums",
            "Albums saved on your SoundCloud account",
        );
        home_account_playlists(app, ui, true, "home-albums");
        footer(app, ui);
    });
}

fn recently_played_panel(app: &mut App, ui: &mut egui::Ui, tracks: &[Track]) {
    ui.add_space(Metrics::SP_6);
    widgets::section_header(app, ui, "Recently Played");
    let queue = std::sync::Arc::new(tracks.to_vec());
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(111, 151, 173))
        .corner_radius(Metrics::RADIUS)
        .inner_margin(egui::Margin::same(Metrics::SP_25 as i8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let first = &queue[0];
                let art = widgets::artwork_img(
                    ui,
                    first.artwork_url(),
                    first.id,
                    &first.title,
                    180.0,
                    Metrics::RADIUS as f32,
                );
                if art.clicked() {
                    app.play_user_queue((*queue).clone(), 0, false);
                }
                ui.vertical(|ui| {
                    ui.set_min_width(280.0);
                    egui::ScrollArea::vertical()
                        .id_salt("home-recent-tracks")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for (index, track) in queue.iter().enumerate() {
                                let response = ui
                                    .add(
                                        egui::Label::new(Type::H4.rich(
                                            &format!("{} – {}", track.artist(), track.title),
                                            egui::Color32::WHITE,
                                        ))
                                        .sense(egui::Sense::click())
                                        .truncate(),
                                    )
                                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                                if response.clicked() {
                                    app.play_user_queue((*queue).clone(), index, false);
                                }
                                if index + 1 < queue.len() {
                                    ui.separator();
                                }
                            }
                        });
                });
            });
        });
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.button("Go to history").clicked() {
            app.library_tab = LibraryTab::History;
            app.navigate(Route::Library);
        }
    });
}

fn home_account_playlists(app: &mut App, ui: &mut egui::Ui, albums: bool, salt: &str) {
    let owned = app.playlists(Key::MyPlaylists);
    let liked = app.playlists(Key::LikedPlaylists);
    let loading = owned.is_loading() || liked.is_loading();
    let mut playlists = owned.rows().to_vec();
    for playlist in liked.rows() {
        if !playlists.iter().any(|existing| existing.id == playlist.id) {
            playlists.push(playlist.clone());
        }
    }
    playlists.retain(|playlist| playlist.is_album() == albums);
    if playlists.is_empty() {
        ui.label(Type::BODY.rich(
            if loading {
                "Loading…"
            } else {
                "Nothing here yet."
            },
            app.theme.text_dim,
        ));
        return;
    }
    app.carousel(ui, salt, |app, ui| {
        for playlist in &playlists {
            let tracks = playlist_tracks(app, playlist.id);
            let fallback_art = tracks.first().and_then(Track::artwork_url);
            let click = widgets::card(
                app,
                ui,
                widgets::Card::opens(
                    playlist.id,
                    &playlist.title,
                    playlist
                        .user
                        .as_ref()
                        .map(|user| user.username.as_str())
                        .unwrap_or(if albums { "Album" } else { "Playlist" }),
                )
                .art(playlist.artwork_url().or(fallback_art))
                .artist(playlist.user.as_ref().map(|user| user.id)),
            );
            if click.play && !tracks.is_empty() {
                app.play_user_queue(tracks, 0, false);
            } else if click.open {
                app.navigate(Route::PlaylistDetail(playlist.id));
            }
        }
    });
}

/// A shelf heading plus its optional explainer line.
fn shelf_header(app: &App, ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.add_space(Metrics::SP_5);
    ui.label(Type::H2.rich(title, app.theme.text));
    if !subtitle.is_empty() {
        ui.label(Type::BODY.rich(subtitle, app.theme.text_dim));
    }
    ui.add_space(Metrics::SP_075);
}

/// One home shelf, drawn from `key`.
///
/// The kind decides the card; the key decides the rows. Both paths (live and
/// demo) come through [`App::tracks`] and friends, so nothing here knows
/// which is running.
fn home_shelf(app: &mut App, ui: &mut egui::Ui, shelf: &crate::demo::HomeShelf, key: Key) {
    use crate::demo::ShelfKind;
    let salt = shelf.salt;
    match shelf.kind {
        ShelfKind::Tracks => {
            let slot = app.tracks(key);
            if data::placeholder(app, ui, &slot, "Nothing here yet.") {
                return;
            }
            track_cards(app, ui, salt, slot.rows());
        }
        ShelfKind::Playlists | ShelfKind::Albums => {
            let slot = app.playlists(key);
            if data::placeholder(app, ui, &slot, "Nothing here yet.") {
                return;
            }
            let albums = shelf.kind == ShelfKind::Albums;
            let playlists: Vec<crate::api::models::Playlist> = slot
                .rows()
                .iter()
                .filter(|pl| pl.is_album() == albums)
                .cloned()
                .collect();
            if playlists.is_empty() {
                ui.label(Type::BODY.rich("Nothing here yet.", app.theme.text_dim));
                return;
            }
            app.carousel(ui, salt, |app, ui| {
                for pl in &playlists {
                    let pid = pl.id;
                    let title = pl.title.clone();
                    let count = pl.track_count.unwrap_or(0);
                    let art = pl.artwork_url();
                    let kind = if pl.is_album() { "Album" } else { "Playlist" };
                    let click = widgets::card(
                        app,
                        ui,
                        widgets::Card::opens(pid, &title, &format!("{kind} · {count} tracks"))
                            .art(art),
                    );
                    if click.play {
                        let tracks = playlist_tracks(app, pid);
                        if tracks.is_empty() {
                            // Not fetched yet: open it, which starts the fetch.
                            app.navigate(Route::PlaylistDetail(pid));
                        } else {
                            app.play_user_queue(tracks, 0, false);
                        }
                    } else if click.open {
                        app.navigate(Route::PlaylistDetail(pid));
                    }
                }
            });
        }
        ShelfKind::Artists => {
            let slot = app.users(key);
            if data::placeholder(app, ui, &slot, "No artists to show yet.") {
                return;
            }
            let users = slot.rows().to_vec();
            app.carousel_sized(ui, salt, Some(AVATAR_CARD), |app, ui| {
                for user in &users {
                    artist_card(app, ui, user, AVATAR_CARD);
                }
            });
        }
        ShelfKind::Stations => {
            let slot = app.tracks(key);
            if data::placeholder(app, ui, &slot, "Play something to seed a station.") {
                return;
            }
            let seeds = std::sync::Arc::new(slot.rows().to_vec());
            app.carousel(ui, salt, |app, ui| {
                for seed in seeds.iter() {
                    let art = seed.artwork_url();
                    let click = widgets::card(
                        app,
                        ui,
                        widgets::Card::plays(
                            seed.id,
                            &format!("{} Station", seed.artist()),
                            "Artist station",
                        )
                        .art(art),
                    );
                    if click.play {
                        // A station keeps going: autoplay on, like the site.
                        app.settings.autoplay = true;
                        app.player.set_autoplay(true);
                        let queue = app.station_queue(seed);
                        app.play_user_queue(queue, 0, false);
                    }
                }
            });
        }
        ShelfKind::Picks => {
            // One artist's likes, as SoundCloud's "Liked By" shelf.
            let slot = app.tracks(key.clone());
            if data::placeholder(app, ui, &slot, "Follow someone to see their picks.") {
                return;
            }
            let owner = match &key {
                Key::UserLikes(id) => app
                    .user(*id)
                    .ready()
                    .map(|u| u.username)
                    .unwrap_or_else(|| "Their".to_owned()),
                _ => "Their".to_owned(),
            };
            let tracks = std::sync::Arc::new(slot.rows().to_vec());
            app.carousel(ui, salt, |app, ui| {
                for (i, track) in tracks.iter().enumerate() {
                    let tracks = tracks.clone();
                    let click = widgets::card(
                        app,
                        ui,
                        widgets::Card {
                            subtitle: &format!("{owner}'s pick"),
                            ..widgets::Card::track(track)
                        },
                    );
                    if click.play {
                        app.play_user_queue((*tracks).clone(), i, false);
                    }
                }
            });
        }
        ShelfKind::Genres => {
            let genres = crate::demo::demo_genres();
            // SoundCloud's shelf is a row of artwork cards. Use the leading
            // track from each public genre window as that genre's live cover.
            let cards: Vec<(&str, Option<Track>)> = genres
                .iter()
                .map(|genre| {
                    let query = genre_query(genre);
                    let cover = app.tracks(Key::Genre(query)).rows().first().cloned();
                    (*genre, cover)
                })
                .collect();
            app.carousel(ui, salt, |app, ui| {
                for (genre, cover) in &cards {
                    if genre_card(app, ui, genre, cover.as_ref()) {
                        let query = genre_query(genre);
                        app.search_query = query.clone();
                        app.navigate(Route::Search(query));
                    }
                }
            });
        }
    }
}

/// Avatar card edge (`--artwork-15x-size`).
const AVATAR_CARD: f32 = 120.0;
const LIBRARY_CARD: f32 = 180.0;

fn library_columns(width: f32) -> usize {
    ((width + Metrics::SP_3) / (LIBRARY_CARD + Metrics::SP_3))
        .floor()
        .clamp(1.0, 6.0) as usize
}
type PlaylistCardItem = (u64, String, String, u64, bool, Option<String>);

/// Round artist avatar + name, as the "Artists to watch out for" shelf.
fn artist_card(app: &mut App, ui: &mut egui::Ui, artist: &crate::api::models::User, size: f32) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.set_min_width(size);
        ui.set_max_width(size);
        // A real avatar when the API gave one, else the deterministic disc.
        let avatar = artist.avatar_url.as_deref().filter(|u| !u.is_empty());
        let resp = match avatar {
            Some(url) => round_avatar(ui, url, &artist.username, size),
            None => avatar_circle(ui, &artist.username, size),
        };
        let id = artist.id;
        if resp.clicked() {
            app.navigate(Route::UserDetail(id));
        }
        ui.add(egui::Label::new(Type::H4.rich(&artist.username, app.theme.text)).truncate());
        ui.label(Type::CAPTION.rich(
            &format!("{} followers", fmt_count(artist.followers_count)),
            app.theme.text_dim,
        ));
    });
}

/// A circular cover: the loaded avatar clipped to a disc, or the initial.
fn round_avatar(ui: &mut egui::Ui, url: &str, name: &str, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let image = egui::Image::new(url)
            .show_loading_spinner(false)
            .fit_to_exact_size(egui::vec2(size, size))
            .corner_radius(size / 2.0);
        if image.load_for_size(ui.ctx(), rect.size()).is_ok() {
            image.paint_at(ui, rect);
        } else {
            paint_initial(ui, rect, name, size);
        }
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The first letter on the name's own colour — one placeholder, used by both
/// [`avatar_circle`] and [`round_avatar`].
fn paint_initial(ui: &egui::Ui, rect: egui::Rect, name: &str, size: f32) {
    let painter = ui.painter();
    painter.circle_filled(
        rect.center(),
        size / 2.0,
        widgets::art_color(name_seed(name)),
    );
    let letter = name
        .chars()
        .find(|c| !c.is_whitespace())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_owned());
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        super::theme::bold((size * 0.4).clamp(10.0, 48.0)),
        egui::Color32::WHITE,
    );
}

fn genre_query(genre: &str) -> String {
    if genre == "All Genres" {
        String::new()
    } else {
        genre.to_owned()
    }
}

/// Artwork, genre name and `Trending`, matching SoundCloud's genre shelf.
fn genre_card(app: &mut App, ui: &mut egui::Ui, genre: &str, cover: Option<&Track>) -> bool {
    let seed = cover.map(|track| track.id).unwrap_or_else(|| {
        genre.bytes().fold(0_u64, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(u64::from(byte))
        })
    });
    let mut clicked = false;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.set_min_width(widgets::CARD_ART);
        ui.set_max_width(widgets::CARD_ART);
        clicked |= widgets::artwork_img(
            ui,
            cover.and_then(Track::artwork_url),
            seed,
            genre,
            widgets::CARD_ART,
            Metrics::RADIUS as f32,
        )
        .clicked();
        clicked |= ui
            .add(
                egui::Label::new(Type::H4.rich(genre, app.theme.text))
                    .sense(egui::Sense::click())
                    .truncate(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked();
        ui.label(Type::BODY.rich("Trending", app.theme.text_dim));
    });
    clicked
}

fn track_cards(app: &mut App, ui: &mut egui::Ui, salt: &str, tracks: &[Track]) {
    let shared = std::sync::Arc::new(tracks.to_vec());
    app.carousel(ui, salt, |app, ui| {
        for (i, track) in shared.iter().enumerate() {
            let shared = shared.clone();
            let click = widgets::card(app, ui, widgets::Card::track(track));
            if click.play {
                app.play_user_queue((*shared).clone(), i, false);
            }
        }
    });
}

// ===== Feed =====

fn feed(app: &mut App, ui: &mut egui::Ui) {
    two_columns(app, ui, |app, ui| {
        ui.add_space(4.0);
        if !app.feed_banner_dismissed {
            egui::Frame::new()
                .fill(app.theme.surface)
                .corner_radius(Metrics::RADIUS_LG)
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("This is your feed")
                                    .font(Type::H4.font())
                                    .color(app.theme.text),
                            );
                            ui.label(
                                egui::RichText::new(
                                    "Follow your favorite artists and see every track they post right here.",
                                )
                                .font(Type::CAPTION.font())
                                .color(app.theme.text_dim),
                            );
                        });
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Min),
                            |ui| {
                                if super::icons::show(
                                    ui,
                                    super::icons::Icon::X,
                                    15.0,
                                    app.theme.text_dim,
                                )
                                .clicked()
                                {
                                    app.feed_banner_dismissed = true;
                                }
                            },
                        );
                    });
                });
            ui.add_space(8.0);
        }
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new("Hear the latest posts from the people you're following:")
                    .font(Type::H3.font())
                    .color(app.theme.text),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut app.show_reposts, "Reposts");
            });
        });
        ui.add_space(Metrics::SP_1);
        // One `/me/feed` response contains both tracks and playlists. The
        // store keeps both halves so playlist posts are not silently lost.
        let slot = app.tracks(Key::Feed);
        let playlist_slot = app.playlists(Key::Feed);
        if !matches!(slot, Slot::Ready(_)) {
            let _ = data::placeholder(app, ui, &slot, "");
            return;
        }
        if slot.rows().is_empty() && playlist_slot.rows().is_empty() {
            ui.label(Type::BODY.rich(
                "Nothing yet — follow some artists and their posts land here.",
                app.theme.text_dim,
            ));
            return;
        }
        // Demo mode has its own activity lines ("X reposted a track"); the
        // live feed does not say who, so the card carries only the track.
        let activity: std::collections::HashMap<u64, crate::demo::FeedItem> = if app.demo {
            crate::demo::demo_feed()
                .into_iter()
                .map(|item| (item.track_id, item))
                .collect()
        } else {
            std::collections::HashMap::new()
        };
        for track in slot.rows() {
            let item = activity.get(&track.id);
            if (track.feed_reposted || item.is_some_and(|i| i.kind == "reposted"))
                && !app.show_reposts
            {
                continue;
            }
            let line = item.map(|i| {
                (
                    i.user.clone(),
                    i.kind.to_string(),
                    format!("{} ago", i.when),
                )
            });
            feed_card(app, ui, track, line, None);
            ui.add_space(Metrics::SP_175);
        }
        if !playlist_slot.rows().is_empty() {
            let playlists: Vec<_> = playlist_slot
                .rows()
                .iter()
                .filter(|playlist| app.show_reposts || !playlist.feed_reposted)
                .cloned()
                .collect();
            if !playlists.is_empty() {
                widgets::section_header(app, ui, "Playlist posts");
                playlist_carousel(app, ui, "feed-playlists", &playlists);
            }
        }
    });
}

/// One SoundCloud-style feed card: activity line, artwork + play + waveform,
/// like/repost/share actions with real counts.
fn feed_card(
    app: &mut App,
    ui: &mut egui::Ui,
    track: &Track,
    activity: Option<(String, String, String)>,
    stamp: Option<String>,
) {
    if let Some((user, kind, when)) = activity {
        let verb = match kind.as_str() {
            "reposted" => "reposted",
            "liked" => "liked",
            _ => "posted",
        };
        ui.horizontal(|ui| {
            avatar_circle(ui, &user, 30.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    egui::RichText::new(&user)
                        .font(Type::H5.font())
                        .color(app.theme.text),
                );
                ui.label(
                    egui::RichText::new(format!("{verb} a track {when}"))
                        .font(Type::CAPTION.font())
                        .color(app.theme.text_dim),
                );
            });
        });
        ui.add_space(6.0);
    }
    ui.horizontal_wrapped(|ui| {
        let art = track.artwork_url();
        if widgets::artwork_img(ui, art, track.id, &track.title, 150.0, 6.0).clicked() {
            app.navigate(Route::TrackDetail(track.id));
        }
        ui.vertical(|ui| {
            ui.set_min_width(200.0);
            ui.horizontal(|ui| {
                if round_play_button(app, ui, 44.0) {
                    app.play_user_queue(vec![track.clone()], 0, false);
                }
                ui.vertical(|ui| {
                    let artist = ui.add(
                        egui::Label::new(
                            egui::RichText::new(crate::bidi::owned(track.artist()))
                                .font(Type::CAPTION.font())
                                .color(app.theme.text_dim),
                        )
                        .sense(egui::Sense::click()),
                    );
                    if let Some(user) = &track.user
                        && artist
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                    {
                        app.navigate(Route::UserDetail(user.id));
                    }
                    let title = egui::RichText::new(crate::bidi::owned(&track.title))
                        .font(Type::H4.font())
                        .color(app.theme.text);
                    if ui
                        .add(
                            egui::Label::new(title)
                                .sense(egui::Sense::click())
                                .truncate(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        app.navigate(Route::TrackDetail(track.id));
                    }
                });
                if let Some(genre) = track.genre.as_deref() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if let Some(stamp) = stamp.clone() {
                            ui.label(
                                egui::RichText::new(stamp)
                                    .font(Type::CAPTION.font())
                                    .color(app.theme.text_dim),
                            );
                        }
                        egui::Frame::new()
                            .fill(app.theme.surface_hover)
                            .corner_radius(10.0)
                            .inner_margin(egui::Margin::symmetric(8, 3))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(format!("# {genre}"))
                                        .font(Type::CAPTION.font())
                                        .color(app.theme.text_dim),
                                );
                            });
                    });
                } else if let Some(stamp) = stamp.clone() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        ui.label(
                            egui::RichText::new(stamp)
                                .font(Type::CAPTION.font())
                                .color(app.theme.text_dim),
                        );
                    });
                }
            });
            // Inline waveform: live when current, click-to-play otherwise.
            let dur = track.effective_duration_ms();
            let (pos, is_current) = {
                let st = app.player.state.lock();
                match st.current.and_then(|i| st.queue.get(i).cloned()) {
                    Some(t) if t.id == track.id => (st.position_ms, true),
                    _ => (0, false),
                }
            };
            let peaks = track
                .waveform_url
                .as_deref()
                .and_then(|url| app.waveforms.get(ui.ctx(), url));
            if let Some(frac) =
                widgets::waveform(app, ui, track.id, pos, dur, 80.0, 110, peaks.as_deref())
            {
                if is_current {
                    app.player.seek_ms((frac as f64 * dur as f64) as u64);
                } else {
                    app.play_user_queue(vec![track.clone()], 0, false);
                }
            }
            // Action row with real counts.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let likes = track.likes_count.unwrap_or(0);
                let liked = app.is_liked(track.id);
                let like_label = if likes > 0 {
                    format!("  {}", crate::util::play_count(likes + u64::from(liked)))
                } else {
                    String::new()
                };
                if action_button(
                    app,
                    ui,
                    super::icons::Icon::Heart,
                    &like_label,
                    liked,
                    "Like",
                ) {
                    app.toggle_like(track.id);
                }
                let reposts = track.favoritings_count.unwrap_or(0);
                let repost_label = if reposts > 0 {
                    format!("  {}", crate::util::play_count(reposts))
                } else {
                    String::new()
                };
                if action_button(
                    app,
                    ui,
                    super::icons::Icon::Repeat,
                    &repost_label,
                    false,
                    "Repost",
                ) {
                    app.toast(format!("Reposted {}", track.title));
                }
                if action_button(app, ui, super::icons::Icon::External, "", false, "Share") {
                    app.toast("Link copied");
                }
                if action_button(app, ui, super::icons::Icon::Copy, "", false, "Copy link") {
                    app.toast("Link copied");
                }
                if action_button(app, ui, super::icons::Icon::Ellipsis, "", false, "More") {
                    app.navigate(Route::TrackDetail(track.id));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        super::icons::show_static(
                            ui,
                            super::icons::Icon::Music,
                            11.0,
                            app.theme.text_dim,
                        );
                        ui.label(
                            egui::RichText::new(crate::util::play_count(
                                track.playback_count.unwrap_or(0),
                            ))
                            .font(Type::CAPTION.font())
                            .color(app.theme.text_dim),
                        );
                    });
                });
            });
        });
    });
}

/// Small pill action button with an SVG icon and optional count.
/// Returns true when clicked. `active` tints the icon orange.
fn action_button(
    app: &App,
    ui: &mut egui::Ui,
    icon: super::icons::Icon,
    text: &str,
    active: bool,
    hover: &str,
) -> bool {
    let tint = if active {
        app.theme.accent
    } else {
        app.theme.text
    };
    egui::Frame::new()
        .fill(app.theme.surface)
        .corner_radius(Metrics::RADIUS)
        .inner_margin(egui::Margin::symmetric(
            Metrics::SP_1 as i8,
            Metrics::SP_HALF as i8,
        ))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = Metrics::SP_HALF;
                super::icons::show_static(ui, icon, 14.0, tint);
                if !text.is_empty() {
                    ui.label(Type::CAPTION.rich(text, tint));
                }
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_text(hover)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// Round play button used on feed cards and banners: SoundCloud's orange
/// disc with a white triangle, drawn rather than typed so it never depends on
/// a glyph.
fn round_play_button(app: &App, ui: &mut egui::Ui, size: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let grow = if resp.hovered() { 1.06 } else { 1.0 };
        ui.painter()
            .circle_filled(rect.center(), size / 2.0 * grow, app.theme.accent);
        let c = rect.center();
        let s = size * 0.24;
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                c + egui::vec2(-s * 0.6, -s),
                c + egui::vec2(-s * 0.6, s),
                c + egui::vec2(s * 0.8, 0.0),
            ],
            app.theme.on_accent,
            egui::Stroke::NONE,
        ));
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

// ===== Library =====

fn library(app: &mut App, ui: &mut egui::Ui) {
    ui.add_space(Metrics::SP_2);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = Metrics::SP_4;
        for tab in [
            LibraryTab::Overview,
            LibraryTab::Likes,
            LibraryTab::Playlists,
            LibraryTab::Albums,
            LibraryTab::Stations,
            LibraryTab::Following,
            LibraryTab::History,
        ] {
            let active = app.library_tab == tab;
            // Same size in both states, as SoundCloud's own tabs: the ink and
            // the rule under the active one carry the difference.
            let text = if active {
                Type::H2.rich(tab.label(), app.theme.text)
            } else {
                Type::H2.rich(tab.label(), app.theme.text_dim)
            };
            let resp = ui
                .add(egui::Label::new(text).sense(egui::Sense::click()))
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if active {
                let r = resp.rect;
                ui.painter().hline(
                    r.x_range(),
                    r.bottom() + 2.0,
                    egui::Stroke::new(2.0, app.theme.accent),
                );
            }
            if resp.clicked() {
                app.library_tab = tab;
            }
        }
    });
    ui.add_space(Metrics::SP_3);
    match app.library_tab {
        LibraryTab::Overview => overview(app, ui),
        LibraryTab::Likes => likes_tab(app, ui),
        LibraryTab::Playlists => playlists_tab(app, ui),
        LibraryTab::Albums => albums_tab(app, ui),
        LibraryTab::Uploads => uploads_tab(app, ui),
        LibraryTab::Stations => stations_tab(app, ui),
        LibraryTab::Following => artists_tab(app, ui),
        LibraryTab::History => history_tab(app, ui),
    }
}

#[allow(dead_code)]
fn uploads_tab(app: &mut App, ui: &mut egui::Ui) {
    let dropped = ui.ctx().input(|input| {
        input
            .raw
            .dropped_files
            .iter()
            .find(|file| !file.path().as_os_str().is_empty())
            .map(|file| file.path().to_path_buf())
    });
    if let Some(path) = dropped {
        app.creator.upload_path = path.display().to_string();
        if app.creator.upload_title.trim().is_empty() {
            app.creator.upload_title = path
                .file_stem()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
        }
    }

    egui::Frame::new()
        .fill(app.theme.surface)
        .corner_radius(Metrics::RADIUS_LG)
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.label(Type::H4.rich("Upload to SoundCloud", app.theme.text));
            ui.label(Type::CAPTION.rich(
                "Drop an audio file on this window or paste its full path. The file streams directly to SoundCloud and is not copied into Fastcloud.",
                app.theme.text_dim,
            ));
            ui.add_space(Metrics::SP_1);
            ui.add(
                egui::TextEdit::singleline(&mut app.creator.upload_path)
                    .hint_text("Audio file path")
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.creator.upload_title)
                        .hint_text("Title (required)")
                        .desired_width(300.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut app.creator.upload_artist)
                        .hint_text("Artist")
                        .desired_width(240.0),
                );
                ui.checkbox(&mut app.creator.upload_public, "Public");
            });
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.creator.upload_genre)
                        .hint_text("Genre")
                        .desired_width(220.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut app.creator.upload_tags)
                        .hint_text("Tags separated by spaces")
                        .desired_width(360.0),
                );
            });
            ui.add(
                egui::TextEdit::multiline(&mut app.creator.upload_description)
                    .hint_text("Description")
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            let clicked = ui
                .add_enabled(
                    !app.creator.uploading,
                    egui::Button::new(if app.creator.uploading {
                        "Uploading…"
                    } else {
                        "Upload track"
                    }),
                )
                .clicked();
            if clicked {
                app.upload_track();
            }
        });
    ui.add_space(Metrics::SP_2);
    account_tracks_tab(app, ui, Key::MyTracks, "Your uploaded tracks");
}

fn likes_tab(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(Type::H4.rich("Hear the tracks you've liked:", app.theme.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.filter)
                    .hint_text("Filter")
                    .desired_width(220.0),
            );
            let list_tint = if app.likes_view_grid {
                app.theme.text_dim
            } else {
                app.theme.accent
            };
            if super::icons::show(ui, super::icons::Icon::List, 17.0, list_tint)
                .on_hover_text("List view")
                .clicked()
            {
                app.likes_view_grid = false;
            }
            let grid_tint = if app.likes_view_grid {
                app.theme.accent
            } else {
                app.theme.text_dim
            };
            if super::icons::show(ui, super::icons::Icon::Grid, 17.0, grid_tint)
                .on_hover_text("Grid view")
                .clicked()
            {
                app.likes_view_grid = true;
            }
            ui.label(Type::CAPTION.rich("View", app.theme.text_dim));
        });
    });
    ui.add_space(Metrics::SP_1);
    let slot = app.tracks(Key::Likes);
    if data::placeholder(
        app,
        ui,
        &slot,
        "Tap the heart on any track to save it here.",
    ) {
        return;
    }
    let tracks = filtered(app, slot.rows());
    if tracks.is_empty() {
        ui.label(Type::BODY.rich("No likes match this filter.", app.theme.text_dim));
    } else if app.likes_view_grid {
        likes_grid(app, ui, &tracks);
    } else {
        track_list(app, ui, &tracks);
    }
}

fn account_tracks_tab(app: &mut App, ui: &mut egui::Ui, key: Key, title: &str) {
    ui.label(Type::H4.rich(title, app.theme.text));
    ui.add_space(Metrics::SP_1);
    let slot = app.tracks(key);
    if data::placeholder(app, ui, &slot, "Nothing here yet.") {
        return;
    }
    let tracks = filtered(app, slot.rows());
    track_list(app, ui, &tracks);
}

/// Apply the page's filter box to a list of tracks.
fn filtered(app: &App, tracks: &[Track]) -> Vec<Track> {
    let needle = app.filter.trim().to_lowercase();
    if needle.is_empty() {
        return tracks.to_vec();
    }
    tracks
        .iter()
        .filter(|t| {
            t.title.to_lowercase().contains(&needle) || t.artist().to_lowercase().contains(&needle)
        })
        .cloned()
        .collect()
}

/// Library overview: recently played cards + likes grid, like soundcloud.com.
fn overview(app: &mut App, ui: &mut egui::Ui) {
    widgets::section_header(app, ui, "Recently played");
    let recent = recent_tracks(app, 6);
    if recent.is_empty() {
        ui.label(Type::BODY.rich(
            "Nothing played yet. Press play on anything.",
            app.theme.text_dim,
        ));
    } else {
        track_cards(app, ui, "overview-recent", &recent);
    }

    widgets::section_header(app, ui, "Likes");
    let slot = app.tracks(Key::Likes);
    if data::placeholder(
        app,
        ui,
        &slot,
        "Tap the heart on any track to save it here.",
    ) {
        return;
    }
    let likes: Vec<Track> = slot.rows().iter().take(6).cloned().collect();
    likes_grid(app, ui, &likes);

    let playlists = merged_library_playlists(app);
    let playlist_items: Vec<PlaylistCardItem> = playlists
        .iter()
        .filter(|playlist| !playlist.is_album())
        .take(6)
        .map(|playlist| {
            (
                playlist.id,
                playlist.title.clone(),
                playlist
                    .user
                    .as_ref()
                    .map(|user| user.username.clone())
                    .unwrap_or_else(|| "Playlist".to_owned()),
                playlist.id + 7,
                false,
                playlist.artwork_url().map(str::to_owned),
            )
        })
        .collect();
    if !playlist_items.is_empty() {
        widgets::section_header(app, ui, "Playlists");
        playlist_card_grid(app, ui, &playlist_items);
    }

    let album_items: Vec<PlaylistCardItem> = playlists
        .iter()
        .filter(|playlist| playlist.is_album())
        .take(6)
        .map(|playlist| {
            (
                playlist.id,
                playlist.title.clone(),
                playlist
                    .user
                    .as_ref()
                    .map(|user| user.username.clone())
                    .unwrap_or_else(|| "Album".to_owned()),
                playlist.id + 7,
                false,
                playlist.artwork_url().map(str::to_owned),
            )
        })
        .collect();
    if !album_items.is_empty() {
        widgets::section_header(app, ui, "Albums");
        playlist_card_grid(app, ui, &album_items);
    }

    let following: Vec<_> = app
        .users(Key::Following)
        .rows()
        .iter()
        .take(6)
        .cloned()
        .collect();
    if !following.is_empty() {
        widgets::section_header(app, ui, "Following");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(Metrics::SP_3, Metrics::SP_6);
            for artist in &following {
                artist_card(app, ui, artist, LIBRARY_CARD);
            }
        });
    }
    footer(app, ui);
}

fn merged_library_playlists(app: &App) -> Vec<crate::api::models::Playlist> {
    let mut playlists = app.playlists(Key::MyPlaylists).rows().to_vec();
    for playlist in app.playlists(Key::LikedPlaylists).rows() {
        if !playlists.iter().any(|existing| existing.id == playlist.id) {
            playlists.push(playlist.clone());
        }
    }
    playlists
}

/// Card grid with ♥-prefixed titles, like the Library likes section.
fn likes_grid(app: &mut App, ui: &mut egui::Ui, tracks: &[Track]) {
    let shared = std::sync::Arc::new(tracks.to_vec());
    let columns = library_columns(ui.available_width());
    for (row_index, row) in shared.chunks(columns).enumerate() {
        ui.push_id(("likes-row", row_index), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(Metrics::SP_3, Metrics::SP_6);
                for (column, track) in row.iter().enumerate() {
                    let i = row_index * columns + column;
                    let queue = shared.clone();
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.set_min_width(LIBRARY_CARD);
                        ui.set_max_width(LIBRARY_CARD);
                        let art = track.artwork_url();
                        if widgets::artwork_img(
                            ui,
                            art,
                            track.id,
                            &track.title,
                            LIBRARY_CARD,
                            Metrics::RADIUS as f32,
                        )
                        .clicked()
                        {
                            app.play_user_queue((*queue).clone(), i, false);
                        }
                        let title =
                            egui::RichText::new(format!("♥ {}", crate::bidi::owned(&track.title)))
                                .font(Type::H4.font())
                                .color(app.theme.text);
                        if ui
                            .add(
                                egui::Label::new(title)
                                    .sense(egui::Sense::click())
                                    .truncate(),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            app.play_user_queue((*queue).clone(), i, false);
                        }
                        let artist = ui.add(
                            egui::Label::new(
                                egui::RichText::new(crate::bidi::owned(track.artist()))
                                    .font(Type::CAPTION.font())
                                    .color(app.theme.text_dim),
                            )
                            .sense(egui::Sense::click())
                            .truncate(),
                        );
                        if let Some(user) = &track.user
                            && artist
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                        {
                            app.navigate(Route::UserDetail(user.id));
                        }
                    });
                }
            });
        });
    }
}

fn playlists_tab(app: &mut App, ui: &mut egui::Ui) {
    // New playlist composer.
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.new_playlist_name)
                .hint_text("New playlist name…")
                .desired_width(240.0),
        );
        if ui.button("+ Create").clicked() {
            let name = app.new_playlist_name.trim().to_owned();
            if name.is_empty() {
                app.toast("Give the playlist a name first");
            } else {
                app.create_playlist(name.clone());
                app.new_playlist_name.clear();
                app.toast(format!("Creating playlist {name}…"));
            }
        }
    });
    ui.add_space(Metrics::SP_1);
    ui.horizontal(|ui| {
        ui.label(Type::H4.rich(
            "Hear your own playlists and the playlists you've liked:",
            app.theme.text,
        ));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::ComboBox::from_id_salt("playlist-scope")
                .selected_text(if app.playlists_mine_only {
                    "Yours"
                } else {
                    "All"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.playlists_mine_only, false, "All");
                    ui.selectable_value(&mut app.playlists_mine_only, true, "Yours");
                });
            ui.add(
                egui::TextEdit::singleline(&mut app.filter)
                    .hint_text("Filter")
                    .desired_width(200.0),
            );
        });
    });
    ui.add_space(Metrics::SP_1);

    let q = app.filter.trim().to_lowercase();
    // (id, title, subtitle, seed, liked_heart)
    let mut items: Vec<PlaylistCardItem> = Vec::new();
    // The synthetic Liked Songs card belongs only to the offline demo. Live
    // Library mirrors SoundCloud's own playlist collection exactly.
    if app.demo {
        let liked_n = app.tracks(Key::Likes).rows().len().max(app.liked.len());
        items.push((
            0,
            "Liked Songs".to_owned(),
            format!("Playlist • {liked_n} tracks"),
            7,
            true,
            None,
        ));
    }
    let owned_slot = app.playlists(Key::MyPlaylists);
    let liked_slot = app.playlists(Key::LikedPlaylists);
    let mut playlists = owned_slot.rows().to_vec();
    for playlist in liked_slot.rows() {
        if !playlists.iter().any(|existing| existing.id == playlist.id) {
            playlists.push(playlist.clone());
        }
    }
    for pl in &playlists {
        if pl.is_album() {
            continue;
        }
        let n = pl.track_count.unwrap_or(0);
        let mine = app
            .account()
            .zip(pl.user.as_ref())
            .is_some_and(|(me, owner)| me.id == owner.id);
        if app.playlists_mine_only && !mine {
            continue;
        }
        items.push((
            pl.id,
            pl.title.clone(),
            format!(
                "{} • {n} tracks",
                pl.user
                    .as_ref()
                    .map(|owner| owner.username.as_str())
                    .unwrap_or(if mine { "You" } else { "Unknown creator" })
            ),
            pl.id + 7,
            false,
            pl.artwork_url().map(str::to_owned),
        ));
    }
    if app.demo {
        for p in app.settings.custom_playlists.clone() {
            let n = app.playlist_track_ids(p.id).len().max(p.track_ids.len());
            items.push((
                p.id,
                p.title.clone(),
                format!("By you • {n} tracks"),
                p.id + 7,
                false,
                None,
            ));
        }
    }
    items.retain(|(_, title, _, _, _, _)| q.is_empty() || title.to_lowercase().contains(&q));
    if items.is_empty() {
        if !matches!(owned_slot, crate::store::Slot::Ready(_)) {
            data::placeholder(app, ui, &owned_slot, "");
            return;
        }
        if !matches!(liked_slot, crate::store::Slot::Ready(_)) {
            data::placeholder(app, ui, &liked_slot, "");
            return;
        }
        ui.label(Type::BODY.rich("No playlists match.", app.theme.text_dim));
        return;
    }
    playlist_card_grid(app, ui, &items);
}

/// Artwork for the Liked Songs card: SoundCloud's `--artist-surface-color`
/// with a heart, drawn as a shape (the interface face has no ♥).
fn liked_art(app: &App, ui: &mut egui::Ui, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        ui.painter()
            .rect_filled(rect, Metrics::RADIUS, super::theme::ARTIST);
        heart(
            ui.painter(),
            rect.center(),
            size * 0.22,
            app.theme.on_accent,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A filled heart: two lobes and a point, so no font is involved.
fn heart(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: egui::Color32) {
    let lobe = radius * 0.55;
    let up = center - egui::vec2(0.0, radius * 0.22);
    painter.circle_filled(up - egui::vec2(lobe * 0.85, 0.0), lobe, color);
    painter.circle_filled(up + egui::vec2(lobe * 0.85, 0.0), lobe, color);
    painter.add(egui::Shape::convex_polygon(
        vec![
            up - egui::vec2(lobe * 1.6, -lobe * 0.35),
            up + egui::vec2(lobe * 1.6, lobe * 0.35),
            center + egui::vec2(0.0, radius),
        ],
        color,
        egui::Stroke::NONE,
    ));
}

fn albums_tab(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(Type::H4.rich(
            "Hear your own albums and the albums you've liked:",
            app.theme.text,
        ));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.filter)
                    .hint_text("Filter")
                    .desired_width(200.0),
            );
        });
    });
    ui.add_space(Metrics::SP_1);
    // An album is a playlist with `is_album` set — SoundCloud has no separate
    // album route, so the same list is filtered.
    let owned_slot = app.playlists(Key::MyPlaylists);
    let liked_slot = app.playlists(Key::LikedPlaylists);
    if !matches!(owned_slot, crate::store::Slot::Ready(_)) {
        data::placeholder(app, ui, &owned_slot, "");
        return;
    }
    if !matches!(liked_slot, crate::store::Slot::Ready(_)) {
        data::placeholder(app, ui, &liked_slot, "");
        return;
    }
    let mut playlists = owned_slot.rows().to_vec();
    for playlist in liked_slot.rows() {
        if !playlists.iter().any(|existing| existing.id == playlist.id) {
            playlists.push(playlist.clone());
        }
    }
    let q = app.filter.trim().to_lowercase();
    let mut items: Vec<PlaylistCardItem> = Vec::new();
    for pl in &playlists {
        if !pl.is_album() || !(q.is_empty() || pl.title.to_lowercase().contains(&q)) {
            continue;
        }
        let mut sub = pl
            .user
            .as_ref()
            .map(|u| u.username.clone())
            .unwrap_or_default();
        if let Some(year) = pl.created_at.as_deref().and_then(|s| s.get(0..4)) {
            if !sub.is_empty() {
                sub.push_str(" · ");
            }
            sub.push_str(year);
        }
        if sub.is_empty() {
            sub = "Album".to_owned();
        }
        items.push((
            pl.id,
            pl.title.clone(),
            sub,
            pl.id + 7,
            false,
            pl.artwork_url().map(str::to_owned),
        ));
    }
    if items.is_empty() {
        ui.label(Type::BODY.rich("No albums yet.", app.theme.text_dim));
        footer(app, ui);
        return;
    }
    playlist_card_grid(app, ui, &items);
}

fn artists_tab(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(Type::H4.rich(
            "Hear what the people you follow have posted:",
            app.theme.text,
        ));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.filter)
                    .hint_text("Filter")
                    .desired_width(200.0),
            );
        });
    });
    ui.add_space(Metrics::SP_1);
    let slot = app.users(Key::Following);
    if data::placeholder(
        app,
        ui,
        &slot,
        "You're not following anyone yet — find artists on Home or Feed.",
    ) {
        return;
    }
    let q = app.filter.trim().to_lowercase();
    // In demo mode the followed set is local; live, the endpoint is the truth.
    let artists: Vec<crate::api::models::User> = slot
        .rows()
        .iter()
        .filter(|u| !app.demo || app.is_following(u.id))
        .filter(|u| q.is_empty() || u.username.to_lowercase().contains(&q))
        .cloned()
        .collect();
    if artists.is_empty() {
        ui.label(Type::BODY.rich(
            "Nobody matches — or you're not following anyone yet.",
            app.theme.text_dim,
        ));
        return;
    }
    // SoundCloud's library uses fixed rows of six followed artists. Keeping
    // the row boundary explicit also prevents a seventh narrow card from
    // appearing on unusually wide windows.
    let columns = library_columns(ui.available_width());
    for (row_index, row) in artists.chunks(columns).enumerate() {
        ui.push_id(("following-row", row_index), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(Metrics::SP_3, Metrics::SP_6);
                for artist in row {
                    artist_card(app, ui, artist, LIBRARY_CARD);
                }
            });
        });
        ui.add_space(Metrics::SP_3);
    }
    footer(app, ui);
}

/// Stations: seeded from what you play, native (no browser trip).
///
/// SoundCloud has no station route at all. One is a seed track plus
/// `/tracks/{urn}/related` with autoplay on, which is what
/// [`App::station_queue`] assembles.
fn stations_tab(app: &mut App, ui: &mut egui::Ui) {
    if !app.demo {
        ui.add_space(Metrics::SP_5);
        ui.vertical_centered(|ui| {
            ui.label(Type::H2.rich("No saved stations", app.theme.text));
            ui.label(Type::BODY.rich(
                "Start a station from a track's menu. FastCloud will not invent library items.",
                app.theme.text_dim,
            ));
        });
        footer(app, ui);
        return;
    }
    ui.label(Type::BODY.rich(
        "Stations play on from a track, like a radio.",
        app.theme.text_dim,
    ));
    ui.add_space(Metrics::SP_1);

    // Seeds: recently played first, then likes.
    let mut seeds = recent_tracks(app, 6);
    if seeds.is_empty() {
        seeds = app
            .tracks(Key::Likes)
            .rows()
            .iter()
            .take(6)
            .cloned()
            .collect();
    }
    if seeds.is_empty() {
        ui.add_space(Metrics::SP_5);
        ui.vertical_centered(|ui| {
            ui.label(Type::H2.rich("Play something to build a station", app.theme.text));
            ui.label(Type::BODY.rich("Stations follow what you listen to.", app.theme.text_dim));
        });
        footer(app, ui);
        return;
    }

    let shared = std::sync::Arc::new(seeds);
    app.carousel(ui, "stations", |app, ui| {
        for seed in shared.iter() {
            let art = seed.artwork_url();
            let click = widgets::card(
                app,
                ui,
                widgets::Card::plays(seed.id, &format!("{} Station", seed.title), seed.artist())
                    .art(art)
                    .artist(seed.user.as_ref().map(|user| user.id)),
            );
            if click.play {
                // A station starts here and keeps going: autoplay on.
                app.settings.autoplay = true;
                app.player.set_autoplay(true);
                let queue = app.station_queue(seed);
                let title = seed.title.clone();
                app.play_user_queue(queue, 0, false);
                app.toast(format!("Station: {title}"));
            }
        }
    });
    footer(app, ui);
}

fn footer(_app: &App, _ui: &mut egui::Ui) {}

/// Shared playlist/album card grid used by the Playlists and Albums tabs.
/// Items are `(id, title, subtitle, art seed, liked-heart art)`.
fn playlist_card_grid(app: &mut App, ui: &mut egui::Ui, items: &[PlaylistCardItem]) {
    let columns = library_columns(ui.available_width());
    for (row_index, row) in items.chunks(columns).enumerate() {
        ui.push_id(("playlist-row", row_index), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(Metrics::SP_3, Metrics::SP_6);
                for (pid, title, sub, seed, heart, art) in row {
                    let pid = *pid;
                    let title = title.clone();
                    let sub = sub.clone();
                    let seed = *seed;
                    let heart = *heart;
                    let art = art.clone();
                    let tracks = playlist_tracks(app, pid);
                    let art = art.or_else(|| {
                        tracks
                            .first()
                            .and_then(|track| track.artwork_url())
                            .map(str::to_owned)
                    });
                    let card = |ui: &mut egui::Ui| {
                        let mut open = false;
                        let mut play = false;
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.set_min_width(LIBRARY_CARD);
                            ui.set_max_width(LIBRARY_CARD);
                            let art_response = if heart {
                                liked_art(app, ui, LIBRARY_CARD)
                            } else {
                                widgets::artwork_img(
                                    ui,
                                    art.as_deref(),
                                    seed,
                                    &title,
                                    LIBRARY_CARD,
                                    Metrics::RADIUS as f32,
                                )
                            };
                            let play_radius = (LIBRARY_CARD * 0.2).clamp(14.0, 30.0);
                            if art_response.hovered() {
                                let center = art_response.rect.center();
                                ui.painter()
                                    .circle_filled(center, play_radius, app.theme.accent);
                                let s = play_radius * 0.55;
                                ui.painter().add(egui::Shape::convex_polygon(
                                    vec![
                                        center + egui::vec2(-s * 0.6, -s),
                                        center + egui::vec2(-s * 0.6, s),
                                        center + egui::vec2(s * 0.8, 0.0),
                                    ],
                                    egui::Color32::WHITE,
                                    egui::Stroke::NONE,
                                ));
                            }
                            if art_response.clicked() {
                                let hit_play =
                                    art_response.interact_pointer_pos().is_some_and(|pos| {
                                        pos.distance(art_response.rect.center()) <= play_radius
                                    });
                                if hit_play && !tracks.is_empty() {
                                    play = true;
                                } else {
                                    open = true;
                                }
                            }
                            let label = if heart {
                                egui::RichText::new(format!("♥ {title}"))
                            } else {
                                egui::RichText::new(&title)
                            };
                            if ui
                                .add(
                                    egui::Label::new(
                                        label.font(Type::H4.font()).color(app.theme.text),
                                    )
                                    .sense(egui::Sense::click())
                                    .truncate(),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                open = true;
                            }
                            ui.label(
                                egui::RichText::new(&sub)
                                    .font(Type::CAPTION.font())
                                    .color(app.theme.text_dim),
                            );
                        });
                        (open, play)
                    };
                    let (open, play) = card(ui);
                    if play && !tracks.is_empty() {
                        app.play_user_queue(tracks, 0, false);
                    } else if open {
                        app.navigate(Route::PlaylistDetail(pid));
                    }
                }
            });
        });
    }
}

fn track_detail(app: &mut App, ui: &mut egui::Ui, id: u64) {
    let slot = app.track(id);
    if data::placeholder_one(app, ui, &slot) {
        return;
    }
    let Some(track) = slot.ready() else {
        return;
    };
    let dur = track.effective_duration_ms();
    // Live position when this is the current track.
    let (pos, is_current) = {
        let st = app.player.state.lock();
        let cur = st.current.and_then(|i| st.queue.get(i).cloned());
        match cur {
            Some(t) if t.id == id => (st.position_ms, true),
            _ => (0, false),
        }
    };
    // "More like this" doubles as the play queue, so pressing play here
    // continues into related tracks rather than stopping after one song.
    let related: Vec<Track> = app
        .tracks(Key::Related(id))
        .rows()
        .iter()
        .filter(|t| t.id != id)
        .cloned()
        .collect();
    let queue = {
        let mut queue = vec![track.clone()];
        queue.extend(related.iter().cloned());
        queue
    };
    let is_own_track = app.account().is_some_and(|account| {
        track
            .user
            .as_ref()
            .is_some_and(|user| user.id == account.id)
    });

    ui.add_space(Metrics::SP_1);
    ui.horizontal(|ui| {
        let art = track.artwork_url();
        widgets::artwork_img(
            ui,
            art,
            track.id,
            &track.title,
            Metrics::artwork(170.0),
            Metrics::RADIUS_LG as f32,
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(Type::H1.rich(&crate::bidi::owned(&track.title), app.theme.text));
            if ui
                .add(
                    egui::Label::new(
                        Type::BODY.rich(&crate::bidi::owned(track.artist()), app.theme.text_dim),
                    )
                    .sense(egui::Sense::click()),
                )
                .on_hover_text("Open artist")
                .clicked()
                && let Some(u) = &track.user
            {
                app.navigate(Route::UserDetail(u.id));
            }
            ui.add_space(Metrics::SP_HALF);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = Metrics::SP_15;
                if let Some(badge) = track.access_badge() {
                    ui.label(Type::H5.rich(badge, app.theme.accent));
                }
                stat(app, ui, super::icons::Icon::Music, track.playback_count);
                stat(app, ui, super::icons::Icon::Heart, track.likes_count);
            });
            ui.add_space(Metrics::SP_1);
            ui.horizontal(|ui| {
                if ui.button("Play").clicked() {
                    app.play_user_queue(queue.clone(), 0, false);
                }
                widgets::like_button(app, ui, track.id, &track.title);
                let reposted = app
                    .tracks(Key::MyRepostedTracks)
                    .rows()
                    .iter()
                    .any(|item| item.id == track.id);
                if ui
                    .button(if reposted { "Unrepost" } else { "Repost" })
                    .clicked()
                {
                    app.set_track_reposted(track.id, !reposted);
                }
                if ui.button("Station").clicked() {
                    app.settings.autoplay = true;
                    app.player.set_autoplay(true);
                    let station = app.station_queue(&track);
                    app.play_user_queue(station, 0, false);
                    app.toast(format!("Station: {}", track.title));
                }
                if let Some(url) = track.permalink_url.clone() {
                    if ui.button("Share").clicked() {
                        ui.ctx().copy_text(url);
                        app.toast("Link copied");
                    }
                } else if ui.button("Share").clicked() {
                    app.toast("This track has no public link");
                }
            });
        });
    });

    if is_own_track {
        creator_track_tools(app, ui, &track);
    } else if let Some(uploader) = &track.user
        && uploader.username.trim() != track.artist().trim()
    {
        ui.horizontal(|ui| {
            ui.label(Type::CAPTION.rich("Uploaded by", app.theme.text_dim));
            if ui
                .add(
                    egui::Label::new(Type::H5.rich(&uploader.username, app.theme.text_dim))
                        .sense(egui::Sense::click()),
                )
                .clicked()
            {
                app.navigate(Route::UserDetail(uploader.id));
            }
            ui.label(Type::CAPTION.rich("on SoundCloud", app.theme.text_dim));
        });
    }

    // Real SoundCloud waveform. Comment avatars are intentionally omitted:
    // they obscured the waveform and comments are not part of this client UI.
    ui.add_space(Metrics::SP_1);
    let peaks = track
        .waveform_url
        .as_deref()
        .and_then(|url| app.waveforms.get(ui.ctx(), url));
    if let Some(frac) = widgets::waveform(app, ui, id, pos, dur, 110.0, 140, peaks.as_deref()) {
        if is_current {
            app.player.seek_ms((frac as f64 * dur as f64) as u64);
        } else {
            app.play_user_queue(queue.clone(), 0, false);
        }
    }
    ui.horizontal(|ui| {
        let elapsed = if is_current { pos } else { 0 };
        ui.label(
            egui::RichText::new(crate::util::fmt_duration_ms(elapsed))
                .font(egui::FontId::monospace(Type::CAPTION.size))
                .color(app.theme.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(crate::util::fmt_duration_ms(dur))
                    .font(egui::FontId::monospace(Type::CAPTION.size))
                    .color(app.theme.text_dim),
            );
        });
    });

    people_carousel(
        app,
        ui,
        "Liked by",
        "track-favoriters",
        Key::TrackFavoriters(id),
    );
    people_carousel(
        app,
        ui,
        "Reposted by",
        "track-reposters",
        Key::TrackReposters(id),
    );

    if !related.is_empty() {
        widgets::section_header(app, ui, "More like this");
        track_rows(app, ui, &related);
    }
}

fn creator_track_tools(app: &mut App, ui: &mut egui::Ui, track: &Track) {
    ui.add_space(Metrics::SP_1);
    ui.horizontal(|ui| {
        if ui.button("Edit track").clicked() {
            app.begin_track_edit(track);
        }
        if ui.button("Storefront").clicked() {
            app.creator.storefront_track_id = Some(track.id);
            app.creator.storefront_title = format!("Get {}", track.title);
            app.creator.storefront_kind = "digital".to_owned();
            app.creator.storefront_link = track.permalink_url.clone().unwrap_or_default();
        }
        if app.creator.confirm_delete_track == Some(track.id) {
            if ui.button("Confirm permanent deletion").clicked() {
                app.delete_track(track.id);
            }
            if ui.button("Cancel").clicked() {
                app.creator.confirm_delete_track = None;
            }
        } else if ui.button("Delete track").clicked() {
            app.creator.confirm_delete_track = Some(track.id);
        }
    });

    if app.creator.edit_track_id == Some(track.id) {
        egui::Frame::new()
            .fill(app.theme.surface)
            .corner_radius(Metrics::RADIUS)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.label(Type::H4.rich("Track metadata", app.theme.text));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut app.creator.edit_title)
                            .hint_text("Title")
                            .desired_width(320.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut app.creator.edit_artist)
                            .hint_text("Artist")
                            .desired_width(260.0),
                    );
                });
                ui.add(
                    egui::TextEdit::multiline(&mut app.creator.edit_description)
                        .hint_text("Description")
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    if ui.button("Save on SoundCloud").clicked() {
                        app.save_track_edit(track.id);
                    }
                    if ui.button("Cancel").clicked() {
                        app.creator.edit_track_id = None;
                    }
                });
            });
    }

    if app.creator.storefront_track_id == Some(track.id) {
        egui::Frame::new()
            .fill(app.theme.surface)
            .corner_radius(Metrics::RADIUS)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.label(Type::H4.rich("Artist Storefront", app.theme.text));
                ui.label(Type::CAPTION.rich(
                    "This replaces the whole storefront module on SoundCloud.",
                    app.theme.text_dim,
                ));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut app.creator.storefront_title)
                            .hint_text("Card title")
                            .desired_width(260.0),
                    );
                    egui::ComboBox::from_id_salt(("storefront-kind", track.id))
                        .selected_text(&app.creator.storefront_kind)
                        .show_ui(ui, |ui| {
                            for kind in [
                                "digital",
                                "vinyl",
                                "cd",
                                "cassette",
                                "apparel",
                                "sample_pack",
                                "subscription",
                                "live_event",
                                "live_stream",
                                "other",
                            ] {
                                ui.selectable_value(
                                    &mut app.creator.storefront_kind,
                                    kind.to_owned(),
                                    kind,
                                );
                            }
                        });
                });
                ui.add(
                    egui::TextEdit::singleline(&mut app.creator.storefront_link)
                        .hint_text("https://…")
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut app.creator.storefront_link_title)
                            .hint_text("Button label")
                            .desired_width(220.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut app.creator.storefront_price)
                            .hint_text("Display price")
                            .desired_width(160.0),
                    );
                });
                ui.add(
                    egui::TextEdit::multiline(&mut app.creator.storefront_description)
                        .hint_text("Storefront description")
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    if ui.button("Save storefront").clicked() {
                        app.save_storefront(track.id);
                    }
                    if ui.button("Cancel").clicked() {
                        app.creator.storefront_track_id = None;
                    }
                });
            });
    }
}

/// An icon plus a count, or nothing when the API did not say.
fn stat(app: &App, ui: &mut egui::Ui, icon: super::icons::Icon, count: Option<u64>) {
    let Some(count) = count.filter(|n| *n > 0) else {
        return;
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        super::icons::show_static(ui, icon, 11.0, app.theme.text_dim);
        ui.label(Type::CAPTION.rich(&crate::util::play_count(count), app.theme.text_dim));
    });
}

fn playlist_detail(app: &mut App, ui: &mut egui::Ui, id: u64) {
    if app.can_go_back()
        && ui
            .button("← Back")
            .on_hover_text("Return to the previous page")
            .clicked()
    {
        app.go_back();
        return;
    }
    ui.add_space(Metrics::SP_1);
    // Three kinds of playlist: our own local ones, our "Liked Songs" (which
    // is not a playlist on SoundCloud at all, only `/me/likes/tracks`), and a
    // real one from the API.
    let local = app
        .demo
        .then(|| app.settings.custom_playlists.iter().find(|p| p.id == id))
        .flatten();
    let is_custom = local.is_some();
    let remote = (!is_custom && id != 0).then(|| app.playlist(id));
    let remote_playlist = remote.as_ref().and_then(|slot| slot.clone().ready());
    if let Some(slot) = &remote
        && data::placeholder_one(app, ui, slot)
    {
        return;
    }
    let title = if app.demo && id == 0 {
        "Liked Songs".to_owned()
    } else if let Some(p) = local {
        p.title.clone()
    } else {
        remote
            .as_ref()
            .and_then(|s| s.clone().ready())
            .map(|p| p.title)
            .unwrap_or_else(|| format!("Playlist {id}"))
    };
    let owner = remote_playlist
        .clone()
        .and_then(|p| p.user)
        .map(|u| u.username);
    let art = remote_playlist
        .as_ref()
        .and_then(|playlist| playlist.artwork_url().map(str::to_owned));
    let owned_by_me = app.account().is_some_and(|me| {
        remote_playlist
            .as_ref()
            .and_then(|playlist| playlist.user.as_ref())
            .is_some_and(|owner| owner.id == me.id)
    });

    let tracks = playlist_tracks(app, id);
    let total_ms: u64 = tracks.iter().map(|t| t.effective_duration_ms()).sum();
    let total_likes: u64 = tracks.iter().filter_map(|t| t.likes_count).sum();
    let total_plays: u64 = tracks.iter().filter_map(|t| t.playback_count).sum();

    // Tinted banner like soundcloud.com's playlist headers: the artwork's
    // hue darkened towards `--background-dark-color`, with light ink on it
    // whichever theme is running.
    let base = widgets::art_color(id + 7);
    let banner = egui::Color32::from_rgb(base.r() / 3 + 12, base.g() / 3 + 12, base.b() / 3 + 12);
    let paper = super::theme::Palette::DARK.primary;
    let paper_dim = super::theme::Palette::DARK.secondary;
    egui::Frame::new()
        .fill(banner)
        .inner_margin(egui::Margin::symmetric(
            Metrics::SP_2 as i8,
            Metrics::SP_175 as i8,
        ))
        .corner_radius(Metrics::RADIUS_LG)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Big round play button.
                if round_play_button(app, ui, 56.0) && !tracks.is_empty() {
                    app.play_user_queue(tracks.clone(), 0, false);
                }
                ui.vertical(|ui| {
                    let kind = match owner.as_deref() {
                        Some(who) => format!("Playlist · {who}"),
                        None => "Playlist".to_owned(),
                    };
                    ui.label(Type::CAPTION.rich(&kind, paper_dim));
                    ui.label(Type::H1.rich(&title, paper));
                    ui.label(Type::CAPTION.rich(
                        &format!(
                            "{} tracks • {}",
                            tracks.len(),
                            crate::util::fmt_duration_ms(total_ms)
                        ),
                        paper_dim,
                    ));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::artwork_img(
                        ui,
                        art.as_deref(),
                        id + 7,
                        &title,
                        Metrics::artwork(140.0),
                        Metrics::RADIUS_LG as f32,
                    );
                });
            });
        });

    ui.add_space(Metrics::SP_1);
    // Action row + stats.
    ui.horizontal(|ui| {
        let mut deleted = false;
        if ui.button("Play all").clicked() && !tracks.is_empty() {
            app.play_user_queue(tracks.clone(), 0, false);
        }
        if ui.button("Queue all").clicked() && !tracks.is_empty() {
            app.player.enqueue(tracks.clone(), false);
            app.toast(format!("Queued {} tracks", tracks.len()));
        }
        let liked = app
            .playlists(Key::LikedPlaylists)
            .rows()
            .iter()
            .any(|playlist| playlist.id == id);
        if id != 0 && ui.button(if liked { "Unlike" } else { "Like" }).clicked() {
            app.set_playlist_liked(id, !liked);
        }
        let reposted = app
            .playlists(Key::MyRepostedPlaylists)
            .rows()
            .iter()
            .any(|playlist| playlist.id == id);
        if id != 0
            && ui
                .button(if reposted { "Unrepost" } else { "Repost" })
                .clicked()
        {
            app.set_playlist_reposted(id, !reposted);
        }
        if ui.button("Share").clicked() {
            match remote
                .as_ref()
                .and_then(|s| s.clone().ready())
                .and_then(|p| p.permalink_url)
            {
                Some(url) => {
                    ui.ctx().copy_text(url);
                    app.toast("Link copied");
                }
                None => app.toast("This playlist has no public link"),
            }
        }
        if is_custom && ui.button("Delete").clicked() {
            app.settings.custom_playlists.retain(|p| p.id != id);
            let _ = app.settings.save();
            app.toast(format!("Deleted {title}"));
            deleted = true;
        } else if owned_by_me {
            if app.creator.confirm_delete_playlist == Some(id) {
                if ui.button("Confirm permanent deletion").clicked() {
                    app.delete_playlist(id);
                    app.toast("Deleting playlist…");
                }
                if ui.button("Cancel").clicked() {
                    app.creator.confirm_delete_playlist = None;
                }
            } else if ui
                .button("Delete from SoundCloud")
                .on_hover_text("Permanently deletes this playlist from your account")
                .clicked()
            {
                app.creator.confirm_delete_playlist = Some(id);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            stat(app, ui, super::icons::Icon::Music, Some(total_plays));
            stat(app, ui, super::icons::Icon::Heart, Some(total_likes));
        });
        if deleted {
            app.navigate(Route::Library);
        }
    });
    if app.route == Route::Library {
        return;
    }
    ui.add_space(Metrics::SP_1);
    if tracks.is_empty() {
        ui.label(Type::BODY.rich("This playlist is empty.", app.theme.text_dim));
    } else {
        playlist_rows(app, ui, id, &tracks);
    }
    if id != 0 {
        people_carousel(
            app,
            ui,
            "Reposted by",
            "playlist-reposters",
            Key::PlaylistReposters(id),
        );
    }
}

fn people_carousel(app: &mut App, ui: &mut egui::Ui, title: &str, salt: &str, key: Key) {
    let users = app.users(key);
    if users.rows().is_empty() {
        return;
    }
    widgets::section_header(app, ui, title);
    let users = users.rows().to_vec();
    app.carousel_sized(ui, salt, Some(AVATAR_CARD), |app, ui| {
        for user in &users {
            artist_card(app, ui, user, AVATAR_CARD);
        }
    });
}

fn user_detail(app: &mut App, ui: &mut egui::Ui, id: u64) {
    let slot = app.user(id);
    if data::placeholder_one(app, ui, &slot) {
        return;
    }
    let Some(user) = slot.ready() else {
        return;
    };
    let name = user.username.clone();
    profile_hero(app, ui, &user);
    let profiles = app.store.web_profiles(Key::UserWebProfiles(id));
    if let Slot::Ready(rows) = profiles
        && !rows.is_empty()
    {
        ui.horizontal_wrapped(|ui| {
            ui.label(Type::CAPTION.rich("Elsewhere:", app.theme.text_dim));
            for profile in rows.iter().filter(|profile| !profile.url.is_empty()) {
                let label = profile
                    .title
                    .as_deref()
                    .or(profile.service.as_deref())
                    .unwrap_or("Website");
                ui.hyperlink_to(label, &profile.url);
            }
        });
    }
    ui.add_space(Metrics::SP_15);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = Metrics::SP_2;
        for (label, tab) in [
            ("All", UserTab::All),
            ("Popular tracks", UserTab::PopularTracks),
            ("Tracks", UserTab::Tracks),
            ("Albums", UserTab::Albums),
            ("Playlists", UserTab::Playlists),
            ("Reposts", UserTab::Reposts),
        ] {
            if profile_tab(app, ui, label, app.user_tab == tab) {
                app.user_tab = tab;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Station").clicked() {
                match app.tracks(Key::UserTracks(id)).rows().first().cloned() {
                    Some(seed) => {
                        app.settings.autoplay = true;
                        app.player.set_autoplay(true);
                        let queue = app.station_queue(&seed);
                        app.play_user_queue(queue, 0, false);
                        app.toast(format!("Station: {name}"));
                    }
                    None => app.toast("Nothing to seed a station with yet"),
                }
            }
            widgets::follow_button(app, ui, id, &name);
        });
    });
    ui.separator();

    if matches!(
        app.user_tab,
        UserTab::All | UserTab::PopularTracks | UserTab::Tracks
    ) {
        let tracks = app.tracks(Key::UserTracks(id));
        if !data::placeholder(app, ui, &tracks, "No public tracks.") {
            let mut rows = tracks.rows().to_vec();
            let heading = match app.user_tab {
                UserTab::PopularTracks => {
                    rows.sort_by_key(|track| std::cmp::Reverse(track.playback_count.unwrap_or(0)));
                    "Popular tracks"
                }
                UserTab::Tracks => "Tracks",
                _ => {
                    rows.truncate(3);
                    "Recent"
                }
            };
            widgets::section_header(app, ui, heading);
            for track in &rows {
                feed_card(app, ui, track, None, track.created_at.clone());
                ui.add_space(Metrics::SP_175);
            }
        }
    }

    let playlists = app.playlists(Key::UserPlaylists(id));
    if let Slot::Ready(rows) = &playlists
        && !rows.is_empty()
        && matches!(
            app.user_tab,
            UserTab::All | UserTab::Albums | UserTab::Playlists
        )
    {
        let sections: Vec<(&str, Vec<_>)> = match app.user_tab {
            UserTab::Albums => vec![(
                "Albums",
                rows.iter()
                    .filter(|playlist| playlist.is_album())
                    .cloned()
                    .collect(),
            )],
            UserTab::Playlists => vec![(
                "Playlists",
                rows.iter()
                    .filter(|playlist| !playlist.is_album())
                    .cloned()
                    .collect(),
            )],
            _ => vec![
                (
                    "Albums",
                    rows.iter()
                        .filter(|playlist| playlist.is_album())
                        .cloned()
                        .collect(),
                ),
                (
                    "Playlists",
                    rows.iter()
                        .filter(|playlist| !playlist.is_album())
                        .cloned()
                        .collect(),
                ),
            ],
        };
        for (heading, section) in sections {
            if section.is_empty() {
                continue;
            }
            widgets::section_header(app, ui, heading);
            playlist_carousel(
                app,
                ui,
                &format!("user-{}", heading.to_ascii_lowercase()),
                &section,
            );
        }
    }
    if matches!(app.user_tab, UserTab::All | UserTab::Reposts) {
        let reposts = app.tracks(Key::UserRepostedTracks(id));
        if !reposts.rows().is_empty() {
            widgets::section_header(app, ui, &format!("{name}'s reposts"));
            track_cards(app, ui, "user-reposts", reposts.rows());
        }
        let reposted_playlists = app.playlists(Key::UserRepostedPlaylists(id));
        if !reposted_playlists.rows().is_empty() {
            widgets::section_header(app, ui, "Reposted playlists");
            playlist_carousel(
                app,
                ui,
                "user-reposted-playlists",
                reposted_playlists.rows(),
            );
        }
    }
    footer(app, ui);
}

fn playlist_carousel(
    app: &mut App,
    ui: &mut egui::Ui,
    salt: &str,
    playlists: &[crate::api::models::Playlist],
) {
    let playlists = playlists.to_vec();
    app.carousel(ui, salt, |app, ui| {
        for playlist in &playlists {
            let subtitle = format!(
                "{} • {} tracks",
                playlist
                    .user
                    .as_ref()
                    .map(|user| user.username.as_str())
                    .unwrap_or("Unknown creator"),
                playlist.track_count.unwrap_or(0)
            );
            let click = widgets::card(
                app,
                ui,
                widgets::Card::opens(playlist.id, &playlist.title, &subtitle)
                    .art(playlist.artwork_url()),
            );
            if click.play {
                let tracks = if playlist.tracks.is_empty() {
                    playlist_tracks(app, playlist.id)
                } else {
                    playlist.tracks.clone()
                };
                if !tracks.is_empty() {
                    app.play_user_queue(tracks, 0, false);
                }
            } else if click.open {
                app.navigate(Route::PlaylistDetail(playlist.id));
            }
        }
    });
}

/// SoundCloud profile masthead: a tall colour field, large circular avatar,
/// and the account name on a dark label over it.
fn profile_hero(app: &App, ui: &mut egui::Ui, user: &crate::api::models::User) {
    let size = egui::vec2(ui.available_width(), 240.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let base = widgets::art_color(name_seed(&user.username));
    let dark = base.gamma_multiply(0.45);
    let steps = 32;
    for step in 0..steps {
        let t = step as f32 / (steps - 1) as f32;
        let colour = egui::Color32::from_rgb(
            (base.r() as f32 * (1.0 - t) + dark.r() as f32 * t) as u8,
            (base.g() as f32 * (1.0 - t) + dark.g() as f32 * t) as u8,
            (base.b() as f32 * (1.0 - t) + dark.b() as f32 * t) as u8,
        );
        let x0 = rect.left() + rect.width() * step as f32 / steps as f32;
        let x1 = rect.left() + rect.width() * (step + 1) as f32 / steps as f32 + 1.0;
        ui.painter().rect_filled(
            egui::Rect::from_min_max(egui::pos2(x0, rect.top()), egui::pos2(x1, rect.bottom())),
            0.0,
            colour,
        );
    }

    let avatar = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(Metrics::SP_3, 32.0),
        egui::vec2(176.0, 176.0),
    );
    let mut painted = false;
    if let Some(url) = user.avatar_url.as_deref().filter(|url| !url.is_empty()) {
        let image = egui::Image::new(url)
            .show_loading_spinner(false)
            .fit_to_exact_size(avatar.size())
            .corner_radius(avatar.width() / 2.0);
        if image.load_for_size(ui.ctx(), avatar.size()).is_ok() {
            image.paint_at(ui, avatar);
            painted = true;
        }
    }
    if !painted {
        paint_initial(ui, avatar, &user.username, avatar.width());
    }
    ui.painter().circle_stroke(
        avatar.center(),
        avatar.width() / 2.0,
        egui::Stroke::new(3.0, app.theme.bg),
    );

    let name_pos = egui::pos2(avatar.right() + Metrics::SP_3, rect.top() + 64.0);
    let galley = ui.painter().layout_no_wrap(
        user.username.clone(),
        Type::DISPLAY3.font(),
        egui::Color32::WHITE,
    );
    let name_bg = egui::Rect::from_min_size(
        name_pos - egui::vec2(10.0, 7.0),
        galley.size() + egui::vec2(20.0, 14.0),
    );
    ui.painter()
        .rect_filled(name_bg, 0.0, egui::Color32::from_black_alpha(210));
    ui.painter().galley(name_pos, galley, egui::Color32::WHITE);

    let stats = format!(
        "{} followers  ·  {} following  ·  {} tracks",
        fmt_count(user.followers_count),
        fmt_count(user.followings_count),
        fmt_count(user.track_count),
    );
    ui.painter().text(
        egui::pos2(name_bg.left(), name_bg.bottom() + Metrics::SP_15),
        egui::Align2::LEFT_TOP,
        stats,
        Type::BODY.font(),
        egui::Color32::WHITE,
    );
}

fn profile_tab(app: &App, ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let colour = if active {
        app.theme.accent
    } else {
        app.theme.text
    };
    let response = ui
        .add(egui::Label::new(Type::H4.rich(label, colour)).sense(egui::Sense::click()))
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if active {
        ui.painter().hline(
            response.rect.x_range(),
            response.rect.bottom() + 4.0,
            egui::Stroke::new(2.0, app.theme.accent),
        );
    }
    response.clicked()
}

// ===== Search / recent / settings =====

/// SoundCloud-style search: a persistent filter rail and one roomy result
/// column. The API still has three separate routes, so albums are split from
/// the playlist response by `Playlist::is_album` after it arrives.
fn search(app: &mut App, ui: &mut egui::Ui) {
    let q = app.search_query.trim().to_owned();
    let title = if q.is_empty() {
        "Search results".to_owned()
    } else {
        format!("Search results for “{q}”")
    };
    page_title(app, ui, &title, None);
    if q.is_empty() {
        ui.label(Type::BODY.rich(
            "Type in the search bar above to find music.",
            app.theme.text_dim,
        ));
        return;
    }

    let playlists = app.playlists(Key::SearchPlaylists(q.clone()));
    let users = app.users(Key::SearchUsers(q.clone()));
    let tracks = app.tracks(Key::SearchTracks(q.clone()));

    let rows = tracks.rows().to_vec();
    let mut people = users.rows().to_vec();
    let sets = playlists.rows().to_vec();
    people.sort_by(|left, right| {
        let left_exact = left.username.eq_ignore_ascii_case(q.trim());
        let right_exact = right.username.eq_ignore_ascii_case(q.trim());
        right_exact
            .cmp(&left_exact)
            .then_with(|| right.followers_count.cmp(&left.followers_count))
            .then_with(|| {
                left.username
                    .to_lowercase()
                    .cmp(&right.username.to_lowercase())
            })
    });
    let albums: Vec<_> = sets.iter().filter(|set| set.is_album()).cloned().collect();
    let plain_playlists: Vec<_> = sets.iter().filter(|set| !set.is_album()).cloned().collect();

    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.vertical(|ui| {
            ui.set_width(224.0_f32.min(ui.available_width()));
            search_filter_rail(app, ui, &rows, &sets);
        });
        ui.add_space(Metrics::SP_3);
        ui.vertical(|ui| {
            ui.set_min_width(360.0);
            match app.search_tab {
                SearchTab::All => {
                    let mut kinds = Vec::new();
                    if !plain_playlists.is_empty() {
                        kinds.push(found_count(plain_playlists.len(), "playlist", "playlists"));
                    }
                    if !rows.is_empty() {
                        kinds.push(found_count(rows.len(), "track", "tracks"));
                    }
                    if !people.is_empty() {
                        kinds.push(found_count(people.len(), "person", "people"));
                    }
                    if !albums.is_empty() {
                        kinds.push(found_count(albums.len(), "album", "albums"));
                    }
                    search_found(app, ui, &kinds.join(", "));
                    if let Some(user) = people.first() {
                        search_person(app, ui, user, 144.0);
                        ui.add_space(Metrics::SP_4);
                    }
                    if let Some(album) = albums.first() {
                        search_set(app, ui, album);
                        ui.add_space(Metrics::SP_5);
                    }
                    for track in rows.iter().take(3) {
                        feed_card(
                            app,
                            ui,
                            track,
                            None,
                            search_stamp(track.created_at.as_deref()),
                        );
                        ui.add_space(Metrics::SP_5);
                    }
                    if albums.is_empty()
                        && let Some(playlist) = plain_playlists.first()
                    {
                        search_set(app, ui, playlist);
                    }
                }
                SearchTab::Tracks => {
                    search_found(app, ui, &found_count(rows.len(), "track", "tracks"));
                    for track in &rows {
                        feed_card(
                            app,
                            ui,
                            track,
                            None,
                            search_stamp(track.created_at.as_deref()),
                        );
                        ui.add_space(Metrics::SP_5);
                    }
                }
                SearchTab::People => {
                    search_found(app, ui, &found_count(people.len(), "person", "people"));
                    for user in &people {
                        search_person(app, ui, user, 144.0);
                        ui.add_space(Metrics::SP_5);
                    }
                }
                SearchTab::Albums => {
                    search_found(app, ui, &found_count(albums.len(), "album", "albums"));
                    for album in &albums {
                        search_set(app, ui, album);
                        ui.add_space(Metrics::SP_5);
                    }
                }
                SearchTab::Playlists => {
                    search_found(
                        app,
                        ui,
                        &found_count(plain_playlists.len(), "playlist", "playlists"),
                    );
                    for playlist in &plain_playlists {
                        search_set(app, ui, playlist);
                        ui.add_space(Metrics::SP_5);
                    }
                }
            }

            let selected_has_rows = match app.search_tab {
                SearchTab::All => !rows.is_empty() || !sets.is_empty() || !people.is_empty(),
                SearchTab::Tracks => !rows.is_empty(),
                SearchTab::People => !people.is_empty(),
                SearchTab::Albums => !albums.is_empty(),
                SearchTab::Playlists => !plain_playlists.is_empty(),
            };
            if !selected_has_rows {
                if tracks.is_loading() || playlists.is_loading() || users.is_loading() {
                    data::placeholder(app, ui, &tracks, "");
                } else if let Some(why) = tracks
                    .error()
                    .or_else(|| playlists.error())
                    .or_else(|| users.error())
                {
                    ui.label(Type::BODY.rich("Search failed.", app.theme.text));
                    ui.label(Type::CAPTION.rich(why, app.theme.text_dim));
                } else {
                    ui.label(Type::BODY.rich("No results.", app.theme.text_dim));
                }
            }
        });
    });
}

fn search_filter_rail(
    app: &mut App,
    ui: &mut egui::Ui,
    tracks: &[Track],
    playlists: &[crate::api::models::Playlist],
) {
    for (label, tab) in [
        ("Everything", SearchTab::All),
        ("Tracks", SearchTab::Tracks),
        ("People", SearchTab::People),
        ("Albums", SearchTab::Albums),
        ("Playlists", SearchTab::Playlists),
    ] {
        let active = app.search_tab == tab;
        let text = if active { app.theme.bg } else { app.theme.text };
        let fill = if active {
            app.theme.text
        } else {
            egui::Color32::TRANSPARENT
        };
        let response = ui.add_sized(
            [ui.available_width(), 28.0],
            egui::Button::new(Type::H4.rich(label, text))
                .fill(fill)
                .stroke(egui::Stroke::NONE)
                .corner_radius(Metrics::RADIUS),
        );
        if response.clicked() {
            app.search_tab = tab;
        }
    }

    ui.add_space(Metrics::SP_3);
    if app.search_tab == SearchTab::Tracks {
        ui.label(Type::H3.rich("Filter results", app.theme.text));
        ui.add_space(Metrics::SP_1);
        for label in ["Added any time", "Any length", "To listen to"] {
            ui.label(Type::H4.rich(label, app.theme.text));
            ui.add_space(Metrics::SP_1);
        }
        ui.add_space(Metrics::SP_2);
    }

    let mut tags: Vec<String> = tracks
        .iter()
        .filter_map(|track| track.genre.as_deref())
        .map(str::trim)
        .filter(|genre| !genre.is_empty())
        .map(str::to_owned)
        .collect();
    for playlist in playlists {
        for track in playlist.tracks.iter().take(3) {
            if let Some(genre) = track.genre.as_deref().map(str::trim)
                && !genre.is_empty()
            {
                tags.push(genre.to_owned());
            }
        }
    }
    tags.sort_by_key(|tag| tag.to_lowercase());
    tags.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    tags.truncate(9);
    if tags.is_empty() {
        tags.extend(
            [
                "Pop",
                "Alternative",
                "Hip hop",
                "Trap",
                "Electronic",
                "Ambient",
            ]
            .into_iter()
            .map(str::to_owned),
        );
    }
    ui.label(Type::H3.rich("Filter by tag", app.theme.text));
    ui.add_space(Metrics::SP_1);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(Metrics::SP_075, Metrics::SP_1);
        for tag in tags {
            let clicked = egui::Frame::new()
                .fill(app.theme.surface_hover)
                .corner_radius(Metrics::RADIUS_PILL)
                .inner_margin(egui::Margin::symmetric(9, 4))
                .show(ui, |ui| {
                    ui.label(Type::CAPTION.rich(&format!("# {tag}"), app.theme.text))
                })
                .response
                .interact(egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked();
            if clicked {
                app.search_query = tag.clone();
                app.navigate(Route::Search(tag));
            }
        }
    });
}

fn search_found(app: &App, ui: &mut egui::Ui, text: &str) {
    if !text.is_empty() {
        ui.label(Type::H4.rich(&format!("Found {text}"), app.theme.text_dim));
        ui.add_space(Metrics::SP_3);
    }
}

fn found_count(count: usize, one: &str, many: &str) -> String {
    let noun = if count == 1 { one } else { many };
    let plus = if count >= 20 { "+" } else { "" };
    format!("{count}{plus} {noun}")
}

fn search_person(app: &mut App, ui: &mut egui::Ui, user: &crate::api::models::User, size: f32) {
    ui.horizontal(|ui| {
        let avatar = user.avatar_url.as_deref().filter(|url| !url.is_empty());
        let clicked = match avatar {
            Some(url) => round_avatar(ui, url, &user.username, size).clicked(),
            None => avatar_circle(ui, &user.username, size).clicked(),
        };
        if clicked {
            app.navigate(Route::UserDetail(user.id));
        }
        ui.add_space(Metrics::SP_2);
        ui.vertical(|ui| {
            ui.add_space(size * 0.36);
            if ui
                .add(
                    egui::Label::new(Type::H4.rich(&user.username, app.theme.text))
                        .sense(egui::Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                app.navigate(Route::UserDetail(user.id));
            }
            stat(
                app,
                ui,
                super::icons::Icon::Users,
                Some(user.followers_count),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            widgets::follow_button(app, ui, user.id, &user.username);
        });
    });
}

fn search_set(app: &mut App, ui: &mut egui::Ui, set: &crate::api::models::Playlist) {
    let queue = if set.tracks.is_empty() {
        playlist_tracks(app, set.id)
    } else {
        set.tracks.clone()
    };
    let liked = app
        .playlists(Key::LikedPlaylists)
        .rows()
        .iter()
        .any(|playlist| playlist.id == set.id);
    let reposted = app
        .playlists(Key::MyRepostedPlaylists)
        .rows()
        .iter()
        .any(|playlist| playlist.id == set.id);
    ui.horizontal(|ui| {
        if widgets::artwork_img(ui, set.artwork_url(), set.id, &set.title, 160.0, 4.0).clicked() {
            app.navigate(Route::PlaylistDetail(set.id));
        }
        ui.add_space(Metrics::SP_2);
        ui.vertical(|ui| {
            ui.set_min_width(280.0);
            ui.horizontal(|ui| {
                if round_play_button(app, ui, 44.0) && !queue.is_empty() {
                    app.play_user_queue(queue.clone(), 0, false);
                }
                ui.vertical(|ui| {
                    let artist = set
                        .user
                        .as_ref()
                        .map(|user| user.username.as_str())
                        .unwrap_or("Unknown creator");
                    ui.label(Type::CAPTION.rich(artist, app.theme.text_dim));
                    let kind = if set.is_album() { "Album" } else { "Playlist" };
                    if ui
                        .add(
                            egui::Label::new(
                                Type::H4.rich(&format!("{}  {kind}", set.title), app.theme.text),
                            )
                            .sense(egui::Sense::click())
                            .truncate(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        app.navigate(Route::PlaylistDetail(set.id));
                    }
                });
                if let Some(stamp) = search_stamp(set.created_at.as_deref()) {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        ui.label(Type::CAPTION.rich(&stamp, app.theme.text_dim));
                    });
                }
            });

            let first = queue.first();
            let duration = first
                .map(Track::effective_duration_ms)
                .filter(|duration| *duration > 0)
                .or(set.duration_ms)
                .unwrap_or(0);
            let peaks = first
                .and_then(|track| track.waveform_url.as_deref())
                .and_then(|url| app.waveforms.get(ui.ctx(), url));
            if widgets::waveform(
                app,
                ui,
                set.id ^ 0x51_45_54,
                0,
                duration,
                72.0,
                110,
                peaks.as_deref(),
            )
            .is_some()
                && !queue.is_empty()
            {
                app.play_user_queue(queue.clone(), 0, false);
            }

            for (index, track) in queue.iter().take(5).enumerate() {
                let row = ui.horizontal(|ui| {
                    widgets::artwork_img(
                        ui,
                        track.artwork_url(),
                        track.id,
                        &track.title,
                        32.0,
                        0.0,
                    );
                    ui.label(Type::H4.rich(&format!("{}  ·", index + 1), app.theme.text_dim));
                    ui.add(
                        egui::Label::new(Type::H4.rich(&track.title, app.theme.text)).truncate(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(Type::CAPTION.rich(
                            &crate::util::play_count(track.playback_count.unwrap_or(0)),
                            app.theme.text_dim,
                        ));
                        super::icons::show_static(
                            ui,
                            super::icons::Icon::Play,
                            10.0,
                            app.theme.text_dim,
                        );
                    });
                });
                if row.response.interact(egui::Sense::click()).clicked() {
                    app.play_user_queue(queue.clone(), index, false);
                }
            }
            let count = set.track_count.unwrap_or(queue.len() as u64);
            ui.label(Type::H4.rich(&format!("View {count} tracks"), app.theme.text));
            ui.horizontal(|ui| {
                if action_button(
                    app,
                    ui,
                    super::icons::Icon::Heart,
                    "",
                    liked,
                    if liked { "Unlike" } else { "Like" },
                ) {
                    app.set_playlist_liked(set.id, !liked);
                }
                if action_button(
                    app,
                    ui,
                    super::icons::Icon::Repeat,
                    "",
                    reposted,
                    if reposted { "Unrepost" } else { "Repost" },
                ) {
                    app.set_playlist_reposted(set.id, !reposted);
                }
                if action_button(app, ui, super::icons::Icon::External, "", false, "Share") {
                    app.toast("Link copied");
                }
                if action_button(app, ui, super::icons::Icon::Copy, "", false, "Copy link") {
                    app.toast("Link copied");
                }
            });
        });
    });
}

fn search_stamp(created_at: Option<&str>) -> Option<String> {
    let value = created_at?.trim();
    if let Ok(at) = chrono::DateTime::parse_from_rfc3339(value) {
        let now = chrono::Utc::now().timestamp().max(0) as u64;
        return Some(super::fmt_relative(now, at.timestamp().max(0) as u64));
    }
    value.get(0..4).map(|year| year.to_owned())
}

fn recent(app: &mut App, ui: &mut egui::Ui) {
    history_tab(app, ui);
}

fn history_tab(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(Type::H4.rich("Recently played:", app.theme.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.filter)
                    .hint_text("Filter")
                    .desired_width(200.0),
            );
        });
    });
    ui.add_space(Metrics::SP_1);
    let tracks = filtered(app, &recent_tracks(app, 25));
    if tracks.is_empty() {
        // Distinguish "the endpoint has not answered" from "you have not
        // played anything": only the second is the user's problem.
        let slot = app.tracks(Key::History);
        if data::placeholder(
            app,
            ui,
            &slot,
            "Nothing here yet — press play on any track.",
        ) {
            if ui.button("Browse home").clicked() {
                app.navigate(Route::Home);
            }
            return;
        }
        ui.label(Type::BODY.rich("Nothing matches that filter.", app.theme.text_dim));
        return;
    }
    track_cards(
        app,
        ui,
        "history-recent",
        &tracks.iter().take(6).cloned().collect::<Vec<_>>(),
    );

    ui.add_space(Metrics::SP_HALF);
    ui.label(Type::H4.rich("Hear the tracks you've played:", app.theme.text));
    ui.add_space(Metrics::SP_1);
    let now = super::now_secs();
    for track in &tracks {
        let stamp = app
            .recent_at
            .get(&track.id)
            .map(|at| super::fmt_relative(now, *at));
        feed_card(app, ui, track, None, stamp);
        ui.add_space(14.0);
    }
}

fn settings(app: &mut App, ui: &mut egui::Ui) {
    page_title(app, ui, "Settings", None);

    account_section(app, ui);

    ui.add_space(Metrics::SP_2);
    ui.label(Type::H4.rich("Theme", app.theme.text));
    ui.horizontal(|ui| {
        let modes = [
            ("Dark", crate::config::ThemeMode::Dark),
            ("Light", crate::config::ThemeMode::Light),
            ("System", crate::config::ThemeMode::System),
        ];
        for (name, mode) in modes {
            if ui.selectable_label(app.theme_mode == mode, name).clicked() {
                app.theme_mode = mode;
                app.theme = crate::ui::theme::Theme::from_mode_ctx(mode, app.accent, ui.ctx());
                app.settings.theme = mode;
                if let Err(e) = app.settings.save() {
                    app.toast(format!("Failed to save: {e}"));
                }
            }
        }
    });

    ui.add_space(Metrics::SP_2);
    ui.label(Type::H4.rich("Appearance", app.theme.text));
    if ui
        .checkbox(&mut app.settings.compact_rows, "Compact track rows")
        .on_hover_text("One line per track, no artwork")
        .changed()
        && let Err(e) = app.settings.save()
    {
        app.toast(format!("Failed to save: {e}"));
    }
    interface_font_row(app, ui);

    ui.add_space(Metrics::SP_2);
    ui.label(Type::H4.rich("Equalizer (10-band)", app.theme.text));
    ui.checkbox(&mut app.settings.eq_enabled, "Enable EQ");
    let freqs = crate::audio::dsp::EQ_BAND_FREQS;
    ui.horizontal(|ui| {
        for (i, gain) in app.settings.eq_gains_db.iter_mut().enumerate() {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(format!("{:.0}", freqs[i])).font(Type::CAPTION.font()),
                );
                ui.add(
                    egui::Slider::new(gain, -12.0..=12.0)
                        .vertical()
                        .show_value(false),
                );
                ui.label(egui::RichText::new(format!("{gain:+.0}")).font(Type::CAPTION.font()));
            });
        }
    });
    app.player
        .set_eq(app.settings.eq_enabled, app.settings.eq_gains_db);

    ui.add_space(Metrics::SP_2);
    ui.label(Type::H4.rich("Winamp mini player", app.theme.text));
    ui.label(Type::CAPTION.rich(
        "Ctrl+M opens a small player in a classic Winamp skin. \
         Drop a .wsz file on either window to wear it. Its title bar rolls the \
         window up to a single strip, sends it to the taskbar, or brings this \
         interface back.",
        app.theme.text_dim,
    ));
    ui.horizontal(|ui| {
        let open = app.mini_open();
        if ui
            .button(if open {
                "Close mini player"
            } else {
                "Open mini player"
            })
            .clicked()
        {
            app.toggle_mini();
        }
        ui.label(Type::CAPTION.rich("Scale", app.theme.text_dim));
        // Whole pixels only: the classic look does not survive interpolation.
        for scale in 1..=4u32 {
            let current = app.settings.winamp_scale == scale;
            if ui.selectable_label(current, format!("{scale}x")).clicked() && !current {
                app.set_mini_scale(scale);
            }
        }
    });
    ui.horizontal(|ui| {
        if ui
            .checkbox(&mut app.settings.winamp_on_top, "Always on top")
            .on_hover_text("Keep the mini player above other windows (the O lamp)")
            .changed()
        {
            // `apply_window_mode` only acts when the mode changed, so tell it
            // this counts as a change.
            app.forget_window_mode();
            if let Err(e) = app.settings.save() {
                app.toast(format!("Failed to save: {e}"));
            }
        }
        let rolled = app.settings.winamp_shade;
        if ui
            .checkbox(&mut app.settings.winamp_shade, "Rolled up")
            .on_hover_text("Windowshade: the title bar only, still playing")
            .changed()
        {
            // Put it back and go through the one path that owns this, so the
            // open window and the setting cannot disagree.
            app.settings.winamp_shade = rolled;
            app.toggle_shade(crate::ui::winamp::Window::Main);
        }
    });
    ui.horizontal(|ui| {
        // Winamp's other two windows, which dock under the main one.
        for (window, label, hint) in [
            (
                crate::ui::winamp::Window::Equalizer,
                "Equalizer",
                "Ten bands, a preamp and the curve, in the skin",
            ),
            (
                crate::ui::winamp::Window::Playlist,
                "Playlist",
                "The queue as Winamp's track list",
            ),
        ] {
            let mut open = match window {
                crate::ui::winamp::Window::Equalizer => app.settings.winamp_eq_window,
                _ => app.settings.winamp_pl_window,
            };
            if ui.checkbox(&mut open, label).on_hover_text(hint).changed() {
                app.toggle_skin_window_setting(window);
            }
        }
    });
    ui.horizontal(|ui| {
        let name = app
            .settings
            .winamp_skin
            .as_ref()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| {
                // The window knows its own skin's name once it is open.
                app.mini
                    .as_ref()
                    .map(|m| m.lock().skin_name().to_owned())
                    .unwrap_or_else(|| "Fastcloud (built in)".to_owned())
            });
        ui.label(Type::CAPTION.rich(&format!("Skin: {name}"), app.theme.text_dim));
        if app.settings.winamp_skin.is_some() && ui.button("Use built-in").clicked() {
            app.use_stock_skin();
        }
        if ui
            .button("Install skin…")
            .on_hover_text("Choose a classic Winamp .wsz or .zip skin")
            .clicked()
            && let Some(path) = rfd::FileDialog::new()
                .set_title("Install a mini-player skin")
                .add_filter("Classic Winamp skin", &["wsz", "zip"])
                .pick_file()
        {
            app.apply_skin_file(path);
        }
        if ui
            .button("Browse skins online")
            .on_hover_text("Winamp Skin Museum")
            .clicked()
        {
            let _ = webbrowser::open("https://skins.webamp.org");
        }
    });
    ui.label(Type::CAPTION.rich(
        "Download a classic Winamp 2 skin (.wsz or .zip), then click Install skin…. You can also drag the file anywhere onto Fastcloud. The installed copy is kept by the app and restored on the next launch.",
        app.theme.text_dim,
    ));

    ui.add_space(Metrics::SP_2);
    ui.label(Type::H4.rich("Storage", app.theme.text));
    let art_mb = app.art.decoded_byte_size() as f64 / 1_048_576.0;
    let budget_mb = crate::images::MAX_DECODED_BYTES as f64 / 1_048_576.0;
    ui.label(Type::CAPTION.rich(
        &format!("Decoded artwork: {art_mb:.1} MiB of {budget_mb:.0} MiB budget"),
        app.theme.text_dim,
    ));
    if !app.demo {
        ui.label(Type::CAPTION.rich(
            &format!("Cached lists: {}", app.store.len()),
            app.theme.text_dim,
        ));
    }
    ui.horizontal(|ui| {
        if ui
            .button("Clear artwork cache")
            .on_hover_text("Deletes downloaded covers; playing is not affected")
            .clicked()
        {
            let freed = app.art.clear_disk_cache();
            app.toast(format!(
                "Cleared artwork cache ({} freed)",
                crate::util::fmt_bytes(freed)
            ));
        }
        if !app.demo
            && ui
                .button("Refresh everything")
                .on_hover_text("Forget every cached list and ask SoundCloud again")
                .clicked()
        {
            app.store.clear();
            app.toast("Reloading from SoundCloud");
        }
    });

    ui.add_space(Metrics::SP_2);
    if ui.button("Keyboard shortcuts (F1)").clicked() {
        app.show_shortcuts = true;
    }

    ui.add_space(Metrics::SP_15);
    ui.separator();
    ui.label(Type::CAPTION.rich(
        &format!(
            "Fastcloud {} · {}",
            app.version_build,
            if app.demo {
                "demo library"
            } else if app.signed_in() {
                "signed in"
            } else {
                "connection required"
            }
        ),
        app.theme.text_dim,
    ));
}

/// The interface font row.
///
/// soundcloud.com's own face is commercially licensed, so Inter ships instead
/// (see [`crate::fonts`]). A user who owns Söhne can point at the file and get
/// the site's exact letterforms; the change needs a restart because egui
/// installs fonts once, when the context is created.
fn interface_font_row(app: &mut App, ui: &mut egui::Ui) {
    ui.add_space(Metrics::SP_075);
    let current = app
        .settings
        .interface_font
        .as_ref()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "Inter (built in)".to_owned());
    ui.horizontal(|ui| {
        ui.label(Type::CAPTION.rich(&format!("Interface font: {current}"), app.theme.text_dim));
        ui.label(Type::CAPTION.rich(
            "· drop a .ttf/.otf on the window to change it",
            app.theme.text_dim,
        ));
        if app.settings.interface_font.is_some() && ui.button("Use Inter").clicked() {
            app.settings.interface_font = None;
            match app.settings.save() {
                Ok(()) => app.toast("Interface font reset - restart to apply"),
                Err(e) => app.toast(format!("Failed to save: {e}")),
            }
        }
    });
    ui.label(Type::CAPTION.rich(
        "soundcloud.com uses Söhne, which is licensed and cannot be bundled. \
         Own it? Drop the file on the window.",
        app.theme.text_dim,
    ));
}
/// Account: the SoundCloud connection, signing in, and the app registration
/// this install uses.
///
/// Fastcloud ships no credentials of its own — SoundCloud treats every client
/// as confidential, so a secret is required and one baked into an open-source
/// binary would be public. [`crate::auth::register`] gets the user their own in
/// one browser sign-in; the fields below are the manual route for someone who
/// already has a pair.
fn account_section(app: &mut App, ui: &mut egui::Ui) {
    ui.label(Type::H4.rich("Account", app.theme.text));

    if app.demo {
        ui.label(Type::BODY.rich(
            "Running on the built-in demo library — no network, no account.",
            app.theme.text_dim,
        ));
    } else if app.signed_in() {
        let who = app.display_name();
        ui.horizontal(|ui| {
            ui.label(Type::BODY.rich(&format!("Signed in as {who}"), app.theme.text));
            if ui.button("Sign out").clicked() {
                app.sign_out();
            }
            if ui
                .button("Disconnect Fastcloud")
                .on_hover_text("Revoke Fastcloud's OAuth access to this SoundCloud account")
                .clicked()
            {
                app.disconnect_account();
            }
        });
    }

    ui.add_space(Metrics::SP_075);
    ui.label(Type::CAPTION.rich(
        "No paid subscription is needed to listen: free SoundCloud accounts stream \
         everything public in full. Tracks marked GO+ need a Go+ subscription on your \
         own account; tracks marked BLOCKED are restricted by the rightsholder and \
         cannot be played off soundcloud.com — we fall back to the 30-second preview \
         when there is one.",
        app.theme.text_dim,
    ));
}

// ===== Right rail (soundcloud.com style) =====

/// The right rail, as soundcloud.com has it: who to follow, what is new, and
/// your last likes. Every list comes from the same store the pages use, so
/// nothing here fetches on its own.
fn right_rail(app: &mut App, ui: &mut egui::Ui) {
    new_tracks_rail(app, ui);
    ui.add_space(Metrics::SP_5);

    // --- Artists you should follow ---
    ui.horizontal(|ui| {
        ui.label(Type::H6.rich("Artists you should follow", app.theme.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Label::new(Type::CAPTION.rich("Refresh list", app.theme.text_dim))
                        .sense(egui::Sense::click()),
                )
                .on_hover_text("Ask SoundCloud again")
                .clicked()
            {
                // Drop the cached list so the next frame refetches.
                if let Some(key) = app.shelf_key(crate::demo::Source::RelatedUsers) {
                    app.store.invalidate(&key);
                }
                app.toast("Suggestions refreshed");
            }
        });
    });
    ui.add_space(Metrics::SP_HALF);
    let suggested = app
        .shelf_key(crate::demo::Source::RelatedUsers)
        .map(|key| app.users(key))
        .unwrap_or(Slot::Ready(Vec::new()));
    for user in suggested.rows().iter().take(3) {
        ui.horizontal(|ui| {
            let avatar = user.avatar_url.as_deref().filter(|u| !u.is_empty());
            let clicked = match avatar {
                Some(url) => round_avatar(ui, url, &user.username, 44.0).clicked(),
                None => avatar_circle(ui, &user.username, 44.0).clicked(),
            };
            if clicked {
                app.navigate(Route::UserDetail(user.id));
            }
            ui.vertical(|ui| {
                if ui
                    .add(
                        egui::Label::new(Type::H4.rich(&user.username, app.theme.text))
                            .sense(egui::Sense::click())
                            .truncate(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    app.navigate(Route::UserDetail(user.id));
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = Metrics::SP_1;
                    stat(
                        app,
                        ui,
                        super::icons::Icon::Users,
                        Some(user.followers_count),
                    );
                    stat(app, ui, super::icons::Icon::Music, Some(user.track_count));
                });
            });
            // Follow sits at the row's right edge, like soundcloud.com.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                widgets::follow_button(app, ui, user.id, &user.username);
            });
        });
        ui.add_space(Metrics::SP_075);
    }
    if suggested.rows().is_empty() {
        ui.label(Type::CAPTION.rich(
            "Play something and suggestions appear here.",
            app.theme.text_dim,
        ));
    }

    // --- Your likes ---
    ui.add_space(Metrics::SP_075);
    let liked = app.tracks(Key::Likes);
    let liked_rows = liked.rows();
    ui.horizontal(|ui| {
        ui.label(Type::H6.rich(&format!("{} likes", liked_rows.len()), app.theme.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Label::new(Type::CAPTION.rich("View all", app.theme.text_dim))
                        .sense(egui::Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                app.library_tab = LibraryTab::Likes;
                app.navigate(Route::Library);
            }
        });
    });
    ui.add_space(Metrics::SP_HALF);
    let shared = std::sync::Arc::new(liked_rows.to_vec());
    for (i, track) in shared.iter().take(3).enumerate() {
        let shared = shared.clone();
        ui.horizontal(|ui| {
            let art = track.artwork_url();
            if widgets::artwork_img(
                ui,
                art,
                track.id,
                &track.title,
                Metrics::artwork(44.0),
                Metrics::RADIUS as f32,
            )
            .clicked()
            {
                app.play_user_queue((*shared).clone(), i, false);
            }
            ui.vertical(|ui| {
                let artist = ui.add(
                    egui::Label::new(Type::CAPTION.rich(
                        &truncate(&crate::bidi::owned(track.artist()), 16),
                        app.theme.text_dim,
                    ))
                    .sense(egui::Sense::click()),
                );
                if let Some(user) = &track.user
                    && artist
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                {
                    app.navigate(Route::UserDetail(user.id));
                }
                let tid = track.id;
                if ui
                    .add(
                        egui::Label::new(
                            Type::H4.rich(&truncate(&track.title, 22), app.theme.text),
                        )
                        .sense(egui::Sense::click())
                        .truncate(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    app.navigate(Route::TrackDetail(tid));
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = Metrics::SP_1;
                    stat(app, ui, super::icons::Icon::Music, track.playback_count);
                    stat(app, ui, super::icons::Icon::Heart, track.likes_count);
                });
            });
        });
        ui.add_space(Metrics::SP_HALF);
    }
    if shared.is_empty() {
        ui.label(Type::CAPTION.rich("Nothing liked yet.", app.theme.text_dim));
    }

    ui.add_space(Metrics::SP_5);
    ui.horizontal(|ui| {
        ui.label(Type::H6.rich("Listening history", app.theme.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Label::new(Type::CAPTION.rich("View all", app.theme.text_dim))
                        .sense(egui::Sense::click()),
                )
                .clicked()
            {
                app.library_tab = LibraryTab::History;
                app.navigate(Route::Library);
            }
        });
    });
    let history = recent_tracks(app, 3);
    for (index, track) in history.iter().enumerate() {
        ui.horizontal(|ui| {
            if widgets::artwork_img(
                ui,
                track.artwork_url(),
                track.id,
                &track.title,
                48.0,
                Metrics::RADIUS as f32,
            )
            .clicked()
            {
                app.play_user_queue(history.clone(), index, false);
            }
            ui.vertical(|ui| {
                let artist = ui.add(
                    egui::Label::new(Type::CAPTION.rich(track.artist(), app.theme.text_dim))
                        .sense(egui::Sense::click()),
                );
                if let Some(user) = &track.user
                    && artist
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                {
                    app.navigate(Route::UserDetail(user.id));
                }
                ui.add(egui::Label::new(Type::H4.rich(&track.title, app.theme.text)).truncate());
            });
        });
        ui.add_space(Metrics::SP_1);
    }
}

fn new_tracks_rail(app: &mut App, ui: &mut egui::Ui) {
    ui.label(Type::H6.rich("New tracks", app.theme.text));
    ui.add_space(Metrics::SP_1);
    let fresh = app
        .shelf_key(crate::demo::Source::RelatedToHistory)
        .map(|key| app.tracks(key))
        .unwrap_or(Slot::Ready(Vec::new()));
    let rows: Vec<Track> = fresh.rows().iter().take(5).cloned().collect();
    if rows.is_empty() {
        ui.label(Type::CAPTION.rich("Nothing yet.", app.theme.text_dim));
        return;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = Metrics::SP_1;
        for (index, track) in rows.iter().enumerate() {
            ui.vertical(|ui| {
                ui.set_max_width(56.0);
                if widgets::artwork_img(ui, track.artwork_url(), track.id, &track.title, 56.0, 28.0)
                    .clicked()
                {
                    app.play_user_queue(rows.clone(), index, false);
                }
                ui.add(
                    egui::Label::new(Type::CAPTION.rich(&track.title, app.theme.text)).truncate(),
                );
            });
        }
    });
}

/// Main content + right rail, like soundcloud.com. Page scroll is outer.
fn two_columns(app: &mut App, ui: &mut egui::Ui, main: impl FnOnce(&mut App, &mut egui::Ui)) {
    ui.horizontal(|ui| {
        let total = ui.available_width();
        let left_w = (total - 360.0).clamp(280.0, 820.0);
        ui.vertical(|ui| {
            ui.set_min_width(left_w);
            ui.set_max_width(left_w);
            main(app, ui);
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.set_min_width(180.0);
            right_rail(app, ui);
        });
    });
}

/// Avatar placeholder: a disc in the same deterministic palette the artwork
/// placeholders use, keyed on the name, with the initial over it.
fn avatar_circle(ui: &mut egui::Ui, name: &str, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        paint_initial(ui, rect, name, size);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// FNV-1a over the name, so one artist always gets one colour.
fn name_seed(name: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

// ===== Helpers =====

/// Recently played, newest first.
///
/// Local listens lead the server history, which may lag behind playback.
fn recent_tracks(app: &App, n: usize) -> Vec<Track> {
    let history = app.tracks(Key::History);
    let mut tracks = Vec::new();
    for id in &app.recent_ids {
        if let Some(track) = app.track(*id).ready() {
            tracks.push(track);
        }
    }
    for track in history.rows() {
        if !tracks.iter().any(|recent| recent.id == track.id) {
            tracks.push(track.clone());
        }
    }
    tracks.truncate(n);
    tracks
}

/// A playlist's tracks as shown: the optimistic list when an edit is
/// outstanding (see [`crate::playlists`]), else what was fetched.
fn playlist_tracks(app: &App, id: u64) -> Vec<Track> {
    // Liked Songs is not a playlist on SoundCloud; it is `/me/likes/tracks`.
    if app.demo && id == 0 {
        return app.tracks(Key::Likes).rows().to_vec();
    }
    // A local playlist holds ids; resolve them against everything in view.
    if app.demo
        && let Some(custom) = app.settings.custom_playlists.iter().find(|p| p.id == id)
    {
        let mut ids = custom.track_ids.clone();
        if let Some(extra) = app.extra_tracks.get(&id) {
            for extra_id in extra {
                if !ids.contains(extra_id) {
                    ids.push(*extra_id);
                }
            }
        }
        return ids
            .into_iter()
            .filter_map(|tid| app.track(tid).ready())
            .collect();
    }
    let fetched = app.tracks(Key::PlaylistTracks(id));
    let rows = fetched.rows();
    // An outstanding edit decides the order; the rows fill in the details.
    if let Some(desired) = app.playlist_edits.view(id) {
        return desired
            .iter()
            .filter_map(|tid| {
                rows.iter()
                    .find(|t| t.id == *tid)
                    .cloned()
                    .or_else(|| app.track(*tid).ready())
            })
            .collect();
    }
    let mut tracks = rows.to_vec();
    if let Some(extra) = app.extra_tracks.get(&id) {
        for tid in extra {
            if tracks.iter().any(|t| t.id == *tid) {
                continue;
            }
            if let Some(track) = app.track(*tid).ready() {
                tracks.push(track);
            }
        }
    }
    tracks
}

fn track_list(app: &mut App, ui: &mut egui::Ui, tracks: &[Track]) {
    egui::ScrollArea::vertical()
        .id_salt("track-list")
        .show(ui, |ui| {
            track_rows(app, ui, tracks);
        });
}

/// Same rows without their own scroll area — for pages that scroll as a whole.
/// The queue is shared by reference: no per-row full-list clone (O(n²)).
/// Ctrl/Cmd-click picks rows, Shift-click picks a range, plain click plays
/// and clears the pick (fastpotify's multi-select).
fn track_rows(app: &mut App, ui: &mut egui::Ui, tracks: &[Track]) {
    use crate::ui::widgets::RowAction;
    let shared = std::sync::Arc::new(tracks.to_vec());
    let order: Vec<u64> = shared.iter().map(|t| t.id).collect();
    // Picked tracks in table order for the multi menu.
    let multi: Vec<Track> = order
        .iter()
        .filter(|id| app.selected.contains(id))
        .filter_map(|id| shared.iter().find(|t| &t.id == id).cloned())
        .collect();
    let multi_ref = (!multi.is_empty()).then_some(multi.as_slice());
    for (idx, t) in shared.iter().enumerate() {
        let t = t.clone();
        let shared = shared.clone();
        let picked = app.selected.contains(&t.id);
        let multi = if picked { multi_ref } else { None };
        match widgets::track_row(app, ui, &t, idx, picked, multi) {
            RowAction::Play => {
                app.clear_selection();
                let queue = (*shared).clone();
                app.play_user_queue(queue, idx, false);
            }
            RowAction::SelectToggle(id) => app.toggle_select(id),
            RowAction::SelectRange(id) => app.select_range(&order, id),
            RowAction::OpenArtist(_) | RowAction::None => {}
        }
    }
}

/// Playlist rows: [`track_rows`] plus per-row Remove / Move up / Move down,
/// applied optimistically (see `crate::playlists`).
fn playlist_rows(app: &mut App, ui: &mut egui::Ui, playlist_id: u64, tracks: &[Track]) {
    track_rows(app, ui, tracks);
    ui.add_space(4.0);
    let ids: Vec<u64> = tracks.iter().map(|t| t.id).collect();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(
            egui::RichText::new("Edit:")
                .font(Type::CAPTION.font())
                .color(app.theme.text_dim),
        );
        for (idx, track) in tracks.iter().enumerate() {
            let label = truncate(&track.title, 12);
            ui.menu_button(label, |ui| {
                if ui.button("Remove from playlist").clicked() {
                    app.remove_from_playlist(playlist_id, track.id);
                    app.toast("Removed");
                    ui.close();
                }
                if idx > 0 && ui.button("Move up").clicked() {
                    app.move_in_playlist(playlist_id, &ids, idx, idx - 1);
                    ui.close();
                }
                if idx + 1 < tracks.len() && ui.button("Move down").clicked() {
                    app.move_in_playlist(playlist_id, &ids, idx, idx + 1);
                    ui.close();
                }
            });
        }
    });
}

fn page_title(app: &App, ui: &mut egui::Ui, title: &str, subtitle: Option<&str>) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(title)
            .font(Type::H1.font())
            .color(app.theme.text),
    );
    if let Some(sub) = subtitle {
        ui.label(
            egui::RichText::new(sub)
                .font(Type::BODY.font())
                .color(app.theme.text_dim),
        );
    }
    ui.add_space(8.0);
}

fn fmt_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        format!(
            "{}…",
            s.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}
