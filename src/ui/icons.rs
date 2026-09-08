//! Vector icons (Lucide, vendored under `assets/icons/`).
//!
//! Crisp at any size and tintable via [`show`]; unlike emoji glyphs they
//! can never turn into tofu squares.
//!
//! [`Icon`] is a catalogue: one variant per vendored file, whether or not a
//! view happens to reference it today. `every_icon_resolves` pins the set, and
//! `include_image!` fails the build if an asset goes missing, so an unused
//! variant is a spare rather than dead weight.

use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Icon {
    Play,
    Pause,
    SkipBack,
    SkipForward,
    Shuffle,
    Repeat,
    Repeat1,
    Volume,
    Volume1,
    Volume2,
    VolumeX,
    Heart,
    Search,
    Settings,
    X,
    Plus,
    Trash,
    Queue,
    Bell,
    User,
    UserPlus,
    Users,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    Ellipsis,
    Mic,
    External,
    Clock,
    Cloud,
    Music,
    ListPlus,
    Check,
    Pencil,
    Copy,
    Grip,
    Loader,
    Home,
    Sparkles,
    TrendingUp,
    Headphones,
    Info,
    Minus,
    CirclePlay,
    Refresh,
    Mail,
    Disc,
    BadgeCheck,
    ArrowLeft,
    ArrowRight,
    Grid,
    List,
    /// The Winamp mini player's button.
    Shrink,
    /// The visualiser toggle.
    AudioLines,
}

