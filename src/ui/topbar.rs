use super::App;
use super::design::components::SidebarMode;
use super::design::widgets as airwave;
use super::route::Route;
use super::theme::{Metrics, Type};
use eframe::egui;

/// Main navbar: cloud logo, primary pages, the shared search field, then the
/// account and player actions.
///
/// Metrics come from soundcloud.com's own tokens (`--header-height: 46px`,
/// `.sc-text-h4` nav labels at 14/20 weight 600, `.sc-input` 36px tall with a
/// 3px radius). The panel (see `ui::App::ui`) owns the fill and the horizontal
/// padding, so the bar's grey runs edge to edge.
pub const BAR_HEIGHT: f32 = Metrics::HEADER_H;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let language = app.settings.language;
    if app.settings.background_image.is_some() {
        paint_wallpaper_titlebar(ui);
    }
    let total = ui.available_width();
    let brand_width = if total < 900.0 { 54.0 } else { 156.0 };
    let search_w = (total * 0.40)
        .clamp(280.0, 600.0)
        .min((total - brand_width - 180.0).max(160.0));
    let search_left = ui.max_rect().left() + (total - search_w) * 0.5;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = Metrics::SP_1;
        brand(app, ui, brand_width);
        history_controls(app, ui);
        ui.add_space((search_left - ui.cursor().left() - Metrics::SP_1).max(0.0));
        search_field(app, ui, search_w);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = Metrics::SP_15;
            let mini_tint = if app.mini_open() {
                app.theme.accent
            } else {
                app.theme.text_dim
            };
            if super::icons::show(ui, super::icons::Icon::Shrink, 17.0, mini_tint)
                .on_hover_text(if app.mini_open() {
                    language.text(
                        "Close the mini player (Ctrl+M)",
                        "Закрыть мини-плеер (Ctrl+M)",
                    )
                } else {
                    language.text("Mini player (Ctrl+M)", "Мини-плеер (Ctrl+M)")
                })
                .clicked()
            {
                app.toggle_mini();
            }
            let queue_tint = if app.show_queue {
                app.theme.accent
            } else {
                app.theme.text_dim
            };
            if super::icons::show(ui, super::icons::Icon::Queue, 18.0, queue_tint)
                .on_hover_text(if app.show_queue {
                    language.text("Close Next up (Q)", "Закрыть очередь (Q)")
                } else {
                    language.text("Open Next up (Q)", "Открыть очередь (Q)")
                })
                .clicked()
            {
                app.show_queue = !app.show_queue;
            }
            network_indicator(app, ui);
        });
    });
}

fn brand(app: &mut App, ui: &mut egui::Ui, width: f32) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 40.0), egui::Sense::click());
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 24.0, rect.center().y),
        egui::Vec2::splat(28.0),
    );
    airwave::paint_glass_rect(
        ui,
        icon_rect.expand(4.0),
        Metrics::RADIUS_INPUT,
        app.theme,
        false,
        Some(app.theme.accent),
    );
    super::icons::paint(
        ui,
        super::icons::Icon::Cloud,
        icon_rect,
        21.0,
        app.theme.accent,
    );
    if width > 100.0 {
        ui.painter().text(
            egui::pos2(rect.left() + 49.0, rect.center().y - 5.0),
            egui::Align2::LEFT_CENTER,
            "FastCloud",
            Type::H4.font(),
            app.theme.text,
        );
        ui.painter().text(
            egui::pos2(rect.left() + 50.0, rect.center().y + 10.0),
            egui::Align2::LEFT_CENTER,
            "AIRWAVE",
            Type::MICRO.font(),
            app.theme.text_dim,
        );
    }
    if response.clicked() {
        app.navigate(Route::Home);
    }
    response.on_hover_text(
        app.settings
            .language
            .text("FastCloud Home", "Главная Fastcloud"),
    );
}

fn history_controls(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = Metrics::SP_HALF;
        let back = ui
            .add_enabled_ui(app.can_go_back(), |ui| {
                super::icons::icon_button(
                    ui,
                    super::icons::Icon::ArrowLeft,
                    18.0,
                    32.0,
                    app.theme.tokens.text_tertiary,
                    app.theme.text,
                    app.settings.language.text("Back", "Назад"),
                )
            })
            .inner;
        if back.clicked() {
            app.go_back();
        }
        let forward = ui
            .add_enabled_ui(app.can_go_forward(), |ui| {
                super::icons::icon_button(
                    ui,
                    super::icons::Icon::ArrowRight,
                    18.0,
                    32.0,
                    app.theme.tokens.text_tertiary,
                    app.theme.text,
                    app.settings.language.text("Forward", "Вперёд"),
                )
            })
            .inner;
        if forward.clicked() {
            app.go_forward();
        }
        if super::icons::icon_button(
            ui,
            super::icons::Icon::Home,
            18.0,
            32.0,
            app.theme.tokens.text_tertiary,
            app.theme.text,
            app.settings.language.text("Home", "Главная"),
        )
        .clicked()
        {
            app.navigate(Route::Home);
        }
    });
}

