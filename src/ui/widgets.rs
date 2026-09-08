use crate::api::models::Track;
use crate::util::{fmt_duration_ms, play_count};
use eframe::egui;

use super::App;
use super::theme::{Metrics, Type};

/// Artwork edge of a standard content card (`--artwork-20x-size`). Carousel
/// arrows centre on it.
pub const CARD_ART: f32 = 160.0;

/// Row artwork (`--artwork-5x-size`).
const ROW_ART: f32 = 40.0;

/// Deterministic placeholder-artwork color derived from an id.
pub fn art_color(id: u64) -> egui::Color32 {
    const PALETTE: [(u8, u8, u8); 12] = [
        (255, 85, 0),    // soundcloud orange
        (153, 51, 255),  // purple
        (0, 170, 255),   // sky
        (0, 200, 120),   // green
        (255, 60, 120),  // pink
        (255, 190, 0),   // amber
        (90, 120, 255),  // indigo
        (0, 200, 200),   // teal
        (220, 50, 50),   // red
        (120, 220, 60),  // lime
        (200, 120, 255), // lavender
        (255, 130, 60),  // light orange
    ];
    let (r, g, b) = PALETTE[(id as usize) % PALETTE.len()];
    egui::Color32::from_rgb(r, g, b)
}

/// Artwork with a real cover URL when available: paints the loaded image,
/// otherwise the deterministic placeholder (same layout, no jumping).
/// Covers flow through the bounded `ArtLoader` (RAM budget + disk cache).
pub fn artwork_img(
    ui: &mut egui::Ui,
    url: Option<&str>,
    seed: u64,
    title: &str,
    size: f32,
    rounding: f32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let mut painted = false;
        if let Some(url) = url.filter(|u| !u.is_empty()) {
            let img = egui::Image::new(url)
                .show_loading_spinner(false)
                .fit_to_exact_size(egui::vec2(size, size))
                .corner_radius(rounding)
                .sense(egui::Sense::hover());
            if let Ok(egui::load::TexturePoll::Ready { .. }) =
                img.load_for_size(ui.ctx(), rect.size())
            {
                img.paint_at(ui, rect);
                painted = true;
            }
        }
        if !painted {
            paint_placeholder(ui, rect, seed, title, rounding);
        }
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn paint_placeholder(ui: &egui::Ui, rect: egui::Rect, seed: u64, title: &str, rounding: f32) {
    let size = rect.width();
    let painter = ui.painter();
    painter.rect_filled(rect, rounding, art_color(seed));
    let letter = title
        .chars()
        .find(|c| !c.is_whitespace())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "♪".to_owned());
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        egui::FontId::proportional((size * 0.42).clamp(10.0, 64.0)),
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 230),
    );
}