/// Image source for an icon (for custom compositions like buttons).
pub fn source(icon: Icon) -> egui::ImageSource<'static> {
    match icon {
        Icon::Play => egui::include_image!("../../assets/icons/play.svg"),
        Icon::Pause => egui::include_image!("../../assets/icons/pause.svg"),
        Icon::SkipBack => egui::include_image!("../../assets/icons/skip-back.svg"),
        Icon::SkipForward => egui::include_image!("../../assets/icons/skip-forward.svg"),
        Icon::Shuffle => egui::include_image!("../../assets/icons/shuffle.svg"),
        Icon::Repeat => egui::include_image!("../../assets/icons/repeat.svg"),
        Icon::Repeat1 => egui::include_image!("../../assets/icons/repeat-1.svg"),
        Icon::Volume => egui::include_image!("../../assets/icons/volume.svg"),
        Icon::Volume1 => egui::include_image!("../../assets/icons/volume-1.svg"),
        Icon::Volume2 => egui::include_image!("../../assets/icons/volume-2.svg"),
        Icon::VolumeX => egui::include_image!("../../assets/icons/volume-x.svg"),
        Icon::Heart => egui::include_image!("../../assets/icons/heart.svg"),
        Icon::Search => egui::include_image!("../../assets/icons/search.svg"),
        Icon::Settings => egui::include_image!("../../assets/icons/settings.svg"),
        Icon::X => egui::include_image!("../../assets/icons/x.svg"),
        Icon::Plus => egui::include_image!("../../assets/icons/plus.svg"),
        Icon::Trash => egui::include_image!("../../assets/icons/trash-2.svg"),
        Icon::Queue => egui::include_image!("../../assets/icons/list-music.svg"),
        Icon::Bell => egui::include_image!("../../assets/icons/bell.svg"),
        Icon::User => egui::include_image!("../../assets/icons/user.svg"),
        Icon::UserPlus => egui::include_image!("../../assets/icons/user-plus.svg"),
        Icon::Users => egui::include_image!("../../assets/icons/users.svg"),
        Icon::ChevronLeft => egui::include_image!("../../assets/icons/chevron-left.svg"),
        Icon::ChevronRight => egui::include_image!("../../assets/icons/chevron-right.svg"),
        Icon::ChevronDown => egui::include_image!("../../assets/icons/chevron-down.svg"),
        Icon::Ellipsis => egui::include_image!("../../assets/icons/ellipsis.svg"),
        Icon::Mic => egui::include_image!("../../assets/icons/mic.svg"),
        Icon::External => egui::include_image!("../../assets/icons/external-link.svg"),
        Icon::Clock => egui::include_image!("../../assets/icons/clock.svg"),
        Icon::Cloud => egui::include_image!("../../assets/icons/cloud.svg"),
        Icon::Music => egui::include_image!("../../assets/icons/music.svg"),
        Icon::ListPlus => egui::include_image!("../../assets/icons/list-plus.svg"),
        Icon::Check => egui::include_image!("../../assets/icons/check.svg"),
        Icon::Pencil => egui::include_image!("../../assets/icons/pencil.svg"),
        Icon::Copy => egui::include_image!("../../assets/icons/copy.svg"),
        Icon::Grip => egui::include_image!("../../assets/icons/grip-vertical.svg"),
        Icon::Loader => egui::include_image!("../../assets/icons/loader-circle.svg"),
        Icon::Home => egui::include_image!("../../assets/icons/house.svg"),
        Icon::Sparkles => egui::include_image!("../../assets/icons/sparkles.svg"),
        Icon::TrendingUp => egui::include_image!("../../assets/icons/trending-up.svg"),
        Icon::Headphones => egui::include_image!("../../assets/icons/headphones.svg"),
        Icon::Info => egui::include_image!("../../assets/icons/info.svg"),
        Icon::Minus => egui::include_image!("../../assets/icons/minus.svg"),
        Icon::Mail => egui::include_image!("../../assets/icons/mail.svg"),
        Icon::CirclePlay => egui::include_image!("../../assets/icons/circle-play.svg"),
        Icon::Refresh => egui::include_image!("../../assets/icons/refresh-cw.svg"),
        Icon::Disc => egui::include_image!("../../assets/icons/disc-3.svg"),
        Icon::BadgeCheck => egui::include_image!("../../assets/icons/badge-check.svg"),
        Icon::ArrowLeft => egui::include_image!("../../assets/icons/arrow-left.svg"),
        Icon::ArrowRight => egui::include_image!("../../assets/icons/arrow-right.svg"),
        Icon::Grid => egui::include_image!("../../assets/icons/layout-grid.svg"),
        Icon::List => egui::include_image!("../../assets/icons/list.svg"),
        Icon::Shrink => egui::include_image!("../../assets/icons/shrink.svg"),
        Icon::AudioLines => egui::include_image!("../../assets/icons/audio-lines.svg"),
    }
}

/// Paint an icon centred in `rect` without allocating space.
pub fn paint(ui: &egui::Ui, icon: Icon, rect: egui::Rect, size: f32, tint: egui::Color32) {
    let icon_rect = egui::Rect::from_center_size(
        rect.center() + play_glyph_offset(icon, size),
        egui::Vec2::splat(size),
    );
    egui::Image::new(source(icon))
        .tint(tint)
        .fit_to_exact_size(egui::Vec2::splat(size))
        .paint_at(ui, icon_rect);
}

/// Horizontal offset that optically centers play triangles
/// (Lucide carries a 1/24-width shift).
pub fn play_glyph_offset(icon: Icon, icon_size: f32) -> egui::Vec2 {
    if matches!(icon, Icon::Play) {
        egui::Vec2::new(icon_size * (0.03 - 1.0 / 24.0), 0.0)
    } else {
        egui::Vec2::ZERO
    }
}

/// A frameless icon control in a fixed cell whose colour lifts on hover.
/// The cell is always `cell` px tall so icons in one row share a centre.
pub fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    size: f32,
    cell: f32,
    color: egui::Color32,
    hover: egui::Color32,
    tooltip: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(cell), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let tint = if response.hovered() || response.has_focus() {
            hover
        } else {
            color
        };
        let scale = if response.is_pointer_button_down_on() {
            0.92
        } else {
            1.0
        };
        paint(ui, icon, rect, size * scale, tint);
    }
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}

