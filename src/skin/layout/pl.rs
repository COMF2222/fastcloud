//! The playlist window, in skin pixels.
//!
//! Winamp 2's own coordinates, cross-checked against Webamp's
//! `playlist-window.css`. Unlike the other two this window resizes, in whole
//! tiles: 25 pixels across and 29 down, because that is how big the border
//! sprites are and a part-tile would show a seam.
//!
//! The frame is nine pieces — corners, edges, a tiled middle — so the layout
//! here is arithmetic on a size rather than a list of fixed rectangles.

use super::Area;

/// The bar across the top, holding the title and the two buttons.
pub const TOP_HEIGHT: u32 = 20;
/// The strip along the bottom, holding the menus, the time and the little
/// visualiser.
pub const BOTTOM_HEIGHT: u32 = 38;
/// The frame down either side.
pub const LEFT_WIDTH: u32 = 12;
pub const RIGHT_WIDTH: u32 = 20;

/// One row of the track list.
pub const ROW_HEIGHT: u32 = 13;

/// The smallest useful window: the frame plus four rows.
///
/// The width does not change: all three windows are docked in one OS window
/// (see [`super::stack`]), so the playlist is always the main window's 275 and
/// only its row count is dragged.
pub const MIN_WIDTH: u32 = super::WINDOW_WIDTH;
pub const MIN_ROWS: u32 = 4;
/// As it opens: the main window's width, and enough rows to be worth opening.
pub const DEFAULT_ROWS: u32 = 8;
/// A ceiling, so a wild drag cannot ask for a window taller than a screen.
pub const MAX_ROWS: u32 = 64;

// The row counts have to be ordered, and the smallest window has to have room
// for its frame — both known at compile time, so checked there.
const _: () = assert!(MIN_ROWS <= DEFAULT_ROWS && DEFAULT_ROWS <= MAX_ROWS);
const _: () = assert!(height(MIN_ROWS) > TOP_HEIGHT + BOTTOM_HEIGHT);

/// The window's height for a number of visible rows.
pub const fn height(rows: u32) -> u32 {
    TOP_HEIGHT + rows * ROW_HEIGHT + BOTTOM_HEIGHT
}

/// How many rows fit a window of this height. Rounds *down*: a part row would
/// draw clipped, which is why Winamp resizes in whole rows.
pub const fn rows(height: u32) -> u32 {
    let inner = height.saturating_sub(TOP_HEIGHT + BOTTOM_HEIGHT);
    inner / ROW_HEIGHT
}

/// The row count for a dragged height, clamped to what the window allows.
pub const fn rows_clamped(height: u32) -> u32 {
    let rows = rows(height);
    if rows < MIN_ROWS {
        MIN_ROWS
    } else if rows > MAX_ROWS {
        MAX_ROWS
    } else {
        rows
    }
}

/// The title bar, which drags the window.
pub const fn title_bar(width: u32) -> Area {
    Area::new(0, 0, width, TOP_HEIGHT)
}

/// The two buttons at the top right. Winamp puts them two and twelve pixels in
/// from the right edge, so they move with the window.
pub const fn close_button(width: u32) -> Area {
    Area::new(width - 11, 3, 9, 9)
}

pub const fn shade_button(width: u32) -> Area {
    Area::new(width - 21, 3, 9, 9)
}

/// The scroll bar's groove and its handle, down the right edge.
pub const SCROLLBAR_W: u32 = 8;
pub const SCROLL_HANDLE_H: u32 = 18;

pub const fn scrollbar(width: u32, height: u32) -> Area {
    Area::new(
        width - RIGHT_WIDTH + 5,
        TOP_HEIGHT,
        SCROLLBAR_W,
        height - TOP_HEIGHT - BOTTOM_HEIGHT,
    )
}

/// The track list itself: everything inside the frame.
pub const fn tracks(width: u32, height: u32) -> Area {
    Area::new(
        LEFT_WIDTH,
        TOP_HEIGHT,
        width - LEFT_WIDTH - RIGHT_WIDTH,
        height - TOP_HEIGHT - BOTTOM_HEIGHT,
    )
}

/// One row of the list, 0..[`rows`].
pub const fn row(width: u32, height: u32, index: u32) -> Area {
    let list = tracks(width, height);
    Area::new(list.x, list.y + index * ROW_HEIGHT, list.width, ROW_HEIGHT)
}

/// The bottom strip's own coordinates are measured from the *right* edge,
/// because that is the corner its art is anchored to.
pub const BOTTOM_RIGHT_WIDTH: u32 = 150;

/// The `mm:ss / mm:ss` readout at the bottom.
pub const fn running_time(width: u32, height: u32) -> Area {
    Area::new(
        width - BOTTOM_RIGHT_WIDTH + 7,
        height - BOTTOM_HEIGHT + 10,
        70,
        6,
    )
}

/// The five transport buttons on the bottom strip, ten pixels square.
pub const ACTION_SIZE: u32 = 10;
pub const ACTIONS: usize = 5;