/// One track row.
///
/// * single click on artwork/title starts playback (artist opens the artist),
/// * Ctrl/Cmd-click toggles selection, Shift-click selects a range (like
///   fastpotify's multi-select); right-click on a picked row acts on all
///   picked songs,
/// * like state is shared with the Library.
///
/// `picked` highlights the row; `multi` carries the picked tracks in table
/// order when this row is one of several picked (else `None`/empty).
pub fn track_row(
    app: &mut App,
    ui: &mut egui::Ui,
    track: &Track,
    index: usize,
    picked: bool,
    multi: Option<&[Track]>,
) -> RowAction {
    let st = app.player.state.lock();
    let (is_current, is_playing) = st
        .current
        .and_then(|i| st.queue.get(i).map(|t| (t.id == track.id, st.is_playing)))
        .unwrap_or((false, false));
    drop(st);
    let track_id = track.id;
    let liked_before = app.is_liked(track_id);
    let multi_count = multi.map(|m| m.len()).unwrap_or(0);
    let multi_picked = picked && multi_count > 1;

    let body = ui.scope(|ui| row_contents(app, ui, track, index, is_current, is_playing, picked));
    let action = body.inner;
    let like_pressed = app.is_liked(track_id) != liked_before;

    // Background click = play. Added after the contents but before nothing
    // else, so inner buttons stay interactive; their clicks fire too and
    // are reconciled below (like suppresses play, artist navigates).
    let bg_id = egui::Id::new(("track-click", track_id));
    let bg = ui
        .interact(body.response.rect, bg_id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let bg_clicked = bg.clicked();
    if multi_picked {
        let count = multi_count;
        bg.context_menu(|ui| {
            picked_menu(app, ui, multi.unwrap_or(&[]), count);
        });
    } else {
        bg.context_menu(|ui| {
            single_menu(app, ui, track, track_id);
        });
    }
    let _ = bg.on_hover_text(if picked {
        "Picked — right-click acts on all picked • Ctrl-click toggles"
    } else {
        "Click to play • Ctrl-click to pick • right-click for more"
    });

    match action {
        // Artist navigation needs no table context: apply here.
        RowAction::OpenArtist(id) => {
            app.navigate(super::route::Route::UserDetail(id));
            RowAction::None
        }
        // Play / selection need the table (order, queue): let the caller apply.
        RowAction::Play => RowAction::Play,
        RowAction::SelectToggle(_) | RowAction::SelectRange(_) => action,
        RowAction::None if bg_clicked && !like_pressed => {
            let mods = ui.input(|i| i.modifiers);
            if mods.command || mods.ctrl {
                RowAction::SelectToggle(track_id)
            } else if mods.shift {
                RowAction::SelectRange(track_id)
            } else {
                RowAction::Play
            }
        }
        RowAction::None => RowAction::None,
    }
}

/// Single-track context menu (plays, queues, likes one song).
fn single_menu(app: &mut App, ui: &mut egui::Ui, track: &Track, track_id: u64) {
    if ui.button("Play").clicked() {
        let t = track.clone();
        app.play_user_queue(vec![t], 0, false);
        ui.close();
    }
    if ui.button("Play next").clicked() {
        app.player.enqueue(vec![track.clone()], true);
        app.toast("Will play next");
        ui.close();
    }
    if ui.button("Add to queue").clicked() {
        app.player.enqueue(vec![track.clone()], false);
        app.toast("Added to queue");
        ui.close();
    }
    let liked = app.is_liked(track_id);
    if ui.button(if liked { "Unlike" } else { "Like" }).clicked() {
        let title = track.title.clone();
        let now = app.toggle_like(track_id);
        app.toast(if now {
            format!("Liked {title}")
        } else {
            format!("Removed {title} from likes")
        });
        ui.close();
    }
    if ui.button("Go to artist").clicked() {
        if let Some(u) = &track.user {
            app.navigate(super::route::Route::UserDetail(u.id));
        }
        ui.close();
    }
}

/// Multi-track menu for a picked selection (table order, like fastpotify's
/// `picked_menu`): one explicit liked state, queue/like/playlist for all.
fn picked_menu(app: &mut App, ui: &mut egui::Ui, songs: &[Track], count: usize) {
    ui.set_min_width(220.0);
    ui.label(Type::CAPTION.rich(&format!("{count} songs"), app.theme.text_dim));
    ui.separator();
    if ui.button("Play next").clicked() {
        app.player.enqueue(songs.to_vec(), true);
        app.toast(format!("Will play next: {count} songs"));
        ui.close();
    }
    if ui.button("Add to queue").clicked() {
        app.player.enqueue(songs.to_vec(), false);
        app.toast(format!("Added {count} songs to queue"));
        ui.close();
    }
    let all_liked = songs.iter().all(|t| app.is_liked(t.id));
    if ui
        .button(if all_liked { "Unlike all" } else { "Like all" })
        .clicked()
    {
        for t in songs {
            let liked = app.is_liked(t.id);
            if liked == all_liked {
                app.toggle_like(t.id);
            }
        }
        app.toast(if all_liked {
            format!("Unliked {count} songs")
        } else {
            format!("Liked {count} songs")
        });
        ui.close();
    }
    ui.menu_button("Add to playlist", |ui| {
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
        let track_ids: Vec<u64> = songs.iter().map(|track| track.id).collect();
        for (pid, name) in playlists {
            if ui.button(name.clone()).clicked() {
                app.add_tracks_to_playlist(pid, &track_ids);
                app.toast(format!("Adding {count} songs to {name}…"));
                ui.close();
            }
        }
    });
}

#[derive(Default)]
pub enum RowAction {
    #[default]
    None,
    Play,
    OpenArtist(u64),
    SelectToggle(u64),
    SelectRange(u64),
}

/// Two stacked lines fitted to the artwork: title first, artist at the bottom.
/// Returns an action when one of the two was clicked.
fn full_row_text(
    app: &App,
    ui: &mut egui::Ui,
    track: &Track,
    is_current: bool,
) -> Option<RowAction> {
    let mut action = None;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.set_min_width(120.0);
        let title = egui::RichText::new(crate::bidi::owned(&track.title))
            .font(Type::H4.font())
            .color(if is_current {
                app.theme.accent
            } else {
                app.theme.text
            });
        if ui
            .add(
                egui::Label::new(title)
                    .sense(egui::Sense::click())
                    .truncate(),
            )
            .on_hover_text("Play")
            .clicked()
        {
            action = Some(RowAction::Play);
        }
        ui.horizontal(|ui| {
            let artist = egui::RichText::new(crate::bidi::owned(track.artist()))
                .font(Type::BODY.font())
                .color(app.theme.text_dim);
            if ui
                .add(
                    egui::Label::new(artist)
                        .sense(egui::Sense::click())
                        .truncate(),
                )
                .on_hover_text("Open artist")
                .clicked()
                && let Some(u) = &track.user
            {
                action = Some(RowAction::OpenArtist(u.id));
            }
            if let Some(badge) = track.access_badge() {
                ui.label(
                    egui::RichText::new(badge)
                        .font(Type::MICRO.font())
                        .color(app.theme.accent),
                );
            }
        });
    });
    action
}

