//! Classic Winamp 2 skins: the sprite sheets, the two colour files, and the
//! sprite rectangles the classic layout is built from.
//!
//! A `.wsz` is a zip ([`zip::Archive`]) of BMPs. Everything the mini player
//! draws comes from fixed rectangles inside those sheets — Winamp had no
//! layout description, only "the play button is at (23, 0) in cbuttons.bmp",
//! so the coordinates below *are* the format. They match Winamp 2.9 and are
//! cross-checked against Webamp's `skinSprites.ts`.
//!
//! Two text files carry colour: `viscolor.txt` (24 visualiser colours, one
//! `r,g,b` a line) and `pledit.txt` (the playlist's ini-style palette). Both
//! are optional; sensible defaults stand in.

pub mod classic;
pub mod layout;
pub mod pixel_text;
pub mod stock;
pub mod zip;

use anyhow::{Context as _, Result};
use eframe::egui;
use std::collections::HashMap;

/// The main window's size in skin pixels. Every skin is this big.
pub const MAIN_WIDTH: u32 = layout::WINDOW_WIDTH;
pub const MAIN_HEIGHT: u32 = layout::WINDOW_HEIGHT;

/// A rectangle inside one sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sprite {
    pub sheet: Sheet,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Sprite {
    const fn new(sheet: Sheet, x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { sheet, x, y, w, h }
    }
}

/// The sheets the mini player needs. A skin may omit any of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sheet {
    Main,
    CButtons,
    Titlebar,
    Shufrep,
    Posbar,
    Volume,
    Balance,
    Monoster,
    Numbers,
    /// `nums_ex.bmp`: the same digits plus a minus sign in its own cell.
    /// Later skins ship it; [`Sheet::Numbers`] is the fallback.
    NumsEx,
    Text,
    Playpaus,
    /// `eqmain.bmp`: the equaliser window.
    EqMain,
    /// `eq_ex.bmp`: the equaliser rolled up. Optional, like `nums_ex.bmp`.
    EqEx,
    /// `pledit.bmp`: the playlist window's frame and buttons.
    Pledit,
}

impl Sheet {
    /// The file inside the archive, lowercased.
    pub fn file(self) -> &'static str {
        match self {
            Self::Main => "main.bmp",
            Self::CButtons => "cbuttons.bmp",
            Self::Titlebar => "titlebar.bmp",
            Self::Shufrep => "shufrep.bmp",
            Self::Posbar => "posbar.bmp",
            Self::Volume => "volume.bmp",
            Self::Balance => "balance.bmp",
            Self::Monoster => "monoster.bmp",
            Self::Numbers => "numbers.bmp",
            Self::NumsEx => "nums_ex.bmp",
            Self::Text => "text.bmp",
            Self::Playpaus => "playpaus.bmp",
            Self::EqMain => "eqmain.bmp",
            Self::EqEx => "eq_ex.bmp",
            Self::Pledit => "pledit.bmp",
        }
    }

    pub const ALL: [Self; 15] = [
        Self::Main,
        Self::CButtons,
        Self::Titlebar,
        Self::Shufrep,
        Self::Posbar,
        Self::Volume,
        Self::Balance,
        Self::Monoster,
        Self::Numbers,
        Self::NumsEx,
        Self::Text,
        Self::Playpaus,
        Self::EqMain,
        Self::EqEx,
        Self::Pledit,
    ];
}

/// The rectangle each piece occupies **inside its sheet**. Winamp's own
/// coordinates, cross-checked against Webamp's `skinSprites.ts`.
///
/// Where a piece goes *in the window* is [`layout`]'s business, not
/// this module's: one source of truth per question.
pub mod sprites {
    use super::{Sheet, Sprite};

    /// The window's background, the whole of `main.bmp`.
    pub const MAIN: Sprite = Sprite::new(Sheet::Main, 0, 0, 275, 116);
    /// Its top strip, which stands in for the shade bar in a skin that ships
    /// no `titlebar.bmp`.
    pub const MAIN_TOP: Sprite = Sprite::new(Sheet::Main, 0, 0, 275, super::layout::SHADE_HEIGHT);

    // ===== titlebar.bmp =====

    /// The title bar, focused and not. Winamp dimmed it when the window lost
    /// focus; a frameless window that never dims looks stuck on top of
    /// everything, so both are drawn.
    pub const TITLE_BAR: Sprite = Sprite::new(Sheet::Titlebar, 27, 0, 275, 14);
    pub const TITLE_BAR_DIM: Sprite = Sprite::new(Sheet::Titlebar, 27, 15, 275, 14);
    /// The whole window when it is rolled up to the title bar.
    pub const SHADE_BAR: Sprite = Sprite::new(Sheet::Titlebar, 27, 29, 275, 14);
    pub const SHADE_BAR_DIM: Sprite = Sprite::new(Sheet::Titlebar, 27, 42, 275, 14);

    /// The Winamp logo, which opens its menu. Ours goes back to the app, and
    /// the pressed sprite is what says so.
    pub const OPTIONS: Sprite = Sprite::new(Sheet::Titlebar, 0, 0, 9, 9);
    pub const OPTIONS_DOWN: Sprite = Sprite::new(Sheet::Titlebar, 0, 9, 9, 9);
    pub const MINIMIZE: Sprite = Sprite::new(Sheet::Titlebar, 9, 0, 9, 9);
    pub const MINIMIZE_DOWN: Sprite = Sprite::new(Sheet::Titlebar, 9, 9, 9, 9);
    /// Rolls the window up. The pair below rolls it back down again.
    pub const SHADE: Sprite = Sprite::new(Sheet::Titlebar, 0, 18, 9, 9);
    pub const SHADE_DOWN: Sprite = Sprite::new(Sheet::Titlebar, 9, 18, 9, 9);
    pub const UNSHADE: Sprite = Sprite::new(Sheet::Titlebar, 0, 27, 9, 9);
    pub const UNSHADE_DOWN: Sprite = Sprite::new(Sheet::Titlebar, 9, 27, 9, 9);
    pub const CLOSE: Sprite = Sprite::new(Sheet::Titlebar, 18, 0, 9, 9);
    pub const CLOSE_DOWN: Sprite = Sprite::new(Sheet::Titlebar, 18, 9, 9, 9);