pub const fn action(width: u32, height: u32, index: usize) -> Area {
    Area::new(
        width - BOTTOM_RIGHT_WIDTH + 3 + index as u32 * ACTION_SIZE,
        height - BOTTOM_HEIGHT + 22,
        ACTION_SIZE,
        ACTION_SIZE,
    )
}

/// Winamp's five bottom menus: ADD, REM, SEL, MISC along the left corner and
/// LIST over on the right.
pub const MENUS: usize = 5;
pub const MENU_LABELS: [&str; MENUS] = ["ADD", "REM", "SEL", "MISC", "LIST"];
pub const MENU_W: u32 = 22;
pub const MENU_H: u32 = 18;
/// How far apart the left four sit, from `playlist-window.css`: 14, 43, 72, 101.
pub const MENU_STEP: u32 = 29;
/// LIST is measured from the right edge, because its art is on the right
/// corner piece rather than the left one.
pub const MENU_RIGHT_INSET: u32 = 44;

/// One menu button's hit area, 0..[`MENUS`].
pub const fn menu(width: u32, height: u32, index: usize) -> Area {
    let x = if index + 1 >= MENUS {
        width - MENU_RIGHT_INSET
    } else {
        14 + index as u32 * MENU_STEP
    };
    Area::new(x, height - BOTTOM_HEIGHT + 8, MENU_W, MENU_H)
}

/// The little visualiser on the bottom strip, left of the readout.
pub const fn visualiser(width: u32, height: u32) -> Area {
    Area::new(
        width - BOTTOM_RIGHT_WIDTH - 75 + 2,
        height - BOTTOM_HEIGHT + 12,
        72,
        16,
    )
}

/// The resize grip in the bottom-right corner.
pub const fn resize_grip(width: u32, height: u32) -> Area {
    Area::new(width - 20, height - 20, 20, 20)
}

// ===== Shade mode =====

/// Rolled up the playlist is a title and a time, like the main window's.
pub const SHADE_HEIGHT: u32 = super::SHADE_HEIGHT;

pub const fn shade_title(width: u32) -> Area {
    // Stops clear of the time, which is thirty pixels in from the right.
    Area::new(5, 4, width.saturating_sub(75), 6)
}

