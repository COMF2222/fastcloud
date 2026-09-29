//! Stable Airwave component geometry currently consumed by the legacy shell.
//!
//! Add a token only when its component starts using it. That keeps dead-code
//! warnings useful while the redesign is migrated in small slices.

use super::primitives::Radius;

pub const COMMAND_BAR_HEIGHT: f32 = 56.0;
pub const CONTEXT_RAIL_WIDTH: f32 = 380.0;
pub const PLAYER_DECK_HEIGHT: f32 = 92.0;
pub const SIDEBAR_COMPACT_WIDTH: f32 = 72.0;
pub const SIDEBAR_WIDE_WIDTH: f32 = 204.0;
pub const NAV_ITEM_HEIGHT: f32 = 44.0;
pub const SIDEBAR_QUICK_ITEM_HEIGHT: f32 = 42.0;
pub const SIDEBAR_FOOTER_RESERVE: f32 = 204.0;
pub const CONTROL_HEIGHT: f32 = 36.0;
pub const CONTROL_RADIUS: u8 = Radius::CONTROL;
pub const CARD_RADIUS: u8 = Radius::CARD;
pub const PANEL_RADIUS: u8 = Radius::PANEL;
pub const PILL_RADIUS: u8 = Radius::PILL;

const WIDE_SIDEBAR_AT: f32 = 1_120.0;
const DOCK_CONTEXT_RAIL_AT: f32 = 1_100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarMode {
    Compact,
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellLayout {
    pub sidebar: SidebarMode,
    pub sidebar_width: f32,
    pub dock_context_rail: bool,
}

impl ShellLayout {
    pub fn resolve(viewport_width: f32, context_rail_open: bool) -> Self {
        let sidebar = if viewport_width >= WIDE_SIDEBAR_AT {
            SidebarMode::Wide
        } else {
            SidebarMode::Compact
        };
        let sidebar_width = match sidebar {
            SidebarMode::Compact => SIDEBAR_COMPACT_WIDTH,
            SidebarMode::Wide => SIDEBAR_WIDE_WIDTH,
        };
        Self {
            sidebar,
            sidebar_width,
            dock_context_rail: context_rail_open && viewport_width >= DOCK_CONTEXT_RAIL_AT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_shell_keeps_labels_visible() {
        let layout = ShellLayout::resolve(1_200.0, true);
        assert_eq!(layout.sidebar, SidebarMode::Wide);
        assert_eq!(layout.sidebar_width, SIDEBAR_WIDE_WIDTH);
        assert!(layout.dock_context_rail);
    }

    #[test]
    fn compact_shell_preserves_content_on_small_windows() {
        let layout = ShellLayout::resolve(1_000.0, false);
        assert_eq!(layout.sidebar, SidebarMode::Compact);
        assert_eq!(layout.sidebar_width, SIDEBAR_COMPACT_WIDTH);
    }

    #[test]
    fn context_rail_becomes_an_overlay_on_small_windows() {
        let layout = ShellLayout::resolve(900.0, true);
        assert_eq!(layout.sidebar, SidebarMode::Compact);
        assert!(!layout.dock_context_rail);
    }
}
