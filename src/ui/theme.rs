//! SoundCloud's design tokens, ported to egui.
//!
//! Values are not guesses: they were read out of soundcloud.com's own
//! stylesheet (`app-*.css`), where the theme is declared as CSS custom
//! properties on `.theme-dark` / `.theme-light`. The two layers below mirror
//! that structure:
//!
//! * [`Palette`] — the raw tokens, one field per `--*-color` SoundCloud
//!   declares. Both themes are `const`, so the values are auditable.
//! * [`Theme`] — the semantic roles the UI paints with (`bg`, `surface`,
//!   `text`, …), derived from a palette plus the accent of the moment.
//!
//! [`Metrics`] and the type scale carry the same treatment: the ladders come
//! from `--spacing-*`, `--borderRadiuses-*`, `--artwork-*-size` and
//! `--typography-*`, expressed in device-independent pixels.
//!
//! ```text
//! .theme-dark  { --surface-color:#121212; --highlight-color:#303030;
//!                --primary-color:#fff;    --secondary-color:#999;   … }
//! .theme-light { --surface-color:#fff;    --highlight-color:#f3f3f3;
//!                --primary-color:#121212; --secondary-color:#666;   … }
//! ```

#![allow(dead_code)]

use eframe::egui;
use egui::Color32;

// ===========================================================================
// Raw tokens
// ===========================================================================

/// `--special-color` — SoundCloud orange, the one hue both themes share.
pub const ORANGE: Color32 = Color32::from_rgb(0xFF, 0x55, 0x00);
/// The orange knocked back for rails and inactive fills.
pub const ORANGE_DIM: Color32 = Color32::from_rgb(0x96, 0x3C, 0x0F);

/// `--error-color`.
pub const ERROR: Color32 = Color32::from_rgb(0xD6, 0x13, 0x48);
/// `--success-color`.
pub const SUCCESS: Color32 = Color32::from_rgb(0x00, 0x88, 0x50);
/// `--artist-surface-color` (dark) — the "artist" accent.
pub const ARTIST: Color32 = Color32::from_rgb(0x5A, 0x45, 0xFD);
/// `--artist-pro-surface-color` (dark) — the Artist Pro badge bronze.
pub const ARTIST_PRO: Color32 = Color32::from_rgb(0x6F, 0x5B, 0x37);

/// One theme's raw tokens, exactly as soundcloud.com declares them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// `--surface-color`: the page itself.
    pub surface: Color32,
    /// `--highlight-color`: bars, cards, inputs, secondary buttons.
    pub highlight: Color32,
    /// `--primary-color`: body text and primary icons.
    pub primary: Color32,
    /// `--secondary-color`: metadata, counts, placeholder text.
    pub secondary: Color32,
    /// `--special-color`: the orange.
    pub special: Color32,
    /// `--link-color`: standard links.
    pub link: Color32,
    /// `--error-color`.
    pub error: Color32,
    /// `--success-color`.
    pub success: Color32,
    /// `--imageBorder-color`: hairlines, artwork edges, dividers.
    pub image_border: Color32,
    /// `--overlay-color`: scrims over artwork.
    pub overlay: Color32,
}

impl Palette {
    /// `.theme-dark`.
    pub const DARK: Self = Self {
        surface: Color32::from_rgb(0x12, 0x12, 0x12),
        highlight: Color32::from_rgb(0x30, 0x30, 0x30),
        primary: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        secondary: Color32::from_rgb(0x99, 0x99, 0x99),
        special: ORANGE,
        link: Color32::from_rgb(0x69, 0x9F, 0xFF),
        error: ERROR,
        success: SUCCESS,
        // hsla(0,0%,100%,0.15)
        image_border: Color32::from_rgba_premultiplied(38, 38, 38, 38),
        // hsla(0,0%,100%,0.4)
        overlay: Color32::from_rgba_premultiplied(102, 102, 102, 102),
    };

