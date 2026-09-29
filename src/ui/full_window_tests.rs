use std::sync::Arc;

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::App;
use super::route::Route;
use super::views::{LibraryTab, SettingsSection};

const WIDE: egui::Vec2 = egui::vec2(1280.0, 800.0);
const NARROW: egui::Vec2 = egui::vec2(760.0, 800.0);

#[derive(Clone, Copy)]
enum Scene {
    Home,
    HomeNarrow,
    HomeCollapsed,
    HomePinned,
    HomeLight,
    HomeWallpaper,
    HomeWallpaperScrolled,
    Queue,
    LibraryOverview,
    LibraryPlaylists,
    LibraryAlbums,
    LibraryStations,
    LibraryFollowing,
    LibraryHistory,
    LibrarySelection,
    LibrarySelectionScrolled,
    Discover,
    DiscoverNarrow,
    Catalog,
    CatalogArtists,
    CatalogNarrow,
    Search,
    SearchNarrow,
    Recent,
    TrackDetail,
    TrackDetailNarrow,
    PlaylistDetail,
    PlaylistDetailNarrow,
    UserDetail,
    UserDetailNarrow,
    Feed,
    FeedScrolled,
    FeedWithoutReposts,
    Settings,
    SettingsNarrow,
    SettingsMiddle,
    SettingsLower,
    SettingsWallpaper,
    SettingsRussian,
}

impl Scene {
    fn name(self) -> &'static str {
        match self {
            Self::Home => "home_window",
            Self::HomeNarrow => "home_narrow_window",
            Self::HomeCollapsed => "home_collapsed_window",
            Self::HomePinned => "home_pinned_window",
            Self::HomeLight => "home_light_window",
            Self::HomeWallpaper => "home_wallpaper_window",
            Self::HomeWallpaperScrolled => "home_wallpaper_scrolled_window",
            Self::Queue => "queue_window",
            Self::LibraryOverview => "library_overview_window",
            Self::LibraryPlaylists => "library_playlists_window",
            Self::LibraryAlbums => "library_albums_window",
            Self::LibraryStations => "library_stations_window",
            Self::LibraryFollowing => "library_following_window",
            Self::LibraryHistory => "library_history_window",
            Self::LibrarySelection => "library_selection_window",
            Self::LibrarySelectionScrolled => "library_selection_scrolled_window",
            Self::Discover => "discover_window",
            Self::DiscoverNarrow => "discover_narrow_window",
            Self::Catalog => "catalog_window",
            Self::CatalogArtists => "catalog_artists_window",
            Self::CatalogNarrow => "catalog_narrow_window",
            Self::Search => "search_window",
            Self::SearchNarrow => "search_narrow_window",
            Self::Recent => "recent_window",
            Self::TrackDetail => "track_detail_window",
            Self::TrackDetailNarrow => "track_detail_narrow_window",
            Self::PlaylistDetail => "playlist_detail_window",
            Self::PlaylistDetailNarrow => "playlist_detail_narrow_window",
            Self::UserDetail => "user_detail_window",
            Self::UserDetailNarrow => "user_detail_narrow_window",
            Self::Feed => "feed_window",
            Self::FeedScrolled => "feed_scrolled_window",
            Self::FeedWithoutReposts => "feed_without_reposts_window",
            Self::Settings => "settings_window",
            Self::SettingsNarrow => "settings_narrow_window",
            Self::SettingsMiddle => "settings_middle_window",
            Self::SettingsLower => "settings_lower_window",
            Self::SettingsWallpaper => "settings_wallpaper_window",
            Self::SettingsRussian => "settings_russian_window",
        }
    }

    fn route(self) -> Route {
        match self {
            Self::Home
            | Self::HomeNarrow
            | Self::HomeCollapsed
            | Self::HomePinned
            | Self::HomeLight
            | Self::HomeWallpaper
            | Self::HomeWallpaperScrolled
            | Self::Queue => Route::Home,
            Self::LibraryOverview
            | Self::LibraryPlaylists
            | Self::LibraryAlbums
            | Self::LibraryStations
            | Self::LibraryFollowing
            | Self::LibraryHistory => Route::Library,
            Self::LibrarySelection | Self::LibrarySelectionScrolled => Route::Library,
            Self::Discover | Self::DiscoverNarrow => Route::Discover,
            Self::Catalog | Self::CatalogArtists | Self::CatalogNarrow => Route::Catalog,
            Self::Search | Self::SearchNarrow => Route::Search("ambient".to_owned()),
            Self::Recent => Route::Recent,
            Self::TrackDetail | Self::TrackDetailNarrow => Route::TrackDetail(1000),
            Self::PlaylistDetail | Self::PlaylistDetailNarrow => Route::PlaylistDetail(2001),
            Self::UserDetail | Self::UserDetailNarrow => Route::UserDetail(12),
            Self::Feed | Self::FeedScrolled | Self::FeedWithoutReposts => Route::Feed,
            Self::Settings
            | Self::SettingsNarrow
            | Self::SettingsMiddle
            | Self::SettingsLower
            | Self::SettingsWallpaper
            | Self::SettingsRussian => Route::Settings,
        }
    }
}

