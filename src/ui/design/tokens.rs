//! Semantic Airwave color roles.

use eframe::egui::Color32;

use super::primitives::Colors;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Dark,
    Light,
}

/// Colors named by purpose. Shared widgets should use these roles instead of
/// depending on a particular graphite or orange value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticColors {
    pub scheme: ColorScheme,
    pub canvas: Color32,
    pub surface: Color32,
    pub surface_raised: Color32,
    pub surface_hover: Color32,
    /// Translucent material used for floating navigation and feature panels.
    pub glass: Color32,
    /// Lighter sheet for content laid over a user wallpaper.
    pub glass_clear: Color32,
    /// Denser glass for controls that must stay readable above artwork.
    pub glass_strong: Color32,
    /// Pointer/selection state for glass components.
    pub glass_hover: Color32,
    /// Outer edge of a glass sheet.
    pub glass_border: Color32,
    /// Inner top edge that catches the virtual light source.
    pub glass_highlight: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_tertiary: Color32,
    pub border_subtle: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub link: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub overlay: Color32,
}

impl SemanticColors {
    pub fn resolve(scheme: ColorScheme, accent: Color32) -> Self {
        let (canvas, surface, surface_raised, surface_hover) = match scheme {
            ColorScheme::Dark => (
                Colors::GRAPHITE_950,
                with_alpha(Colors::GRAPHITE_900, 232),
                with_alpha(Colors::GRAPHITE_850, 240),
                with_alpha(Colors::GRAPHITE_800, 244),
            ),
            ColorScheme::Light => (
                Colors::CLOUD_25,
                Color32::from_rgba_unmultiplied(255, 255, 255, 224),
                with_alpha(Colors::CLOUD_50, 240),
                with_alpha(Colors::CLOUD_100, 248),
            ),
        };
        let (text_primary, text_secondary, text_tertiary, link) = match scheme {
            ColorScheme::Dark => (
                Colors::INK_DARK_PRIMARY,
                Colors::INK_DARK_SECONDARY,
                Colors::INK_DARK_TERTIARY,
                Colors::LINK_DARK,
            ),
            ColorScheme::Light => (
                Colors::CLOUD_900,
                Colors::INK_LIGHT_SECONDARY,
                Colors::INK_LIGHT_TERTIARY,
                Colors::LINK_LIGHT,
            ),
        };
        let accent_hover = match scheme {
            ColorScheme::Dark => blend(accent, Color32::WHITE, 0.16),
            ColorScheme::Light => blend(accent, Color32::BLACK, 0.10),
        };

        Self {
            scheme,
            canvas,
            surface,
            surface_raised,
            surface_hover,
            glass: match scheme {
                ColorScheme::Dark => Color32::from_rgba_unmultiplied(22, 24, 31, 206),
                ColorScheme::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 204),
            },
            glass_clear: match scheme {
                ColorScheme::Dark => Color32::from_rgba_unmultiplied(15, 17, 23, 142),
                ColorScheme::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 154),
            },
            glass_strong: match scheme {
                ColorScheme::Dark => Color32::from_rgba_unmultiplied(25, 27, 35, 238),
                ColorScheme::Light => Color32::from_rgba_unmultiplied(252, 253, 255, 238),
            },
            glass_hover: match scheme {
                ColorScheme::Dark => Color32::from_rgba_unmultiplied(42, 45, 56, 232),
                ColorScheme::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 246),
            },
            glass_border: match scheme {
                ColorScheme::Dark => Color32::from_rgba_unmultiplied(255, 255, 255, 34),
                ColorScheme::Light => Color32::from_rgba_unmultiplied(24, 29, 38, 30),
            },
            glass_highlight: match scheme {
                ColorScheme::Dark => Color32::from_rgba_unmultiplied(255, 255, 255, 58),
                ColorScheme::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 230),
            },
            text_primary,
            text_secondary,
            text_tertiary,
            border_subtle: match scheme {
                ColorScheme::Dark => Color32::from_rgba_unmultiplied(255, 255, 255, 18),
                ColorScheme::Light => Color32::from_rgba_unmultiplied(21, 23, 28, 24),
            },
            accent,
            accent_hover,
            accent_soft: blend(surface, accent, 0.18),
            on_accent: contrast_text(accent),
            link,
            success: Colors::SUCCESS,
            warning: Colors::WARNING,
            danger: Colors::DANGER,
            overlay: Color32::from_rgba_unmultiplied(5, 6, 9, 168),
        }
    }

    pub fn dark(accent: Color32) -> Self {
        Self::resolve(ColorScheme::Dark, accent)
    }

    pub fn light(accent: Color32) -> Self {
        Self::resolve(ColorScheme::Light, accent)
    }

    pub fn is_dark(self) -> bool {
        self.scheme == ColorScheme::Dark
    }
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub(crate) fn blend(from: Color32, to: Color32, amount: f32) -> Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |a: u8, b: u8| ((a as f32 * (1.0 - amount)) + (b as f32 * amount)).round() as u8;
    Color32::from_rgba_unmultiplied(
        channel(from.r(), to.r()),
        channel(from.g(), to.g()),
        channel(from.b(), to.b()),
        channel(from.a(), to.a()),
    )
}

