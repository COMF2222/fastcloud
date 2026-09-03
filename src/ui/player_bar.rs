use super::App;
use crate::util::fmt_duration_ms;
use eframe::egui;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let st = app.player.state.lock();
    let track = st.current.and_then(|i| st.queue.get(i).cloned());
    let playing = st.is_playing;
    let pos = st.position_ms;
    let dur = st.duration_ms.max(1);
    let loading = st.loading;
    let volume = st.volume;
    let shuffle = st.shuffle;
    let repeat = st.repeat;
    let preview = st.preview_fallback;
    drop(st);

    egui::Frame::new()
        .fill(app.theme.surface)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        track
                            .as_ref()
                            .map(|t| t.title.clone())
                            .unwrap_or_else(|| "Nothing playing".into()),
                    )
                    .strong()
                    .color(app.theme.text)
                    .size(14.0),
                );
                ui.label(
                    egui::RichText::new(
                        track
                            .as_ref()
                            .map(|t| t.artist().to_string())
                            .unwrap_or_default(),
                    )
                    .color(app.theme.text_dim)
                    .size(12.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(volume_icon(volume))
                        .on_hover_text("Volume")
                        .clicked()
                    {
                        let next = if volume < 0.01 {
                            0.5
                        } else if volume < 0.7 {
                            1.0
                        } else {
                            0.0
                        };
                        app.player.set_volume(next);
                    }
                    let slider = egui::Slider::new(&mut app.settings.volume, 0.0..=1.0)
                        .show_value(false)
                        .prefix("vol ");
                    if ui.add(slider).changed() {
                        app.player.set_volume(app.settings.volume);
                    }
                    ui.separator();
                    let repeat_label = match repeat {
                        crate::player::RepeatMode::Off => "repeat",
                        crate::player::RepeatMode::All => "repeat*",
                        crate::player::RepeatMode::One => "repeat1",
                    };
                    if ui.button(repeat_label).clicked() {
                        app.player.cycle_repeat();
                    }
                    if ui
                        .button(if shuffle { "shuffle*" } else { "shuffle" })
                        .on_hover_text("Shuffle")
                        .clicked()
                    {
                        app.player.toggle_shuffle();
                    }
                    if ui.button("next").clicked() {
                        app.player.next();
                    }
                    let play_label = if loading {
                        "..."
                    } else if playing {
                        "pause"
                    } else {
                        "play"
                    };
                    if ui.button(play_label).clicked() {
                        app.player.play_pause();
                    }
                    if ui.button("prev").clicked() {
                        app.player.prev();
                    }
                });
            });

            if preview {
                ui.label(
                    egui::RichText::new("preview (30s) — full stream needs authorized API access")
                        .small()
                        .color(app.theme.text_dim),
                );
            }

            // Seek bar
            let mut seek = pos as f64;
            let slider = egui::Slider::new(&mut seek, 0.0..=dur as f64)
                .show_value(false)
                .text("");
            let resp = ui.add(slider);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(fmt_duration_ms(pos))
                        .small()
                        .color(app.theme.text_dim),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(fmt_duration_ms(dur))
                            .small()
                            .color(app.theme.text_dim),
                    );
                });
            });
            if resp.drag_stopped() || (resp.lost_focus() && !resp.has_focus()) {
                app.player.seek_ms(seek as u64);
            }
        });
}

fn volume_icon(v: f32) -> &'static str {
    if v < 0.01 {
        "mute"
    } else if v < 0.5 {
        "vol-low"
    } else {
        "vol-high"
    }
}
