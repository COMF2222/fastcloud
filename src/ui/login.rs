//! The screen shown before the app has an application to call SoundCloud with.
//!
//! Fastcloud cannot ship API keys (see [`crate::auth`]), so a fresh install has
//! nothing to fetch or play with. This is the way out of that: one button
//! guides both browser approvals SoundCloud requires, registers the user's own
//! application, and signs that application into the account.
//!
//! It is a *gate*, not a page: [`eframe::App::ui`] draws this instead of the
//! interface until there is a connection. Falling through to the app with no
//! credentials is what used to happen, and it silently showed the demo library
//! as though it were the user's own.

use super::App;
use super::connect::Connection;
use super::theme::{Metrics, Type};
use eframe::egui;

/// Where SoundCloud sells the subscription its API requires.
const ARTIST_PRO_URL: &str = "https://checkout.soundcloud.com/artist/buy/artist-pro";

/// Draw the connect screen. Returns nothing: it acts through `app`.
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.theme;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme.bg))
        .show(ui, |ui| {
            // A single centred column, as wide as reading comfort allows.
            let card = 460.0;
            let available = ui.available_size();
            let pad_x = ((available.x - card) / 2.0).max(0.0);
            let rect = egui::Rect::from_min_size(
                ui.max_rect().min + egui::vec2(pad_x, (available.y * 0.14).min(140.0)),
                egui::vec2(card, available.y),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.vertical_centered(|ui| body(app, ui));
            });
        });
}

fn body(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.theme;
    ui.label(Type::H1.rich("Fastcloud", theme.text));
    ui.add_space(Metrics::SP_HALF);
    ui.label(Type::BODY.rich("SoundCloud, native and light.", theme.text_dim));
    ui.add_space(Metrics::SP_4);

    match app.connection.clone() {
        Connection::Starting => {
            waiting(app, ui, "Looking for your SoundCloud connection…");
        }
        Connection::Disconnected => idle(app, ui),
        Connection::Pairing { code, url } => pairing(app, ui, &code, &url),
        Connection::Registering => {
            waiting(app, ui, "Finish setting up Fastcloud in your browser…");
            if let Some(url) = &app.auth_url {
                ui.add_space(Metrics::SP_1);
                ui.hyperlink_to("Open the final SoundCloud approval", url);
            }
        }
        Connection::Connected { .. } => {
            ui.label(Type::H4.rich("Your SoundCloud account is required", theme.text));
            ui.add_space(Metrics::SP_1);
            ui.label(Type::BODY.rich(
                "Fastcloud opens your library only after SoundCloud confirms who you are.",
                theme.text_dim,
            ));
            ui.add_space(Metrics::SP_2);
            app.sign_in_controls(ui);
        }
        Connection::NeedsArtistPro(message) => needs_artist_pro(app, ui, &message),
        Connection::Failed(message) => failed(app, ui, &message),
    }

    ui.add_space(Metrics::SP_5);
    ui.label(Type::CAPTION.rich(
        "Fastcloud is not affiliated with SoundCloud.",
        theme.text_dim,
    ));
}

/// The first thing a new user sees.
fn idle(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.theme;
    // A registration already on this machine only needs a token, so the button
    // says what it will actually do rather than promising a browser.
    if app.stored_app {
        if connect_button(app, ui, "Reconnect to SoundCloud") {
            app.reconnect();
        }
        ui.add_space(Metrics::SP_15);
        ui.label(Type::CAPTION.rich(
            "This machine already has a SoundCloud app registered, but could not \
             reach SoundCloud at launch.",
            theme.text_dim,
        ));
        return;
    }
    if connect_button(app, ui, "Connect your SoundCloud account") {
        app.start_connect();
    }
    ui.add_space(Metrics::SP_15);
    ui.label(Type::CAPTION.rich(
        "Opens SoundCloud in your browser. Fastcloud never sees your password — \
         SoundCloud hands back an app key of your own, which is stored in your \
         operating system's keyring.",
        theme.text_dim,
    ));
    ui.add_space(Metrics::SP_15);
    ui.label(Type::CAPTION.rich(
        "SoundCloud only issues API keys to accounts with an Artist Pro \
         subscription. That is their requirement for the API, not ours.",
        theme.text_dim,
    ));
}