pub(crate) fn contrast_text(background: Color32) -> Color32 {
    let luminance = relative_luminance(background);
    let white_contrast = 1.05 / (luminance + 0.05);
    let black_contrast = (luminance + 0.05) / 0.05;
    if black_contrast >= white_contrast {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

fn relative_luminance(color: Color32) -> f32 {
    fn channel(value: u8) -> f32 {
        let value = value as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contrast_ratio(a: Color32, b: Color32) -> f32 {
        let a = relative_luminance(a);
        let b = relative_luminance(b);
        let (light, dark) = if a >= b { (a, b) } else { (b, a) };
        (light + 0.05) / (dark + 0.05)
    }

    #[test]
    fn schemes_have_distinct_surface_levels() {
        for colors in [
            SemanticColors::dark(Colors::PULSE_ORANGE),
            SemanticColors::light(Colors::PULSE_ORANGE),
        ] {
            assert_ne!(colors.canvas, colors.surface);
            assert_ne!(colors.surface, colors.surface_raised);
            assert_ne!(colors.surface_raised, colors.surface_hover);
        }
    }

    #[test]
    fn primary_text_meets_wcag_aa_on_the_canvas() {
        for colors in [
            SemanticColors::dark(Colors::PULSE_ORANGE),
            SemanticColors::light(Colors::PULSE_ORANGE),
        ] {
            assert!(contrast_ratio(colors.text_primary, colors.canvas) >= 4.5);
        }
    }

    #[test]
    fn custom_accent_drives_all_interactive_roles() {
        let custom = Color32::from_rgb(0x31, 0xC4, 0xFF);
        let colors = SemanticColors::dark(custom);
        assert_eq!(colors.accent, custom);
        assert_ne!(colors.accent_hover, Colors::PULSE_ORANGE);
        assert_ne!(colors.accent_soft, Colors::PULSE_ORANGE);
        assert!(contrast_ratio(colors.on_accent, colors.accent) >= 4.5);
    }

    #[test]
    fn glass_roles_are_translucent_and_layered() {
        for colors in [
            SemanticColors::dark(Colors::PULSE_ORANGE),
            SemanticColors::light(Colors::PULSE_ORANGE),
        ] {
            assert!(colors.glass.a() < 255);
            assert!(colors.glass_clear.a() < colors.glass.a());
            assert!(colors.glass_strong.a() > colors.glass.a());
            assert!(colors.glass_border.a() < colors.glass_highlight.a());
            assert_ne!(colors.glass, colors.surface);
        }
    }
}
