use super::App;
use crate::util::fmt_duration_ms;
use eframe::egui;
use egui::{Align, Layout, Margin, Rect, UiBuilder, pos2, vec2};

use super::icons::{self, Icon};
use super::theme::{Metrics, Type};

/// Airwave's persistent bottom player deck. Fixed bands keep artwork,
/// transport, timeline and actions stable while labels and window width vary.
pub const BAR_HEIGHT: f32 = Metrics::PLAYER_H;
const COVER: f32 = 52.0;
const WAVEFORM_AT: f32 = 1_000.0;
const MIN_CENTER_W: f32 = 300.0;
const COMPACT_ACTIONS_W: f32 = 88.0;
const WIDE_ACTIONS_W: f32 = 140.0;

const TIME_W: f32 = 40.0;
const VOLUME_ICON_W: f32 = 28.0;
const CONTROL_GAP: f32 = 8.0;
const MIN_PROGRESS_W: f32 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimelineStyle {
    Line,
    Waveform,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DeckLayout {
    now_playing_width: f32,
    center_width: f32,
    actions_width: f32,
    timeline: TimelineStyle,
}

impl DeckLayout {
    fn resolve(width: f32) -> Self {
        let wide = width >= WAVEFORM_AT;
        let actions_width = if wide {
            WIDE_ACTIONS_W
        } else {
            COMPACT_ACTIONS_W
        };
        let preferred_now_playing = if wide {
            (width * 0.28).clamp(280.0, 340.0)
        } else {
            (width * 0.30).clamp(210.0, 240.0)
        };
        let now_playing_width =
            preferred_now_playing.min((width - actions_width - MIN_CENTER_W).max(0.0));
        Self {
            now_playing_width,
            center_width: (width - now_playing_width - actions_width).max(0.0),
            actions_width,
            timeline: if wide {
                TimelineStyle::Waveform
            } else {
                TimelineStyle::Line
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ProgressLayout {
    bar_width: f32,
}

fn progress_fixed_width(_preview: bool) -> f32 {
    TIME_W * 2.0 + CONTROL_GAP * 2.0
}

fn progress_layout(width: f32, preview: bool) -> ProgressLayout {
    let fixed = progress_fixed_width(preview);
    let bar_width = (width - fixed).max(MIN_PROGRESS_W);
    ProgressLayout { bar_width }
}

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
    let error = st.error.clone();
    drop(st);
    let waveform_seed = track.as_ref().map_or(0, |track| track.id);
    let waveform = track
        .as_ref()
        .and_then(|track| track.waveform_url.as_deref())
        .and_then(|url| app.waveforms.get(ui.ctx(), url));

    egui::Panel::bottom(egui::Id::new("player_bar"))
        .exact_size(BAR_HEIGHT)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::TRANSPARENT)
                .inner_margin(Margin::ZERO),
        )
        .show(ui, |ui| {
            let full = ui.max_rect();
            let deck_width = (full.width() - 28.0).clamp(0.0, 1_000.0);
            let rect = Rect::from_center_size(full.center(), vec2(deck_width, 76.0));
            ui.painter().add(
                egui::epaint::Shadow {
                    offset: [0, 9],
                    blur: 30,
                    spread: 2,
                    color: egui::Color32::from_black_alpha(150),
                }
                .as_shape(rect, Metrics::RADIUS_LG),
            );
            ui.painter()
                .rect_filled(rect, Metrics::RADIUS_LG, app.theme.tokens.glass_strong);
            ui.painter().rect_stroke(
                rect,
                Metrics::RADIUS_LG,
                egui::Stroke::new(1.0, app.theme.tokens.glass_border),
                egui::StrokeKind::Inside,
            );
            ui.painter().line_segment(
                [
                    pos2(rect.left() + 20.0, rect.top() + 1.0),
                    pos2(rect.right() - 20.0, rect.top() + 1.0),
                ],
                egui::Stroke::new(1.0, app.theme.tokens.glass_highlight),
            );
            ui.painter().line_segment(
                [
                    pos2(rect.left() + 18.0, rect.top() + 0.5),
                    pos2(rect.left() + 150.0, rect.top() + 0.5),
                ],
                egui::Stroke::new(1.0, app.theme.accent.gamma_multiply(0.55)),
            );
            let layout = DeckLayout::resolve(rect.width());
            let now_playing = Rect::from_min_max(
                rect.min,
                pos2(rect.left() + layout.now_playing_width, rect.bottom()),
            );
            let actions = Rect::from_min_max(
                pos2(rect.right() - layout.actions_width, rect.top()),
                rect.max,
            );
            let center = Rect::from_min_max(
                pos2(now_playing.right(), rect.top()),
                pos2(actions.left(), rect.bottom()),
            );
            let transport_rect =
                Rect::from_min_max(center.min, pos2(center.right(), center.top() + 45.0));
            let timeline_rect = Rect::from_min_max(
                pos2(center.left(), transport_rect.bottom() - 3.0),
                center.max,
            );

            track_cluster(app, ui, now_playing, track.as_ref());
            transport(app, ui, transport_rect, loading, playing, shuffle, repeat);
            progress(
                app,
                ui,
                timeline_rect,
                pos,
                dur,
                preview,
                layout.timeline,
                waveform_seed,
                waveform.as_deref(),
            );
            player_actions(
                app,
                ui,
                actions,
                volume,
                layout.timeline == TimelineStyle::Waveform,
            );

            if let Some(err) = error {
                app.toast_once(err);
            } else {
                app.clear_error_notice();
            }
        });
}

// ===== Centre: transport above the adaptive timeline =====

fn transport(
    app: &mut App,
    ui: &mut egui::Ui,
    rect: Rect,
    loading: bool,
    playing: bool,
    shuffle: bool,
    repeat: crate::player::RepeatMode,
) {
    let cy = rect.center().y;
    let ink = app.theme.text;
    let dim = app.theme.text_dim;
    let accent = app.theme.accent;

    let widths = [30.0, 40.0, 30.0, 28.0, 28.0];
    let gap = 6.0;
    let total: f32 = widths.iter().sum::<f32>() + gap * 4.0;
    let mut x = rect.center().x - total / 2.0;
    let mut slot = |width: f32| {
        let r = Rect::from_center_size(pos2(x + width / 2.0, cy), vec2(width, 32.0));
        x += width + gap;
        r
    };
    let centered = |ui: &mut egui::Ui, r: Rect| {
        ui.new_child(
            UiBuilder::new()
                .max_rect(r)
                .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
        )
    };

    let mut cell = centered(ui, slot(widths[0]));
    if icons::icon_button(
        &mut cell,
        Icon::SkipBack,
        16.0,
        widths[0],
        ink,
        ink,
        "Previous",
    )
    .clicked()
    {
        app.player.prev();
    }

    // Play disc: white circle, dark glyph — like soundcloud.com.
    // (theme.text is near-white on dark, near-black on light, so the disc
    // reads on both; the glyph takes the bar colour.)
    let disc = slot(widths[1]);
    if loading {
        ui.painter().circle_filled(disc.center(), 20.0, ink);
        let mut cell = centered(ui, disc);
        icons::spinner(&mut cell, 20.0, app.theme.tokens.surface_raised);
    } else {
        let mut cell = centered(ui, disc);
        if icons::circle_button(
            &mut cell,
            if playing { Icon::Pause } else { Icon::Play },
            40.0,
            ink,
            ink,
            app.theme.tokens.surface_raised,
            if playing {
                "Pause (Space)"
            } else {
                "Play (Space)"
            },
        )
        .clicked()
        {
            app.player.play_pause();
        }
    }

    let mut cell = centered(ui, slot(widths[2]));
    if icons::icon_button(
        &mut cell,
        Icon::SkipForward,
        16.0,
        widths[2],
        ink,
        ink,
        "Next",
    )
    .clicked()
    {
        app.player.next();
    }

    let mut cell = centered(ui, slot(widths[3]));
    let sh = if shuffle { accent } else { dim };
    if icons::icon_button(
        &mut cell,
        Icon::Shuffle,
        15.0,
        widths[3],
        sh,
        if shuffle { accent } else { ink },
        "Shuffle",
    )
    .clicked()
    {
        app.player.toggle_shuffle();
    }

    let (rep_icon, rep_color) = match repeat {
        crate::player::RepeatMode::Off => (Icon::Repeat, dim),
        crate::player::RepeatMode::All => (Icon::Repeat, accent),
        crate::player::RepeatMode::One => (Icon::Repeat1, accent),
    };
    let mut cell = centered(ui, slot(widths[4]));
    if icons::icon_button(
        &mut cell,
        rep_icon,
        15.0,
        widths[4],
        rep_color,
        if repeat == crate::player::RepeatMode::Off {
            ink
        } else {
            accent
        },
        "Repeat",
    )
    .clicked()
    {
        app.player.cycle_repeat();
    }
}

// ===== Timeline: waveform on wide windows, hairline on compact ones =====

#[expect(
    clippy::too_many_arguments,
    reason = "timeline state is frame-local UI data"
)]
fn progress(
    app: &mut App,
    ui: &mut egui::Ui,
    rect: Rect,
    pos: u64,
    dur: u64,
    preview: bool,
    style: TimelineStyle,
    waveform_seed: u64,
    waveform: Option<&[f32]>,
) {
    // Fixed reservations: times and gaps. The slider takes the rest; volume
    // has its own non-overlapping band to the right.
    let layout = progress_layout(rect.width(), preview);
    let bar_w = layout.bar_width;

    // Fill the whole band: SoundCloud's middle section runs from the
    // transport to the track cluster with no dead space, so the artwork and
    // its text sit hard against the window's right edge.
    let mut x = rect.left();
    let mut take = |w: f32| {
        let r = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));
        x += w + CONTROL_GAP;
        r
    };

    // Times are painted, not laid out — changing digits never shift the bar.
    let elapsed_rect = take(TIME_W);
    let shown_pos = match app.seek_preview {
        Some(f) => (f as f64 * dur as f64) as u64,
        None => pos,
    };
    ui.painter().text(
        elapsed_rect.center(),
        egui::Align2::CENTER_CENTER,
        fmt_duration_ms(shown_pos),
        egui::FontId::monospace(11.0),
        app.theme.text_dim,
    );

    let bar_rect = take(bar_w);
    let mut bar_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(bar_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let frac = if dur > 0 {
        pos as f32 / dur as f32
    } else {
        0.0
    };
    let shown_frac = app.seek_preview.unwrap_or(frac);
    let event = match style {
        TimelineStyle::Line => thin_slider(
            &mut bar_ui,
            shown_frac,
            bar_w,
            app.theme.accent,
            app.theme.separator,
        ),
        TimelineStyle::Waveform => waveform_slider(
            &mut bar_ui,
            shown_frac,
            bar_w,
            waveform_seed,
            waveform,
            app.theme.accent,
            app.theme.tokens.text_tertiary,
        ),
    };
    match event {
        SliderEvent::Dragging(v) => {
            app.seek_preview = Some(v);
        }
        SliderEvent::Committed(v) => {
            app.seek_preview = None;
            if dur > 0 {
                app.player.seek_ms((v as f64 * dur as f64) as u64);
            }
        }
        SliderEvent::None => {}
    }

    let dur_rect = take(TIME_W);
    ui.painter().text(
        dur_rect.center(),
        egui::Align2::CENTER_CENTER,
        fmt_duration_ms(dur),
        egui::FontId::monospace(11.0),
        app.theme.text_dim,
    );

    if preview {
        ui.painter().text(
            pos2(bar_rect.right() - 2.0, bar_rect.top() + 3.0),
            egui::Align2::RIGHT_TOP,
            "PREVIEW",
            egui::FontId::proportional(9.0),
            app.theme.accent,
        );
    }
}