/// A code is out: the user is signing in on soundcloud.com.
fn pairing(app: &mut App, ui: &mut egui::Ui, code: &str, url: &str) {
    let theme = app.theme;
    ui.horizontal(|ui| {
        // Centre the spinner and its label together.
        let width = 260.0;
        ui.add_space((ui.available_width() - width).max(0.0) / 2.0);
        super::icons::spinner(ui, 18.0, theme.accent);
        ui.add_space(Metrics::SP_1);
        ui.label(Type::BODY.rich("Waiting for SoundCloud…", theme.text));
    });
    ui.add_space(Metrics::SP_2);
    ui.label(Type::BODY.rich(
        "Sign in on the page that opened. If it did not open, go to \
         soundcloud.com/activate and enter this code:",
        theme.text_dim,
    ));
    ui.add_space(Metrics::SP_1);
    // The code is the fallback path, so it is shown big and selectable rather
    // than as a hint: a user reading it off to a phone needs to see it.
    ui.label(Type::H1.rich(code, theme.accent));
    ui.add_space(Metrics::SP_1);
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - 260.0).max(0.0) / 2.0);
        if ui.button("Open the page again").clicked() {
            let _ = webbrowser::open(url);
        }
        if ui.button("Copy the code").clicked() {
            ui.ctx().copy_text(code.to_owned());
            app.toast("Code copied");
        }
    });
    ui.add_space(Metrics::SP_2);
    if ui.button("Cancel").clicked() {
        app.cancel_connect();
    }
}

/// Anything that takes a moment and cannot be cancelled usefully.
fn waiting(app: &App, ui: &mut egui::Ui, what: &str) {
    let theme = app.theme;
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - 280.0).max(0.0) / 2.0);
        super::icons::spinner(ui, 18.0, theme.accent);
        ui.add_space(Metrics::SP_1);
        ui.label(Type::BODY.rich(what, theme.text));
    });
}

/// The account signed in but cannot have API keys.
fn needs_artist_pro(app: &mut App, ui: &mut egui::Ui, message: &str) {
    let theme = app.theme;
    ui.label(Type::H4.rich("This account cannot get API keys", theme.text));
    ui.add_space(Metrics::SP_1);
    ui.label(Type::BODY.rich(message, theme.text_dim));
    ui.add_space(Metrics::SP_2);
    if connect_button(app, ui, "See Artist Pro") {
        let _ = webbrowser::open(ARTIST_PRO_URL);
    }
    ui.add_space(Metrics::SP_1);
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - 240.0).max(0.0) / 2.0);
        if ui.button("Try another account").clicked() {
            app.start_connect();
        }
    });
}

/// Everything else, in SoundCloud's own words where they gave any.
fn failed(app: &mut App, ui: &mut egui::Ui, message: &str) {
    let theme = app.theme;
    ui.label(Type::H4.rich("Could not connect", theme.text));
    ui.add_space(Metrics::SP_1);
    ui.label(Type::BODY.rich(message, theme.error));
    ui.add_space(Metrics::SP_2);
    if connect_button(app, ui, "Try again") {
        app.start_connect();
    }
}

/// The one prominent button on this screen: accent-filled, wide, centred.
///
/// `egui::Button` is not used because the accent fill and the width would have
/// to be set through the style on every call, and this screen has one such
/// button in each of its states.
fn connect_button(app: &App, ui: &mut egui::Ui, label: &str) -> bool {
    let theme = app.theme;
    let size = egui::vec2(ui.available_width().min(300.0), 44.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if response.hovered() {
            theme.accent_dim
        } else {
            theme.accent
        };
        ui.painter().rect_filled(rect, 22.0, fill);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            Type::H4.font(),
            theme.on_accent,
        );
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