/// One line: title, then the artist after a dot — the compact list.
fn compact_row_text(
    app: &App,
    ui: &mut egui::Ui,
    track: &Track,
    is_current: bool,
) -> Option<RowAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.set_min_width(160.0);
        let title = egui::RichText::new(crate::bidi::owned(&track.title))
            .font(Type::H4.font())
            .color(if is_current {
                app.theme.accent
            } else {
                app.theme.text
            });
        if ui
            .add(
                egui::Label::new(title)
                    .sense(egui::Sense::click())
                    .truncate(),
            )
            .on_hover_text("Play")
            .clicked()
        {
            action = Some(RowAction::Play);
        }
        if let Some(badge) = track.access_badge() {
            ui.label(
                egui::RichText::new(badge)
                    .font(Type::MICRO.font())
                    .color(app.theme.accent),
            );
        }
        // A drawn dot: the interface font maps the bullet to a blank glyph.
        let (dot, _) = ui.allocate_exact_size(egui::vec2(4.0, 4.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(dot.center(), 1.5, app.theme.text_dim);
        let artist = egui::RichText::new(crate::bidi::owned(track.artist()))
            .font(Type::BODY.font())
            .color(app.theme.text_dim);
        if ui
            .add(
                egui::Label::new(artist)
                    .sense(egui::Sense::click())
                    .truncate(),
            )
            .on_hover_text("Open artist")
            .clicked()
            && let Some(u) = &track.user
        {
            action = Some(RowAction::OpenArtist(u.id));
        }
    });
    action
}

#[allow(clippy::too_many_arguments)]
fn row_contents(
    app: &mut App,
    ui: &mut egui::Ui,
    track: &Track,
    index: usize,
    is_current: bool,
    is_playing: bool,
    picked: bool,
) -> RowAction {
    let mut action = RowAction::None;

    egui::Frame::new()
        .fill(if picked {
            // Picked rows read as one block (accent wash); hover still lifts.
            let hovered = ui.rect_contains_pointer(ui.cursor());
            app.theme
                .accent
                .gamma_multiply(if hovered { 0.30 } else { 0.20 })
        } else if is_current {
            egui::Color32::from_rgba_unmultiplied(
                app.theme.accent.r(),
                app.theme.accent.g(),
                app.theme.accent.b(),
                30,
            )
        } else {
            egui::Color32::TRANSPARENT
        })
        .inner_margin(if app.settings.compact_rows {
            egui::Margin::symmetric(Metrics::SP_1 as i8, 3)
        } else {
            egui::Margin::symmetric(Metrics::SP_1 as i8, Metrics::SP_075 as i8)
        })
        .corner_radius(Metrics::RADIUS)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let compact = app.settings.compact_rows;
                ui.label(
                    egui::RichText::new(format!("{}", index + 1))
                        .font(egui::FontId::monospace(Type::CAPTION.size))
                        .color(app.theme.text_dim),
                );
                // Compact rows drop the artwork and put artist and title on
                // one line, the way SoundCloud's own compact lists do.
                if !compact
                    && artwork_img(
                        ui,
                        track.artwork_url(),
                        track.id,
                        &track.title,
                        ROW_ART,
                        Metrics::RADIUS as f32,
                    )
                    .clicked()
                {
                    action = RowAction::Play;
                }
                if compact {
                    if let Some(next) = compact_row_text(app, ui, track, is_current) {
                        action = next;
                    }
                } else if let Some(next) = full_row_text(app, ui, track, is_current) {
                    action = next;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(plays) = track.playback_count {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            super::icons::show_static(
                                ui,
                                super::icons::Icon::Music,
                                11.0,
                                app.theme.text_dim,
                            );
                            ui.label(Type::CAPTION.rich(&play_count(plays), app.theme.text_dim));
                        });
                    }
                    ui.label(Type::CAPTION.rich(
                        &fmt_duration_ms(track.effective_duration_ms()),
                        app.theme.text_dim,
                    ));
                    if like_button(app, ui, track.id, &track.title) {
                        // toast handled inside
                    }
                    let icon = if is_current && is_playing {
                        super::icons::Icon::Pause
                    } else {
                        super::icons::Icon::Play
                    };
                    if super::icons::show(ui, icon, 16.0, app.theme.text)
                        .on_hover_text("Play")
                        .clicked()
                    {
                        action = RowAction::Play;
                    }
                });
            });
        });

    action
}

