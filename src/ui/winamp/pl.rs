//! The playlist window: the queue as Winamp's track list.
//!
//! Winamp's own window, drawn from [`crate::skin::layout::pl`] and the sprites
//! in `pledit.bmp`. It resizes in whole tiles, so the frame is drawn from nine
//! pieces rather than one background.
//!
//! Its five bottom menus are Winamp's ADD / REM / SEL / MISC / LIST. The
//! entries are the ones a SoundCloud queue can honour: there is no filesystem,
//! so "add file" is the app's own search and "load list" is a playlist rather
//! than a `.pls`.

use super::{Command, Frame, MiniPlayer, Snapshot, SortBy};
use crate::skin::layout::Area;
use crate::skin::layout::pl as layout;
use crate::skin::layout::stack::Window;
use crate::skin::{Sheet, pixel_text, sprites};
use eframe::egui;

/// The playlist's own view state, which no other window shares.
#[derive(Debug, Default)]
pub(super) struct View {
    /// The first visible row.
    pub scroll: usize,
    /// Selected rows, as queue indices. Winamp's list is multi-select.
    pub selected: std::collections::BTreeSet<usize>,
    /// Where a shift-range started.
    pub anchor: Option<usize>,
}

/// Which of the five bottom menus is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Menu {
    Add,
    Remove,
    Select,
    Misc,
    List,
}

impl MiniPlayer {
    /// Draw the playlist, rolled up or not.
    pub(super) fn pl_ui(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
        shade: bool,
    ) {
        // No `pledit.bmp` means no art for this window at all.
        if !self.skin.has(Sheet::Pledit) {
            return;
        }
        let width = crate::skin::MAIN_WIDTH;
        if shade {
            self.pl_shade_ui(ctx, ui, frame, state, width);
            return;
        }
        let rows = frame.stack.playlist_rows.max(layout::MIN_ROWS);
        let height = layout::height(rows);
        self.pl_frame(ctx, ui, frame, width, height);
        self.pl_title_bar(ctx, ui, frame, width);
        self.pl_tracks(ui, frame, state, width, height, rows);
        self.pl_scrollbar(ctx, ui, frame, state, width, height, rows);
        self.pl_bottom(ctx, ui, frame, state, width, height);
    }

    /// The nine-piece frame: two corners and a tile across the top, a tiled
    /// strip down each side, and two wide corners with a tile across the
    /// bottom.
    fn pl_frame(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &Frame,
        width: u32,
        height: u32,
    ) {
        // Top: left corner, then tiles, then the title block, then tiles, then
        // the right corner. Winamp centres the title block, so the two runs of
        // tile are whatever is left either side.
        let (left, tile, title, right) = if frame.focused {
            (
                sprites::PL_TOP_LEFT,
                sprites::PL_TOP_TILE,
                sprites::PL_TOP_TITLE,
                sprites::PL_TOP_RIGHT,
            )
        } else {
            (
                sprites::PL_TOP_LEFT_DIM,
                sprites::PL_TOP_TILE_DIM,
                sprites::PL_TOP_TITLE_DIM,
                sprites::PL_TOP_RIGHT_DIM,
            )
        };
        let title_x = (width.saturating_sub(title.w)) / 2;
        self.blit(ctx, ui, frame, left, (0, 0));
        self.tile_across(ctx, ui, frame, tile, left.w, title_x, 0);
        self.blit(ctx, ui, frame, title, (title_x, 0));
        self.tile_across(ctx, ui, frame, tile, title_x + title.w, width - right.w, 0);
        self.blit(ctx, ui, frame, right, (width - right.w, 0));

        // Sides, tiled down to the bottom strip.
        let bottom_top = height - layout::BOTTOM_HEIGHT;
        self.tile_down(
            ctx,
            ui,
            frame,
            sprites::PL_LEFT_TILE,
            0,
            layout::TOP_HEIGHT,
            bottom_top,
        );
        self.tile_down(
            ctx,
            ui,
            frame,
            sprites::PL_RIGHT_TILE,
            width - layout::RIGHT_WIDTH,
            layout::TOP_HEIGHT,
            bottom_top,
        );

        // Bottom: the two wide corners, and tiles between them if the window is
        // wider than they are.
        let bl = sprites::PL_BOTTOM_LEFT;
        let br = sprites::PL_BOTTOM_RIGHT;
        self.blit(ctx, ui, frame, bl, (0, bottom_top));
        if width > bl.w + br.w {
            self.tile_across(
                ctx,
                ui,
                frame,
                sprites::PL_BOTTOM_TILE,
                bl.w,
                width - br.w,
                bottom_top,
            );
        }
        self.blit(ctx, ui, frame, br, (width - br.w, bottom_top));
    }

