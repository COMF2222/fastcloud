//! The Winamp mini player: the window wearing a classic `.wsz` skin.
//!
//! `Ctrl+M` opens it (or the button in the top bar). It is the app's one
//! window — not a second one — and is drawn entirely from sprite rectangles
//! ([`crate::skin`]) at a whole scale, 1× to 4×, because the classic look only
//! survives on exact pixels.
//!
//! ## Three states, one window
//!
//! Winamp's title bar had three buttons and this keeps all three, because
//! between them they are what makes a small player usable:
//!
//! * **close** puts the app's own interface back ([`Command::Close`]);
//! * **minimize** sends the window to the taskbar and leaves it playing;
//! * **shade** rolls it up to its 14-pixel title bar — still the mini player,
//!   with the title, the time, the transport and a seek bar, but no interface
//!   ([`Command::ToggleShade`]).
//!
//! ## Where the coordinates come from
//!
//! Nothing here chooses a position. [`crate::skin::layout`] holds every
//! rectangle in Winamp's own numbers and [`crate::skin::sprites`] holds the
//! art to fill them, and the two are checked against each other by tests. This
//! module only asks "which control, in which state".
//!
//! ## Why the window drives itself
//!
//! The first cut pushed every value (title, position, spectrum) from the main
//! window's `logic()`, which repaints at 5 Hz — so the bars crawled at 5 Hz
//! and the time digits ticked unevenly. The window now reads
//! [`crate::player::Player`] directly (its methods are `&self` and
//! thread-safe) and keeps its **own** [`crate::vis::Analyser`], so it runs on
//! its own clock at Winamp's 60 Hz. Only what needs `App` — closing,
//! minimizing, rolling up, persisting a slider — travels back through
//! [`Shared`].
//!
//! ## Why meshes
//!
//! The title is a 5×6 pixel font and the analyser is nineteen bars of up to
//! sixteen rows: drawn as individual rectangles that is ~1800 shapes a frame,
//! each one tessellated separately. Both go into a single [`egui::Mesh`]
//! instead, which is one draw call and no per-pixel tessellation.

mod eq;
mod eq_curve;
mod pl;

use crate::player::Player;
use crate::skin::layout::{self, Area};
use crate::skin::{Sheet, Skin, Sprite, pixel_text, sprites};
use crate::vis;
use eframe::egui;
use std::sync::{Arc, Mutex};

// Which windows exist and how they stack is the skin layout's business, but the
// app talks about them too, so the two names travel with the mini player rather
// than making every caller reach into the layout.
pub use crate::skin::layout::stack::{Stack, Window};

/// The window's size in skin pixels.
pub const WIDTH: f32 = crate::skin::MAIN_WIDTH as f32;
pub const HEIGHT: f32 = crate::skin::MAIN_HEIGHT as f32;
/// A window rolled up to its title bar. Every one of the three rolls up to the
/// same fourteen pixels, because every one is the same title bar.
pub const SHADE_HEIGHT: f32 = layout::SHADE_HEIGHT as f32;
// Rolling up has to make the window shorter, or the mode is pointless.
const _: () = assert!(SHADE_HEIGHT < HEIGHT);

/// How often the marquee advances, in seconds per character. Winamp scrolled
/// a long title at about four characters a second.
const MARQUEE_HZ: f64 = 4.0;

/// What the mini player needs the app itself to do. Everything else it does
/// straight on the [`Player`].
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Close the mini player and show the main window again.
    Close,
    /// Send the window to the taskbar, still playing.
    Minimize,
    /// Roll a window up to its title bar, or back down.
    ToggleShade(Window),
    /// Open or close the equaliser or the playlist.
    ToggleWindow(Window),
    /// The volume moved; persist it (0–100).
    Volume(u8),
    /// The balance moved; persist it (-100 left … 100 right).
    Balance(i8),
    /// Spectrum → oscilloscope → off, as Winamp's V did.
    CycleVisualiser,
    /// Keep the window above the others, as Winamp's "always on top".
    ToggleOnTop,
    /// Fold stereo to mono, or restore the source channels.
    SetMono(bool),
    /// Cycle the classic 2x, 3x and 4x window sizes.
    CycleScale,
    /// Pick an exact classic scale from the O menu.
    SetScale(u32),
    /// Open the current track in Fastcloud's detail page.
    TrackInfoCurrent,
    /// The wordmark at the bottom right: which skin, and which build.
    About,
    /// The equaliser changed; persist it and send it to the player.
    Eq(EqSettings),
    /// Save the current curve as this track's own preset, which AUTO reloads.
    SaveEqPreset,
    /// Forget this track's preset.
    ClearEqPreset,
    /// Play the queue entry at this index.
    PlayAt(usize),
    /// Remove these queue entries (indices into the queue, ascending).
    RemoveTracks(Vec<usize>),
    /// Empty the queue.
    ClearQueue,
    /// Keep only the selection, dropping everything else.
    CropQueue(Vec<usize>),
    /// Sort the queue by title, or by artist.
    SortQueue(SortBy),
    /// Shuffle the queue's order.
    ShuffleQueue,
    /// Save the queue as a playlist, which is Winamp's "save list".
    SaveQueue,
    /// Open the track's page, which is Winamp's file info.
    TrackInfo(usize),
    /// Add tracks: there is no filesystem, so this is the app's own search.
    AddTracks,
    /// The playlist was dragged to a new number of rows.
    ResizePlaylist(u32),
}

/// How Winamp's SORT menu orders the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Title,
    Artist,
    Reverse,
}

/// The equaliser's settings, which the app persists.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqSettings {
    pub enabled: bool,
    /// Winamp's AUTO: reload each track's own preset when it starts.
    pub auto: bool,
    pub preamp_db: f32,
    pub gains_db: [f32; 10],
}

impl Default for EqSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            auto: false,
            preamp_db: 0.0,
            gains_db: [0.0; 10],
        }
    }
}

/// The little that crosses between the app and the window.
#[derive(Debug, Default)]
pub struct Shared {
    /// Which trace the visualiser shows, mirrored from settings.
    pub visualiser: crate::ui::visualiser::Mode,
    /// Which windows are open and which are rolled up.
    pub stack: Stack,
    /// Whether the window is pinned above the others.
    pub on_top: bool,
    /// The equaliser's settings, mirrored from the app.
    pub eq: EqSettings,
    /// Whether the current track has a saved preset, which the PRESETS menu
    /// shows and AUTO reloads.
    pub has_preset: bool,
    /// Commands the skin produced, drained by the app each frame.
    pub commands: Vec<Command>,
}

/// The mini player's state: the skin, its textures, the player it drives and
/// its own analyser.
pub struct MiniPlayer {
    skin: Skin,
    /// Sprite textures, keyed by the sheet and rectangle they came from.
    textures: std::collections::HashMap<(Sheet, u32, u32, u32, u32), egui::TextureHandle>,
    player: Arc<Player>,
    /// This window's own analyser, stepped in its own frame so the bars keep
    /// Winamp's 60 Hz beat whatever the main window is doing.
    analyser: vis::Analyser,
    /// Where the seek thumb is being dragged to, 0..=1. The seek itself waits
    /// for the release (see the seek bar in [`Self::ui`]).
    seek_preview: Option<f32>,
    /// Show the time counting down instead of up, as clicking it did.
    time_remaining: bool,
    /// The playlist's own view state: what is scrolled to and selected.
    playlist: pl::View,
    /// The equaliser graph's line colours, read from `eqmain.bmp`'s one-pixel
    /// column when the skin is worn: nineteen rows, top to bottom. Read once
    /// rather than per frame, because it cannot change without a new skin.
    eq_curve_colors: Option<Vec<egui::Color32>>,
    /// The preamp line's colour, from the same sheet.
    preamp_ink: egui::Color32,
    /// Commands produced by a window whose frame has already been dropped.
    pending: Vec<Command>,
    pub shared: Arc<Mutex<Shared>>,
    /// Whole-pixel scale, 1–4.
    pub scale: u32,
}

impl MiniPlayer {
    pub fn new(skin: Skin, scale: u32, player: Arc<Player>) -> Self {
        let mut this = Self {
            skin,
            textures: std::collections::HashMap::new(),
            player,
            analyser: vis::Analyser::default(),
            seek_preview: None,
            time_remaining: false,
            playlist: pl::View::default(),
            eq_curve_colors: None,
            preamp_ink: egui::Color32::GRAY,
            pending: Vec::new(),
            shared: Arc::new(Mutex::new(Shared::default())),
            scale: scale.clamp(1, 4),
        };
        this.read_skin_colors();
        this
    }

