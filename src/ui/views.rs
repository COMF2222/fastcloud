use super::App;
use super::data;
use super::design::widgets::{self as airwave, ButtonVariant, Surface, SurfaceTone, ViewState};
use super::route::Route;
use super::theme::{Metrics, Type};
use super::widgets;
use crate::api::models::Track;
use crate::config::QuickAccessShortcut;
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
pub enum SearchMode {
    #[default]
    Text,
    Vibe,
    SoundCloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchSurface {
    Wave,
    Text,
    Vibe,
    SoundCloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMove {
    Previous,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextLane {
    Lexical(usize),
    Vibe(usize),
}

impl LibraryTab {
    fn label(self, language: crate::config::Language) -> &'static str {
        match self {
            Self::Overview => language.text("Overview", "Обзор"),
            Self::Likes => language.text("Likes", "Лайки"),
            Self::Playlists => language.text("Playlists", "Плейлисты"),
            Self::Albums => language.text("Albums", "Альбомы"),
            Self::Uploads => language.text("Tracks", "Треки"),
            Self::Stations => language.text("Stations", "Станции"),
            Self::Following => language.text("Following", "Подписки"),
            Self::History => language.text("History", "История"),
        }
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    if matches!(app.route, Route::Settings) {
        // The section rail and heading belong to the settings shell; only
        // the long list of controls scrolls. Otherwise the rail vanishes and
        // leaves an empty column halfway down the page.
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(
                Metrics::SP_075 as i8,
                Metrics::SP_15 as i8,
            ))
            .show(ui, |ui| settings(app, ui));
        return;
    }
    let selection_tracks = match (&app.route, app.library_tab) {
        (Route::Likes, _) | (Route::Library, LibraryTab::Likes) => {
            Some(filtered(app, app.tracks(Key::Likes).rows()))
        }
        _ => None,
    };
    if let Some(tracks) = selection_tracks {
        library_selection_bar(app, ui, &tracks);
    }
    let page_salt = page_scroll_salt(&app.route);
    let page_offset = egui::scroll_area::State::load(ui.ctx(), ui.make_persistent_id(page_salt))
        .map_or(0.0, |state| state.offset.y);
    if should_show_detail_actions(&app.route, page_offset) {
        detail_sticky_actions(app, ui);
    }
    // One outer scroll for the whole page; inner lists expand into it.
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt(page_salt)
        .wheel_scroll_multiplier(page_wheel_multiplier(&app.route))
        .auto_shrink([false, false]);
    if let Some(offset) = app.screenshot_scroll_offset.take() {
        scroll = scroll.vertical_scroll_offset(offset);
    }
    scroll.show(ui, |ui| {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(
                Metrics::SP_075 as i8,
                Metrics::SP_15 as i8,
            ))
            .show(ui, |ui| show_inner(app, ui));
    });
}

fn page_wheel_multiplier(route: &Route) -> egui::Vec2 {
    if matches!(route, Route::Settings) {
        egui::vec2(1.0, 1.85)
    } else {
        egui::Vec2::ONE
    }
}

fn page_scroll_salt(route: &Route) -> egui::Id {
    match route {
        Route::Home => egui::Id::new(("page", 0_u8)),
        Route::Discover => egui::Id::new(("page", 1_u8)),
        Route::Catalog => egui::Id::new(("page", 12_u8)),
        Route::Feed => egui::Id::new(("page", 2_u8)),
        Route::Library => egui::Id::new(("page", 3_u8)),
        Route::Likes => egui::Id::new(("page", 4_u8)),
        Route::Search(query) => egui::Id::new(("page", 5_u8, query.as_str())),
        Route::TrackDetail(id) => egui::Id::new(("page", 6_u8, *id)),
        Route::PlaylistDetail(id) => egui::Id::new(("page", 7_u8, *id)),
        Route::UserDetail(id) => egui::Id::new(("page", 8_u8, *id)),
        Route::Recent => egui::Id::new(("page", 9_u8)),
        Route::Following => egui::Id::new(("page", 10_u8)),
        Route::Settings => egui::Id::new(("page", 11_u8)),
    }
}

fn should_show_detail_actions(route: &Route, page_offset: f32) -> bool {
    page_offset >= 220.0
        && matches!(
            route,
            Route::TrackDetail(_) | Route::PlaylistDetail(_) | Route::UserDetail(_)
        )
}

fn detail_sticky_actions(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::top(egui::Id::new("detail-sticky-actions"))
        .resizable(false)
        .show(ui, |ui| {
            egui::Frame::new()
                .fill(app.theme.tokens.surface_raised)
                .stroke(egui::Stroke::new(1.0, app.theme.tokens.border_subtle))
                .corner_radius(Metrics::RADIUS_INPUT)
                .inner_margin(egui::Margin::symmetric(
                    Metrics::SP_1 as i8,
                    Metrics::SP_075 as i8,
                ))
                .show(ui, |ui| match app.route.clone() {
                    Route::TrackDetail(id) => {
                        let Some(track) = app.track(id).ready() else {
                            return;
                        };
                        let related = app.tracks(Key::Related(id));
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Label::new(Type::H4.rich(&track.title, app.theme.text))
                                    .truncate(),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    widgets::like_button(app, ui, track.id, &track.title);
                                    if airwave::action_button(
                                        ui,
                                        app.theme,
                                        ButtonVariant::Primary,
                                        "Play",
                                    )
                                    .clicked()
                                    {
                                        app.play_user_queue(
                                            detail_queue(&track, related.rows()),
                                            0,
                                            false,
                                        );
                                    }
                                },
                            );
                        });
                    }
                    Route::PlaylistDetail(id) => {
                        let title = playlist_title(app, id);
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Label::new(Type::H4.rich(&title, app.theme.text)).truncate(),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if airwave::action_button(
                                        ui,
                                        app.theme,
                                        ButtonVariant::Primary,
                                        "Play",
                                    )
                                    .clicked()
                                    {
                                        let tracks = playlist_tracks(app, id);
                                        if !tracks.is_empty() {
                                            app.play_user_queue(tracks, 0, false);
                                        }
                                    }
                                },
                            );
                        });
                    }
                    Route::UserDetail(id) => {
                        let Some(user) = app.user(id).ready() else {
                            return;
                        };
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Label::new(Type::H4.rich(&user.username, app.theme.text))
                                    .truncate(),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    widgets::follow_button(app, ui, id, &user.username);
                                    if airwave::action_button(
                                        ui,
                                        app.theme,
                                        ButtonVariant::Secondary,
                                        "Station",
                                    )
                                    .clicked()
                                    {
                                        let seed =
                                            app.tracks(Key::UserTracks(id)).rows().first().cloned();
                                        if let Some(seed) = seed {
                                            app.settings.autoplay = true;
                                            app.player.set_autoplay(true);
                                            let queue = app.station_queue(&seed);
                                            app.play_user_queue(queue, 0, false);
                                        } else {
                                            app.toast("Nothing to seed a station with yet");
                                        }
                                    }
                                },
                            );
                        });
                    }
                    _ => {}
                });
        });
}

fn show_inner(app: &mut App, ui: &mut egui::Ui) {
    match app.route.clone() {
        Route::Home => home(app, ui),
        Route::Discover => discover(app, ui),
        Route::Catalog => catalog(app, ui),
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

// ===== Home =====

fn home(app: &mut App, ui: &mut egui::Ui) {
    use crate::demo::{HomeShelf, ShelfKind, Source};
    let language = app.settings.language;

    ui.add_space(Metrics::SP_HALF);
    let recent = recent_tracks(app, 12);
    let current = {
        let state = app.player.state.lock();
        state
            .current
            .and_then(|index| state.queue.get(index).cloned())
    };
    let focus = current.or_else(|| {
        recent
            .first()
            .cloned()
            .or_else(|| app.tracks(Key::Likes).rows().first().cloned())
    });

    continue_listening_hero(app, ui, focus.as_ref());
    quick_picks(app, ui);

    app.home_recommendation_seed = home_recommendation_seed(
        app.home_recommendation_seed,
        app.tracks(Key::Likes).rows(),
        &recent,
    );
    if let Some(seed) = app.home_recommendation_seed {
        let made_for_you = HomeShelf {
            title: language.text("Made for you", "Для тебя"),
            subtitle: language.text(
                "A fresh lane shaped by your latest listening",
                "Подборка по тому, что ты слушал недавно",
            ),
            salt: "airwave-made-for-you",
            kind: ShelfKind::Tracks,
            source: Source::RelatedToHistory,
        };
        shelf_header(app, ui, made_for_you.title, made_for_you.subtitle);
        home_shelf(app, ui, &made_for_you, Key::Related(seed));
    }

    let fresh = HomeShelf {
        title: language.text("Fresh from artists", "Новое от исполнителей"),
        subtitle: language.text(
            "Recent tracks from people you follow",
            "Свежие треки от твоих подписок",
        ),
        salt: "airwave-following-fresh",
        kind: ShelfKind::Tracks,
        source: Source::RecentlyPlayed,
    };
    shelf_header(app, ui, fresh.title, fresh.subtitle);
    home_shelf(app, ui, &fresh, Key::FollowingTracks);

    if recent.is_empty() {
        widgets::section_header(app, ui, language.text("Recently played", "Недавно слушали"));
        ui.label(Type::BODY.rich(
            language.text(
                "Press play on anything to get started.",
                "Включи любой трек, чтобы начать.",
            ),
            app.theme.text_dim,
        ));
    } else {
        recently_played_panel(app, ui, &recent);
    }

    shelf_header(
        app,
        ui,
        language.text("Your playlists", "Твои плейлисты"),
        language.text(
            "Pick up a saved collection without leaving Home",
            "Продолжай слушать прямо с главной",
        ),
    );
    home_account_playlists(app, ui, false, "airwave-home-playlists");
    footer(app, ui);
}

fn home_recommendation_seed(
    existing: Option<u64>,
    likes: &[Track],
    recent: &[Track],
) -> Option<u64> {
    existing.or_else(|| {
        likes
            .first()
            .or_else(|| recent.first())
            .map(|track| track.id)
    })
}

#[cfg(test)]
mod home_seed_tests {
    use super::home_recommendation_seed;

    #[test]
    fn switching_tracks_does_not_change_an_established_recommendation_seed() {
        let tracks = crate::demo::demo_tracks();
        let first = home_recommendation_seed(None, &tracks[..1], &tracks[1..2]);
        let after_switch = home_recommendation_seed(first, &tracks[2..3], &tracks[3..4]);
        assert_eq!(after_switch, first);
    }
}

fn recently_played_panel(app: &mut App, ui: &mut egui::Ui, tracks: &[Track]) {
    widgets::section_header(
        app,
        ui,
        app.settings
            .language
            .text("Recently played", "Недавно слушали"),
    );
    let surface = if app.settings.background_image.is_some() {
        Surface::clear()
    } else {
        Surface::glass()
    };
    surface
        .padding(Metrics::SP_1 as i8)
        .show(ui, app.theme, |ui| {
            track_rows(app, ui, tracks);
        });
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui
            .button(app.settings.language.text("Go to history", "К истории"))
            .clicked()
        {
            app.library_tab = LibraryTab::History;
            app.navigate(Route::Library);
        }
    });
}

fn continue_listening_hero(app: &mut App, ui: &mut egui::Ui, track: Option<&Track>) {
    ui.label(
        Type::H1.rich(
            app.settings
                .language
                .text("Good to see you", "Рады тебя видеть"),
            app.theme.text,
        ),
    );
    ui.label(Type::BODY.rich(
        app.settings.language.text(
            "Continue listening or launch a new wave.",
            "Продолжай слушать или открой новую волну.",
        ),
        app.theme.text_dim,
    ));
    ui.add_space(Metrics::SP_2);

    let surface = if app.settings.background_image.is_some() {
        Surface::clear()
    } else {
        Surface::glass()
    };
    surface.show(ui, app.theme, |ui| {
        let Some(track) = track else {
            ui.set_min_height(132.0);
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.label(Type::H2.rich("Your next track starts here", app.theme.text));
                    if airwave::action_button(
                        ui,
                        app.theme,
                        ButtonVariant::Secondary,
                        "Explore music",
                    )
                    .clicked()
                    {
                        app.open_search();
                    }
                });
            });
            return;
        };

        let state = app.player.state.lock();
        let is_current = state
            .current
            .and_then(|index| state.queue.get(index))
            .is_some_and(|current| current.id == track.id);
        let playing = is_current && state.is_playing;
        let position = if is_current { state.position_ms } else { 0 };
        drop(state);
        let duration = track.effective_duration_ms().max(1);
        let peaks = track
            .waveform_url
            .as_deref()
            .and_then(|url| app.waveforms.get(ui.ctx(), url));
        let art_size = if ui.available_width() >= 720.0 {
            176.0
        } else {
            132.0
        };

        ui.horizontal(|ui| {
            if widgets::artwork_img(
                ui,
                track.artwork_url(),
                track.id,
                &track.title,
                art_size,
                Metrics::RADIUS_LG as f32,
            )
            .on_hover_text("Open track")
            .clicked()
            {
                app.navigate(Route::TrackDetail(track.id));
            }
            ui.add_space(Metrics::SP_2);
            ui.vertical(|ui| {
                ui.set_min_height(art_size);
                ui.label(Type::MICRO.rich("CONTINUE LISTENING", app.theme.accent));
                ui.add(egui::Label::new(Type::H1.rich(&track.title, app.theme.text)).truncate());
                let artist = ui
                    .add(
                        egui::Label::new(Type::BODY.rich(track.artist(), app.theme.text_dim))
                            .sense(egui::Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if artist.clicked()
                    && let Some(user) = &track.user
                {
                    app.navigate(Route::UserDetail(user.id));
                }
                ui.add_space(Metrics::SP_1);
                ui.horizontal(|ui| {
                    let play_label = if playing { "Pause" } else { "Play" };
                    if airwave::action_button(ui, app.theme, ButtonVariant::Primary, play_label)
                        .clicked()
                    {
                        if is_current {
                            app.player.play_pause();
                        } else {
                            app.play_user_queue(vec![track.clone()], 0, false);
                        }
                    }
                    if airwave::action_button(ui, app.theme, ButtonVariant::Secondary, "Next up")
                        .clicked()
                    {
                        app.show_queue = true;
                    }
                });
                ui.add_space(Metrics::SP_HALF);
                if let Some(frac) = widgets::waveform(
                    app,
                    ui,
                    track.id,
                    position,
                    duration,
                    42.0,
                    96,
                    peaks.as_deref(),
                ) {
                    if !is_current {
                        app.play_user_queue(vec![track.clone()], 0, false);
                    }
                    app.player.seek_ms((frac as f64 * duration as f64) as u64);
                }
            });
        });
    });
}

fn quick_picks(app: &mut App, ui: &mut egui::Ui) {
    let language = app.settings.language;
    widgets::section_header(app, ui, language.text("Quick Access", "Быстрый доступ"));
    let items: Vec<_> = app
        .settings
        .quick_access
        .iter()
        .filter(|item| item.is_media())
        .take(8)
        .cloned()
        .collect();
    if items.is_empty() {
        ui.label(Type::CAPTION.rich(
            language.text(
                "Pin a track, playlist or album from its page to find it here.",
                "Закрепи трек, плейлист или альбом на его странице — он появится здесь.",
            ),
            app.theme.text_dim,
        ));
        return;
    }
    let mut destination = None;
    ui.horizontal_wrapped(|ui| {
        for item in &items {
            let (label, detail, icon, artwork, route) = match item {
                QuickAccessShortcut::Track {
                    id,
                    title,
                    artist,
                    artwork_url,
                } => (
                    title.as_str(),
                    artist.as_str(),
                    super::icons::Icon::Music,
                    artwork_url.as_deref(),
                    Route::TrackDetail(*id),
                ),
                QuickAccessShortcut::Playlist {
                    id,
                    title,
                    artist,
                    artwork_url,
                } => (
                    title.as_str(),
                    artist.as_str(),
                    super::icons::Icon::ListPlus,
                    artwork_url.as_deref(),
                    Route::PlaylistDetail(*id),
                ),
                QuickAccessShortcut::Album {
                    id,
                    title,
                    artist,
                    artwork_url,
                } => (
                    title.as_str(),
                    artist.as_str(),
                    super::icons::Icon::Disc,
                    artwork_url.as_deref(),
                    Route::PlaylistDetail(*id),
                ),
                _ => continue,
            };
            let width = ((ui.available_width() - Metrics::SP_1) / 3.0).max(180.0);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width.min(300.0), 66.0), egui::Sense::click());
            airwave::paint_clear_glass_rect(
                ui,
                rect,
                Metrics::RADIUS_INPUT,
                app.theme,
                response.hovered().then_some(app.theme.accent),
            );
            let icon_rect = egui::Rect::from_center_size(
                egui::pos2(rect.left() + 25.0, rect.center().y),
                egui::Vec2::splat(28.0),
            );
            let painted = artwork.is_some_and(|url| {
                let image = egui::Image::new(url)
                    .show_loading_spinner(false)
                    .fit_to_exact_size(icon_rect.size())
                    .corner_radius(Metrics::RADIUS);
                if image.load_for_size(ui.ctx(), icon_rect.size()).is_ok() {
                    image.paint_at(ui, icon_rect);
                    true
                } else {
                    false
                }
            });
            if !painted {
                super::icons::paint(ui, icon, icon_rect, 18.0, app.theme.accent);
            }
            let clip = egui::Rect::from_min_max(
                egui::pos2(rect.left() + 48.0, rect.top()),
                egui::pos2(rect.right() - 8.0, rect.bottom()),
            );
            let painter = ui.painter().with_clip_rect(clip);
            painter.text(
                egui::pos2(clip.left(), rect.center().y - 9.0),
                egui::Align2::LEFT_CENTER,
                label,
                Type::H4.font(),
                app.theme.text,
            );
            painter.text(
                egui::pos2(clip.left(), rect.center().y + 10.0),
                egui::Align2::LEFT_CENTER,
                detail,
                Type::CAPTION.font(),
                app.theme.text_dim,
            );
            if response.clicked() {
                destination = Some(route);
            }
        }
    });
    if let Some(route) = destination {
        app.navigate(route);
    }
}

