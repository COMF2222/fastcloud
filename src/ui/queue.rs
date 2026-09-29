use eframe::egui;

use super::App;
use super::design::widgets::{self as airwave, ButtonVariant, Surface};
use super::icons::{self, Icon};
use super::theme::{Metrics, Type};

/// Right-side queue panel: now playing, explicit actions, then playback order.
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let (current, upcoming): (
        Option<crate::api::models::Track>,
        Vec<(usize, crate::api::models::Track)>,
    ) = app.player.snapshot();

    ui.label(Type::MICRO.rich("PLAY QUEUE", app.theme.accent));
    ui.horizontal(|ui| {
        ui.heading(Type::H2.rich("Next up", app.theme.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::show(ui, Icon::X, 16.0, app.theme.text_dim)
                .on_hover_text("Close")
                .clicked()
            {
                app.show_queue = false;
            }
        });
    });
    ui.label(Type::CAPTION.rich(
        &format!("{} tracks waiting", upcoming.len()),
        app.theme.text_dim,
    ));
    ui.add_space(Metrics::SP_15);

    if let Some(track) = current.as_ref() {
        Surface::clear().show(ui, app.theme, |ui| {
            ui.label(Type::MICRO.rich("NOW PLAYING", app.theme.accent));
            ui.add_space(Metrics::SP_HALF);
            queue_track_identity(app, ui, track, None);
        });
        ui.add_space(Metrics::SP_15);
    }

    ui.horizontal(|ui| {
        let save = ui
            .add_enabled_ui(current.is_some(), |ui| {
                airwave::action_button(ui, app.theme, ButtonVariant::Secondary, "Save queue")
                    .clicked()
            })
            .inner;
        if save {
            let ids = app.player.queue_track_ids();
            if ids.is_empty() {
                app.toast("Queue is empty");
            } else {
                let n = ids.len();
                app.create_playlist_with_tracks(format!("Queue ({n} tracks)"), ids);
                app.toast(format!("Saving {n} tracks as a playlist…"));
            }
        }
        let clear = ui
            .add_enabled_ui(!upcoming.is_empty(), |ui| {
                airwave::action_button(ui, app.theme, ButtonVariant::Secondary, "Clear").clicked()
            })
            .inner;
        if clear {
            let n = app.player.clear_upcoming();
            app.toast(if n == 0 {
                "Queue is already empty".to_owned()
            } else {
                format!("Cleared {n} upcoming")
            });
        }
    });
    ui.add_space(Metrics::SP_2);
    ui.label(Type::MICRO.rich("UP NEXT", app.theme.text_dim));
    ui.add_space(Metrics::SP_HALF);

    egui::ScrollArea::vertical()
        .id_salt("queue-list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if upcoming.is_empty() && current.is_none() {
                ui.label(Type::BODY.rich("Nothing next — play something.", app.theme.text_dim));
            }
            for (order, (idx, track)) in upcoming.iter().enumerate() {
                queue_row(app, ui, track, *idx, order + 1);
            }

            ui.add_space(Metrics::SP_125);
            ui.separator();
            ui.add_space(Metrics::SP_1);
            ui.horizontal(|ui| {
                ui.label(Type::H4.rich("Autoplay station", app.theme.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut on = app.settings.autoplay;
                    if ui.checkbox(&mut on, "").changed() {
                        app.settings.autoplay = on;
                        app.player.set_autoplay(on);
                        if let Err(error) = app.settings.save() {
                            app.toast(format!("Failed to save: {error}"));
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

fn queue_row(
    app: &mut App,
    ui: &mut egui::Ui,
    track: &crate::api::models::Track,
    idx: usize,
    order: usize,
) {
    let row = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(8, 6))
        .corner_radius(Metrics::RADIUS_INPUT)
        .show(ui, |ui| {
            queue_track_identity(app, ui, track, Some((idx, order)))
        });
    if row.response.hovered() {
        ui.painter().rect_stroke(
            row.response.rect,
            Metrics::RADIUS_INPUT,
            egui::Stroke::new(1.0, app.theme.tokens.glass_border),
            egui::StrokeKind::Inside,
        );
    }
    ui.scope_builder(egui::UiBuilder::new().max_rect(row.response.rect), |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(Metrics::SP_HALF);
            let more =
                icons::show(ui, Icon::Ellipsis, 16.0, app.theme.text_dim).on_hover_text("More");
            egui::Popup::menu(&more).show(|ui| row_menu(app, ui, track, idx));
            let heart_tint = if app.is_liked(track.id) {
                app.theme.accent
            } else {
                app.theme.text_dim
            };
            if icons::show(ui, Icon::Heart, 15.0, heart_tint)
                .on_hover_text("Like")
                .clicked()
            {
                let liked = app.toggle_like(track.id);
                app.toast(if liked { "Liked" } else { "Unliked" });
            }
        });
    });
    ui.add_space(Metrics::SP_HALF);
}

fn queue_track_identity(
    app: &mut App,
    ui: &mut egui::Ui,
    track: &crate::api::models::Track,
    target: Option<(usize, usize)>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = Metrics::SP_1;
        if let Some((_, order)) = target {
            ui.label(Type::MICRO.rich(&format!("{order:02}"), app.theme.text_dim));
        }
        let artwork = super::widgets::artwork_img(
            ui,
            track.artwork_url(),
            track.id,
            &track.title,
            Metrics::artwork(48.0),
            Metrics::RADIUS as f32,
        );
        if let Some((idx, _)) = target
            && artwork.clicked()
        {
            app.player.skip_to(idx);
        }
        ui.vertical(|ui| {
            let reserved_actions = if target.is_some() { 66.0 } else { 8.0 };
            ui.set_width((ui.available_width() - reserved_actions).max(80.0));
            let title = ui
                .add(
                    egui::Label::new(
                        Type::H4.rich(&crate::bidi::owned(&track.title), app.theme.text),
                    )
                    .sense(egui::Sense::click())
                    .truncate(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if let Some((idx, _)) = target
                && title.clicked()
            {
                app.player.skip_to(idx);
            }
            ui.label(Type::CAPTION.rich(&crate::bidi::owned(track.artist()), app.theme.text_dim));
        });
    });
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
    menu_item(ui, Icon::External, "Share", || app.toast("Link copied"));
    menu_item(ui, Icon::ListPlus, "Add to Next up", || {
        app.player.enqueue(vec![track.clone()], true);
        app.toast("Will play next");
    });
    menu_item(ui, Icon::X, "Remove from Next up", || {
        app.player.remove_at(idx);
    });
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
        for (playlist_id, name) in playlists {
            if ui.button(name.clone()).clicked() {
                let list = app.add_to_playlist(playlist_id, track.id);
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
