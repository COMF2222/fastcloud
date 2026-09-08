//! Where each control sits in each window, in skin pixels.
//!
//! These are Winamp 2's own coordinates, cross-checked against Webamp's CSS.
//! Every classic skin paints its background to match them, so they are not ours
//! to choose: a control drawn a few pixels off sits on the wrong part of
//! somebody's artwork.
//!
//! Keeping them here as data rather than scattered constants is what caught
//! the time display sitting at x=36 — which is the *minus sign's* cell, not
//! the first digit's.
//!
//! This module is the main window; [`eq`] and [`pl`] are the other two, and
//! [`stack`] is how the three sit together.

pub mod eq;
pub mod pl;
pub mod stack;

/// A rectangle in the window, in skin pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Area {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Half-open on the far edges, so neighbouring areas cannot both claim a
    /// pixel. The renderer hit-tests in screen space (egui owns that), so this
    /// is here for the layout's own checks.
    #[cfg(test)]
    pub fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    pub const fn right(&self) -> u32 {
        self.x + self.width
    }

    #[cfg(test)]
    pub const fn bottom(&self) -> u32 {
        self.y + self.height
    }

    /// How many whole 5-pixel font cells fit across this area.
    pub const fn cells(&self) -> usize {
        (self.width / super::pixel_text::CHAR_W) as usize
    }
}

/// The main window. Every classic skin is exactly this size.
pub const WINDOW_WIDTH: u32 = 275;
pub const WINDOW_HEIGHT: u32 = 116;
/// The window rolled up to its title bar ("shade mode").
pub const SHADE_HEIGHT: u32 = 14;