fn toggle_media_quick_access(app: &mut App, item: QuickAccessShortcut) {
    let before = app.settings.quick_access.clone();
    let pinned = app.settings.toggle_quick_access(item);
    if let Err(error) = app.settings.save() {
        app.settings.quick_access = before;
        app.toast(format!("Could not save Quick Access: {error}"));
    } else {
        app.toast(app.settings.language.text(
            if pinned {
                "Added to Quick Access"
            } else {
                "Removed from Quick Access"
            },
            if pinned {
                "Добавлено в быстрый доступ"
            } else {
                "Удалено из быстрого доступа"
            },
        ));
    }
}

fn quick_access_button(app: &mut App, ui: &mut egui::Ui, item: QuickAccessShortcut) {
    let pinned = app
        .settings
        .quick_access
        .iter()
        .any(|saved| saved.same_target(&item));
    let label = app.settings.language.text(
        if pinned {
            "Remove from Quick Access"
        } else {
            "Add to Quick Access"
        },
        if pinned {
            "Убрать из быстрого доступа"
        } else {
            "В быстрый доступ"
        },
    );
    if airwave::action_button(ui, app.theme, ButtonVariant::Secondary, label).clicked() {
        toggle_media_quick_access(app, item);
    }
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
            let click = widgets::media_card(
                app,
                ui,
                widgets::MediaCard::opens(
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
                    let click = widgets::media_card(
                        app,
                        ui,
                        widgets::MediaCard::opens(pid, &title, &format!("{kind} · {count} tracks"))
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
                    let click = widgets::media_card(
                        app,
                        ui,
                        widgets::MediaCard::plays(
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
                    let click = widgets::media_card(
                        app,
                        ui,
                        widgets::MediaCard {
                            subtitle: &format!("{owner}'s pick"),
                            ..widgets::MediaCard::track(track)
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
    widgets::artwork_img(ui, Some(url), name_seed(name), name, size, size / 2.0)
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
            let click = widgets::media_card(app, ui, widgets::MediaCard::track(track));
            if click.play {
                app.play_user_queue((*shared).clone(), i, false);
            }
        }
    });
}

// ===== Feed =====

#[derive(Clone, Default)]
struct FeedHeights {
    width: f32,
    scale: f32,
    rows: std::collections::HashMap<u64, f32>,
}

impl FeedHeights {
    fn row_height(&self, id: u64, has_activity: bool) -> f32 {
        self.rows.get(&id).copied().unwrap_or_else(|| {
            // Until measured, allow for artwork wrapping below the text column.
            let body = if self.width < 400.0 { 360.0 } else { 210.0 };
            body + if has_activity { 42.0 } else { 0.0 }
        })
    }
}

fn feed_row_visible(top: f32, height: f32, viewport: egui::Rect) -> bool {
    top <= viewport.bottom() + 200.0 && top + height >= viewport.top() - 200.0
}

fn feed_header_stacks(width: f32) -> bool {
    width < 560.0
}

#[cfg(test)]
mod feed_visibility_tests {
    use super::*;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(0.0, 1000.0), egui::pos2(800.0, 1600.0))
    }

    #[test]
    fn keeps_partially_visible_cards() {
        assert!(feed_row_visible(900.0, 250.0, viewport()));
    }

    #[test]
    fn skips_cards_above_overscan() {
        assert!(!feed_row_visible(0.0, 250.0, viewport()));
    }

    #[test]
    fn skips_cards_below_overscan() {
        assert!(!feed_row_visible(2000.0, 250.0, viewport()));
    }

    #[test]
    fn long_feed_only_renders_nearby_measured_cards() {
        let rendered = (0..10_000)
            .filter(|index| feed_row_visible(*index as f32 * 250.0, 250.0, viewport()))
            .count();
        assert_eq!(rendered, 5);
    }

    #[test]
    fn cold_feed_does_not_build_all_cards() {
        let heights = FeedHeights {
            width: 600.0,
            ..Default::default()
        };
        let mut top = 0.0;
        let mut rendered = 0;
        for id in 0..10_000 {
            let height = heights.row_height(id, false);
            rendered += usize::from(feed_row_visible(top, height, viewport()));
            top += height;
        }
        assert_eq!(rendered, 6);
    }

    #[test]
    fn measured_height_overrides_estimate() {
        let mut heights = FeedHeights::default();
        heights.rows.insert(7, 275.0);
        assert_eq!(heights.row_height(7, true), 275.0);
    }

    #[test]
    fn narrow_feed_reserves_more_space_for_wrapped_artwork() {
        let narrow = FeedHeights {
            width: 300.0,
            ..Default::default()
        };
        let wide = FeedHeights {
            width: 600.0,
            ..Default::default()
        };
        assert!(narrow.row_height(1, false) > wide.row_height(1, false));
    }

    #[test]
    fn feed_header_stacks_before_the_reposts_toggle_overlaps_the_title() {
        assert!(feed_header_stacks(448.0));
    }

    #[test]
    fn feed_header_stays_inline_when_the_main_column_is_wide() {
        assert!(!feed_header_stacks(720.0));
    }
}

fn feed(app: &mut App, ui: &mut egui::Ui) {
    let language = app.settings.language;
    ui.scope(|ui| {
        ui.add_space(4.0);
        let feed_title = language.text("Hear the latest posts from the people you're following:", "Новые публикации тех, на кого ты подписан:");
        let stacked = feed_header_stacks(ui.available_width());
        let header = Surface::glass().show(ui, app.theme, |ui| {
            if stacked {
                feed_header_identity(app, ui, feed_title);
                ui.add_space(Metrics::SP_1);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut app.show_reposts, language.text("Include reposts", "Показывать репосты"));
                    feed_header_dismiss(app, ui);
                });
            } else {
                ui.horizontal(|ui| {
                    feed_header_identity(app, ui, feed_title);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        feed_header_dismiss(app, ui);
                        ui.checkbox(&mut app.show_reposts, language.text("Include reposts", "Показывать репосты"));
                    });
                });
            }
            if !app.feed_banner_dismissed {
                ui.add_space(Metrics::SP_1);
                ui.label(Type::BODY.rich(
                    language.text("New uploads, reposts and discoveries from the artists you follow — in one continuous stream.", "Новые треки, репосты и находки от твоих подписок — в одной ленте."),
                    app.theme.text_dim,
                ));
            }
        });
        ui.painter().vline(
            header.response.rect.left() + 1.0,
            (header.response.rect.top() + 18.0)..=(header.response.rect.bottom() - 18.0),
            egui::Stroke::new(2.0, app.theme.accent),
        );
        ui.add_space(Metrics::SP_2);
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
                language.text("Nothing yet — follow some artists and their posts land here.", "Пока пусто. Подпишись на исполнителей, и их записи появятся здесь."),
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
        let cache_id = ui.id().with("feed-measured-heights");
        let mut heights = ui
            .ctx()
            .data_mut(|data| data.remove_temp::<FeedHeights>(cache_id))
            .unwrap_or_default();
        let width = ui.available_width();
        let scale = ui.ctx().pixels_per_point();
        if heights.width != width || heights.scale != scale {
            heights.rows.clear();
            heights.width = width;
            heights.scale = scale;
        }
        let mut current_heights = std::collections::HashMap::new();
        for track in slot.rows() {
            let item = activity.get(&track.id);
            if (track.feed_reposted || item.is_some_and(|i| i.kind == "reposted"))
                && !app.show_reposts
            {
                continue;
            }
            let top = ui.cursor().top();
            let height = heights.row_height(track.id, item.is_some());
            if !feed_row_visible(top, height, ui.clip_rect()) {
                ui.add_space(height);
                // Do not turn an estimate into a measured cache entry.
                if heights.rows.contains_key(&track.id) {
                    current_heights.insert(track.id, height);
                }
                continue;
            }
            let line = item.map(|i| {
                (
                    i.user.clone(),
                    i.kind.to_string(),
                    format!("{} ago", i.when),
                )
            });
            ui.push_id(("feed-track", track.id), |ui| {
                feed_card(app, ui, track, line, None);
            });
            ui.add_space(Metrics::SP_175);
            current_heights.insert(track.id, ui.cursor().top() - top);
        }
        heights.rows = current_heights;
        ui.ctx()
            .data_mut(|data| data.insert_temp(cache_id, heights));
        if !playlist_slot.rows().is_empty() {
            let playlists: Vec<_> = playlist_slot
                .rows()
                .iter()
                .filter(|playlist| app.show_reposts || !playlist.feed_reposted)
                .collect();
            if !playlists.is_empty() {
                widgets::section_header(app, ui, "Playlist posts");
                playlist_carousel(app, ui, "feed-playlists", playlists);
            }
        }
    });
}

fn feed_header_identity(app: &App, ui: &mut egui::Ui, feed_title: &str) {
    ui.horizontal(|ui| {
        let (icon_rect, _) = ui.allocate_exact_size(egui::Vec2::splat(48.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(icon_rect.center(), 24.0, app.theme.tokens.accent_soft);
        super::icons::paint(
            ui,
            super::icons::Icon::AudioLines,
            icon_rect,
            21.0,
            app.theme.accent,
        );
        ui.add_space(Metrics::SP_HALF);
        ui.vertical(|ui| {
            ui.label(Type::MICRO.rich("LIVE SIGNAL  /  FOLLOWING", app.theme.accent));
            ui.label(Type::H2.rich("Your feed", app.theme.text));
            ui.label(Type::CAPTION.rich(feed_title, app.theme.text_dim));
        });
    });
}

fn feed_header_dismiss(app: &mut App, ui: &mut egui::Ui) {
    if !app.feed_banner_dismissed
        && super::icons::show(ui, super::icons::Icon::X, 15.0, app.theme.text_dim)
            .on_hover_text("Hide introduction")
            .clicked()
    {
        app.feed_banner_dismissed = true;
    }
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
    let response = Surface::glass()
        .padding(Metrics::SP_15 as i8)
        .show(ui, app.theme, |ui| {
            feed_card_content(app, ui, track, activity, stamp);
        });
    let rect = response.response.rect;
    ui.painter().line_segment(
        [
            egui::pos2(rect.left() + 18.0, rect.top() + 1.0),
            egui::pos2(
                (rect.left() + 150.0).min(rect.right() - 18.0),
                rect.top() + 1.0,
            ),
        ],
        egui::Stroke::new(1.4, app.theme.accent.gamma_multiply(0.72)),
    );
}

fn feed_card_content(
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
    let language = app.settings.language;
    let tabs = [
        (LibraryTab::Overview, LibraryTab::Overview.label(language)),
        (LibraryTab::Likes, LibraryTab::Likes.label(language)),
        (LibraryTab::Playlists, LibraryTab::Playlists.label(language)),
        (LibraryTab::Albums, LibraryTab::Albums.label(language)),
        (LibraryTab::Stations, LibraryTab::Stations.label(language)),
        (LibraryTab::Following, LibraryTab::Following.label(language)),
        (LibraryTab::History, LibraryTab::History.label(language)),
    ];
    let header = Surface::glass().show(ui, app.theme, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(Type::MICRO.rich(
                    language.text("YOUR ARCHIVE  /  ALWAYS IN TUNE", "ТВОЯ МУЗЫКА"),
                    app.theme.accent,
                ));
                ui.label(Type::H2.rich(language.text("Library", "Библиотека"), app.theme.text));
                ui.label(Type::CAPTION.rich(
                    language.text(
                        "Everything you saved, followed and played — shaped into your own signal.",
                        "Все лайки, подписки и прослушивания — твоя музыкальная коллекция.",
                    ),
                    app.theme.text_dim,
                ));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::Vec2::splat(52.0), egui::Sense::hover());
                ui.painter().circle_stroke(
                    rect.center(),
                    25.0,
                    egui::Stroke::new(1.0, app.theme.tokens.glass_border),
                );
                ui.painter().circle_stroke(
                    rect.center(),
                    17.0,
                    egui::Stroke::new(1.0, app.theme.accent.gamma_multiply(0.65)),
                );
                super::icons::paint(ui, super::icons::Icon::Music, rect, 18.0, app.theme.accent);
            });
        });
        ui.add_space(Metrics::SP_15);
        if let Some(tab) = airwave::segmented_tabs(ui, app.theme, app.library_tab, &tabs) {
            app.clear_selection();
            app.library_tab = tab;
        }
    });
    ui.painter().line_segment(
        [
            egui::pos2(
                header.response.rect.left() + 20.0,
                header.response.rect.top() + 1.0,
            ),
            egui::pos2(
                (header.response.rect.left() + 210.0).min(header.response.rect.right() - 20.0),
                header.response.rect.top() + 1.0,
            ),
        ],
        egui::Stroke::new(1.4, app.theme.accent.gamma_multiply(0.72)),
    );
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

fn library_tab_intro(
    app: &App,
    ui: &mut egui::Ui,
    icon: super::icons::Icon,
    title: &str,
    description: &str,
) {
    Surface::glass().show(ui, app.theme, |ui| {
        ui.horizontal(|ui| {
            let (icon_rect, _) =
                ui.allocate_exact_size(egui::Vec2::splat(44.0), egui::Sense::hover());
            ui.painter().rect_filled(
                icon_rect,
                Metrics::RADIUS_INPUT,
                app.theme.tokens.accent_soft,
            );
            super::icons::paint(ui, icon, icon_rect, 20.0, app.theme.accent);
            ui.add_space(Metrics::SP_HALF);
            ui.vertical(|ui| {
                ui.label(Type::H2.rich(title, app.theme.text));
                ui.label(Type::CAPTION.rich(description, app.theme.text_dim));
            });
        });
    });
    ui.add_space(Metrics::SP_2);
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
    library_tab_intro(
        app,
        ui,
        super::icons::Icon::Heart,
        app.settings.language.text("Liked tracks", "Любимые треки"),
        app.settings.language.text(
            "Everything you saved, ready to play as one continuous collection.",
            "Всё сохранённое — одна непрерывная коллекция.",
        ),
    );
    ui.horizontal(|ui| {
        ui.label(Type::MICRO.rich("YOUR COLLECTION", app.theme.text_dim));
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
            t.title.to_lowercase().contains(&needle)
                || t.artist().to_lowercase().contains(&needle)
                || t.genre
                    .as_deref()
                    .is_some_and(|genre| genre.to_lowercase().contains(&needle))
        })
        .cloned()
        .collect()
}