    pub fn skin_name(&self) -> &str {
        &self.skin.name
    }

    /// Swap the skin, dropping the textures cut from the old one.
    pub fn set_skin(&mut self, skin: Skin) {
        self.skin = skin;
        self.textures.clear();
        self.read_skin_colors();
    }

    /// Pull the colours that are art rather than palette out of the sheets.
    ///
    /// Winamp keeps the equaliser graph's line colours *in* `eqmain.bmp` as a
    /// one-pixel column, and the preamp line as a one-pixel row. Both are read
    /// here so the graph does not crop a sprite on every frame.
    fn read_skin_colors(&mut self) {
        self.eq_curve_colors = self.skin.column(sprites::EQ_GRAPH_COLORS);
        self.preamp_ink = self
            .skin
            .sprite(sprites::EQ_PREAMP_LINE)
            .and_then(|image| image.pixels.first().copied())
            .filter(|colour| colour.a() > 0)
            .unwrap_or(self.skin.playlist.normal);
    }

    /// Which windows are open and which are rolled up.
    pub fn stack(&self) -> Stack {
        self.shared.lock().unwrap_or_else(|p| p.into_inner()).stack
    }

    /// Whether the main window is rolled up to its title bar.
    #[cfg(test)]
    pub fn shade(&self) -> bool {
        self.stack().is_shade(Window::Main)
    }

    /// The stack's height in skin pixels: all the open windows together.
    pub fn height(&self) -> f32 {
        self.stack().size().1 as f32
    }

    /// A sprite as a texture, cut and uploaded once.
    fn texture(&mut self, ctx: &egui::Context, sprite: Sprite) -> Option<egui::TextureId> {
        let key = (sprite.sheet, sprite.x, sprite.y, sprite.w, sprite.h);
        if let Some(handle) = self.textures.get(&key) {
            return Some(handle.id());
        }
        let image = self.skin.sprite(sprite)?;
        // Nearest sampling: the classic look is exact pixels, not a blur.
        let handle = ctx.load_texture(
            format!("skin-{:?}-{}-{}", sprite.sheet, sprite.x, sprite.y),
            image,
            egui::TextureOptions::NEAREST,
        );
        let id = handle.id();
        self.textures.insert(key, handle);
        Some(id)
    }

    /// Draw one frame of the mini player into `ui`.
    ///
    /// Reads the player, applies transport straight to it, and leaves only
    /// [`Command`]s the app must handle on [`Shared::commands`].
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let (mode, stack, on_top, eq, has_preset) = {
            let shared = self.shared.lock().unwrap_or_else(|p| p.into_inner());
            (
                shared.visualiser,
                shared.stack,
                shared.on_top,
                shared.eq,
                shared.has_preset,
            )
        };
        let state = self.snapshot(&ctx, mode);
        let mut frame = Frame {
            origin: ui.max_rect().min,
            scale: self.scale as f32,
            commands: Vec::new(),
            on_top,
            stack,
            eq,
            has_preset,
            // Winamp dimmed an unfocused window's title bar, and the sheets
            // carry both rows for it. A frameless window that never dims looks
            // stuck on top of everything.
            focused: ctx.input(|i| i.viewport().focused.unwrap_or(true)),
        };

        // Each window paints in its own coordinate space, clipped to its own
        // area, so a renderer never has to know what is stacked above it and
        // cannot paint over its neighbours.
        for (window, top) in stack.open_windows().collect::<Vec<_>>() {
            let mut frame = frame.offset(top);
            let clip = stack.area_of(window).map(|area| {
                frame
                    .offset(0)
                    .rect(Area::new(0, 0, area.width, area.height))
            });
            let mut scoped =
                ui.new_child(egui::UiBuilder::new().max_rect(clip.unwrap_or(ui.max_rect())));
            if let Some(clip) = clip {
                scoped.set_clip_rect(clip);
            }
            let shade = stack.is_shade(window);
            match window {
                Window::Main => {
                    if shade {
                        self.shade_ui(&ctx, &mut scoped, &mut frame, &state);
                    } else {
                        self.full_ui(&ctx, &mut scoped, &mut frame, &state, mode);
                    }
                    self.title_bar(&ctx, &mut scoped, &mut frame, shade);
                }
                Window::Equalizer => self.eq_ui(&ctx, &mut scoped, &mut frame, shade),
                Window::Playlist => self.pl_ui(&ctx, &mut scoped, &mut frame, &state, shade),
            }
            let produced = std::mem::take(&mut frame.commands);
            drop(frame);
            self.pending.extend(produced);
        }
        frame.commands.append(&mut self.pending);

        if !frame.commands.is_empty() {
            let mut shared = self.shared.lock().unwrap_or_else(|p| p.into_inner());
            shared.commands.extend(frame.commands);
        }

