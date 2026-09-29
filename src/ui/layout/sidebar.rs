use eframe::egui;

use crate::config::{Language, QuickAccessShortcut};

use super::super::App;
use super::super::design::components::{self, SidebarMode};
use super::super::design::widgets as airwave;
use super::super::icons::{self, Icon};
use super::super::route::Route;
use super::super::theme::{Metrics, Type};

pub fn show(app: &mut App, ui: &mut egui::Ui, mode: SidebarMode) {
    let language = app.settings.language;
    ui.set_min_height(ui.available_height());
    paint_sidebar_atmosphere(app, ui);
    ui.add_space(Metrics::SP_1);

    if nav_item(
        app,
        ui,
        mode,
        Icon::Home,
        language.text("Home", "Главная"),
        is_home(&app.route),
    )
    .clicked()
    {
        app.navigate(Route::Home);
    }
    if nav_item(
        app,
        ui,
        mode,
        Icon::Search,
        language.text("Search", "Поиск"),
        matches!(app.route, Route::Search(_)),
    )
    .clicked()
    {
        app.open_search();
    }
    if nav_item(
        app,
        ui,
        mode,
        Icon::Disc,
        language.text("Catalog", "Каталог"),
        matches!(app.route, Route::Catalog),
    )
    .clicked()
    {
        app.navigate(Route::Catalog);
    }
    if nav_item(
        app,
        ui,
        mode,
        Icon::Bell,
        language.text("Feed", "Лента"),
        matches!(app.route, Route::Feed),
    )
    .clicked()
    {
        app.navigate(Route::Feed);
    }
    if nav_item(
        app,
        ui,
        mode,
        Icon::Headphones,
        language.text("Library", "Библиотека"),
        is_library(&app.route),
    )
    .clicked()
    {
        app.navigate(Route::Library);
    }

    if mode == SidebarMode::Wide
        && let Some((shortcut, remove)) = quick_access(app, ui)
    {
        if remove {
            let before = app.settings.quick_access.clone();
            app.settings.toggle_quick_access(shortcut);
            if let Err(error) = app.settings.save() {
                app.settings.quick_access = before;
                app.toast(format!("Could not save Quick Access: {error}"));
            }
        } else {
            activate_quick_access(app, shortcut);
        }
    }

    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        super::super::topbar::sidebar_profile(app, ui, mode);
        ui.add_space(Metrics::SP_1);
        if nav_item(
            app,
            ui,
            mode,
            Icon::Settings,
            language.text("Settings", "Настройки"),
            matches!(app.route, Route::Settings),
        )
        .clicked()
        {
            app.navigate(Route::Settings);
        }
        if nav_item(
            app,
            ui,
            mode,
            Icon::Shrink,
            if app.sidebar_collapsed {
                language.text("Expand sidebar", "Развернуть панель")
            } else {
                language.text("Collapse sidebar", "Свернуть панель")
            },
            false,
        )
        .clicked()
        {
            app.sidebar_collapsed = !app.sidebar_collapsed;
        }
        let language_label = match language {
            Language::English => "English",
            Language::Russian => "Русский",
        };
        if nav_item(app, ui, mode, Icon::Globe, language_label, false).clicked() {
            app.settings.language = match language {
                Language::English => Language::Russian,
                Language::Russian => Language::English,
            };
            if let Err(error) = app.settings.save() {
                app.settings.language = language;
                app.toast(format!("Could not save language: {error}"));
            }
        }
        ui.add_space(Metrics::SP_1);
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, app.theme.separator),
        );
    });
}