fn paint_wallpaper_titlebar(ui: &egui::Ui) {
    let rect = ui.max_rect();
    let mut mesh = egui::Mesh::default();
    let top = egui::Color32::from_white_alpha(13);
    let bottom = egui::Color32::from_white_alpha(4);
    for (position, color) in [
        (rect.left_top(), top),
        (rect.right_top(), top),
        (rect.right_bottom(), bottom),
        (rect.left_bottom(), bottom),
    ] {
        mesh.colored_vertex(position, color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(mesh));
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        egui::Stroke::new(0.5, egui::Color32::from_white_alpha(18)),
    );
}

/// A slow request has been in flight this long before we say anything.
const BUSY_AFTER: std::time::Duration = std::time::Duration::from_millis(1000);

/// Spinner + reason while SoundCloud is slow or rate-limiting us.
///
/// Nothing is drawn for a quick request: only calls that outlast
/// [`BUSY_AFTER`], or an active 429 cool-down, are worth a word.
fn network_indicator(app: &mut App, ui: &mut egui::Ui) {
    let net = app.player.api().activity();
    let cooldown = net.cooldown_left();
    if cooldown.is_none() && !net.busy(BUSY_AFTER) {
        return;
    }
    // The bar is otherwise static; keep the spinner turning.
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(100));
    let (label, tint, tooltip) = match cooldown {
        Some(left) => (
            format!("Rate limited · {}s", left.as_secs().max(1)),
            app.theme.accent,
            "SoundCloud asked us to slow down; requests resume automatically",
        ),
        None => (
            "Waiting for SoundCloud".to_owned(),
            app.theme.text_dim,
            "The request is taking a while",
        ),
    };
    ui.add(egui::Label::new(Type::CAPTION.rich(&label, tint)).truncate())
        .on_hover_text(tooltip);
    super::icons::spinner(ui, 14.0, tint);
}

fn search_field(app: &mut App, ui: &mut egui::Ui, width: f32) {
    let mut clear = false;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 38.0), egui::Sense::hover());
    if app.settings.background_image.is_some() {
        airwave::paint_clear_glass_rect(ui, rect, Metrics::RADIUS_PILL, app.theme, None);
    } else {
        airwave::paint_glass_rect(ui, rect, Metrics::RADIUS_PILL, app.theme, false, None);
    }
    let response = ui
        .scope_builder(
            egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(12.0, 4.0))),
            |ui| {
                ui.horizontal_centered(|ui| {
                    super::icons::show_static(
                        ui,
                        super::icons::Icon::Search,
                        15.0,
                        app.theme.text_dim,
                    );
                    let reserved = if app.search_query.is_empty() {
                        48.0
                    } else {
                        20.0
                    };
                    let input = ui.add(
                        egui::TextEdit::singleline(&mut app.search_query)
                            .id(egui::Id::new("global-search-input"))
                            .hint_text(
                                app.settings
                                    .language
                                    .text("What do you want to hear?", "Что хочешь послушать?"),
                            )
                            .font(Type::BODY.font())
                            .frame(egui::Frame::NONE)
                            .desired_width((ui.available_width() - reserved).max(80.0)),
                    );
                    if app.search_query.is_empty() {
                        ui.label(Type::CAPTION.rich("Ctrl+K", app.theme.text_dim));
                    } else if super::icons::show(
                        ui,
                        super::icons::Icon::X,
                        15.0,
                        app.theme.text_dim,
                    )
                    .on_hover_text(app.settings.language.text("Clear", "Очистить"))
                    .clicked()
                    {
                        clear = true;
                    }
                    input
                })
                .inner
            },
        )
        .inner;

    if app.search_focus_requested {
        response.request_focus();
        app.search_focus_requested = false;
    }

    if response.gained_focus() && !matches!(app.route, Route::Search(_)) {
        app.navigate(Route::Search(app.search_query.clone()));
    }

    if clear {
        app.search_query.clear();
        app.commit_search_now();
        app.navigate(Route::Search(String::new()));
        response.request_focus();
    } else if response.changed() {
        app.mark_search_edited();
        app.navigate(Route::Search(app.search_query.clone()));
    }

    let focus_results = response.has_focus()
        && super::views::search_query_is_ready(&app.search_query)
        && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
    if focus_results {
        app.commit_search_now();
        app.search_selection = 0;
        response.surrender_focus();
    }

    if response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
        app.commit_search_now();
        app.navigate(Route::Search(app.search_query.clone()));
    }
    if response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        response.surrender_focus();
    }

    if response.has_focus() && app.search_query.trim().is_empty() && !app.search_history.is_empty()
    {
        let history = app
            .search_history
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        let mut selected = None;
        egui::Area::new(egui::Id::new("search-history-popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(response.rect.left_bottom() + egui::vec2(0.0, 5.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .fill(app.theme.surface)
                    .stroke(egui::Stroke::new(1.0, app.theme.separator))
                    .corner_radius(Metrics::RADIUS)
                    .show(ui, |ui| {
                        ui.set_min_width(width.min(420.0));
                        ui.label(Type::CAPTION.rich("Recent searches", app.theme.text_dim));
                        for query in history {
                            if ui
                                .add_sized(
                                    [ui.available_width(), 28.0],
                                    egui::Button::new(Type::BODY.rich(&query, app.theme.text))
                                        .fill(egui::Color32::TRANSPARENT)
                                        .stroke(egui::Stroke::NONE),
                                )
                                .clicked()
                            {
                                selected = Some(query);
                            }
                        }
                    });
            });
        if let Some(query) = selected {
            app.search_query = query;
            app.commit_search_now();
            app.navigate(Route::Search(app.search_query.clone()));
        }
    }
}