        // Only ask for another frame while something actually moves: the
        // analyser still settling, a clock that has to tick, or a title long
        // enough to scroll. A paused window with a short title costs nothing.
        if state.wants_frames {
            ctx.request_repaint_after(vis::STEP);
        }
    }

    /// The whole 275×116 window.
    fn full_ui(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
        mode: crate::ui::visualiser::Mode,
    ) {
        // Background first: everything else sits in its holes.
        self.blit(ctx, ui, frame, sprites::MAIN, (0, 0));
        // A real skin keeps its title bar in `titlebar.bmp`, and leaves
        // `main.bmp`'s top fourteen rows unpainted — so the bar goes on over
        // it. The stock skin draws both the same, so this is a no-op there.
        if self.skin.has(Sheet::Titlebar) {
            let bar = if frame.focused {
                sprites::TITLE_BAR
            } else {
                sprites::TITLE_BAR_DIM
            };
            self.blit_at(ctx, ui, frame, bar, layout::TITLE_BAR);
        }
        self.clutter_bar(ctx, ui, frame, mode);
        self.transport(ctx, ui, frame, state);
        self.position(ctx, ui, frame, state);
        self.sliders(ctx, ui, frame, state);
        self.window_buttons(ctx, ui, frame);
        self.status(ctx, ui, frame, state);
        self.time(ctx, ui, frame, state, layout::TIME_DIGITS, layout::MINUS_EX);
        self.marquee(ui, frame, layout::MARQUEE, &state.title, state.tick);
        self.media_info(ui, frame, state);
        self.mono_stereo(ctx, ui, frame, state);
        self.visualiser(ui, frame, state, mode);
    }

    /// The window rolled up: title, time, transport and a seek bar, all on the
    /// title bar itself. Winamp's shade mode.
    fn shade_ui(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
    ) {
        // A skin without `titlebar.bmp` has no shade art, so the main
        // background's own top strip stands in: the same pixels the unrolled
        // window shows there.
        let bar = if !self.skin.has(Sheet::Titlebar) {
            sprites::MAIN_TOP
        } else if frame.focused {
            sprites::SHADE_BAR
        } else {
            sprites::SHADE_BAR_DIM
        };
        self.blit(ctx, ui, frame, bar, (0, 0));

        self.marquee(ui, frame, layout::SHADE_MARQUEE, &state.title, state.tick);
        self.shade_time(ui, frame, state);

        // The transport is the same five presses, drawn by the background
        // rather than by sprites of their own: Winamp's shade bar has the
        // glyphs painted in, and only the hit areas are ours.
        for (area, press) in [
            (layout::SHADE_PREVIOUS, Press::Previous),
            (layout::SHADE_PLAY, Press::PlayPause),
            (layout::SHADE_PAUSE, Press::PlayPause),
            (layout::SHADE_STOP, Press::Stop),
            (layout::SHADE_NEXT, Press::Next),
        ] {
            if self.hit(ui, frame, area, "shade-transport").clicked() {
                self.press(press);
            }
        }
        if self
            .hit(ui, frame, layout::SHADE_EJECT, "shade-eject")
            .clicked()
        {
            frame.commands.push(Command::Close);
        }
        self.shade_position(ctx, ui, frame, state);
    }

    /// Read the player and step this window's analyser.
    fn snapshot(&mut self, ctx: &egui::Context, mode: crate::ui::visualiser::Mode) -> Snapshot {
        use crate::ui::visualiser::Mode;
        let (
            track,
            queue,
            current,
            playing,
            stopped,
            loading,
            position_ms,
            duration_ms,
            shuffle,
            repeat,
            volume,
            balance,
            mono,
            sample_rate,
            bitrate_kbps,
            channels,
        ) = {
            let st = self.player.state.lock();
            let track = st.current.and_then(|i| st.queue.get(i).cloned());
            // The playlist needs a row per queue entry; a title and a length is
            // all it draws, so the tracks themselves are not cloned.
            let queue: Vec<Entry> = st
                .queue
                .iter()
                .map(|t| Entry {
                    title: format!("{} - {}", t.artist(), t.title),
                    duration_ms: t.effective_duration_ms(),
                })
                .collect();
            (
                track,
                queue,
                st.current,
                st.is_playing,
                st.current.is_none(),
                st.loading,
                st.position_ms,
                st.duration_ms,
                st.shuffle,
                st.repeat,
                st.volume,
                st.balance,
                st.mono,
                st.sample_rate,
                st.bitrate_kbps,
                st.channels,
            )
        };
        let title = match &track {
            Some(t) => format!("{} - {}", t.artist(), t.title),
            None => "Fastcloud".to_owned(),
        };
        let tap = self.player.output_handle().tap();
        let (bars, scope) = match mode {
            Mode::Spectrum => {
                let samples = tap.window(vis::FFT_SAMPLES, vis::LAG);
                (
                    self.analyser.step(&samples, std::time::Instant::now()),
                    [7; vis::COLUMNS],
                )
            }
            Mode::Scope => {
                let samples = tap.window(vis::SCOPE_SAMPLES, vis::LAG);
                ([vis::Bar::default(); vis::BARS], vis::scope(&samples))
            }
            Mode::Off => ([vis::Bar::default(); vis::BARS], [7; vis::COLUMNS]),
        };
        let now = ctx.input(|i| i.time);
        // The rolled-up window has a shorter marquee, so it scrolls titles the
        // full one would not: take the smaller of the two.
        let scrolls = title.chars().count() > layout::SHADE_MARQUEE_CHARS;
        let wants_frames = playing
            || loading
            || scrolls
            || (mode == Mode::Spectrum && !self.analyser.settled())
            || mode == Mode::Scope;
        Snapshot {
            title,
            queue,
            current,
            position_ms,
            duration_ms,
            playing,
            stopped,
            loading,
            shuffle,
            repeat_on: repeat != crate::player::RepeatMode::Off,
            volume: (volume.clamp(0.0, 1.0) * 100.0).round() as u8,
            balance: (balance.clamp(-1.0, 1.0) * 100.0).round() as i8,
            mono,
            sample_rate,
            bitrate_kbps,
            channels,
            tick: (now * MARQUEE_HZ) as usize,
            bars,
            scope,
            wants_frames,
        }
    }

    /// The five transport buttons, eject, shuffle and repeat.
    fn transport(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
    ) {
        // Winamp has five buttons and separate play and pause: the pair sit
        // side by side rather than one replacing the other.
        for (area, up, down, press) in [
            (
                layout::PREVIOUS,
                sprites::PREVIOUS,
                sprites::PREVIOUS_DOWN,
                Press::Previous,
            ),
            (layout::PLAY, sprites::PLAY, sprites::PLAY_DOWN, Press::Play),
            (
                layout::PAUSE,
                sprites::PAUSE,
                sprites::PAUSE_DOWN,
                Press::Pause,
            ),
            (layout::STOP, sprites::STOP, sprites::STOP_DOWN, Press::Stop),
            (layout::NEXT, sprites::NEXT, sprites::NEXT_DOWN, Press::Next),
        ] {
            if self.button(ctx, ui, frame, area, up, down, "") {
                self.press(press);
            }
        }
        // Eject closes the mini player, as it opened files in Winamp.
        if self.button(
            ctx,
            ui,
            frame,
            layout::EJECT,
            sprites::EJECT,
            sprites::EJECT_DOWN,
            "Back to Fastcloud",
        ) {
            frame.commands.push(Command::Close);
        }

        // Shuffle and repeat: lit when on.
        let (shuffle, shuffle_down) = if state.shuffle {
            (sprites::SHUFFLE_ON, sprites::SHUFFLE_ON_DOWN)
        } else {
            (sprites::SHUFFLE, sprites::SHUFFLE_DOWN)
        };
        if self.button(
            ctx,
            ui,
            frame,
            layout::SHUFFLE,
            shuffle,
            shuffle_down,
            "Shuffle",
        ) {
            self.press(Press::ToggleShuffle);
        }
        let (repeat, repeat_down) = if state.repeat_on {
            (sprites::REPEAT_ON, sprites::REPEAT_ON_DOWN)
        } else {
            (sprites::REPEAT, sprites::REPEAT_DOWN)
        };
        if self.button(
            ctx,
            ui,
            frame,
            layout::REPEAT,
            repeat,
            repeat_down,
            "Repeat",
        ) {
            self.press(Press::CycleRepeat);
        }
    }

    /// The seek bar: the groove, then the thumb where the track is.
    ///
    /// Dragging shows a preview and only seeks on release. Seeking on every
    /// frame of a drag asked the player for a position outside the buffered
    /// window over and over, and each one of those reloads the stream — the
    /// audio stuttered and the thumb fought the pointer.
    fn position(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
    ) {
        self.blit_at(ctx, ui, frame, sprites::POSBAR, layout::POSITION);
        let fraction = self.seek(
            ui,
            frame,
            state,
            layout::POSITION,
            layout::POSITION_TRAVEL,
            layout::POSITION_THUMB_W,
            "posbar",
        );
        if let Some((fraction, down)) = fraction {
            let thumb = if down {
                sprites::POSBAR_THUMB_DOWN
            } else {
                sprites::POSBAR_THUMB
            };
            let x = layout::POSITION.x + (fraction * layout::POSITION_TRAVEL as f32).round() as u32;
            self.blit(ctx, ui, frame, thumb, (x, layout::POSITION.y));
        }
    }

    /// Shade mode's 17-pixel seek bar. Same drag rules, three-pixel thumb.
    fn shade_position(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
    ) {
        let travel = layout::SHADE_POSITION.width - layout::SHADE_POSITION_THUMB_W;
        // Only a skin with `titlebar.bmp` has the groove; without it the bar
        // is still draggable, just unmarked.
        if self.skin.has(Sheet::Titlebar) {
            self.blit_at(
                ctx,
                ui,
                frame,
                sprites::SHADE_POSBAR,
                layout::SHADE_POSITION,
            );
        }
        let dragged = self.seek(
            ui,
            frame,
            state,
            layout::SHADE_POSITION,
            travel,
            layout::SHADE_POSITION_THUMB_W,
            "shade-posbar",
        );
        if let Some((fraction, _)) = dragged
            && self.skin.has(Sheet::Titlebar)
        {
            // The ends have their own sprites, so the thumb never overhangs
            // the groove it sits in.
            let steps = (fraction * travel as f32).round() as u32;
            let thumb = if steps == 0 {
                sprites::SHADE_POSBAR_THUMB_LEFT
            } else if steps >= travel {
                sprites::SHADE_POSBAR_THUMB_RIGHT
            } else {
                sprites::SHADE_POSBAR_THUMB
            };
            let x = layout::SHADE_POSITION.x + steps;
            self.blit(ctx, ui, frame, thumb, (x, layout::SHADE_POSITION.y));
        }
    }

    /// The seek interaction shared by both modes.
    ///
    /// Returns where the thumb should be drawn and whether it is held, or
    /// `None` when there is nothing to seek in.
    #[allow(clippy::too_many_arguments)]
    fn seek(
        &mut self,
        ui: &mut egui::Ui,
        frame: &Frame,
        state: &Snapshot,
        area: Area,
        travel: u32,
        thumb_w: u32,
        salt: &str,
    ) -> Option<(f32, bool)> {
        let rect = frame.rect(area);
        let seekable = state.duration_ms > 0 && !state.stopped;
        let response = ui.interact(
            rect,
            ui.id().with(("mini-seek", salt)),
            if seekable {
                egui::Sense::click_and_drag()
            } else {
                egui::Sense::hover()
            },
        );
        // The pointer's position as a fraction of the thumb's travel, so the
        // thumb sits under the finger rather than half a thumb to its right.
        let pointer_fraction = |pos: egui::Pos2| {
            let half_thumb = thumb_w as f32 / 2.0 * frame.scale;
            ((pos.x - rect.left() - half_thumb) / (travel as f32 * frame.scale)).clamp(0.0, 1.0)
        };
        if seekable {
            if (response.drag_started() || response.dragged())
                && let Some(pos) = response.interact_pointer_pos()
            {
                self.seek_preview = Some(pointer_fraction(pos));
            }
            if response.drag_stopped() {
                let fraction = response
                    .interact_pointer_pos()
                    .map(pointer_fraction)
                    .or(self.seek_preview);
                self.seek_preview = None;
                if let Some(fraction) = fraction {
                    self.player
                        .seek_ms((fraction as f64 * state.duration_ms as f64) as u64);
                }
            } else if response.clicked()
                && let Some(pos) = response.interact_pointer_pos()
            {
                self.seek_preview = None;
                self.player
                    .seek_ms((pointer_fraction(pos) as f64 * state.duration_ms as f64) as u64);
            }
        }
        if !seekable {
            return None;
        }
        // What the thumb shows: the drag while there is one, else the track.
        let fraction = self
            .seek_preview
            .unwrap_or(if state.duration_ms > 0 {
                state.position_ms as f32 / state.duration_ms as f32
            } else {
                0.0
            })
            .clamp(0.0, 1.0);
        Some((fraction, response.is_pointer_button_down_on()))
    }

    /// Volume and balance: a strip whose row is the level, then its thumb.
    fn sliders(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
    ) {
        // Volume: 0..=1 over twenty-eight rows.
        let level = mini_volume_position(f32::from(state.volume) / 100.0);
        let (volume_response, volume_event) = self.slider(
            ctx,
            ui,
            frame,
            layout::VOLUME,
            level,
            sprites::volume_row(strip_row(level)),
            (sprites::VOLUME_THUMB, sprites::VOLUME_THUMB_DOWN),
            layout::VOLUME_TRAVEL,
            "volume",
            &format!("Volume {}%", state.volume),
        );
        match volume_event {
            MiniSliderEvent::Dragging(moved) => {
                self.player.set_volume(mini_volume_gain(moved));
            }
            MiniSliderEvent::Committed(moved) => {
                let gain = mini_volume_gain(moved);
                let percent = (gain * 100.0).round().clamp(0.0, 100.0) as u8;
                self.player.set_volume(gain);
                frame.commands.push(Command::Volume(percent));
            }
            MiniSliderEvent::None => {}
        }
        // The wheel over the volume moves it 5% a notch, as in the big window.
        let notches = slider_wheel_notches(ui, &volume_response);
        if notches != 0 {
            let percent = (i32::from(state.volume) + notches * 5).clamp(0, 100) as u8;
            self.player.set_volume(f32::from(percent) / 100.0);
            frame.commands.push(Command::Volume(percent));
        }

        // Balance: -1..=1, so the strip's row is the *distance* from centre —
        // Winamp's art brightens as the sound moves off centre, not as it
        // moves right.
        let balance = f32::from(state.balance) / 100.0;
        let (_, balance_event) = self.slider(
            ctx,
            ui,
            frame,
            layout::BALANCE,
            (balance + 1.0) / 2.0,
            sprites::balance_row(strip_row(balance.abs())),
            (sprites::BALANCE_THUMB, sprites::BALANCE_THUMB_DOWN),
            layout::BALANCE_TRAVEL,
            "balance",
            &balance_label(state.balance),
        );
        match balance_event {
            MiniSliderEvent::Dragging(moved) => {
                self.player.set_balance(balance_of(moved));
            }
            MiniSliderEvent::Committed(moved) => {
                let balance = balance_of(moved);
                let percent = (balance * 100.0).round().clamp(-100.0, 100.0) as i8;
                self.player.set_balance(balance);
                frame.commands.push(Command::Balance(percent));
            }
            MiniSliderEvent::None => {}
        }
    }

    /// One horizontal slider: the strip row for its level, then the thumb.
    /// Returns the new position, 0..=1, when the pointer moved it.
    #[allow(clippy::too_many_arguments)]
    fn slider(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &Frame,
        area: Area,
        position: f32,
        strip: Sprite,
        thumb: (Sprite, Sprite),
        travel: u32,
        salt: &str,
        tooltip: &str,
    ) -> (egui::Response, MiniSliderEvent) {
        self.blit_at(ctx, ui, frame, strip, area);
        let rect = frame.rect(area);
        let id = ui.id().with(("mini-slider", salt));
        let response = ui
            .interact(rect, id, egui::Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(tooltip);
        let thumb_sprite = if response.is_pointer_button_down_on() {
            thumb.1
        } else {
            thumb.0
        };
        let pointer = response.interact_pointer_pos().map(|pos| {
            let half_thumb = thumb_sprite.w as f32 / 2.0 * frame.scale;
            ((pos.x - rect.left() - half_thumb) / (travel as f32 * frame.scale)).clamp(0.0, 1.0)
        });
        let memory = id.with("drag-value");
        let dragging = ui.data(|data| data.get_temp::<f32>(memory));
        let mut event = MiniSliderEvent::None;
        if (response.drag_started() || response.dragged())
            && let Some(value) = pointer
        {
            ui.data_mut(|data| data.insert_temp(memory, value));
            event = MiniSliderEvent::Dragging(value);
        }
        if response.drag_stopped() {
            if let Some(value) = dragging.or(pointer) {
                event = MiniSliderEvent::Committed(value);
            }
            ui.data_mut(|data| data.remove::<f32>(memory));
        } else if response.clicked()
            && let Some(value) = pointer
        {
            event = MiniSliderEvent::Committed(value);
        }
        // Paint the pointer's current value immediately, without waiting for
        // the next player snapshot. Keep repainting during a paused drag too.
        if response.is_pointer_button_down_on() {
            ctx.request_repaint();
        }
        let position = event.value().unwrap_or(position);
        // Winamp's thumbs are 11 tall in a 13-tall track, centred.
        let y = area.y + (area.height - thumb_sprite.h) / 2;
        let x = area.x + (position.clamp(0.0, 1.0) * travel as f32).round() as u32;
        self.blit(ctx, ui, frame, thumb_sprite, (x, y));
        (response, event)
    }

    /// The EQ and playlist buttons, which open Winamp's other two windows. Both
    /// light up while their window is open, as Winamp's did.
    fn window_buttons(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, frame: &mut Frame) {
        let (eq, eq_down) = if frame.stack.is_open(Window::Equalizer) {
            (sprites::EQ_ON, sprites::EQ_ON_DOWN)
        } else {
            (sprites::EQ, sprites::EQ_DOWN)
        };
        if self.button(ctx, ui, frame, layout::EQ_BUTTON, eq, eq_down, "Equaliser") {
            frame
                .commands
                .push(Command::ToggleWindow(Window::Equalizer));
        }
        let (playlist, playlist_down) = if frame.stack.is_open(Window::Playlist) {
            (sprites::PLAYLIST_ON, sprites::PLAYLIST_ON_DOWN)
        } else {
            (sprites::PLAYLIST, sprites::PLAYLIST_DOWN)
        };
        if self.button(
            ctx,
            ui,
            frame,
            layout::PLAYLIST_BUTTON,
            playlist,
            playlist_down,
            "Playlist",
        ) {
            frame.commands.push(Command::ToggleWindow(Window::Playlist));
        }
        // The wordmark at the bottom right. Winamp's opened its about box;
        // ours says which skin is on and what build this is.
        if self
            .hit(ui, frame, layout::ABOUT, "about")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!("Skin: {}", self.skin.name))
            .clicked()
        {
            frame.commands.push(Command::About);
        }
    }

    /// The O A I D V strip. Winamp's five little lamps: options, album info,
    /// track info, doubled size, visualiser.
    fn clutter_bar(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        mode: crate::ui::visualiser::Mode,
    ) {
        // The strip comes from `titlebar.bmp`; a skin without one leaves
        // `main.bmp`'s own pixels showing there, which is what Winamp draws
        // over anyway.
        if self.skin.has(Sheet::Titlebar) {
            self.blit_at(ctx, ui, frame, sprites::CLUTTER_BAR, layout::CLUTTER_BAR);
        }
        // O is FastPotify's options menu, rather than another close button.
        let options = self
            .hit(ui, frame, layout::CLUTTER_O, "clutter-o")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Options");
        if options.is_pointer_button_down_on() {
            self.blit_at(ctx, ui, frame, sprites::CLUTTER_O_ON, layout::CLUTTER_O);
        }
        egui::Popup::menu(&options).show(|ui| {
            ui.set_min_width(132.0);
            ui.horizontal(|ui| {
                ui.label("Size");
                for scale in 1..=4 {
                    if ui
                        .selectable_label(self.scale == scale, format!("{scale}x"))
                        .clicked()
                    {
                        frame.commands.push(Command::SetScale(scale));
                        ui.close();
                    }
                }
            });
            let mut on_top = frame.on_top;
            if ui.checkbox(&mut on_top, "Always on top").clicked() {
                frame.commands.push(Command::ToggleOnTop);
            }
            if ui.button("Equalizer").clicked() {
                frame
                    .commands
                    .push(Command::ToggleWindow(Window::Equalizer));
                ui.close();
            }
            if ui.button("Playlist").clicked() {
                frame.commands.push(Command::ToggleWindow(Window::Playlist));
                ui.close();
            }
            if ui.button("Big window").clicked() {
                frame.commands.push(Command::Close);
                ui.close();
            }
        });

        for (area, lit, salt, tooltip, command) in [
            (
                layout::CLUTTER_A,
                sprites::CLUTTER_A_ON,
                "clutter-a",
                "Always on top",
                Command::ToggleOnTop,
            ),
            (
                layout::CLUTTER_I,
                sprites::CLUTTER_I_ON,
                "clutter-i",
                "Song info",
                Command::TrackInfoCurrent,
            ),
            (
                layout::CLUTTER_D,
                sprites::CLUTTER_D_ON,
                "clutter-d",
                "Size: 2x, 3x, 4x",
                Command::CycleScale,
            ),
            (
                layout::CLUTTER_V,
                sprites::CLUTTER_V_ON,
                "clutter-v",
                "Visualiser",
                Command::CycleVisualiser,
            ),
        ] {
            let response = self
                .hit(ui, frame, area, salt)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(tooltip);
            let latched = match command {
                Command::ToggleOnTop => frame.on_top,
                Command::CycleScale => self.scale >= 2,
                Command::CycleVisualiser => mode != crate::ui::visualiser::Mode::Off,
                _ => false,
            };
            if response.is_pointer_button_down_on() || latched {
                self.blit_at(ctx, ui, frame, lit, area);
            }
            if response.clicked() {
                frame.commands.push(command);
            }
        }
    }

    /// The play/pause/stop mark, and the sliver that lights while loading.
    fn status(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
    ) {
        if state.loading {
            // Winamp showed the work indicator *instead of* the mark.
            self.blit_at(ctx, ui, frame, sprites::WORKING, layout::WORK_INDICATOR);
            return;
        }
        let mark = if state.stopped {
            sprites::STOPPED
        } else if state.playing {
            sprites::PLAYING
        } else {
            sprites::PAUSED
        };
        self.blit_at(ctx, ui, frame, mark, layout::STATUS);
    }

    /// Mono/stereo lamps and switches, matching FastPotify's player.
    fn mono_stereo(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
    ) {
        let (mono, stereo) = match (state.channels, state.mono) {
            (0, false) => (sprites::MONO, sprites::STEREO),
            (_, true) | (1, false) => (sprites::MONO_ON, sprites::STEREO),
            _ => (sprites::MONO, sprites::STEREO_ON),
        };
        self.blit_at(ctx, ui, frame, mono, layout::MONO);
        self.blit_at(ctx, ui, frame, stereo, layout::STEREO);
        if self
            .hit(ui, frame, layout::MONO, "mono")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Play in mono")
            .clicked()
            && !state.mono
        {
            frame.commands.push(Command::SetMono(true));
        }
        if self
            .hit(ui, frame, layout::STEREO, "stereo")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Play in stereo")
            .clicked()
            && state.mono
        {
            frame.commands.push(Command::SetMono(false));
        }
    }

    /// The bitrate and sample rate, in the pixel font, as Winamp showed them.
    fn media_info(&self, ui: &egui::Ui, frame: &Frame, state: &Snapshot) {
        if state.stopped {
            return;
        }
        let ink = self.skin.vis_colors[2];
        if state.bitrate_kbps > 0 {
            self.pixels(
                ui,
                frame,
                layout::KBPS,
                &state.bitrate_kbps.to_string(),
                ink,
            );
        }
        if state.sample_rate > 0 {
            // Winamp's field is two characters wide: kHz, rounded.
            let khz = (state.sample_rate / 1000).min(99);
            self.pixels(ui, frame, layout::KHZ, &khz.to_string(), ink);
        }
    }

    /// Transport straight on the player — no round trip through the app.
    fn press(&self, press: Press) {
        match press {
            // Winamp's play button restarts the track from the top; ours
            // resumes a pause, because that is what a play button means in
            // every other player the user has open.
            Press::Play => self.player.play(),
            Press::Pause => self.player.play_pause(),
            Press::PlayPause => self.player.play_pause(),
            Press::Stop => self.player.stop(),
            Press::Next => {
                self.player.next();
            }
            Press::Previous => {
                self.player.prev();
            }
            Press::ToggleShuffle => self.player.toggle_shuffle(),
            Press::CycleRepeat => self.player.cycle_repeat(),
        }
    }

    /// An invisible click target over one area. Used where the art is painted
    /// into the background and only the hit region is ours.
    fn hit(&self, ui: &mut egui::Ui, frame: &Frame, area: Area, salt: &str) -> egui::Response {
        ui.interact(
            frame.rect(area),
            ui.id().with(("mini-hit", salt, area.x, area.y)),
            egui::Sense::click(),
        )
    }

    /// Paint a sprite at a skin-pixel position, at its own size.
    fn blit(
        &mut self,
        ctx: &egui::Context,
        ui: &egui::Ui,
        frame: &Frame,
        sprite: Sprite,
        at: (u32, u32),
    ) {
        let Some(id) = self.texture(ctx, sprite) else {
            return;
        };
        let rect = egui::Rect::from_min_size(
            frame.at(at.0, at.1),
            egui::vec2(sprite.w as f32 * frame.scale, sprite.h as f32 * frame.scale),
        );
        ui.painter().image(
            id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }

    /// Paint a sprite into the area the layout gives it.
    fn blit_at(
        &mut self,
        ctx: &egui::Context,
        ui: &egui::Ui,
        frame: &Frame,
        sprite: Sprite,
        area: Area,
    ) {
        self.blit(ctx, ui, frame, sprite, (area.x, area.y));
    }

    /// A two-state button in its layout slot. Returns true when it was
    /// clicked.
    #[allow(clippy::too_many_arguments)]
    fn button(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &Frame,
        area: Area,
        up: Sprite,
        down: Sprite,
        tooltip: &str,
    ) -> bool {
        let rect = frame.rect(area);
        let response = ui.interact(
            rect,
            ui.id().with((
                "mini-button",
                rect.min.x.to_bits(),
                rect.min.y.to_bits(),
                area.x,
                area.y,
            )),
            egui::Sense::click(),
        );
        let sprite = if response.is_pointer_button_down_on() {
            down
        } else {
            up
        };
        self.blit_at(ctx, ui, frame, sprite, area);
        let response = if tooltip.is_empty() {
            response
        } else {
            response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(tooltip)
        };
        response.clicked()
    }

    /// The title bar: its three buttons, and the drag that moves the window.
    fn title_bar(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        shade: bool,
    ) {
        // Dragging moves the window; there is no OS frame to grab. Registered
        // before the buttons so their own rects win the pixels they cover.
        let drag = ui.interact(
            frame.rect(layout::TITLE_BAR),
            ui.id().with("mini-titlebar"),
            egui::Sense::click_and_drag(),
        );
        if drag.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        // Double-clicking the bar rolls the window up, as Winamp's did. Closing
        // on a double click (the first cut) meant a mis-aimed drag threw the
        // skin away.
        if drag.double_clicked() {
            frame.commands.push(Command::ToggleShade(Window::Main));
        }

        // The Winamp logo opens the menu; ours is the app's own interface.
        let art = self.skin.has(Sheet::Titlebar);
        let options = if art {
            self.button(
                ctx,
                ui,
                frame,
                layout::OPTIONS_BUTTON,
                sprites::OPTIONS,
                sprites::OPTIONS_DOWN,
                "Back to Fastcloud",
            )
        } else {
            self.hit(ui, frame, layout::OPTIONS_BUTTON, "options")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Back to Fastcloud")
                .clicked()
        };
        if options {
            frame.commands.push(Command::Close);
        }

        // A skin with no `titlebar.bmp` has no art for these three either, so
        // they stay hit-only rather than painting somebody else's sprites.
        for (area, up, down, salt, tooltip, command) in [
            (
                layout::MINIMIZE_BUTTON,
                sprites::MINIMIZE,
                sprites::MINIMIZE_DOWN,
                "minimize",
                "Minimize",
                Command::Minimize,
            ),
            (
                layout::SHADE_BUTTON,
                if shade {
                    sprites::UNSHADE
                } else {
                    sprites::SHADE
                },
                if shade {
                    sprites::UNSHADE_DOWN
                } else {
                    sprites::SHADE_DOWN
                },
                "shade",
                if shade { "Unroll" } else { "Roll up" },
                Command::ToggleShade(Window::Main),
            ),
            (
                layout::CLOSE_BUTTON,
                sprites::CLOSE,
                sprites::CLOSE_DOWN,
                "close",
                "Back to Fastcloud",
                Command::Close,
            ),
        ] {
            let clicked = if art {
                self.button(ctx, ui, frame, area, up, down, tooltip)
            } else {
                self.hit(ui, frame, area, salt)
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(tooltip)
                    .clicked()
            };
            if clicked {
                frame.commands.push(command);
            }
        }
    }

    /// The four-digit time in `numbers.bmp`'s cells, and the click that turns
    /// it into a countdown.
    ///
    /// A stopped player shows the blank cell in all four positions: Winamp's
    /// display went dark rather than sitting at `00:00`.
    fn time(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
        cells: [Area; 4],
        minus: Area,
    ) {
        // A click on the digits counts down instead of up, as Winamp's did.
        let box_area = Area::new(
            minus.x,
            cells[0].y,
            cells[3].right() - minus.x,
            cells[0].height,
        );
        if self
            .hit(ui, frame, box_area, "time")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(if self.time_remaining {
                "Time remaining — click for elapsed"
            } else {
                "Time elapsed — click for remaining"
            })
            .clicked()
        {
            self.time_remaining = !self.time_remaining;
        }

        let shown_ms = self.shown_ms(state);
        let digits = clock_digits(shown_ms);
        // `nums_ex.bmp` gives the minus a cell of its own; without it Winamp
        // borrows five pixels out of `numbers.bmp`.
        let ex = self.skin.has(Sheet::NumsEx);
        if self.time_remaining && !state.stopped && state.duration_ms > 0 {
            if ex {
                self.blit_at(ctx, ui, frame, sprites::MINUS_EX, layout::MINUS_EX);
            } else {
                self.blit_at(ctx, ui, frame, sprites::MINUS, layout::MINUS);
            }
        }
        for (cell, digit) in cells.iter().zip(digits) {
            // Winamp leaves a leading zero blank, and blanks the lot when
            // there is nothing loaded.
            let index = if state.stopped {
                sprites::BLANK_DIGIT
            } else {
                digit
            };
            let sprite = if ex {
                sprites::digit_ex(index)
            } else {
                sprites::digit(index)
            };
            self.blit_at(ctx, ui, frame, sprite, *cell);
        }
    }

    /// Shade mode's time: five pixel-font cells instead of digit art, because
    /// the rolled-up bar has no room for 13-pixel numbers.
    fn shade_time(&mut self, ui: &mut egui::Ui, frame: &mut Frame, state: &Snapshot) {
        if self
            .hit(ui, frame, layout::SHADE_TIME, "shade-time")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.time_remaining = !self.time_remaining;
        }
        if state.stopped {
            return;
        }
        let shown_ms = self.shown_ms(state);
        let digits = clock_digits(shown_ms);
        let sign = if self.time_remaining && state.duration_ms > 0 {
            '-'
        } else {
            ' '
        };
        let text: Vec<char> = [
            sign,
            // A leading zero is blank here too.
            if digits[0] == 0 {
                ' '
            } else {
                char::from(b'0' + digits[0] as u8)
            },
            char::from(b'0' + digits[1] as u8),
            char::from(b'0' + digits[2] as u8),
            char::from(b'0' + digits[3] as u8),
        ]
        .to_vec();
        let ink = self.skin.vis_colors[2];
        // Winamp spaces these five by hand, so each is placed on its own.
        for (offset, character) in layout::SHADE_TIME_CELLS.iter().zip(text) {
            let cell = Area::new(
                layout::SHADE_TIME.x + offset,
                layout::SHADE_TIME.y,
                pixel_text::CHAR_W,
                pixel_text::CHAR_H,
            );
            self.pixels(ui, frame, cell, &character.to_string(), ink);
        }
    }

    /// The time the display should show: the seek preview while one is being
    /// dragged (so the digits agree with the thumb), counting down when asked.
    fn shown_ms(&self, state: &Snapshot) -> u64 {
        let elapsed = match self.seek_preview {
            Some(f) => (f as f64 * state.duration_ms as f64) as u64,
            None => state.position_ms,
        };
        if self.time_remaining && state.duration_ms > 0 {
            state.duration_ms.saturating_sub(elapsed)
        } else {
            elapsed
        }
    }

    /// The scrolling title in its box, in whichever mode's marquee.
    fn marquee(&self, ui: &egui::Ui, frame: &Frame, area: Area, text: &str, tick: usize) {
        let chars = (area.width / pixel_text::CHAR_W) as usize;
        let shown = pixel_text::marquee(text, chars, tick);
        self.pixels(ui, frame, area, &shown, self.skin.vis_colors[2]);
    }

    /// Pixel-font text in one box, as a single mesh.
    ///
    /// Clipped to the box: a proportional glyph can be one pixel wider than
    /// its cell, and the last one must not bleed past the display well.
    fn pixels(&self, ui: &egui::Ui, frame: &Frame, area: Area, text: &str, ink: egui::Color32) {
        let (cw, ch) = (pixel_text::CHAR_W, pixel_text::CHAR_H);
        let box_rect = frame.rect(area);
        // One mesh for the whole line: ~900 lit pixels as separate rects were
        // 900 tessellated shapes a frame, which is what made the window lag.
        let mut mesh = egui::Mesh::default();
        for (index, character) in text.chars().enumerate() {
            // Winamp's cells carry their own bearing, so the step is the cell
            // width exactly — a wider step walks the line out of the display.
            let cell_x = area.x + index as u32 * cw;
            if cell_x + cw > area.right() {
                break;
            }
            for y in 0..ch {
                // Runs of lit pixels become one quad each.
                let mut run: Option<u32> = None;
                for x in 0..=cw {
                    let lit = x < cw
                        && self
                            .skin
                            .sheet(Sheet::Text)
                            .and_then(|font| crate::skin::classic::glyph_lit(font, character, x, y))
                            .unwrap_or_else(|| pixel_text::lit(character, x, y));
                    match (lit, run) {
                        (true, None) => run = Some(x),
                        (false, Some(start)) => {
                            mesh.add_colored_rect(
                                egui::Rect::from_min_size(
                                    frame.at(cell_x + start, area.y + y),
                                    egui::vec2((x - start) as f32 * frame.scale, frame.scale),
                                ),
                                ink,
                            );
                            run = None;
                        }
                        _ => {}
                    }
                }
            }
        }
        if !mesh.is_empty() {
            ui.painter_at(box_rect).add(egui::Shape::mesh(mesh));
        }
    }

    /// The analyser or the oscilloscope, in the skin's own colours, as one
    /// mesh. Clicking it cycles spectrum → scope → off, as Winamp's V did.
    fn visualiser(
        &self,
        ui: &mut egui::Ui,
        frame: &mut Frame,
        state: &Snapshot,
        mode: crate::ui::visualiser::Mode,
    ) {
        if self
            .hit(ui, frame, layout::VISUALIZER, "visualiser")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(mode.label())
            .clicked()
        {
            frame.commands.push(Command::CycleVisualiser);
        }
        self.visualiser_in(ui, frame, state, mode, layout::VISUALIZER);
    }

    /// The trace itself, in whichever well it is asked for: the main window's
    /// 76×16 hole, or the playlist's 72-pixel strip.
    fn visualiser_in(
        &self,
        ui: &egui::Ui,
        frame: &Frame,
        state: &Snapshot,
        mode: crate::ui::visualiser::Mode,
        area: Area,
    ) {
        use crate::ui::visualiser::Mode;
        if mode == Mode::Off {
            return;
        }
        let (vw, vh) = (area.width, area.height);
        let mut mesh = egui::Mesh::default();
        let mut quad = |x: u32, y: u32, w: u32, h: u32, colour: egui::Color32| {
            mesh.add_colored_rect(
                egui::Rect::from_min_size(
                    frame.at(area.x + x, area.y + y),
                    egui::vec2(w as f32 * frame.scale, h as f32 * frame.scale),
                ),
                colour,
            );
        };
        if mode == Mode::Scope {
            // One column a sample, shaded by distance from the centre.
            for (column, row) in state.scope.iter().enumerate().take(vw as usize) {
                let shade = vis::scope_shade(*row);
                let colour = self.skin.vis_colors[18 + shade.min(4)];
                quad(column as u32, u32::from(*row).min(vh - 1), 1, 1, colour);
            }
        } else {
            // Nineteen bars, three columns wide with one between. Rows of the
            // same colour are one quad, so a full-height bar is sixteen
            // quads rather than forty-eight.
            for (i, bar) in state.bars.iter().enumerate() {
                let x0 = i as u32 * 4;
                for row in 0..u32::from(bar.height).min(vh) {
                    let y = vh - 1 - row;
                    // Winamp's ramp is bottom-to-top over sixteen colours.
                    let colour = self.skin.vis_colors[2 + (row as usize).min(15)];
                    quad(x0, y, 3, 1, colour);
                }
                if let Some(peak) = bar.peak {
                    let y = vh.saturating_sub(u32::from(peak)).min(vh - 1);
                    quad(x0, y, 3, 1, self.skin.vis_colors[23]);
                }
            }
        }
        if !mesh.is_empty() {
            ui.painter().add(egui::Shape::mesh(mesh));
        }
    }
}