/// Shared like toggle with toast. Returns the new liked state when clicked.
pub fn like_button(app: &mut App, ui: &mut egui::Ui, track_id: u64, title: &str) -> bool {
    let liked = app.is_liked(track_id);
    let tint = if liked {
        app.theme.accent
    } else {
        app.theme.text_dim
    };
    let resp = super::icons::show(ui, super::icons::Icon::Heart, 16.0, tint)
        .on_hover_text(if liked { "Unlike" } else { "Like" });
    if resp.clicked() {
        let now = app.toggle_like(track_id);
        app.toast(if now {
            format!("Liked {title}")
        } else {
            format!("Removed {title} from likes")
        });
        now
    } else {
        liked
    }
}

/// Shared follow toggle with toast. Returns the new state when clicked.
///
/// SoundCloud pills: outlined "Follow", filled "Following" with a tick. The
/// tick and the plus are drawn rather than typed, so the button never depends
/// on a font carrying them — and a user-supplied interface face
/// ([`crate::config::Settings::interface_font`]) may well not.
pub fn follow_button(app: &mut App, ui: &mut egui::Ui, user_id: u64, name: &str) -> bool {
    let following = app.is_following(user_id);
    let label = if following { "Following" } else { "Follow" };
    let font = Type::H4.font();
    let color = if following {
        app.theme.text
    } else {
        app.theme.accent
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    let tick_w = if following { 18.0 } else { 14.0 };
    let size = galley.size() + egui::vec2(24.0 + tick_w, 14.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered();
        let radius = rect.height() / 2.0;
        let stroke = if following {
            app.theme.separator
        } else {
            app.theme.accent
        };
        if hovered {
            ui.painter()
                .rect_filled(rect, radius, app.theme.surface_hover);
        }
        ui.painter().rect_stroke(
            rect,
            radius,
            egui::Stroke::new(1.0, stroke),
            egui::StrokeKind::Inside,
        );
        let glyph_x = rect.left() + 12.0 + tick_w / 2.0;
        if following {
            tick(
                ui.painter(),
                egui::pos2(glyph_x, rect.center().y),
                5.0,
                color,
            );
        } else {
            plus(
                ui.painter(),
                egui::pos2(glyph_x, rect.center().y),
                5.0,
                color,
            );
        }
        let pos = egui::pos2(
            rect.left() + 12.0 + tick_w + 4.0,
            rect.center().y - galley.size().y / 2.0,
        );
        ui.painter().galley(pos, galley, color);
    }
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        let now = app.toggle_follow(user_id);
        app.toast(if now {
            format!("Following {name}")
        } else {
            format!("Unfollowed {name}")
        });
        now
    } else {
        following
    }
}

/// A check mark as two strokes.
fn tick(painter: &egui::Painter, center: egui::Pos2, half: f32, color: egui::Color32) {
    let stroke = egui::Stroke::new(2.0, color);
    let left = egui::pos2(center.x - half, center.y);
    let low = egui::pos2(center.x - half * 0.25, center.y + half * 0.7);
    let right = egui::pos2(center.x + half, center.y - half * 0.7);
    painter.line_segment([left, low], stroke);
    painter.line_segment([low, right], stroke);
}

