use super::App;
use super::route::Route;
use super::widgets;
use crate::api::models::Track;
use eframe::egui;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    match app.route.clone() {
        Route::Home => home(app, ui),
        Route::Likes => likes(app, ui),
        Route::Recent => recent(app, ui),
        Route::Following => following(app, ui),
        Route::Search(q) => search(app, ui, q),
        Route::TrackDetail(id) => track_detail(app, ui, id),
        Route::PlaylistDetail(id) => playlist_detail(app, ui, id),
        Route::UserDetail(id) => user_detail(app, ui, id),
        Route::Settings => settings(app, ui),
    }
}

fn home(app: &mut App, ui: &mut egui::Ui) {
    header(app, ui, "Home");
    if app.demo {
        demo_tracks(app, ui);
    } else {
        ui.label(
            egui::RichText::new("Connect your SoundCloud account in Settings.")
                .color(app.theme.text_dim),
        );
    }
}

fn likes(app: &mut App, ui: &mut egui::Ui) {
    header(app, ui, "Likes");
    if app.demo {
        demo_tracks(app, ui);
    }
}

fn recent(app: &mut App, ui: &mut egui::Ui) {
    header(app, ui, "Recently played");
    if app.demo {
        demo_tracks(app, ui);
    }
}

fn following(app: &mut App, ui: &mut egui::Ui) {
    header(app, ui, "Following");
    ui.label(egui::RichText::new("Not available in demo mode.").color(app.theme.text_dim));
}

fn search(app: &mut App, ui: &mut egui::Ui, _initial: String) {
    header(app, ui, "Search");
    ui.horizontal(|ui| {
        let mut q = app.search_query.clone();
        ui.add(egui::TextEdit::singleline(&mut q).hint_text("Tracks, playlists, people…"));
        app.search_query = q;
    });
    if app.demo && !app.search_query.trim().is_empty() {
        let q = app.search_query.to_lowercase();
        let tracks: Vec<Track> = crate::demo::demo_tracks()
            .into_iter()
            .filter(|t| t.title.to_lowercase().contains(&q))
            .collect();
        track_list(app, ui, &tracks);
    }
}

fn track_detail(app: &mut App, ui: &mut egui::Ui, id: u64) {
    header(app, ui, "Track");
    if app.demo {
        if let Some(track) = crate::demo::demo_tracks().into_iter().find(|t| t.id == id) {
            track_list(app, ui, std::slice::from_ref(&track));
            if ui.button("Show related").clicked() {
                app.route = Route::Home;
            }
        }
    }
}

fn playlist_detail(app: &mut App, ui: &mut egui::Ui, id: u64) {
    header(app, ui, "Playlist");
    if app.demo {
        let tracks: Vec<Track> = crate::demo::demo_tracks()
            .into_iter()
            .filter(|t| t.id % 2 == (id % 2))
            .collect();
        track_list(app, ui, &tracks);
    }
    if let Some(dropped) = widgets::playlist_drop_zone(
        ui,
        egui::Id::new("pl-drop"),
        "Drop tracks to add to playlist",
    ) {
        app.toast(format!("Added track {dropped} to playlist {id}"));
    }
}

fn user_detail(app: &mut App, ui: &mut egui::Ui, id: u64) {
    header(app, ui, "Artist");
    if app.demo {
        let tracks: Vec<Track> = crate::demo::demo_tracks()
            .into_iter()
            .filter(|t| t.id % 3 == id % 3)
            .collect();
        track_list(app, ui, &tracks);
    }
}

fn settings(app: &mut App, ui: &mut egui::Ui) {
    header(app, ui, "Settings");

    ui.label(egui::RichText::new("Theme").strong());
    ui.horizontal(|ui| {
        let modes = [
            ("Dark", crate::config::ThemeMode::Dark),
            ("Light", crate::config::ThemeMode::Light),
            ("System", crate::config::ThemeMode::System),
        ];
        for (name, mode) in modes {
            if ui.selectable_label(app.theme_mode == mode, name).clicked() {
                app.theme_mode = mode;
                app.theme = crate::ui::theme::Theme::from_mode(mode, app.accent);
            }
        }
    });

    ui.add_space(8.0);
    ui.label(egui::RichText::new("Equalizer (10-band)").strong());
    ui.checkbox(&mut app.settings.eq_enabled, "Enable EQ");
    let freqs = crate::audio::dsp::EQ_BAND_FREQS;
    ui.horizontal(|ui| {
        for (i, gain) in app.settings.eq_gains_db.iter_mut().enumerate() {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(format!("{:.0}", freqs[i])).small());
                ui.add(
                    egui::Slider::new(gain, -12.0..=12.0)
                        .vertical()
                        .show_value(false),
                );
                ui.label(egui::RichText::new(format!("{gain:+.0}")).small());
            });
        }
    });
    app.player
        .set_eq(app.settings.eq_enabled, app.settings.eq_gains_db);

    ui.add_space(8.0);
    ui.label(egui::RichText::new("Account").strong());
    if app.demo {
        ui.label("Running in demo mode. Set FASTCLOUD_CLIENT_ID/SECRET to connect a real account.");
    } else {
        ui.label("OAuth status is managed via the system keyring.");
    }

    ui.add_space(8.0);
    if ui.button("Save settings").clicked() {
        if let Err(e) = app.settings.save() {
            app.toast(format!("Failed to save: {e}"));
        } else {
            app.toast("Settings saved");
        }
    }
}

fn demo_tracks(app: &mut App, ui: &mut egui::Ui) {
    let tracks = crate::demo::demo_tracks();
    track_list(app, ui, &tracks);
}

fn track_list(app: &mut App, ui: &mut egui::Ui, tracks: &[Track]) {
    let owned: Vec<Track> = tracks.to_vec();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, track) in owned.iter().enumerate() {
            let t = track.clone();
            let queue = owned.clone();
            let idx = i;
            let play = move |app: &mut App| {
                app.player.play_queue(queue, idx, false);
            };
            widgets::track_row(app, ui, &t, idx, play, None);
        }
    });
}
fn header(app: &mut App, ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.heading(egui::RichText::new(title).strong().color(app.theme.text));
    ui.add_space(6.0);
}