    /// The title bar: the drag strip and the two buttons.
    fn pl_title_bar(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        width: u32,
    ) {
        let drag = ui.interact(
            frame.rect(layout::title_bar(width)),
            ui.id().with("pl-titlebar"),
            egui::Sense::click_and_drag(),
        );
        if drag.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            frame.commands.push(Command::ToggleShade(Window::Playlist));
        }
        // Winamp's playlist buttons carry only their pressed sprite; the
        // released state is the bar behind them.
        let shade_area = layout::shade_button(width);
        let shade_down = self
            .hit(ui, frame, shade_area, "pl-shade")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Roll up");
        if shade_down.is_pointer_button_down_on() {
            let sprite = if frame.stack.is_shade(Window::Playlist) {
                sprites::PL_UNSHADE
            } else {
                sprites::PL_SHADE
            };
            self.blit_at(ctx, ui, frame, sprite, shade_area);
        }
        if shade_down.clicked() {
            frame.commands.push(Command::ToggleShade(Window::Playlist));
        }
        let close_area = layout::close_button(width);
        let close = self
            .hit(ui, frame, close_area, "pl-close")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Close the playlist");
        if close.is_pointer_button_down_on() {
            self.blit_at(ctx, ui, frame, sprites::PL_CLOSE, close_area);
        }
        if close.clicked() {
            frame.commands.push(Command::ToggleWindow(Window::Playlist));
        }
    }

    /// The track rows: number, title, length, in the skin's playlist palette.
    fn pl_tracks(
        &mut self,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
        width: u32,
        height: u32,
        rows: u32,
    ) {
        let palette = self.skin.playlist;
        // PLEDIT.bmp only contains the chrome.  Paint the list well itself so
        // an empty/short queue never exposes the transparent native window.
        let list = layout::tracks(width, height);
        ui.painter()
            .rect_filled(frame.rect(list), 0.0, palette.background);
        let scroll = self
            .playlist
            .scroll
            .min(state.queue.len().saturating_sub(1));
        for row in 0..rows as usize {
            let index = scroll + row;
            let Some(entry) = state.queue.get(index) else {
                break;
            };
            let area = layout::row(width, height, row as u32);
            let selected = self.playlist.selected.contains(&index);
            let current = state.current == Some(index);
            if selected {
                ui.painter()
                    .rect_filled(frame.rect(area), 0.0, palette.selected_background);
            }
            let ink = if current {
                palette.current
            } else {
                palette.normal
            };

            // The length is right-aligned in its own column, and the title gets
            // whatever is left — Winamp truncates rather than wrapping.
            let length = crate::util::fmt::duration_ms(entry.duration_ms);
            let length_w = length.chars().count() as u32 * pixel_text::CHAR_W;
            let number = format!("{}. ", index + 1);
            let title_w = area.width.saturating_sub(length_w + 4);
            let text = format!("{number}{}", entry.title);
            self.pixels(
                ui,
                frame,
                Area::new(area.x + 2, area.y + 3, title_w, pixel_text::CHAR_H),
                &text,
                ink,
            );
            self.pixels(
                ui,
                frame,
                Area::new(
                    area.right() - length_w - 2,
                    area.y + 3,
                    length_w,
                    pixel_text::CHAR_H,
                ),
                &length,
                ink,
            );

            // A click selects, a double click plays — as Winamp's list did.
            let response = self.hit(ui, frame, area, &format!("pl-row-{row}"));
            if response.clicked() {
                let modifiers = ui.input(|i| i.modifiers);
                self.select_row(index, modifiers.command, modifiers.shift);
            }
            if response.double_clicked() {
                frame.commands.push(Command::PlayAt(index));
            }
        }

        // The wheel scrolls the list, three rows a notch as Winamp's did.
        if ui.rect_contains_pointer(frame.rect(layout::tracks(width, height))) {
            let notches = ui.input(super::wheel_notches);
            if notches != 0.0 {
                let by = (notches * 3.0).round() as i64;
                let max = state.queue.len().saturating_sub(rows as usize);
                let next = (self.playlist.scroll as i64 - by).clamp(0, max as i64);
                self.playlist.scroll = next as usize;
            }
        }
    }

    /// Selection, with Winamp's own modifiers: plain replaces, ctrl toggles,
    /// shift extends from the anchor.
    fn select_row(&mut self, index: usize, toggle: bool, extend: bool) {
        if extend {
            let anchor = self.playlist.anchor.unwrap_or(index);
            let (from, to) = if anchor <= index {
                (anchor, index)
            } else {
                (index, anchor)
            };
            self.playlist.selected = (from..=to).collect();
            return;
        }
        if toggle {
            if !self.playlist.selected.insert(index) {
                self.playlist.selected.remove(&index);
            }
        } else {
            self.playlist.selected.clear();
            self.playlist.selected.insert(index);
        }
        self.playlist.anchor = Some(index);
    }

    /// The scroll bar down the right edge.
    #[allow(clippy::too_many_arguments)]
    fn pl_scrollbar(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &Frame,
        state: &Snapshot,
        width: u32,
        height: u32,
        rows: u32,
    ) {
        let bar = layout::scrollbar(width, height);
        let hidden = state.queue.len().saturating_sub(rows as usize);
        if hidden == 0 {
            return;
        }
        let travel = bar.height - layout::SCROLL_HANDLE_H;
        let fraction = self.playlist.scroll as f32 / hidden as f32;
        let response = ui.interact(
            frame.rect(bar),
            ui.id().with("pl-scrollbar"),
            egui::Sense::click_and_drag(),
        );
        let handle = if response.is_pointer_button_down_on() {
            sprites::PL_SCROLL_HANDLE_DOWN
        } else {
            sprites::PL_SCROLL_HANDLE
        };
        let y = bar.y + (fraction * travel as f32).round() as u32;
        self.blit(ctx, ui, frame, handle, (bar.x, y));
        if (response.dragged() || response.clicked())
            && let Some(pos) = response.interact_pointer_pos()
        {
            let half = layout::SCROLL_HANDLE_H as f32 / 2.0 * frame.scale;
            let moved = ((pos.y - frame.rect(bar).top() - half) / (travel as f32 * frame.scale))
                .clamp(0.0, 1.0);
            self.playlist.scroll = (moved * hidden as f32).round() as usize;
        }
    }

    /// The bottom strip: the five menus, the readout, the transport, the little
    /// visualiser and the resize grip.
    fn pl_bottom(
        &mut self,
        _ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
        width: u32,
        height: u32,
    ) {
        // The frame already paints the skin's bottom controls. Keep their
        // artwork and wire the corresponding hit areas instead of erasing it.
        self.pl_menus(ui, frame, state, width, height);
        self.pl_running_time(ui, frame, state, width, height);
        self.pl_actions(ui, frame, width, height);
        // At the fixed 275px width, the optional visualizer would cover
        // REM/SEL/MISC. Reserve that space for the queue menus.

        // Dragging the corner resizes the list, in whole rows: the pointer's
        // distance from the window's top *is* the height it is asking for, so
        // the layout converts it back into rows and clamps it. Accumulating a
        // delta instead would drift, because a refused resize is not undone.
        let grip = layout::resize_grip(width, height);
        let response = ui
            .interact(
                frame.rect(grip),
                ui.id().with("pl-resize"),
                egui::Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeSouth);
        if response.dragged()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let top = frame.rect(Area::new(0, 0, width, height)).top();
            let asked = ((pos.y - top) / frame.scale).max(0.0) as u32;
            let rows = layout::rows_clamped(asked);
            if rows != frame.stack.playlist_rows {
                frame.commands.push(Command::ResizePlaylist(rows));
            }
        }
    }

    /// The little visualiser on the bottom strip: the same analyser as the main
    /// window's, in a 72-pixel well.
    #[allow(dead_code)]
    fn pl_visualiser(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
        width: u32,
        height: u32,
    ) {
        let area = layout::visualiser(width, height);
        // Winamp's well is part of `pledit.bmp`, cropped from the wide corner.
        self.blit_at(ctx, ui, frame, sprites::PL_VIS_BACKGROUND, area);
        let mode = self
            .shared
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .visualiser;
        self.visualiser_in(ui, frame, state, mode, area);
        if self
            .hit(ui, frame, area, "pl-visualiser")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(mode.label())
            .clicked()
        {
            frame.commands.push(Command::CycleVisualiser);
        }
    }

    /// Winamp's five menus. Each is a button that pops its own list open.
    #[allow(dead_code)]
    fn pl_menus(
        &mut self,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
        width: u32,
        height: u32,
    ) {
        let selection: Vec<usize> = self.playlist.selected.iter().copied().collect();
        let any = !selection.is_empty();
        let queued = !state.queue.is_empty();
        for (index, menu) in [
            Menu::Add,
            Menu::Remove,
            Menu::Select,
            Menu::Misc,
            Menu::List,
        ]
        .into_iter()
        .enumerate()
        {
            // The hit area and the label painted into `pledit.bmp` come from the
            // same place, so a click lands on the word it looks like it lands on.
            let area = layout::menu(width, height, index);
            let label = layout::MENU_LABELS[index];
            let response = self
                .hit(ui, frame, area, label)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(label);
            egui::Popup::menu(&response).show(|ui| {
                ui.set_min_width(170.0);
                match menu {
                    Menu::Add => {
                        // No filesystem: Winamp's three "add" entries all
                        // become the one thing this app can do.
                        if ui.button("Add tracks…").clicked() {
                            frame.commands.push(Command::AddTracks);
                            ui.close();
                        }
                    }
                    Menu::Remove => {
                        if ui
                            .add_enabled(any, egui::Button::new("Remove selected"))
                            .clicked()
                        {
                            frame
                                .commands
                                .push(Command::RemoveTracks(selection.clone()));
                            ui.close();
                        }
                        if ui
                            .add_enabled(any, egui::Button::new("Crop to selection"))
                            .clicked()
                        {
                            frame.commands.push(Command::CropQueue(selection.clone()));
                            ui.close();
                        }
                        if ui
                            .add_enabled(queued, egui::Button::new("Remove all"))
                            .clicked()
                        {
                            frame.commands.push(Command::ClearQueue);
                            ui.close();
                        }
                    }
                    Menu::Select => {
                        if ui.button("Select all").clicked() {
                            self.playlist.selected = (0..state.queue.len()).collect();
                            ui.close();
                        }
                        if ui.button("Select none").clicked() {
                            self.playlist.selected.clear();
                            ui.close();
                        }
                        if ui.button("Invert selection").clicked() {
                            let all: std::collections::BTreeSet<usize> =
                                (0..state.queue.len()).collect();
                            self.playlist.selected =
                                all.difference(&self.playlist.selected).copied().collect();
                            ui.close();
                        }
                    }
                    Menu::Misc => {
                        if ui
                            .add_enabled(queued, egui::Button::new("Sort by title"))
                            .clicked()
                        {
                            frame.commands.push(Command::SortQueue(SortBy::Title));
                            ui.close();
                        }
                        if ui
                            .add_enabled(queued, egui::Button::new("Sort by artist"))
                            .clicked()
                        {
                            frame.commands.push(Command::SortQueue(SortBy::Artist));
                            ui.close();
                        }
                        if ui
                            .add_enabled(queued, egui::Button::new("Reverse"))
                            .clicked()
                        {
                            frame.commands.push(Command::SortQueue(SortBy::Reverse));
                            ui.close();
                        }
                        if ui
                            .add_enabled(queued, egui::Button::new("Randomise"))
                            .clicked()
                        {
                            frame.commands.push(Command::ShuffleQueue);
                            ui.close();
                        }
                        ui.separator();
                        if ui
                            .add_enabled(any, egui::Button::new("Track info"))
                            .clicked()
                        {
                            if let Some(first) = selection.first() {
                                frame.commands.push(Command::TrackInfo(*first));
                            }
                            ui.close();
                        }
                    }
                    Menu::List => {
                        // No `.pls` files, so "save list" is a playlist.
                        if ui
                            .add_enabled(queued, egui::Button::new("Save as playlist…"))
                            .clicked()
                        {
                            frame.commands.push(Command::SaveQueue);
                            ui.close();
                        }
                        if ui
                            .add_enabled(queued, egui::Button::new("New list"))
                            .clicked()
                        {
                            frame.commands.push(Command::ClearQueue);
                            ui.close();
                        }
                    }
                }
            });
        }
    }

    /// The `elapsed / total` readout, and how many tracks are in the queue.
    #[allow(dead_code)]
    fn pl_running_time(
        &self,
        ui: &egui::Ui,
        frame: &Frame,
        state: &Snapshot,
        width: u32,
        height: u32,
    ) {
        let total: u64 = state.queue.iter().map(|e| e.duration_ms).sum();
        let text = format!(
            "{}/{}",
            crate::util::fmt::duration_ms(state.position_ms),
            crate::util::fmt::duration_ms(total)
        );
        self.pixels(
            ui,
            frame,
            layout::running_time(width, height),
            &text,
            self.skin.playlist.normal,
        );
    }

    /// The five little transport buttons on the bottom strip.
    #[allow(dead_code)]
    fn pl_actions(&mut self, ui: &mut egui::Ui, frame: &mut Frame, width: u32, height: u32) {
        use super::Press;
        for (index, (press, tooltip)) in [
            (Press::Previous, "Previous"),
            (Press::Play, "Play"),
            (Press::Pause, "Pause"),
            (Press::Stop, "Stop"),
            (Press::Next, "Next"),
        ]
        .into_iter()
        .enumerate()
        {
            let area = layout::action(width, height, index);
            if self
                .hit(ui, frame, area, &format!("pl-action-{index}"))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(tooltip)
                .clicked()
            {
                self.press(press);
            }
        }
    }

    /// Rolled up: the title on the left, the time on the right.
    fn pl_shade_ui(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
        width: u32,
    ) {
        let left = sprites::PL_SHADE_LEFT;
        let right = sprites::PL_SHADE_RIGHT;
        self.blit(ctx, ui, frame, left, (0, 0));
        self.tile_across(
            ctx,
            ui,
            frame,
            sprites::PL_SHADE_TILE,
            left.w,
            width - right.w,
            0,
        );
        self.blit(ctx, ui, frame, right, (width - right.w, 0));
        self.pl_title_bar(ctx, ui, frame, width);

        let ink = self.skin.playlist.normal;
        let title = match state.current.and_then(|i| state.queue.get(i)) {
            Some(entry) => entry.title.clone(),
            None => "Fastcloud".to_owned(),
        };
        let box_area = layout::shade_title(width);
        let shown = pixel_text::marquee(&title, box_area.cells(), state.tick);
        self.pixels(ui, frame, box_area, &shown, ink);
        self.pixels(
            ui,
            frame,
            layout::shade_time(width),
            &crate::util::fmt::duration_ms(state.position_ms),
            ink,
        );
    }

    /// Repeat a sprite across a run, clipping the last one.
    #[allow(clippy::too_many_arguments)]
    fn tile_across(
        &mut self,
        ctx: &egui::Context,
        ui: &egui::Ui,
        frame: &Frame,
        sprite: crate::skin::Sprite,
        from: u32,
        to: u32,
        y: u32,
    ) {
        if sprite.w == 0 {
            return;
        }
        let mut x = from;
        while x < to {
            let mut piece = sprite;
            // The last tile is cut short rather than hanging past the edge.
            piece.w = sprite.w.min(to - x);
            self.blit(ctx, ui, frame, piece, (x, y));
            x += sprite.w;
        }
    }

    /// Repeat a sprite down a run, clipping the last one.
    #[allow(clippy::too_many_arguments)]
    fn tile_down(
        &mut self,
        ctx: &egui::Context,
        ui: &egui::Ui,
        frame: &Frame,
        sprite: crate::skin::Sprite,
        x: u32,
        from: u32,
        to: u32,
    ) {
        if sprite.h == 0 {
            return;
        }
        let mut y = from;
        while y < to {
            let mut piece = sprite;
            piece.h = sprite.h.min(to - y);
            self.blit(ctx, ui, frame, piece, (x, y));
            y += sprite.h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        View::default()
    }

    /// Plain click replaces the selection, ctrl toggles, shift extends — and a
    /// shift-range works in either direction.
    #[test]
    fn selection_follows_winamps_modifiers() {
        let mut player = crate::ui::winamp::tests::mini(1);
        player.playlist = view();

        player.select_row(3, false, false);
        assert_eq!(
            player.playlist.selected.iter().copied().collect::<Vec<_>>(),
            [3]
        );

        // Ctrl adds, and a second ctrl-click on the same row removes it.
        player.select_row(5, true, false);
        assert_eq!(
            player.playlist.selected.iter().copied().collect::<Vec<_>>(),
            [3, 5]
        );
        player.select_row(5, true, false);
        assert_eq!(
            player.playlist.selected.iter().copied().collect::<Vec<_>>(),
            [3]
        );

        // Shift extends from the anchor, downward…
        player.select_row(3, false, false);
        player.select_row(6, false, true);
        assert_eq!(
            player.playlist.selected.iter().copied().collect::<Vec<_>>(),
            [3, 4, 5, 6]
        );
        // …and upward, which is the case a naive range gets wrong.
        player.select_row(6, false, false);
        player.select_row(2, false, true);
        assert_eq!(
            player.playlist.selected.iter().copied().collect::<Vec<_>>(),
            [2, 3, 4, 5, 6]
        );

        // A plain click after all that clears back to one row.
        player.select_row(9, false, false);
        assert_eq!(
            player.playlist.selected.iter().copied().collect::<Vec<_>>(),
            [9]
        );
    }

    /// Shift with no anchor selects just that row, rather than everything from
    /// zero.
    #[test]
    fn shift_without_an_anchor_selects_one_row() {
        let mut player = crate::ui::winamp::tests::mini(1);
        player.playlist = view();
        player.select_row(4, false, true);
        assert_eq!(
            player.playlist.selected.iter().copied().collect::<Vec<_>>(),
            [4]
        );
    }
}