// ===== Right: queue, mini-player and volume =====

fn player_actions(app: &mut App, ui: &mut egui::Ui, rect: Rect, volume: f32, show_mini: bool) {
    let cell = 36.0;
    let gap = 4.0;
    let count = if show_mini { 3.0 } else { 2.0 };
    let total = cell * count + gap * (count - 1.0);
    let mut x = rect.center().x - total / 2.0;
    let mut take = || {
        let area = Rect::from_center_size(pos2(x + cell / 2.0, rect.center().y), vec2(cell, cell));
        x += cell + gap;
        area
    };
    let child = |ui: &mut egui::Ui, area: Rect| {
        ui.new_child(
            UiBuilder::new()
                .max_rect(area)
                .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
        )
    };

    if show_mini {
        let mut mini_ui = child(ui, take());
        let tint = if app.mini_open() {
            app.theme.accent
        } else {
            app.theme.text_dim
        };
        if icons::icon_button(
            &mut mini_ui,
            Icon::Shrink,
            16.0,
            cell,
            tint,
            app.theme.text,
            "Open mini-player (Ctrl+M)",
        )
        .clicked()
        {
            app.toggle_mini();
        }
    }

    let mut queue_ui = child(ui, take());
    let queue_tint = if app.show_queue {
        app.theme.accent
    } else {
        app.theme.text_dim
    };
    if icons::icon_button(
        &mut queue_ui,
        Icon::Queue,
        17.0,
        cell,
        queue_tint,
        app.theme.text,
        if app.show_queue {
            "Close Next up (Q)"
        } else {
            "Open Next up (Q)"
        },
    )
    .clicked()
    {
        app.show_queue = !app.show_queue;
    }

    volume_control(app, ui, take(), volume);
}