/// Keep the account control anchored to the rail's bottom, not floating in
/// the title bar. The same menu remains available in compact mode.
pub(super) fn sidebar_profile(app: &mut App, ui: &mut egui::Ui, mode: SidebarMode) {
    let account = app.account();
    let name = account
        .as_ref()
        .map(|me| me.username.clone())
        .unwrap_or_else(|| app.display_name());
    let avatar = account
        .as_ref()
        .and_then(|me| me.avatar_url.clone())
        .filter(|url| !url.is_empty());
    let initial = name
        .chars()
        .find(|c| !c.is_whitespace())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "F".to_owned());
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 46.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        if resp.hovered() {
            airwave::paint_clear_glass_rect(ui, rect, Metrics::RADIUS_INPUT, app.theme, None);
        }
        let center = egui::pos2(
            if mode == SidebarMode::Compact {
                rect.center().x
            } else {
                rect.left() + 25.0
            },
            rect.center().y,
        );
        let disc = egui::Rect::from_center_size(center, egui::vec2(30.0, 30.0));
        let mut painted = false;
        if let Some(url) = &avatar {
            let image = egui::Image::new(url)
                .show_loading_spinner(false)
                .fit_to_exact_size(disc.size())
                .corner_radius(15.0);
            if image.load_for_size(ui.ctx(), disc.size()).is_ok() {
                image.paint_at(ui, disc);
                painted = true;
            }
        }
        if !painted {
            ui.painter().circle_filled(center, 15.0, app.theme.accent);
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                initial,
                Type::H3.font(),
                app.theme.on_accent,
            );
        }
        if mode == SidebarMode::Wide {
            ui.painter().text(
                egui::pos2(rect.left() + 49.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &name,
                Type::H4.font(),
                app.theme.text,
            );
            super::icons::paint(
                ui,
                super::icons::Icon::ChevronDown,
                egui::Rect::from_center_size(
                    egui::pos2(rect.right() - 17.0, rect.center().y),
                    egui::vec2(16.0, 16.0),
                ),
                14.0,
                app.theme.text_dim,
            );
        }
    }
    let resp = resp
        .on_hover_text(name)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    egui::Popup::menu(&resp).show(|ui| {
        profile_menu(app, ui);
    });
}

/// Profile dropdown like soundcloud.com: Profile / Likes / Stations /
/// Following / Tracks / Settings.
fn profile_menu(app: &mut App, ui: &mut egui::Ui) {
    let language = app.settings.language;
    ui.set_min_width(210.0);
    // Own profile when the account is known, else Settings (where you
    // connect one) — the row must never dead-end.
    let own = app
        .account()
        .map(|me| Route::UserDetail(me.id))
        .unwrap_or(Route::Settings);

    let mut chosen: Option<Route> = None;
    if menu_row(
        ui,
        super::icons::Icon::User,
        language.text("Profile", "Профиль"),
    ) {
        chosen = Some(own.clone());
    }
    if menu_row(
        ui,
        super::icons::Icon::Heart,
        language.text("Likes", "Лайки"),
    ) {
        app.library_tab = super::views::LibraryTab::Likes;
        chosen = Some(Route::Library);
    }
    // Stations are a native page, not a browser trip.
    if menu_row(
        ui,
        super::icons::Icon::Music,
        language.text("Stations", "Станции"),
    ) {
        app.library_tab = super::views::LibraryTab::Stations;
        chosen = Some(Route::Library);
    }
    if menu_row(
        ui,
        super::icons::Icon::Users,
        language.text("Following", "Подписки"),
    ) {
        chosen = Some(Route::Following);
    }
    if menu_row(
        ui,
        super::icons::Icon::Disc,
        language.text("Tracks", "Треки"),
    ) {
        chosen = Some(own);
    }
    ui.separator();
    if menu_row(
        ui,
        super::icons::Icon::Shrink,
        language.text("Mini player", "Мини-плеер"),
    ) {
        app.toggle_mini();
        ui.close();
    }
    if menu_row(
        ui,
        super::icons::Icon::Settings,
        language.text("Settings", "Настройки"),
    ) {
        chosen = Some(Route::Settings);
    }
    if let Some(route) = chosen {
        app.navigate(route);
        ui.close();
    }
}

fn menu_row(ui: &mut egui::Ui, icon: super::icons::Icon, label: &str) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = Metrics::SP_125;
        super::icons::show_static(ui, icon, 16.0, ui.visuals().text_color());
        if ui.button(label).clicked() {
            clicked = true;
        }
    });
    clicked
}
