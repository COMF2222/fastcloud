//! The built-in skin, drawn in code rather than shipped as bitmaps.
//!
//! Winamp's own layout needs eleven sprite sheets. Rather than ship (and
//! license) someone else's art, the stock look is generated at startup in
//! SoundCloud's colours: the same sprite rectangles [`super::sprites`]
//! names, filled with flat panels, bevels and glyphs. A dropped `.wsz`
//! replaces it wholesale, so both paths go through one [`Skin`].
//!
//! Every sheet is exactly the size the classic coordinates expect, so a
//! generated sheet and a real one crop identically.

use super::{Bitmap, PlaylistColors, Sheet, Skin, default_vis_colors};
use eframe::egui::Color32;
use std::collections::HashMap;

/// SoundCloud orange, and the greys the dark UI uses.
const ACCENT: [u8; 3] = [255, 85, 0];
const ACCENT_DIM: [u8; 3] = [150, 60, 15];
const PANEL: [u8; 3] = [26, 26, 26];
const PANEL_LIGHT: [u8; 3] = [38, 38, 38];
const DISPLAY: [u8; 3] = [12, 12, 12];
const INK: [u8; 3] = [245, 245, 245];
const EDGE: [u8; 3] = [51, 51, 51];

/// A sheet being painted: RGBA, origin top-left.
struct Canvas {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            rgba: vec![0; (width * height * 4) as usize],
        }
    }

    fn set(&mut self, x: u32, y: u32, rgb: [u8; 3]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let at = ((y * self.width + x) * 4) as usize;
        self.rgba[at] = rgb[0];
        self.rgba[at + 1] = rgb[1];
        self.rgba[at + 2] = rgb[2];
        self.rgba[at + 3] = 255;
    }

    fn fill(&mut self, x: u32, y: u32, w: u32, h: u32, rgb: [u8; 3]) {
        for row in y..y.saturating_add(h) {
            for col in x..x.saturating_add(w) {
                self.set(col, row, rgb);
            }
        }
    }

    /// A one-pixel border.
    fn stroke(&mut self, x: u32, y: u32, w: u32, h: u32, rgb: [u8; 3]) {
        if w == 0 || h == 0 {
            return;
        }
        self.fill(x, y, w, 1, rgb);
        self.fill(x, y + h - 1, w, 1, rgb);
        self.fill(x, y, 1, h, rgb);
        self.fill(x + w - 1, y, 1, h, rgb);
    }

    /// A button face: panel fill, light top-left, dark bottom-right.
    fn button(&mut self, x: u32, y: u32, w: u32, h: u32, pressed: bool) {
        let (face, light, dark) = if pressed {
            (PANEL, EDGE, PANEL_LIGHT)
        } else {
            (PANEL_LIGHT, [64, 64, 64], EDGE)
        };
        self.fill(x, y, w, h, face);
        self.fill(x, y, w, 1, light);
        self.fill(x, y, 1, h, light);
        self.fill(x, y + h - 1, w, 1, dark);
        self.fill(x + w - 1, y, 1, h, dark);
    }

    /// A filled triangle pointing left or right, for the transport glyphs.
    fn triangle(&mut self, cx: u32, cy: u32, half: u32, right: bool, rgb: [u8; 3]) {
        for dy in 0..=half {
            let width = half - dy;
            for step in 0..=width {
                let x = if right {
                    cx.saturating_sub(half).saturating_add(step)
                } else {
                    cx.saturating_add(half).saturating_sub(step)
                };
                self.set(x, cy.saturating_sub(dy), rgb);
                self.set(x, cy + dy, rgb);
            }
        }
    }

    /// A word in the 3×6 face, stepping four pixels a letter.
    fn text(&mut self, x: u32, y: u32, word: &str, rgb: [u8; 3]) {
        for (i, letter) in word.chars().enumerate() {
            let rows = tiny_glyph(letter);
            for (dy, row) in rows.iter().enumerate() {
                for dx in 0..TINY_W {
                    if row & (1 << (TINY_W - 1 - dx)) != 0 {
                        self.set(x + i as u32 * (TINY_W + 1) + dx, y + dy as u32, rgb);
                    }
                }
            }
        }
    }

    /// A word centred in a rectangle. The equaliser and playlist labels sit in
    /// sprites of their own, and Winamp centres each one in its button.
    ///
    /// A word too wide for the box is dropped rather than drawn spilling out of
    /// it: these boxes are Winamp's own sprites, and a label that runs past one
    /// lands on the sprite beside it — which is blitted somewhere else entirely,
    /// so the word would come apart on screen.
    fn centred(&mut self, x: u32, y: u32, w: u32, h: u32, word: &str, rgb: [u8; 3]) {
        let Some(text_w) = text_width(word) else {
            return;
        };
        if text_w > w || TINY_H > h {
            return;
        }
        self.text(x + (w - text_w) / 2, y + (h - TINY_H) / 2, word, rgb);
    }

    /// A sunken well: a dark panel with a light edge under it, which is what
    /// every display area in the classic skin is.
    fn well(&mut self, x: u32, y: u32, w: u32, h: u32) {
        self.fill(x, y, w, h, DISPLAY);
        self.stroke(x, y, w, h, EDGE);
    }

    fn into_bitmap(self) -> Bitmap {
        Bitmap {
            width: self.width,
            height: self.height,
            rgba: self.rgba,
        }
    }
}

/// A 3×6 face, narrow enough for the labels Winamp fits in 23 and 27 pixels.
/// [`super::pixel_text`]'s 5-wide cells would run "STEREO" to 30 pixels, one
/// past the sprite that has to hold it.
const TINY_W: u32 = 3;
const TINY_H: u32 = 6;

/// A word's width in this face: three pixels a letter and one between, so no
/// trailing gap. `None` for the empty string, which has no width to centre.
fn text_width(word: &str) -> Option<u32> {
    let letters = word.chars().count() as u32;
    (letters > 0).then(|| letters * (TINY_W + 1) - 1)
}

fn tiny_glyph(letter: char) -> [u8; 6] {
    match letter.to_ascii_uppercase() {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b010, 0b111],
        'K' => [0b101, 0b101, 0b110, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101, 0b101],
        'O' => [0b111, 0b101, 0b101, 0b101, 0b101, 0b111],
        'P' => [0b111, 0b101, 0b111, 0b100, 0b100, 0b100],
        'Q' => [0b111, 0b101, 0b101, 0b101, 0b111, 0b001],
        'R' => [0b111, 0b101, 0b111, 0b110, 0b101, 0b101],
        'S' => [0b111, 0b100, 0b111, 0b001, 0b001, 0b111],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b011, 0b001, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b001, 0b111],
        // The dB marks need a sign each, and they read at three pixels wide.
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000, 0b000],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000, 0b000],
        _ => [0; 6],
    }
}

/// Build the stock skin.
pub fn stock() -> Skin {
    super::classic::load().unwrap_or_else(|error| {
        log::error!("built-in classic skin: {error}");
        generated()
    })
}

fn generated() -> Skin {
    let mut sheets = HashMap::new();
    sheets.insert(Sheet::Main, main_sheet());
    sheets.insert(Sheet::CButtons, cbuttons_sheet());
    sheets.insert(Sheet::Titlebar, titlebar_sheet());
    sheets.insert(Sheet::Shufrep, shufrep_sheet());
    sheets.insert(Sheet::Posbar, posbar_sheet());
    sheets.insert(Sheet::Volume, slider_sheet(false));
    sheets.insert(Sheet::Balance, slider_sheet(true));
    sheets.insert(Sheet::Monoster, monoster_sheet());
    sheets.insert(Sheet::Numbers, numbers_sheet());
    sheets.insert(Sheet::Text, text_sheet());
    sheets.insert(Sheet::Playpaus, playpaus_sheet());
    sheets.insert(Sheet::EqMain, eqmain_sheet());
    sheets.insert(Sheet::EqEx, eq_ex_sheet());
    sheets.insert(Sheet::Pledit, pledit_sheet());
    Skin {
        name: "Fastcloud".to_owned(),
        sheets,
        vis_colors: vis_colors(),
        playlist: PlaylistColors {
            normal: Color32::from_rgb(200, 200, 200),
            current: Color32::from_rgb(ACCENT[0], ACCENT[1], ACCENT[2]),
            background: Color32::from_rgb(DISPLAY[0], DISPLAY[1], DISPLAY[2]),
            selected_background: Color32::from_rgb(PANEL_LIGHT[0], PANEL_LIGHT[1], PANEL_LIGHT[2]),
        },
    }
}

/// The visualiser ramp: dark to accent, then the peak and scope shades.
fn vis_colors() -> [Color32; 24] {
    let mut colors = default_vis_colors();
    colors[0] = Color32::from_rgb(DISPLAY[0], DISPLAY[1], DISPLAY[2]);
    colors[1] = Color32::from_rgb(EDGE[0], EDGE[1], EDGE[2]);
    colors[2] = Color32::from_rgb(INK[0], INK[1], INK[2]);
    // 2..=17 are the bar ramp, bottom to top in Winamp's order.
    for (i, slot) in colors[2..18].iter_mut().enumerate() {
        let t = i as f32 / 15.0;
        *slot = Color32::from_rgb(
            lerp(ACCENT_DIM[0], ACCENT[0], t),
            lerp(ACCENT_DIM[1], ACCENT[1], t),
            lerp(ACCENT_DIM[2], 160, t),
        );
    }
    // 18..=22 are the oscilloscope's shades, 23 the peak mark.
    for (i, slot) in colors[18..23].iter_mut().enumerate() {
        let t = 1.0 - i as f32 / 4.0;
        *slot = Color32::from_rgb(
            lerp(ACCENT_DIM[0], ACCENT[0], t),
            lerp(ACCENT_DIM[1], ACCENT[1], t),
            lerp(ACCENT_DIM[2], ACCENT[2], t),
        );
    }
    colors[23] = Color32::from_rgb(INK[0], INK[1], INK[2]);
    colors
}