/// A tapered curve gives quiet listening most of the mini slider's travel.
pub(super) fn mini_volume_gain(position: f32) -> f32 {
    position.clamp(0.0, 1.0).powi(2)
}

pub(super) fn mini_volume_position(gain: f32) -> f32 {
    gain.clamp(0.0, 1.0).sqrt()
}

#[derive(Clone, Copy)]
enum MiniSliderEvent {
    None,
    Dragging(f32),
    Committed(f32),
}

impl MiniSliderEvent {
    fn value(self) -> Option<f32> {
        match self {
            Self::None => None,
            Self::Dragging(value) | Self::Committed(value) => Some(value),
        }
    }
}

/// Slider position as stereo balance, with FastPotify/Winamp centre detent.
fn balance_of(position: f32) -> f32 {
    let balance = position.clamp(0.0, 1.0) * 2.0 - 1.0;
    if balance.abs() < 0.08 { 0.0 } else { balance }
}

/// One frame's geometry and the commands it produced.
///
/// The origin and the scale are the only two things every painter needs, and
/// passing them as one value is what let the drawing code take layout areas
/// rather than loose pixel pairs. Each window gets its own, offset to its top,
/// so a renderer works in its own coordinates.
struct Frame {
    origin: egui::Pos2,
    scale: f32,
    commands: Vec<Command>,
    /// Whether the window is pinned above the others, which lights a lamp.
    on_top: bool,
    /// Which windows are open, which lights the EQ and PL buttons.
    stack: Stack,
    /// The equaliser's settings, which its own sliders read.
    eq: EqSettings,
    /// Whether the current track has a saved preset.
    has_preset: bool,
    /// Whether the OS window has focus, which dims the title bars.
    focused: bool,
}