    /// The O A I D V strip, and each lamp lit.
    pub const CLUTTER_BAR: Sprite = Sprite::new(Sheet::Titlebar, 304, 0, 8, 43);
    pub const CLUTTER_O_ON: Sprite = Sprite::new(Sheet::Titlebar, 304, 47, 8, 8);
    pub const CLUTTER_A_ON: Sprite = Sprite::new(Sheet::Titlebar, 312, 55, 8, 7);
    pub const CLUTTER_I_ON: Sprite = Sprite::new(Sheet::Titlebar, 320, 62, 8, 7);
    pub const CLUTTER_D_ON: Sprite = Sprite::new(Sheet::Titlebar, 328, 69, 8, 8);
    pub const CLUTTER_V_ON: Sprite = Sprite::new(Sheet::Titlebar, 336, 77, 8, 7);

    /// Shade mode's seek bar: a 17-pixel groove and a 3-pixel thumb, with
    /// end caps for the two extremes.
    pub const SHADE_POSBAR: Sprite = Sprite::new(Sheet::Titlebar, 0, 36, 17, 7);
    pub const SHADE_POSBAR_THUMB: Sprite = Sprite::new(Sheet::Titlebar, 20, 36, 3, 7);
    pub const SHADE_POSBAR_THUMB_LEFT: Sprite = Sprite::new(Sheet::Titlebar, 17, 36, 3, 7);
    pub const SHADE_POSBAR_THUMB_RIGHT: Sprite = Sprite::new(Sheet::Titlebar, 23, 36, 3, 7);

    // ===== cbuttons.bmp: released row at y=0, pressed row at y=18 =====

    pub const PREVIOUS: Sprite = Sprite::new(Sheet::CButtons, 0, 0, 23, 18);
    pub const PREVIOUS_DOWN: Sprite = Sprite::new(Sheet::CButtons, 0, 18, 23, 18);
    pub const PLAY: Sprite = Sprite::new(Sheet::CButtons, 23, 0, 23, 18);
    pub const PLAY_DOWN: Sprite = Sprite::new(Sheet::CButtons, 23, 18, 23, 18);
    pub const PAUSE: Sprite = Sprite::new(Sheet::CButtons, 46, 0, 23, 18);
    pub const PAUSE_DOWN: Sprite = Sprite::new(Sheet::CButtons, 46, 18, 23, 18);
    pub const STOP: Sprite = Sprite::new(Sheet::CButtons, 69, 0, 23, 18);
    pub const STOP_DOWN: Sprite = Sprite::new(Sheet::CButtons, 69, 18, 23, 18);
    pub const NEXT: Sprite = Sprite::new(Sheet::CButtons, 92, 0, 22, 18);
    pub const NEXT_DOWN: Sprite = Sprite::new(Sheet::CButtons, 92, 18, 22, 18);
    pub const EJECT: Sprite = Sprite::new(Sheet::CButtons, 114, 0, 22, 16);
    pub const EJECT_DOWN: Sprite = Sprite::new(Sheet::CButtons, 114, 16, 22, 16);

    // ===== shufrep.bmp =====

    // Shuffle and repeat: off/on × released/pressed.
    pub const SHUFFLE: Sprite = Sprite::new(Sheet::Shufrep, 28, 0, 47, 15);
    pub const SHUFFLE_DOWN: Sprite = Sprite::new(Sheet::Shufrep, 28, 15, 47, 15);
    pub const SHUFFLE_ON: Sprite = Sprite::new(Sheet::Shufrep, 28, 30, 47, 15);
    pub const SHUFFLE_ON_DOWN: Sprite = Sprite::new(Sheet::Shufrep, 28, 45, 47, 15);
    pub const REPEAT: Sprite = Sprite::new(Sheet::Shufrep, 0, 0, 28, 15);
    pub const REPEAT_DOWN: Sprite = Sprite::new(Sheet::Shufrep, 0, 15, 28, 15);
    pub const REPEAT_ON: Sprite = Sprite::new(Sheet::Shufrep, 0, 30, 28, 15);
    pub const REPEAT_ON_DOWN: Sprite = Sprite::new(Sheet::Shufrep, 0, 45, 28, 15);

    /// The EQ and playlist buttons live on the same sheet, off/on × pressed.
    pub const EQ: Sprite = Sprite::new(Sheet::Shufrep, 0, 61, 23, 12);
    pub const EQ_ON: Sprite = Sprite::new(Sheet::Shufrep, 0, 73, 23, 12);
    pub const EQ_DOWN: Sprite = Sprite::new(Sheet::Shufrep, 46, 61, 23, 12);
    pub const EQ_ON_DOWN: Sprite = Sprite::new(Sheet::Shufrep, 46, 73, 23, 12);
    pub const PLAYLIST: Sprite = Sprite::new(Sheet::Shufrep, 23, 61, 23, 12);
    pub const PLAYLIST_ON: Sprite = Sprite::new(Sheet::Shufrep, 23, 73, 23, 12);
    pub const PLAYLIST_DOWN: Sprite = Sprite::new(Sheet::Shufrep, 69, 61, 23, 12);
    pub const PLAYLIST_ON_DOWN: Sprite = Sprite::new(Sheet::Shufrep, 69, 73, 23, 12);

    // ===== posbar.bmp =====

    /// The seek bar's groove and its thumb.
    pub const POSBAR: Sprite = Sprite::new(Sheet::Posbar, 0, 0, 248, 10);
    pub const POSBAR_THUMB: Sprite = Sprite::new(Sheet::Posbar, 248, 0, 29, 10);
    pub const POSBAR_THUMB_DOWN: Sprite = Sprite::new(Sheet::Posbar, 278, 0, 29, 10);

    // ===== volume.bmp and balance.bmp =====

    /// Volume: twenty-eight 13px rows, brightest at the top.
    pub const VOLUME_ROWS: u32 = 28;
    pub const VOLUME_ROW_H: u32 = 13;
    /// Classic skin frames have two padding pixels between visible rows.
    pub const VOLUME_ROW_STRIDE: u32 = 15;
    pub const VOLUME_THUMB: Sprite = Sprite::new(Sheet::Volume, 15, 422, 14, 11);
    pub const VOLUME_THUMB_DOWN: Sprite = Sprite::new(Sheet::Volume, 0, 422, 14, 11);
    pub const BALANCE_THUMB: Sprite = Sprite::new(Sheet::Balance, 15, 422, 14, 11);
    pub const BALANCE_THUMB_DOWN: Sprite = Sprite::new(Sheet::Balance, 0, 422, 14, 11);

