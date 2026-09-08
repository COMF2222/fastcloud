//! The equaliser window: ten bands, a preamp, ON, AUTO, PRESETS and the graph.
//!
//! Winamp's own window, drawn from [`crate::skin::layout::eq`] and the sprites
//! in `eqmain.bmp`. The sliders drive [`crate::audio::dsp::Eq10`] straight
//! through the app (the curve has to be persisted, so it cannot go to the
//! player behind `App`'s back the way transport does).

use super::{Command, Frame, MiniPlayer, eq_curve};
use crate::audio::dsp::EQ_RANGE_DB;
use crate::skin::layout::Area;
use crate::skin::layout::eq as layout;
use crate::skin::layout::stack::Window;
use crate::skin::{Sheet, sprites};
use eframe::egui;

impl MiniPlayer {
    /// Draw the equaliser, rolled up or not.
    pub(super) fn eq_ui(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        shade: bool,
    ) {
        if shade {
            self.eq_shade_ui(ctx, ui, frame);
            return;
        }
        // A skin with no `eqmain.bmp` has no equaliser art at all. Rather than
        // paint holes, the window is not drawn — and `App` will not open it,
        // so this is belt and braces.
        if !self.skin.has(Sheet::EqMain) {
            return;
        }
        self.blit(ctx, ui, frame, sprites::EQ_MAIN, (0, 0));
        self.blit_at(ctx, ui, frame, sprites::EQ_TITLE_BAR, layout::TITLE_BAR);
        // The graph's well and the three dB marks are part of qmain.bmp,
        // kept away from the window's own background so a skin can style them.
        self.blit_at(ctx, ui, frame, sprites::EQ_GRAPH, layout::GRAPH);
        self.eq_title_bar(ctx, ui, frame);
        self.eq_buttons(ctx, ui, frame);
        self.eq_bands(ctx, ui, frame);
        self.eq_graph(ui, frame);
    }