// ===== Volume: compact popover with a tapered gain curve =====

fn volume_control(app: &mut App, ui: &mut egui::Ui, rect: Rect, volume: f32) {
    let cy = rect.center().y;
    let shown_gain = app.volume_preview.unwrap_or(volume).clamp(0.0, 1.0);
    // Only an actual zero is mute. With the tapered slider, very quiet audible
    // values can legitimately be below one percent.
    let muted = shown_gain <= f32::EPSILON;
    let slider_position = volume_position_from_gain(shown_gain);
    let icon_rect = Rect::from_min_size(pos2(rect.left(), cy - 14.0), vec2(VOLUME_ICON_W, 28.0));
    let mut icon_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(icon_rect)
            .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
    );
    let vol_icon = if muted {
        Icon::VolumeX
    } else if shown_gain < 0.5 {
        Icon::Volume1
    } else {
        Icon::Volume2
    };
    if icons::icon_button(
        &mut icon_ui,
        vol_icon,
        15.0,
        VOLUME_ICON_W,
        app.theme.text_dim,
        app.theme.text,
        if muted { "Unmute" } else { "Mute" },
    )
    .clicked()
    {
        let next = if muted {
            if app.last_volume < 0.01 {
                0.8
            } else {
                app.last_volume
            }
        } else {
            app.last_volume = shown_gain;
            0.0
        };
        app.volume_preview = None;
        app.settings.volume = next;
        app.player.set_volume(next);
    }
    // A foreground area escapes the player's clipping rectangle. A short
    // grace period lets the pointer cross from the speaker into the popover.
    let popup = Rect::from_min_size(
        pos2(icon_rect.center().x - 22.0, rect.top() - 144.0),
        vec2(44.0, 144.0),
    );
    let hover_id = ui.id().with("volume-hover-until");
    let now = ui.input(|input| input.time);
    let open_until = ui
        .data(|data| data.get_temp::<f64>(hover_id))
        .unwrap_or(0.0);
    let over_icon = ui.rect_contains_pointer(icon_rect);
    let over_popup = ui.input(|input| {
        input
            .pointer
            .hover_pos()
            .is_some_and(|pos| popup.contains(pos))
    });
    let open = over_icon || now < open_until || app.volume_preview.is_some();
    if over_icon || (open && over_popup) {
        ui.data_mut(|data| data.insert_temp(hover_id, now + 0.25));
    }
    if open {
        egui::Area::new(ui.id().with("volume-popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(popup.min)
            .movable(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(app.theme.surface)
                    .stroke(egui::Stroke::new(1.0, app.theme.separator))
                    .corner_radius(4)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.spacing_mut().slider_width = 118.0;
                        ui.visuals_mut().selection.bg_fill = app.theme.accent;
                        let mut position = slider_position;
                        let response = ui.add(
                            egui::Slider::new(&mut position, 0.0..=1.0)
                                .vertical()
                                .show_value(false),
                        );
                        if response.changed() {
                            let gain = volume_gain_from_position(position);
                            app.settings.volume = gain;
                            app.player.set_volume(gain);
                        }
                        app.volume_preview = response
                            .dragged()
                            .then(|| volume_gain_from_position(position));
                    });
            });
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(250));
    }
    // The wheel over the volume moves it one step a notch. Windows reports a
    // notch as its lines-per-scroll setting (three by default), so a notch is
    // counted per event rather than per reported line — otherwise one flick
    // jumped 15%.
    let over_volume = over_icon || (open && over_popup);
    if over_volume {
        let notches = ui.input(wheel_notches);
        if notches != 0.0 {
            let position = (slider_position + notches * VOLUME_STEP).clamp(0.0, 1.0);
            let next = volume_gain_from_position(position);
            app.volume_preview = None;
            app.settings.volume = next;
            app.player.set_volume(next);
        }
    }
}

