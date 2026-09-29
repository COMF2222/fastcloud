//! Raw Airwave values. These names describe the value, not where it is used.

use eframe::egui::Color32;

/// Neutral and brand colors used to build semantic themes.
pub struct Colors;

impl Colors {
    pub const GRAPHITE_950: Color32 = Color32::from_rgb(0x09, 0x0A, 0x0D);
    pub const GRAPHITE_900: Color32 = Color32::from_rgb(0x11, 0x13, 0x19);
    pub const GRAPHITE_850: Color32 = Color32::from_rgb(0x17, 0x1A, 0x21);
    pub const GRAPHITE_800: Color32 = Color32::from_rgb(0x1E, 0x22, 0x2B);

    pub const CLOUD_25: Color32 = Color32::from_rgb(0xF7, 0xF8, 0xFA);
    pub const CLOUD_50: Color32 = Color32::from_rgb(0xF0, 0xF2, 0xF5);
    pub const CLOUD_100: Color32 = Color32::from_rgb(0xE8, 0xEC, 0xF2);
    pub const CLOUD_900: Color32 = Color32::from_rgb(0x15, 0x17, 0x1C);

    pub const INK_DARK_PRIMARY: Color32 = Color32::from_rgb(0xF5, 0xF7, 0xFA);
    pub const INK_DARK_SECONDARY: Color32 = Color32::from_rgb(0xA0, 0xA8, 0xB5);
    pub const INK_DARK_TERTIARY: Color32 = Color32::from_rgb(0x6F, 0x77, 0x83);
    pub const INK_LIGHT_SECONDARY: Color32 = Color32::from_rgb(0x5F, 0x68, 0x75);
    pub const INK_LIGHT_TERTIARY: Color32 = Color32::from_rgb(0x6C, 0x74, 0x80);

    /// FastCloud's default accent. Custom user accents remain supported.
    pub const PULSE_ORANGE: Color32 = Color32::from_rgb(0xFF, 0x5B, 0x24);
    pub const LINK_DARK: Color32 = Color32::from_rgb(0x7A, 0xA7, 0xFF);
    pub const LINK_LIGHT: Color32 = Color32::from_rgb(0x1E, 0x5E, 0xD8);
    pub const SUCCESS: Color32 = Color32::from_rgb(0x38, 0xC7, 0x93);
    pub const WARNING: Color32 = Color32::from_rgb(0xF6, 0xB9, 0x4A);
    pub const DANGER: Color32 = Color32::from_rgb(0xF0, 0x5A, 0x72);
}

/// The 4-point spacing scale used by Airwave components.
pub struct Space;

impl Space {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
    pub const SECTION: f32 = 48.0;
}

/// Corner radii used by component tokens.
pub struct Radius;

impl Radius {
    pub const CONTROL: u8 = 10;
    pub const CARD: u8 = 14;
    pub const PANEL: u8 = 20;
    pub const PILL: u8 = u8::MAX;
}
