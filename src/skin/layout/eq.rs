//! The equaliser window, in skin pixels.
//!
//! Winamp 2's own coordinates, cross-checked against Webamp's
//! `equalizer-window.css`. The window is the same 275 wide as the main one and
//! docks under it, which is why it has no width of its own.
//!
//! Ten bands and a preamp, each a vertical slider; a graph that draws the
//! curve; ON and AUTO; and a presets button. Rolled up it keeps the volume and
//! balance sliders, because those are the two a person reaches for most.

use super::Area;

/// Same width as the main window: the two are docked, so a different width
/// would show a step down the side.
pub const WIDTH: u32 = super::WINDOW_WIDTH;
pub const HEIGHT: u32 = super::WINDOW_HEIGHT;
/// Rolled up to its title bar, like every other window.
pub const SHADE_HEIGHT: u32 = super::SHADE_HEIGHT;

pub const TITLE_BAR: Area = Area::new(0, 0, WIDTH, SHADE_HEIGHT);
/// The equaliser's title bar has two buttons, not four: no options menu and no
/// minimize, because the window is not the app.
pub const SHADE_BUTTON: Area = Area::new(254, 3, 9, 9);
pub const CLOSE_BUTTON: Area = Area::new(264, 3, 9, 9);

pub const ON_BUTTON: Area = Area::new(14, 18, 26, 12);
pub const AUTO_BUTTON: Area = Area::new(40, 18, 32, 12);
pub const PRESETS_BUTTON: Area = Area::new(217, 18, 44, 12);
/// The little screen that draws the curve.
pub const GRAPH: Area = Area::new(86, 17, 113, 19);

/// A band's cell: the slider's track. Winamp's background art is one pixel
/// taller than the slider inside it, which is why the two differ.
pub const BAND_WIDTH: u32 = 14;
pub const BAND_HEIGHT: u32 = 63;
pub const BAND_TOP: u32 = 38;
/// The thumb, and how far it can travel down the track.
pub const THUMB_HEIGHT: u32 = 11;
pub const THUMB_TRAVEL: u32 = BAND_HEIGHT - 1 - THUMB_HEIGHT;

/// The preamp, which is a band with its own slot to the left of the ten.
pub const PREAMP: Area = Area::new(21, BAND_TOP, BAND_WIDTH, BAND_HEIGHT);

/// The ten bands' left edges. Eighteen pixels apart, starting at 78 — the
/// gap between the preamp and the first band is where the dB labels go.
pub const BAND_LEFT: u32 = 78;
pub const BAND_STEP: u32 = 18;
pub const BANDS: usize = 10;

/// One band's area, 0..[`BANDS`].
pub const fn band(index: usize) -> Area {
    Area::new(
        BAND_LEFT + index as u32 * BAND_STEP,
        BAND_TOP,
        BAND_WIDTH,
        BAND_HEIGHT,
    )
}

/// The `+12dB`, `0dB` and `-12dB` marks down the left side.
pub const PLUS_12DB: Area = Area::new(45, 36, 22, 8);
pub const ZERO_DB: Area = Area::new(45, 64, 22, 8);
pub const MINUS_12DB: Area = Area::new(45, 95, 22, 8);

// The band geometry has to hold for the sliders to work at all, and all of it is
// known at compile time — so it is checked there rather than in a test, where
// clippy rightly points out that asserting on constants proves nothing at run
// time.
const _: () = assert!(THUMB_HEIGHT < BAND_HEIGHT, "the thumb cannot travel");
const _: () = assert!(THUMB_TRAVEL == 51, "Winamp's band travel is 51 pixels");
// The marks bracket the travel: +12 at the top of the track, -12 at the bottom
// of it, and 0 between them.
const _: () = assert!(PLUS_12DB.y <= BAND_TOP);
const _: () = assert!(MINUS_12DB.y + MINUS_12DB.height >= BAND_TOP + BAND_HEIGHT - THUMB_HEIGHT);
const _: () = assert!(ZERO_DB.y > PLUS_12DB.y && ZERO_DB.y < MINUS_12DB.y);

// ===== Shade mode: volume and balance on the title bar =====

/// Winamp's rolled-up equaliser is a volume and a balance slider, so a person
/// who wants only those two need not keep the whole window open.
pub const SHADE_VOLUME: Area = Area::new(61, 4, 97, 6);
pub const SHADE_BALANCE: Area = Area::new(164, 4, 43, 6);
/// Both sliders' thumbs are three pixels wide.
pub const SHADE_THUMB_W: u32 = 3;
pub const SHADE_VOLUME_TRAVEL: u32 = SHADE_VOLUME.width - SHADE_THUMB_W;
pub const SHADE_BALANCE_TRAVEL: u32 = SHADE_BALANCE.width - SHADE_THUMB_W;