    /// `.theme-light,:root`.
    pub const LIGHT: Self = Self {
        surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        highlight: Color32::from_rgb(0xF3, 0xF3, 0xF3),
        primary: Color32::from_rgb(0x12, 0x12, 0x12),
        secondary: Color32::from_rgb(0x66, 0x66, 0x66),
        special: ORANGE,
        link: Color32::from_rgb(0x04, 0x4D, 0xD2),
        error: ERROR,
        success: SUCCESS,
        // rgba(18,18,18,0.15)
        image_border: Color32::from_rgba_premultiplied(3, 3, 3, 38),
        // rgba(18,18,18,0.4)
        overlay: Color32::from_rgba_premultiplied(7, 7, 7, 102),
    };

    /// True for the dark theme (`--primary-color` is the light one there).
    pub fn is_dark(&self) -> bool {
        let sum = self.surface.r() as u16 + self.surface.g() as u16 + self.surface.b() as u16;
        sum < 384
    }
}

// ===========================================================================
// Metrics
// ===========================================================================

/// Spacing, radii and component sizes, from `--spacing-*`,
/// `--borderRadiuses-*` and the component rules that use them.
pub struct Metrics;

impl Metrics {
    // --spacing-0_25x … --spacing-8x
    pub const SP_QUARTER: f32 = 2.0;
    pub const SP_HALF: f32 = 4.0;
    pub const SP_075: f32 = 6.0;
    pub const SP_1: f32 = 8.0;
    pub const SP_125: f32 = 10.0;
    pub const SP_15: f32 = 12.0;
    pub const SP_175: f32 = 14.0;
    pub const SP_2: f32 = 16.0;
    pub const SP_25: f32 = 20.0;
    pub const SP_3: f32 = 24.0;
    pub const SP_35: f32 = 28.0;
    pub const SP_4: f32 = 32.0;
    pub const SP_5: f32 = 40.0;
    pub const SP_6: f32 = 48.0;
    pub const SP_7: f32 = 56.0;
    pub const SP_8: f32 = 64.0;

    // --borderRadiuses-*
    /// Buttons, cards, banners — the radius SoundCloud uses almost everywhere.
    pub const RADIUS: u8 = 4;
    /// Dialogs and popovers.
    pub const RADIUS_LG: u8 = 8;
    /// Text inputs (`.sc-input`).
    pub const RADIUS_INPUT: u8 = 3;
    /// Pills and tags (`--tag-body-border-radius:100px`).
    pub const RADIUS_PILL: u8 = 100;

    /// `--header-height`.
    pub const HEADER_H: f32 = 46.0;
    /// `--play-controls-height`.
    pub const PLAYER_H: f32 = 48.0;
    /// `.sc-input` height.
    pub const INPUT_H: f32 = 36.0;
    /// `.sc-button-medium` min height.
    pub const BUTTON_H: f32 = 32.0;
    /// `.sc-button-small.sc-button-icon` min size.
    pub const ICON_BUTTON: f32 = 24.0;
    /// `--tag-body-height`.
    pub const TAG_H: f32 = 24.0;
    /// `.playbackTimeline__progressBackground` height.
    pub const TIMELINE_H: f32 = 2.0;
    /// `.l-container` gutter (`--spacing-2x`).
    pub const GUTTER: f32 = Self::SP_2;

    /// `--artwork-*-size`: the only artwork edges SoundCloud ships.
    pub const ARTWORK: [f32; 16] = [
        8.0, 16.0, 24.0, 32.0, 40.0, 48.0, 64.0, 104.0, 120.0, 128.0, 144.0, 160.0, 200.0, 232.0,
        320.0, 360.0,
    ];

    /// Snap an artwork edge to the nearest size SoundCloud actually serves,
    /// so covers are never resampled to an in-between size.
    pub fn artwork(px: f32) -> f32 {
        Self::ARTWORK
            .iter()
            .copied()
            .min_by(|a, b| (a - px).abs().total_cmp(&(b - px).abs()))
            .unwrap_or(px)
    }
}

// ===========================================================================
// Type scale
// ===========================================================================