impl Frame {
    /// The same frame, shifted down to a window's top edge.
    fn offset(&self, top: u32) -> Self {
        Self {
            origin: self.origin + egui::vec2(0.0, top as f32 * self.scale),
            scale: self.scale,
            commands: Vec::new(),
            on_top: self.on_top,
            stack: self.stack,
            eq: self.eq,
            has_preset: self.has_preset,
            focused: self.focused,
        }
    }

    /// A skin-pixel position on screen.
    fn at(&self, x: u32, y: u32) -> egui::Pos2 {
        self.origin + egui::vec2(x as f32 * self.scale, y as f32 * self.scale)
    }

    /// An area's rectangle on screen.
    fn rect(&self, area: Area) -> egui::Rect {
        egui::Rect::from_min_size(
            self.at(area.x, area.y),
            egui::vec2(
                area.width as f32 * self.scale,
                area.height as f32 * self.scale,
            ),
        )
    }
}

/// A transport press, applied straight to the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Press {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    ToggleShuffle,
    CycleRepeat,
}

/// One frame's worth of player state, read once so no lock is held while
/// painting.
struct Snapshot {
    title: String,
    /// The queue, one row per entry, for the playlist window.
    queue: Vec<Entry>,
    /// Which queue entry is playing.
    current: Option<usize>,
    position_ms: u64,
    duration_ms: u64,
    playing: bool,
    stopped: bool,
    /// A track is being fetched, which lights the work indicator.
    loading: bool,
    shuffle: bool,
    repeat_on: bool,
    volume: u8,
    /// -100 hard left … 100 hard right.
    balance: i8,
    mono: bool,
    sample_rate: u32,
    bitrate_kbps: u32,
    /// Source channels: 1 lights mono, 2+ lights stereo, 0 lights neither.
    channels: u16,
    tick: usize,
    bars: [vis::Bar; vis::BARS],
    scope: [u8; vis::COLUMNS],
    /// Whether anything on screen still has to move.
    wants_frames: bool,
}