    /// One row of the volume strip. The rows are stacked, dimmest first.
    pub const fn volume_row(row: u32) -> Sprite {
        Sprite::new(
            Sheet::Volume,
            0,
            row * VOLUME_ROW_STRIDE,
            super::layout::VOLUME.width,
            VOLUME_ROW_H,
        )
    }

    /// One row of the balance strip. Balance art starts nine pixels in.
    pub const fn balance_row(row: u32) -> Sprite {
        Sprite::new(
            Sheet::Balance,
            9,
            row * VOLUME_ROW_STRIDE,
            super::layout::BALANCE.width,
            VOLUME_ROW_H,
        )
    }

    // ===== monoster.bmp: lit above, dim below =====

    pub const MONO: Sprite = Sprite::new(Sheet::Monoster, 29, 12, 27, 12);
    pub const MONO_ON: Sprite = Sprite::new(Sheet::Monoster, 29, 0, 27, 12);
    pub const STEREO: Sprite = Sprite::new(Sheet::Monoster, 0, 12, 29, 12);
    pub const STEREO_ON: Sprite = Sprite::new(Sheet::Monoster, 0, 0, 29, 12);

    // ===== numbers.bmp / nums_ex.bmp =====

    /// A digit cell. Cell ten is blank, which is what a leading zero and a
    /// stopped player show.
    pub const DIGIT_W: u32 = 9;
    pub const DIGIT_H: u32 = 13;
    pub const BLANK_DIGIT: u32 = 10;

    pub const fn digit(cell: u32) -> Sprite {
        Sprite::new(Sheet::Numbers, cell * DIGIT_W, 0, DIGIT_W, DIGIT_H)
    }

    /// `nums_ex.bmp`'s cells, where the minus sign has one of its own.
    pub const fn digit_ex(cell: u32) -> Sprite {
        Sprite::new(Sheet::NumsEx, cell * DIGIT_W, 0, DIGIT_W, DIGIT_H)
    }

    /// The minus sign: one row of pixels borrowed from `numbers.bmp`, or a
    /// whole cell when the skin ships `nums_ex.bmp`.
    pub const MINUS: Sprite = Sprite::new(Sheet::Numbers, 20, 6, 5, 1);
    pub const MINUS_EX: Sprite = Sprite::new(Sheet::NumsEx, 99, 0, DIGIT_W, DIGIT_H);

    // ===== playpaus.bmp =====

    /// The play/pause/stop indicator left of the time.
    pub const PLAYING: Sprite = Sprite::new(Sheet::Playpaus, 0, 0, 9, 9);
    pub const PAUSED: Sprite = Sprite::new(Sheet::Playpaus, 9, 0, 9, 9);
    pub const STOPPED: Sprite = Sprite::new(Sheet::Playpaus, 18, 0, 9, 9);
    /// The three-pixel sliver that lights while a track is loading. Winamp's
    /// sheet holds a nine-pixel sprite here and shows only its left edge.
    pub const WORKING: Sprite = Sprite::new(Sheet::Playpaus, 39, 0, 3, 9);

    // ===== eqmain.bmp: the equaliser =====

    pub const EQ_MAIN: Sprite = Sprite::new(Sheet::EqMain, 0, 0, 275, 116);
    pub const EQ_TITLE_BAR: Sprite = Sprite::new(Sheet::EqMain, 0, 134, 275, 14);
    pub const EQ_CLOSE: Sprite = Sprite::new(Sheet::EqMain, 0, 116, 9, 9);
    pub const EQ_CLOSE_DOWN: Sprite = Sprite::new(Sheet::EqMain, 0, 125, 9, 9);
    /// The shade button's pressed state is only in `eq_ex.bmp`; released is a
    /// patch of the title bar, which is why Webamp calls it a fallback.
    pub const EQ_SHADE: Sprite = Sprite::new(Sheet::EqMain, 254, 152, 9, 9);
    pub const EQ_SHADE_DOWN: Sprite = Sprite::new(Sheet::EqEx, 1, 38, 9, 9);

    /// The ON toggle, lit and not. (The main window's *button* that opens this
    /// window is `EQ`/`EQ_ON` up in the shufrep block; this is the switch
    /// inside it.)
    pub const EQ_ENABLED: Sprite = Sprite::new(Sheet::EqMain, 69, 119, 26, 12);
    pub const EQ_ENABLED_DOWN: Sprite = Sprite::new(Sheet::EqMain, 187, 119, 26, 12);
    pub const EQ_DISABLED: Sprite = Sprite::new(Sheet::EqMain, 10, 119, 26, 12);
    pub const EQ_DISABLED_DOWN: Sprite = Sprite::new(Sheet::EqMain, 128, 119, 26, 12);
    pub const EQ_AUTO_ON: Sprite = Sprite::new(Sheet::EqMain, 95, 119, 32, 12);
    pub const EQ_AUTO_ON_DOWN: Sprite = Sprite::new(Sheet::EqMain, 213, 119, 32, 12);
    pub const EQ_AUTO: Sprite = Sprite::new(Sheet::EqMain, 36, 119, 32, 12);
    pub const EQ_AUTO_DOWN: Sprite = Sprite::new(Sheet::EqMain, 154, 119, 32, 12);
    pub const EQ_PRESETS: Sprite = Sprite::new(Sheet::EqMain, 224, 164, 44, 12);
    pub const EQ_PRESETS_DOWN: Sprite = Sprite::new(Sheet::EqMain, 224, 176, 44, 12);

    pub const EQ_THUMB: Sprite = Sprite::new(Sheet::EqMain, 0, 164, 11, 11);
    pub const EQ_THUMB_DOWN: Sprite = Sprite::new(Sheet::EqMain, 0, 176, 11, 11);

    /// The graph's background and the one-pixel column its line colours come
    /// from: nineteen rows, top to bottom.
    pub const EQ_GRAPH: Sprite = Sprite::new(Sheet::EqMain, 0, 294, 113, 19);
    pub const EQ_GRAPH_COLORS: Sprite = Sprite::new(Sheet::EqMain, 115, 294, 1, 19);
    pub const EQ_PREAMP_LINE: Sprite = Sprite::new(Sheet::EqMain, 0, 314, 113, 1);

    /// A band's background. Winamp keeps twenty-eight of them in a 14×2 grid
    /// starting at (13, 164), one per slider position, so the track itself
    /// brightens as the band is raised.
    pub const EQ_BAND_STEPS: u32 = 28;