/// A plus as two strokes.
fn plus(painter: &egui::Painter, center: egui::Pos2, half: f32, color: egui::Color32) {
    let stroke = egui::Stroke::new(2.0, color);
    painter.line_segment(
        [
            egui::pos2(center.x - half, center.y),
            egui::pos2(center.x + half, center.y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x, center.y - half),
            egui::pos2(center.x, center.y + half),
        ],
        stroke,
    );
}

// ===== Waveform =====

/// Deterministic placeholder waveform bars in 0..=1 (until real peaks load).
pub fn wave_bars(seed: u64, n: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n);
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15).max(1);
    for i in 0..n {
        z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut t = z;
        t = (t ^ (t >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        t = (t ^ (t >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        t ^= t >> 31;
        let v = (t as f64 / u64::MAX as f64) as f32;
        let env = 0.55 + 0.45 * ((i as f32 * 0.11 + seed as f32).sin() * 0.5 + 0.5);
        out.push((env * (0.25 + 0.75 * v)).clamp(0.05, 1.0));
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn waveform(
    app: &App,
    ui: &mut egui::Ui,
    seed: u64,
    pos_ms: u64,
    dur_ms: u64,
    height: f32,
    bars: usize,
    samples: Option<&[f32]>,
) -> Option<f32> {
    let width = ui.available_width().max(80.0);
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let visible_bars = bars.max((rect.width() / 4.0).round() as usize);
        let heights = samples
            .filter(|samples| !samples.is_empty())
            .map(|samples| resample_peaks(samples, visible_bars))
            .unwrap_or_else(|| wave_bars(seed, visible_bars));
        let n = heights.len().max(1) as f32;
        let gap = 1.25;
        let bar_w = (rect.width() / n - gap).max(1.0);
        let progress = if dur_ms > 0 {
            (pos_ms as f32 / dur_ms as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let baseline = rect.top() + rect.height() * 0.68;
        p.hline(
            rect.x_range(),
            baseline,
            egui::Stroke::new(1.0, app.theme.separator.gamma_multiply(0.65)),
        );
        for (i, h) in heights.iter().enumerate() {
            let x = rect.left() + i as f32 * (bar_w + gap);
            let upper = 2.0 + h * (baseline - rect.top() - 4.0).max(1.0);
            let lower = 1.0 + h * (rect.bottom() - baseline - 3.0).max(1.0) * 0.72;
            let upper_bar = egui::Rect::from_min_max(
                egui::pos2(x, baseline - upper),
                egui::pos2(x + bar_w, baseline - 1.0),
            );
            let lower_bar = egui::Rect::from_min_max(
                egui::pos2(x, baseline + 2.0),
                egui::pos2(x + bar_w, baseline + lower),
            );
            let played = (i as f32 + 1.0) / n <= progress;
            let colour = if played {
                app.theme.accent
            } else {
                app.theme.text_dim.gamma_multiply(0.90)
            };
            p.rect_filled(upper_bar, 0.0, colour);
            p.rect_filled(lower_bar, 0.0, colour.gamma_multiply(0.72));
        }
        if resp.hovered()
            && let Some(pointer) = resp.hover_pos()
        {
            p.vline(
                pointer.x,
                rect.y_range(),
                egui::Stroke::new(1.0, app.theme.text.gamma_multiply(0.55)),
            );
        }
        let duration = fmt_duration_ms(dur_ms);
        let galley = p.layout_no_wrap(duration, Type::MICRO.font(), app.theme.text);
        let label = egui::Rect::from_min_size(
            rect.right_bottom() - galley.size() - egui::vec2(5.0, 3.0),
            galley.size() + egui::vec2(4.0, 2.0),
        );
        p.rect_filled(label, 1.0, app.theme.bg.gamma_multiply(0.88));
        p.galley(label.min + egui::vec2(2.0, 1.0), galley, app.theme.text);
    }
    if (resp.clicked() || resp.dragged())
        && let Some(mx) = resp.interact_pointer_pos()
    {
        let frac = ((mx.x - resp.rect.left()) / resp.rect.width()).clamp(0.0, 1.0);
        return Some(frac);
    }
    None
}

/// Reduce/expand SoundCloud's sample array to the number of visible bars.
/// A bar takes the maximum from its source interval, preserving short peaks.
fn resample_peaks(samples: &[f32], bars: usize) -> Vec<f32> {
    let bars = bars.max(1);
    (0..bars)
        .map(|index| {
            let start = index * samples.len() / bars;
            let mut end = (index + 1) * samples.len() / bars;
            if end <= start {
                end = (start + 1).min(samples.len());
            }
            samples[start.min(samples.len() - 1)..end.max(start + 1).min(samples.len())]
                .iter()
                .copied()
                .fold(0.01_f32, f32::max)
                .clamp(0.01, 1.0)
        })
        .collect()
}

/// Result of interacting with a content card.
#[derive(Default)]
pub struct CardClick {
    pub play: bool,
    pub open: bool,
}

/// What a content card shows.
///
/// A builder rather than eight positional arguments: the four strings were
/// interchangeable at the call site, which is how a subtitle ends up in the
/// title slot.
pub struct Card<'a> {
    /// Cover URL, when the API gave one. The placeholder stands in otherwise.
    pub art: Option<&'a str>,
    /// Seeds the placeholder's colour and letter — usually the entity's id.
    pub seed: u64,
    pub title: &'a str,
    /// The line above the title, as soundcloud.com stacks them.
    pub subtitle: &'a str,
    /// Artist profile opened by clicking the subtitle.
    pub artist_id: Option<u64>,
    /// Artwork edge. Defaults to [`CARD_ART`].
    pub size: f32,
    /// Whether clicking the title opens the thing (playlists, albums) rather
    /// than playing it (tracks).
    pub title_opens: bool,
}

impl<'a> Card<'a> {
    /// A track card: cover, artist over title, title plays.
    pub fn track(track: &'a Track) -> Self {
        Self {
            art: track.artwork_url(),
            seed: track.id,
            title: &track.title,
            subtitle: track.artist(),
            artist_id: track.user.as_ref().map(|user| user.id),
            size: CARD_ART,
            title_opens: false,
        }
    }

    /// A card for anything that opens rather than plays.
    pub fn opens(seed: u64, title: &'a str, subtitle: &'a str) -> Self {
        Self {
            art: None,
            seed,
            title,
            subtitle,
            artist_id: None,
            size: CARD_ART,
            title_opens: true,
        }
    }

    /// A card that plays when its title is clicked.
    pub fn plays(seed: u64, title: &'a str, subtitle: &'a str) -> Self {
        Self {
            title_opens: false,
            ..Self::opens(seed, title, subtitle)
        }
    }

    pub fn art(mut self, art: Option<&'a str>) -> Self {
        self.art = art;
        self
    }

    pub fn artist(mut self, artist_id: Option<u64>) -> Self {
        self.artist_id = artist_id;
        self
    }
}

/// Draw a content card: artwork with a hover play overlay, then the artist
/// line and the title, as soundcloud.com stacks them (`.sc-text-h4` title over
/// a `.sc-text-secondary` line). Artwork click always plays.
pub fn card(app: &mut App, ui: &mut egui::Ui, card: Card<'_>) -> CardClick {
    let Card {
        art: art_url,
        seed,
        title,
        subtitle,
        artist_id,
        size,
        title_opens,
    } = card;
    let mut out = CardClick::default();
    ui.vertical(|ui| {
        // Card grids use a large vertical gap between rows. Do not inherit it
        // between the title and artist inside a single card.
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.set_min_width(size);
        ui.set_max_width(size);
        let art = artwork_img(ui, art_url, seed, title, size, Metrics::RADIUS as f32);
        if art.hovered() {
            // Play overlay disc with a triangle (avoids font glyphs).
            let rect = art.rect;
            let center = rect.center();
            let r = (size * 0.2).clamp(14.0, 30.0);
            let painter = ui.painter();
            painter.circle_filled(center, r, app.theme.accent);
            let s = r * 0.55;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + egui::vec2(-s * 0.6, -s),
                    center + egui::vec2(-s * 0.6, s),
                    center + egui::vec2(s * 0.8, 0.0),
                ],
                egui::Color32::WHITE,
                egui::Stroke::NONE,
            ));
        }
        if art.clicked() {
            if title_opens {
                let play_radius = (size * 0.2).clamp(14.0, 30.0);
                if art
                    .interact_pointer_pos()
                    .is_some_and(|pos| pos.distance(art.rect.center()) <= play_radius)
                {
                    out.play = true;
                } else {
                    out.open = true;
                }
            } else {
                out.play = true;
            }
        }
        let title_resp = ui.add(
            egui::Label::new(
                egui::RichText::new(crate::bidi::owned(title))
                    .font(Type::H4.font())
                    .color(app.theme.text),
            )
            .sense(egui::Sense::click())
            .truncate(),
        );
        if title_resp.clicked() {
            if title_opens {
                out.open = true;
            } else {
                out.play = true;
            }
        }
        let subtitle = ui.add(
            egui::Label::new(
                egui::RichText::new(crate::bidi::owned(subtitle))
                    .font(Type::BODY.font())
                    .color(app.theme.text_dim),
            )
            .sense(if artist_id.is_some() {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            })
            .truncate(),
        );
        if let Some(user_id) = artist_id
            && subtitle
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
        {
            app.navigate(crate::ui::route::Route::UserDetail(user_id));
        }
    });
    out
}

/// Section header like "Made for you".
pub fn section_header(app: &App, ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(title)
            .font(Type::H2.font())
            .color(app.theme.text),
    );
    ui.add_space(6.0);
}

/// Horizontal carousel with ‹ › arrow buttons (soundcloud.com style).
/// Scrollbar hidden; arrows page by 80% of the visible width. `pending` is
/// a one-frame scroll target (applied once so manual scrolling keeps working).
/// Returns the inner value plus the visible/content widths for arrows.
pub fn carousel_plain<R>(
    ui: &mut egui::Ui,
    salt: &str,
    pending: Option<f32>,
    add_cards: impl FnOnce(&mut egui::Ui) -> R,
) -> CarouselOut<R> {
    let mut area = egui::ScrollArea::horizontal()
        .id_salt(salt)
        .auto_shrink([false, true])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    if let Some(x) = pending {
        area = area.horizontal_scroll_offset(x);
    }
    let out = area.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = Metrics::SP_4;
            add_cards(ui)
        })
    });
    let max = (out.content_size.x - out.inner_rect.width()).max(0.0);
    CarouselOut {
        inner: out.inner.inner,
        view: out.inner_rect,
        offset_x: out.state.offset.x,
        max_x: max,
    }
}