pub const fn shade_time(width: u32) -> Area {
    Area::new(width - 60, 4, 30, 6)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wider window than the app ever opens.
    ///
    /// The stack docks every window at 275 (see [`super::super::stack`]), so
    /// nothing calls these with another width today — but they all take one, and
    /// arithmetic that only works at one value is arithmetic waiting to break.
    /// The step is the top tile's width, which is what a resize would move by.
    const WIDER: u32 = MIN_WIDTH + 25 * 4;

    /// The two heights the window is checked at. Kept here rather than beside
    /// [`height`] because nothing outside the tests wants a *height*: the app
    /// stores rows and the stack asks for them.
    const DEFAULT_HEIGHT: u32 = height(DEFAULT_ROWS);
    const MIN_HEIGHT: u32 = height(MIN_ROWS);

    /// The frame's pieces and the list between them add up to the window,
    /// whatever size it is.
    #[test]
    fn the_frame_and_the_list_fill_the_window() {
        for rows_shown in [MIN_ROWS, DEFAULT_ROWS, 30] {
            let h = height(rows_shown);
            let w = MIN_WIDTH;
            let list = tracks(w, h);
            assert_eq!(list.y, TOP_HEIGHT);
            assert_eq!(list.bottom(), h - BOTTOM_HEIGHT);
            assert_eq!(list.height, rows_shown * ROW_HEIGHT);
            assert_eq!(list.x, LEFT_WIDTH);
            assert_eq!(list.right(), w - RIGHT_WIDTH);
            // The rows tile the list exactly.
            assert_eq!(rows(h), rows_shown);
            let last = row(w, h, rows_shown - 1);
            assert_eq!(last.bottom(), list.bottom(), "the last row is short");
            for index in 0..rows_shown {
                let r = row(w, h, index);
                assert!(r.bottom() <= list.bottom(), "row {index} overflows");
                assert_eq!(r.height, ROW_HEIGHT);
            }
        }
    }

    /// Everything on the bottom strip stays on it, and nothing overlaps the
    /// list above.
    #[test]
    fn the_bottom_strip_holds_its_contents() {
        let (w, h) = (MIN_WIDTH, DEFAULT_HEIGHT);
        let strip_top = h - BOTTOM_HEIGHT;
        let list = tracks(w, h);
        for (name, area) in [
            ("running_time", running_time(w, h)),
            ("visualiser", visualiser(w, h)),
            ("action 0", action(w, h, 0)),
            ("action 4", action(w, h, ACTIONS - 1)),
            ("resize_grip", resize_grip(w, h)),
        ] {
            assert!(area.y >= strip_top, "{name} is above the strip");
            assert!(area.bottom() <= h, "{name} leaves the window");
            assert!(area.right() <= w, "{name} leaves the window");
            assert!(area.y >= list.bottom(), "{name} sits on the list");
        }
        // The five buttons are a contiguous row.
        for index in 1..ACTIONS {
            assert_eq!(
                action(w, h, index).x - action(w, h, index - 1).x,
                ACTION_SIZE
            );
        }
        // The visualiser sits left of the readout, not under it.
        assert!(visualiser(w, h).right() <= running_time(w, h).x);
    }

    /// The two title-bar buttons follow the right edge as the window grows.
    #[test]
    fn the_buttons_track_the_right_edge() {
        for w in [MIN_WIDTH, WIDER] {
            let close = close_button(w);
            let shade = shade_button(w);
            assert_eq!(close.right(), w - 2);
            assert!(shade.right() <= close.x, "the buttons overlap");
            assert!(close.bottom() <= TOP_HEIGHT);
            assert!(shade.bottom() <= TOP_HEIGHT);
            // …and the scroll bar stays inside the right frame.
            let bar = scrollbar(w, DEFAULT_HEIGHT);
            assert!(bar.right() <= w);
            assert!(bar.x >= w - RIGHT_WIDTH);
            assert!(SCROLL_HANDLE_H <= bar.height, "no room for the handle");
        }
    }

    /// A window at the minimum still has room for its rows, and the height
    /// arithmetic round-trips.
    #[test]
    fn the_smallest_window_still_works() {
        assert_eq!(MIN_HEIGHT, height(MIN_ROWS));
        assert_eq!(rows(MIN_HEIGHT), MIN_ROWS);
        // A height that is not a whole number of rows rounds down rather than
        // drawing a clipped one.
        assert_eq!(rows(MIN_HEIGHT + ROW_HEIGHT - 1), MIN_ROWS);
        assert_eq!(rows(0), 0);
        assert_eq!(rows(TOP_HEIGHT + BOTTOM_HEIGHT), 0);
        // The row count the window opens at, and the ceiling a drag clamps to,
        // both round-trip through the height.
        assert_eq!(DEFAULT_HEIGHT, height(DEFAULT_ROWS));
        assert_eq!(rows(DEFAULT_HEIGHT), DEFAULT_ROWS);
        assert_eq!(rows(height(MAX_ROWS)), MAX_ROWS);
        // A drag past either end clamps rather than asking for a window that
        // cannot draw its own frame.
        assert_eq!(rows_clamped(0), MIN_ROWS);
        assert_eq!(rows_clamped(MIN_HEIGHT), MIN_ROWS);
        assert_eq!(rows_clamped(DEFAULT_HEIGHT), DEFAULT_ROWS);
        assert_eq!(rows_clamped(height(MAX_ROWS + 100)), MAX_ROWS);
        assert_eq!(rows_clamped(u32::MAX), MAX_ROWS);
    }

    /// The five menu buttons sit on the bottom strip without overlapping each
    /// other, and LIST stays over on the right as the window widens.
    #[test]
    fn the_menu_buttons_are_a_row_on_the_strip() {
        for w in [MIN_WIDTH, WIDER] {
            let h = DEFAULT_HEIGHT;
            let strip_top = h - BOTTOM_HEIGHT;
            assert_eq!(MENU_LABELS.len(), MENUS);
            for index in 0..MENUS {
                let area = menu(w, h, index);
                assert!(area.y >= strip_top, "menu {index} is above the strip");
                assert!(area.bottom() <= h, "menu {index} leaves the window");
                assert!(area.right() <= w, "menu {index} leaves the window");
                assert_eq!((area.width, area.height), (MENU_W, MENU_H));
            }
            // The left four step evenly; LIST is anchored to the right edge, so
            // it is not part of that run.
            for index in 1..MENUS - 1 {
                assert_eq!(
                    menu(w, h, index).x - menu(w, h, index - 1).x,
                    MENU_STEP,
                    "menu {index} is out of step"
                );
            }
            let list = menu(w, h, MENUS - 1);
            assert!(
                list.x > menu(w, h, MENUS - 2).right(),
                "LIST runs into MISC"
            );
            assert_eq!(list.right(), w - MENU_RIGHT_INSET + MENU_W);
        }
        // The four left menus stay on the left corner piece and LIST on the
        // right one, which is what lets each be painted into its own sprite.
        let (w, h) = (MIN_WIDTH, DEFAULT_HEIGHT);
        assert!(menu(w, h, MENUS - 2).right() <= 125, "MISC left its piece");
        assert!(menu(w, h, MENUS - 1).x >= w - BOTTOM_RIGHT_WIDTH);
    }

    /// Rolled up, the title and the time share the bar without overlapping.
    #[test]
    fn the_shade_row_does_not_overlap_itself() {
        for w in [MIN_WIDTH, WIDER] {
            let title = shade_title(w);
            let time = shade_time(w);
            assert!(
                title.right() <= time.x,
                "the title runs into the time: {} > {}",
                title.right(),
                time.x
            );
            assert!(time.right() <= w - 20, "the time is under the buttons");
            assert!(title.bottom() <= SHADE_HEIGHT);
            assert!(time.bottom() <= SHADE_HEIGHT);
        }
    }
}