    pub const fn eq_band(step: u32) -> Sprite {
        let step = if step >= EQ_BAND_STEPS {
            EQ_BAND_STEPS - 1
        } else {
            step
        };
        Sprite::new(
            Sheet::EqMain,
            13 + (step % 14) * 15,
            164 + (step / 14) * 65,
            14,
            63,
        )
    }

    // ===== eq_ex.bmp: the equaliser rolled up =====

    pub const EQ_SHADE_BAR: Sprite = Sprite::new(Sheet::EqEx, 0, 0, 275, 14);
    pub const EQ_SHADE_CLOSE: Sprite = Sprite::new(Sheet::EqEx, 11, 38, 9, 9);
    pub const EQ_SHADE_CLOSE_DOWN: Sprite = Sprite::new(Sheet::EqEx, 11, 47, 9, 9);
    pub const EQ_SHADE_UNSHADE: Sprite = Sprite::new(Sheet::EqEx, 1, 47, 9, 9);
    /// The rolled-up sliders are drawn from three pieces, so they can be any
    /// width: a left cap, a middle that repeats, a right cap.
    pub const EQ_SHADE_VOL_LEFT: Sprite = Sprite::new(Sheet::EqEx, 1, 30, 3, 7);
    pub const EQ_SHADE_VOL_MID: Sprite = Sprite::new(Sheet::EqEx, 4, 30, 3, 7);
    pub const EQ_SHADE_VOL_RIGHT: Sprite = Sprite::new(Sheet::EqEx, 7, 30, 3, 7);
    pub const EQ_SHADE_BAL_LEFT: Sprite = Sprite::new(Sheet::EqEx, 11, 30, 3, 7);
    pub const EQ_SHADE_BAL_MID: Sprite = Sprite::new(Sheet::EqEx, 14, 30, 3, 7);
    pub const EQ_SHADE_BAL_RIGHT: Sprite = Sprite::new(Sheet::EqEx, 17, 30, 3, 7);

    // ===== pledit.bmp: the playlist =====

    /// The title bar, in three pieces plus two corners so it can be any width.
    pub const PL_TOP_LEFT: Sprite = Sprite::new(Sheet::Pledit, 0, 0, 25, 20);
    pub const PL_TOP_TILE: Sprite = Sprite::new(Sheet::Pledit, 127, 0, 25, 20);
    pub const PL_TOP_TITLE: Sprite = Sprite::new(Sheet::Pledit, 26, 0, 100, 20);
    pub const PL_TOP_RIGHT: Sprite = Sprite::new(Sheet::Pledit, 153, 0, 25, 20);
    /// Unfocused, which is the row below. Kept because a skin may differ.
    pub const PL_TOP_LEFT_DIM: Sprite = Sprite::new(Sheet::Pledit, 0, 21, 25, 20);
    pub const PL_TOP_TILE_DIM: Sprite = Sprite::new(Sheet::Pledit, 127, 21, 25, 20);
    pub const PL_TOP_TITLE_DIM: Sprite = Sprite::new(Sheet::Pledit, 26, 21, 100, 20);
    pub const PL_TOP_RIGHT_DIM: Sprite = Sprite::new(Sheet::Pledit, 153, 21, 25, 20);

    /// The sides, which tile down.
    pub const PL_LEFT_TILE: Sprite = Sprite::new(Sheet::Pledit, 0, 42, 12, 29);
    pub const PL_RIGHT_TILE: Sprite = Sprite::new(Sheet::Pledit, 31, 42, 20, 29);

    /// The bottom strip: two wide corners and a tile between them.
    pub const PL_BOTTOM_LEFT: Sprite = Sprite::new(Sheet::Pledit, 0, 72, 125, 38);
    pub const PL_BOTTOM_RIGHT: Sprite = Sprite::new(Sheet::Pledit, 126, 72, 150, 38);
    pub const PL_BOTTOM_TILE: Sprite = Sprite::new(Sheet::Pledit, 179, 0, 25, 38);
    /// The little visualiser's well on the bottom strip.
    pub const PL_VIS_BACKGROUND: Sprite = Sprite::new(Sheet::Pledit, 205, 0, 75, 38);

    pub const PL_CLOSE: Sprite = Sprite::new(Sheet::Pledit, 52, 42, 9, 9);
    pub const PL_SHADE: Sprite = Sprite::new(Sheet::Pledit, 62, 42, 9, 9);
    pub const PL_UNSHADE: Sprite = Sprite::new(Sheet::Pledit, 150, 42, 9, 9);
    pub const PL_SCROLL_HANDLE: Sprite = Sprite::new(Sheet::Pledit, 52, 53, 8, 18);
    pub const PL_SCROLL_HANDLE_DOWN: Sprite = Sprite::new(Sheet::Pledit, 61, 53, 8, 18);

    /// Rolled up, in three pieces like everything else that resizes.
    pub const PL_SHADE_LEFT: Sprite = Sprite::new(Sheet::Pledit, 72, 42, 25, 14);
    pub const PL_SHADE_TILE: Sprite = Sprite::new(Sheet::Pledit, 72, 57, 25, 14);
    pub const PL_SHADE_RIGHT: Sprite = Sprite::new(Sheet::Pledit, 99, 42, 50, 14);
}

/// A decoded sprite sheet: RGBA, ready for an egui texture.
#[derive(Debug, Clone)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Bitmap {
    /// Crop a sprite out, as an egui colour image.
    pub fn crop(&self, sprite: Sprite) -> egui::ColorImage {
        let w = sprite.w.min(self.width.saturating_sub(sprite.x)) as usize;
        let h = sprite.h.min(self.height.saturating_sub(sprite.y)) as usize;
        let mut pixels = Vec::with_capacity(w * h);
        for row in 0..h {
            let y = sprite.y as usize + row;
            let start = (y * self.width as usize + sprite.x as usize) * 4;
            for col in 0..w {
                let at = start + col * 4;
                pixels.push(egui::Color32::from_rgba_unmultiplied(
                    self.rgba[at],
                    self.rgba[at + 1],
                    self.rgba[at + 2],
                    self.rgba[at + 3],
                ));
            }
        }
        egui::ColorImage {
            size: [w.max(1), h.max(1)],
            pixels: if pixels.is_empty() {
                vec![egui::Color32::TRANSPARENT]
            } else {
                pixels
            },
            source_size: egui::vec2(w.max(1) as f32, h.max(1) as f32),
        }
    }
}