fn lerp(from: u8, to: u8, t: f32) -> u8 {
    (f32::from(from) + (f32::from(to) - f32::from(from)) * t.clamp(0.0, 1.0)) as u8
}

/// `main.bmp`: the window, its title bar strip, the display well and the
/// holes the other sheets sit in.
///
/// The wells are sized from [`super::layout`] rather than by eye, because what
/// they have to contain is what the layout says goes there. Doing it by eye is
/// what left the visualiser's hole hanging two pixels below the display and the
/// clutter bar sunk *inside* the well it is supposed to sit beside.
fn main_sheet() -> Bitmap {
    use super::layout as l;
    let mut c = Canvas::new(super::MAIN_WIDTH, super::MAIN_HEIGHT);
    c.fill(0, 0, super::MAIN_WIDTH, super::MAIN_HEIGHT, PANEL);
    c.stroke(0, 0, super::MAIN_WIDTH, super::MAIN_HEIGHT, EDGE);

    // Title bar band.
    c.fill(0, 0, super::MAIN_WIDTH, l::SHADE_HEIGHT, PANEL_LIGHT);
    c.fill(0, l::SHADE_HEIGHT - 1, super::MAIN_WIDTH, 1, EDGE);

    // The clutter bar's own sunken strip, left of the display.
    c.fill(
        l::CLUTTER_BAR.x,
        l::CLUTTER_BAR.y,
        l::CLUTTER_BAR.width,
        l::CLUTTER_BAR.height,
        DISPLAY,
    );

    // The top display: the status mark, the time and the scrolling title. It
    // starts clear of the clutter bar and ends past the widest thing below it,
    // so the two dark panels line up on the right.
    let well_x = l::CLUTTER_BAR.right() + 2;
    let well_right = l::STEREO.right() + 1;
    let top_bottom = l::KBPS.y - 2;
    c.fill(
        well_x,
        l::CLUTTER_BAR.y,
        well_right - well_x,
        top_bottom - l::CLUTTER_BAR.y,
        DISPLAY,
    );
    c.stroke(
        well_x,
        l::CLUTTER_BAR.y,
        well_right - well_x,
        top_bottom - l::CLUTTER_BAR.y,
        EDGE,
    );

    // The row under it, holding the bitrate, the sample rate and the two
    // channel lamps. Winamp keeps these on the display's background rather
    // than on the panel, so they get a well of their own beside the
    // visualiser — one rectangle over both would have run under the volume
    // slider, which starts two pixels above where the visualiser ends.
    let info_x = l::VISUALIZER.right() + 4;
    c.fill(
        info_x,
        l::MONO.y - 1,
        well_right - info_x,
        l::MONO.height + 2,
        DISPLAY,
    );
    c.stroke(
        info_x,
        l::MONO.y - 1,
        well_right - info_x,
        l::MONO.height + 2,
        EDGE,
    );

    // The visualiser's own box, a shade darker so it reads as a screen.
    let vis = l::VISUALIZER;
    c.fill(vis.x - 1, vis.y - 1, vis.width + 2, vis.height + 2, EDGE);
    c.fill(vis.x, vis.y, vis.width, vis.height, [8, 8, 8]);

    // The transport well, wrapping the button row and the wordmark.
    let row_top = l::PREVIOUS.y - 4;
    c.fill(
        l::CLUTTER_BAR.x,
        row_top,
        well_right - l::CLUTTER_BAR.x,
        super::MAIN_HEIGHT - row_top - 1,
        PANEL,
    );
    c.fill(
        l::CLUTTER_BAR.x,
        row_top,
        well_right - l::CLUTTER_BAR.x,
        1,
        EDGE,
    );
    c.into_bitmap()
}

/// `cbuttons.bmp`: five transport buttons plus eject, released then pressed.
fn cbuttons_sheet() -> Bitmap {
    let mut c = Canvas::new(136, 36);
    for (row, pressed) in [(0, false), (18, true)] {
        // prev, play, pause, stop are 23 wide; next is 22.
        let widths = [23, 23, 23, 23, 22];
        let mut x = 0;
        for (i, w) in widths.iter().enumerate() {
            c.button(x, row, *w, 18, pressed);
            let cx = x + w / 2;
            let cy = row + 9;
            let ink = if pressed { ACCENT } else { INK };
            match i {
                0 => {
                    // |◀◀
                    c.triangle(cx + 2, cy, 4, false, ink);
                    c.triangle(cx + 6, cy, 4, false, ink);
                    c.fill(cx - 5, cy - 4, 1, 9, ink);
                }
                1 => c.triangle(cx + 2, cy, 5, true, ink),
                2 => {
                    c.fill(cx - 3, cy - 4, 2, 9, ink);
                    c.fill(cx + 1, cy - 4, 2, 9, ink);
                }
                3 => c.fill(cx - 4, cy - 4, 8, 9, ink),
                _ => {
                    // ▶▶|
                    c.triangle(cx - 2, cy, 4, true, ink);
                    c.triangle(cx - 6, cy, 4, true, ink);
                    c.fill(cx + 4, cy - 4, 1, 9, ink);
                }
            }
            x += w;
        }
    }
    // Eject is 16 tall, so its own two rows sit at y=0 and y=16 — not on the
    // 18-pixel grid the others use. Drawn after them for that reason.
    for (row, pressed) in [(0, false), (16, true)] {
        c.button(114, row, 22, 16, pressed);
        let ink = if pressed { ACCENT } else { INK };
        // An eject glyph: a triangle over a line.
        c.triangle(125, row + 6, 4, false, ink);
        c.fill(121, row + 11, 9, 2, ink);
    }
    c.into_bitmap()
}

/// `titlebar.bmp`: the bar in both modes, the three buttons, the clutter bar
/// and shade mode's little seek groove.
///
/// Winamp's own sheet is 344×87 with the pieces scattered across it; the
/// coordinates in [`super::sprites`] are the format, so this fills those exact
/// rectangles and leaves the rest transparent.
fn titlebar_sheet() -> Bitmap {
    use super::sprites as s;
    let mut c = Canvas::new(344, 87);

    // The bar, unrolled and rolled up. The two are the same band; the rolled
    // one carries the whole window, so it gets the outer border too.
    for sprite in [s::TITLE_BAR, s::SHADE_BAR] {
        c.fill(sprite.x, sprite.y, sprite.w, sprite.h, PANEL_LIGHT);
        c.fill(sprite.x, sprite.y + sprite.h - 1, sprite.w, 1, EDGE);
    }
    // Shade mode's display well, behind the title and the time.
    c.fill(s::SHADE_BAR.x + 18, s::SHADE_BAR.y + 2, 141, 10, DISPLAY);

    // The three title-bar buttons, released above pressed.
    for (sprite, down, glyph) in [
        (s::OPTIONS, s::OPTIONS_DOWN, Glyph::Cloud),
        (s::MINIMIZE, s::MINIMIZE_DOWN, Glyph::Minimize),
        (s::SHADE, s::SHADE_DOWN, Glyph::Shade),
        (s::UNSHADE, s::UNSHADE_DOWN, Glyph::Unshade),
        (s::CLOSE, s::CLOSE_DOWN, Glyph::Close),
    ] {
        for (sprite, pressed) in [(sprite, false), (down, true)] {
            c.button(sprite.x, sprite.y, sprite.w, sprite.h, pressed);
            let ink = if pressed { ACCENT } else { INK };
            let (x, y) = (sprite.x, sprite.y);
            match glyph {
                // A cloud, for the app this is a skin of.
                Glyph::Cloud => {
                    c.fill(x + 2, y + 5, 5, 2, ink);
                    c.fill(x + 3, y + 3, 3, 2, ink);
                }
                // Winamp's minimize is a line along the bottom.
                Glyph::Minimize => c.fill(x + 2, y + 6, 5, 1, ink),
                // Shade rolls up: a line at the top.
                Glyph::Shade => c.fill(x + 2, y + 2, 5, 1, ink),
                // Unshade rolls back down: a line at the top and a box under.
                Glyph::Unshade => {
                    c.fill(x + 2, y + 2, 5, 1, ink);
                    c.stroke(x + 2, y + 4, 5, 3, ink);
                }
                Glyph::Close => {
                    for i in 0..5 {
                        c.set(x + 2 + i, y + 2 + i, ink);
                        c.set(x + 6 - i, y + 2 + i, ink);
                    }
                }
            }
        }
    }

    // The clutter bar and its five lamps. The strip is a sunken panel; a lit
    // lamp is the letter in the accent colour.
    let bar = s::CLUTTER_BAR;
    c.fill(bar.x, bar.y, bar.w, bar.h, PANEL);
    c.stroke(bar.x, bar.y, bar.w, bar.h, EDGE);
    for (sprite, letter) in [
        (s::CLUTTER_O_ON, 'O'),
        (s::CLUTTER_A_ON, 'A'),
        (s::CLUTTER_I_ON, 'I'),
        (s::CLUTTER_D_ON, 'D'),
        (s::CLUTTER_V_ON, 'V'),
    ] {
        c.fill(sprite.x, sprite.y, sprite.w, sprite.h, PANEL);
        c.text(sprite.x + 2, sprite.y + 1, &letter.to_string(), ACCENT);
    }

    // Shade mode's seek bar: a groove and a three-pixel thumb, plus the two
    // end caps Winamp swaps in at the extremes.
    let groove = s::SHADE_POSBAR;
    c.fill(groove.x, groove.y, groove.w, groove.h, PANEL);
    c.fill(groove.x, groove.y + 3, groove.w, 1, EDGE);
    for sprite in [
        s::SHADE_POSBAR_THUMB_LEFT,
        s::SHADE_POSBAR_THUMB,
        s::SHADE_POSBAR_THUMB_RIGHT,
    ] {
        c.fill(sprite.x, sprite.y, sprite.w, sprite.h, ACCENT);
    }
    c.into_bitmap()
}

