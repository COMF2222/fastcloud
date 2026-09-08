use super::App;
use super::route::Route;
use super::theme::{Metrics, Type};
use eframe::egui;

/// SoundCloud-style top navbar: cloud logo, Home / Feed / Library,
/// a wide search field, then messages / notifications / avatar.
///
/// Metrics come from soundcloud.com's own tokens (`--header-height: 46px`,
/// `.sc-text-h4` nav labels at 14/20 weight 600, `.sc-input` 36px tall with a
/// 3px radius). The panel (see `ui::App::ui`) owns the fill and the horizontal
/// padding, so the bar's grey runs edge to edge.
pub const BAR_HEIGHT: f32 = Metrics::HEADER_H;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = Metrics::SP_2;

        // Cloud logo → Home.
        if wordmark(app, ui).clicked() {
            app.navigate(Route::Home);
        }

        nav_link(ui, app, "Home", Route::Home);
        nav_link(ui, app, "Feed", Route::Feed);
        nav_link(ui, app, "Library", Route::Library);

        // Search: everything between the nav and the right-hand actions.
        // Reserve exactly what the actions need (avatar, mini player
        // + the network indicator), so the field is as wide as it can be.
        let actions_w = 145.0 + network_width(app);
        let search_w = (ui.available_width() - actions_w).max(160.0);
        search_field(app, ui, search_w);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = Metrics::SP_15;
            profile_button(app, ui);
            let mini_tint = if app.mini_open() {
                app.theme.accent
            } else {
                app.theme.text_dim
            };
            if super::icons::show(ui, super::icons::Icon::Shrink, 17.0, mini_tint)
                .on_hover_text(if app.mini_open() {
                    "Close the mini player (Ctrl+M)"
                } else {
                    "Mini player (Ctrl+M)"
                })
                .clicked()
            {
                app.toggle_mini();
            }
            network_indicator(app, ui);
        });
    });
}

fn wordmark(app: &App, ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(48.0, 32.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        super::icons::paint(ui, super::icons::Icon::Cloud, rect, 38.0, app.theme.text);
    }
    response
        .on_hover_text("Home")
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A slow request has been in flight this long before we say anything.
const BUSY_AFTER: std::time::Duration = std::time::Duration::from_millis(1000);

/// Width the network indicator needs, so the search field can reserve it.
fn network_width(app: &App) -> f32 {
    let net = app.player.api().activity();
    if net.cooldown_left().is_some() {
        150.0
    } else if net.busy(BUSY_AFTER) {
        130.0
    } else {
        0.0
    }
}

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

/// Rounded search field with the magnifier inside it, like soundcloud.com:
/// `--input-default-background-color` fill, 3px corners, no visible border
/// until focus.
fn search_field(app: &mut App, ui: &mut egui::Ui, width: f32) {
    let height = 30.0;
    // Centre the box on the bar's midline: `allocate_exact_size` in a
    // `horizontal_centered` row already centres it, but the text inside a
    // frameless TextEdit sits at the top of its own rect, so the field is
    // laid out from an explicit centred rect instead.
    let (outer, _) = ui.allocate_exact_size(
        egui::vec2(width, ui.available_height()),
        egui::Sense::hover(),
    );
    let rect = egui::Rect::from_center_size(outer.center(), egui::vec2(width, height));
    let radius = Metrics::RADIUS_INPUT as f32;
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, radius, app.theme.surface);
        super::icons::paint(
            ui,
            super::icons::Icon::Search,
            egui::Rect::from_center_size(
                egui::pos2(rect.right() - 16.0, rect.center().y),
                egui::vec2(16.0, 16.0),
            ),
            15.0,
            app.theme.text_dim,
        );
    }
    // The editor gets a band the height of one line, centred in the box, so
    // the text and its hint sit on the field's midline.
    let line = Type::BODY.line_height;
    let inner = egui::Rect::from_min_max(
        egui::pos2(rect.left() + Metrics::SP_125, rect.center().y - line / 2.0),
        egui::pos2(rect.right() - 30.0, rect.center().y + line / 2.0),
    );
    let mut field = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let resp = field.add(
        egui::TextEdit::singleline(&mut app.search_query)
            .id(egui::Id::new("topbar-search"))
            .hint_text("Search for artists, bands, tracks, podcasts")
            .font(Type::BODY.font())
            .margin(egui::Margin::ZERO)
            .frame(egui::Frame::NONE)
            .desired_width(inner.width()),
    );
    if resp.changed() && !app.search_query.trim().is_empty() {
        let q = app.search_query.clone();
        if !matches!(&app.route, Route::Search(cur) if *cur == q) {
            app.navigate(Route::Search(q));
        }
    }
    if resp.lost_focus()
        && ui.input(|i| i.key_pressed(egui::Key::Enter))
        && !app.search_query.trim().is_empty()
    {
        let q = app.search_query.clone();
        app.navigate(Route::Search(q));
    }
}