/// Library overview: recently played cards + likes grid, like soundcloud.com.
fn overview(app: &mut App, ui: &mut egui::Ui) {
    let language = app.settings.language;
    let likes_slot = app.tracks(Key::Likes);
    soundprint_panel(app, ui, likes_slot.rows());

    widgets::section_header(
        app,
        ui,
        language.text("Fresh from people you follow", "Новое от подписок"),
    );
    let fresh = app.tracks(Key::FollowingTracks);
    if fresh.is_loading() {
        ui.label(Type::BODY.rich(
            language.text("Loading fresh releases…", "Загружаем новинки…"),
            app.theme.text_dim,
        ));
    } else if fresh.rows().is_empty() {
        ui.label(Type::BODY.rich(
            language.text(
                "Follow artists on SoundCloud and their newest tracks will appear here.",
                "Подпишись на исполнителей в SoundCloud — их новые треки появятся здесь.",
            ),
            app.theme.text_dim,
        ));
    } else {
        let tracks: Vec<_> = fresh.rows().iter().take(6).cloned().collect();
        track_cards(app, ui, "overview-following-fresh", &tracks);
    }

    widgets::section_header(app, ui, language.text("Recently played", "Недавно слушали"));
    let recent = recent_tracks(app, 6);
    if recent.is_empty() {
        ui.label(Type::BODY.rich(
            language.text(
                "Nothing played yet. Press play on anything.",
                "Ты пока ничего не слушал. Включи любой трек.",
            ),
            app.theme.text_dim,
        ));
    } else {
        track_cards(app, ui, "overview-recent", &recent);
    }

    widgets::section_header(app, ui, language.text("Likes", "Лайки"));
    let slot = likes_slot;
    if data::placeholder(
        app,
        ui,
        &slot,
        language.text(
            "Tap the heart on any track to save it here.",
            "Нажми сердечко на любом треке, чтобы сохранить его здесь.",
        ),
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

fn soundprint_panel(app: &mut App, ui: &mut egui::Ui, tracks: &[Track]) {
    let shares = super::soundprint::summarize(tracks, 7);
    Surface::glass().show(ui, app.theme, |ui| {
        ui.horizontal(|ui| {
            let (icon_rect, _) =
                ui.allocate_exact_size(egui::Vec2::splat(48.0), egui::Sense::hover());
            ui.painter().rect_filled(
                icon_rect,
                Metrics::RADIUS_INPUT,
                app.theme.tokens.accent_soft,
            );
            super::icons::paint(
                ui,
                super::icons::Icon::AudioLines,
                icon_rect,
                21.0,
                app.theme.accent,
            );
            ui.add_space(Metrics::SP_HALF);
            ui.vertical(|ui| {
                ui.label(Type::MICRO.rich("LISTENING DNA", app.theme.accent));
                ui.label(Type::H2.rich("Your soundprint", app.theme.text));
                ui.label(Type::CAPTION.rich(
                    "The genres shaping your collection. Select a frequency to open its tracks.",
                    app.theme.text_dim,
                ));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.vertical(|ui| {
                    ui.label(Type::H3.rich(&tracks.len().to_string(), app.theme.text));
                    ui.label(Type::MICRO.rich("LIKED TRACKS", app.theme.text_dim));
                });
            });
        });
        ui.add_space(Metrics::SP_2);
        if shares.is_empty() {
            ui.label(Type::BODY.rich(
                "Like a few tracks with genre tags and your soundprint will appear here.",
                app.theme.text_dim,
            ));
            return;
        }

        let lane_width = ui.available_width();
        let lane_height = 54.0;
        let (lane, _) =
            ui.allocate_exact_size(egui::vec2(lane_width, lane_height), egui::Sense::hover());
        let colors = [
            app.theme.accent,
            egui::Color32::from_rgb(108, 126, 255),
            egui::Color32::from_rgb(68, 201, 173),
            egui::Color32::from_rgb(227, 91, 129),
            egui::Color32::from_rgb(244, 184, 72),
            egui::Color32::from_rgb(149, 98, 232),
            egui::Color32::from_rgb(85, 169, 235),
        ];
        let widths = super::soundprint::lane_widths(&shares, lane_width);
        let mut x = lane.left();
        for (index, (share, width)) in shares.iter().zip(widths).enumerate() {
            let width = width.min(lane.right() - x);
            if width <= 0.0 {
                break;
            }
            let segment = egui::Rect::from_min_size(
                egui::pos2(x, lane.top()),
                egui::vec2(width, lane_height),
            );
            let color = colors[index % colors.len()];
            ui.painter().rect_filled(
                segment.shrink2(egui::vec2(2.0, 0.0)),
                Metrics::RADIUS_INPUT,
                color.gamma_multiply(if app.theme.dark { 0.72 } else { 0.55 }),
            );
            let response = ui
                .interact(
                    segment,
                    ui.id().with(("soundprint", index)),
                    egui::Sense::click(),
                )
                .on_hover_text(format!("Open {} likes", share.genre))
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if response.hovered() {
                ui.painter().rect_stroke(
                    segment.shrink(1.5),
                    Metrics::RADIUS_INPUT,
                    egui::Stroke::new(1.5, color),
                    egui::StrokeKind::Inside,
                );
            }
            if response.clicked() {
                app.filter = share.genre.clone();
                app.library_tab = LibraryTab::Likes;
            }
            if width >= 132.0 {
                ui.painter().text(
                    segment.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{}  {}%", share.genre, (share.ratio * 100.0).round() as u32),
                    Type::CAPTION.font(),
                    egui::Color32::WHITE,
                );
            }
            x += width;
        }
        ui.add_space(Metrics::SP_1);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(Metrics::SP_1, Metrics::SP_HALF);
            for (index, share) in shares.iter().enumerate() {
                let color = colors[index % colors.len()];
                let response = ui.add(
                    egui::Button::new(Type::CAPTION.rich(
                        &format!("{}  {} tracks", share.genre, share.tracks),
                        app.theme.text,
                    ))
                    .fill(color.gamma_multiply(if app.theme.dark { 0.18 } else { 0.10 }))
                    .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.55)))
                    .corner_radius(Metrics::RADIUS_PILL),
                );
                if response.clicked() {
                    app.filter = share.genre.clone();
                    app.library_tab = LibraryTab::Likes;
                }
            }
        });
    });
    ui.add_space(Metrics::SP_2);
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
    let columns = library_columns(ui.available_width());
    for (row_index, row) in tracks.chunks(columns).enumerate() {
        ui.push_id(("likes-row", row_index), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(Metrics::SP_3, Metrics::SP_6);
                for (column, track) in row.iter().enumerate() {
                    let i = row_index * columns + column;
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.set_min_width(LIBRARY_CARD);
                        ui.set_max_width(LIBRARY_CARD);
                        let mut selected = app.selected.contains(&track.id);
                        if ui.checkbox(&mut selected, "Select track").changed() {
                            app.toggle_select(track.id);
                        }
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
                            app.play_user_queue(tracks.to_vec(), i, false);
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
                            app.play_user_queue(tracks.to_vec(), i, false);
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
    library_tab_intro(
        app,
        ui,
        super::icons::Icon::Queue,
        app.settings.language.text("Playlists", "Плейлисты"),
        app.settings.language.text(
            "Shape a listening path from your own mixes and saved collections.",
            "Собери свой маршрут из собственных миксов и сохранённых подборок.",
        ),
    );
    Surface::new(SurfaceTone::Default).show(ui, app.theme, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(Type::H4.rich("Create a playlist", app.theme.text));
                ui.label(Type::CAPTION.rich(
                    "Give it a name now; add tracks from any track menu.",
                    app.theme.text_dim,
                ));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if airwave::action_button(ui, app.theme, ButtonVariant::Primary, "Create").clicked()
                {
                    let name = app.new_playlist_name.trim().to_owned();
                    if name.is_empty() {
                        app.toast("Give the playlist a name first");
                    } else {
                        app.create_playlist(name.clone());
                        app.new_playlist_name.clear();
                        app.toast(format!("Creating playlist {name}…"));
                    }
                }
                ui.add(
                    egui::TextEdit::singleline(&mut app.new_playlist_name)
                        .hint_text("New playlist name…")
                        .desired_width(260.0),
                );
            });
        });
    });
    ui.add_space(Metrics::SP_2);
    ui.horizontal(|ui| {
        ui.label(Type::MICRO.rich("YOUR COLLECTION", app.theme.text_dim));
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
    library_tab_intro(
        app,
        ui,
        super::icons::Icon::Disc,
        app.settings.language.text("Albums", "Альбомы"),
        app.settings.language.text(
            "Long-form releases from your own catalogue and the records you saved.",
            "Альбомы из твоей коллекции и сохранённые релизы.",
        ),
    );
    ui.horizontal(|ui| {
        ui.label(Type::MICRO.rich("YOUR COLLECTION", app.theme.text_dim));
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
        airwave::state_panel(
            ui,
            app.theme,
            ViewState::Empty {
                title: "No albums saved yet",
                detail: "Save an album on SoundCloud and it will appear here as a full release, not a loose playlist.",
            },
        );
        footer(app, ui);
        return;
    }
    playlist_card_grid(app, ui, &items);
}

fn artists_tab(app: &mut App, ui: &mut egui::Ui) {
    library_tab_intro(
        app,
        ui,
        super::icons::Icon::Users,
        app.settings.language.text("Following", "Подписки"),
        app.settings.language.text(
            "Artists you chose to keep close, with their newest sounds one click away.",
            "Исполнители, на которых ты подписан, и их новые треки.",
        ),
    );
    ui.horizontal(|ui| {
        ui.label(Type::MICRO.rich("ARTISTS", app.theme.text_dim));
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
    const ARTIST_CARD: f32 = 148.0;
    let columns = ((ui.available_width() + Metrics::SP_3) / (ARTIST_CARD + Metrics::SP_3))
        .floor()
        .max(1.0) as usize;
    for (row_index, row) in artists.chunks(columns).enumerate() {
        ui.push_id(("following-row", row_index), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(Metrics::SP_3, Metrics::SP_6);
                for artist in row {
                    artist_card(app, ui, artist, ARTIST_CARD);
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
    library_tab_intro(
        app,
        ui,
        super::icons::Icon::AudioLines,
        app.settings.language.text("Stations", "Станции"),
        app.settings.language.text(
            "Start with one track and let related sounds carry the session forward.",
            "Начни с одного трека и слушай похожие дальше.",
        ),
    );
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
            let click = widgets::media_card(
                app,
                ui,
                widgets::MediaCard::plays(
                    seed.id,
                    &format!("{} Station", seed.title),
                    seed.artist(),
                )
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

fn detail_queue(track: &Track, related: &[Track]) -> Vec<Track> {
    let mut queue = Vec::with_capacity(related.len() + 1);
    queue.push(track.clone());
    queue.extend(related.iter().filter(|item| item.id != track.id).cloned());
    queue
}

fn playlist_title(app: &App, id: u64) -> String {
    if app.demo && id == 0 {
        return "Liked Songs".to_owned();
    }
    if app.demo
        && let Some(playlist) = app
            .settings
            .custom_playlists
            .iter()
            .find(|playlist| playlist.id == id)
    {
        return playlist.title.clone();
    }
    app.playlist(id)
        .ready()
        .map(|playlist| playlist.title)
        .unwrap_or_else(|| format!("Playlist {id}"))
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
    let is_own_track = app.account().is_some_and(|account| {
        track
            .user
            .as_ref()
            .is_some_and(|user| user.id == account.id)
    });

    ui.add_space(Metrics::SP_1);
    let art = track.artwork_url();
    widgets::media_hero(
        app,
        ui,
        widgets::MediaHero {
            artwork: art,
            seed: track.id,
            title: &track.title,
            round_artwork: false,
        },
        |app, ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(Type::MICRO.rich("NOW ON AIR  /  TRACK", app.theme.accent));
            ui.add_space(Metrics::SP_HALF);
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
            ui.horizontal_wrapped(|ui| {
                if airwave::action_button(ui, app.theme, ButtonVariant::Primary, "Play").clicked() {
                    app.play_user_queue(detail_queue(&track, &related), 0, false);
                }
                widgets::like_button(app, ui, track.id, &track.title);
                quick_access_button(
                    app,
                    ui,
                    QuickAccessShortcut::Track {
                        id: track.id,
                        title: track.title.clone(),
                        artist: track.artist().to_owned(),
                        artwork_url: track.artwork_url().map(str::to_owned),
                    },
                );
                let reposted = app
                    .tracks(Key::MyRepostedTracks)
                    .rows()
                    .iter()
                    .any(|item| item.id == track.id);
                if airwave::action_button(
                    ui,
                    app.theme,
                    ButtonVariant::Secondary,
                    if reposted { "Unrepost" } else { "Repost" },
                )
                .clicked()
                {
                    app.set_track_reposted(track.id, !reposted);
                }
                if airwave::action_button(ui, app.theme, ButtonVariant::Secondary, "Station")
                    .clicked()
                {
                    app.settings.autoplay = true;
                    app.player.set_autoplay(true);
                    let station = app.station_queue(&track);
                    app.play_user_queue(station, 0, false);
                    app.toast(format!("Station: {}", track.title));
                }
                if let Some(url) = track.permalink_url.clone() {
                    if airwave::action_button(ui, app.theme, ButtonVariant::Secondary, "Share")
                        .clicked()
                    {
                        ui.ctx().copy_text(url);
                        app.toast("Link copied");
                    }
                } else if airwave::action_button(ui, app.theme, ButtonVariant::Secondary, "Share")
                    .clicked()
                {
                    app.toast("This track has no public link");
                }
            });
        },
    );

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
    ui.add_space(Metrics::SP_2);
    Surface::glass().show(ui, app.theme, |ui| {
        ui.horizontal(|ui| {
            ui.label(Type::MICRO.rich("WAVEFORM  /  LISTEN & SEEK", app.theme.accent));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(Type::CAPTION.rich(
                    track.genre.as_deref().unwrap_or("SoundCloud track"),
                    app.theme.text_dim,
                ));
            });
        });
        ui.add_space(Metrics::SP_1);
        let peaks = track
            .waveform_url
            .as_deref()
            .and_then(|url| app.waveforms.get(ui.ctx(), url));
        if let Some(frac) = widgets::waveform(app, ui, id, pos, dur, 110.0, 140, peaks.as_deref()) {
            if is_current {
                app.player.seek_ms((frac as f64 * dur as f64) as u64);
            } else {
                app.play_user_queue(detail_queue(&track, &related), 0, false);
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
    Surface::glass()
        .padding(Metrics::SP_1 as i8)
        .show(ui, app.theme, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(Type::MICRO.rich("CREATOR TOOLS", app.theme.accent));
            ui.horizontal_wrapped(|ui| {
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

#[cfg(test)]
mod detail_queue_tests {
    use super::*;

    #[test]
    fn detail_queue_starts_with_the_track_and_omits_its_related_duplicate() {
        let track: Track = serde_json::from_str(r#"{"id":7,"title":"Focus"}"#).unwrap();
        let related: Vec<Track> = serde_json::from_str(
            r#"[{"id":8,"title":"Next"},{"id":7,"title":"Focus again"},{"id":9,"title":"Later"}]"#,
        )
        .unwrap();

        let ids: Vec<_> = detail_queue(&track, &related)
            .into_iter()
            .map(|item| item.id)
            .collect();

        assert_eq!(ids, [7, 8, 9]);
    }

    #[test]
    fn sticky_actions_only_appear_after_a_detail_hero_scrolls_away() {
        assert!(!should_show_detail_actions(&Route::TrackDetail(7), 219.0));
        assert!(should_show_detail_actions(&Route::PlaylistDetail(7), 220.0));
        assert!(should_show_detail_actions(&Route::UserDetail(7), 480.0));
        assert!(!should_show_detail_actions(&Route::Home, 480.0));
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
        .as_ref()
        .and_then(|playlist| playlist.user.as_ref())
        .map(|user| user.username.clone());
    let media_kind = if remote_playlist
        .as_ref()
        .is_some_and(|playlist| playlist.is_album())
    {
        "Album"
    } else {
        "Playlist"
    };
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

    widgets::media_hero(
        app,
        ui,
        widgets::MediaHero {
            artwork: art.as_deref(),
            seed: id + 7,
            title: &title,
            round_artwork: false,
        },
        |app, ui| {
            let kind = match owner.as_deref() {
                Some(who) => format!("{media_kind} · {who}"),
                None => media_kind.to_owned(),
            };
            ui.label(Type::MICRO.rich("YOUR COLLECTION  /  ON AIR", app.theme.accent));
            ui.label(Type::CAPTION.rich(&kind, app.theme.text_dim));
            ui.label(Type::H1.rich(&title, app.theme.text));
            ui.label(Type::CAPTION.rich(
                &format!(
                    "{} tracks · {}",
                    tracks.len(),
                    crate::util::fmt_duration_ms(total_ms)
                ),
                app.theme.text_dim,
            ));
            ui.add_space(Metrics::SP_1);
            if round_play_button(app, ui, 48.0) && !tracks.is_empty() {
                app.play_user_queue(tracks.clone(), 0, false);
            }
        },
    );

    let pinned_collection = if media_kind == "Album" {
        QuickAccessShortcut::Album {
            id,
            title: title.clone(),
            artist: owner.clone().unwrap_or_else(|| media_kind.to_owned()),
            artwork_url: art.clone(),
        }
    } else {
        QuickAccessShortcut::Playlist {
            id,
            title: title.clone(),
            artist: owner.clone().unwrap_or_else(|| media_kind.to_owned()),
            artwork_url: art.clone(),
        }
    };
    ui.add_space(Metrics::SP_1);
    Surface::glass()
        .padding(Metrics::SP_15 as i8)
        .show(ui, app.theme, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(Type::MICRO.rich("COLLECTION CONTROLS", app.theme.accent));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    stat(app, ui, super::icons::Icon::Heart, Some(total_likes));
                    stat(app, ui, super::icons::Icon::Music, Some(total_plays));
                });
            });
            ui.add_space(Metrics::SP_HALF);
            ui.horizontal_wrapped(|ui| {
                let mut deleted = false;
                if airwave::action_button(ui, app.theme, ButtonVariant::Primary, "Play all")
                    .clicked()
                    && !tracks.is_empty()
                {
                    app.play_user_queue(tracks.clone(), 0, false);
                }
                if airwave::action_button(ui, app.theme, ButtonVariant::Secondary, "Queue all")
                    .clicked()
                    && !tracks.is_empty()
                {
                    app.player.enqueue(tracks.clone(), false);
                    app.toast(format!("Queued {} tracks", tracks.len()));
                }
                quick_access_button(app, ui, pinned_collection.clone());
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
                    match remote_playlist
                        .as_ref()
                        .and_then(|playlist| playlist.permalink_url.clone())
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
                if deleted {
                    app.navigate(Route::Library);
                }
            });
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
    ui.add_space(Metrics::SP_2);
    Surface::glass()
        .padding(Metrics::SP_15 as i8)
        .show(ui, app.theme, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(Type::MICRO.rich("ARTIST CHANNELS", app.theme.accent));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::follow_button(app, ui, id, &name);
                    if airwave::action_button(ui, app.theme, ButtonVariant::Secondary, "Station")
                        .clicked()
                    {
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
                });
            });
            ui.add_space(Metrics::SP_HALF);
            ui.horizontal_wrapped(|ui| {
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
            });
        });
    ui.add_space(Metrics::SP_1);

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

fn playlist_carousel<'a>(
    app: &mut App,
    ui: &mut egui::Ui,
    salt: &str,
    playlists: impl IntoIterator<Item = &'a crate::api::models::Playlist>,
) {
    let playlists: Vec<_> = playlists.into_iter().collect();
    app.carousel(ui, salt, |app, ui| {
        let text_height = ui.fonts_mut(|fonts| {
            fonts.row_height(&Type::H4.font()) + fonts.row_height(&Type::BODY.font())
        });
        let size = egui::vec2(widgets::CARD_ART, widgets::CARD_ART + text_height + 4.0);
        widgets::virtual_card_strip(ui, playlists.len(), size, |ui, index| {
            let playlist = &playlists[index];
            let subtitle = format!(
                "{} • {} tracks",
                playlist
                    .user
                    .as_ref()
                    .map(|user| user.username.as_str())
                    .unwrap_or("Unknown creator"),
                playlist.track_count.unwrap_or(0)
            );
            let click = widgets::media_card(
                app,
                ui,
                widgets::MediaCard::opens(playlist.id, &playlist.title, &subtitle)
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
        });
    });
}

/// SoundCloud profile masthead: a tall colour field, large circular avatar,
/// and the account name on a dark label over it.
fn profile_hero(app: &mut App, ui: &mut egui::Ui, user: &crate::api::models::User) {
    let stats = format!(
        "{} followers  ·  {} following  ·  {} tracks",
        fmt_count(user.followers_count),
        fmt_count(user.followings_count),
        fmt_count(user.track_count),
    );
    widgets::media_hero(
        app,
        ui,
        widgets::MediaHero {
            artwork: user.avatar_url.as_deref(),
            seed: name_seed(&user.username),
            title: &user.username,
            round_artwork: true,
        },
        |app, ui| {
            ui.label(Type::CAPTION.rich("Artist", app.theme.text_dim));
            ui.label(Type::DISPLAY3.rich(&user.username, app.theme.text));
            ui.add_space(Metrics::SP_1);
            ui.label(Type::BODY.rich(&stats, app.theme.text_dim));
        },
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

fn catalog(app: &mut App, ui: &mut egui::Ui) {
    let language = app.settings.language;
    let surface = if app.settings.background_image.is_some() {
        Surface::clear()
    } else {
        Surface::glass()
    };
    surface.show(ui, app.theme, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(Type::MICRO.rich(
            language.text("EXPLORE THE CATALOG", "ИССЛЕДУЙ КАТАЛОГ"),
            app.theme.accent,
        ));
        ui.label(Type::H1.rich(language.text("Catalog", "Каталог"), app.theme.text));
        ui.label(Type::BODY.rich(
            language.text(
                "Find albums and artists across SoundCloud.",
                "Находи альбомы и исполнителей в SoundCloud.",
            ),
            app.theme.text_dim,
        ));
        ui.add_space(Metrics::SP_2);
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(!app.catalog_artists, language.text("Albums", "Альбомы"))
                .clicked()
            {
                app.catalog_artists = false;
            }
            if ui
                .selectable_label(app.catalog_artists, language.text("Artists", "Исполнители"))
                .clicked()
            {
                app.catalog_artists = true;
            }
            ui.add_space(Metrics::SP_1);
            let response = ui.add(
                egui::TextEdit::singleline(&mut app.catalog_query)
                    .hint_text(
                        language.text("Search albums or artists", "Поиск альбома или исполнителя"),
                    )
                    .desired_width(280.0),
            );
            if response.changed() {
                app.catalog_query_edited_at = Some(std::time::Instant::now());
            }
            let enter =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            if enter || ui.button(language.text("Search", "Найти")).clicked() {
                app.catalog_committed_query = app.catalog_query.trim().to_owned();
                app.catalog_query_edited_at = None;
            }
            if !app.catalog_query.is_empty() && ui.button("×").clicked() {
                app.catalog_query.clear();
                app.catalog_committed_query.clear();
                app.catalog_query_edited_at = None;
            }
        });
    });
    if let Some(edited_at) = app.catalog_query_edited_at {
        let wait = std::time::Duration::from_millis(220);
        if edited_at.elapsed() >= wait {
            app.catalog_committed_query = app.catalog_query.trim().to_owned();
            app.catalog_query_edited_at = None;
        } else {
            ui.ctx().request_repaint_after(wait - edited_at.elapsed());
        }
    }
    ui.add_space(Metrics::SP_3);
    if app.catalog_artists {
        let query = if app.catalog_committed_query.is_empty() {
            if app.demo {
                String::new()
            } else {
                "artist".to_owned()
            }
        } else {
            app.catalog_committed_query.clone()
        };
        let artists = app.users(Key::SearchUsers(query));
        if artists.is_loading() && artists.rows().is_empty() {
            ui.label(Type::BODY.rich(
                language.text("Loading artists…", "Загружаем исполнителей…"),
                app.theme.text_dim,
            ));
        } else if let Some(error) = artists.error() {
            ui.label(Type::BODY.rich(
                language.text(
                    "Could not load artists.",
                    "Не удалось загрузить исполнителей.",
                ),
                app.theme.text,
            ));
            ui.label(Type::CAPTION.rich(error, app.theme.text_dim));
        } else if artists.rows().is_empty() {
            ui.label(Type::BODY.rich(
                language.text(
                    "No artists found. Try another name.",
                    "Исполнители не найдены. Попробуй другое имя.",
                ),
                app.theme.text_dim,
            ));
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(Metrics::SP_3, Metrics::SP_4);
                for artist in artists.rows() {
                    artist_card(app, ui, artist, LIBRARY_CARD);
                }
            });
        }
    } else {
        let query = if app.catalog_committed_query.is_empty() {
            if app.demo {
                String::new()
            } else {
                "album".to_owned()
            }
        } else {
            app.catalog_committed_query.clone()
        };
        let playlists = app.playlists(Key::SearchPlaylists(query));
        let albums: Vec<_> = playlists
            .rows()
            .iter()
            .filter(|playlist| playlist.is_album())
            .collect();
        if playlists.is_loading() && albums.is_empty() {
            ui.label(Type::BODY.rich(
                language.text("Loading albums…", "Загружаем альбомы…"),
                app.theme.text_dim,
            ));
        } else if let Some(error) = playlists.error() {
            ui.label(Type::BODY.rich(
                language.text("Could not load albums.", "Не удалось загрузить альбомы."),
                app.theme.text,
            ));
            ui.label(Type::CAPTION.rich(error, app.theme.text_dim));
        } else if albums.is_empty() {
            ui.label(Type::BODY.rich(
                language.text(
                    "No albums found. Try another title.",
                    "Альбомы не найдены. Попробуй другое название.",
                ),
                app.theme.text_dim,
            ));
        } else {
            catalog_album_grid(app, ui, &albums);
        }
    }
    ui.add_space(Metrics::SP_3);
    if ui
        .button(language.text("Explore your personal wave →", "Открыть личную волну →"))
        .clicked()
    {
        app.navigate(Route::Discover);
    }
}

fn catalog_album_grid(app: &mut App, ui: &mut egui::Ui, albums: &[&crate::api::models::Playlist]) {
    let columns = library_columns(ui.available_width());
    for (row_index, row) in albums.chunks(columns).enumerate() {
        ui.push_id(("catalog-album-row", row_index), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(Metrics::SP_3, Metrics::SP_4);
                for album in row {
                    let mut open = false;
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.set_width(LIBRARY_CARD);
                        open |= widgets::artwork_img(
                            ui,
                            album.artwork_url(),
                            album.id + 7,
                            &album.title,
                            LIBRARY_CARD,
                            Metrics::RADIUS as f32,
                        )
                        .clicked();
                        open |= ui
                            .add(
                                egui::Label::new(Type::H4.rich(&album.title, app.theme.text))
                                    .sense(egui::Sense::click())
                                    .truncate(),
                            )
                            .clicked();
                        if let Some(owner) = &album.user {
                            ui.add(
                                egui::Label::new(
                                    Type::CAPTION.rich(&owner.username, app.theme.text_dim),
                                )
                                .truncate(),
                            );
                        }
                    });
                    if open {
                        app.navigate(Route::PlaylistDetail(album.id));
                    }
                }
            });
        });
    }
}

fn discover(app: &mut App, ui: &mut egui::Ui) {
    let wide = ui.available_width() >= 760.0;
    let surface = if app.settings.background_image.is_some() {
        Surface::clear()
    } else {
        Surface::glass()
    };
    let hero = surface
        .padding(Metrics::SP_3 as i8)
        .show(ui, app.theme, |ui| {
            if wide {
                ui.horizontal(|ui| {
                    let copy_width = (ui.available_width() - 286.0).max(320.0);
                    ui.vertical(|ui| {
                        ui.set_width(copy_width);
                        discover_copy_and_controls(app, ui);
                    });
                    ui.add_space(Metrics::SP_2);
                    discover_signal_preview(app, ui, egui::vec2(254.0, 176.0));
                });
            } else {
                discover_copy_and_controls(app, ui);
                ui.add_space(Metrics::SP_2);
                discover_signal_preview(app, ui, egui::vec2(ui.available_width(), 116.0));
            }
        });
    ui.painter().line_segment(
        [
            egui::pos2(
                hero.response.rect.left() + 24.0,
                hero.response.rect.top() + 1.0,
            ),
            egui::pos2(
                (hero.response.rect.left() + 260.0).min(hero.response.rect.right() - 24.0),
                hero.response.rect.top() + 1.0,
            ),
        ],
        egui::Stroke::new(1.5, app.theme.accent.gamma_multiply(0.8)),
    );
    ui.add_space(Metrics::SP_3);
    let surface = if app.settings.background_image.is_some() {
        Surface::clear()
    } else {
        Surface::glass()
    };
    surface
        .padding(Metrics::SP_15 as i8)
        .show(ui, app.theme, |ui| search_genre_ticker(app, ui));
    ui.add_space(Metrics::SP_3);
    ui.horizontal(|ui| {
        ui.label(
            Type::MICRO.rich(
                app.settings
                    .language
                    .text("PERSONAL FREQUENCY", "ЛИЧНАЯ ВОЛНА"),
                app.theme.accent,
            ),
        );
        ui.separator();
        ui.label(Type::H4.rich(
            app.settings.language.text("Your wave", "Твоя волна"),
            app.theme.text,
        ));
    });
    ui.label(Type::CAPTION.rich(
        app.settings.language.text(
            "A living mix tuned by your recent plays, likes and repeat listens.",
            "Живой микс из недавних прослушиваний, лайков и любимых треков.",
        ),
        app.theme.text_dim,
    ));
    ui.add_space(Metrics::SP_1);
    search_wave(app, ui);
}

fn discover_copy_and_controls(app: &mut App, ui: &mut egui::Ui) {
    let language = app.settings.language;
    ui.label(Type::MICRO.rich(
        language.text("AIRWAVE DISCOVERY  /  LIVE", "AIRWAVE / ПОИСК НОВОГО"),
        app.theme.accent,
    ));
    ui.add_space(Metrics::SP_HALF);
    ui.label(Type::DISPLAY3.rich(
        language.text("Find your next frequency", "Найди новую волну"),
        app.theme.text,
    ));
    ui.label(Type::BODY.rich(
        language.text(
            "Describe a mood, a place or a moment. Airwave turns it into a path through sound.",
            "Опиши настроение, место или момент — Airwave подберёт музыку.",
        ),
        app.theme.text_dim,
    ));
    ui.add_space(Metrics::SP_2);
    let input = egui::Frame::new()
        .fill(if app.settings.background_image.is_some() {
            app.theme.tokens.glass_clear
        } else {
            app.theme.tokens.glass
        })
        .stroke(egui::Stroke::new(1.0, app.theme.tokens.glass_border))
        .corner_radius(Metrics::RADIUS_INPUT)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.discover_query)
                    .id_salt("discover-vibe-query")
                    .hint_text(language.text(
                        "Late night ambient, energetic house…",
                        "Ночной эмбиент, энергичный хаус…",
                    ))
                    .font(Type::BODY.font())
                    .frame(egui::Frame::NONE)
                    .desired_width(ui.available_width()),
            )
        })
        .inner;
    let enter = input.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
    ui.add_space(Metrics::SP_1);
    ui.horizontal(|ui| {
        let ready = search_query_is_ready(&app.discover_query);
        let submit = ui
            .add_enabled_ui(ready, |ui| {
                airwave::action_button(
                    ui,
                    app.theme,
                    ButtonVariant::Primary,
                    language.text("Explore Vibe", "Искать по настроению"),
                )
                .clicked()
            })
            .inner;
        ui.label(Type::CAPTION.rich(
            language.text(
                "Mood + genre ranking · results come from SoundCloud",
                "По настроению и жанру · результаты из SoundCloud",
            ),
            app.theme.text_dim,
        ));
        if ready && (submit || enter) {
            app.search_mode = SearchMode::Vibe;
            app.search_query = app.discover_query.trim().to_owned();
            app.commit_search_now();
            app.navigate(Route::Search(app.search_query.clone()));
        }
    });
}