/// A quadratic taper gives the quiet end most of the physical slider travel:
/// 25% position is 6.25% gain, while the full-right position remains 100%.
fn volume_gain_from_position(position: f32) -> f32 {
    position.clamp(0.0, 1.0).powi(2)
}

fn volume_position_from_gain(gain: f32) -> f32 {
    gain.clamp(0.0, 1.0).sqrt()
}

/// One wheel notch is 5% of the volume, like soundcloud.com and Winamp.
pub const VOLUME_STEP: f32 = 0.05;

/// Wheel notches from this frame's raw wheel events.
fn wheel_notches(input: &egui::InputState) -> f32 {
    input
        .events
        .iter()
        .filter_map(|event| match event {
            egui::Event::MouseWheel { unit, delta, .. } => Some(notches_of(*unit, delta.y)),
            _ => None,
        })
        .sum()
}

/// Notches in one wheel event.
///
/// A `Line`/`Page` event is one detent however many lines the system asks
/// for; a `Point` event (trackpads, smooth wheels) counts ~50 px a notch and
/// rounds away from zero so a small flick is still worth one step.
pub fn notches_of(unit: egui::MouseWheelUnit, delta_y: f32) -> f32 {
    const NOTCH_PX: f32 = 50.0;
    if delta_y == 0.0 {
        return 0.0;
    }
    match unit {
        egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => delta_y.signum(),
        egui::MouseWheelUnit::Point => {
            let notches = delta_y / NOTCH_PX;
            if notches.abs() < 1.0 {
                notches.signum()
            } else {
                notches.round()
            }
        }
    }
}

