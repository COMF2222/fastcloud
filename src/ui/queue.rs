use super::App;
use eframe::egui;

use super::icons::{self, Icon};
use super::theme::{Metrics, Type};

/// Right-side queue panel: now playing + next up in play order.
/// Click a row to skip to it, × removes it, Clear drops everything
/// except the current track. ••• opens the row menu.
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let (current, upcoming): (
        Option<crate::api::models::Track>,
        Vec<(usize, crate::api::models::Track)>,
    ) = app.player.snapshot();

    ui.horizontal(|ui| {
        ui.heading(Type::H3.rich("Next up", app.theme.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::show(ui, Icon::X, 16.0, app.theme.text_dim)
                .on_hover_text("Close")
                .clicked()
            {
                app.show_queue = false;
            }
            let clear = ui
                .add_enabled(!upcoming.is_empty(), egui::Button::new("Clear"))
                .on_hover_text("Remove everything except now playing");
            if clear.clicked() {
                let n = app.player.clear_upcoming();
                app.toast(if n == 0 {
                    "Queue is already empty".to_owned()
                } else {
                    format!("Cleared {n} upcoming")
                });
            }
            let save = ui
                .add_enabled(current.is_some(), egui::Button::new("Save"))
                .on_hover_text("Save the queue as a playlist");
            if save.clicked() {
                let ids = app.player.queue_track_ids();
                if ids.is_empty() {
                    app.toast("Queue is empty");
                } else {
                    let n = ids.len();
                    app.create_playlist_with_tracks(format!("Queue ({n} tracks)"), ids);
                    app.toast(format!("Saving {n} tracks as a playlist…"));
                }
            }
        });
    });
    ui.add_space(Metrics::SP_HALF);

    egui::ScrollArea::vertical()
        .id_salt("queue-list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if upcoming.is_empty() && current.is_none() {
                ui.label(Type::BODY.rich("Nothing next — play something.", app.theme.text_dim));
            }
            for (idx, t) in &upcoming {
                queue_row(app, ui, t, *idx);
            }

            ui.add_space(Metrics::SP_125);
            ui.separator();
            ui.add_space(Metrics::SP_HALF);
            ui.horizontal(|ui| {
                ui.label(Type::H4.rich("Autoplay station", app.theme.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut on = app.settings.autoplay;
                    if ui.checkbox(&mut on, "").changed() {
                        app.settings.autoplay = on;
                        app.player.set_autoplay(on);
                        if let Err(e) = app.settings.save() {
                            app.toast(format!("Failed to save: {e}"));
                        } else {
                            app.toast(if on {
                                "Autoplay on: similar tracks keep playing"
                            } else {
                                "Autoplay off"
                            });
                        }
                    }
                });
            });
            ui.label(Type::CAPTION.rich(
                "Hear related tracks based on what's playing now.",
                app.theme.text_dim,
            ));
        });
}

fn queue_row(app: &mut App, ui: &mut egui::Ui, track: &crate::api::models::Track, idx: usize) {
    let track_id = track.id;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = Metrics::SP_1;
        if super::widgets::artwork_img(
            ui,
            track.artwork_url(),
            track_id,
            &track.title,
            Metrics::artwork(44.0),
            Metrics::RADIUS as f32,
        )
        .clicked()
        {
            app.player.skip_to(idx);
        }
        ui.vertical(|ui| {
            ui.set_min_width(60.0);
            ui.label(Type::CAPTION.rich(&crate::bidi::owned(track.artist()), app.theme.text_dim));
            let title = Type::H4.rich(&crate::bidi::owned(&track.title), app.theme.text);
            if ui
                .add(
                    egui::Label::new(title)
                        .sense(egui::Sense::click())
                        .truncate(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                app.player.skip_to(idx);
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // ••• row menu.
            let more =
                icons::show(ui, Icon::Ellipsis, 16.0, app.theme.text_dim).on_hover_text("More");
            egui::Popup::menu(&more).show(|ui| {
                row_menu(app, ui, track, idx);
            });
            let heart_tint = if app.is_liked(track_id) {
                app.theme.accent
            } else {
                app.theme.text_dim
            };
            if icons::show(ui, Icon::Heart, 15.0, heart_tint)
                .on_hover_text("Like")
                .clicked()
            {
                let now = app.toggle_like(track_id);
                app.toast(if now { "Liked" } else { "Unliked" });
            }
        });
    });
    ui.add_space(Metrics::SP_QUARTER);
}

fn row_menu(app: &mut App, ui: &mut egui::Ui, track: &crate::api::models::Track, idx: usize) {
    let liked = app.is_liked(track.id);
    menu_item(
        ui,
        Icon::Heart,
        if liked { "Liked" } else { "Like" },
        || {
            app.toggle_like(track.id);
            app.toast(if liked { "Unliked" } else { "Liked" });
        },
    );
    let title = track.title.clone();
    menu_item(ui, Icon::Repeat, "Repost", || {
        app.toast(format!("Reposted {title}"));
    });
    menu_item(ui, Icon::External, "Share", || {
        app.toast("Link copied");
    });
    menu_item(ui, Icon::ListPlus, "Add to Next up", || {
        app.player.enqueue(vec![track.clone()], true);
        app.toast("Will play next");
    });
    menu_item(ui, Icon::X, "Remove from Next up", || {
        app.player.remove_at(idx);
    });
    // Add to Playlist submenu.
    ui.menu_button("Add to Playlist", |ui| {
        let playlists: Vec<(u64, String)> = if app.demo {
            app.settings
                .custom_playlists
                .iter()
                .map(|playlist| (playlist.id, playlist.title.clone()))
                .collect()
        } else {
            app.playlists(crate::store::Key::MyPlaylists)
                .rows()
                .iter()
                .filter(|playlist| !playlist.is_album())
                .map(|playlist| (playlist.id, playlist.title.clone()))
                .collect()
        };
        if playlists.is_empty() {
            ui.label("No playlists yet");
        }
        for (pid, name) in playlists {
            if ui.button(name.clone()).clicked() {
                let list = app.add_to_playlist(pid, track.id);
                app.toast(if list == "playlist" {
                    format!("Adding to {name}…")
                } else {
                    format!("Added to {list}")
                });
                ui.close();
            }
        }
    });
    menu_item(ui, Icon::Music, "Start station", || {
        app.settings.autoplay = true;
        app.player.set_autoplay(true);
        let queue = app.station_queue(track);
        app.play_user_queue(queue, 0, false);
    });
}

fn menu_item(ui: &mut egui::Ui, icon: Icon, label: &str, mut action: impl FnMut()) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = Metrics::SP_1;
        icons::show_static(ui, icon, 14.0, ui.visuals().text_color());
        if ui.button(label).clicked() {
            action();
            ui.close();
        }
    });
}