/// Which mark a title-bar button carries.
#[derive(Clone, Copy)]
enum Glyph {
    Cloud,
    Minimize,
    Shade,
    Unshade,
    Close,
}

/// `shufrep.bmp`: repeat (28×15) then shuffle (47×15) in four states each,
/// and below them the EQ and playlist buttons (23×12).
fn shufrep_sheet() -> Bitmap {
    use super::sprites as s;
    let mut c = Canvas::new(92, 85);
    // Winamp's rows are off, off-pressed, on, on-pressed.
    for (row, on, pressed) in [
        (0, false, false),
        (15, false, true),
        (30, true, false),
        (45, true, true),
    ] {
        let ink = if on { ACCENT } else { [120, 120, 120] };
        c.button(0, row, 28, 15, pressed);
        // Two arrows for repeat.
        c.fill(6, row + 6, 16, 2, ink);
        c.triangle(21, row + 7, 3, true, ink);
        c.button(28, row, 47, 15, pressed);
        // Crossing lines for shuffle.
        for i in 0..16u32 {
            c.set(34 + i, row + 4 + i / 4, ink);
            c.set(34 + i, row + 10 - i / 4, ink);
        }
        c.triangle(56, row + 5, 3, true, ink);
        c.triangle(56, row + 10, 3, true, ink);
    }
    // The two window buttons, off/on × released/pressed, labelled.
    for (sprite, label, on, pressed) in [
        (s::EQ, "EQ", false, false),
        (s::EQ_ON, "EQ", true, false),
        (s::EQ_DOWN, "EQ", false, true),
        (s::EQ_ON_DOWN, "EQ", true, true),
        (s::PLAYLIST, "PL", false, false),
        (s::PLAYLIST_ON, "PL", true, false),
        (s::PLAYLIST_DOWN, "PL", false, true),
        (s::PLAYLIST_ON_DOWN, "PL", true, true),
    ] {
        c.button(sprite.x, sprite.y, sprite.w, sprite.h, pressed);
        let ink = if on { ACCENT } else { [140, 140, 140] };
        c.text(sprite.x + 6, sprite.y + 3, label, ink);
    }
    c.into_bitmap()
}

/// `posbar.bmp`: the groove, then two thumb states.
fn posbar_sheet() -> Bitmap {
    let mut c = Canvas::new(307, 10);
    c.fill(0, 0, 248, 10, PANEL);
    c.fill(0, 4, 248, 2, EDGE);
    for (x, pressed) in [(248, false), (278, true)] {
        c.button(x, 0, 29, 10, pressed);
        let ink = if pressed { ACCENT } else { INK };
        c.fill(x + 13, 2, 3, 6, ink);
    }
    c.into_bitmap()
}

/// `volume.bmp` and `balance.bmp`: twenty-eight 13px rows, then the two thumbs
/// at y=422.
///
/// The two sheets are the same shape and differ only in what the rows mean —
/// volume fills from the left, balance from the middle out — so one generator
/// makes both. Balance's art starts nine pixels in, as Winamp's does.
fn slider_sheet(balance: bool) -> Bitmap {
    use super::sprites as s;
    let mut c = Canvas::new(68, 433);
    let inset = if balance { 9 } else { 0 };
    let width = if balance {
        super::layout::BALANCE.width
    } else {
        super::layout::VOLUME.width
    };
    for row in 0..s::VOLUME_ROWS {
        let y = row * s::VOLUME_ROW_STRIDE;
        let t = row as f32 / (s::VOLUME_ROWS - 1) as f32;
        c.fill(0, y, 68, s::VOLUME_ROW_H, PANEL);
        let colour = [
            lerp(ACCENT_DIM[0], ACCENT[0], t),
            lerp(ACCENT_DIM[1], ACCENT[1], t),
            lerp(ACCENT_DIM[2], ACCENT[2], t),
        ];
        if balance {
            // Row 0 is centre, so the mark grows out from the middle: the
            // strip shows *how far off* centre the sound is, not which way.
            let half = ((t * (width as f32 / 2.0 - 2.0)) as u32).max(1);
            let mid = inset + width / 2;
            c.fill(mid - half, y + 4, half * 2, 5, colour);
        } else {
            // A ramp that gets longer and warmer with the level.
            let filled = ((t * (width as f32 - 8.0)) as u32).max(1);
            c.fill(inset + 2, y + 4, filled, 5, colour);
        }
    }
    for (x, pressed) in [(0, true), (15, false)] {
        c.button(x, 422, 14, 11, pressed);
        let ink = if pressed { ACCENT } else { INK };
        c.fill(x + 6, 424, 2, 7, ink);
    }
    c.into_bitmap()
}

/// `monoster.bmp`: MONO and STEREO, lit on the top row and dim below.
fn monoster_sheet() -> Bitmap {
    use super::sprites as s;
    let mut c = Canvas::new(56, 24);
    for (sprite, label, on) in [
        (s::STEREO_ON, "STEREO", true),
        (s::STEREO, "STEREO", false),
        (s::MONO_ON, "MONO", true),
        (s::MONO, "MONO", false),
    ] {
        let ink = if on { ACCENT } else { [70, 70, 70] };
        c.fill(sprite.x, sprite.y, sprite.w, sprite.h, DISPLAY);
        // Centred in its own sprite: STEREO is six letters in 29 pixels.
        let text_w = label.chars().count() as u32 * (TINY_W + 1) - 1;
        let x = sprite.x + (sprite.w.saturating_sub(text_w)) / 2;
        c.text(x, sprite.y + 3, label, ink);
    }
    c.into_bitmap()
}

/// `numbers.bmp`: eleven 9×13 cells, `0`–`9` then blank, and the minus sign
/// Winamp borrows five pixels of at (20, 6).
fn numbers_sheet() -> Bitmap {
    let mut c = Canvas::new(99, 13);
    for digit in 0..10u32 {
        let x = digit * 9;
        c.fill(x, 0, 9, 13, DISPLAY);
        paint_digit(&mut c, x, 0, digit as u8);
    }
    // The eleventh cell stays blank: Winamp draws it for a leading zero.
    c.fill(90, 0, 9, 13, DISPLAY);
    // The countdown's minus, inside the `2` cell as the format has it. The
    // digit is drawn already, so this sits in its middle bar — which is where
    // Winamp's own sheet keeps it.
    let minus = super::sprites::MINUS;
    c.fill(minus.x, minus.y, minus.w, minus.h, ACCENT);
    c.into_bitmap()
}

/// A seven-segment digit in a 9×13 cell.
fn paint_digit(c: &mut Canvas, x: u32, y: u32, digit: u8) {
    // segments: top, top-left, top-right, middle, bottom-left, bottom-right, bottom
    const SEGMENTS: [[bool; 7]; 10] = [
        [true, true, true, false, true, true, true],     // 0
        [false, false, true, false, false, true, false], // 1
        [true, false, true, true, true, false, true],    // 2
        [true, false, true, true, false, true, true],    // 3
        [false, true, true, true, false, true, false],   // 4
        [true, true, false, true, false, true, true],    // 5
        [true, true, false, true, true, true, true],     // 6
        [true, false, true, false, false, true, false],  // 7
        [true, true, true, true, true, true, true],      // 8
        [true, true, true, true, false, true, true],     // 9
    ];
    let on = SEGMENTS[digit as usize % 10];
    let ink = ACCENT;
    let (w, h) = (7u32, 11u32);
    let (x, y) = (x + 1, y + 1);
    if on[0] {
        c.fill(x + 1, y, w - 2, 2, ink);
    }
    if on[3] {
        c.fill(x + 1, y + h / 2 - 1, w - 2, 2, ink);
    }
    if on[6] {
        c.fill(x + 1, y + h - 2, w - 2, 2, ink);
    }
    if on[1] {
        c.fill(x, y + 1, 2, h / 2 - 1, ink);
    }
    if on[2] {
        c.fill(x + w - 2, y + 1, 2, h / 2 - 1, ink);
    }
    if on[4] {
        c.fill(x, y + h / 2, 2, h / 2 - 1, ink);
    }
    if on[5] {
        c.fill(x + w - 2, y + h / 2, 2, h / 2 - 1, ink);
    }
}

/// `text.bmp`: Winamp's 5×6 bitmap font, three rows of thirty-one cells.
/// The stock skin paints a readable block per character rather than art.
fn text_sheet() -> Bitmap {
    let mut c = Canvas::new(155, 18);
    c.fill(0, 0, 155, 18, DISPLAY);
    // Row 0: A–Z then punctuation; the glyphs are drawn by `pixel_text`, so
    // only the background is needed here. Kept for layout compatibility.
    c.into_bitmap()
}