// ===== Left: artwork, metadata and Like =====

fn track_cluster(
    app: &mut App,
    ui: &mut egui::Ui,
    rect: Rect,
    track: Option<&crate::api::models::Track>,
) {
    let cover_rect = Rect::from_center_size(
        pos2(rect.left() + COVER / 2.0, rect.center().y),
        vec2(COVER, COVER),
    );
    let icon_size = 36.0;
    let heart_rect = Rect::from_center_size(
        pos2(rect.right() - icon_size / 2.0, rect.center().y),
        vec2(icon_size, icon_size),
    );
    let text_rect = Rect::from_min_max(
        pos2(cover_rect.right() + Metrics::SP_125, rect.top()),
        pos2(heart_rect.left() - Metrics::SP_HALF, rect.bottom()),
    );
    let child = |ui: &mut egui::Ui, area: Rect| {
        ui.new_child(
            UiBuilder::new()
                .max_rect(area)
                .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
        )
    };
    let Some(t) = track else {
        let mut text_ui = ui.new_child(UiBuilder::new().max_rect(text_rect));
        text_ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new("Nothing playing")
                    .font(Type::H4.font())
                    .color(app.theme.text_dim),
            );
        });
        return;
    };

    let heart_tint = if app.is_liked(t.id) {
        app.theme.accent
    } else {
        app.theme.text_dim
    };
    let mut heart_ui = child(ui, heart_rect);
    if icons::icon_button(
        &mut heart_ui,
        Icon::Heart,
        15.0,
        icon_size,
        heart_tint,
        app.theme.text,
        "Like",
    )
    .clicked()
    {
        let now = app.toggle_like(t.id);
        app.toast(if now {
            format!("Liked {}", t.title)
        } else {
            format!("Removed {} from likes", t.title)
        });
    }

    let mut text_ui = ui.new_child(UiBuilder::new().max_rect(text_rect));
    let title_clicked = text_ui
        .vertical(|ui| {
            ui.set_min_width(text_rect.width());
            ui.set_max_width(text_rect.width());
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.add_space(13.0);
            let title_clicked = ui
                .add(
                    egui::Label::new(Type::H5.rich(&crate::bidi::owned(&t.title), app.theme.text))
                        .sense(egui::Sense::click())
                        .truncate(),
                )
                .on_hover_text(&t.title)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked();
            let artist_clicked = ui
                .add(
                    egui::Label::new(
                        Type::CAPTION.rich(&crate::bidi::owned(t.artist()), app.theme.text_dim),
                    )
                    .sense(egui::Sense::click())
                    .truncate(),
                )
                .on_hover_text(t.artist())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked();
            (title_clicked, artist_clicked)
        })
        .inner;
    let (title_clicked, artist_clicked) = title_clicked;
    if title_clicked {
        let id = t.id;
        app.navigate(super::route::Route::TrackDetail(id));
    }
    if artist_clicked && let Some(u) = &t.user {
        app.navigate(super::route::Route::UserDetail(u.id));
    }
    let mut cover_ui = child(ui, cover_rect);
    if super::widgets::artwork_img(
        &mut cover_ui,
        t.artwork_url(),
        t.id,
        &t.title,
        COVER,
        Metrics::RADIUS as f32,
    )
    .clicked()
    {
        let id = t.id;
        app.navigate(super::route::Route::TrackDetail(id));
    }
}