fn discover_signal_preview(app: &App, ui: &mut egui::Ui, size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    if app.settings.background_image.is_some() {
        airwave::paint_clear_glass_rect(
            ui,
            rect,
            Metrics::RADIUS_LG,
            app.theme,
            Some(app.theme.accent),
        );
    } else {
        airwave::paint_glass_rect(
            ui,
            rect,
            Metrics::RADIUS_LG,
            app.theme,
            false,
            Some(app.theme.accent),
        );
    }
    let center = rect.center();
    for (index, radius) in [30.0_f32, 52.0, 76.0].into_iter().enumerate() {
        ui.painter().circle_stroke(
            center,
            radius.min(rect.height() * 0.42),
            egui::Stroke::new(
                if index == 0 { 1.5 } else { 1.0 },
                app.theme.accent.gamma_multiply(0.48 - index as f32 * 0.12),
            ),
        );
    }
    let bars = [0.20_f32, 0.42, 0.68, 0.92, 0.58, 0.34, 0.72, 0.48, 0.26];
    let gap = 5.0;
    let bar_width = ((rect.width() * 0.58) - gap * (bars.len() - 1) as f32) / bars.len() as f32;
    let left = center.x - rect.width() * 0.29;
    for (index, amplitude) in bars.into_iter().enumerate() {
        let height = 54.0 * amplitude;
        let bar = egui::Rect::from_center_size(
            egui::pos2(left + index as f32 * (bar_width + gap), center.y),
            egui::vec2(bar_width.max(2.0), height),
        );
        ui.painter()
            .rect_filled(bar, bar_width * 0.5, app.theme.accent);
    }
    ui.painter().text(
        egui::pos2(center.x, rect.bottom() - 18.0),
        egui::Align2::CENTER_CENTER,
        "SIGNAL READY",
        Type::MICRO.font(),
        app.theme.text_dim,
    );
}