/// A loaded skin: its sheets and its colours.
pub struct Skin {
    pub name: String,
    pub(crate) sheets: HashMap<Sheet, Bitmap>,
    /// The twenty-four visualiser colours from `viscolor.txt`.
    pub vis_colors: [egui::Color32; 24],
    /// The playlist palette from `pledit.txt`, for the playlist window.
    #[allow(dead_code)]
    pub playlist: PlaylistColors,
}

/// `pledit.txt`'s palette. Defaults are Winamp's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaylistColors {
    pub normal: egui::Color32,
    pub current: egui::Color32,
    pub background: egui::Color32,
    pub selected_background: egui::Color32,
}

impl Default for PlaylistColors {
    fn default() -> Self {
        Self {
            normal: egui::Color32::from_rgb(0x00, 0xFF, 0x00),
            current: egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),
            background: egui::Color32::from_rgb(0x00, 0x00, 0x00),
            selected_background: egui::Color32::from_rgb(0x00, 0x00, 0xC6),
        }
    }
}

/// Winamp's stock visualiser palette: black, then the green-to-yellow-to-red
/// ramp for the bars, the peak mark, and the oscilloscope's five greens.
pub fn default_vis_colors() -> [egui::Color32; 24] {
    const RAW: [(u8, u8, u8); 24] = [
        (0, 0, 0),
        (24, 33, 41),
        (239, 249, 255),
        (0, 34, 41),
        (24, 132, 165),
        (24, 148, 189),
        (24, 165, 206),
        (24, 181, 231),
        (24, 189, 239),
        (56, 206, 247),
        (82, 214, 255),
        (107, 222, 255),
        (140, 235, 255),
        (165, 243, 255),
        (198, 251, 255),
        (231, 255, 255),
        (255, 255, 255),
        (214, 235, 255),
        (181, 222, 255),
        (156, 206, 255),
        (132, 189, 255),
        (107, 173, 255),
        (82, 156, 255),
        (57, 140, 255),
    ];
    RAW.map(|(r, g, b)| egui::Color32::from_rgb(r, g, b))
}

impl Skin {
    /// Load a `.wsz` from bytes. Missing sheets are simply absent, so a
    /// partial skin still opens — Winamp behaved the same.
    pub fn from_wsz(name: impl Into<String>, data: Vec<u8>) -> Result<Self> {
        let archive = zip::Archive::open(data).context("open skin archive")?;
        let mut sheets = HashMap::new();
        for sheet in Sheet::ALL {
            if let Some(bytes) = archive.read(sheet.file()) {
                match bytes.and_then(|b| decode_bmp(&b)) {
                    Ok(bitmap) => {
                        sheets.insert(sheet, bitmap);
                    }
                    Err(e) => log::warn!("skin {}: {e}", sheet.file()),
                }
            }
        }
        anyhow::ensure!(
            sheets.contains_key(&Sheet::Main),
            "the skin has no main.bmp"
        );
        let vis_colors = archive
            .read("viscolor.txt")
            .and_then(|r| r.ok())
            .map(|b| parse_viscolor(&String::from_utf8_lossy(&b)))
            .unwrap_or_else(default_vis_colors);
        let playlist = archive
            .read("pledit.txt")
            .and_then(|r| r.ok())
            .map(|b| parse_pledit(&String::from_utf8_lossy(&b)))
            .unwrap_or_default();
        Ok(Self {
            name: name.into(),
            sheets,
            vis_colors,
            playlist,
        })
    }

    /// One sheet, whole. The stock skin's tests check its sizes this way.
    #[allow(dead_code)]
    pub fn sheet(&self, sheet: Sheet) -> Option<&Bitmap> {
        self.sheets.get(&sheet)
    }

    /// Whether the skin ships a sheet, so a fallback can be chosen before
    /// anything is painted. `nums_ex.bmp` and `titlebar.bmp` are both optional
    /// and both have one.
    pub fn has(&self, sheet: Sheet) -> bool {
        self.sheets.contains_key(&sheet)
    }

    /// Fill sheets omitted by a partial skin from a complete fallback skin.
    ///
    /// Classic Winamp accepted partial archives, and quite a few community
    /// skins rely on the player supplying the missing controls. Keeping the
    /// custom sheets and borrowing only the absent ones prevents invisible
    /// transport/title-bar buttons without replacing the selected skin.
    pub fn fill_missing_from(&mut self, fallback: &Self) -> usize {
        let mut filled = 0;
        for sheet in Sheet::ALL {
            if !self.sheets.contains_key(&sheet)
                && let Some(bitmap) = fallback.sheets.get(&sheet)
            {
                self.sheets.insert(sheet, bitmap.clone());
                filled += 1;
            }
        }
        filled
    }

    /// Crop a sprite, or `None` when the skin lacks that sheet.
    pub fn sprite(&self, sprite: Sprite) -> Option<egui::ColorImage> {
        self.sheets.get(&sprite.sheet).map(|b| b.crop(sprite))
    }

    /// A sprite's pixels as a column of colours, top to bottom.
    ///
    /// `eqmain.bmp` keeps the equaliser graph's line colours as a one-pixel
    /// column ([`sprites::EQ_GRAPH_COLORS`]): the curve takes its colour from
    /// the row it is on, which is how Winamp grades the line without drawing a
    /// gradient. Read once when the skin is worn, not per frame.
    pub fn column(&self, sprite: Sprite) -> Option<Vec<egui::Color32>> {
        let image = self.sprite(sprite)?;
        let width = image.size[0];
        if width == 0 || image.size[1] == 0 {
            return None;
        }
        // The leftmost pixel of each row: a wider sprite is still one column of
        // colour, and this is what makes a one-pixel crop and a wide one agree.
        Some(image.pixels.iter().step_by(width).copied().collect())
    }
}