fn quick_access(app: &App, ui: &mut egui::Ui) -> Option<(QuickAccessShortcut, bool)> {
    let available = (ui.available_height() - components::SIDEBAR_FOOTER_RESERVE).max(0.0);
    if available < components::SIDEBAR_QUICK_ITEM_HEIGHT + 38.0
        || !app
            .settings
            .quick_access
            .iter()
            .any(QuickAccessShortcut::is_media)
    {
        return None;
    }

    ui.add_space(Metrics::SP_2);
    ui.horizontal(|ui| {
        ui.label(Type::MICRO.rich(
            app.settings.language.text("QUICK ACCESS", "БЫСТРЫЙ ДОСТУП"),
            app.theme.tokens.text_tertiary,
        ));
        let rect = ui.available_rect_before_wrap();
        ui.painter().hline(
            rect.x_range(),
            rect.center().y,
            egui::Stroke::new(1.0, app.theme.tokens.glass_border),
        );
    });
    ui.add_space(Metrics::SP_HALF);

    let mut clicked = None;
    egui::ScrollArea::vertical()
        .id_salt("sidebar-quick-access")
        .max_height(available - 38.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for shortcut in app
                .settings
                .quick_access
                .iter()
                .filter(|item| item.is_media())
            {
                let response = quick_access_item(app, ui, shortcut);
                if response.clicked() {
                    clicked = Some((shortcut.clone(), false));
                }
                response.context_menu(|ui| {
                    if ui
                        .button(
                            app.settings
                                .language
                                .text("Remove from Quick Access", "Убрать из быстрого доступа"),
                        )
                        .clicked()
                    {
                        clicked = Some((shortcut.clone(), true));
                        ui.close();
                    }
                });
            }
        });
    clicked
}

fn quick_access_item(
    app: &App,
    ui: &mut egui::Ui,
    shortcut: &QuickAccessShortcut,
) -> egui::Response {
    let (icon, label, description) = quick_access_meta(shortcut, app.settings.language);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), components::SIDEBAR_QUICK_ITEM_HEIGHT),
        egui::Sense::click(),
    );
    if response.hovered() {
        airwave::paint_clear_glass_rect(
            ui,
            rect.shrink2(egui::vec2(0.0, 2.0)),
            Metrics::RADIUS_INPUT,
            app.theme,
            Some(app.theme.accent),
        );
    }
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 18.0, rect.center().y),
        egui::Vec2::splat(30.0),
    );
    let artwork = match shortcut {
        QuickAccessShortcut::Track { artwork_url, .. }
        | QuickAccessShortcut::Playlist { artwork_url, .. }
        | QuickAccessShortcut::Album { artwork_url, .. } => artwork_url.as_deref(),
        _ => None,
    };
    let painted = artwork.is_some_and(|url| {
        let image = egui::Image::new(url)
            .show_loading_spinner(false)
            .fit_to_exact_size(icon_rect.size())
            .corner_radius(Metrics::RADIUS);
        if image.load_for_size(ui.ctx(), icon_rect.size()).is_ok() {
            image.paint_at(ui, icon_rect);
            true
        } else {
            false
        }
    });
    if !painted {
        ui.painter()
            .rect_filled(icon_rect, Metrics::RADIUS, app.theme.tokens.accent_soft);
        icons::paint(ui, icon, icon_rect, 15.0, app.theme.accent);
    }

    let text_clip = egui::Rect::from_min_max(
        egui::pos2(icon_rect.right() + 9.0, rect.top()),
        egui::pos2(rect.right() - 8.0, rect.bottom()),
    );
    let painter = ui.painter().with_clip_rect(text_clip);
    painter.text(
        egui::pos2(text_clip.left(), rect.center().y - 7.0),
        egui::Align2::LEFT_CENTER,
        label,
        Type::CAPTION.font(),
        app.theme.text,
    );
    painter.text(
        egui::pos2(text_clip.left(), rect.center().y + 9.0),
        egui::Align2::LEFT_CENTER,
        description,
        Type::MICRO.font(),
        app.theme.tokens.text_tertiary,
    );

    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(format!(
            "{} {label}",
            app.settings.language.text("Open", "Открыть")
        ))
}

fn quick_access_meta(
    shortcut: &QuickAccessShortcut,
    language: crate::config::Language,
) -> (Icon, &str, &str) {
    match shortcut {
        QuickAccessShortcut::Track { title, artist, .. } => (Icon::Music, title, artist),
        QuickAccessShortcut::Playlist { title, artist, .. } => (Icon::ListPlus, title, artist),
        QuickAccessShortcut::Album { title, artist, .. } => (Icon::Disc, title, artist),
        QuickAccessShortcut::Likes => (
            Icon::Heart,
            language.text("Likes", "Лайки"),
            language.text("Your saved tracks", "Сохранённые треки"),
        ),
        QuickAccessShortcut::DailyMix => (
            Icon::Sparkles,
            language.text("Daily mix", "Микс дня"),
            language.text("Based on your plays", "По вашей истории"),
        ),
        QuickAccessShortcut::Fresh => (
            Icon::TrendingUp,
            language.text("Fresh", "Новинки"),
            language.text("New from following", "От подписок"),
        ),
        QuickAccessShortcut::Vibe => (
            Icon::AudioLines,
            "Vibe",
            language.text("Describe a feeling", "По настроению"),
        ),
        QuickAccessShortcut::History => (
            Icon::Clock,
            language.text("History", "История"),
            language.text("Recently played", "Недавно слушали"),
        ),
        QuickAccessShortcut::Station => (
            Icon::Disc,
            language.text("Station", "Станция"),
            language.text("Keep music flowing", "Музыка без пауз"),
        ),
    }
}