fn runtime() -> &'static tokio::runtime::Runtime {
    Box::leak(Box::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("test runtime"),
    ))
}

fn app_for_scene(scene: Scene) -> App {
    let runtime = runtime();
    let temp_root = std::env::temp_dir().join(format!(
        "fastcloud-full-window-{}-{}",
        std::process::id(),
        scene.name()
    ));
    let client = Arc::new(crate::api::ApiClient::new(None, true));
    let cache = Arc::new(
        crate::audio::cache::AudioCache::new(temp_root.join("audio"), 8 * 1024 * 1024)
            .expect("test audio cache"),
    );
    let output = Arc::new(crate::audio::output::AudioOutput::silent([0.0; 10], 0.8));
    let player = Arc::new(crate::player::Player::new(
        output,
        Arc::clone(&client),
        cache,
        temp_root.join("settings.json"),
        runtime.handle().clone(),
    ));
    let tracks = crate::demo::demo_tracks();
    {
        let mut state = player.state.lock();
        state.queue = tracks.clone();
        state.order = (0..tracks.len()).collect();
        state.current = Some(0);
        state.duration_ms = tracks[0].effective_duration_ms();
    }

    let mut settings = crate::config::Settings {
        background_image: None,
        reduced_motion: true,
        ..crate::config::Settings::default()
    };
    if matches!(scene, Scene::HomePinned) {
        use crate::config::QuickAccessShortcut;
        settings.quick_access = vec![
            QuickAccessShortcut::Track {
                id: 1000,
                title: "Northern Lights".to_owned(),
                artist: "SoundCloud Demo".to_owned(),
                artwork_url: None,
            },
            QuickAccessShortcut::Playlist {
                id: 2001,
                title: "Neon Nights".to_owned(),
                artist: "Playlist".to_owned(),
                artwork_url: None,
            },
            QuickAccessShortcut::Album {
                id: 2101,
                title: "Afterglow".to_owned(),
                artist: "Album".to_owned(),
                artwork_url: None,
            },
        ];
    }
    if matches!(scene, Scene::SettingsRussian) {
        settings.language = crate::config::Language::Russian;
    }
    if matches!(
        scene,
        Scene::HomeWallpaper | Scene::HomeWallpaperScrolled | Scene::SettingsWallpaper
    ) {
        let wallpaper = temp_root.join("wallpaper.png");
        let picture = image::RgbImage::from_fn(1280, 800, |x, y| {
            let wave = ((x / 32 + y / 32) % 2) as u8;
            if (x / 28 + y / 20) % 9 == 0 {
                image::Rgb([238, 235, 230])
            } else {
                image::Rgb([
                    42 + (x / 8) as u8,
                    70 + (y / 7) as u8,
                    if wave == 0 { 172 } else { 82 },
                ])
            }
        });
        picture.save(&wallpaper).expect("save wallpaper fixture");
        settings.background_image = Some(
            url::Url::from_file_path(&wallpaper)
                .expect("wallpaper file URL")
                .to_string(),
        );
        settings.background_blur = 0;
        settings.background_dim = 0.12;
        settings.background_opacity = 0.08;
    }
    let art = crate::images::ArtLoader::new(temp_root.join("art"), runtime.handle().clone());
    let store = crate::store::Store::new(client, runtime.handle().clone());
    let mut app = App::new(
        player,
        settings,
        true,
        None,
        None,
        None,
        None,
        None,
        art,
        runtime.handle().clone(),
        store,
        None,
    );
    app.route = scene.route();
    app.feed_banner_dismissed = true;
    if matches!(scene, Scene::HomeLight) {
        app.theme_mode = crate::config::ThemeMode::Light;
        app.theme = crate::ui::theme::Theme::light(app.accent);
    }
    if matches!(scene, Scene::HomeCollapsed) {
        app.sidebar_collapsed = true;
    }
    match scene {
        Scene::LibraryOverview => app.library_tab = LibraryTab::Overview,
        Scene::LibraryPlaylists => app.library_tab = LibraryTab::Playlists,
        Scene::LibraryAlbums => app.library_tab = LibraryTab::Albums,
        Scene::LibraryStations => app.library_tab = LibraryTab::Stations,
        Scene::LibraryFollowing => app.library_tab = LibraryTab::Following,
        Scene::LibraryHistory => app.library_tab = LibraryTab::History,
        Scene::LibrarySelection | Scene::LibrarySelectionScrolled => {
            app.library_tab = LibraryTab::Likes;
            app.selected
                .extend(tracks.iter().take(3).map(|track| track.id));
            if matches!(scene, Scene::LibrarySelectionScrolled) {
                app.screenshot_scroll_offset = Some(420.0);
            }
        }
        Scene::Search | Scene::SearchNarrow => {
            app.search_query = "ambient".to_owned();
            app.search_committed_query = "ambient".to_owned();
            app.search_focus_requested = false;
        }
        Scene::CatalogArtists => app.catalog_artists = true,
        Scene::FeedWithoutReposts => app.show_reposts = false,
        Scene::FeedScrolled => app.screenshot_scroll_offset = Some(720.0),
        Scene::HomeWallpaperScrolled => app.screenshot_scroll_offset = Some(960.0),
        Scene::Queue => app.show_queue = true,
        Scene::SettingsMiddle | Scene::SettingsLower => {}
        Scene::Home
        | Scene::HomeNarrow
        | Scene::HomeCollapsed
        | Scene::HomePinned
        | Scene::HomeLight
        | Scene::HomeWallpaper
        | Scene::Discover
        | Scene::DiscoverNarrow
        | Scene::Catalog
        | Scene::CatalogNarrow
        | Scene::Recent
        | Scene::TrackDetail
        | Scene::TrackDetailNarrow
        | Scene::PlaylistDetail
        | Scene::PlaylistDetailNarrow
        | Scene::UserDetail
        | Scene::UserDetailNarrow
        | Scene::Feed
        | Scene::Settings
        | Scene::SettingsNarrow
        | Scene::SettingsWallpaper
        | Scene::SettingsRussian => {}
    }
    app
}

