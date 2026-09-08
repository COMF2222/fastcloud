//! Which windows are open, and where they sit.
//!
//! Winamp had three top-level windows that docked to each other. This app has
//! one (see [`crate::ui::App::show_mini`]), so the three stack inside it:
//! the main window, then the equaliser, then the playlist, each as tall as it
//! needs and all 275 wide. Opening one makes the OS window taller; rolling one
//! up makes it shorter.
//!
//! Keeping the arithmetic here rather than in the renderer is what lets a test
//! check that the three never overlap and always add up to the window.

use super::{Area, eq, pl};

/// One of the three windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Window {
    Main,
    Equalizer,
    Playlist,
}

/// The order they stack in, top to bottom. Winamp's own docking order.
pub const ORDER: [Window; 3] = [Window::Main, Window::Equalizer, Window::Playlist];

/// What is open and what is rolled up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stack {
    /// The equaliser is open. (The main window is always open — it *is* the
    /// mini player.)
    pub equalizer: bool,
    /// The playlist is open.
    pub playlist: bool,
    /// Each window's shade state, in [`ORDER`].
    pub shade: [bool; 3],
    /// How many rows the playlist shows, which is the only thing that resizes.
    pub playlist_rows: u32,
}

impl Default for Stack {
    fn default() -> Self {
        Self {
            equalizer: false,
            playlist: false,
            shade: [false; 3],
            playlist_rows: pl::DEFAULT_ROWS,
        }
    }
}

impl Stack {
    /// Whether a window is open. The main one always is.
    pub fn is_open(&self, window: Window) -> bool {
        match window {
            Window::Main => true,
            Window::Equalizer => self.equalizer,
            Window::Playlist => self.playlist,
        }
    }

    pub fn set_open(&mut self, window: Window, open: bool) {
        match window {
            Window::Main => {}
            Window::Equalizer => self.equalizer = open,
            Window::Playlist => self.playlist = open,
        }
    }

    fn index(window: Window) -> usize {
        match window {
            Window::Main => 0,
            Window::Equalizer => 1,
            Window::Playlist => 2,
        }
    }

    /// Whether a window is rolled up to its title bar.
    pub fn is_shade(&self, window: Window) -> bool {
        self.shade[Self::index(window)]
    }

    pub fn set_shade(&mut self, window: Window, shade: bool) {
        self.shade[Self::index(window)] = shade;
    }

    pub fn toggle_shade(&mut self, window: Window) {
        let at = Self::index(window);
        self.shade[at] = !self.shade[at];
    }

    /// A window's height, rolled up or not.
    pub fn height_of(&self, window: Window) -> u32 {
        match window {
            // Each window's rolled-up height comes from its own module, because
            // each is its own title bar: they agree today, and a skin format
            // that disagreed would break here rather than silently.
            Window::Main if self.is_shade(window) => super::SHADE_HEIGHT,
            Window::Equalizer if self.is_shade(window) => eq::SHADE_HEIGHT,
            Window::Playlist if self.is_shade(window) => pl::SHADE_HEIGHT,
            Window::Main => super::WINDOW_HEIGHT,
            Window::Equalizer => eq::HEIGHT,
            Window::Playlist => pl::height(self.playlist_rows.max(pl::MIN_ROWS)),
        }
    }

    /// Where a window's top edge is, or `None` when it is not open.
    pub fn top_of(&self, window: Window) -> Option<u32> {
        if !self.is_open(window) {
            return None;
        }
        let mut y = 0;
        for other in ORDER {
            if other == window {
                return Some(y);
            }
            if self.is_open(other) {
                y += self.height_of(other);
            }
        }
        None
    }

    /// A window's whole rectangle inside the stack.
    pub fn area_of(&self, window: Window) -> Option<Area> {
        let y = self.top_of(window)?;
        Some(Area::new(0, y, super::WINDOW_WIDTH, self.height_of(window)))
    }