/// Avatar + chevron opening the profile menu.
fn profile_button(app: &mut App, ui: &mut egui::Ui) {
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
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(48.0, 28.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let center = egui::pos2(rect.left() + 14.0, rect.center().y);
        let disc = egui::Rect::from_center_size(center, egui::vec2(28.0, 28.0));
        let mut painted = false;
        if let Some(url) = &avatar {
            let image = egui::Image::new(url)
                .show_loading_spinner(false)
                .fit_to_exact_size(disc.size())
                .corner_radius(14.0);
            if image.load_for_size(ui.ctx(), disc.size()).is_ok() {
                image.paint_at(ui, disc);
                painted = true;
            }
        }
        if !painted {
            ui.painter().circle_filled(center, 14.0, app.theme.accent);
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                initial,
                Type::H3.font(),
                app.theme.on_accent,
            );
        }
        super::icons::paint(
            ui,
            super::icons::Icon::ChevronDown,
            egui::Rect::from_center_size(
                egui::pos2(rect.right() - 8.0, rect.center().y),
                egui::vec2(16.0, 16.0),
            ),
            14.0,
            app.theme.text_dim,
        );
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
    ui.set_min_width(210.0);
    // Own profile when the account is known, else Settings (where you
    // connect one) — the row must never dead-end.
    let own = app
        .account()
        .map(|me| Route::UserDetail(me.id))
        .unwrap_or(Route::Settings);

    let mut chosen: Option<Route> = None;
    if menu_row(ui, super::icons::Icon::User, "Profile") {
        chosen = Some(own.clone());
    }
    if menu_row(ui, super::icons::Icon::Heart, "Likes") {
        app.library_tab = super::views::LibraryTab::Likes;
        chosen = Some(Route::Library);
    }
    // Stations are a native page, not a browser trip.
    if menu_row(ui, super::icons::Icon::Music, "Stations") {
        app.library_tab = super::views::LibraryTab::Stations;
        chosen = Some(Route::Library);
    }
    if menu_row(ui, super::icons::Icon::Users, "Following") {
        chosen = Some(Route::Following);
    }
    if menu_row(ui, super::icons::Icon::Disc, "Tracks") {
        chosen = Some(own);
    }
    ui.separator();
    if menu_row(ui, super::icons::Icon::Shrink, "Mini player") {
        app.toggle_mini();
        ui.close();
    }
    if menu_row(ui, super::icons::Icon::Settings, "Settings") {
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

fn nav_link(ui: &mut egui::Ui, app: &mut App, label: &str, route: Route) {
    let active = std::mem::discriminant(&app.route) == std::mem::discriminant(&route)
        || matches!(
            (&app.route, &route),
            (Route::Likes, Route::Library)
                | (Route::Recent, Route::Library)
                | (Route::Following, Route::Library)
                | (Route::PlaylistDetail(_), Route::Library)
        );
    // `.sc-text-h4` in both states; only the colour changes, as on the site.
    let text = Type::H4.rich(
        label,
        if active {
            app.theme.text
        } else {
            app.theme.text_dim
        },
    );
    let resp = ui
        .add(egui::Label::new(text).sense(egui::Sense::click()))
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    // Active nav gets SoundCloud's orange underline, not a text underline.
    if active {
        let r = resp.rect;
        ui.painter().hline(
            r.x_range(),
            r.bottom() + 3.0,
            egui::Stroke::new(2.0, app.theme.text),
        );
    }
    if resp.clicked() {
        app.navigate(route);
    }
}