/// Decode a BMP into RGBA. Winamp skins are 8-bit or 24-bit, which `image`
/// handles; anything it can read works.
fn decode_bmp(bytes: &[u8]) -> Result<Bitmap> {
    let decoded = image::load_from_memory_with_format(bytes, image::ImageFormat::Bmp)
        .context("decode BMP")?;
    let rgba = decoded.to_rgba8();
    Ok(Bitmap {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

/// `viscolor.txt`: twenty-four `r,g,b` lines, anything after them ignored.
/// Comments start at `//`. Short files fall back to the stock palette for
/// the colours they do not name.
pub fn parse_viscolor(text: &str) -> [egui::Color32; 24] {
    let mut colors = default_vis_colors();
    for (slot, line) in colors.iter_mut().zip(text.lines()) {
        let line = line.split("//").next().unwrap_or("");
        let mut parts = line.split(',').filter_map(|p| p.trim().parse::<u8>().ok());
        if let (Some(r), Some(g), Some(b)) = (parts.next(), parts.next(), parts.next()) {
            *slot = egui::Color32::from_rgb(r, g, b);
        }
    }
    colors
}

/// `pledit.txt`: an ini-ish `[Text]` section of `#rrggbb` values.
pub fn parse_pledit(text: &str) -> PlaylistColors {
    let mut out = PlaylistColors::default();
    for line in text.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some(color) = parse_hex(value.trim()) else {
            continue;
        };
        match key.trim().to_lowercase().as_str() {
            "normal" => out.normal = color,
            "current" => out.current = color,
            "normalbg" => out.background = color,
            "selectedbg" => out.selected_background = color,
            _ => {}
        }
    }
    out
}

/// `#RRGGBB`, with or without the hash.
fn parse_hex(value: &str) -> Option<egui::Color32> {
    let hex = value.trim_start_matches('#');
    if hex.len() < 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(egui::Color32::from_rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bitmap(width: u32, height: u32) -> Bitmap {
        // Each pixel encodes its own coordinates, so a crop is checkable.
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                rgba.extend_from_slice(&[x as u8, y as u8, 0, 255]);
            }
        }
        Bitmap {
            width,
            height,
            rgba,
        }
    }

    #[test]
    fn a_sprite_is_cropped_from_its_sheet() {
        let sheet = bitmap(40, 20);
        let image = sheet.crop(Sprite::new(Sheet::CButtons, 23, 0, 10, 5));
        assert_eq!(image.size, [10, 5]);
        // Top-left of the crop is the sheet's (23, 0).
        assert_eq!(image.pixels[0], egui::Color32::from_rgb(23, 0, 0));
        // …and the next row starts one down.
        assert_eq!(image.pixels[10], egui::Color32::from_rgb(23, 1, 0));
    }

    /// A sprite that runs off the sheet is clamped, not a panic: skins in the
    /// wild ship short sheets (no eject button, a cut-down volume strip).
    #[test]
    fn a_sprite_past_the_edge_is_clamped() {
        let sheet = bitmap(20, 10);
        let image = sheet.crop(Sprite::new(Sheet::CButtons, 15, 8, 20, 20));
        assert_eq!(image.size, [5, 2]);
        let empty = sheet.crop(Sprite::new(Sheet::CButtons, 100, 100, 10, 10));
        assert_eq!(empty.size, [1, 1], "a fully outside crop is one pixel");
    }

    #[test]
    fn a_partial_skin_keeps_its_art_and_borrows_missing_controls() {
        let mut custom_main = bitmap(MAIN_WIDTH, MAIN_HEIGHT);
        custom_main.rgba[0] = 77;
        let custom_pixel = custom_main.rgba[0];
        let mut custom = Skin {
            name: "Partial".to_owned(),
            sheets: HashMap::from([(Sheet::Main, custom_main)]),
            vis_colors: default_vis_colors(),
            playlist: PlaylistColors::default(),
        };
        let fallback = Skin {
            name: "Fallback".to_owned(),
            sheets: HashMap::from([
                (Sheet::Main, bitmap(MAIN_WIDTH, MAIN_HEIGHT)),
                (Sheet::CButtons, bitmap(136, 36)),
            ]),
            vis_colors: default_vis_colors(),
            playlist: PlaylistColors::default(),
        };

        assert_eq!(custom.fill_missing_from(&fallback), 1);
        assert!(custom.has(Sheet::CButtons));
        assert_eq!(custom.sheet(Sheet::Main).unwrap().rgba[0], custom_pixel);
        assert_eq!(custom.name, "Partial");
    }

    #[test]
    fn viscolor_reads_the_first_twenty_four_lines() {
        let text = "0,0,0\n255,0,0 // red\n0,255,0\nnot a colour\n";
        let colors = parse_viscolor(text);
        assert_eq!(colors[0], egui::Color32::BLACK);
        assert_eq!(colors[1], egui::Color32::from_rgb(255, 0, 0));
        assert_eq!(colors[2], egui::Color32::from_rgb(0, 255, 0));
        // A bad line keeps the stock colour rather than turning black.
        assert_eq!(colors[3], default_vis_colors()[3]);
        assert_eq!(colors.len(), 24);
    }

    #[test]
    fn pledit_reads_the_palette_and_keeps_defaults() {
        let text = "[Text]\nNormal=#00FF00\nCurrent=#FFFFFF\nNormalBG=#101010\n";
        let colors = parse_pledit(text);
        assert_eq!(colors.normal, egui::Color32::from_rgb(0, 255, 0));
        assert_eq!(colors.background, egui::Color32::from_rgb(0x10, 0x10, 0x10));
        // Not named: Winamp's own blue.
        assert_eq!(
            colors.selected_background,
            PlaylistColors::default().selected_background
        );
        assert_eq!(parse_pledit("nonsense"), PlaylistColors::default());
    }

    #[test]
    fn hex_colours_take_either_form() {
        assert_eq!(
            parse_hex("#FF8800"),
            Some(egui::Color32::from_rgb(255, 136, 0))
        );
        assert_eq!(
            parse_hex("ff8800"),
            Some(egui::Color32::from_rgb(255, 136, 0))
        );
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn sheet_files_are_the_winamp_names() {
        assert_eq!(Sheet::Main.file(), "main.bmp");
        assert_eq!(Sheet::CButtons.file(), "cbuttons.bmp");
        assert_eq!(Sheet::NumsEx.file(), "nums_ex.bmp");
        assert_eq!(Sheet::EqMain.file(), "eqmain.bmp");
        assert_eq!(Sheet::EqEx.file(), "eq_ex.bmp");
        assert_eq!(Sheet::Pledit.file(), "pledit.bmp");
        // Every member has its own file, lowercased: the archive is looked up
        // by leaf name, so two sheets sharing one would silently alias. Counted
        // this way rather than against a number, which goes stale as sheets are
        // added and says nothing about what went wrong.
        let names: std::collections::BTreeSet<&str> =
            Sheet::ALL.iter().map(|sheet| sheet.file()).collect();
        assert_eq!(
            names.len(),
            Sheet::ALL.len(),
            "two sheets share a file name"
        );
        for name in names {
            assert_eq!(name, name.to_lowercase(), "{name} is not lowercased");
            assert!(name.ends_with(".bmp"), "{name} is not a bitmap");
        }
    }

    /// Every sprite must be inside the sheet it names, and every control the
    /// layout places must have art the same size as its slot. This is the
    /// join between the two modules: [`layout`] says *where*, `sprites` says
    /// *what*, and a mismatch means a stretched or clipped control.
    #[test]
    fn sprites_and_layout_agree_on_sizes() {
        for (name, sprite, area) in [
            ("PREVIOUS", sprites::PREVIOUS, layout::PREVIOUS),
            ("PLAY", sprites::PLAY, layout::PLAY),
            ("PAUSE", sprites::PAUSE, layout::PAUSE),
            ("STOP", sprites::STOP, layout::STOP),
            ("NEXT", sprites::NEXT, layout::NEXT),
            ("EJECT", sprites::EJECT, layout::EJECT),
            ("SHUFFLE", sprites::SHUFFLE, layout::SHUFFLE),
            ("REPEAT", sprites::REPEAT, layout::REPEAT),
            ("EQ", sprites::EQ, layout::EQ_BUTTON),
            ("PLAYLIST", sprites::PLAYLIST, layout::PLAYLIST_BUTTON),
            ("POSBAR", sprites::POSBAR, layout::POSITION),
            ("MONO", sprites::MONO, layout::MONO),
            ("STEREO", sprites::STEREO, layout::STEREO),
            ("OPTIONS", sprites::OPTIONS, layout::OPTIONS_BUTTON),
            ("MINIMIZE", sprites::MINIMIZE, layout::MINIMIZE_BUTTON),
            ("SHADE", sprites::SHADE, layout::SHADE_BUTTON),
            ("CLOSE", sprites::CLOSE, layout::CLOSE_BUTTON),
            ("CLUTTER_BAR", sprites::CLUTTER_BAR, layout::CLUTTER_BAR),
            ("CLUTTER_O", sprites::CLUTTER_O_ON, layout::CLUTTER_O),
            ("CLUTTER_A", sprites::CLUTTER_A_ON, layout::CLUTTER_A),
            ("CLUTTER_I", sprites::CLUTTER_I_ON, layout::CLUTTER_I),
            ("CLUTTER_D", sprites::CLUTTER_D_ON, layout::CLUTTER_D),
            ("CLUTTER_V", sprites::CLUTTER_V_ON, layout::CLUTTER_V),
            ("STATUS", sprites::PLAYING, layout::STATUS),
            ("WORKING", sprites::WORKING, layout::WORK_INDICATOR),
            ("MINUS_EX", sprites::MINUS_EX, layout::MINUS_EX),
            ("MINUS", sprites::MINUS, layout::MINUS),
            ("DIGIT", sprites::digit(0), layout::MINUTE_TENS),
            (
                "SHADE_POSBAR",
                sprites::SHADE_POSBAR,
                layout::SHADE_POSITION,
            ),
        ] {
            assert_eq!(
                (sprite.w, sprite.h),
                (area.width, area.height),
                "{name}: the art is {}×{} but its slot is {}×{}",
                sprite.w,
                sprite.h,
                area.width,
                area.height
            );
        }
        // The strips are as wide as their tracks, one row tall.
        assert_eq!(sprites::volume_row(0).w, layout::VOLUME.width);
        // Classic BMP frames are 13px tall at 15px intervals. Using 13 as
        // the stride makes the track appear to roll vertically while dragging.
        assert_eq!(sprites::volume_row(1).y, 15);
        assert_eq!(sprites::volume_row(27).y, 405);
        assert_eq!(sprites::balance_row(27).y, 405);
        assert_eq!(sprites::balance_row(0).w, layout::BALANCE.width);
        assert_eq!(sprites::VOLUME_THUMB.w, layout::VOLUME_THUMB_W);
        assert_eq!(sprites::BALANCE_THUMB.w, layout::BALANCE_THUMB_W);
        assert_eq!(sprites::POSBAR_THUMB.w, layout::POSITION_THUMB_W);
        assert_eq!(
            sprites::SHADE_POSBAR_THUMB.w,
            layout::SHADE_POSITION_THUMB_W
        );
        // The title bar covers the window's width, in both modes.
        assert_eq!(sprites::TITLE_BAR.w, MAIN_WIDTH);
        assert_eq!(sprites::SHADE_BAR.w, MAIN_WIDTH);
        assert_eq!(sprites::SHADE_BAR.h, layout::SHADE_HEIGHT);
    }

    /// The digit cells step across `numbers.bmp` without overlapping, and the
    /// eleventh is the blank one.
    #[test]
    fn the_digit_cells_tile_their_sheet() {
        for cell in 0..=sprites::BLANK_DIGIT {
            let sprite = sprites::digit(cell);
            assert_eq!(sprite.x, cell * sprites::DIGIT_W);
            assert_eq!(sprite.sheet, Sheet::Numbers);
            assert_eq!(sprites::digit_ex(cell).sheet, Sheet::NumsEx);
        }
        assert_eq!(sprites::BLANK_DIGIT, 10, "ten digits, then the blank");
    }

    /// The same join for the equaliser and the playlist. Their windows came
    /// after the main one and had no such check, which is how `PL_VIS_BACKGROUND`
    /// came to be 75×38 for a 72×16 slot — [`Bitmap::crop`] clamps and
    /// `blit_at` paints at the sprite's own size, so the overhang only showed
    /// because the window clips it.
    #[test]
    fn the_eq_and_playlist_sprites_fit_their_slots() {
        use layout::{Area, eq, pl};
        for (name, sprite, area) in [
            (
                "EQ_MAIN",
                sprites::EQ_MAIN,
                Area::new(0, 0, eq::WIDTH, eq::HEIGHT),
            ),
            ("EQ_TITLE_BAR", sprites::EQ_TITLE_BAR, eq::TITLE_BAR),
            ("EQ_SHADE_BAR", sprites::EQ_SHADE_BAR, eq::TITLE_BAR),
            ("EQ_CLOSE", sprites::EQ_CLOSE, eq::CLOSE_BUTTON),
            ("EQ_SHADE", sprites::EQ_SHADE, eq::SHADE_BUTTON),
            ("EQ_SHADE_CLOSE", sprites::EQ_SHADE_CLOSE, eq::CLOSE_BUTTON),
            (
                "EQ_SHADE_UNSHADE",
                sprites::EQ_SHADE_UNSHADE,
                eq::SHADE_BUTTON,
            ),
            ("EQ_ENABLED", sprites::EQ_ENABLED, eq::ON_BUTTON),
            ("EQ_DISABLED", sprites::EQ_DISABLED, eq::ON_BUTTON),
            ("EQ_AUTO", sprites::EQ_AUTO, eq::AUTO_BUTTON),
            ("EQ_AUTO_ON", sprites::EQ_AUTO_ON, eq::AUTO_BUTTON),
            ("EQ_PRESETS", sprites::EQ_PRESETS, eq::PRESETS_BUTTON),
            ("EQ_GRAPH", sprites::EQ_GRAPH, eq::GRAPH),
            ("eq_band", sprites::eq_band(0), eq::PREAMP),
            (
                "eq_band(last)",
                sprites::eq_band(sprites::EQ_BAND_STEPS - 1),
                eq::band(0),
            ),
        ] {
            assert_eq!(
                (sprite.w, sprite.h),
                (area.width, area.height),
                "{name}: the art is {}×{} but its slot is {}×{}",
                sprite.w,
                sprite.h,
                area.width,
                area.height
            );
        }
        // The thumb travels inside the band, so it must be shorter than it and
        // narrower by the two pixels the track's own edges take.
        assert_eq!(sprites::EQ_THUMB.h, eq::THUMB_HEIGHT);
        assert_eq!(sprites::EQ_THUMB.w, eq::BAND_WIDTH - 3);
        // The graph's colour column is one pixel wide and as tall as the graph:
        // one colour per row is what grades the curve.
        assert_eq!(sprites::EQ_GRAPH_COLORS.w, 1);
        assert_eq!(sprites::EQ_GRAPH_COLORS.h, eq::GRAPH.height);
        assert_eq!(sprites::EQ_PREAMP_LINE.w, eq::GRAPH.width);
        assert_eq!(sprites::EQ_PREAMP_LINE.h, 1);
        // The rolled-up sliders are drawn from three pieces of one width.
        for piece in [
            sprites::EQ_SHADE_VOL_LEFT,
            sprites::EQ_SHADE_VOL_MID,
            sprites::EQ_SHADE_VOL_RIGHT,
            sprites::EQ_SHADE_BAL_LEFT,
            sprites::EQ_SHADE_BAL_MID,
            sprites::EQ_SHADE_BAL_RIGHT,
        ] {
            assert_eq!(piece.w, eq::SHADE_THUMB_W, "a shade slider piece is off");
        }

        // The playlist's frame is arithmetic on a size, so the pieces are
        // checked against the metrics rather than against fixed rectangles.
        let (w, h) = (pl::MIN_WIDTH, pl::height(pl::DEFAULT_ROWS));
        for (name, sprite) in [
            ("PL_TOP_LEFT", sprites::PL_TOP_LEFT),
            ("PL_TOP_TILE", sprites::PL_TOP_TILE),
            ("PL_TOP_TITLE", sprites::PL_TOP_TITLE),
            ("PL_TOP_RIGHT", sprites::PL_TOP_RIGHT),
            ("PL_TOP_LEFT_DIM", sprites::PL_TOP_LEFT_DIM),
            ("PL_TOP_TILE_DIM", sprites::PL_TOP_TILE_DIM),
            ("PL_TOP_TITLE_DIM", sprites::PL_TOP_TITLE_DIM),
            ("PL_TOP_RIGHT_DIM", sprites::PL_TOP_RIGHT_DIM),
        ] {
            assert_eq!(sprite.h, pl::TOP_HEIGHT, "{name} is not the bar's height");
        }
        assert_eq!(sprites::PL_LEFT_TILE.w, pl::LEFT_WIDTH);
        assert_eq!(sprites::PL_RIGHT_TILE.w, pl::RIGHT_WIDTH);
        assert_eq!(sprites::PL_BOTTOM_LEFT.h, pl::BOTTOM_HEIGHT);
        assert_eq!(sprites::PL_BOTTOM_RIGHT.h, pl::BOTTOM_HEIGHT);
        assert_eq!(sprites::PL_BOTTOM_TILE.h, pl::BOTTOM_HEIGHT);
        assert_eq!(sprites::PL_BOTTOM_RIGHT.w, pl::BOTTOM_RIGHT_WIDTH);
        // The two bottom corners cover the narrowest window between them, which
        // is why the tile is never drawn at the width the app uses.
        assert_eq!(
            sprites::PL_BOTTOM_LEFT.w + sprites::PL_BOTTOM_RIGHT.w,
            pl::MIN_WIDTH
        );
        for (name, sprite, area) in [
            ("PL_CLOSE", sprites::PL_CLOSE, pl::close_button(w)),
            ("PL_SHADE", sprites::PL_SHADE, pl::shade_button(w)),
            ("PL_UNSHADE", sprites::PL_UNSHADE, pl::shade_button(w)),
        ] {
            assert_eq!(
                (sprite.w, sprite.h),
                (area.width, area.height),
                "{name} does not fill its slot"
            );
        }
        assert_eq!(sprites::PL_SCROLL_HANDLE.w, pl::SCROLLBAR_W);
        assert_eq!(sprites::PL_SCROLL_HANDLE.h, pl::SCROLL_HANDLE_H);
        for (name, sprite) in [
            ("PL_SHADE_LEFT", sprites::PL_SHADE_LEFT),
            ("PL_SHADE_TILE", sprites::PL_SHADE_TILE),
            ("PL_SHADE_RIGHT", sprites::PL_SHADE_RIGHT),
        ] {
            assert_eq!(sprite.h, pl::SHADE_HEIGHT, "{name} is not the shade height");
        }
        // The little visualiser: its art is the well *and* the strip behind it,
        // so it is bigger than the trace's own area on purpose.
        let vis = pl::visualiser(w, h);
        assert!(
            sprites::PL_VIS_BACKGROUND.w >= vis.width && sprites::PL_VIS_BACKGROUND.h >= vis.height,
            "the visualiser's well is smaller than the trace"
        );
    }

    /// A skin without `main.bmp` is not a skin; anything else may be missing.
    #[test]
    fn a_skin_needs_its_background() {
        let empty = zip_with(&[("viscolor.txt", b"0,0,0")]);
        assert!(Skin::from_wsz("broken", empty).is_err());
    }

    /// One-member-per-name stored zip, reusing the archive tests' layout.
    fn zip_with(files: &[(&str, &[u8])]) -> Vec<u8> {
        // Only the single-file case is needed here.
        let (name, body) = files[0];
        super::zip::tests_support::stored_zip(name, body)
    }
}