pub struct CarouselOut<R> {
    pub inner: R,
    pub view: egui::Rect,
    pub offset_x: f32,
    pub max_x: f32,
}

/// ‹ › arrow circles over a carousel edge. Returns true when paged.
/// Call after the content closure finished (separate borrow from it).
///
/// `art_size` centres the arrows on the artwork rather than the whole row:
/// a card is artwork + two text lines, so the row's own centre sits well
/// below the cover — which is what made the arrows look too high.
pub fn carousel_arrows(
    ui: &mut egui::Ui,
    id_salt: &str,
    view: egui::Rect,
    offset_x: f32,
    max_x: f32,
    art_size: Option<f32>,
    on_page: &mut dyn FnMut(f32),
) {
    if max_x <= 1.0 {
        return;
    }
    let cy = match art_size {
        Some(size) => view.top() + size / 2.0,
        None => view.center().y,
    };
    let step = view.width() * 0.8;
    if offset_x > 1.0
        && arrow_button(
            ui,
            egui::Id::new(id_salt).with("left"),
            view.left() + 20.0,
            cy,
            true,
        )
    {
        on_page((offset_x - step).max(0.0));
    }
    if offset_x < max_x - 1.0
        && arrow_button(
            ui,
            egui::Id::new(id_salt).with("right"),
            view.right() - 20.0,
            cy,
            false,
        )
    {
        on_page((offset_x + step).min(max_x));
    }
}