// ===== Timeline painters =====

enum SliderEvent {
    None,
    Dragging(f32),
    Committed(f32),
}

/// A 2px rail with a filled head, as `.playbackTimeline__progressBackground`
/// (`rgba(--primary-rgb, .15)`) under `.playbackTimeline__progressBar`
/// (`--special-color`). The rail thickens under the pointer, which the site
/// does not do but every desktop player does, and the handle only appears
/// then — so a resting bar is exactly SoundCloud's hairline.
fn thin_slider(
    ui: &mut egui::Ui,
    frac: f32,
    width: f32,
    fill: egui::Color32,
    track_col: egui::Color32,
) -> SliderEvent {
    let frac = frac.clamp(0.0, 1.0);
    let (rect, resp) =
        ui.allocate_exact_size(vec2(width.max(40.0), 16.0), egui::Sense::click_and_drag());
    let active = resp.hovered() || resp.dragged();
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let thickness = if active {
            Metrics::TIMELINE_H * 2.0
        } else {
            Metrics::TIMELINE_H
        };
        let radius = thickness / 2.0;
        let bar = Rect::from_center_size(rect.center(), vec2(rect.width(), thickness));
        p.rect_filled(bar, radius, track_col);
        let w = bar.width() * frac;
        if w > 0.5 {
            p.rect_filled(
                Rect::from_min_size(bar.left_top(), vec2(w, bar.height())),
                radius,
                fill,
            );
        }
        if active {
            p.circle_filled(pos2(bar.left() + w, bar.center().y), 6.0, fill);
            p.circle_filled(
                pos2(bar.left() + w, bar.center().y),
                2.2,
                egui::Color32::WHITE,
            );
        }
    }
    if resp.drag_stopped() {
        if let Some(mx) = resp.interact_pointer_pos() {
            return SliderEvent::Committed(seek_frac(mx.x, rect.left(), rect.width()));
        }
        return SliderEvent::Committed(frac);
    }
    if resp.dragged()
        && let Some(mx) = resp.interact_pointer_pos()
    {
        return SliderEvent::Dragging(seek_frac(mx.x, rect.left(), rect.width()));
    }
    if resp.clicked()
        && let Some(mx) = resp.interact_pointer_pos()
    {
        return SliderEvent::Committed(seek_frac(mx.x, rect.left(), rect.width()));
    }
    SliderEvent::None
}