    /// The title bar: the drag strip and the two buttons.
    fn eq_title_bar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, frame: &mut Frame) {
        let drag = ui.interact(
            frame.rect(layout::TITLE_BAR),
            ui.id().with("eq-titlebar"),
            egui::Sense::click_and_drag(),
        );
        if drag.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            frame.commands.push(Command::ToggleShade(Window::Equalizer));
        }
        if self.button(
            ctx,
            ui,
            frame,
            layout::SHADE_BUTTON,
            sprites::EQ_SHADE,
            sprites::EQ_SHADE_DOWN,
            "Roll up",
        ) {
            frame.commands.push(Command::ToggleShade(Window::Equalizer));
        }
        if self.button(
            ctx,
            ui,
            frame,
            layout::CLOSE_BUTTON,
            sprites::EQ_CLOSE,
            sprites::EQ_CLOSE_DOWN,
            "Close the equaliser",
        ) {
            frame
                .commands
                .push(Command::ToggleWindow(Window::Equalizer));
        }
    }

    /// ON, AUTO and PRESETS.
    fn eq_buttons(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, frame: &mut Frame) {
        let mut eq = frame.eq;
        let (on, on_down) = if eq.enabled {
            (sprites::EQ_ENABLED, sprites::EQ_ENABLED_DOWN)
        } else {
            (sprites::EQ_DISABLED, sprites::EQ_DISABLED_DOWN)
        };
        if self.button(
            ctx,
            ui,
            frame,
            layout::ON_BUTTON,
            on,
            on_down,
            "Equaliser on or off",
        ) {
            eq.enabled = !eq.enabled;
            frame.commands.push(Command::Eq(eq));
        }

        let (auto, auto_down) = if eq.auto {
            (sprites::EQ_AUTO_ON, sprites::EQ_AUTO_ON_DOWN)
        } else {
            (sprites::EQ_AUTO, sprites::EQ_AUTO_DOWN)
        };
        if self.button(
            ctx,
            ui,
            frame,
            layout::AUTO_BUTTON,
            auto,
            auto_down,
            "Load each track's own preset",
        ) {
            eq.auto = !eq.auto;
            frame.commands.push(Command::Eq(eq));
        }

        // PRESETS opens a menu, as Winamp's did. The entries are the ones this
        // app can honour: a per-track preset, and the flat curve.
        let presets = ui
            .interact(
                frame.rect(layout::PRESETS_BUTTON),
                ui.id().with("eq-presets"),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let sprite = if presets.is_pointer_button_down_on() {
            sprites::EQ_PRESETS_DOWN
        } else {
            sprites::EQ_PRESETS
        };
        self.blit_at(ctx, ui, frame, sprite, layout::PRESETS_BUTTON);
        egui::Popup::menu(&presets).show(|ui| {
            ui.set_min_width(150.0);
            if ui.button("Save for this track").clicked() {
                frame.commands.push(Command::SaveEqPreset);
                ui.close();
            }
            if ui
                .add_enabled(frame.has_preset, egui::Button::new("Forget this track's"))
                .clicked()
            {
                frame.commands.push(Command::ClearEqPreset);
                ui.close();
            }
            ui.separator();
            if ui.button("Flat").clicked() {
                let mut flat = frame.eq;
                flat.preamp_db = 0.0;
                flat.gains_db = [0.0; 10];
                frame.commands.push(Command::Eq(flat));
                ui.close();
            }
        });
    }

    /// The preamp and the ten bands.
    fn eq_bands(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, frame: &mut Frame) {
        let mut eq = frame.eq;
        let mut changed = false;
        if let Some(db) = self.eq_band(ctx, ui, frame, layout::PREAMP, eq.preamp_db, "preamp") {
            eq.preamp_db = db;
            changed = true;
        }
        for index in 0..layout::BANDS {
            let area = layout::band(index);
            let salt = format!("band-{index}");
            if let Some(db) = self.eq_band(ctx, ui, frame, area, eq.gains_db[index], &salt) {
                eq.gains_db[index] = db;
                changed = true;
            }
        }
        if changed {
            frame.commands.push(Command::Eq(eq));
        }
    }

    /// One vertical slider. Returns its new gain in dB when it moved.
    ///
    /// Winamp's track art brightens as the band is raised — twenty-eight
    /// background states in a grid — so the background is chosen from the value
    /// rather than being one fixed sprite.
    fn eq_band(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &Frame,
        area: Area,
        gain_db: f32,
        salt: &str,
    ) -> Option<f32> {
        // 0 at the bottom, 27 at the top.
        let fraction = fraction_of(gain_db);
        let step = ((fraction * (sprites::EQ_BAND_STEPS - 1) as f32).round() as u32)
            .min(sprites::EQ_BAND_STEPS - 1);
        self.blit_at(ctx, ui, frame, sprites::eq_band(step), area);

        let rect = frame.rect(area);
        let response = ui
            .interact(
                rect,
                ui.id().with(("eq-band", salt)),
                egui::Sense::click_and_drag(),
            )
            .on_hover_text(format!("{gain_db:+.1} dB"));

        // The thumb hangs from the top of the track downward, so a raised band
        // has its thumb high up.
        let travel = layout::THUMB_TRAVEL;
        let from_top = ((1.0 - fraction) * travel as f32).round() as u32;
        let thumb = if response.is_pointer_button_down_on() {
            sprites::EQ_THUMB_DOWN
        } else {
            sprites::EQ_THUMB
        };
        self.blit(ctx, ui, frame, thumb, (area.x + 1, area.y + from_top));

        if (response.dragged() || response.clicked())
            && let Some(pos) = response.interact_pointer_pos()
        {
            let half = layout::THUMB_HEIGHT as f32 / 2.0 * frame.scale;
            let down =
                ((pos.y - rect.top() - half) / (travel as f32 * frame.scale)).clamp(0.0, 1.0);
            let db = gain_of(1.0 - down);
            // Winamp snaps the middle to flat, because hitting exactly 0 dB on
            // a 51-pixel travel otherwise takes real aim.
            let snapped = if db.abs() < EQ_RANGE_DB * 0.08 {
                0.0
            } else {
                db
            };
            return Some(snapped);
        }
        None
    }

    /// The little screen that draws the curve, as one mesh.
    fn eq_graph(&self, ui: &egui::Ui, frame: &Frame) {
        let area = layout::GRAPH;
        let mut mesh = egui::Mesh::default();
        let quad =
            |mesh: &mut egui::Mesh, x: u32, y: u32, w: u32, h: u32, colour: egui::Color32| {
                mesh.add_colored_rect(
                    egui::Rect::from_min_size(
                        frame.at(area.x + x, area.y + y),
                        egui::vec2(w as f32 * frame.scale, h as f32 * frame.scale),
                    ),
                    colour,
                );
            };
        // The preamp's line first, so the curve draws over it. Winamp's own is a
        // one-pixel row of `eqmain.bmp` tiled across the graph.
        let preamp = eq_curve::preamp_row(frame.eq.preamp_db, EQ_RANGE_DB) as u32;
        quad(&mut mesh, 0, preamp, area.width, 1, self.preamp_ink);
        // Winamp colours the line by *row*, from `eqmain.bmp`'s one-pixel colour
        // column: the curve is greener at the top and redder at the bottom.
        for column in eq_curve::curve(&frame.eq.gains_db, EQ_RANGE_DB) {
            // One colour per row of the run, so a steep stretch is graded rather
            // than one flat block.
            for row in column.top..column.top + column.height {
                let colour = self.curve_ink(row);
                quad(&mut mesh, column.x as u32, row as u32, 1, 1, colour);
            }
        }
        if !mesh.is_empty() {
            ui.painter_at(frame.rect(area)).add(egui::Shape::mesh(mesh));
        }
    }

    /// The curve's colour on one row of the graph.
    ///
    /// A skin ships nineteen of these in a one-pixel column; without it the
    /// visualiser's ramp stands in, which is the same idea in whatever palette
    /// the skin does have.
    fn curve_ink(&self, row: usize) -> egui::Color32 {
        if let Some(colours) = &self.eq_curve_colors
            && !colours.is_empty()
        {
            return colours[row.min(colours.len() - 1)];
        }
        let shade = row * 15 / eq_curve::HEIGHT.max(1);
        self.skin.vis_colors[(17 - shade.min(15)).max(2)]
    }

    /// Rolled up: the volume and balance sliders on the title bar.
    fn eq_shade_ui(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, frame: &mut Frame) {
        let bar = if self.skin.has(Sheet::EqEx) {
            sprites::EQ_SHADE_BAR
        } else if self.skin.has(Sheet::EqMain) {
            sprites::EQ_TITLE_BAR
        } else {
            return;
        };
        self.blit(ctx, ui, frame, bar, (0, 0));

        // `eq_ex.bmp` carries its own pair of buttons, so the rolled-up window
        // gets those rather than the tall one's.
        if self.skin.has(Sheet::EqEx) {
            let unshade = self
                .hit(ui, frame, layout::SHADE_BUTTON, "eq-unshade")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Unroll");
            if unshade.is_pointer_button_down_on() {
                self.blit_at(
                    ctx,
                    ui,
                    frame,
                    sprites::EQ_SHADE_UNSHADE,
                    layout::SHADE_BUTTON,
                );
            }
            if unshade.clicked() {
                frame.commands.push(Command::ToggleShade(Window::Equalizer));
            }
            if self.button(
                ctx,
                ui,
                frame,
                layout::CLOSE_BUTTON,
                sprites::EQ_SHADE_CLOSE,
                sprites::EQ_SHADE_CLOSE_DOWN,
                "Close the equaliser",
            ) {
                frame
                    .commands
                    .push(Command::ToggleWindow(Window::Equalizer));
            }
        } else {
            self.eq_title_bar(ctx, ui, frame);
        }

        // The two sliders are drawn from three-pixel pieces, so they can be any
        // width. Only a skin with `eq_ex.bmp` has them.
        if !self.skin.has(Sheet::EqEx) {
            return;
        }
        let state = self.player.state.lock();
        let (volume, balance) = (state.volume, state.balance);
        drop(state);

        // Volume runs the whole way; balance is centre-out, so its position is
        // the balance mapped into 0..=1 and its snap is back at the middle.
        let volume_slider = ShadeSlider {
            area: layout::SHADE_VOLUME,
            travel: layout::SHADE_VOLUME_TRAVEL,
            pieces: [
                sprites::EQ_SHADE_VOL_LEFT,
                sprites::EQ_SHADE_VOL_MID,
                sprites::EQ_SHADE_VOL_RIGHT,
            ],
            salt: "eq-shade-volume",
        };
        if let Some(moved) = self.eq_shade_slider(
            ctx,
            ui,
            frame,
            &volume_slider,
            super::mini_volume_position(volume),
        ) {
            let gain = super::mini_volume_gain(moved);
            let percent = (gain * 100.0).round().clamp(0.0, 100.0) as u8;
            self.player.set_volume(gain);
            frame.commands.push(Command::Volume(percent));
        }
        let balance_slider = ShadeSlider {
            area: layout::SHADE_BALANCE,
            travel: layout::SHADE_BALANCE_TRAVEL,
            pieces: [
                sprites::EQ_SHADE_BAL_LEFT,
                sprites::EQ_SHADE_BAL_MID,
                sprites::EQ_SHADE_BAL_RIGHT,
            ],
            salt: "eq-shade-balance",
        };
        if let Some(moved) =
            self.eq_shade_slider(ctx, ui, frame, &balance_slider, (balance + 1.0) / 2.0)
        {
            let raw = moved * 2.0 - 1.0;
            let snapped = if raw.abs() < 0.1 { 0.0 } else { raw };
            let percent = (snapped * 100.0).round().clamp(-100.0, 100.0) as i8;
            self.player.set_balance(f32::from(percent) / 100.0);
            frame.commands.push(Command::Balance(percent));
        }
    }

    /// One of the rolled-up equaliser's sliders: a groove drawn from three
    /// pieces, with the thumb somewhere along it.
    fn eq_shade_slider(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &Frame,
        slider: &ShadeSlider,
        position: f32,
    ) -> Option<f32> {
        let ShadeSlider {
            area,
            travel,
            pieces,
            salt,
        } = *slider;
        let steps = (position.clamp(0.0, 1.0) * travel as f32).round() as u32;
        // Left cap, middle repeated, right cap — the thumb is the piece that
        // matches where it sits, as Winamp's does.
        let piece = if steps == 0 {
            pieces[0]
        } else if steps >= travel {
            pieces[2]
        } else {
            pieces[1]
        };
        self.blit(ctx, ui, frame, piece, (area.x + steps, area.y));

        let rect = frame.rect(area);
        let response = ui.interact(
            rect,
            ui.id().with(("eq-shade-slider", salt)),
            egui::Sense::click_and_drag(),
        );
        if (response.dragged() || response.clicked())
            && let Some(pos) = response.interact_pointer_pos()
        {
            let half = layout::SHADE_THUMB_W as f32 / 2.0 * frame.scale;
            return Some(
                ((pos.x - rect.left() - half) / (travel as f32 * frame.scale)).clamp(0.0, 1.0),
            );
        }
        None
    }
}