macro_rules! areas {
    ($($(#[$attr:meta])* $name:ident = ($x:expr, $y:expr, $w:expr, $h:expr);)*) => {
        $($(#[$attr])* pub const $name: Area = Area::new($x, $y, $w, $h);)*
        /// Every control, so the layout can be checked against the window.
        #[cfg(test)]
        pub const ALL: &[(&str, Area)] = &[$((stringify!($name), $name),)*];
    };
}

areas! {
    TITLE_BAR = (0, 0, 275, 14);
    /// The Winamp logo, which opens the options menu.
    OPTIONS_BUTTON = (6, 3, 9, 9);
    MINIMIZE_BUTTON = (244, 3, 9, 9);
    /// Rolls the window up to its title bar and back.
    SHADE_BUTTON = (254, 3, 9, 9);
    CLOSE_BUTTON = (264, 3, 9, 9);

    /// The O A I D V strip down the display's left edge.
    CLUTTER_BAR = (10, 22, 8, 43);
    CLUTTER_O = (10, 25, 8, 8);
    CLUTTER_A = (10, 33, 8, 7);
    CLUTTER_I = (10, 40, 8, 7);
    CLUTTER_D = (10, 47, 8, 8);
    CLUTTER_V = (10, 55, 8, 7);

    /// Lit while a track is loading.
    WORK_INDICATOR = (24, 28, 3, 9);
    /// The play / pause / stop mark.
    STATUS = (26, 28, 9, 9);

    /// The minus sign when the skin ships `nums_ex.bmp`: a full digit cell.
    MINUS_EX = (36, 26, 9, 13);
    /// The minus borrowed from `numbers.bmp`: one row of pixels.
    MINUS = (38, 32, 5, 1);
    MINUTE_TENS = (48, 26, 9, 13);
    MINUTE_ONES = (60, 26, 9, 13);
    SECOND_TENS = (78, 26, 9, 13);
    SECOND_ONES = (90, 26, 9, 13);

    /// The scrolling title. 154 pixels of 5-pixel cells: 30 characters, and
    /// the step is the cell width exactly — the cells carry their own bearing.
    /// [`Area::cells`] is the count.
    MARQUEE = (111, 27, 154, 6);
    KBPS = (111, 43, 15, 6);
    KHZ = (156, 43, 10, 6);

    VISUALIZER = (24, 43, 76, 16);

    MONO = (212, 41, 27, 12);
    STEREO = (239, 41, 29, 12);

    VOLUME = (107, 57, 68, 13);
    BALANCE = (177, 57, 38, 13);
    EQ_BUTTON = (219, 58, 23, 12);
    PLAYLIST_BUTTON = (242, 58, 23, 12);

    POSITION = (16, 72, 248, 10);

    PREVIOUS = (16, 88, 23, 18);
    PLAY = (39, 88, 23, 18);
    PAUSE = (62, 88, 23, 18);
    STOP = (85, 88, 23, 18);
    NEXT = (108, 88, 22, 18);
    EJECT = (136, 89, 22, 16);
    SHUFFLE = (164, 89, 47, 15);
    REPEAT = (210, 89, 28, 15);
    /// The Winamp wordmark at the bottom right.
    ABOUT = (253, 91, 13, 15);
}

/// The digit cells of the time display, left to right.
pub const TIME_DIGITS: [Area; 4] = [MINUTE_TENS, MINUTE_ONES, SECOND_TENS, SECOND_ONES];

/// How far each slider's thumb can travel: the track less the thumb.
pub const VOLUME_THUMB_W: u32 = 14;
pub const BALANCE_THUMB_W: u32 = 14;
pub const POSITION_THUMB_W: u32 = 29;
pub const VOLUME_TRAVEL: u32 = VOLUME.width - VOLUME_THUMB_W;
pub const BALANCE_TRAVEL: u32 = BALANCE.width - BALANCE_THUMB_W;
pub const POSITION_TRAVEL: u32 = POSITION.width - POSITION_THUMB_W;

// A thumb wider than its track has nowhere to go, and the subtractions above
// would underflow. Checked here rather than in a test: it is a property of the
// numbers, so the build should not get as far as running tests.
const _: () = assert!(VOLUME_THUMB_W < VOLUME.width);
const _: () = assert!(BALANCE_THUMB_W < BALANCE.width);
const _: () = assert!(POSITION_THUMB_W < POSITION.width);

// ===== Shade mode: the window as one title bar =====

pub const SHADE_TIME: Area = Area::new(127, 4, 30, 6);
pub const SHADE_PREVIOUS: Area = Area::new(169, 2, 7, 10);
pub const SHADE_PLAY: Area = Area::new(176, 2, 10, 10);
pub const SHADE_PAUSE: Area = Area::new(186, 2, 9, 10);
pub const SHADE_STOP: Area = Area::new(195, 2, 9, 10);
pub const SHADE_NEXT: Area = Area::new(204, 2, 10, 10);
pub const SHADE_EJECT: Area = Area::new(215, 2, 10, 10);
pub const SHADE_POSITION: Area = Area::new(226, 4, 17, 7);
/// The mini seek bar's thumb.
pub const SHADE_POSITION_THUMB_W: u32 = 3;
const _: () = assert!(SHADE_POSITION_THUMB_W < SHADE_POSITION.width);

/// The rolled-up window's title, between the options button and the time.
///
/// Winamp's shade bar is one sprite with no marked-out well, so unlike
/// everything else here this rectangle is ours: it starts clear of the options
/// button and stops clear of [`SHADE_TIME`], and holds whole 5-pixel cells.
pub const SHADE_MARQUEE: Area = Area::new(20, 4, 105, 6);
/// How many characters the rolled-up marquee holds. The full one's count is
/// [`MARQUEE`]`.cells()`; only this one is needed outside the layout, because
/// it is the shorter of the two and so decides when a title has to scroll.
pub const SHADE_MARQUEE_CHARS: usize = SHADE_MARQUEE.cells();

/// The time's five cells in shade mode: a sign, the minutes, then the seconds,
/// as offsets inside [`SHADE_TIME`]. Winamp spaces them by hand rather than on
/// a grid — the pairs sit closer to each other than to the gap in the middle.
pub const SHADE_TIME_CELLS: [u32; 5] = [1, 7, 12, 20, 25];

/// Shade mode's controls, checked against the rolled-up window.
#[cfg(test)]
pub const SHADE_ALL: &[(&str, Area)] = &[
    ("SHADE_MARQUEE", SHADE_MARQUEE),
    ("SHADE_TIME", SHADE_TIME),
    ("SHADE_PREVIOUS", SHADE_PREVIOUS),
    ("SHADE_PLAY", SHADE_PLAY),
    ("SHADE_PAUSE", SHADE_PAUSE),
    ("SHADE_STOP", SHADE_STOP),
    ("SHADE_NEXT", SHADE_NEXT),
    ("SHADE_EJECT", SHADE_EJECT),
    ("SHADE_POSITION", SHADE_POSITION),
    ("OPTIONS_BUTTON", OPTIONS_BUTTON),
    ("MINIMIZE_BUTTON", MINIMIZE_BUTTON),
    ("SHADE_BUTTON", SHADE_BUTTON),
    ("CLOSE_BUTTON", CLOSE_BUTTON),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_fits_inside_the_window() {
        for (name, area) in ALL {
            assert!(
                area.right() <= WINDOW_WIDTH && area.bottom() <= WINDOW_HEIGHT,
                "{name} leaves the window: {area:?}"
            );
            assert!(area.width > 0 && area.height > 0, "{name} is empty");
        }
    }

    #[test]
    fn shade_controls_fit_the_rolled_up_window() {
        for (name, area) in SHADE_ALL {
            assert!(
                area.right() <= WINDOW_WIDTH && area.bottom() <= SHADE_HEIGHT,
                "{name} leaves the shade bar: {area:?}"
            );
            assert!(area.width > 0 && area.height > 0, "{name} is empty");
        }
    }

    /// Shade mode's row reads left to right with nothing on top of anything
    /// else: options, title, time, transport, then the seek bar — and the
    /// title-bar buttons stay clear at the right end.
    #[test]
    fn the_shade_row_does_not_overlap_itself() {
        let order = [
            ("OPTIONS_BUTTON", OPTIONS_BUTTON),
            ("SHADE_MARQUEE", SHADE_MARQUEE),
            ("SHADE_TIME", SHADE_TIME),
            ("SHADE_PREVIOUS", SHADE_PREVIOUS),
            ("SHADE_PLAY", SHADE_PLAY),
            ("SHADE_PAUSE", SHADE_PAUSE),
            ("SHADE_STOP", SHADE_STOP),
            ("SHADE_NEXT", SHADE_NEXT),
            ("SHADE_EJECT", SHADE_EJECT),
            ("SHADE_POSITION", SHADE_POSITION),
            ("MINIMIZE_BUTTON", MINIMIZE_BUTTON),
            ("SHADE_BUTTON", SHADE_BUTTON),
            ("CLOSE_BUTTON", CLOSE_BUTTON),
        ];
        for pair in order.windows(2) {
            let [(name, left), (next, right)] = [pair[0], pair[1]];
            assert!(
                left.right() <= right.x,
                "{name} runs into {next}: {} > {}",
                left.right(),
                right.x
            );
        }
    }

    /// The rolled-up title and time hold whole 5-pixel cells: a partial cell
    /// would clip a glyph down its middle.
    #[test]
    fn the_shade_display_holds_whole_cells() {
        const CELL: u32 = crate::skin::pixel_text::CHAR_W;
        assert_eq!(SHADE_MARQUEE.width % CELL, 0);
        assert_eq!(SHADE_MARQUEE_CHARS, 21);
        assert_eq!(SHADE_MARQUEE.height, crate::skin::pixel_text::CHAR_H);
        assert_eq!(SHADE_TIME.height, crate::skin::pixel_text::CHAR_H);
        // The five time cells fit their box, and the pairs are grouped: the
        // gap in the middle is wider than the one inside a pair.
        let last = SHADE_TIME_CELLS[4] + CELL;
        assert!(last <= SHADE_TIME.width, "the time's cells leave its box");
        let inside_pair = SHADE_TIME_CELLS[2] - SHADE_TIME_CELLS[1];
        let between_pairs = SHADE_TIME_CELLS[3] - SHADE_TIME_CELLS[2];
        assert!(between_pairs > inside_pair, "the colon's gap went missing");
    }

    /// The five transport buttons are one unbroken row, as the artwork has
    /// them: a gap would show the background between two buttons.
    #[test]
    fn the_transport_row_is_contiguous() {
        assert_eq!(PREVIOUS.right(), PLAY.x);
        assert_eq!(PLAY.right(), PAUSE.x);
        assert_eq!(PAUSE.right(), STOP.x);
        assert_eq!(STOP.right(), NEXT.x);
        // All five sit on one line.
        for area in [PREVIOUS, PLAY, PAUSE, STOP, NEXT] {
            assert_eq!(area.y, PREVIOUS.y);
            assert_eq!(area.height, PREVIOUS.height);
        }
    }

    /// Thirty five-pixel cells fit the marquee, and they stop inside the
    /// display well (which ends at x=265). Stepping by six ran thirty of them
    /// to 291 — past the well and past the window.
    #[test]
    fn the_marquee_holds_thirty_cells_inside_the_display() {
        const CELL: u32 = crate::skin::pixel_text::CHAR_W;
        const CHARS: u32 = 30;
        assert_eq!(CELL, 5);
        assert_eq!(MARQUEE.width / CELL, CHARS);
        assert_eq!(MARQUEE.cells(), CHARS as usize);
        assert!(
            MARQUEE.x + CHARS * CELL <= MARQUEE.right(),
            "thirty cells do not fit the marquee"
        );
        assert_eq!(MARQUEE.right(), 265, "the display well ends here");
        assert!(MARQUEE.right() <= WINDOW_WIDTH);
        assert_eq!(MARQUEE.height, crate::skin::pixel_text::CHAR_H);
    }

    /// The digits are on Winamp's own cells, not packed nine pixels apart from
    /// the minus sign's slot: that is what made the time land on the wrong
    /// part of every skin's display.
    #[test]
    fn the_time_digits_are_on_winamps_cells() {
        assert_eq!(
            TIME_DIGITS.map(|cell| cell.x),
            [48, 60, 78, 90],
            "the digit cells moved"
        );
        // The minus sits left of the first digit, not under it.
        assert!(MINUS_EX.right() <= MINUTE_TENS.x);
        assert!(MINUS.right() <= MINUTE_TENS.x);
        // The wide gap between the pairs is where the colon is painted.
        assert!(MINUTE_ONES.right() < SECOND_TENS.x);
        for cell in TIME_DIGITS {
            assert_eq!((cell.width, cell.height), (9, 13));
        }
    }

    #[test]
    fn hit_testing_uses_half_open_edges() {
        assert!(PLAY.contains(39, 88));
        assert!(PLAY.contains(61, 105));
        assert!(!PLAY.contains(62, 88), "that pixel belongs to PAUSE");
        assert!(!PLAY.contains(39, 106));
    }

    /// No two controls may overlap: a click has to belong to one of them.
    #[test]
    fn controls_do_not_overlap() {
        // The clutter bar is the strip its five lamps sit on; the status mark
        // shares the work indicator's column, as in Winamp; and MINUS_EX and
        // MINUS are two ways of drawing the same sign, never both at once.
        const NESTED: [&str; 8] = [
            "CLUTTER_BAR",
            "CLUTTER_O",
            "CLUTTER_A",
            "CLUTTER_I",
            "CLUTTER_D",
            "CLUTTER_V",
            "WORK_INDICATOR",
            "MINUS",
        ];
        let flat: Vec<&(&str, Area)> = ALL
            .iter()
            .filter(|(name, _)| !NESTED.contains(name))
            .collect();
        for (i, (name, a)) in flat.iter().enumerate() {
            for (other, b) in flat.iter().skip(i + 1) {
                // Everything sits on the title bar; and Winamp's own
                // shuffle and repeat sprites share their border column, so a
                // single pixel of overlap is the artwork, not a mistake.
                if *name == "TITLE_BAR" || *other == "TITLE_BAR" {
                    continue;
                }
                let x_overlap = a.right().min(b.right()).saturating_sub(a.x.max(b.x));
                let y_overlap = a.bottom().min(b.bottom()).saturating_sub(a.y.max(b.y));
                assert!(
                    x_overlap <= 1 || y_overlap == 0,
                    "{name} overlaps {other} by {x_overlap}x{y_overlap} pixels"
                );
            }
        }
    }

    /// The lamps stay on the strip they light up.
    #[test]
    fn the_clutter_lamps_sit_on_their_bar() {
        for (name, area) in [
            ("O", CLUTTER_O),
            ("A", CLUTTER_A),
            ("I", CLUTTER_I),
            ("D", CLUTTER_D),
            ("V", CLUTTER_V),
        ] {
            assert!(
                area.x >= CLUTTER_BAR.x
                    && area.right() <= CLUTTER_BAR.right()
                    && area.y >= CLUTTER_BAR.y
                    && area.bottom() <= CLUTTER_BAR.bottom(),
                "the {name} lamp is off the clutter bar"
            );
        }
    }

    /// A thumb has to have room to move. (That it *fits* its track at all is a
    /// `const` assertion up by the travels, since the subtraction would
    /// underflow.)
    #[test]
    fn sliders_have_travel() {
        assert_eq!(VOLUME_TRAVEL, 54);
        assert_eq!(BALANCE_TRAVEL, 24);
        assert_eq!(POSITION_TRAVEL, 219);
        assert_eq!(
            SHADE_POSITION.width - SHADE_POSITION_THUMB_W,
            14,
            "the rolled-up seek bar has fourteen pixels of travel"
        );
    }
}