fn waveform_slider(
    ui: &mut egui::Ui,
    frac: f32,
    width: f32,
    seed: u64,
    samples: Option<&[f32]>,
    fill: egui::Color32,
    track_col: egui::Color32,
) -> SliderEvent {
    let frac = frac.clamp(0.0, 1.0);
    let (rect, response) =
        ui.allocate_exact_size(vec2(width.max(40.0), 20.0), egui::Sense::click_and_drag());
    if ui.is_rect_visible(rect) {
        let bars = ((rect.width() / 4.0).round() as usize).clamp(24, 160);
        let step = rect.width() / bars as f32;
        let center = rect.center().y;
        for index in 0..bars {
            let peak = waveform_peak(samples, seed, index, bars);
            let height = 3.0 + peak * (rect.height() - 4.0);
            let x = rect.left() + (index as f32 + 0.5) * step;
            let played = (index as f32 + 0.5) / bars as f32 <= frac;
            ui.painter().vline(
                x,
                (center - height / 2.0)..=(center + height / 2.0),
                egui::Stroke::new(
                    (step * 0.46).clamp(1.0, 2.0),
                    if played { fill } else { track_col },
                ),
            );
        }
        if response.hovered() || response.dragged() {
            let x = rect.left() + rect.width() * frac;
            ui.painter().circle_filled(pos2(x, center), 4.0, fill);
        }
    }
    if response.drag_stopped() {
        if let Some(pointer) = response.interact_pointer_pos() {
            return SliderEvent::Committed(seek_frac(pointer.x, rect.left(), rect.width()));
        }
        return SliderEvent::Committed(frac);
    }
    if response.dragged()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        return SliderEvent::Dragging(seek_frac(pointer.x, rect.left(), rect.width()));
    }
    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        return SliderEvent::Committed(seek_frac(pointer.x, rect.left(), rect.width()));
    }
    SliderEvent::None
}

fn waveform_peak(samples: Option<&[f32]>, seed: u64, index: usize, bars: usize) -> f32 {
    if let Some(samples) = samples.filter(|samples| !samples.is_empty()) {
        let start = index * samples.len() / bars;
        let end = ((index + 1) * samples.len() / bars)
            .max(start + 1)
            .min(samples.len());
        return samples[start.min(samples.len() - 1)..end]
            .iter()
            .copied()
            .fold(0.0_f32, f32::max)
            .clamp(0.05, 1.0);
    }

    let mut value = seed ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^= value >> 31;
    0.18 + 0.82 * (value as f64 / u64::MAX as f64) as f32
}