    /// The open windows and their tops, top to bottom.
    pub fn open_windows(&self) -> impl Iterator<Item = (Window, u32)> + '_ {
        ORDER
            .into_iter()
            .filter_map(move |window| self.top_of(window).map(|top| (window, top)))
    }

    /// The whole stack's size in skin pixels, which is what the OS window is
    /// resized to.
    pub fn size(&self) -> (u32, u32) {
        let height = ORDER
            .into_iter()
            .filter(|window| self.is_open(*window))
            .map(|window| self.height_of(window))
            .sum();
        (super::WINDOW_WIDTH, height)
    }

    /// Which window a skin-pixel row belongs to.
    ///
    /// The renderer does not need this — each window paints into its own
    /// clipped child `Ui`, so egui hit-tests for it — but it is how the tiling
    /// invariant is *stated*: every row of the stack belongs to exactly one
    /// window, and none past the end.
    #[cfg(test)]
    pub fn window_at(&self, y: u32) -> Option<Window> {
        self.open_windows()
            .find(|(window, top)| y >= *top && y < top + self.height_of(*window))
            .map(|(window, _)| window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_window_is_always_open_and_on_top() {
        let stack = Stack::default();
        assert!(stack.is_open(Window::Main));
        assert_eq!(stack.top_of(Window::Main), Some(0));
        assert!(!stack.is_open(Window::Equalizer));
        assert!(!stack.is_open(Window::Playlist));
        assert_eq!(stack.top_of(Window::Equalizer), None);
        assert_eq!(stack.size(), (275, 116));
        // Closing the main window is not a thing; asking does nothing.
        let mut stack = stack;
        stack.set_open(Window::Main, false);
        assert!(stack.is_open(Window::Main));
    }

    /// The open windows tile the stack: no gaps, no overlaps, and the total is
    /// the window's height.
    #[test]
    fn the_open_windows_tile_the_stack() {
        for (equalizer, playlist) in [(false, false), (true, false), (false, true), (true, true)] {
            for shade in [[false; 3], [true; 3], [false, true, false]] {
                let stack = Stack {
                    equalizer,
                    playlist,
                    shade,
                    ..Stack::default()
                };
                let (width, height) = stack.size();
                assert_eq!(width, 275, "the stack is always the classic width");
                let mut expected_top = 0;
                for (window, top) in stack.open_windows() {
                    assert_eq!(top, expected_top, "{window:?} is not flush");
                    let area = stack.area_of(window).expect("an open window has an area");
                    assert_eq!(area.y, top);
                    assert_eq!(area.height, stack.height_of(window));
                    assert!(area.width == 275);
                    expected_top += stack.height_of(window);
                }
                assert_eq!(expected_top, height, "the stack does not add up");
                // Every row belongs to exactly one window.
                for y in 0..height {
                    assert!(stack.window_at(y).is_some(), "row {y} belongs to nothing");
                }
                assert_eq!(stack.window_at(height), None, "past the end");
            }
        }
    }

    /// Rolling a window up makes the stack shorter by exactly what it lost, and
    /// moves everything under it up by the same amount.
    #[test]
    fn rolling_one_up_shortens_the_stack() {
        let mut stack = Stack {
            equalizer: true,
            playlist: true,
            ..Stack::default()
        };
        let (_, tall) = stack.size();
        let playlist_top = stack.top_of(Window::Playlist).expect("open");

        stack.toggle_shade(Window::Equalizer);
        let (_, short) = stack.size();
        assert_eq!(
            tall - short,
            eq::HEIGHT - super::super::SHADE_HEIGHT,
            "the stack lost the wrong amount"
        );
        assert_eq!(
            stack.top_of(Window::Playlist),
            Some(playlist_top - (eq::HEIGHT - super::super::SHADE_HEIGHT)),
            "the playlist did not move up"
        );
        assert_eq!(
            stack.height_of(Window::Equalizer),
            super::super::SHADE_HEIGHT
        );
        // …and rolling it back down restores the stack exactly.
        stack.toggle_shade(Window::Equalizer);
        assert_eq!(stack.size().1, tall);
        assert_eq!(stack.top_of(Window::Playlist), Some(playlist_top));
    }

    /// Closing a window closes the gap, rather than leaving a hole.
    #[test]
    fn closing_the_middle_window_closes_the_gap() {
        let mut stack = Stack {
            equalizer: true,
            playlist: true,
            ..Stack::default()
        };
        stack.set_open(Window::Equalizer, false);
        assert_eq!(
            stack.top_of(Window::Playlist),
            Some(super::super::WINDOW_HEIGHT),
            "the playlist did not close up under the main window"
        );
        assert_eq!(
            stack.size().1,
            super::super::WINDOW_HEIGHT + pl::height(pl::DEFAULT_ROWS)
        );
    }

    /// The playlist is the one window that resizes, and the stack follows.
    #[test]
    fn the_playlist_resizes_the_stack() {
        let mut stack = Stack {
            playlist: true,
            ..Stack::default()
        };
        let (_, before) = stack.size();
        stack.playlist_rows += 4;
        let (_, after) = stack.size();
        assert_eq!(after - before, 4 * pl::ROW_HEIGHT);
        // Fewer rows than the minimum are ignored, so the frame always fits.
        stack.playlist_rows = 0;
        assert_eq!(
            stack.height_of(Window::Playlist),
            pl::height(pl::MIN_ROWS),
            "the playlist shrank past its frame"
        );
    }

    /// Every window rolled up is three title bars and nothing else.
    #[test]
    fn all_three_rolled_up_is_three_bars() {
        let stack = Stack {
            equalizer: true,
            playlist: true,
            shade: [true; 3],
            ..Stack::default()
        };
        assert_eq!(stack.size(), (275, 3 * super::super::SHADE_HEIGHT));
        assert_eq!(stack.top_of(Window::Equalizer), Some(14));
        assert_eq!(stack.top_of(Window::Playlist), Some(28));
    }
}