/// `playpaus.bmp`: the three state marks, and the work indicator at x=39.
fn playpaus_sheet() -> Bitmap {
    let mut c = Canvas::new(42, 9);
    c.fill(0, 0, 42, 9, DISPLAY);
    c.triangle(4, 4, 3, true, ACCENT);
    c.fill(10, 1, 2, 7, ACCENT);
    c.fill(14, 1, 2, 7, ACCENT);
    c.fill(19, 2, 6, 6, ACCENT);
    // The loading sliver: three pixels wide, as `sprites::WORKING` crops it.
    c.fill(39, 0, 3, 9, ACCENT);
    c.into_bitmap()
}

/// `eqmain.bmp`: the equaliser window, 275×315.
///
/// The sheet is far taller than the window because the band track has
/// twenty-eight states — a 14×2 grid from y=164 down — and the graph and its
/// colour column live below them at y=294. Every rectangle here comes from
/// [`super::sprites`] or [`super::layout::eq`]; the height is what the lowest
/// sprite needs, not a round number.
fn eqmain_sheet() -> Bitmap {
    use super::layout::eq as l;
    use super::sprites as s;
    let mut c = Canvas::new(275, 315);

    // ===== The window background, at (0, 0) =====
    c.fill(0, 0, l::WIDTH, l::HEIGHT, PANEL);
    c.stroke(0, 0, l::WIDTH, l::HEIGHT, EDGE);
    // The title bar band is part of the background as well as being its own
    // sprite, because a skin may dim one and not the other.
    c.fill(0, 0, l::WIDTH, l::SHADE_HEIGHT, PANEL_LIGHT);
    c.fill(0, l::SHADE_HEIGHT - 1, l::WIDTH, 1, EDGE);
    // The well the graph sits in, one pixel proud of it on every side.
    c.well(
        l::GRAPH.x - 1,
        l::GRAPH.y - 1,
        l::GRAPH.width + 2,
        l::GRAPH.height + 2,
    );
    // The band row's own recess. It starts one pixel clear of the graph's well
    // — the two are neighbours, and overlapping them ate the well's bottom edge
    // — and runs from the preamp to the last band. The band sprites cover it, so
    // all that ever shows is the four-pixel gap between each pair, which is
    // exactly what has to read as a gap.
    let bands_right = l::band(l::BANDS - 1).right();
    let recess_x = l::PREAMP.x - 3;
    let recess_w = bands_right - l::PREAMP.x + 6;
    c.fill(
        recess_x,
        l::BAND_TOP - 1,
        recess_w,
        l::BAND_HEIGHT + 2,
        [18, 18, 18],
    );
    c.fill(recess_x, l::BAND_TOP - 1, recess_w, 1, EDGE);
    // The three dB marks, painted into the background: Winamp has no sprite
    // for them, so a skin styles them by styling the window.
    for (mark, label) in [
        (l::PLUS_12DB, "+12"),
        (l::ZERO_DB, "0"),
        (l::MINUS_12DB, "-12"),
    ] {
        c.centred(
            mark.x,
            mark.y,
            mark.width,
            mark.height,
            label,
            [130, 130, 130],
        );
    }

    // ===== The title bar, as its own sprite at y=134 =====
    let bar = s::EQ_TITLE_BAR;
    c.fill(bar.x, bar.y, bar.w, bar.h, PANEL_LIGHT);
    c.fill(bar.x, bar.y + bar.h - 1, bar.w, 1, EDGE);
    c.centred(bar.x + 8, bar.y, 60, bar.h, "EQUALIZER", INK);

    // ===== The two title-bar buttons =====
    // Close is a cross; shade is a line at the top, as the main window's is.
    for (sprite, pressed) in [(s::EQ_CLOSE, false), (s::EQ_CLOSE_DOWN, true)] {
        c.button(sprite.x, sprite.y, sprite.w, sprite.h, pressed);
        let ink = if pressed { ACCENT } else { INK };
        for i in 0..5 {
            c.set(sprite.x + 2 + i, sprite.y + 2 + i, ink);
            c.set(sprite.x + 6 - i, sprite.y + 2 + i, ink);
        }
    }
    let shade = s::EQ_SHADE;
    c.button(shade.x, shade.y, shade.w, shade.h, false);
    c.fill(shade.x + 2, shade.y + 2, 5, 1, INK);

    // ===== ON, AUTO and PRESETS =====
    // Winamp keeps the lit and unlit states side by side, and the pressed pair
    // 118 pixels to the right of each. Lit is the accent; unlit is grey.
    for (sprite, label, on, pressed) in [
        (s::EQ_DISABLED, "ON", false, false),
        (s::EQ_DISABLED_DOWN, "ON", false, true),
        (s::EQ_ENABLED, "ON", true, false),
        (s::EQ_ENABLED_DOWN, "ON", true, true),
        (s::EQ_AUTO, "AUTO", false, false),
        (s::EQ_AUTO_DOWN, "AUTO", false, true),
        (s::EQ_AUTO_ON, "AUTO", true, false),
        (s::EQ_AUTO_ON_DOWN, "AUTO", true, true),
        (s::EQ_PRESETS, "PRESETS", false, false),
        (s::EQ_PRESETS_DOWN, "PRESETS", false, true),
    ] {
        c.button(sprite.x, sprite.y, sprite.w, sprite.h, pressed);
        let ink = if on {
            ACCENT
        } else if pressed {
            INK
        } else {
            [140, 140, 140]
        };
        c.centred(sprite.x, sprite.y, sprite.w, sprite.h, label, ink);
    }

    // ===== The band track, twenty-eight states =====
    // Step 0 is the bottom of the range and step 27 the top; the track's own
    // art brightens with it, which is what makes a raised band read as raised
    // even with the thumb covering the middle.
    for step in 0..s::EQ_BAND_STEPS {
        let sprite = s::eq_band(step);
        let t = step as f32 / (s::EQ_BAND_STEPS - 1) as f32;
        c.fill(sprite.x, sprite.y, sprite.w, sprite.h, PANEL);
        // The groove the thumb slides in, down the middle of the cell.
        let groove_x = sprite.x + sprite.w / 2 - 1;
        c.fill(groove_x, sprite.y + 1, 2, sprite.h - 2, DISPLAY);
        // …and the part of it below the thumb, lit to the band's own colour.
        let travel = l::THUMB_TRAVEL;
        let from_top = ((1.0 - t) * travel as f32).round() as u32;
        let lit_top = sprite.y + from_top + l::THUMB_HEIGHT / 2;
        let colour = [
            lerp(ACCENT_DIM[0], ACCENT[0], t),
            lerp(ACCENT_DIM[1], ACCENT[1], t),
            lerp(ACCENT_DIM[2], ACCENT[2], t),
        ];
        c.fill(
            groove_x,
            lit_top,
            2,
            (sprite.y + sprite.h - 1).saturating_sub(lit_top),
            colour,
        );
    }

    // ===== The thumb, released and pressed =====
    for (sprite, pressed) in [(s::EQ_THUMB, false), (s::EQ_THUMB_DOWN, true)] {
        c.button(sprite.x, sprite.y, sprite.w, sprite.h, pressed);
        let ink = if pressed { ACCENT } else { INK };
        // A grip line across the middle, as every Winamp thumb has.
        c.fill(sprite.x + 2, sprite.y + sprite.h / 2, sprite.w - 4, 1, ink);
    }

    // ===== The graph, its colour column and the preamp line =====
    let graph = s::EQ_GRAPH;
    c.fill(graph.x, graph.y, graph.w, graph.h, DISPLAY);
    // A centre line, so a flat curve is visibly flat rather than invisible.
    c.fill(graph.x, graph.y + graph.h / 2, graph.w, 1, [30, 30, 30]);
    // Nineteen rows of colour, top to bottom: the curve takes its colour from
    // the row it is on, which is how Winamp gets a graded line for free.
    let colours = s::EQ_GRAPH_COLORS;
    for row in 0..colours.h {
        let t = row as f32 / (colours.h - 1) as f32;
        c.fill(
            colours.x,
            colours.y + row,
            colours.w,
            1,
            [
                lerp(ACCENT[0], ACCENT_DIM[0], t),
                lerp(160, ACCENT_DIM[1], t),
                lerp(60, ACCENT_DIM[2], t),
            ],
        );
    }
    let preamp = s::EQ_PREAMP_LINE;
    c.fill(preamp.x, preamp.y, preamp.w, preamp.h, [120, 120, 120]);
    c.into_bitmap()
}