/// One rolled-up slider's geometry and art. Grouped because the two differ only
/// in these four things, and passing them one by one was nine arguments.
struct ShadeSlider {
    area: Area,
    /// How far the thumb can move: the groove less the thumb's own width.
    travel: u32,
    /// Left cap, repeating middle, right cap.
    pieces: [crate::skin::Sprite; 3],
    salt: &'static str,
}

/// The gain a slider position means, and the position a gain means. Free
/// functions so the round trip can be tested without a window.
fn gain_of(fraction: f32) -> f32 {
    fraction.clamp(0.0, 1.0) * 2.0 * EQ_RANGE_DB - EQ_RANGE_DB
}

fn fraction_of(gain_db: f32) -> f32 {
    ((gain_db.clamp(-EQ_RANGE_DB, EQ_RANGE_DB) + EQ_RANGE_DB) / (2.0 * EQ_RANGE_DB)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A slider's position and its gain are the same thing measured two ways,
    /// so the round trip has to be exact at the ends and the middle.
    #[test]
    fn a_slider_position_is_its_gain() {
        assert_eq!(gain_of(0.0), -EQ_RANGE_DB, "the bottom is -12 dB");
        assert_eq!(gain_of(1.0), EQ_RANGE_DB, "the top is +12 dB");
        assert_eq!(gain_of(0.5), 0.0, "the middle is flat");
        for db in [-EQ_RANGE_DB, -6.0, 0.0, 6.0, EQ_RANGE_DB] {
            let back = gain_of(fraction_of(db));
            assert!((back - db).abs() < 0.001, "{db} came back as {back}");
        }
        // Out of range clamps rather than running off the track.
        assert_eq!(fraction_of(100.0), 1.0);
        assert_eq!(fraction_of(-100.0), 0.0);
        assert_eq!(gain_of(2.0), EQ_RANGE_DB);
        assert_eq!(gain_of(-1.0), -EQ_RANGE_DB);
    }

    /// The band background has twenty-eight states, and the ends of the range
    /// pick the ends of that grid.
    #[test]
    fn the_band_art_covers_the_whole_range() {
        let last = sprites::EQ_BAND_STEPS - 1;
        let step = |db: f32| {
            let fraction = fraction_of(db);
            ((fraction * last as f32).round() as u32).min(last)
        };
        assert_eq!(step(-EQ_RANGE_DB), 0);
        assert_eq!(step(EQ_RANGE_DB), last);
        // Twenty-eight states have no exact middle: flat is 13.5 of 27, which
        // rounds to 14. So the check is that it lands as close to the centre as
        // an even grid allows, not on a number picked by eye.
        let flat = step(0.0);
        assert_eq!(flat, 14, "flat is not the middle of the grid");
        assert!(
            flat.abs_diff(last - flat) <= 1,
            "flat is {flat}, which is off centre by more than the grid forces"
        );
        // Raising and lowering by the same amount must pick states the same
        // distance from the ends, or the track would brighten faster than it
        // dims. The pair sums to `last` because the grid's centre is 13.5 —
        // which is also why flat rounds *up* to 14 rather than sitting on it.
        for db in [3.0, 6.0, 9.0, EQ_RANGE_DB] {
            assert_eq!(
                step(db) + step(-db),
                last,
                "{db:+} dB and {:+} dB are not mirrored",
                -db
            );
        }
        // Every step is reachable, so no art goes unused.
        let reached: std::collections::BTreeSet<u32> = (0..=100)
            .map(|percent| step(gain_of(percent as f32 / 100.0)))
            .collect();
        assert_eq!(
            reached.len() as u32,
            sprites::EQ_BAND_STEPS,
            "some band art can never be shown"
        );
    }

    /// Every band sprite has to fit `eqmain.bmp`, whose grid is 14 across and
    /// two down: a step past the end would crop blank.
    #[test]
    fn the_band_sprites_stay_on_their_sheet() {
        for step in 0..sprites::EQ_BAND_STEPS {
            let sprite = sprites::eq_band(step);
            assert_eq!(sprite.sheet, Sheet::EqMain);
            assert_eq!(
                (sprite.w, sprite.h),
                (layout::BAND_WIDTH, layout::BAND_HEIGHT)
            );
            // Winamp's sheet is 275 wide and 315 tall.
            assert!(sprite.x + sprite.w <= 275, "step {step} runs off the right");
            assert!(
                sprite.y + sprite.h <= 315,
                "step {step} runs off the bottom"
            );
        }
        // Out of range clamps to the last, rather than reading past the sheet.
        assert_eq!(
            sprites::eq_band(sprites::EQ_BAND_STEPS),
            sprites::eq_band(sprites::EQ_BAND_STEPS - 1)
        );
    }
}