fn harness(scene: Scene, size: egui::Vec2) -> Harness<'static, App> {
    Harness::builder()
        .with_size(size)
        .with_pixels_per_point(1.0)
        .build_eframe(move |creation| {
            egui_extras::install_image_loaders(&creation.egui_ctx);
            crate::fonts::install(&creation.egui_ctx, None);
            let app = app_for_scene(scene);
            creation
                .egui_ctx
                .add_bytes_loader(Arc::new(app.art.clone()));
            app.theme.apply(&creation.egui_ctx);
            app
        })
}

#[test]
fn full_window_home_matches_golden_image() {
    snapshot(Scene::Home, WIDE);
}

#[test]
fn full_window_home_narrow_matches_golden_image() {
    snapshot(Scene::HomeNarrow, NARROW);
}

#[test]
fn full_window_home_collapsed_matches_golden_image() {
    snapshot(Scene::HomeCollapsed, WIDE);
}

#[test]
fn full_window_home_pinned_matches_golden_image() {
    snapshot(Scene::HomePinned, WIDE);
}

#[test]
fn full_window_home_light_matches_golden_image() {
    snapshot(Scene::HomeLight, WIDE);
}

#[test]
fn full_window_home_wallpaper_matches_golden_image() {
    snapshot(Scene::HomeWallpaper, WIDE);
}

#[test]
fn full_window_home_wallpaper_scrolled_matches_golden_image() {
    snapshot(Scene::HomeWallpaperScrolled, WIDE);
}

#[test]
fn full_window_queue_matches_golden_image() {
    snapshot(Scene::Queue, WIDE);
}

#[test]
fn full_window_library_overview_matches_golden_image() {
    snapshot(Scene::LibraryOverview, WIDE);
}

#[test]
fn full_window_library_playlists_matches_golden_image() {
    snapshot(Scene::LibraryPlaylists, WIDE);
}

#[test]
fn full_window_library_albums_matches_golden_image() {
    snapshot(Scene::LibraryAlbums, WIDE);
}

#[test]
fn full_window_library_stations_matches_golden_image() {
    snapshot(Scene::LibraryStations, WIDE);
}

#[test]
fn full_window_library_following_matches_golden_image() {
    snapshot(Scene::LibraryFollowing, WIDE);
}

#[test]
fn full_window_library_history_matches_golden_image() {
    snapshot(Scene::LibraryHistory, WIDE);
}