/// `pledit.bmp`: the playlist window's nine-piece frame, its buttons and its
/// bottom strip, 280×110.
///
/// The frame pieces tile, so the window can be any size: two corners and a
/// repeating tile across the top, a strip down each side, and two wide corners
/// along the bottom. The five menu labels and the transport glyphs are painted
/// *into* the bottom corners rather than being sprites of their own, because
/// that is where Winamp keeps them — the renderer only owns the hit areas.
fn pledit_sheet() -> Bitmap {
    use super::layout::pl as l;
    use super::sprites as s;
    let mut c = Canvas::new(280, 110);

    // ===== The title bar, focused on the top row and dim below =====
    for (row, focused) in [(0u32, true), (1, false)] {
        let (face, ink) = if focused {
            (PANEL_LIGHT, INK)
        } else {
            (PANEL, [120, 120, 120])
        };
        let (left, tile, title, right) = if focused {
            (
                s::PL_TOP_LEFT,
                s::PL_TOP_TILE,
                s::PL_TOP_TITLE,
                s::PL_TOP_RIGHT,
            )
        } else {
            (
                s::PL_TOP_LEFT_DIM,
                s::PL_TOP_TILE_DIM,
                s::PL_TOP_TITLE_DIM,
                s::PL_TOP_RIGHT_DIM,
            )
        };
        let _ = row;
        for piece in [left, tile, title, right] {
            c.fill(piece.x, piece.y, piece.w, piece.h, face);
            // The bar's own top highlight and the line under it.
            c.fill(piece.x, piece.y, piece.w, 1, EDGE);
            c.fill(piece.x, piece.y + piece.h - 1, piece.w, 1, EDGE);
        }
        // The corners carry the window's outer edge.
        c.fill(left.x, left.y, 1, left.h, EDGE);
        c.fill(right.x + right.w - 1, right.y, 1, right.h, EDGE);
        c.centred(title.x, title.y, title.w, title.h, "PLAYLIST", ink);
    }

    // ===== The sides, which tile down =====
    let side = s::PL_LEFT_TILE;
    c.fill(side.x, side.y, side.w, side.h, PANEL);
    c.fill(side.x, side.y, 1, side.h, EDGE);
    let right = s::PL_RIGHT_TILE;
    c.fill(right.x, right.y, right.w, right.h, PANEL);
    c.fill(right.x + right.w - 1, right.y, 1, right.h, EDGE);
    // The scroll bar's groove is part of the right strip: the layout puts the
    // bar five pixels in from the frame's left edge, so the groove goes there.
    c.fill(right.x + 5, right.y, l::SCROLLBAR_W, right.h, DISPLAY);

    // ===== The title-bar buttons and the scroll handle =====
    // Winamp's playlist buttons ship only their pressed state; the released one
    // is the bar behind them, which is why each is drawn pressed.
    for (sprite, glyph) in [
        (s::PL_CLOSE, PlGlyph::Close),
        (s::PL_SHADE, PlGlyph::Shade),
        (s::PL_UNSHADE, PlGlyph::Unshade),
    ] {
        c.button(sprite.x, sprite.y, sprite.w, sprite.h, true);
        let (x, y) = (sprite.x, sprite.y);
        match glyph {
            PlGlyph::Close => {
                for i in 0..5 {
                    c.set(x + 2 + i, y + 2 + i, ACCENT);
                    c.set(x + 6 - i, y + 2 + i, ACCENT);
                }
            }
            PlGlyph::Shade => c.fill(x + 2, y + 2, 5, 1, ACCENT),
            PlGlyph::Unshade => {
                c.fill(x + 2, y + 2, 5, 1, ACCENT);
                c.stroke(x + 2, y + 4, 5, 3, ACCENT);
            }
        }
    }
    for (sprite, pressed) in [
        (s::PL_SCROLL_HANDLE, false),
        (s::PL_SCROLL_HANDLE_DOWN, true),
    ] {
        c.button(sprite.x, sprite.y, sprite.w, sprite.h, pressed);
        let ink = if pressed { ACCENT } else { INK };
        c.fill(sprite.x + 2, sprite.y + sprite.h / 2, sprite.w - 4, 1, ink);
    }

    // ===== Rolled up: three pieces so the bar can be any width =====
    for (piece, cap) in [
        (s::PL_SHADE_LEFT, true),
        (s::PL_SHADE_TILE, false),
        (s::PL_SHADE_RIGHT, true),
    ] {
        c.fill(piece.x, piece.y, piece.w, piece.h, PANEL_LIGHT);
        c.stroke(piece.x, piece.y, piece.w, piece.h, EDGE);
        if !cap {
            // The tile has no left or right edge: it repeats.
            c.fill(piece.x, piece.y + 1, 1, piece.h - 2, PANEL_LIGHT);
            c.fill(
                piece.x + piece.w - 1,
                piece.y + 1,
                1,
                piece.h - 2,
                PANEL_LIGHT,
            );
        }
    }
    // The label has to fit the left cap's own 25 pixels. "PLAYLIST" is 31 wide
    // in this face, so it would run onto the sprite beside it — which is blitted
    // at the *other* end of the window, and the word would come apart.
    c.centred(
        s::PL_SHADE_LEFT.x,
        s::PL_SHADE_LEFT.y,
        s::PL_SHADE_LEFT.w,
        s::PL_SHADE_LEFT.h,
        "PL",
        INK,
    );

    // ===== The bottom strip's left corner =====
    let bl = s::PL_BOTTOM_LEFT;
    c.fill(bl.x, bl.y, bl.w, bl.h, PANEL);
    c.fill(bl.x, bl.y, bl.w, 1, EDGE);
    c.fill(bl.x, bl.y, 1, bl.h, EDGE);
    c.fill(bl.x, bl.y + bl.h - 1, bl.w, 1, EDGE);

    // ===== The bottom strip's right corner: time, transport, grip =====
    let br = s::PL_BOTTOM_RIGHT;
    c.fill(br.x, br.y, br.w, br.h, PANEL);
    c.fill(br.x, br.y, br.w, 1, EDGE);
    c.fill(br.x + br.w - 1, br.y, 1, br.h, EDGE);
    c.fill(br.x, br.y + br.h - 1, br.w, 1, EDGE);

    // Everything on the strip is placed from the layout, converted from window
    // coordinates into whichever piece carries it: the left corner is anchored
    // to x=0 and the right one to the window's right edge, so the two need
    // different arithmetic. Placing by eye is what would drift the labels off
    // their own hit areas.
    let (w, h) = (l::MIN_WIDTH, l::height(l::DEFAULT_ROWS));
    let strip_top = h - l::BOTTOM_HEIGHT;
    let on_left = |area: super::layout::Area| (bl.x + area.x, bl.y + area.y - strip_top);
    let on_right =
        |area: super::layout::Area| (br.x + area.x - (w - br.w), br.y + area.y - strip_top);

    // The five menu labels. Four sit on the left corner and LIST on the right,
    // which is why Winamp anchors it to the opposite edge.
    for (index, label) in l::MENU_LABELS.iter().enumerate() {
        let area = l::menu(w, h, index);
        let (x, y) = if area.right() <= bl.w {
            on_left(area)
        } else {
            on_right(area)
        };
        c.centred(x, y, area.width, area.height, label, [150, 150, 150]);
    }

    let time = l::running_time(w, h);
    let (tx, ty) = on_right(time);
    c.well(tx - 2, ty - 2, time.width + 4, time.height + 4);
    for index in 0..l::ACTIONS {
        let area = l::action(w, h, index);
        let (x, y) = on_right(area);
        c.button(x, y, area.width, area.height, false);
        let (cx, cy) = (x + area.width / 2, y + area.height / 2);
        match index {
            0 => {
                c.triangle(cx + 1, cy, 2, false, INK);
                c.fill(cx - 3, cy - 2, 1, 5, INK);
            }
            1 => c.triangle(cx + 1, cy, 3, true, INK),
            2 => {
                c.fill(cx - 2, cy - 2, 2, 5, INK);
                c.fill(cx + 1, cy - 2, 2, 5, INK);
            }
            3 => c.fill(cx - 2, cy - 2, 5, 5, INK),
            _ => {
                c.triangle(cx - 1, cy, 2, true, INK);
                c.fill(cx + 2, cy - 2, 1, 5, INK);
            }
        }
    }
    // The grip: three diagonal ticks in the corner, as every resizable Winamp
    // window has.
    let grip = l::resize_grip(w, h);
    let (gx, gy) = on_right(grip);
    for step in 0..3u32 {
        let inset = 3 + step * 4;
        for i in 0..4u32 {
            c.set(
                gx + grip.width - inset - i,
                gy + grip.height - 3 - i,
                [90, 90, 90],
            );
        }
    }

    // ===== The tile between the bottom corners, for a wider window =====
    let tile = s::PL_BOTTOM_TILE;
    c.fill(tile.x, tile.y, tile.w, tile.h, PANEL);
    c.fill(tile.x, tile.y, tile.w, 1, EDGE);
    c.fill(tile.x, tile.y + tile.h - 1, tile.w, 1, EDGE);

    // ===== The little visualiser's well =====
    let vis = s::PL_VIS_BACKGROUND;
    c.fill(vis.x, vis.y, vis.w, vis.h, EDGE);
    c.fill(vis.x + 1, vis.y + 1, vis.w - 2, vis.h - 2, [8, 8, 8]);
    c.into_bitmap()
}

/// Which mark a playlist title-bar button carries.
#[derive(Clone, Copy)]
enum PlGlyph {
    Close,
    Shade,
    Unshade,
}

