use eframe::egui;

use super::super::App;
use super::super::design::components;
use super::super::theme::Metrics;

/// Show the queue over content when the window is too narrow to dock it.
pub fn show_overlay(app: &mut App, ctx: &egui::Context) {
    let viewport = ctx.content_rect();
    let width = components::CONTEXT_RAIL_WIDTH.min((viewport.width() - 96.0).max(240.0));
    let height = (viewport.height()
        - components::COMMAND_BAR_HEIGHT
        - components::PLAYER_DECK_HEIGHT
        - Metrics::SP_4)
        .max(240.0);
    let theme = app.theme;

    egui::Area::new(egui::Id::new("airwave-context-rail-overlay"))
        .anchor(
            egui::Align2::RIGHT_TOP,
            egui::vec2(
                -Metrics::SP_2,
                components::COMMAND_BAR_HEIGHT + Metrics::SP_2,
            ),
        )
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme.tokens.surface_raised)
                .stroke(egui::Stroke::new(1.0, theme.separator))
                .corner_radius(Metrics::RADIUS_LG)
                .inner_margin(egui::Margin::same(Metrics::SP_2 as i8))
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.set_height(height);
                    super::super::queue::show(app, ui);
                });
        });
}