/// Search follows the donor app's state machine: the global field owns the
/// query, short input stays on a personalized wave, and controls only appear
/// once a debounced query is ready.
fn search(app: &mut App, ui: &mut egui::Ui) {
    let language = app.settings.language;
    let query = committed_search_query(app, ui.ctx());
    ui.add_space(Metrics::SP_1);
    let header = Surface::glass().show(ui, app.theme, |ui| {
        ui.horizontal(|ui| {
            let (icon_rect, _) =
                ui.allocate_exact_size(egui::Vec2::splat(48.0), egui::Sense::hover());
            ui.painter().rect_filled(
                icon_rect,
                Metrics::RADIUS_INPUT,
                app.theme.tokens.accent_soft,
            );
            super::icons::paint(
                ui,
                super::icons::Icon::Search,
                icon_rect,
                22.0,
                app.theme.accent,
            );
            ui.add_space(Metrics::SP_HALF);
            ui.vertical(|ui| {
                ui.label(Type::MICRO.rich(
                    language.text("EXPLORE  /  THE WHOLE FREQUENCY", "ПОИСК / ВСЯ МУЗЫКА"),
                    app.theme.accent,
                ));
                if search_query_is_ready(&query) {
                    ui.add(
                        egui::Label::new(Type::H2.rich(
                            &match language {
                                crate::config::Language::English => {
                                    format!("Search results for “{query}”")
                                }
                                crate::config::Language::Russian => {
                                    format!("Результаты поиска: «{query}»")
                                }
                            },
                            app.theme.text,
                        ))
                        .truncate(),
                    );
                    ui.label(Type::CAPTION.rich(
                        language.text(
                            "Tracks, people and moods connected to your search.",
                            "Треки, исполнители и настроения по твоему запросу.",
                        ),
                        app.theme.text_dim,
                    ));
                } else {
                    ui.label(Type::H2.rich(language.text("Search", "Поиск"), app.theme.text));
                    ui.label(Type::CAPTION.rich(
                        language.text(
                            "A fresh wave based on what you listen to.",
                            "Новая волна на основе твоей музыки.",
                        ),
                        app.theme.text_dim,
                    ));
                }
            });
        });
        if search_query_is_ready(&query) {
            ui.add_space(Metrics::SP_15);
            search_mode_switcher(app, ui);
        }
        ui.add_space(Metrics::SP_15);
        ui.separator();
        ui.add_space(Metrics::SP_HALF);
        search_genre_ticker(app, ui);
    });
    ui.painter().line_segment(
        [
            egui::pos2(
                header.response.rect.left() + 18.0,
                header.response.rect.top() + 1.0,
            ),
            egui::pos2(
                (header.response.rect.left() + 210.0).min(header.response.rect.right() - 18.0),
                header.response.rect.top() + 1.0,
            ),
        ],
        egui::Stroke::new(1.4, app.theme.accent.gamma_multiply(0.75)),
    );
    ui.add_space(Metrics::SP_3);

    match search_surface(&query, app.search_mode) {
        SearchSurface::Wave => search_wave(app, ui),
        SearchSurface::Text => text_search(app, ui, &query),
        SearchSurface::Vibe => vibe_search(app, ui, &query),
        SearchSurface::SoundCloud => soundcloud_search(app, ui, &query),
    }
}

pub(super) fn search_query_is_ready(query: &str) -> bool {
    query.trim().chars().count() >= 2
}

fn search_surface(query: &str, mode: SearchMode) -> SearchSurface {
    if !search_query_is_ready(query) {
        return SearchSurface::Wave;
    }
    match mode {
        SearchMode::Text => SearchSurface::Text,
        SearchMode::Vibe => SearchSurface::Vibe,
        SearchMode::SoundCloud => SearchSurface::SoundCloud,
    }
}

fn next_search_selection(current: usize, result_count: usize, direction: SearchMove) -> usize {
    if result_count == 0 {
        return 0;
    }
    let current = current.min(result_count - 1);
    match direction {
        SearchMove::Previous => current.checked_sub(1).unwrap_or(result_count - 1),
        SearchMove::Next => (current + 1) % result_count,
    }
}

fn committed_search_query(app: &mut App, ctx: &egui::Context) -> String {
    const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(350);
    let typed = app.search_query.trim().to_owned();
    if typed == app.search_committed_query {
        app.search_query_edited_at = None;
        return typed;
    }

    let edited_at = app
        .search_query_edited_at
        .get_or_insert_with(std::time::Instant::now);
    let elapsed = edited_at.elapsed();
    if elapsed >= DEBOUNCE {
        app.search_committed_query = typed.clone();
        app.search_query_edited_at = None;
        app.remember_search();
        typed
    } else {
        ctx.request_repaint_after(DEBOUNCE - elapsed);
        app.search_committed_query.clone()
    }
}

fn search_mode_switcher(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(Type::CAPTION.rich(
            app.settings.language.text("Search with", "Режим поиска"),
            app.theme.text_dim,
        ));
        if let Some(mode) =
            search_mode_picker(ui, &app.theme, app.search_mode, app.settings.language).inner
        {
            app.search_mode = mode;
            app.search_selection = 0;
        }
    });
}

fn search_mode_picker(
    ui: &mut egui::Ui,
    theme: &super::theme::Theme,
    current: SearchMode,
    language: crate::config::Language,
) -> egui::InnerResponse<Option<SearchMode>> {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = Metrics::SP_075;
        let mut chosen = None;
        for (label, mode) in [
            (language.text("Text", "Текст"), SearchMode::Text),
            (language.text("✦ Vibe", "✦ Настроение"), SearchMode::Vibe),
            ("SoundCloud", SearchMode::SoundCloud),
        ] {
            let active = current == mode;
            let response = ui.add(
                egui::Button::new(
                    Type::CAPTION.rich(label, if active { theme.accent } else { theme.text_dim }),
                )
                .fill(if active {
                    theme.tokens.accent_soft
                } else {
                    theme.tokens.glass
                })
                .stroke(egui::Stroke::new(
                    1.0,
                    if active {
                        theme.accent.gamma_multiply(0.65)
                    } else {
                        theme.tokens.glass_border
                    },
                ))
                .corner_radius(Metrics::RADIUS_PILL)
                .min_size(egui::vec2(92.0, 30.0)),
            );
            if response.clicked() {
                chosen = Some(mode);
            }
        }
        chosen
    })
}

fn search_genre_ticker(app: &mut App, ui: &mut egui::Ui) {
    ui.label(
        Type::CAPTION.rich(
            app.settings
                .language
                .text("Jump into a vibe", "Выбери настроение"),
            app.theme.text_dim,
        ),
    );
    egui::ScrollArea::horizontal()
        .id_salt("search-genre-ticker")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = Metrics::SP_075;
                for genre in crate::demo::demo_genres().into_iter().skip(1) {
                    if ui
                        .add(
                            egui::Button::new(Type::CAPTION.rich(genre, app.theme.text))
                                .fill(app.theme.surface_hover)
                                .stroke(egui::Stroke::new(1.0, app.theme.separator))
                                .corner_radius(Metrics::RADIUS_PILL),
                        )
                        .clicked()
                    {
                        app.search_mode = SearchMode::Vibe;
                        app.search_selection = 0;
                        app.search_query = genre.to_owned();
                        app.commit_search_now();
                        app.navigate(Route::Search(app.search_query.clone()));
                    }
                }
            });
        });
}

fn search_wave(app: &mut App, ui: &mut egui::Ui) {
    let seeds = recent_tracks(app, 8);
    let mut lanes = Vec::new();
    let mut loading = false;
    let mut first_error = None;

    for seed in seeds.iter().take(4) {
        let slot = app.tracks(Key::Related(seed.id));
        loading |= slot.is_loading();
        if first_error.is_none() {
            first_error = slot.error().map(str::to_owned);
        }
        if !slot.rows().is_empty() {
            lanes.push(slot.rows().to_vec());
        }
    }

    let mut genres: Vec<String> = seeds
        .iter()
        .filter_map(|track| track.genre.as_deref())
        .map(str::trim)
        .filter(|genre| !genre.is_empty())
        .map(str::to_owned)
        .collect();
    genres.sort_by_key(|genre| genre.to_lowercase());
    genres.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    for fallback in ["Alternative Rock", "Electronic", "Hip-hop & Rap", "Ambient"] {
        if genres.len() >= 4 {
            break;
        }
        if !genres
            .iter()
            .any(|genre| genre.eq_ignore_ascii_case(fallback))
        {
            genres.push(fallback.to_owned());
        }
    }
    for genre in genres.into_iter().take(4) {
        let slot = app.tracks(Key::Genre(genre));
        loading |= slot.is_loading();
        if first_error.is_none() {
            first_error = slot.error().map(str::to_owned);
        }
        if !slot.rows().is_empty() {
            lanes.push(slot.rows().to_vec());
        }
    }

    if lanes.is_empty() && seeds.is_empty() {
        let likes = app.tracks(Key::Likes);
        loading |= likes.is_loading();
        if !likes.rows().is_empty() {
            lanes.push(likes.rows().to_vec());
        }
    }
    let rows = interleave_discovery_lanes(lanes);

    if rows.is_empty() {
        if loading {
            search_status(app, ui, "Building your wave…", None);
        } else if let Some(error) = first_error.as_deref() {
            search_status(app, ui, "Your wave could not load.", Some(error));
        } else {
            search_status(app, ui, "Play or like something to shape your wave.", None);
        }
        return;
    }

    discover_results(app, ui, &rows, loading);
}

fn interleave_discovery_lanes(lanes: Vec<Vec<Track>>) -> Vec<Track> {
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let longest = lanes.iter().map(Vec::len).max().unwrap_or_default();
    for index in 0..longest {
        for lane in &lanes {
            if let Some(track) = lane.get(index)
                && seen.insert(track.id)
            {
                rows.push(track.clone());
            }
        }
    }
    rows
}

fn discover_results(app: &mut App, ui: &mut egui::Ui, rows: &[Track], loading: bool) {
    const BATCH: usize = 24;
    let id = ui.id().with(("discover-visible", app.search_wave_seed));
    let visible = ui
        .ctx()
        .data(|data| data.get_temp::<usize>(id))
        .unwrap_or(BATCH)
        .clamp(1, rows.len());
    let shown = &rows[..visible];

    app.search_selection = app.search_selection.min(shown.len() - 1);
    let keyboard = search_keyboard_action(app, ui, shown.len());
    if keyboard.activate {
        app.play_user_queue(rows.to_vec(), app.search_selection, false);
    }
    ui.label(Type::H3.rich("Top result", app.theme.text));
    ui.add_space(Metrics::SP_1);
    search_top_result(
        app,
        ui,
        &shown[0],
        rows,
        app.search_selection == 0,
        keyboard.moved,
    );
    if shown.len() > 1 {
        ui.add_space(Metrics::SP_3);
        ui.label(Type::H3.rich("Keep exploring", app.theme.text));
        ui.add_space(Metrics::SP_1);
        vibe_wall(app, ui, shown, 1, keyboard.moved);
    }

    let (sentinel, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 42.0), egui::Sense::hover());
    if visible < rows.len() {
        ui.painter().text(
            sentinel.center(),
            egui::Align2::CENTER_CENTER,
            "Loading more frequencies…",
            Type::CAPTION.font(),
            app.theme.text_dim,
        );
        if ui.is_rect_visible(sentinel) {
            ui.ctx().data_mut(|data| {
                data.insert_temp(id, (visible + BATCH).min(rows.len()));
            });
            ui.ctx().request_repaint();
        }
    } else if loading {
        ui.painter().text(
            sentinel.center(),
            egui::Align2::CENTER_CENTER,
            "Tuning more sources…",
            Type::CAPTION.font(),
            app.theme.text_dim,
        );
    }
}

fn text_mix_plan(lexical: usize, vibe: usize) -> Vec<TextLane> {
    let mut plan = Vec::with_capacity(lexical + vibe);
    let (mut lexical_index, mut vibe_index) = (0, 0);
    while lexical_index < lexical || vibe_index < vibe {
        let vibe_slot = plan.len() % 7 == 5;
        if vibe_index < vibe && (vibe_slot || lexical_index >= lexical) {
            plan.push(TextLane::Vibe(vibe_index));
            vibe_index += 1;
        } else if lexical_index < lexical {
            plan.push(TextLane::Lexical(lexical_index));
            lexical_index += 1;
        }
    }
    plan
}

fn text_search(app: &mut App, ui: &mut egui::Ui, query: &str) {
    let lexical = app.tracks(Key::SearchTracks(query.to_owned()));
    let playlists = app.playlists(Key::SearchPlaylists(query.to_owned()));
    let users = app.users(Key::SearchUsers(query.to_owned()));

    let profile = super::vibe::profile(query);
    let mut vibe_candidates = Vec::new();
    let mut vibe_loading = false;
    for genre in profile.genres.iter().take(2) {
        let slot = app.tracks(Key::Genre(genre.clone()));
        vibe_loading |= slot.is_loading();
        vibe_candidates.extend(slot.rows().iter().cloned());
    }
    let vibe_rows = super::vibe::rank(&profile, vibe_candidates);
    let lexical_rows = lexical.rows();
    let mut seen = std::collections::HashSet::new();
    let mut rows = Vec::new();
    for lane in text_mix_plan(lexical_rows.len(), vibe_rows.len().min(8)) {
        let track = match lane {
            TextLane::Lexical(index) => lexical_rows.get(index),
            TextLane::Vibe(index) => vibe_rows.get(index),
        };
        if let Some(track) = track
            && seen.insert(track.id)
        {
            rows.push(track.clone());
        }
    }

    search_entity_strip(app, ui, playlists.rows(), users.rows());
    if !playlists.rows().is_empty() || !users.rows().is_empty() {
        ui.add_space(Metrics::SP_3);
    }
    if rows.is_empty() {
        if lexical.is_loading() || vibe_loading {
            search_status(app, ui, "Searching tracks…", None);
        } else if let Some(error) = lexical.error() {
            search_status(app, ui, "Text search failed.", Some(error));
        } else {
            search_status(app, ui, "No results.", None);
        }
        return;
    }
    search_results(app, ui, &rows);
}

fn vibe_search(app: &mut App, ui: &mut egui::Ui, query: &str) {
    let profile = super::vibe::profile(query);
    let mut candidates = Vec::new();
    let mut loading = false;
    let mut first_error = None;
    for genre in &profile.genres {
        let slot = app.tracks(Key::Genre(genre.clone()));
        loading |= slot.is_loading();
        if first_error.is_none() {
            first_error = slot.error().map(str::to_owned);
        }
        candidates.extend(slot.rows().iter().cloned());
    }
    let mut rows = super::vibe::rank(&profile, candidates);

    // Fastcloud has no Qdrant endpoint. Keep the donor's Vibe-only surface,
    // but use lexical SoundCloud results as a graceful fallback for unknown
    // descriptions rather than showing a dead page.
    if rows.is_empty() && !loading {
        let fallback = app.tracks(Key::SearchTracks(query.to_owned()));
        loading |= fallback.is_loading();
        if first_error.is_none() {
            first_error = fallback.error().map(str::to_owned);
        }
        rows.extend(fallback.rows().iter().cloned());
    }

    if rows.is_empty() {
        search_status(
            app,
            ui,
            if loading {
                "Finding that vibe…"
            } else {
                "No tracks matched this vibe."
            },
            first_error.as_deref(),
        );
        return;
    }
    search_results(app, ui, &rows);
}

fn soundcloud_search(app: &mut App, ui: &mut egui::Ui, query: &str) {
    let tracks = app.tracks(Key::SearchTracks(query.to_owned()));
    let playlists = app.playlists(Key::SearchPlaylists(query.to_owned()));
    let users = app.users(Key::SearchUsers(query.to_owned()));

    search_entity_strip(app, ui, playlists.rows(), users.rows());
    if !playlists.rows().is_empty() || !users.rows().is_empty() {
        ui.add_space(Metrics::SP_3);
    }
    if tracks.rows().is_empty() {
        if tracks.is_loading() || playlists.is_loading() || users.is_loading() {
            search_status(app, ui, "Searching SoundCloud…", None);
        } else if let Some(error) = tracks
            .error()
            .or_else(|| playlists.error())
            .or_else(|| users.error())
        {
            search_status(app, ui, "SoundCloud search failed.", Some(error));
        } else {
            search_status(app, ui, "No results.", None);
        }
        return;
    }
    search_results(app, ui, tracks.rows());
}

fn search_entity_strip(
    app: &mut App,
    ui: &mut egui::Ui,
    playlists: &[crate::api::models::Playlist],
    users: &[crate::api::models::User],
) {
    if playlists.is_empty() && users.is_empty() {
        return;
    }
    egui::ScrollArea::horizontal()
        .id_salt("search-entities")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = Metrics::SP_1;
                for user in users.iter().take(6) {
                    let response = search_entity_card(
                        app,
                        ui,
                        user.avatar_url.as_deref(),
                        user.id,
                        &user.username,
                        "Artist",
                        true,
                    );
                    if response.clicked() {
                        app.navigate(Route::UserDetail(user.id));
                    }
                }
                for playlist in playlists.iter().take(6) {
                    let kind = if playlist.is_album() {
                        "Album"
                    } else {
                        "Playlist"
                    };
                    let response = search_entity_card(
                        app,
                        ui,
                        playlist.artwork_url(),
                        playlist.id,
                        &playlist.title,
                        kind,
                        false,
                    );
                    if response.clicked() {
                        app.navigate(Route::PlaylistDetail(playlist.id));
                    }
                }
            });
        });
}