/// A `--typography-*` entry: size, line height and the weight family.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub line_height: f32,
    pub weight: Weight,
    /// `--typography-heading6-text-transform: uppercase`.
    pub uppercase: bool,
    /// `letter-spacing`, in points (already resolved from `em`).
    pub tracking: f32,
}

impl TextStyle {
    pub const fn new(size: f32, line_height: f32, weight: Weight) -> Self {
        Self {
            size,
            line_height,
            weight,
            uppercase: false,
            tracking: 0.0,
        }
    }

    const fn caps(mut self, tracking: f32) -> Self {
        self.uppercase = true;
        self.tracking = tracking;
        self
    }

    /// The egui font this style resolves to.
    pub fn font(&self) -> egui::FontId {
        self.weight.font(self.size)
    }

    /// Apply the style's casing to a label.
    pub fn text(&self, s: &str) -> String {
        if self.uppercase {
            s.to_uppercase()
        } else {
            s.to_owned()
        }
    }

    /// A `RichText` carrying font, casing and tracking.
    pub fn rich(&self, s: &str, color: Color32) -> egui::RichText {
        let mut t = egui::RichText::new(self.text(s))
            .font(self.font())
            .color(color);
        if self.tracking != 0.0 {
            t = t.extra_letter_spacing(self.tracking);
        }
        t
    }
}

/// Which real font family a style uses. SoundCloud sets 400 for body and
/// 600 for headings; egui cannot synthesise weights, so each maps to a
/// family registered in [`crate::fonts`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    /// 400.
    Regular,
    /// 500/600 — headings, buttons, track titles.
    Medium,
    /// 700 — display sizes.
    Bold,
}

impl Weight {
    pub fn font(self, size: f32) -> egui::FontId {
        match self {
            Weight::Regular => regular(size),
            Weight::Medium => medium(size),
            Weight::Bold => bold(size),
        }
    }
}

/// `--typography-*`, resolved against SoundCloud's 14px rem
/// (`--typography-root-font-size: 0.875em`).
pub struct Type;

impl Type {
    /// 60/36, weight 600, −0.02em.
    pub const DISPLAY1: TextStyle = TextStyle::new(60.0, 36.0, Weight::Bold);
    /// 40/36, weight 600.
    pub const DISPLAY2: TextStyle = TextStyle::new(40.0, 36.0, Weight::Bold);
    /// 32/36, weight 600.
    pub const DISPLAY3: TextStyle = TextStyle::new(32.0, 36.0, Weight::Bold);
    /// 28/36 — page titles.
    pub const H1: TextStyle = TextStyle::new(28.0, 36.0, Weight::Bold);
    /// 22/28 — section headings ("More of what you like").
    pub const H2: TextStyle = TextStyle::new(22.0, 28.0, Weight::Bold);
    /// 17/24 — card group titles, dialog headings.
    pub const H3: TextStyle = TextStyle::new(17.0, 24.0, Weight::Medium);
    /// 14/20 — track titles, nav links, buttons.
    pub const H4: TextStyle = TextStyle::new(14.0, 20.0, Weight::Medium);
    /// 12/16 — timestamps, counts in a heavier cut.
    pub const H5: TextStyle = TextStyle::new(12.0, 16.0, Weight::Medium);
    /// 10/16 uppercase +0.1em — sidebar headers.
    pub const H6: TextStyle = TextStyle::new(10.0, 16.0, Weight::Medium).caps(1.0);
    /// 14/20 — body copy.
    pub const BODY: TextStyle = TextStyle::new(14.0, 20.0, Weight::Regular);
    /// 17/24 — lead paragraphs.
    pub const BODY_LG: TextStyle = TextStyle::new(17.0, 24.0, Weight::Regular);
    /// 12/16 — metadata under a title.
    pub const CAPTION: TextStyle = TextStyle::new(12.0, 16.0, Weight::Regular);
    /// 10/16 uppercase +0.071em — the smallest label SoundCloud ships.
    pub const MICRO: TextStyle = TextStyle::new(10.0, 16.0, Weight::Medium).caps(0.7);
}

