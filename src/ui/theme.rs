#![allow(dead_code)]

use eframe::egui;

pub const ORANGE: egui::Color32 = egui::Color32::from_rgb(255, 85, 17);
pub const ORANGE_DIM: egui::Color32 = egui::Color32::from_rgb(150, 60, 15);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub bg: egui::Color32,
    pub surface: egui::Color32,
    pub surface_hover: egui::Color32,
    pub text: egui::Color32,
    pub text_dim: egui::Color32,
    pub accent: egui::Color32,
    pub accent_dim: egui::Color32,
    pub separator: egui::Color32,
}

impl Theme {
    pub fn dark(accent: egui::Color32) -> Self {
        Self {
            bg: egui::Color32::from_rgb(18, 18, 20),
            surface: egui::Color32::from_rgb(28, 28, 32),
            surface_hover: egui::Color32::from_rgb(40, 40, 46),
            text: egui::Color32::from_rgb(240, 240, 240),
            text_dim: egui::Color32::from_rgb(150, 150, 155),
            accent,
            accent_dim: ORANGE_DIM,
            separator: egui::Color32::from_rgb(45, 45, 50),
        }
    }

    pub fn light(accent: egui::Color32) -> Self {
        Self {
            bg: egui::Color32::from_rgb(248, 248, 250),
            surface: egui::Color32::from_rgb(255, 255, 255),
            surface_hover: egui::Color32::from_rgb(235, 235, 240),
            text: egui::Color32::from_rgb(25, 25, 30),
            text_dim: egui::Color32::from_rgb(110, 110, 120),
            accent,
            accent_dim: ORANGE_DIM,
            separator: egui::Color32::from_rgb(225, 225, 230),
        }
    }

    pub fn from_mode(mode: crate::config::ThemeMode, accent: egui::Color32) -> Self {
        match mode {
            crate::config::ThemeMode::Dark | crate::config::ThemeMode::System => Self::dark(accent),
            crate::config::ThemeMode::Light => Self::light(accent),
        }
    }

    pub fn apply(&self, ctx: &egui::Context) {
        let visuals = if self.bg.r() > 128 {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        let mut visuals = visuals;
        visuals.panel_fill = self.bg;
        visuals.window_fill = self.surface;
        visuals.selection.bg_fill = self.accent_dim;
        visuals.selection.stroke = egui::Stroke::new(1.0, self.accent);
        visuals.widgets.noninteractive.bg_fill = self.surface;
        visuals.widgets.hovered.bg_fill = self.surface_hover;
        ctx.set_visuals(visuals);
    }
}

/// Extract a dominant accent color from cover art pixels.
pub fn accent_from_image(rgba: &[u8]) -> egui::Color32 {
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
        best = egui::Color32::from_rgb(r, g, b);
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
        assert!(dark.bg.r() < 128);
        let light = Theme::from_mode(crate::config::ThemeMode::Light, ORANGE);
        assert!(light.bg.r() > 128);
    }
}