///
/// The bar itself is at the top; the two sliders' three-pixel pieces are at
/// y=30, and the rolled-up window's own buttons below them.
fn eq_ex_sheet() -> Bitmap {
    use super::layout::eq as l;
    use super::sprites as s;
    let mut c = Canvas::new(275, 56);

    // The bar carries the whole window when it is rolled up, so it gets the
    // outer border too.
    let bar = s::EQ_SHADE_BAR;
    c.fill(bar.x, bar.y, bar.w, bar.h, PANEL_LIGHT);
    c.stroke(bar.x, bar.y, bar.w, bar.h, EDGE);
    // The label goes left of the volume slider, in the room Winamp leaves.
    c.centred(4, bar.y, l::SHADE_VOLUME.x - 6, bar.h, "EQ", INK);
    // The two grooves the sliders run in, sunk into the bar.
    for slider in [l::SHADE_VOLUME, l::SHADE_BALANCE] {
        c.well(
            slider.x - 1,
            slider.y - 1,
            slider.width + 2,
            slider.height + 2,
        );
    }

    // The slider pieces: a left cap, a middle, a right cap, for each of the
    // two sliders. Winamp swaps the piece for the one that matches where the
    // thumb sits, which is what makes an end-stop look different.
    for (pieces, colour) in [
        (
            [
                s::EQ_SHADE_VOL_LEFT,
                s::EQ_SHADE_VOL_MID,
                s::EQ_SHADE_VOL_RIGHT,
            ],
            ACCENT,
        ),
        (
            [
                s::EQ_SHADE_BAL_LEFT,
                s::EQ_SHADE_BAL_MID,
                s::EQ_SHADE_BAL_RIGHT,
            ],
            [200, 200, 200],
        ),
    ] {
        for (index, piece) in pieces.iter().enumerate() {
            c.fill(piece.x, piece.y, piece.w, piece.h, PANEL_LIGHT);
            // The caps are dimmer, so hitting an end reads as hitting an end.
            let ink = if index == 1 { colour } else { ACCENT_DIM };
            c.fill(piece.x, piece.y + 1, piece.w, piece.h - 2, ink);
        }
    }

    // The rolled-up window's own close and unshade buttons.
    for (sprite, pressed) in [(s::EQ_SHADE_CLOSE, false), (s::EQ_SHADE_CLOSE_DOWN, true)] {
        c.button(sprite.x, sprite.y, sprite.w, sprite.h, pressed);
        let ink = if pressed { ACCENT } else { INK };
        for i in 0..5 {
            c.set(sprite.x + 2 + i, sprite.y + 2 + i, ink);
            c.set(sprite.x + 6 - i, sprite.y + 2 + i, ink);
        }
    }
    // Unshade rolls back down: a line at the top and a box under it.
    let unshade = s::EQ_SHADE_UNSHADE;
    c.button(unshade.x, unshade.y, unshade.w, unshade.h, true);
    c.fill(unshade.x + 2, unshade.y + 2, 5, 1, ACCENT);
    c.stroke(unshade.x + 2, unshade.y + 4, 5, 3, ACCENT);
    // The tall window's shade button carries its pressed state here.
    let shade_down = s::EQ_SHADE_DOWN;
    c.button(shade_down.x, shade_down.y, shade_down.w, shade_down.h, true);
    c.fill(shade_down.x + 2, shade_down.y + 2, 5, 1, ACCENT);
    c.into_bitmap()
}

#[cfg(test)]
mod tests {
    use super::generated as stock;
    use super::*;
    use crate::skin::{layout, sprites};

    #[test]
    fn the_stock_skin_has_every_sheet_the_layout_needs() {
        let skin = stock();
        assert_eq!(skin.name, "Fastcloud");
        // Every sheet but `nums_ex.bmp`, which is the optional one: without it
        // the renderer falls back to `numbers.bmp`, and that path needs
        // exercising by the skin the app ships with.
        for sheet in Sheet::ALL {
            if sheet == Sheet::NumsEx {
                assert!(!skin.has(sheet), "the stock skin is the fallback path");
                continue;
            }
            assert!(skin.sheet(sheet).is_some(), "{sheet:?} is missing");
        }
    }

    /// Every sprite must fit the sheet it names, or a crop would come back
    /// clipped and the control would paint short.
    #[test]
    fn sheets_are_big_enough_for_their_sprites() {
        let skin = stock();
        for sprite in every_sprite() {
            if sprite.sheet == Sheet::NumsEx {
                continue;
            }
            let sheet = skin
                .sheet(sprite.sheet)
                .unwrap_or_else(|| panic!("{:?} is missing", sprite.sheet));
            assert!(
                sprite.x + sprite.w <= sheet.width && sprite.y + sprite.h <= sheet.height,
                "{sprite:?} does not fit its {}×{} sheet",
                sheet.width,
                sheet.height
            );
            let image = skin.sprite(sprite).expect("crops");
            assert_eq!(image.size, [sprite.w as usize, sprite.h as usize]);
        }
    }

    /// Every sprite the renderer can ask for. Kept here rather than in
    /// `sprites` because listing them is a test's business, and a new sprite
    /// that nobody draws art for should fail this.
    fn every_sprite() -> Vec<crate::skin::Sprite> {
        vec![
            sprites::MAIN,
            sprites::MAIN_TOP,
            sprites::TITLE_BAR,
            sprites::SHADE_BAR,
            sprites::OPTIONS,
            sprites::OPTIONS_DOWN,
            sprites::MINIMIZE,
            sprites::MINIMIZE_DOWN,
            sprites::SHADE,
            sprites::SHADE_DOWN,
            sprites::UNSHADE,
            sprites::UNSHADE_DOWN,
            sprites::CLOSE,
            sprites::CLOSE_DOWN,
            sprites::CLUTTER_BAR,
            sprites::CLUTTER_O_ON,
            sprites::CLUTTER_A_ON,
            sprites::CLUTTER_I_ON,
            sprites::CLUTTER_D_ON,
            sprites::CLUTTER_V_ON,
            sprites::SHADE_POSBAR,
            sprites::SHADE_POSBAR_THUMB,
            sprites::SHADE_POSBAR_THUMB_LEFT,
            sprites::SHADE_POSBAR_THUMB_RIGHT,
            sprites::PREVIOUS,
            sprites::PREVIOUS_DOWN,
            sprites::PLAY,
            sprites::PLAY_DOWN,
            sprites::PAUSE,
            sprites::PAUSE_DOWN,
            sprites::STOP,
            sprites::STOP_DOWN,
            sprites::NEXT,
            sprites::NEXT_DOWN,
            sprites::EJECT,
            sprites::EJECT_DOWN,
            sprites::SHUFFLE,
            sprites::SHUFFLE_DOWN,
            sprites::SHUFFLE_ON,
            sprites::SHUFFLE_ON_DOWN,
            sprites::REPEAT,
            sprites::REPEAT_DOWN,
            sprites::REPEAT_ON,
            sprites::REPEAT_ON_DOWN,
            sprites::EQ,
            sprites::EQ_ON,
            sprites::EQ_DOWN,
            sprites::EQ_ON_DOWN,
            sprites::PLAYLIST,
            sprites::PLAYLIST_ON,
            sprites::PLAYLIST_DOWN,
            sprites::PLAYLIST_ON_DOWN,
            sprites::POSBAR,
            sprites::POSBAR_THUMB,
            sprites::POSBAR_THUMB_DOWN,
            sprites::VOLUME_THUMB,
            sprites::VOLUME_THUMB_DOWN,
            sprites::BALANCE_THUMB,
            sprites::BALANCE_THUMB_DOWN,
            sprites::volume_row(0),
            sprites::volume_row(sprites::VOLUME_ROWS - 1),
            sprites::balance_row(0),
            sprites::balance_row(sprites::VOLUME_ROWS - 1),
            sprites::MONO,
            sprites::MONO_ON,
            sprites::STEREO,
            sprites::STEREO_ON,
            sprites::digit(0),
            sprites::digit(sprites::BLANK_DIGIT),
            sprites::MINUS,
            sprites::PLAYING,
            sprites::PAUSED,
            sprites::STOPPED,
            sprites::WORKING,
            // eqmain.bmp
            sprites::EQ_MAIN,
            sprites::EQ_TITLE_BAR,
            sprites::EQ_CLOSE,
            sprites::EQ_CLOSE_DOWN,
            sprites::EQ_SHADE,
            sprites::EQ_ENABLED,
            sprites::EQ_ENABLED_DOWN,
            sprites::EQ_DISABLED,
            sprites::EQ_DISABLED_DOWN,
            sprites::EQ_AUTO,
            sprites::EQ_AUTO_DOWN,
            sprites::EQ_AUTO_ON,
            sprites::EQ_AUTO_ON_DOWN,
            sprites::EQ_PRESETS,
            sprites::EQ_PRESETS_DOWN,
            sprites::EQ_THUMB,
            sprites::EQ_THUMB_DOWN,
            sprites::EQ_GRAPH,
            sprites::EQ_GRAPH_COLORS,
            sprites::EQ_PREAMP_LINE,
            sprites::eq_band(0),
            sprites::eq_band(13),
            sprites::eq_band(14),
            sprites::eq_band(sprites::EQ_BAND_STEPS - 1),
            // eq_ex.bmp
            sprites::EQ_SHADE_BAR,
            sprites::EQ_SHADE_DOWN,
            sprites::EQ_SHADE_CLOSE,
            sprites::EQ_SHADE_CLOSE_DOWN,
            sprites::EQ_SHADE_UNSHADE,
            sprites::EQ_SHADE_VOL_LEFT,
            sprites::EQ_SHADE_VOL_MID,
            sprites::EQ_SHADE_VOL_RIGHT,
            sprites::EQ_SHADE_BAL_LEFT,
            sprites::EQ_SHADE_BAL_MID,
            sprites::EQ_SHADE_BAL_RIGHT,
            // pledit.bmp
            sprites::PL_TOP_LEFT,
            sprites::PL_TOP_TILE,
            sprites::PL_TOP_TITLE,
            sprites::PL_TOP_RIGHT,
            sprites::PL_TOP_LEFT_DIM,
            sprites::PL_TOP_TILE_DIM,
            sprites::PL_TOP_TITLE_DIM,
            sprites::PL_TOP_RIGHT_DIM,
            sprites::PL_LEFT_TILE,
            sprites::PL_RIGHT_TILE,
            sprites::PL_BOTTOM_LEFT,
            sprites::PL_BOTTOM_RIGHT,
            sprites::PL_BOTTOM_TILE,
            sprites::PL_VIS_BACKGROUND,
            sprites::PL_CLOSE,
            sprites::PL_SHADE,
            sprites::PL_UNSHADE,
            sprites::PL_SCROLL_HANDLE,
            sprites::PL_SCROLL_HANDLE_DOWN,
            sprites::PL_SHADE_LEFT,
            sprites::PL_SHADE_TILE,
            sprites::PL_SHADE_RIGHT,
        ]
    }