// ===== Font helpers =====
//
// Real weights, not `RichText::strong()` (which only brightens the colour —
// that is why every label rendered at weight 400 before). The families are
// registered in `crate::fonts::install`.

/// Regular (400).
pub fn regular(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Proportional)
}

/// Medium (500) — track titles, nav, buttons.
pub fn medium(size: f32) -> egui::FontId {
    egui::FontId::new(
        size,
        egui::FontFamily::Name(crate::fonts::FAMILY_MEDIUM.into()),
    )
}

/// Bold (700) — section headings, page titles.
pub fn bold(size: f32) -> egui::FontId {
    egui::FontId::new(
        size,
        egui::FontFamily::Name(crate::fonts::FAMILY_BOLD.into()),
    )
}

// ===========================================================================
// Semantic roles
// ===========================================================================

/// The colours the UI paints with, derived from a [`Palette`].
///
/// Field names are roles, not values, so a view never has to know which
/// theme is active. `accent` is the only role that changes at runtime: it
/// follows the artwork when the user turns that on, and falls back to
/// `--special-color`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// The palette these roles came from.
    pub palette: Palette,
    /// True while the dark theme is active.
    pub dark: bool,

    /// The page (`--surface-color`).
    pub bg: Color32,
    /// Bars, cards, inputs, secondary buttons (`--highlight-color`).
    pub surface: Color32,
    /// A row or tile under the pointer.
    pub surface_hover: Color32,
    /// Body text and primary icons (`--primary-color`).
    pub text: Color32,
    /// Metadata (`--secondary-color`).
    pub text_dim: Color32,
    /// The accent of the moment (orange, or the artwork's hue).
    pub accent: Color32,
    /// The accent knocked back, for rails behind a filled bar.
    pub accent_dim: Color32,
    /// Readable ink on top of `accent`.
    pub on_accent: Color32,
    /// Hairlines and artwork edges (`--imageBorder-color`).
    pub separator: Color32,
    /// Standard links (`--link-color`).
    pub link: Color32,
    /// Destructive/error text (`--error-color`).
    pub error: Color32,
    /// Confirmation (`--success-color`).
    pub success: Color32,
    /// Scrim over artwork (`--overlay-color`).
    pub overlay: Color32,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark(ORANGE)
    }
}

impl Theme {
    /// Roles for a palette and an accent.
    pub const fn new(palette: Palette, accent: Color32) -> Self {
        // `Palette::is_dark` is not const-callable through a reference here,
        // so the same comparison is inlined.
        let dark =
            (palette.surface.r() as u16 + palette.surface.g() as u16 + palette.surface.b() as u16)
                < 384;
        Self {
            palette,
            dark,
            bg: palette.surface,
            surface: palette.highlight,
            // SoundCloud fades *text* on hover rather than lifting the
            // background; lists still need a hint, so the highlight is
            // nudged one step further from the page.
            surface_hover: if dark {
                Color32::from_rgb(0x3D, 0x3D, 0x3D)
            } else {
                Color32::from_rgb(0xE7, 0xE7, 0xE7)
            },
            text: palette.primary,
            text_dim: palette.secondary,
            accent,
            accent_dim: ORANGE_DIM,
            on_accent: Color32::WHITE,
            separator: palette.image_border,
            link: palette.link,
            error: palette.error,
            success: palette.success,
            overlay: palette.overlay,
        }
    }

    /// soundcloud.com's dark theme: `#121212` page, `#303030` panels, white
    /// text, `#999` metadata, orange accent.
    pub const fn dark(accent: Color32) -> Self {
        Self::new(Palette::DARK, accent)
    }

    /// soundcloud.com's light theme: white page, `#f3f3f3` panels, `#121212`
    /// text, `#666` metadata.
    pub const fn light(accent: Color32) -> Self {
        Self::new(Palette::LIGHT, accent)
    }