/// One row of the playlist: what it draws, and nothing more.
struct Entry {
    title: String,
    duration_ms: u64,
}

/// Which row of a 28-row strip shows a 0..=1 level.
fn strip_row(level: f32) -> u32 {
    let rows = sprites::VOLUME_ROWS - 1;
    ((level.clamp(0.0, 1.0) * rows as f32).round() as u32).min(rows)
}

/// `mm:ss` as four digits, minutes capped at 99 — the display has two cells.
fn clock_digits(position_ms: u64) -> [u32; 4] {
    let total = position_ms / 1000;
    let minutes = (total / 60).min(99);
    let seconds = total % 60;
    [
        (minutes / 10) as u32,
        (minutes % 10) as u32,
        (seconds / 10) as u32,
        (seconds % 10) as u32,
    ]
}

/// What the balance slider says on hover.
fn balance_label(balance: i8) -> String {
    match balance {
        0 => "Balance: centre".to_owned(),
        b if b < 0 => format!("Balance: {}% left", -i32::from(b)),
        b => format!("Balance: {b}% right"),
    }
}

/// Wheel notches from this frame's raw wheel events (see
/// [`crate::ui::player_bar`], which shares the rule).
fn wheel_notches(input: &egui::InputState) -> f32 {
    input
        .events
        .iter()
        .filter_map(|event| match event {
            egui::Event::MouseWheel { unit, delta, .. } => {
                Some(super::player_bar::notches_of(*unit, delta.y))
            }
            _ => None,
        })
        .sum()
}

