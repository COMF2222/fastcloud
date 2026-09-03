use crate::api::models::Track;
use crate::util::{fmt_duration_ms, play_count};
use eframe::egui;

use super::App;
use super::route::TrackDragPayload;

/// One track row: index, title, artist, plays, duration, play/like buttons,
/// draggable into playlists.
pub fn track_row(
    app: &mut App,
    ui: &mut egui::Ui,
    track: &Track,
    index: usize,
    on_play: impl FnOnce(&mut App),
    on_like: Option<&dyn Fn(&mut App)>,
) {
    let is_current = {
        let st = app.player.state.lock();
        st.current
            .map(|i| st.queue.get(i).map(|t| t.id == track.id).unwrap_or(false))
            .unwrap_or(false)
    };
    let track_id = track.id;

    let dnd_id = egui::Id::new(("track-dnd", track_id));
    let payload = TrackDragPayload { track_id };

    let body = ui.dnd_drag_source(dnd_id, payload, |ui| {
        row_contents(app, ui, track, index, is_current, on_play, on_like);
    });
    let _ = body;
}

#[allow(clippy::too_many_arguments)]
fn row_contents(
    app: &mut App,
    ui: &mut egui::Ui,
    track: &Track,
    index: usize,
    is_current: bool,
    on_play: impl FnOnce(&mut App),
    on_like: Option<&dyn Fn(&mut App)>,
) {
    egui::Frame::new()
        .fill(if is_current {
            egui::Color32::from_rgba_unmultiplied(
                app.theme.accent.r(),
                app.theme.accent.g(),
                app.theme.accent.b(),
                30,
            )
        } else {
            egui::Color32::TRANSPARENT
        })
        .inner_margin(egui::Margin::symmetric(8, 6))
        .corner_radius(6.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{}", index + 1))
                        .small()
                        .color(app.theme.text_dim)
                        .monospace(),
                );
                ui.label(
                    egui::RichText::new(&track.title)
                        .strong()
                        .color(if is_current {
                            app.theme.accent
                        } else {
                            app.theme.text
                        }),
                );
                ui.label(
                    egui::RichText::new(track.artist())
                        .small()
                        .color(app.theme.text_dim),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(plays) = track.playback_count {
                        ui.label(
                            egui::RichText::new(format!("▶ {}", play_count(plays)))
                                .small()
                                .color(app.theme.text_dim),
                        );
                    }
                    ui.label(
                        egui::RichText::new(fmt_duration_ms(track.effective_duration_ms()))
                            .small()
                            .color(app.theme.text_dim),
                    );
                    if let Some(on_like) = on_like {
                        if ui.button("❤").on_hover_text("Like").clicked() {
                            on_like(app);
                        }
                    }
                    if ui.button("▶").on_hover_text("Play").clicked() {
                        on_play(app);
                    }
                });
            });
        });
}

/// A playlist drop zone; returns dropped track id when released here this frame.
pub fn playlist_drop_zone(ui: &mut egui::Ui, _id: egui::Id, label: &str) -> Option<u64> {
    let frame = egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 4));
    let (inner, payload) = ui.dnd_drop_zone::<TrackDragPayload, _>(frame, |ui| {
        ui.set_min_width(ui.available_width() * 0.999);
        ui.label(egui::RichText::new(label).small());
    });
    let _ = inner;
    payload.map(|p| p.track_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_carries_track_id() {
        let p = TrackDragPayload { track_id: 42 };
        assert_eq!(p.track_id, 42);
    }
}