fn search_entity_card(
    app: &mut App,
    ui: &mut egui::Ui,
    image: Option<&str>,
    id: u64,
    title: &str,
    kind: &str,
    round: bool,
) -> egui::Response {
    egui::Frame::new()
        .fill(app.theme.surface.gamma_multiply(0.9))
        .stroke(egui::Stroke::new(1.0, app.theme.separator))
        .corner_radius(Metrics::RADIUS_LG)
        .inner_margin(egui::Margin::same(Metrics::SP_1 as i8))
        .show(ui, |ui| {
            ui.set_width(176.0);
            ui.horizontal(|ui| {
                let art = widgets::artwork_img(
                    ui,
                    image,
                    id,
                    title,
                    46.0,
                    if round { 23.0 } else { Metrics::RADIUS as f32 },
                );
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(Type::H5.rich(title, app.theme.text)).truncate());
                    ui.label(Type::CAPTION.rich(kind, app.theme.text_dim));
                });
                art
            })
            .response
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn search_status(app: &App, ui: &mut egui::Ui, message: &str, detail: Option<&str>) {
    egui::Frame::new()
        .fill(app.theme.surface.gamma_multiply(0.82))
        .stroke(egui::Stroke::new(1.0, app.theme.separator))
        .corner_radius(Metrics::RADIUS_LG)
        .inner_margin(egui::Margin::same(Metrics::SP_3 as i8))
        .show(ui, |ui| {
            ui.label(Type::BODY.rich(message, app.theme.text));
            if let Some(detail) = detail {
                ui.label(Type::CAPTION.rich(detail, app.theme.text_dim));
            }
        });
}
#[derive(Default)]
struct SearchKeyboardAction {
    moved: bool,
    activate: bool,
}

fn search_keyboard_action(
    app: &mut App,
    ui: &mut egui::Ui,
    result_count: usize,
) -> SearchKeyboardAction {
    if result_count == 0 || ui.ctx().egui_wants_keyboard_input() {
        return SearchKeyboardAction::default();
    }

    let (direction, activate) = ui.input_mut(|input| {
        let direction = if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
            || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
        {
            Some(SearchMove::Next)
        } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
            || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
        {
            Some(SearchMove::Previous)
        } else {
            None
        };
        let activate =
            direction.is_none() && input.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
        (direction, activate)
    });

    if let Some(direction) = direction {
        app.search_selection = next_search_selection(app.search_selection, result_count, direction);
    }
    SearchKeyboardAction {
        moved: direction.is_some(),
        activate,
    }
}

fn search_results(app: &mut App, ui: &mut egui::Ui, rows: &[Track]) {
    let rows = &rows[..rows.len().min(48)];
    let Some(top) = rows.first() else {
        return;
    };

    app.search_selection = app.search_selection.min(rows.len() - 1);
    let keyboard = search_keyboard_action(app, ui, rows.len());
    if keyboard.activate {
        app.play_user_queue(rows.to_vec(), app.search_selection, false);
    }

    ui.label(Type::H3.rich("Top result", app.theme.text));
    ui.add_space(Metrics::SP_1);
    search_top_result(
        app,
        ui,
        top,
        rows,
        app.search_selection == 0,
        keyboard.moved,
    );

    if rows.len() > 1 {
        ui.add_space(Metrics::SP_3);
        ui.label(Type::H3.rich("More results", app.theme.text));
        ui.add_space(Metrics::SP_1);
        vibe_wall(app, ui, rows, 1, keyboard.moved);
    }
}

fn search_top_result(
    app: &mut App,
    ui: &mut egui::Ui,
    track: &Track,
    queue: &[Track],
    selected: bool,
    reveal_selection: bool,
) {
    let mut play = false;
    let mut open = false;
    let response = Surface::glass()
        .show(ui, app.theme, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let show_signal = ui.available_width() >= 800.0;
                let art_size = if ui.available_width() >= 620.0 {
                    116.0
                } else {
                    88.0
                };
                if widgets::artwork_img(
                    ui,
                    track.artwork_url(),
                    track.id,
                    &track.title,
                    art_size,
                    Metrics::RADIUS_LG as f32,
                )
                .clicked()
                {
                    open = true;
                }
                ui.add_space(Metrics::SP_2);
                ui.vertical(|ui| {
                    if show_signal {
                        ui.set_width(ui.available_width().min(480.0));
                    }
                    ui.label(Type::MICRO.rich("TOP RESULT", app.theme.accent));
                    ui.add(
                        egui::Label::new(Type::H2.rich(&track.title, app.theme.text)).truncate(),
                    );
                    ui.label(Type::BODY.rich(track.artist(), app.theme.text_dim));
                    if let Some(genre) = track.genre.as_deref() {
                        let genre = format!("# {genre}");
                        ui.label(Type::CAPTION.rich(&genre, app.theme.text_dim));
                    }
                    ui.add_space(Metrics::SP_1);
                    ui.horizontal(|ui| {
                        play =
                            airwave::action_button(ui, app.theme, ButtonVariant::Primary, "Play")
                                .clicked();
                        open =
                            airwave::action_button(ui, app.theme, ButtonVariant::Secondary, "Open")
                                .clicked()
                                || open;
                    });
                });
                if show_signal {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        paint_search_signal_preview(app, ui, track);
                    });
                }
            });
        })
        .response;

    if selected && reveal_selection {
        paint_search_selection(ui, response.rect, app.theme);
        response.scroll_to_me(Some(egui::Align::Center));
    }
    if play {
        app.search_selection = 0;
        app.play_user_queue(queue.to_vec(), 0, false);
    }
    if open {
        app.search_selection = 0;
        app.navigate(Route::TrackDetail(track.id));
    }
}

fn paint_search_signal_preview(app: &App, ui: &mut egui::Ui, track: &Track) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(176.0, 108.0), egui::Sense::hover());
    airwave::paint_glass_rect(
        ui,
        rect,
        Metrics::RADIUS_INPUT,
        app.theme,
        false,
        Some(app.theme.accent),
    );
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, rect.top() + 12.0),
        egui::Align2::LEFT_TOP,
        "PREVIEW SIGNAL",
        Type::MICRO.font(),
        app.theme.text_dim,
    );
    let bars = widgets::wave_bars(track.id, 30);
    let baseline = rect.center().y + 5.0;
    let bar_width = 3.0;
    let gap = 2.0;
    for (index, amplitude) in bars.into_iter().enumerate() {
        let height = 31.0 * amplitude.max(0.14);
        let bar = egui::Rect::from_min_size(
            egui::pos2(
                rect.left() + 12.0 + index as f32 * (bar_width + gap),
                baseline - height,
            ),
            egui::vec2(bar_width, height * 1.35),
        );
        ui.painter()
            .rect_filled(bar, bar_width * 0.5, app.theme.accent.gamma_multiply(0.85));
    }
    ui.painter().text(
        egui::pos2(rect.right() - 12.0, rect.bottom() - 12.0),
        egui::Align2::RIGHT_BOTTOM,
        crate::util::fmt_duration_ms(track.effective_duration_ms()),
        Type::CAPTION.font(),
        app.theme.text_dim,
    );
}

fn paint_search_selection(ui: &egui::Ui, rect: egui::Rect, theme: super::theme::Theme) {
    ui.painter().rect_stroke(
        rect.shrink(1.0),
        Metrics::RADIUS_LG,
        egui::Stroke::new(1.5, theme.tokens.accent.gamma_multiply(0.72)),
        egui::StrokeKind::Inside,
    );
}

/// Cover-first result wall with stable cells and metadata painted on the art.
fn vibe_wall(
    app: &mut App,
    ui: &mut egui::Ui,
    rows: &[Track],
    start_index: usize,
    reveal_selection: bool,
) {
    const GAP: f32 = 12.0;
    let available = ui.available_width().max(1.0);
    let target = if available >= 1_050.0 { 198.0 } else { 172.0 };
    let columns = (((available + GAP) / (target + GAP)).floor() as usize).clamp(2, 6);
    let edge =
        ((available - GAP * columns.saturating_sub(1) as f32) / columns as f32).clamp(112.0, 224.0);
    let queue = rows;
    let wall = &queue[start_index.min(queue.len())..];
    let mut play_index = None;

    for (row_index, chunk) in wall.chunks(columns).enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for (column, track) in chunk.iter().enumerate() {
                let index = start_index + row_index * columns + column;
                let response = widgets::artwork_img(
                    ui,
                    track.artwork_url(),
                    track.id,
                    &track.title,
                    edge,
                    Metrics::RADIUS_LG as f32,
                );
                let overlay = egui::Rect::from_min_max(
                    egui::pos2(response.rect.left(), response.rect.bottom() - edge * 0.32),
                    response.rect.max,
                );
                ui.painter().rect_filled(
                    overlay,
                    egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: Metrics::RADIUS_LG,
                        se: Metrics::RADIUS_LG,
                    },
                    egui::Color32::from_black_alpha(184),
                );
                let text_left = overlay.left() + Metrics::SP_1;
                ui.painter().text(
                    egui::pos2(text_left, overlay.top() + Metrics::SP_075),
                    egui::Align2::LEFT_TOP,
                    truncate(&track.title, (edge / 8.0).max(12.0) as usize),
                    Type::H5.font(),
                    egui::Color32::WHITE,
                );
                ui.painter().text(
                    egui::pos2(text_left, overlay.bottom() - Metrics::SP_075),
                    egui::Align2::LEFT_BOTTOM,
                    truncate(track.artist(), (edge / 9.0).max(10.0) as usize),
                    Type::CAPTION.font(),
                    egui::Color32::from_white_alpha(190),
                );
                let selected = index == app.search_selection;
                if response.hovered() || selected {
                    paint_search_selection(ui, response.rect, app.theme);
                }
                if selected && reveal_selection {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                if response.clicked() {
                    app.search_selection = index;
                    play_index = Some(index);
                }
                if response.double_clicked() {
                    app.navigate(Route::TrackDetail(track.id));
                }
            }
        });
        ui.add_space(GAP);
    }

    if let Some(index) = play_index {
        app.play_user_queue(queue.to_vec(), index, false);
    }
}

fn recent(app: &mut App, ui: &mut egui::Ui) {
    history_tab(app, ui);
}

fn history_tab(app: &mut App, ui: &mut egui::Ui) {
    library_tab_intro(
        app,
        ui,
        super::icons::Icon::Clock,
        app.settings
            .language
            .text("Listening history", "История прослушивания"),
        app.settings.language.text(
            "Return to recent discoveries without rebuilding the queue from memory.",
            "Возвращайся к недавним находкам без поиска по памяти.",
        ),
    );
    ui.horizontal(|ui| {
        ui.label(Type::MICRO.rich("RECENTLY PLAYED", app.theme.text_dim));
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum SettingsSection {
    Appearance,
    Playback,
    Audio,
    Cache,
    Integrations,
    Shortcuts,
    Account,
    About,
}

impl SettingsSection {
    const ALL: [Self; 8] = [
        Self::Account,
        Self::Appearance,
        Self::Playback,
        Self::Audio,
        Self::Cache,
        Self::Integrations,
        Self::Shortcuts,
        Self::About,
    ];

    fn label(self, language: crate::config::Language) -> &'static str {
        match self {
            Self::Account => language.text("Account", "Аккаунт"),
            Self::Appearance => language.text("Appearance", "Оформление"),
            Self::Playback => language.text("Playback", "Воспроизведение"),
            Self::Audio => language.text("Audio & Equalizer", "Звук и эквалайзер"),
            Self::Cache => language.text("Cache & Memory", "Кэш и память"),
            Self::Integrations => language.text("Integrations", "Интеграции"),
            Self::Shortcuts => language.text("Shortcuts", "Горячие клавиши"),
            Self::About => language.text("About", "О приложении"),
        }
    }

    fn icon(self) -> super::icons::Icon {
        match self {
            Self::Appearance => super::icons::Icon::Sparkles,
            Self::Playback => super::icons::Icon::CirclePlay,
            Self::Audio => super::icons::Icon::AudioLines,
            Self::Cache => super::icons::Icon::Disc,
            Self::Integrations => super::icons::Icon::External,
            Self::Shortcuts => super::icons::Icon::List,
            Self::Account => super::icons::Icon::User,
            Self::About => super::icons::Icon::Info,
        }
    }
}

fn settings_navigation(app: &App, ui: &mut egui::Ui) -> SettingsSection {
    let active_id = egui::Id::new("settings-active-section");
    let active = ui
        .data_mut(|data| data.get_temp::<SettingsSection>(active_id))
        .unwrap_or(SettingsSection::Account);
    ui.set_width(232.0);
    let rail_height = ui.available_height();
    if app.settings.background_image.is_some() {
        Surface::clear()
    } else {
        Surface::glass()
    }
    .padding(Metrics::SP_15 as i8)
    .show(ui, app.theme, |ui| {
        ui.set_min_height((rail_height - Metrics::SP_15 * 2.0).max(0.0));
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(34.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, Metrics::RADIUS_INPUT, app.theme.tokens.accent_soft);
            super::icons::paint(
                ui,
                super::icons::Icon::Settings,
                rect,
                17.0,
                app.theme.accent,
            );
            ui.vertical(|ui| {
                ui.label(Type::MICRO.rich(
                    app.settings.language.text("SETTINGS", "НАСТРОЙКИ"),
                    app.theme.accent,
                ));
                ui.label(Type::CAPTION.rich("Fastcloud", app.theme.text_dim));
            });
        });
        ui.add_space(Metrics::SP_1);
        for section in SettingsSection::ALL {
            let label = section.label(app.settings.language);
            let selected = section == active;
            let (rect, response) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 46.0), egui::Sense::click());
            if ui.is_rect_visible(rect) {
                if selected || response.hovered() {
                    airwave::paint_glass_rect(
                        ui,
                        rect,
                        Metrics::RADIUS_INPUT,
                        app.theme,
                        selected,
                        selected.then_some(app.theme.accent),
                    );
                }
                if selected {
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            rect.left_center() - egui::vec2(0.0, 10.0),
                            egui::vec2(3.0, 20.0),
                        ),
                        Metrics::RADIUS_PILL,
                        app.theme.accent,
                    );
                }
                let icon_rect = egui::Rect::from_center_size(
                    egui::pos2(rect.left() + 24.0, rect.center().y),
                    egui::Vec2::splat(18.0),
                );
                super::icons::paint(
                    ui,
                    section.icon(),
                    icon_rect,
                    17.0,
                    if selected {
                        app.theme.accent
                    } else {
                        app.theme.text_dim
                    },
                );
                ui.painter().text(
                    egui::pos2(rect.left() + 43.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    Type::CAPTION.font(),
                    if selected {
                        app.theme.text
                    } else {
                        app.theme.text_dim
                    },
                );
            }
            if response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                ui.data_mut(|data| data.insert_temp(active_id, section));
            }
            ui.add_space(2.0);
        }
    });
    ui.data(|data| data.get_temp::<SettingsSection>(active_id))
        .unwrap_or(SettingsSection::Account)
}

fn settings_content_frame<R>(
    app: &mut App,
    ui: &mut egui::Ui,
    section: SettingsSection,
    content: impl FnOnce(&mut App, &mut egui::Ui) -> R,
) -> R {
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt(("settings-content", section))
        .wheel_scroll_multiplier(egui::vec2(1.0, 1.85))
        .auto_shrink([false, false])
        .max_height(ui.available_height());
    if let Some(offset) = app.screenshot_scroll_offset.take() {
        scroll = scroll.vertical_scroll_offset(offset);
    }
    scroll
        .show(ui, |ui| ui.vertical(|ui| content(app, ui)).inner)
        .inner
}

fn settings_section_rule(app: &App, ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.label(Type::MICRO.rich(label, app.theme.accent));
        let rect = ui.available_rect_before_wrap();
        ui.painter().hline(
            rect.x_range(),
            rect.center().y,
            egui::Stroke::new(1.0, app.theme.separator),
        );
        ui.allocate_space(egui::vec2(rect.width(), 1.0));
    });
    ui.add_space(Metrics::SP_HALF);
}

fn settings_section_card<R>(
    app: &mut App,
    ui: &mut egui::Ui,
    label: &str,
    content: impl FnOnce(&mut App, &mut egui::Ui) -> R,
) -> R {
    let surface = if app.settings.background_image.is_some() {
        Surface::clear()
    } else {
        Surface::glass()
    };
    surface
        .padding(Metrics::SP_3 as i8)
        .show(ui, app.theme, |ui| {
            settings_section_rule(app, ui, label);
            content(app, ui)
        })
        .inner
}