fn seek_frac(pointer_x: f32, left: f32, width: f32) -> f32 {
    if width <= 0.0 {
        return 0.0;
    }
    ((pointer_x - left) / width).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{
        DeckLayout, MIN_CENTER_W, TimelineStyle, VOLUME_STEP, WAVEFORM_AT, notches_of,
        progress_fixed_width, progress_layout, seek_frac, volume_gain_from_position,
        volume_position_from_gain, waveform_peak,
    };
    use eframe::egui::MouseWheelUnit;

    #[test]
    fn compact_deck_preserves_the_transport_band() {
        let layout = DeckLayout::resolve(760.0);

        assert!(layout.center_width >= MIN_CENTER_W);
        assert_eq!(layout.timeline, TimelineStyle::Line);
    }

    #[test]
    fn wide_deck_uses_the_airwave_timeline() {
        let layout = DeckLayout::resolve(WAVEFORM_AT);

        assert_eq!(layout.timeline, TimelineStyle::Waveform);
    }

    #[test]
    fn deck_bands_consume_the_available_width() {
        let layout = DeckLayout::resolve(1_280.0);
        let occupied = layout.now_playing_width + layout.center_width + layout.actions_width;

        assert!((occupied - 1_280.0).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_controls_fit_without_touching_the_track_cluster() {
        let layout = progress_layout(320.0, false);

        assert!(progress_fixed_width(false) + layout.bar_width <= 320.0);
        assert!(layout.bar_width >= 40.0);
    }

    #[test]
    fn seek_preview_keeps_volume_clear_of_artwork() {
        let layout = progress_layout(320.0, true);

        assert!(progress_fixed_width(true) + layout.bar_width <= 320.0);
    }

    #[test]
    fn seek_fraction_clamps() {
        assert_eq!(seek_frac(50.0, 0.0, 100.0), 0.5);
        assert_eq!(seek_frac(-10.0, 0.0, 100.0), 0.0);
        assert_eq!(seek_frac(150.0, 0.0, 100.0), 1.0);
        assert_eq!(seek_frac(0.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn volume_taper_makes_the_quiet_range_easier_to_select() {
        assert_eq!(volume_gain_from_position(0.25), 0.0625);
        assert_eq!(volume_gain_from_position(0.5), 0.25);
        assert!((volume_position_from_gain(0.0625) - 0.25).abs() < f32::EPSILON);
    }

    /// A detent is one step whatever the system's lines-per-scroll: Windows
    /// reports three lines a notch, which used to move the volume 15%.
    #[test]
    fn a_wheel_notch_is_one_volume_step() {
        assert_eq!(notches_of(MouseWheelUnit::Line, 0.0), 0.0);
        assert_eq!(notches_of(MouseWheelUnit::Line, 3.0), 1.0);
        assert_eq!(notches_of(MouseWheelUnit::Line, -3.0), -1.0);
        assert_eq!(notches_of(MouseWheelUnit::Page, 1.0), 1.0);
        assert!((notches_of(MouseWheelUnit::Line, 3.0) * VOLUME_STEP - 0.05).abs() < 1e-6);
    }

    /// Trackpads and smooth wheels report points: ~50 px a notch, and a
    /// small flick still counts as one step.
    #[test]
    fn point_deltas_round_to_notches() {
        assert_eq!(notches_of(MouseWheelUnit::Point, 50.0), 1.0);
        assert_eq!(notches_of(MouseWheelUnit::Point, -50.0), -1.0);
        assert_eq!(notches_of(MouseWheelUnit::Point, 12.0), 1.0);
        assert_eq!(notches_of(MouseWheelUnit::Point, 150.0), 3.0);
    }

    #[test]
    fn waveform_resampling_preserves_a_short_peak() {
        let samples = [0.1, 0.9, 0.2, 0.3];

        assert_eq!(waveform_peak(Some(&samples), 0, 0, 2), 0.9);
    }
}