fn arrow_button(ui: &mut egui::Ui, id: egui::Id, x: f32, y: f32, left: bool) -> bool {
    let rect = egui::Rect::from_center_size(egui::pos2(x, y), egui::vec2(32.0, 32.0));
    let resp = ui.interact(rect, id, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let bg = if resp.hovered() {
            egui::Color32::from_rgba_unmultiplied(10, 10, 10, 220)
        } else {
            egui::Color32::from_rgba_unmultiplied(10, 10, 10, 150)
        };
        p.circle_filled(rect.center(), 16.0, bg);
        p.circle_stroke(
            rect.center(),
            16.0,
            egui::Stroke::new(1.0, egui::Color32::WHITE),
        );
        // Drawn, not typed: an interface face need not carry the guillemets,
        // and one that claims them without an outline leaves a bare circle.
        chevron(p, rect.center(), 5.0, left, egui::Color32::WHITE);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// A chevron (‹ or ›) as two strokes, centred on `center`.
pub fn chevron(
    painter: &egui::Painter,
    center: egui::Pos2,
    half: f32,
    left: bool,
    color: egui::Color32,
) {
    let dx = if left { half * 0.62 } else { -half * 0.62 };
    let tip = egui::pos2(center.x - dx, center.y);
    let top = egui::pos2(center.x + dx, center.y - half);
    let bottom = egui::pos2(center.x + dx, center.y + half);
    let stroke = egui::Stroke::new(2.0, color);
    painter.line_segment([top, tip], stroke);
    painter.line_segment([tip, bottom], stroke);
}

/// Full carousel in one call; the one-frame scroll target is threaded
/// through explicitly so card closures stay borrow-free. Prefer
/// [`App::carousel`](super::App::carousel), which keeps the target map.
#[allow(dead_code)]
pub fn carousel_with_pending<R>(
    ui: &mut egui::Ui,
    salt: &str,
    pending: Option<f32>,
    add_cards: impl FnOnce(&mut egui::Ui) -> R,
    mut on_page: impl FnMut(f32),
) -> R {
    let out = carousel_plain(ui, salt, pending, add_cards);
    carousel_arrows(
        ui,
        salt,
        out.view,
        out.offset_x,
        out.max_x,
        None,
        &mut on_page,
    );
    out.inner
}

/// Lays out only the rows intersecting the visible area of the enclosing
/// scroll view (fastpotify's `virtual_rows`). Every row must occupy exactly
/// `row_height`; the gaps above/below keep the scrollbar honest.
///
/// NOTE: track/queue rows are still variable-height, so lists don't use this
/// yet — it lands with fixed-height rows. Kept (tested) as the blessed path.
#[allow(dead_code)]
pub fn virtual_rows(
    ui: &mut egui::Ui,
    count: usize,
    row_height: f32,
    mut row: impl FnMut(&mut egui::Ui, usize),
) {
    if count == 0 || row_height <= 0.0 {
        return;
    }
    let previous_spacing = ui.spacing().item_spacing;
    ui.spacing_mut().item_spacing.y = 0.0;
    let clip = ui.clip_rect();
    let start_y = ui.cursor().top();
    let width = ui.available_width();
    let (first, last) = visible_range(count, row_height, start_y, clip.top(), clip.bottom());
    if first > 0 {
        ui.allocate_space(egui::vec2(width, first as f32 * row_height));
    }
    for index in first..last {
        row(ui, index);
    }
    if last < count {
        ui.allocate_space(egui::vec2(width, (count - last) as f32 * row_height));
    }
    ui.spacing_mut().item_spacing = previous_spacing;
}

/// Pure index math behind [`virtual_rows`]: which rows `[first, last)` the
/// viewport `[clip_top, clip_bottom)` covers. One extra row below for
/// partial visibility.
#[allow(dead_code)]
pub fn visible_range(
    count: usize,
    row_height: f32,
    start_y: f32,
    clip_top: f32,
    clip_bottom: f32,
) -> (usize, usize) {
    if count == 0 || row_height <= 0.0 {
        return (0, 0);
    }
    let first = (((clip_top - start_y) / row_height).floor().max(0.0) as usize).min(count);
    let last = (((clip_bottom - start_y) / row_height).ceil().max(0.0) as usize + 1).min(count);
    (first, last.max(first))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn art_color_is_deterministic() {
        assert_eq!(art_color(7), art_color(7));
        assert_ne!(art_color(1), art_color(2));
    }

    #[test]
    fn visible_range_covers_viewport_plus_one() {
        // 100 rows of 56px starting at y=0, viewport shows y=100..=300.
        let (first, last) = visible_range(100, 56.0, 0.0, 100.0, 300.0);
        assert_eq!(first, 1);
        assert_eq!(last, 7);
        // Scrolled past the end clamps to count.
        assert_eq!(visible_range(10, 56.0, 0.0, 10_000.0, 11_000.0), (10, 10));
        // Empty list draws nothing.
        assert_eq!(visible_range(0, 56.0, 0.0, 0.0, 500.0), (0, 0));
    }

    #[test]
    fn wave_bars_deterministic_and_bounded() {
        let a = wave_bars(1001, 64);
        let b = wave_bars(1001, 64);
        assert_eq!(a.len(), 64);
        assert_eq!(a, b);
        assert!(a.iter().all(|&v| (0.0..=1.0).contains(&v)));
        assert!(a.iter().any(|&v| v > 0.5));
    }

    #[test]
    fn waveform_resampling_keeps_short_peaks() {
        let samples = [0.1, 0.9, 0.2, 0.3, 1.0, 0.4];

        assert_eq!(resample_peaks(&samples, 3), vec![0.9, 0.3, 1.0]);
    }
}