fn settings(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_top(|ui| {
        let active = ui.vertical(|ui| settings_navigation(app, ui)).inner;
        ui.add_space(Metrics::SP_2);
        let section_label = active.label(app.settings.language);
        let language = app.settings.language;
        settings_content_frame(app, ui, active, |app, ui| {
            ui.set_min_width((ui.available_width() - Metrics::SP_1).max(360.0));

    if active == SettingsSection::Account {
    settings_section_card(app, ui, section_label, |app, ui| {
        account_section(app, ui);
    });
    }

    if active == SettingsSection::Appearance {
    settings_section_card(app, ui, section_label, |app, ui| {
    let language = app.settings.language;
    ui.label(Type::H4.rich(language.text("Startup", "Запуск"), app.theme.text));
    ui.label(Type::CAPTION.rich(
        language.text("Choose the page Fastcloud opens after launch.", "Выберите страницу, которая откроется при запуске Fastcloud."),
        app.theme.text_dim,
    ));
    let previous_startup = app.settings.startup_page;
    egui::ComboBox::from_id_salt("startup-page")
        .selected_text(match app.settings.startup_page {
            crate::config::StartupPage::Home => language.text("Home", "Главная"),
            crate::config::StartupPage::Search => language.text("Search", "Поиск"),
            crate::config::StartupPage::Library => language.text("Library", "Библиотека"),
            crate::config::StartupPage::Settings => language.text("Settings", "Настройки"),
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut app.settings.startup_page,
                crate::config::StartupPage::Home,
                language.text("Home", "Главная"),
            );
            ui.selectable_value(
                &mut app.settings.startup_page,
                crate::config::StartupPage::Search,
                language.text("Search", "Поиск"),
            );
            ui.selectable_value(
                &mut app.settings.startup_page,
                crate::config::StartupPage::Library,
                language.text("Library", "Библиотека"),
            );
            ui.selectable_value(
                &mut app.settings.startup_page,
                crate::config::StartupPage::Settings,
                language.text("Settings", "Настройки"),
            );
        });
    if app.settings.startup_page != previous_startup
        && let Err(error) = app.settings.save()
    {
        app.toast(format!("Failed to save: {error}"));
    }

    ui.add_space(Metrics::SP_2);
    ui.label(Type::H4.rich(language.text("Theme", "Тема"), app.theme.text));
    ui.horizontal(|ui| {
        let modes = [
            (language.text("Dark", "Тёмная"), crate::config::ThemeMode::Dark),
            (language.text("Light", "Светлая"), crate::config::ThemeMode::Light),
            (language.text("System", "Системная"), crate::config::ThemeMode::System),
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
    ui.label(Type::H4.rich(language.text("Appearance", "Оформление"), app.theme.text));
    ui.horizontal(|ui| {
        ui.label(Type::CAPTION.rich(language.text("Accent", "Акцент"), app.theme.text_dim));
        let mut rgb = app.settings.accent_rgb;
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            app.settings.accent_rgb = rgb;
            app.accent = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            app.theme =
                crate::ui::theme::Theme::from_mode_ctx(app.theme_mode, app.accent, ui.ctx());
            if let Err(error) = app.settings.save() {
                app.toast(format!("Failed to save: {error}"));
            }
        }
        if ui.button(language.text("Reset accent", "Сбросить цвет")).clicked() {
            app.settings.accent_rgb = [0xFF, 0x55, 0x00];
            app.accent = crate::ui::theme::ORANGE;
            app.theme =
                crate::ui::theme::Theme::from_mode_ctx(app.theme_mode, app.accent, ui.ctx());
            if let Err(error) = app.settings.save() {
                app.toast(format!("Failed to save: {error}"));
            }
        }
    });
    ui.add_space(Metrics::SP_075);
    ui.label(Type::CAPTION.rich(language.text("Background image", "Фоновое изображение"), app.theme.text_dim));
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.background_draft)
                .hint_text("https://example.com/wallpaper.jpg")
                .desired_width(430.0),
        );
        if ui.button(language.text("Apply URL", "Применить URL")).clicked() {
            let candidate = app.background_draft.trim().to_owned();
            let valid = url::Url::parse(&candidate)
                .is_ok_and(|url| matches!(url.scheme(), "http" | "https" | "file"));
            if candidate.is_empty() {
                app.settings.background_image = None;
            } else if valid {
                app.settings.background_image = Some(candidate.clone());
            } else {
                app.toast("Use an http:// or https:// image URL");
            }
            if (candidate.is_empty() || valid)
                && let Err(error) = app.settings.save()
            {
                app.toast(format!("Failed to save: {error}"));
            }
        }
        if ui.button(language.text("Choose file…", "Выбрать файл…")).clicked()
            && let Some(path) = rfd::FileDialog::new()
                .set_title("Choose a Fastcloud background")
                .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
                .pick_file()
        {
            match url::Url::from_file_path(path) {
                Ok(uri) => {
                    app.background_draft = uri.to_string();
                    app.settings.background_image = Some(app.background_draft.clone());
                    if let Err(error) = app.settings.save() {
                        app.toast(format!("Failed to save: {error}"));
                    }
                }
                Err(()) => app.toast("That image path cannot be opened"),
            }
        }
        if app.settings.background_image.is_some() && ui.button(language.text("Clear", "Удалить")).clicked() {
            app.settings.background_image = None;
            app.background_draft.clear();
            if let Err(error) = app.settings.save() {
                app.toast(format!("Failed to save: {error}"));
            }
        }
    });
    if app.settings.background_image.is_some() {
        ui.horizontal(|ui| {
            ui.label(Type::CAPTION.rich(language.text("Edge darkening", "Затемнение краёв"), app.theme.text_dim));
            let opacity = ui.add(
                egui::Slider::new(&mut app.settings.background_opacity, 0.0..=0.7).show_value(true),
            );
            ui.label(Type::CAPTION.rich(language.text("Background darkening", "Затемнение фона"), app.theme.text_dim));
            let dim = ui.add(
                egui::Slider::new(&mut app.settings.background_dim, 0.0..=0.85).show_value(true),
            );
            ui.label(Type::CAPTION.rich(language.text("Blur", "Размытие"), app.theme.text_dim));
            let blur = ui.add(
                egui::Slider::new(&mut app.settings.background_blur, 0..=40)
                    .suffix(" px")
                    .show_value(true),
            );
            if (opacity.drag_stopped() || dim.drag_stopped() || blur.drag_stopped())
                && let Err(error) = app.settings.save()
            {
                app.toast(format!("Failed to save: {error}"));
            }
        });
    }
    if ui
        .checkbox(&mut app.settings.compact_rows, language.text("Compact track rows", "Компактные строки треков"))
        .on_hover_text(language.text("One line per track, no artwork", "Одна строка на трек, без обложек"))
        .changed()
        && let Err(e) = app.settings.save()
    {
        app.toast(format!("Failed to save: {e}"));
    }
    if ui
        .checkbox(&mut app.settings.reduced_motion, language.text("Reduce motion", "Меньше анимации"))
        .on_hover_text(language.text("Stops optional continuous animation and lowers idle repaint activity", "Отключает необязательную анимацию и снижает нагрузку в простое"))
        .changed()
        && let Err(error) = app.settings.save()
    {
        app.toast(format!("Failed to save: {error}"));
    }
    interface_font_row(app, ui);
    });
    }

    if active == SettingsSection::Cache {
    settings_section_card(app, ui, section_label, |app, ui| {
    ui.label(Type::H4.rich(language.text("Performance profile", "Профиль производительности"), app.theme.text));
    ui.label(Type::CAPTION.rich(
        language.text("Controls decoded artwork memory and the cover resolution requested from SoundCloud.", "Определяет расход памяти на обложки и качество загружаемых изображений."),
        app.theme.text_dim,
    ));
    let previous_profile = app.settings.memory_profile;
    ui.horizontal(|ui| {
        for (label, profile, hint) in [
            (
                language.text("Eco", "Экономный"),
                crate::config::MemoryProfile::Eco,
                "24 MiB decoded, up to 32 covers; best for low-memory systems",
            ),
            (
                language.text("Balanced", "Сбалансированный"),
                crate::config::MemoryProfile::Balanced,
                "32 MiB decoded, up to 48 covers",
            ),
            (
                language.text("Quality", "Качество"),
                crate::config::MemoryProfile::Quality,
                "48 MiB decoded, up to 64 covers; sharper high-DPI cards",
            ),
        ] {
            ui.selectable_value(&mut app.settings.memory_profile, profile, label)
                .on_hover_text(hint);
        }
    });
    if app.settings.memory_profile != previous_profile {
        app.art.set_profile(app.settings.memory_profile);
        app.art.install_context_budget(ui.ctx());
        app.art.evict(ui.ctx());
        if let Err(error) = app.settings.save() {
            app.toast(format!("Failed to save: {error}"));
        }
    }
    });
    }

    if active == SettingsSection::Audio {
    settings_section_card(app, ui, section_label, |app, ui| {
    ui.label(Type::H4.rich(language.text("Equalizer (10-band)", "Эквалайзер (10 полос)"), app.theme.text));
    let mut eq_changed = ui
        .checkbox(&mut app.settings.eq_enabled, language.text("Enable EQ", "Включить эквалайзер"))
        .changed();
    let mut eq_drag_stopped = false;
    let freqs = crate::audio::dsp::EQ_BAND_FREQS;
    ui.horizontal(|ui| {
        for (i, gain) in app.settings.eq_gains_db.iter_mut().enumerate() {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(format!("{:.0}", freqs[i])).font(Type::CAPTION.font()),
                );
                let response = ui.add(
                    egui::Slider::new(gain, -12.0..=12.0)
                        .vertical()
                        .show_value(false),
                );
                eq_changed |= response.changed();
                eq_drag_stopped |= response.drag_stopped();
                ui.label(egui::RichText::new(format!("{gain:+.0}")).font(Type::CAPTION.font()));
            });
        }
    });
    if eq_changed {
        app.player
            .set_eq(app.settings.eq_enabled, app.settings.eq_gains_db);
    }
    if (eq_drag_stopped || eq_changed && !ui.input(|input| input.pointer.primary_down()))
        && let Err(error) = app.settings.save()
    {
        app.toast(format!("Failed to save: {error}"));
    }
    });
    }

    if active == SettingsSection::Playback {
    settings_section_card(app, ui, section_label, |app, ui| {
    ui.label(Type::H4.rich(language.text("Mini player", "Мини-плеер"), app.theme.text));
    ui.label(Type::CAPTION.rich(
        language.text("Ctrl+M switches this window to a compact player. Airwave is the modern low-motion view; classic Winamp skins remain available.", "Ctrl+M переключает окно в компактный режим. Airwave — современный мини-плеер; классические скины Winamp тоже доступны."),
        app.theme.text_dim,
    ));
    let previous_style = app.settings.mini_player_style;
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut app.settings.mini_player_style,
            crate::config::MiniPlayerStyle::Airwave,
            "Airwave",
        );
        ui.selectable_value(
            &mut app.settings.mini_player_style,
            crate::config::MiniPlayerStyle::Winamp,
            language.text("Winamp classic", "Классический Winamp"),
        );
    });
    if app.settings.mini_player_style != previous_style {
        if let Err(error) = app.settings.save() {
            app.toast(format!("Failed to save: {error}"));
        }
        if app.mini_open() {
            app.close_mini();
            app.toggle_mini();
        }
    }
    ui.horizontal(|ui| {
        let open = app.mini_open();
        if ui
            .button(if open {
                language.text("Close mini player", "Закрыть мини-плеер")
            } else {
                language.text("Open mini player", "Открыть мини-плеер")
            })
            .clicked()
        {
            app.toggle_mini();
        }
        if app.settings.mini_player_style == crate::config::MiniPlayerStyle::Winamp {
            ui.label(Type::CAPTION.rich(language.text("Scale", "Масштаб"), app.theme.text_dim));
        }
        // Whole pixels only: the classic look does not survive interpolation.
        if app.settings.mini_player_style == crate::config::MiniPlayerStyle::Winamp {
            for scale in 1..=4u32 {
                let current = app.settings.winamp_scale == scale;
                if ui.selectable_label(current, format!("{scale}x")).clicked() && !current {
                    app.set_mini_scale(scale);
                }
            }
        }
    });
    ui.horizontal(|ui| {
        if ui
            .checkbox(&mut app.settings.winamp_on_top, language.text("Always on top", "Поверх остальных окон"))
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
        if app.settings.mini_player_style == crate::config::MiniPlayerStyle::Winamp {
            let rolled = app.settings.winamp_shade;
            if ui
                .checkbox(&mut app.settings.winamp_shade, language.text("Rolled up", "Свернуть до заголовка"))
                .on_hover_text("Windowshade: the title bar only, still playing")
                .changed()
            {
                app.settings.winamp_shade = rolled;
                app.toggle_shade(crate::ui::winamp::Window::Main);
            }
        }
    });
    if app.settings.mini_player_style == crate::config::MiniPlayerStyle::Winamp {
        ui.horizontal(|ui| {
            // Winamp's other two windows, which dock under the main one.
            for (window, label, hint) in [
                (
                    crate::ui::winamp::Window::Equalizer,
                    language.text("Equalizer", "Эквалайзер"),
                    "Ten bands, a preamp and the curve, in the skin",
                ),
                (
                    crate::ui::winamp::Window::Playlist,
                    language.text("Playlist", "Плейлист"),
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
            ui.label(Type::CAPTION.rich(&format!("{}: {name}", language.text("Skin", "Скин")), app.theme.text_dim));
            if app.settings.winamp_skin.is_some() && ui.button(language.text("Use built-in", "Встроенный скин")).clicked() {
                app.use_stock_skin();
            }
            if ui
                .button(language.text("Install skin…", "Установить скин…"))
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
                .button(language.text("Browse skins online", "Найти скины в сети"))
                .on_hover_text("Winamp Skin Museum")
                .clicked()
            {
                let _ = webbrowser::open("https://skins.webamp.org");
            }
        });
        ui.label(Type::CAPTION.rich(
            language.text("Download a classic Winamp 2 skin (.wsz or .zip), then click Install skin…. You can also drag the file anywhere onto Fastcloud. The installed copy is kept by the app and restored on the next launch.", "Скачайте скин Winamp 2 (.wsz или .zip) и нажмите «Установить скин». Также файл можно перетащить в окно Fastcloud. Скин сохранится и загрузится при следующем запуске."),
            app.theme.text_dim,
        ));
    }

    });
    }

    if active == SettingsSection::Integrations {
    settings_section_card(app, ui, section_label, |app, ui| {
    ui.label(Type::H4.rich(language.text("Integrations", "Интеграции"), app.theme.text));
    ui.label(Type::CAPTION.rich(
        language.text("Show the current track in your Discord profile. Create a Discord application and paste its application id below.", "Показывайте текущий трек в профиле Discord. Создайте приложение Discord и вставьте его ID ниже."),
        app.theme.text_dim,
    ));
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.settings.discord_client_id)
                .hint_text(language.text("Discord application id", "ID приложения Discord"))
                .desired_width(260.0),
        );
        if ui.button(language.text("Save Discord id", "Сохранить ID Discord")).clicked() {
            if let Err(error) = app.settings.save() {
                app.toast(format!("Failed to save: {error}"));
            } else if app.settings.discord_presence {
                app.reload_discord_presence();
                app.toast("Discord Rich Presence reconnected");
            }
        }
        let mut enabled = app.settings.discord_presence;
        if ui.checkbox(&mut enabled, "Rich Presence").changed() {
            if enabled && app.settings.discord_client_id.trim().is_empty() {
                app.toast("Paste a Discord application id first");
            } else {
                app.settings.discord_presence = enabled;
                app.reload_discord_presence();
                if let Err(error) = app.settings.save() {
                    app.toast(format!("Failed to save: {error}"));
                }
            }
        }
    });
    ui.collapsing(language.text("How to get the Discord application id", "Как получить ID приложения Discord"), |ui| {
        ui.label(Type::CAPTION.rich(
            "1. Open the Discord Developer Portal and choose New Application.\n2. Open General Information.\n3. Copy Application ID (not the bot token) and paste it above.\n4. Keep the Discord desktop app running.",
            app.theme.text_dim,
        ));
        ui.hyperlink_to(
            "Open Discord Developer Portal",
            "https://discord.com/developers/applications",
        );
    });

    ui.add_space(Metrics::SP_1);
    ui.label(Type::BODY.rich(language.text("Import from Yandex Music", "Импорт из Яндекс Музыки"), app.theme.text));
    ui.label(Type::CAPTION.rich(
        language.text("Copies your liked tracks into a new SoundCloud playlist. Matching uses artist, title, and duration; the token is used once and is not saved.", "Копирует любимые треки в новый плейлист SoundCloud. Поиск совпадений идёт по исполнителю, названию и длительности; токен не сохраняется."),
        app.theme.text_dim,
    ));
    ui.horizontal(|ui| {
        ui.add_enabled(
            !app.yandex_import.running,
            egui::TextEdit::singleline(&mut app.yandex_token)
                .password(true)
                .hint_text("Yandex Music OAuth token")
                .desired_width(300.0),
        );
        let can_start = !app.yandex_import.running && !app.yandex_token.trim().is_empty();
        if ui
            .add_enabled(can_start, egui::Button::new(language.text("Import likes", "Импортировать лайки")))
            .clicked()
        {
            app.start_yandex_import();
        }
    });
    ui.collapsing(language.text("How to get a Yandex OAuth token", "Как получить токен Яндекс OAuth"), |ui| {
        ui.label(Type::CAPTION.rich(
            "Register your own application in Yandex OAuth, request the Yandex Music permission if it is offered for your account, and set the verification-code redirect URI. Then use Yandex's manual-token flow and paste only the access_token value above. Fastcloud never saves it. Do not paste your password or use third-party token generators. Yandex Music has no stable public API, so this import may stop working when Yandex changes its private endpoints.",
            app.theme.text_dim,
        ));
        ui.horizontal_wrapped(|ui| {
            ui.hyperlink_to(
                "Register a Yandex OAuth app",
                "https://oauth.yandex.ru/client/new",
            );
            ui.hyperlink_to(
                "Official manual-token instructions",
                "https://yandex.com/dev/id/doc/ru/tokens/debug-token",
            );
        });
    });
    if app.yandex_import.running {
        let progress = if app.yandex_import.total == 0 {
            0.0
        } else {
            app.yandex_import.current as f32 / app.yandex_import.total as f32
        };
        ui.add(
            egui::ProgressBar::new(progress)
                .show_percentage()
                .text(format!(
                    "{} / {} / {} matched",
                    app.yandex_import.current, app.yandex_import.total, app.yandex_import.matched
                )),
        );
        ui.label(Type::CAPTION.rich(&app.yandex_import.current_track, app.theme.text_dim));
    }

    ui.add_space(Metrics::SP_2);
    ui.label(Type::H4.rich(language.text("Storage", "Хранилище"), app.theme.text));
    let art_mb = app.art.decoded_byte_size() as f64 / 1_048_576.0;
    let budget_mb = app.art.budget().decoded_bytes as f64 / 1_048_576.0;
    ui.label(Type::CAPTION.rich(
        &format!("{}: {art_mb:.1} MiB / {budget_mb:.0} MiB", language.text("Decoded artwork", "Обложки в памяти")),
        app.theme.text_dim,
    ));
    if !app.demo {
        ui.label(Type::CAPTION.rich(
            &format!("{}: {}", language.text("Cached lists", "Списки в кэше"), app.store.len()),
            app.theme.text_dim,
        ));
    }
    let audio_bytes = app.player.audio_cache_size();
    ui.horizontal(|ui| {
        ui.label(Type::CAPTION.rich(
            &format!("{}: {}", language.text("Audio cache", "Аудиокэш"), crate::util::fmt_bytes(audio_bytes)),
            app.theme.text_dim,
        ));
        egui::ComboBox::from_id_salt("audio-cache-limit")
            .selected_text(if app.settings.audio_cache_limit_mb == 0 {
                language.text("Disabled", "Отключён").to_owned()
            } else {
                format!("{} MiB", app.settings.audio_cache_limit_mb)
            })
            .show_ui(ui, |ui| {
                for limit in [0, 256, 512, 1024, 2048, 4096, 8192] {
                    let label = if limit == 0 {
                        language.text("Disabled", "Отключён").to_owned()
                    } else {
                        format!("{limit} MiB")
                    };
                    if ui
                        .selectable_value(&mut app.settings.audio_cache_limit_mb, limit, label)
                        .changed()
                    {
                        app.player.set_audio_cache_limit_mb(limit);
                        if let Err(error) = app.settings.save() {
                            app.toast(format!("Failed to save: {error}"));
                        }
                    }
                }
            });
        if ui.button(language.text("Clear audio cache", "Очистить аудиокэш")).clicked() {
            let freed = app.player.clear_audio_cache();
            app.toast(format!(
                "Cleared audio cache ({} freed)",
                crate::util::fmt_bytes(freed)
            ));
        }
    });
    ui.horizontal(|ui| {
        if ui
            .button(language.text("Clear artwork cache", "Очистить кэш обложек"))
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
                .button(language.text("Refresh everything", "Обновить все данные"))
                .on_hover_text("Forget every cached list and ask SoundCloud again")
                .clicked()
        {
            app.store.clear();
            app.toast("Reloading from SoundCloud");
        }
    });
    });
    }

    if active == SettingsSection::Shortcuts {
    settings_section_card(app, ui, section_label, |app, ui| {
        if ui.button(language.text("Keyboard shortcuts (F1)", "Горячие клавиши (F1)")).clicked() {
            app.show_shortcuts = true;
        }
    });
    }

    if active == SettingsSection::About {
    settings_section_card(app, ui, section_label, |app, ui| {
    ui.label(Type::CAPTION.rich(
        &format!(
            "Fastcloud {} · {}",
            app.version_build,
            if app.demo {
                language.text("demo library", "демо-библиотека")
            } else if app.signed_in() {
                language.text("signed in", "вход выполнен")
            } else {
                language.text("connection required", "нужно подключение")
            }
        ),
        app.theme.text_dim,
    ));
    });
    }
        });
    });
}