/// Round filled button with a centered glyph. Grows + retints on hover.
pub fn circle_button(
    ui: &mut egui::Ui,
    icon: Icon,
    diameter: f32,
    fill: egui::Color32,
    fill_hover: egui::Color32,
    icon_color: egui::Color32,
    tooltip: &str,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::Vec2::splat(diameter), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let grow = if hovered { 1.05 } else { 1.0 };
        let fill = if hovered { fill_hover } else { fill };
        ui.painter()
            .circle_filled(rect.center(), diameter / 2.0 * grow, fill);
        let icon_size = diameter * 0.46;
        paint(ui, icon, rect, icon_size, icon_color);
    }
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}

/// Animated busy indicator, paced independently of the graphics driver.
pub fn spinner(ui: &mut egui::Ui, size: f32, color: egui::Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
        let radius = size / 2.0 - 2.0;
        let start = ui.input(|input| input.time) * std::f64::consts::TAU * 1.2;
        let sweep = 250_f64.to_radians();
        let points = (0..20)
            .map(|index| {
                let angle = start + sweep * f64::from(index) / 19.0;
                let (sin, cos) = angle.sin_cos();
                rect.center() + radius * egui::vec2(cos as f32, sin as f32)
            })
            .collect();
        ui.painter()
            .add(egui::Shape::line(points, egui::Stroke::new(2.0, color)));
    }
    response
}

/// Show a tinted clickable icon. Behaves like a frameless button with a
/// pointing-hand cursor.
pub fn show(ui: &mut egui::Ui, icon: Icon, size: f32, tint: egui::Color32) -> egui::Response {
    ui.add(
        egui::Image::new(source(icon))
            .fit_to_exact_size(egui::vec2(size, size))
            .tint(tint)
            .sense(egui::Sense::click()),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Non-interactive icon (decorative glyph inside a row).
pub fn show_static(
    ui: &mut egui::Ui,
    icon: Icon,
    size: f32,
    tint: egui::Color32,
) -> egui::Response {
    ui.add(
        egui::Image::new(source(icon))
            .fit_to_exact_size(egui::vec2(size, size))
            .tint(tint)
            .sense(egui::Sense::hover()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_resolves() {
        let icons = [
            Icon::Play,
            Icon::Pause,
            Icon::SkipBack,
            Icon::SkipForward,
            Icon::Shuffle,
            Icon::Repeat,
            Icon::Repeat1,
            Icon::Volume,
            Icon::Volume1,
            Icon::Volume2,
            Icon::VolumeX,
            Icon::Heart,
            Icon::Search,
            Icon::Settings,
            Icon::X,
            Icon::Plus,
            Icon::Trash,
            Icon::Queue,
            Icon::Bell,
            Icon::User,
            Icon::UserPlus,
            Icon::Users,
            Icon::ChevronLeft,
            Icon::ChevronRight,
            Icon::ChevronDown,
            Icon::Ellipsis,
            Icon::Mic,
            Icon::External,
            Icon::Clock,
            Icon::Cloud,
            Icon::Music,
            Icon::ListPlus,
            Icon::Check,
            Icon::Pencil,
            Icon::Copy,
            Icon::Grip,
            Icon::Loader,
            Icon::Home,
            Icon::Sparkles,
            Icon::TrendingUp,
            Icon::Headphones,
            Icon::Info,
            Icon::Minus,
            Icon::Mail,
            Icon::CirclePlay,
            Icon::Refresh,
            Icon::Disc,
            Icon::BadgeCheck,
            Icon::ArrowLeft,
            Icon::ArrowRight,
            Icon::Grid,
            Icon::List,
            Icon::Shrink,
            Icon::AudioLines,
        ];
        // include_image! fails to compile on missing files, so reaching
        // here already proves the assets exist; this just pins the count.
        assert_eq!(icons.len(), 54);
        let _ = icons.map(source);
    }
}