    /// Resolve a [`crate::config::ThemeMode`], asking the OS when the user
    /// picked "System".
    pub fn from_mode(mode: crate::config::ThemeMode, accent: Color32) -> Self {
        match mode {
            crate::config::ThemeMode::Dark => Self::dark(accent),
            crate::config::ThemeMode::Light => Self::light(accent),
            crate::config::ThemeMode::System => Self::dark(accent),
        }
    }

    /// Like [`Self::from_mode`], but "System" follows the desktop.
    pub fn from_mode_ctx(
        mode: crate::config::ThemeMode,
        accent: Color32,
        ctx: &egui::Context,
    ) -> Self {
        match mode {
            crate::config::ThemeMode::System => {
                match ctx
                    .input(|i| i.raw.system_theme)
                    .unwrap_or(egui::Theme::Dark)
                {
                    egui::Theme::Light => Self::light(accent),
                    egui::Theme::Dark => Self::dark(accent),
                }
            }
            other => Self::from_mode(other, accent),
        }
    }

    /// The accent at the strength SoundCloud gives a selected row.
    pub fn accent_wash(&self, strength: f32) -> Color32 {
        self.accent.gamma_multiply(strength)
    }

    /// Text faded the way SoundCloud fades a hovered label
    /// (`--link-primary-hover-color: hsla(0,0%,100%,0.4)`).
    pub fn faded(&self, color: Color32, hovered: bool) -> Color32 {
        if hovered {
            color.gamma_multiply(0.6)
        } else {
            color
        }
    }

    /// Apply the palette, the type scale and SoundCloud's metrics to egui's
    /// own widgets, so buttons, menus and text fields agree with the
    /// hand-painted views.
    pub fn apply(&self, ctx: &egui::Context) {
        let mut style = (*ctx.global_style()).clone();
        let visuals = &mut style.visuals;
        *visuals = if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.dark_mode = self.dark;
        visuals.panel_fill = self.bg;
        visuals.window_fill = self.surface;
        visuals.extreme_bg_color = self.bg;
        visuals.faint_bg_color = self.surface_hover;
        visuals.override_text_color = Some(self.text);
        visuals.weak_text_color = Some(self.text_dim);
        visuals.hyperlink_color = self.link;
        visuals.error_fg_color = self.error;
        visuals.warn_fg_color = self.accent;
        visuals.selection.bg_fill = self.accent.gamma_multiply(0.35);
        visuals.selection.stroke = egui::Stroke::new(1.0, self.accent);
        visuals.window_stroke = egui::Stroke::new(1.0, self.separator);
        visuals.window_corner_radius = egui::CornerRadius::same(Metrics::RADIUS_LG);
        visuals.menu_corner_radius = egui::CornerRadius::same(Metrics::RADIUS);
        visuals.text_cursor.stroke = egui::Stroke::new(2.0, self.accent);
        visuals.striped = false;
        visuals.slider_trailing_fill = true;
        visuals.handle_shape = egui::style::HandleShape::Circle;
        // `--button-focused-box-shadow: 0 0 0 2px <link> inset`
        visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;

        let corner = egui::CornerRadius::same(Metrics::RADIUS);
        for widget in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = corner;
            widget.bg_stroke = egui::Stroke::NONE;
            widget.fg_stroke = egui::Stroke::new(1.0, self.text);
            widget.expansion = 0.0;
        }
        visuals.widgets.noninteractive.corner_radius = corner;
        visuals.widgets.noninteractive.bg_fill = self.surface;
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, self.separator);
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, self.text);
        // `--button-secondary-*`: highlight fill, primary ink.
        visuals.widgets.inactive.bg_fill = self.surface;
        visuals.widgets.inactive.weak_bg_fill = self.surface;
        visuals.widgets.hovered.bg_fill = self.surface_hover;
        visuals.widgets.hovered.weak_bg_fill = self.surface_hover;
        visuals.widgets.active.bg_fill = self.accent;
        visuals.widgets.active.weak_bg_fill = self.accent;
        visuals.widgets.open.bg_fill = self.surface_hover;
        visuals.widgets.open.weak_bg_fill = self.surface_hover;

        use egui::FontFamily::Monospace;
        use egui::{FontId, TextStyle as Ts};
        style.text_styles = [
            (Ts::Small, Type::CAPTION.font()),
            (Ts::Body, Type::BODY.font()),
            (Ts::Button, Type::H4.font()),
            (Ts::Heading, Type::H2.font()),
            (Ts::Monospace, FontId::new(Type::CAPTION.size, Monospace)),
        ]
        .into();
        // `--spacing-1x` between controls, `6px 12px` inside a button.
        style.spacing.item_spacing = egui::vec2(Metrics::SP_1, Metrics::SP_075);
        style.spacing.button_padding = egui::vec2(Metrics::SP_15, Metrics::SP_075);
        style.spacing.interact_size = egui::vec2(Metrics::BUTTON_H, Metrics::BUTTON_H);
        style.spacing.menu_margin = egui::Margin::same(Metrics::SP_075 as i8);
        style.spacing.window_margin = egui::Margin::same(Metrics::SP_2 as i8);
        style.spacing.scroll = egui::style::ScrollStyle {
            bar_width: 8.0,
            floating_width: 6.0,
            floating_allocated_width: 0.0,
            handle_min_length: 28.0,
            bar_inner_margin: 3.0,
            bar_outer_margin: 2.0,
            dormant_background_opacity: 0.0,
            dormant_handle_opacity: 0.0,
            active_background_opacity: 0.0,
            active_handle_opacity: 0.55,
            interact_handle_opacity: 0.85,
            foreground_color: true,
            ..egui::style::ScrollStyle::floating()
        };
        style.interaction.selectable_labels = false;
        style.interaction.tooltip_delay = 0.4;
        style.animation_time = 0.12;
        style.url_in_tooltip = false;
        ctx.set_global_style(style);
    }
}

