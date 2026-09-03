use super::App;
use super::route::Route;
use eframe::egui;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(app.theme.surface)
        .inner_margin(egui::Margin::symmetric(8, 12))
        .show(ui, |ui| {
            ui.set_min_width(200.0);
            ui.heading(
                egui::RichText::new("Fastcloud")
                    .strong()
                    .color(app.theme.accent),
            );
            ui.add_space(8.0);

            nav_item(ui, app, "🏠  Home", &Route::Home);
            nav_item(ui, app, "❤  Likes", &Route::Likes);
            nav_item(ui, app, "🕒  Recent", &Route::Recent);
            nav_item(ui, app, "👥  Following", &Route::Following);
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            nav_item(ui, app, "⚙  Settings", &Route::Settings);

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                if app.demo {
                    ui.label(
                        egui::RichText::new("demo mode")
                            .small()
                            .color(app.theme.accent),
                    );
                }
                ui.label(
                    egui::RichText::new(app.version_build)
                        .small()
                        .color(app.theme.text_dim),
                );
            });
        });
}

fn nav_item(ui: &mut egui::Ui, app: &mut App, label: &str, route: &Route) {
    let active = std::mem::discriminant(&app.route) == std::mem::discriminant(route);
    let text = if active {
        egui::RichText::new(label).color(app.theme.accent).strong()
    } else {
        egui::RichText::new(label).color(app.theme.text)
    };
    let resp = ui.selectable_label(active, text);
    if resp.clicked() {
        app.route = route.clone();
    }
}