    #[test]
    fn the_main_sheet_is_the_window_size() {
        let skin = stock();
        let main = skin.sheet(Sheet::Main).unwrap();
        assert_eq!(main.width, crate::skin::MAIN_WIDTH);
        assert_eq!(main.height, crate::skin::MAIN_HEIGHT);
    }

    #[test]
    fn every_digit_paints_something() {
        let numbers = numbers_sheet();
        for digit in 0..10u32 {
            let x = digit * 9;
            let lit = (0..13).any(|y| {
                (0..9).any(|dx| {
                    let at = (((y * numbers.width) + x + dx) * 4) as usize;
                    numbers.rgba[at] > 100
                })
            });
            assert!(lit, "digit {digit} is blank");
        }
    }

    /// Whether a sprite has anything on it. A control whose art came out blank
    /// is invisible, which is the failure mode a generated skin has instead of
    /// a missing file.
    fn is_painted(sprite: crate::skin::Sprite) -> bool {
        let skin = stock();
        let image = skin.sprite(sprite).expect("crops");
        image.pixels.iter().any(|p| p.a() > 0)
    }

    /// Every control's art has to have pixels on it — otherwise the window
    /// draws a hole where a button should be, which is what the first cut of
    /// the title-bar buttons did.
    #[test]
    fn every_sprite_has_art() {
        for sprite in every_sprite() {
            if sprite.sheet == Sheet::NumsEx || sprite.sheet == Sheet::Text {
                // No `nums_ex.bmp`, and `text.bmp` is background only: the
                // marquee's glyphs come from `pixel_text`.
                continue;
            }
            if sprite == sprites::digit(sprites::BLANK_DIGIT) {
                continue;
            }
            assert!(is_painted(sprite), "{sprite:?} came out blank");
        }
    }

    /// …and every pixel of it, not just some.
    ///
    /// A generated sprite is blitted straight over the background, so one
    /// unpainted pixel inside it is a hole the window shows through. Real skins
    /// use transparency on purpose; this one has no reason to, so a gap means
    /// the generator missed a rectangle it was asked to fill.
    #[test]
    fn generated_sprites_are_fully_painted() {
        let skin = stock();
        for sprite in every_sprite() {
            if sprite.sheet == Sheet::NumsEx {
                continue;
            }
            let image = skin.sprite(sprite).expect("crops");
            let clear = image
                .pixels
                .iter()
                .enumerate()
                .find(|(_, p)| p.a() < 255)
                .map(|(i, _)| (i % sprite.w as usize, i / sprite.w as usize));
            assert_eq!(
                clear, None,
                "{sprite:?} has a see-through pixel at {clear:?}"
            );
        }
    }

    /// The window's whole background is painted, so no part of the desktop
    /// shows through a skin that is meant to be a solid rectangle. (The window
    /// is transparent by request — see `main.rs` — for skins that are not.)
    #[test]
    fn the_background_covers_the_window() {
        let skin = stock();
        let main = skin.sprite(sprites::MAIN).expect("crops");
        assert_eq!(
            main.size,
            [
                crate::skin::MAIN_WIDTH as usize,
                crate::skin::MAIN_HEIGHT as usize
            ]
        );
        assert!(
            main.pixels.iter().all(|p| p.a() == 255),
            "the window has a see-through pixel"
        );
    }