/// The interface font row.
///
/// soundcloud.com's own face is commercially licensed, so Inter ships instead
/// (see [`crate::fonts`]). A user who owns Söhne can point at the file and get
/// the site's exact letterforms; the change needs a restart because egui
/// installs fonts once, when the context is created.
fn interface_font_row(app: &mut App, ui: &mut egui::Ui) {
    let language = app.settings.language;
    ui.add_space(Metrics::SP_075);
    let current = app
        .settings
        .interface_font
        .as_ref()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "Inter (built in)".to_owned());
    ui.horizontal(|ui| {
        ui.label(Type::CAPTION.rich(
            &format!(
                "{}: {current}",
                language.text("Interface font", "Шрифт интерфейса")
            ),
            app.theme.text_dim,
        ));
        ui.label(Type::CAPTION.rich(
            language.text(
                "· drop a .ttf/.otf on the window to change it",
                "· перетащи .ttf/.otf в окно, чтобы заменить",
            ),
            app.theme.text_dim,
        ));
        if app.settings.interface_font.is_some()
            && ui
                .button(language.text("Use Inter", "Использовать Inter"))
                .clicked()
        {
            app.settings.interface_font = None;
            match app.settings.save() {
                Ok(()) => app.toast("Interface font reset - restart to apply"),
                Err(e) => app.toast(format!("Failed to save: {e}")),
            }
        }
    });
    ui.label(Type::CAPTION.rich(
        language.text("soundcloud.com uses Söhne, which is licensed and cannot be bundled. Own it? Drop the file on the window.", "SoundCloud использует платный шрифт Söhne, поэтому мы не можем включить его в приложение. Если у тебя есть лицензия, перетащи файл шрифта в окно."),
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
    let language = app.settings.language;
    ui.label(Type::H4.rich(language.text("Account", "Аккаунт"), app.theme.text));

    if app.demo {
        ui.label(Type::BODY.rich(
            language.text(
                "Running on the built-in demo library — no network, no account.",
                "Демо-библиотека работает без сети и аккаунта.",
            ),
            app.theme.text_dim,
        ));
    } else if app.signed_in() {
        let who = app.display_name();
        ui.horizontal(|ui| {
            ui.label(Type::BODY.rich(
                &format!("{} {who}", language.text("Signed in as", "Вы вошли как")),
                app.theme.text,
            ));
            if ui.button(language.text("Sign out", "Выйти")).clicked() {
                app.sign_out();
            }
            if ui
                .button(language.text("Disconnect Fastcloud", "Отключить Fastcloud"))
                .on_hover_text(language.text(
                    "Revoke Fastcloud's OAuth access to this SoundCloud account",
                    "Отозвать доступ Fastcloud к аккаунту SoundCloud",
                ))
                .clicked()
            {
                app.disconnect_account();
            }
        });
    }

    ui.add_space(Metrics::SP_075);
    ui.label(Type::CAPTION.rich(
        language.text(
            "Public tracks do not require a paid listener subscription, but Fastcloud currently asks you to register an API app, which SoundCloud restricts to Artist Pro accounts. GO+ tracks require Go+; blocked tracks may only offer a preview.",
            "Для публичных треков платная подписка слушателя не нужна, но сейчас Fastcloud просит зарегистрировать API-приложение, а SoundCloud разрешает это только с Artist Pro. Треки GO+ требуют Go+; заблокированные треки могут давать лишь фрагмент.",
        ),
        app.theme.text_dim,
    ));
}

// ===== Right rail (soundcloud.com style) =====

/// The right rail, as soundcloud.com has it: who to follow, what is new, and
/// your last likes. Every list comes from the same store the pages use, so
/// nothing here fetches on its own.
#[allow(dead_code)]
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

#[allow(dead_code)]
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
#[allow(dead_code)]
fn two_columns(app: &mut App, ui: &mut egui::Ui, main: impl FnOnce(&mut App, &mut egui::Ui)) {
    ui.horizontal(|ui| {
        let total = ui.available_width();
        let with_rail = feed_shows_right_rail(total);
        let left_w = if with_rail {
            (total - 360.0).clamp(280.0, 820.0)
        } else {
            total
        };
        ui.vertical(|ui| {
            ui.set_min_width(left_w);
            ui.set_max_width(left_w);
            main(app, ui);
        });
        if with_rail {
            ui.separator();
            ui.vertical(|ui| {
                ui.set_min_width(180.0);
                right_rail(app, ui);
            });
        }
    });
}

fn feed_shows_right_rail(width: f32) -> bool {
    width >= 920.0
}

#[cfg(test)]
mod feed_columns_tests {
    use super::feed_shows_right_rail;

    #[test]
    fn feed_hides_the_right_rail_when_it_would_cramp_the_main_column() {
        assert!(!feed_shows_right_rail(664.0));
    }

    #[test]
    fn feed_keeps_the_right_rail_on_wide_content() {
        assert!(feed_shows_right_rail(1_184.0));
    }
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

fn library_selection_bar(app: &mut App, ui: &mut egui::Ui, tracks: &[Track]) {
    if tracks.is_empty() {
        return;
    }
    let songs = selected_tracks(tracks, &app.selected);
    if songs.is_empty() {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Select all visible").clicked() {
                app.selected.extend(tracks.iter().map(|track| track.id));
            }
            ui.label(Type::CAPTION.rich(
                "Ctrl-click to select · Shift-click for a range",
                app.theme.text_dim,
            ));
        });
        return;
    }
    egui::Panel::bottom(egui::Id::new("library-selection-actions"))
        .resizable(false)
        .show(ui, |ui| {
            let action = airwave::selection_actions(ui, app.theme, songs.len(), |ui| {
                widgets::picked_menu(app, ui, &songs, songs.len());
            });
            match action {
                Some(airwave::SelectionAction::Play) => {
                    app.play_user_queue(songs, 0, false);
                    app.clear_selection();
                }
                Some(airwave::SelectionAction::Queue) => {
                    app.toast(format!("Added {} songs to queue", songs.len()));
                    app.player.enqueue(songs, false);
                }
                Some(airwave::SelectionAction::SelectAll) => {
                    app.selected.extend(tracks.iter().map(|track| track.id));
                }
                Some(airwave::SelectionAction::Clear) => app.clear_selection(),
                None => {}
            }
        });
}

fn selected_tracks(tracks: &[Track], selected: &std::collections::BTreeSet<u64>) -> Vec<Track> {
    tracks
        .iter()
        .filter(|track| selected.contains(&track.id))
        .cloned()
        .collect()
}

#[cfg(test)]
mod library_selection_tests {
    use super::*;

    fn tracks() -> Vec<Track> {
        serde_json::from_str(
            r#"[{"id":3,"title":"Three"},{"id":1,"title":"One"},{"id":2,"title":"Two"}]"#,
        )
        .unwrap()
    }

    #[test]
    fn selection_preserves_display_order() {
        let selected = [1, 3].into_iter().collect();
        let ids: Vec<_> = selected_tracks(&tracks(), &selected)
            .iter()
            .map(|track| track.id)
            .collect();
        assert_eq!(ids, [3, 1]);
    }

    #[test]
    fn selection_excludes_tracks_hidden_by_filter() {
        let selected = [1, 99].into_iter().collect();
        let ids: Vec<_> = selected_tracks(&tracks(), &selected)
            .iter()
            .map(|track| track.id)
            .collect();
        assert_eq!(ids, [1]);
    }

    #[test]
    fn empty_selection_has_no_actions() {
        assert!(selected_tracks(&tracks(), &Default::default()).is_empty());
    }
}

fn track_list(app: &mut App, ui: &mut egui::Ui, tracks: &[Track]) {
    egui::ScrollArea::vertical()
        .id_salt("track-list")
        .show(ui, |ui| {
            track_rows(app, ui, tracks);
        });
}

/// Same rows without their own scroll area — for pages that scroll as a whole.
/// Borrow the queue while drawing; copy it only when playback starts.
/// Ctrl/Cmd-click picks rows, Shift-click picks a range, plain click plays
/// and clears the pick (fastpotify's multi-select).
fn track_rows(app: &mut App, ui: &mut egui::Ui, tracks: &[Track]) {
    use crate::ui::widgets::RowAction;
    let order: Vec<u64> = tracks.iter().map(|t| t.id).collect();
    // Picked tracks in table order for the multi menu.
    let multi = selected_tracks(tracks, &app.selected);
    let multi_ref = (!multi.is_empty()).then_some(multi.as_slice());
    for (idx, t) in tracks.iter().enumerate() {
        let picked = app.selected.contains(&t.id);
        let multi = if picked { multi_ref } else { None };
        match widgets::track_row(app, ui, t, idx, picked, multi) {
            RowAction::Play => {
                app.clear_selection();
                let queue = tracks.to_vec();
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

#[cfg(test)]
mod search_filter_tests {
    use super::{
        SearchMode, SearchMove, SearchSurface, TextLane, interleave_discovery_lanes,
        next_search_selection, page_wheel_multiplier, search_mode_picker, search_query_is_ready,
        search_surface, text_mix_plan,
    };
    use crate::ui::route::Route;
    use crate::ui::theme::Theme;
    use eframe::egui;

    #[test]
    fn one_character_does_not_start_a_search() {
        assert!(!search_query_is_ready(" a "));
    }

    #[test]
    fn two_characters_start_a_search() {
        assert!(search_query_is_ready(" ab "));
    }

    #[test]
    fn donor_search_defaults_to_text() {
        assert_eq!(SearchMode::default(), SearchMode::Text);
    }

    #[test]
    fn empty_query_is_the_landing_wave() {
        assert_eq!(
            search_surface(" ", SearchMode::SoundCloud),
            SearchSurface::Wave
        );
    }

    #[test]
    fn search_surface_follows_the_selected_source() {
        assert_eq!(
            search_surface("hyper", SearchMode::Text),
            SearchSurface::Text
        );
        assert_eq!(
            search_surface("hyper", SearchMode::Vibe),
            SearchSurface::Vibe
        );
        assert_eq!(
            search_surface("hyper", SearchMode::SoundCloud),
            SearchSurface::SoundCloud
        );
    }

    #[test]
    fn next_result_wraps_to_the_first_result() {
        assert_eq!(next_search_selection(3, 4, SearchMove::Next), 0);
    }

    #[test]
    fn previous_result_wraps_to_the_last_result() {
        assert_eq!(next_search_selection(0, 4, SearchMove::Previous), 3);
    }

    #[test]
    fn empty_results_keep_a_safe_selection() {
        assert_eq!(next_search_selection(8, 0, SearchMove::Next), 0);
    }

    #[test]
    fn text_results_get_a_vibe_pinch_in_every_seventh_slot() {
        assert_eq!(
            text_mix_plan(8, 2),
            vec![
                TextLane::Lexical(0),
                TextLane::Lexical(1),
                TextLane::Lexical(2),
                TextLane::Lexical(3),
                TextLane::Lexical(4),
                TextLane::Vibe(0),
                TextLane::Lexical(5),
                TextLane::Lexical(6),
                TextLane::Lexical(7),
                TextLane::Vibe(1),
            ]
        );
    }

    #[test]
    fn discovery_interleaves_sources_and_deduplicates_tracks() {
        let track = |id| {
            serde_json::from_value(serde_json::json!({
                "id": id,
                "title": format!("Track {id}")
            }))
            .unwrap()
        };

        let rows = interleave_discovery_lanes(vec![
            vec![track(1), track(2), track(3)],
            vec![track(10), track(2), track(11)],
        ]);
        let ids: Vec<_> = rows.iter().map(|track| track.id).collect();

        assert_eq!(ids, [1, 10, 2, 3, 11]);
    }

    #[test]
    fn settings_page_scrolls_faster_than_regular_pages() {
        assert!(page_wheel_multiplier(&Route::Settings).y > page_wheel_multiplier(&Route::Home).y);
    }

    #[test]
    fn search_mode_picker_only_consumes_its_control_row() {
        let mut height = 0.0;
        egui::__run_test_ui(|ui| {
            ui.set_height(600.0);
            height = search_mode_picker(
                ui,
                &Theme::dark(egui::Color32::from_rgb(255, 85, 0)),
                SearchMode::Vibe,
                crate::config::Language::English,
            )
            .response
            .rect
            .height();
        });
        assert!(height <= 40.0, "picker consumed {height}px of page height");
    }
}