/// Extract a dominant accent color from cover art pixels.
pub fn accent_from_image(rgba: &[u8]) -> Color32 {
    // Average saturated bright pixels.
    let mut best = ORANGE;
    let mut best_score = 0.0f32;
    let mut acc_r = 0u64;
    let mut acc_g = 0u64;
    let mut acc_b = 0u64;
    let mut count = 0u64;
    for px in rgba.chunks_exact(4) {
        let r = px[0] as f32;
        let g = px[1] as f32;
        let b = px[2] as f32;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let sat = if max > 0.0 { (max - min) / max } else { 0.0 };
        let score = sat * max;
        if score > best_score {
            best_score = score;
        }
        if sat > 0.25 && max > 80.0 {
            acc_r += px[0] as u64;
            acc_g += px[1] as u64;
            acc_b += px[2] as u64;
            count += 1;
        }
    }
    let n = count.max(1);
    let r = (acc_r / n) as u8;
    let g = (acc_g / n) as u8;
    let b = (acc_b / n) as u8;
    if count > 0 {
        best = Color32::from_rgb(r, g, b);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_prefers_saturated() {
        let img = [
            255, 100, 20, 255, // orange
            250, 250, 250, 255, // white (skipped: low sat)
            20, 30, 40, 255, // dark (skipped)
            240, 110, 30, 255, // orange
        ];
        let c = accent_from_image(&img);
        assert!(c.r() > 200);
        assert!(c.g() < 150);
    }

    #[test]
    fn accent_fallback_on_gray() {
        let img = [128u8, 128, 128, 255];
        let c = accent_from_image(&img);
        assert_eq!(c, ORANGE);
    }

    #[test]
    fn theme_modes() {
        let dark = Theme::from_mode(crate::config::ThemeMode::Dark, ORANGE);
        assert!(dark.dark);
        assert_eq!(dark.bg, Color32::from_rgb(0x12, 0x12, 0x12));
        assert_eq!(dark.surface, Color32::from_rgb(0x30, 0x30, 0x30));
        assert_eq!(dark.text, Color32::WHITE);
        assert_eq!(dark.text_dim, Color32::from_rgb(0x99, 0x99, 0x99));

        let light = Theme::from_mode(crate::config::ThemeMode::Light, ORANGE);
        assert!(!light.dark);
        assert_eq!(light.bg, Color32::WHITE);
        assert_eq!(light.surface, Color32::from_rgb(0xF3, 0xF3, 0xF3));
        assert_eq!(light.text, Color32::from_rgb(0x12, 0x12, 0x12));
        assert_eq!(light.text_dim, Color32::from_rgb(0x66, 0x66, 0x66));
    }

    /// The palettes are transcriptions, not taste: they must keep matching
    /// `.theme-dark` / `.theme-light` in soundcloud.com's stylesheet.
    #[test]
    fn palettes_match_soundcloud_css() {
        assert!(Palette::DARK.is_dark());
        assert!(!Palette::LIGHT.is_dark());
        // --special-color is the same in both themes.
        assert_eq!(Palette::DARK.special, Palette::LIGHT.special);
        assert_eq!(Palette::DARK.special, Color32::from_rgb(255, 85, 0));
        // --link-color differs (#699fff dark, #044dd2 light).
        assert_ne!(Palette::DARK.link, Palette::LIGHT.link);
        // --error/--success are theme-independent.
        assert_eq!(Palette::DARK.error, Palette::LIGHT.error);
        assert_eq!(Palette::DARK.success, Palette::LIGHT.success);
    }

    /// The type ladder must stay monotonic and land on SoundCloud's sizes.
    #[test]
    fn type_scale_matches_typography_tokens() {
        assert_eq!(Type::H1.size, 28.0);
        assert_eq!(Type::H2.size, 22.0);
        assert_eq!(Type::H3.size, 17.0);
        assert_eq!(Type::H4.size, 14.0);
        assert_eq!(Type::H5.size, 12.0);
        assert_eq!(Type::H6.size, 10.0);
        assert_eq!(Type::BODY.size, 14.0);
        assert_eq!(Type::CAPTION.size, 12.0);
        let ladder = [
            Type::DISPLAY1.size,
            Type::DISPLAY2.size,
            Type::DISPLAY3.size,
            Type::H1.size,
            Type::H2.size,
            Type::H3.size,
            Type::H4.size,
            Type::H5.size,
            Type::H6.size,
        ];
        assert!(
            ladder.windows(2).all(|w| w[0] > w[1]),
            "type scale is not monotonic: {ladder:?}"
        );
        // --typography-heading6-text-transform: uppercase
        assert_eq!(Type::H6.text("Playlists"), "PLAYLISTS");
        assert_eq!(Type::MICRO.text("New"), "NEW");
        // Everything else keeps the caller's casing.
        assert_eq!(Type::H4.text("Playlists"), "Playlists");
        assert_eq!(Type::BODY.text("Playlists"), "Playlists");
    }

    #[test]
    fn artwork_snaps_to_served_sizes() {
        assert_eq!(Metrics::artwork(160.0), 160.0);
        assert_eq!(Metrics::artwork(150.0), 144.0);
        assert_eq!(Metrics::artwork(42.0), 40.0);
        assert_eq!(Metrics::artwork(1.0), 8.0);
        assert_eq!(Metrics::artwork(10_000.0), 360.0);
    }

    /// `apply` must survive a real context and leave our roles in place.
    #[test]
    fn apply_installs_palette_and_type_scale() {
        let ctx = egui::Context::default();
        let theme = Theme::dark(ORANGE);
        theme.apply(&ctx);
        let style = ctx.global_style();
        assert_eq!(style.visuals.panel_fill, theme.bg);
        assert_eq!(style.visuals.window_fill, theme.surface);
        assert_eq!(style.visuals.override_text_color, Some(theme.text));
        assert_eq!(style.visuals.hyperlink_color, theme.link);
        assert!(style.visuals.dark_mode);
        assert_eq!(
            style.text_styles[&egui::TextStyle::Body].size,
            Type::BODY.size
        );
        assert_eq!(
            style.text_styles[&egui::TextStyle::Heading].size,
            Type::H2.size
        );
        assert_eq!(style.spacing.item_spacing.x, Metrics::SP_1);
    }
}