    /// Every control the layout places has art the same size to fill it, and
    /// that art sits inside the window. `sprites_and_layout_agree_on_sizes`
    /// checks the sizes; this checks the *stock skin* actually has the pixels,
    /// which is what a screenshot would be looked at for.
    #[test]
    fn every_control_in_the_window_has_art() {
        let skin = stock();
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
            ("POSITION", sprites::POSBAR, layout::POSITION),
            ("VOLUME", sprites::volume_row(0), layout::VOLUME),
            ("BALANCE", sprites::balance_row(0), layout::BALANCE),
            ("MONO", sprites::MONO, layout::MONO),
            ("STEREO", sprites::STEREO, layout::STEREO),
            ("STATUS", sprites::PLAYING, layout::STATUS),
            ("WORKING", sprites::WORKING, layout::WORK_INDICATOR),
            ("OPTIONS", sprites::OPTIONS, layout::OPTIONS_BUTTON),
            ("MINIMIZE", sprites::MINIMIZE, layout::MINIMIZE_BUTTON),
            ("SHADE", sprites::SHADE, layout::SHADE_BUTTON),
            ("CLOSE", sprites::CLOSE, layout::CLOSE_BUTTON),
            ("CLUTTER_BAR", sprites::CLUTTER_BAR, layout::CLUTTER_BAR),
            ("DIGIT", sprites::digit(8), layout::MINUTE_TENS),
            (
                "SHADE_POSITION",
                sprites::SHADE_POSBAR,
                layout::SHADE_POSITION,
            ),
        ] {
            let image = skin.sprite(sprite).expect("crops");
            assert_eq!(
                image.size,
                [area.width as usize, area.height as usize],
                "{name}: the art does not fill its slot"
            );
            assert!(
                image.pixels.iter().any(|p| p.a() > 0),
                "{name} is a hole in the window"
            );
            assert!(
                area.right() <= crate::skin::MAIN_WIDTH
                    && area.bottom() <= crate::skin::MAIN_HEIGHT,
                "{name} hangs off the window"
            );
        }
    }

    /// The two states of a button must differ, or a press shows nothing.
    #[test]
    fn pressed_buttons_look_different() {
        let skin = stock();
        for (name, up, down) in [
            ("previous", sprites::PREVIOUS, sprites::PREVIOUS_DOWN),
            ("play", sprites::PLAY, sprites::PLAY_DOWN),
            ("pause", sprites::PAUSE, sprites::PAUSE_DOWN),
            ("stop", sprites::STOP, sprites::STOP_DOWN),
            ("next", sprites::NEXT, sprites::NEXT_DOWN),
            ("eject", sprites::EJECT, sprites::EJECT_DOWN),
            ("shuffle", sprites::SHUFFLE, sprites::SHUFFLE_ON),
            ("repeat", sprites::REPEAT, sprites::REPEAT_ON),
            ("eq", sprites::EQ, sprites::EQ_ON),
            ("playlist", sprites::PLAYLIST, sprites::PLAYLIST_ON),
            ("options", sprites::OPTIONS, sprites::OPTIONS_DOWN),
            ("minimize", sprites::MINIMIZE, sprites::MINIMIZE_DOWN),
            ("shade", sprites::SHADE, sprites::UNSHADE),
            ("close", sprites::CLOSE, sprites::CLOSE_DOWN),
            ("mono", sprites::MONO, sprites::MONO_ON),
            ("stereo", sprites::STEREO, sprites::STEREO_ON),
        ] {
            let a = skin.sprite(up).expect("crops");
            let b = skin.sprite(down).expect("crops");
            assert_ne!(a.pixels, b.pixels, "{name} looks the same either way");
        }
    }

    /// The balance strip marks the middle at centre and both ends at the
    /// extremes, which is the whole difference from the volume strip.
    #[test]
    fn the_balance_strip_grows_from_its_middle() {
        let sheet = slider_sheet(true);
        let lit_at = |row: u32, x: u32| {
            let y = row * crate::skin::sprites::VOLUME_ROW_STRIDE + 6;
            let at = (((y * sheet.width) + x) * 4) as usize;
            sheet.rgba[at] > 60
        };
        let middle = 9 + crate::skin::layout::BALANCE.width / 2;
        assert!(lit_at(0, middle), "centre is marked on the first row");
        assert!(!lit_at(0, 12), "the first row is not a full bar");
        let last = crate::skin::sprites::VOLUME_ROWS - 1;
        assert!(lit_at(last, middle - 8), "the last row reaches out left");
        assert!(lit_at(last, middle + 8), "…and right");
    }

    /// Every label the generated sheets carry has to fit the sprite it is
    /// centred in, and every letter of it has to have a glyph.
    ///
    /// [`Canvas::centred`] drops a word that does not fit rather than letting it
    /// spill onto the sprite next door — which is right, but silent, so a label
    /// added without a glyph or a couple of pixels too wide would simply not
    /// appear. This is the check that says so.
    #[test]
    fn every_label_fits_its_sprite_and_has_glyphs() {
        for (name, word, sprite) in [
            ("EQ ON", "ON", sprites::EQ_DISABLED),
            ("EQ AUTO", "AUTO", sprites::EQ_AUTO),
            ("EQ PRESETS", "PRESETS", sprites::EQ_PRESETS),
            ("EQ shade", "EQ", sprites::EQ_SHADE_BAR),
            ("PL title", "PLAYLIST", sprites::PL_TOP_TITLE),
            ("PL shade", "PL", sprites::PL_SHADE_LEFT),
            ("ADD", "ADD", sprites::PL_BOTTOM_LEFT),
            ("MISC", "MISC", sprites::PL_BOTTOM_LEFT),
            ("LIST", "LIST", sprites::PL_BOTTOM_RIGHT),
        ] {
            let width = text_width(word).expect("a label is not empty");
            assert!(
                width <= sprite.w && TINY_H <= sprite.h,
                "{name}: \"{word}\" is {width}×{TINY_H} in a {}×{} sprite",
                sprite.w,
                sprite.h
            );
            for letter in word.chars() {
                assert_ne!(
                    tiny_glyph(letter),
                    [0; 6],
                    "{name}: '{letter}' has no glyph, so \"{word}\" reads with a hole"
                );
            }
        }
        // The menu labels have their own boxes, and those are the tight ones:
        // four letters in 22 pixels.
        for (index, label) in crate::skin::layout::pl::MENU_LABELS.iter().enumerate() {
            let width = text_width(label).expect("a menu label is not empty");
            assert!(
                width <= crate::skin::layout::pl::MENU_W,
                "menu {index} (\"{label}\") is {width} wide in a {} box",
                crate::skin::layout::pl::MENU_W
            );
            for letter in label.chars() {
                assert_ne!(tiny_glyph(letter), [0; 6], "'{letter}' has no glyph");
            }
        }
        // The dB marks, which are the only labels with a sign in them.
        for (mark, label) in [
            (crate::skin::layout::eq::PLUS_12DB, "+12"),
            (crate::skin::layout::eq::ZERO_DB, "0"),
            (crate::skin::layout::eq::MINUS_12DB, "-12"),
        ] {
            let width = text_width(label).expect("a dB mark is not empty");
            assert!(width <= mark.width, "\"{label}\" does not fit its mark");
            for letter in label.chars() {
                assert_ne!(tiny_glyph(letter), [0; 6], "'{letter}' has no glyph");
            }
        }
    }

    /// …and a word that does not fit is dropped whole, not clipped.
    #[test]
    fn an_oversized_label_is_dropped() {
        let mut c = Canvas::new(40, 10);
        c.centred(0, 0, 10, 10, "PLAYLIST", INK);
        assert!(
            c.rgba.iter().all(|byte| *byte == 0),
            "a label too wide for its box was drawn anyway"
        );
        // …and one that fits is drawn.
        c.centred(0, 0, 40, 10, "PL", INK);
        assert!(
            c.rgba.iter().any(|byte| *byte != 0),
            "a label that fits was dropped"
        );
    }

    #[test]
    fn the_ramp_runs_from_dim_to_accent() {
        let colors = vis_colors();
        assert_eq!(colors.len(), 24);
        // The bar ramp brightens.
        assert!(colors[17].r() >= colors[2].r());
        assert_eq!(colors[23], Color32::from_rgb(INK[0], INK[1], INK[2]));
    }

    /// The band track's twenty-eight states have to *look* like a rising level,
    /// or the equaliser's most Winamp-ish detail is twenty-eight identical
    /// sprites. The lit part of the groove grows from the bottom as the step
    /// rises, which is what the art is for.
    #[test]
    fn the_band_art_rises_with_the_step() {
        let skin = stock();
        let lit = |step: u32| {
            let sprite = sprites::eq_band(step);
            let image = skin.sprite(sprite).expect("crops");
            let middle = sprite.w as usize / 2;
            // Count the rows whose groove is brighter than the empty groove.
            (0..sprite.h as usize)
                .filter(|row| {
                    let pixel = image.pixels[row * sprite.w as usize + middle];
                    u32::from(pixel.r()) + u32::from(pixel.g()) + u32::from(pixel.b()) > 90
                })
                .count()
        };
        let bottom = lit(0);
        let middle = lit(13);
        let top = lit(sprites::EQ_BAND_STEPS - 1);
        assert!(
            bottom < middle && middle < top,
            "the track does not rise: {bottom} → {middle} → {top}"
        );
        // Every state differs from its neighbour, so no two positions of the
        // slider paint the same background.
        let mut seen = std::collections::BTreeSet::new();
        for step in 0..sprites::EQ_BAND_STEPS {
            seen.insert(lit(step));
        }
        assert!(
            seen.len() >= sprites::EQ_BAND_STEPS as usize - 2,
            "only {} of {} band states look different",
            seen.len(),
            sprites::EQ_BAND_STEPS
        );
    }

    /// The graph's colour column grades top to bottom, because the curve takes
    /// its colour from the row it is on.
    #[test]
    fn the_graph_colours_grade_down_the_column() {
        let skin = stock();
        let column = skin
            .column(sprites::EQ_GRAPH_COLORS)
            .expect("the stock skin has a colour column");
        assert_eq!(column.len(), crate::skin::layout::eq::GRAPH.height as usize);
        assert!(
            column.iter().all(|c| c.a() == 255),
            "a colour is see-through"
        );
        // Warm at the top, dim at the bottom: the top of the graph is +12 dB.
        assert!(
            column[0].g() > column[column.len() - 1].g(),
            "the column does not grade"
        );
        // The preamp line is its own row, and it is not the background.
        let line = skin.sprite(sprites::EQ_PREAMP_LINE).expect("crops");
        assert!(line.pixels.iter().all(|p| p.a() == 255));
        assert_ne!(
            line.pixels[0],
            Color32::from_rgb(DISPLAY[0], DISPLAY[1], DISPLAY[2]),
            "the preamp line is invisible on the graph"
        );
    }

    /// The playlist's title bar dims when the window loses focus, which is what
    /// the second row of `pledit.bmp` is for. Identical rows would make an
    /// unfocused window look focused — the bug the main window's title bar had.
    #[test]
    fn the_playlist_title_bar_dims() {
        let skin = stock();
        for (name, lit, dim) in [
            ("left", sprites::PL_TOP_LEFT, sprites::PL_TOP_LEFT_DIM),
            ("tile", sprites::PL_TOP_TILE, sprites::PL_TOP_TILE_DIM),
            ("title", sprites::PL_TOP_TITLE, sprites::PL_TOP_TITLE_DIM),
            ("right", sprites::PL_TOP_RIGHT, sprites::PL_TOP_RIGHT_DIM),
        ] {
            let a = skin.sprite(lit).expect("crops");
            let b = skin.sprite(dim).expect("crops");
            assert_ne!(a.pixels, b.pixels, "the {name} piece does not dim");
        }
    }

    /// The frame's pieces tile: a run of the top tile has to meet the corners
    /// without a seam, which means the tile's own edges cannot be borders.
    #[test]
    fn the_playlist_frame_tiles_without_a_seam() {
        let skin = stock();
        let tile = skin.sprite(sprites::PL_SHADE_TILE).expect("crops");
        let w = sprites::PL_SHADE_TILE.w as usize;
        // The rolled-up bar's tile repeats horizontally, so its first and last
        // columns must match: a border down either side would draw a line every
        // 25 pixels across the bar.
        for row in 0..sprites::PL_SHADE_TILE.h as usize {
            assert_eq!(
                tile.pixels[row * w],
                tile.pixels[row * w + w - 1],
                "the shade tile has an edge at row {row}"
            );
        }
        // The sides tile vertically, so their first and last rows must match.
        for sprite in [sprites::PL_LEFT_TILE, sprites::PL_RIGHT_TILE] {
            let image = skin.sprite(sprite).expect("crops");
            let w = sprite.w as usize;
            let last = (sprite.h as usize - 1) * w;
            for col in 0..w {
                assert_eq!(
                    image.pixels[col],
                    image.pixels[last + col],
                    "{sprite:?} has an edge at column {col}"
                );
            }
        }
    }

    /// Print the sheets as text, so a person (or a coding agent that cannot
    /// look at a PNG) can read the layout. `cargo test dump_the_skin --
    /// --ignored --nocapture`.
    #[test]
    #[ignore = "prints the skin for eyeballing"]
    fn dump_the_skin() {
        for (name, sheet) in [
            ("main.bmp", main_sheet()),
            ("titlebar.bmp", titlebar_sheet()),
            ("cbuttons.bmp", cbuttons_sheet()),
            ("shufrep.bmp", shufrep_sheet()),
            ("monoster.bmp", monoster_sheet()),
            ("eqmain.bmp", eqmain_sheet()),
            ("eq_ex.bmp", eq_ex_sheet()),
            ("pledit.bmp", pledit_sheet()),
        ] {
            println!("=== {name} ({}×{}) ===", sheet.width, sheet.height);
            for y in 0..sheet.height {
                let mut line = String::new();
                for x in 0..sheet.width {
                    let at = (((y * sheet.width) + x) * 4) as usize;
                    let (r, g, b, a) = (
                        sheet.rgba[at],
                        sheet.rgba[at + 1],
                        sheet.rgba[at + 2],
                        sheet.rgba[at + 3],
                    );
                    if a == 0 {
                        line.push('~');
                        continue;
                    }
                    let lum = (u32::from(r) * 3 + u32::from(g) * 6 + u32::from(b)) / 10;
                    line.push(match lum {
                        0..=10 => ' ',
                        11..=30 => '.',
                        31..=60 => ':',
                        61..=120 => '+',
                        _ => '#',
                    });
                }
                println!("{y:3}|{line}");
            }
        }
    }
    #[test]
    fn lerp_stays_inside_its_ends() {
        assert_eq!(lerp(0, 255, 0.0), 0);
        assert_eq!(lerp(0, 255, 1.0), 255);
        assert_eq!(lerp(0, 255, 2.0), 255, "clamped");
        assert_eq!(lerp(10, 20, 0.5), 15);
    }
}