fn snapshot(scene: Scene, size: egui::Vec2) {
    let mut harness = harness(scene, size);
    let section = match scene {
        Scene::SettingsMiddle | Scene::SettingsRussian => Some(SettingsSection::Appearance),
        Scene::SettingsLower => Some(SettingsSection::Audio),
        _ => None,
    };
    if let Some(section) = section {
        harness.ctx.data_mut(|data| {
            data.insert_temp(egui::Id::new("settings-active-section"), section);
        });
    }
    if matches!(
        scene,
        Scene::HomeWallpaper | Scene::HomeWallpaperScrolled | Scene::SettingsWallpaper
    ) {
        for _ in 0..40 {
            harness.run_steps(1);
            if harness.state().ready_background_uri.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(harness.state().ready_background_uri.is_some());
        harness.run_steps(2);
    } else {
        harness.run_steps(4);
    }
    harness.snapshot(format!("airwave/full_window/{}", scene.name()));
}

#[test]
fn full_window_library_selection_matches_golden_image() {
    snapshot(Scene::LibrarySelection, WIDE);
}

#[test]
fn full_window_scrolled_library_selection_matches_golden_image() {
    snapshot(Scene::LibrarySelectionScrolled, WIDE);
}

#[test]
fn full_window_discover_matches_golden_image() {
    snapshot(Scene::Discover, WIDE);
}

#[test]
fn full_window_discover_narrow_matches_golden_image() {
    snapshot(Scene::DiscoverNarrow, NARROW);
}

#[test]
fn full_window_catalog_matches_golden_image() {
    snapshot(Scene::Catalog, WIDE);
}

#[test]
fn full_window_catalog_artists_matches_golden_image() {
    snapshot(Scene::CatalogArtists, WIDE);
}

#[test]
fn full_window_catalog_narrow_matches_golden_image() {
    snapshot(Scene::CatalogNarrow, NARROW);
}

#[test]
fn full_window_search_matches_golden_image() {
    snapshot(Scene::Search, WIDE);
}

#[test]
fn full_window_search_narrow_matches_golden_image() {
    snapshot(Scene::SearchNarrow, NARROW);
}

#[test]
fn full_window_recent_matches_golden_image() {
    snapshot(Scene::Recent, WIDE);
}

#[test]
fn full_window_track_detail_matches_golden_image() {
    snapshot(Scene::TrackDetail, WIDE);
}

#[test]
fn full_window_track_detail_narrow_matches_golden_image() {
    snapshot(Scene::TrackDetailNarrow, NARROW);
}

#[test]
fn full_window_playlist_detail_matches_golden_image() {
    snapshot(Scene::PlaylistDetail, WIDE);
}

#[test]
fn full_window_playlist_detail_narrow_matches_golden_image() {
    snapshot(Scene::PlaylistDetailNarrow, NARROW);
}

#[test]
fn full_window_user_detail_matches_golden_image() {
    snapshot(Scene::UserDetail, WIDE);
}

#[test]
fn full_window_user_detail_narrow_matches_golden_image() {
    snapshot(Scene::UserDetailNarrow, NARROW);
}

#[test]
fn full_window_feed_matches_golden_image() {
    snapshot(Scene::Feed, WIDE);
}

#[test]
fn full_window_scrolled_feed_matches_golden_image() {
    snapshot(Scene::FeedScrolled, WIDE);
}

#[test]
fn full_window_feed_without_reposts_matches_golden_image() {
    snapshot(Scene::FeedWithoutReposts, NARROW);
}

#[test]
fn full_window_settings_matches_golden_image() {
    snapshot(Scene::Settings, WIDE);
}

#[test]
fn full_window_settings_narrow_matches_golden_image() {
    snapshot(Scene::SettingsNarrow, NARROW);
}

#[test]
fn full_window_settings_middle_matches_golden_image() {
    snapshot(Scene::SettingsMiddle, WIDE);
}

#[test]
fn full_window_settings_lower_matches_golden_image() {
    snapshot(Scene::SettingsLower, WIDE);
}

#[test]
fn full_window_settings_wallpaper_matches_golden_image() {
    snapshot(Scene::SettingsWallpaper, WIDE);
}

#[test]
fn full_window_settings_russian_matches_golden_image() {
    snapshot(Scene::SettingsRussian, WIDE);
}

#[test]
fn settings_only_shows_the_selected_section() {
    let mut harness = harness(Scene::Settings, WIDE);
    harness.run_steps(2);
    assert!(harness.query_by_label("Account").is_some());
    assert!(harness.query_by_label("Startup").is_none());
    assert!(harness.query_by_label("Performance profile").is_none());

    harness.ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new("settings-active-section"),
            SettingsSection::Appearance,
        );
    });
    harness.run_steps(2);
    assert!(harness.query_by_label("Startup").is_some());
    assert!(harness.query_by_label("Account").is_none());
    assert!(harness.query_by_label("Performance profile").is_none());

    harness.ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new("settings-active-section"),
            SettingsSection::Audio,
        );
    });
    harness.run_steps(2);
    assert!(harness.query_by_label("Equalizer (10-band)").is_some());
    assert!(harness.query_by_label("Startup").is_none());
    assert!(harness.query_by_label("Mini player").is_none());

    harness.ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new("settings-active-section"),
            SettingsSection::Appearance,
        );
    });

    harness.state_mut().settings.language = crate::config::Language::Russian;
    harness.run_steps(2);
    assert!(harness.query_by_label("Запуск").is_some());
    assert!(harness.query_by_label("Startup").is_none());
}