fn activate_quick_access(app: &mut App, shortcut: QuickAccessShortcut) {
    match shortcut {
        QuickAccessShortcut::Track { id, .. } => app.navigate(Route::TrackDetail(id)),
        QuickAccessShortcut::Playlist { id, .. } | QuickAccessShortcut::Album { id, .. } => {
            app.navigate(Route::PlaylistDetail(id));
        }
        // Old action entries remain readable from settings but are never shown.
        _ => {}
    }
}
fn nav_item(
    app: &App,
    ui: &mut egui::Ui,
    mode: SidebarMode,
    icon: Icon,
    label: &str,
    active: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), components::NAV_ITEM_HEIGHT),
        egui::Sense::click(),
    );
    let hovered = response.hovered();
    if active || hovered {
        airwave::paint_glass_rect(
            ui,
            rect.shrink2(egui::vec2(1.0, 2.0)),
            Metrics::RADIUS_INPUT,
            app.theme,
            active,
            active.then_some(app.theme.accent),
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(0.5),
            Metrics::RADIUS_INPUT,
            egui::Stroke::new(1.0, app.theme.tokens.accent_hover),
            egui::StrokeKind::Inside,
        );
    }
    if active {
        paint_wave_mark(ui, rect, app.theme.accent);
    }

    let icon_center_x = match mode {
        SidebarMode::Compact => rect.center().x,
        SidebarMode::Wide => rect.left() + 25.0,
    };
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(icon_center_x, rect.center().y),
        egui::Vec2::splat(24.0),
    );
    icons::paint(
        ui,
        icon,
        icon_rect,
        19.0,
        if active {
            app.theme.text
        } else {
            app.theme.text_dim
        },
    );
    if mode == SidebarMode::Wide {
        ui.painter().text(
            egui::pos2(rect.left() + 50.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            Type::H4.font(),
            if active {
                app.theme.text
            } else {
                app.theme.text_dim
            },
        );
        if active {
            let dot = egui::pos2(rect.right() - 15.0, rect.center().y);
            ui.painter().circle_filled(dot, 3.0, app.theme.accent);
            ui.painter().circle_stroke(
                dot,
                6.0,
                egui::Stroke::new(1.0, app.theme.accent.gamma_multiply(0.35)),
            );
        }
    }

    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if mode == SidebarMode::Compact {
        response.on_hover_text(label)
    } else {
        response
    }
}

fn paint_sidebar_atmosphere(app: &App, ui: &egui::Ui) {
    let rect = ui.max_rect();
    let source = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 30.0, rect.top() + 72.0),
        egui::Vec2::splat(22.0),
    );
    ui.painter().add(
        egui::epaint::Shadow {
            offset: [0, 0],
            blur: 82,
            spread: 24,
            color: app.theme.accent.gamma_multiply(0.12),
        }
        .as_shape(source, 11.0),
    );
    ui.painter().vline(
        rect.right() - 0.5,
        rect.y_range(),
        egui::Stroke::new(1.0, app.theme.tokens.glass_border),
    );
}

fn paint_wave_mark(ui: &egui::Ui, rect: egui::Rect, color: egui::Color32) {
    let heights = [6.0, 12.0, 9.0];
    for (index, height) in heights.into_iter().enumerate() {
        let bar = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 4.0 + index as f32 * 3.0, rect.center().y),
            egui::vec2(2.0, height),
        );
        ui.painter().rect_filled(bar, 1.0, color);
    }
}

fn is_home(route: &Route) -> bool {
    matches!(route, Route::Home)
}

fn is_library(route: &Route) -> bool {
    matches!(
        route,
        Route::Library | Route::Likes | Route::Recent | Route::Following | Route::PlaylistDetail(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_children_keep_the_library_destination_active() {
        assert!(is_library(&Route::Likes));
        assert!(is_library(&Route::Recent));
        assert!(is_library(&Route::PlaylistDetail(42)));
        assert!(!is_library(&Route::Feed));
    }
}