// Each rolled-up slider needs room for its thumb to move, and Winamp's two
// travels are these. Compile-time, for the same reason as the band geometry.
const _: () = assert!(SHADE_THUMB_W < SHADE_VOLUME.width);
const _: () = assert!(SHADE_THUMB_W < SHADE_BALANCE.width);
const _: () = assert!(SHADE_VOLUME_TRAVEL == 94);
const _: () = assert!(SHADE_BALANCE_TRAVEL == 40);

/// Every control, so the layout can be checked against the window.
#[cfg(test)]
pub const ALL: &[(&str, Area)] = &[
    ("TITLE_BAR", TITLE_BAR),
    ("SHADE_BUTTON", SHADE_BUTTON),
    ("CLOSE_BUTTON", CLOSE_BUTTON),
    ("ON_BUTTON", ON_BUTTON),
    ("AUTO_BUTTON", AUTO_BUTTON),
    ("PRESETS_BUTTON", PRESETS_BUTTON),
    ("GRAPH", GRAPH),
    ("PREAMP", PREAMP),
    ("PLUS_12DB", PLUS_12DB),
    ("ZERO_DB", ZERO_DB),
    ("MINUS_12DB", MINUS_12DB),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_fits_inside_the_window() {
        for (name, area) in ALL {
            assert!(
                area.right() <= WIDTH && area.bottom() <= HEIGHT,
                "{name} leaves the window: {area:?}"
            );
            assert!(area.width > 0 && area.height > 0, "{name} is empty");
        }
        for index in 0..BANDS {
            let band = band(index);
            assert!(
                band.right() <= WIDTH && band.bottom() <= HEIGHT,
                "band {index} leaves the window: {band:?}"
            );
        }
    }

    /// The bands are an evenly spaced row with a gap between each, and none of
    /// them touches the preamp — its slot is the odd one out.
    #[test]
    fn the_bands_are_an_even_row() {
        assert_eq!(band(0).x, 78);
        assert_eq!(band(BANDS - 1).x, 240, "the last band moved");
        for index in 1..BANDS {
            assert_eq!(
                band(index).x - band(index - 1).x,
                BAND_STEP,
                "band {index} is out of step"
            );
            assert!(
                band(index - 1).right() < band(index).x,
                "bands {} and {index} touch",
                index - 1
            );
        }
        assert!(PREAMP.right() < band(0).x, "the preamp runs into band 0");
        for index in 0..BANDS {
            assert_eq!(band(index).y, PREAMP.y, "band {index} is off the row");
            assert_eq!(band(index).height, PREAMP.height);
        }
    }

    /// The dB marks have to line up with the ends and the middle of the thumb's
    /// travel. (The arithmetic itself is checked at compile time, above.)
    #[test]
    fn the_bands_have_travel_and_marks() {
        // The marks sit left of the bands, in the preamp's gap.
        for mark in [PLUS_12DB, ZERO_DB, MINUS_12DB] {
            assert!(mark.x > PREAMP.right(), "a dB mark is on the preamp");
            assert!(mark.right() <= band(0).x, "a dB mark is on band 0");
        }
    }

    /// Rolled up, the two sliders and the two buttons share fourteen pixels
    /// without overlapping.
    #[test]
    fn the_shade_row_does_not_overlap_itself() {
        let row = [
            ("SHADE_VOLUME", SHADE_VOLUME),
            ("SHADE_BALANCE", SHADE_BALANCE),
            ("SHADE_BUTTON", SHADE_BUTTON),
            ("CLOSE_BUTTON", CLOSE_BUTTON),
        ];
        for pair in row.windows(2) {
            let [(name, left), (next, right)] = [pair[0], pair[1]];
            assert!(
                left.right() <= right.x,
                "{name} runs into {next}: {} > {}",
                left.right(),
                right.x
            );
        }
        for (name, area) in row {
            assert!(area.bottom() <= SHADE_HEIGHT, "{name} leaves the shade bar");
        }
    }

    /// The window docks under the main one, so it must be the same width.
    #[test]
    fn the_window_matches_the_main_one() {
        assert_eq!(WIDTH, super::super::WINDOW_WIDTH);
        assert_eq!(HEIGHT, super::super::WINDOW_HEIGHT);
        assert_eq!(SHADE_HEIGHT, super::super::SHADE_HEIGHT);
    }
}