/// Whole wheel notches over one mini slider. Trackpad point deltas accumulate
/// instead of turning every tiny vertical movement into a full 5% jump.
fn slider_wheel_notches(ui: &egui::Ui, response: &egui::Response) -> i32 {
    const NOTCH: f32 = 50.0;
    if !response.hovered() {
        return 0;
    }
    let (lines, points) = ui.input(|input| {
        let mut lines = 0.0f32;
        let mut points = 0.0f32;
        for event in &input.events {
            if let egui::Event::MouseWheel { unit, delta, .. } = event {
                match unit {
                    egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => {
                        lines += if delta.y.abs() >= 1.0 {
                            delta.y.signum()
                        } else {
                            delta.y
                        };
                    }
                    egui::MouseWheelUnit::Point => points += delta.y,
                }
            }
        }
        (lines, points)
    });
    let id = response.id.with("wheel");
    let total = ui.data(|data| data.get_temp::<f32>(id)).unwrap_or(0.0) + points + lines * NOTCH;
    let notches = (total / NOTCH).trunc();
    ui.data_mut(|data| data.insert_temp(id, total - notches * NOTCH));
    notches as i32
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A player with no audio device, enough to drive the window in tests.
    fn player() -> Arc<Player> {
        let output = Arc::new(crate::audio::output::AudioOutput::silent([0.0; 10], 0.8));
        let client = Arc::new(crate::api::ApiClient::demo());
        let cache = Arc::new(crate::audio::cache::AudioCache::disabled());
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        let handle = rt.handle().clone();
        // The handle must outlive the runtime's guard for the window's sake;
        // tests only read state, so leaking one runtime is fine.
        std::mem::forget(rt);
        Arc::new(Player::new(
            output,
            client,
            cache,
            std::env::temp_dir().join("fastcloud-test-settings.json"),
            handle,
        ))
    }

    pub(super) fn mini(scale: u32) -> MiniPlayer {
        MiniPlayer::new(crate::skin::stock::stock(), scale, player())
    }

    #[test]
    fn the_window_is_the_classic_size() {
        assert_eq!(WIDTH, 275.0);
        assert_eq!(HEIGHT, 116.0);
        assert_eq!(SHADE_HEIGHT, 14.0, "the rolled-up window is its title bar");
    }

    #[test]
    fn the_scale_is_clamped_to_whole_pixels() {
        assert_eq!(mini(0).scale, 1, "a zero scale would draw nothing");
        assert_eq!(
            mini(9).scale,
            4,
            "past 4× the window is bigger than the app"
        );
        assert_eq!(mini(3).scale, 3);
    }

    #[test]
    fn mini_volume_reserves_travel_for_quiet_levels() {
        assert_eq!(mini_volume_gain(0.0), 0.0);
        assert_eq!(mini_volume_gain(1.0), 1.0);
        assert!((mini_volume_gain(0.25) - 0.0625).abs() < f32::EPSILON);
        let gain = 0.08;
        assert!((mini_volume_gain(mini_volume_position(gain)) - gain).abs() < 1e-6);
    }

    #[test]
    fn balance_has_winamp_centre_detent_and_full_channel_range() {
        assert_eq!(balance_of(0.0), -1.0);
        assert_eq!(balance_of(0.5), 0.0);
        assert_eq!(balance_of(0.53), 0.0);
        assert!(balance_of(0.55) > 0.0);
        assert_eq!(balance_of(1.0), 1.0);
    }

    #[test]
    fn a_new_skin_drops_the_old_textures() {
        let mut mini = mini(2);
        assert_eq!(mini.skin_name(), "Fastcloud");
        mini.set_skin(crate::skin::stock::stock());
        assert!(mini.textures.is_empty());
    }

    #[test]
    fn commands_queue_for_the_app_thread() {
        let mini = mini(1);
        {
            let mut shared = mini.shared.lock().unwrap();
            shared.commands.push(Command::Close);
            shared.commands.push(Command::Volume(40));
        }
        let drained: Vec<Command> = std::mem::take(&mut mini.shared.lock().unwrap().commands);
        assert_eq!(drained, vec![Command::Close, Command::Volume(40)]);
        assert!(mini.shared.lock().unwrap().commands.is_empty());
    }

    /// The stack's height *is* the mode, and that is what the app resizes to.
    #[test]
    fn rolling_up_changes_the_height_only() {
        let mini = mini(2);
        assert!(!mini.shade());
        assert_eq!(mini.height(), HEIGHT);
        mini.shared
            .lock()
            .unwrap()
            .stack
            .set_shade(Window::Main, true);
        assert!(mini.shade());
        assert_eq!(mini.height(), SHADE_HEIGHT);
    }

    /// Nothing playing, nothing moving: the window must not ask for frames,
    /// or an idle mini player would spin the GPU at 60 Hz forever.
    #[test]
    fn an_idle_window_asks_for_no_frames() {
        let ctx = egui::Context::default();
        let mut mini = mini(1);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let state = {
                // `snapshot` needs the context the window runs in.
                let ctx = ui.ctx().clone();
                mini.snapshot(&ctx, crate::ui::visualiser::Mode::Spectrum)
            };
            assert!(!state.playing);
            assert!(state.stopped);
            assert_eq!(state.duration_ms, 0);
            assert!(state.bars.iter().all(|b| b.height == 0));
            assert!(state.scope.iter().all(|row| *row == 7));
            assert!(
                !state.wants_frames,
                "an idle window must not schedule repaints"
            );
            // The idle title fits both marquees, so it does not scroll — and
            // the rolled-up one is the shorter of the two.
            assert!("Fastcloud".chars().count() <= layout::SHADE_MARQUEE_CHARS);
            assert!(layout::SHADE_MARQUEE_CHARS < layout::MARQUEE.cells());
        });
        output.textures_delta.clear();
    }

    /// The oscilloscope animates continuously, so it always wants frames.
    #[test]
    fn the_scope_always_wants_frames() {
        let ctx = egui::Context::default();
        let mut mini = mini(1);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let ctx = ui.ctx().clone();
            let state = mini.snapshot(&ctx, crate::ui::visualiser::Mode::Scope);
            assert!(state.wants_frames);
            let off = mini.snapshot(&ctx, crate::ui::visualiser::Mode::Off);
            assert!(!off.wants_frames);
        });
        output.textures_delta.clear();
    }

    /// A whole frame must paint without panicking at every scale, in every
    /// combination of windows and shade states, and the mesh path must not
    /// explode the shape count.
    #[test]
    fn a_frame_paints_at_every_scale() {
        for scale in 1..=4 {
            for shade in [false, true] {
                let ctx = egui::Context::default();
                let mut mini = mini(scale);
                {
                    let mut shared = mini.shared.lock().unwrap();
                    shared.stack.set_shade(Window::Main, shade);
                    // At 4× the three windows are 1100 px wide and taller than
                    // most screens; the point here is that they all paint.
                    shared.stack.equalizer = true;
                    shared.stack.playlist = true;
                }
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    mini.ui(ui);
                });
                output.textures_delta.clear();
            }
        }
    }

    /// Every window, open and rolled up, in every combination. This is the
    /// grid the stack's own tests cover geometrically; here it has to paint.
    #[test]
    fn every_window_combination_paints() {
        for equalizer in [false, true] {
            for playlist in [false, true] {
                for shade in [[false; 3], [true; 3], [false, true, false]] {
                    let ctx = egui::Context::default();
                    let mut mini = mini(1);
                    {
                        let mut shared = mini.shared.lock().unwrap();
                        shared.stack.equalizer = equalizer;
                        shared.stack.playlist = playlist;
                        shared.stack.shade = shade;
                    }
                    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                        mini.ui(ui);
                    });
                    output.textures_delta.clear();
                }
            }
        }
    }

    /// Every state of the display must paint: playing, paused, loading, a
    /// countdown, a moved balance, a raised equaliser and a queue with rows in
    /// it. These are the branches the two `time` paths, the sliders, the graph
    /// and the track list take.
    #[test]
    fn a_frame_paints_in_every_state() {
        for (playing, loading, remaining, shade) in [
            (true, false, false, false),
            (false, false, true, false),
            (false, true, false, false),
            (true, false, true, true),
        ] {
            let ctx = egui::Context::default();
            let mut mini = mini(2);
            mini.time_remaining = remaining;
            {
                let mut shared = mini.shared.lock().unwrap();
                shared.stack.set_shade(Window::Main, shade);
                shared.stack.equalizer = true;
                shared.stack.playlist = true;
                shared.eq.enabled = true;
                shared.eq.preamp_db = 3.0;
                shared.eq.gains_db = [6.0, -6.0, 0.0, 12.0, -12.0, 3.0, 0.0, -3.0, 9.0, 0.0];
                shared.has_preset = true;
            }
            // More tracks than the playlist shows, so the scroll bar draws.
            mini.playlist.selected = [1, 2].into_iter().collect();
            {
                let mut st = mini.player.state.lock();
                st.queue = crate::demo::demo_tracks();
                st.order = (0..st.queue.len()).collect();
                st.current = Some(0);
                st.is_playing = playing;
                st.loading = loading;
                st.position_ms = 61_000;
                st.duration_ms = 245_000;
                st.balance = -0.5;
                st.sample_rate = 44_100;
                st.bitrate_kbps = 128;
            }
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                mini.ui(ui);
            });
            output.textures_delta.clear();
        }
    }

    /// The clock caps minutes at the two cells it has, rather than spilling a
    /// three-digit count into the seconds.
    #[test]
    fn the_clock_fits_its_four_cells() {
        assert_eq!(clock_digits(0), [0, 0, 0, 0]);
        assert_eq!(clock_digits(61_000), [0, 1, 0, 1]);
        assert_eq!(clock_digits(245_000), [0, 4, 0, 5]);
        assert_eq!(clock_digits(3_599_000), [5, 9, 5, 9]);
        // Two hours: 120 minutes would need three cells, so it pins at 99:59.
        assert_eq!(clock_digits(7_200_000), [9, 9, 0, 0]);
        for digits in [clock_digits(u64::MAX / 2), clock_digits(86_400_000)] {
            assert!(digits.iter().all(|d| *d < 10), "a cell holds one digit");
        }
    }

    /// The strip row is an index into twenty-eight rows of art, never past
    /// the end — a row past it would crop blank.
    #[test]
    fn the_strip_row_stays_on_the_sheet() {
        assert_eq!(strip_row(0.0), 0);
        assert_eq!(strip_row(1.0), sprites::VOLUME_ROWS - 1);
        assert_eq!(strip_row(2.0), sprites::VOLUME_ROWS - 1, "clamped");
        assert_eq!(strip_row(-1.0), 0, "clamped");
        assert_eq!(strip_row(0.5), 14);
    }

    #[test]
    fn the_balance_label_says_which_way() {
        assert_eq!(balance_label(0), "Balance: centre");
        assert_eq!(balance_label(-100), "Balance: 100% left");
        assert_eq!(balance_label(60), "Balance: 60% right");
    }
}
