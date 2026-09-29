//! Reusable egui controls styled exclusively through Airwave semantic tokens.

use eframe::egui;

use super::components;
use crate::ui::theme::{Metrics, Theme, Type};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceTone {
    Default,
    Raised,
    Glass,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    tone: SurfaceTone,
    padding: i8,
    radius: u8,
    border: bool,
}

impl Surface {
    pub const fn new(tone: SurfaceTone) -> Self {
        Self {
            tone,
            padding: Metrics::SP_2 as i8,
            radius: components::PANEL_RADIUS,
            border: true,
        }
    }

    pub const fn raised() -> Self {
        Self::new(SurfaceTone::Raised)
    }

    pub const fn glass() -> Self {
        Self::new(SurfaceTone::Glass)
    }

    pub const fn clear() -> Self {
        Self::new(SurfaceTone::Clear)
    }

    pub const fn padding(mut self, padding: i8) -> Self {
        self.padding = padding;
        self
    }

    pub const fn radius(mut self, radius: u8) -> Self {
        self.radius = radius;
        self
    }

    pub const fn border(mut self, border: bool) -> Self {
        self.border = border;
        self
    }

    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        theme: Theme,
        content: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let fill = match self.tone {
            SurfaceTone::Default => theme.tokens.surface,
            SurfaceTone::Raised => theme.tokens.surface_raised,
            SurfaceTone::Glass => theme.tokens.glass,
            SurfaceTone::Clear => theme.tokens.glass_clear,
        };
        let shadow = match self.tone {
            SurfaceTone::Default => egui::epaint::Shadow {
                offset: [0, 4],
                blur: 14,
                spread: 0,
                color: if theme.dark {
                    egui::Color32::from_black_alpha(72)
                } else {
                    egui::Color32::from_black_alpha(24)
                },
            },
            SurfaceTone::Raised => egui::epaint::Shadow {
                offset: [0, 10],
                blur: 28,
                spread: 1,
                color: if theme.dark {
                    egui::Color32::from_black_alpha(118)
                } else {
                    egui::Color32::from_black_alpha(38)
                },
            },
            SurfaceTone::Glass | SurfaceTone::Clear => egui::epaint::Shadow {
                offset: [0, 12],
                blur: 34,
                spread: 1,
                color: if theme.dark {
                    egui::Color32::from_black_alpha(138)
                } else {
                    egui::Color32::from_black_alpha(34)
                },
            },
        };
        let response = egui::Frame::new()
            .fill(fill)
            .shadow(shadow)
            .stroke(if self.border {
                egui::Stroke::new(
                    1.0,
                    if matches!(self.tone, SurfaceTone::Glass | SurfaceTone::Clear) {
                        theme.tokens.glass_border
                    } else {
                        theme.tokens.border_subtle
                    },
                )
            } else {
                egui::Stroke::NONE
            })
            .corner_radius(self.radius)
            .inner_margin(egui::Margin::same(self.padding))
            .show(ui, content);
        if matches!(self.tone, SurfaceTone::Glass | SurfaceTone::Clear)
            && ui.is_rect_visible(response.response.rect)
        {
            paint_glass_highlight(ui, response.response.rect, self.radius, theme);
        }
        response
    }
}

/// Paint a glass sheet for hand-laid components. The material is deliberately
/// made from alpha, edge light and shadow instead of a blur pass, which keeps
/// it cheap enough for egui while preserving the same depth hierarchy.
pub fn paint_glass_rect(
    ui: &egui::Ui,
    rect: egui::Rect,
    radius: impl Into<egui::CornerRadius> + Copy,
    theme: Theme,
    raised: bool,
    accent: Option<egui::Color32>,
) {
    paint_glass_rect_with_fill(ui, rect, radius, theme, raised, accent, None);
}

pub fn paint_clear_glass_rect(
    ui: &egui::Ui,
    rect: egui::Rect,
    radius: impl Into<egui::CornerRadius> + Copy,
    theme: Theme,
    accent: Option<egui::Color32>,
) {
    paint_glass_rect_with_fill(
        ui,
        rect,
        radius,
        theme,
        false,
        accent,
        Some(theme.tokens.glass_clear),
    );
}

fn paint_glass_rect_with_fill(
    ui: &egui::Ui,
    rect: egui::Rect,
    radius: impl Into<egui::CornerRadius> + Copy,
    theme: Theme,
    raised: bool,
    accent: Option<egui::Color32>,
    fill: Option<egui::Color32>,
) {
    let radius = radius.into();
    ui.painter().add(
        egui::epaint::Shadow {
            offset: [0, if raised { 10 } else { 5 }],
            blur: if raised { 30 } else { 18 },
            spread: 0,
            color: egui::Color32::from_black_alpha(if theme.dark { 118 } else { 30 }),
        }
        .as_shape(rect, radius),
    );
    ui.painter().rect_filled(
        rect,
        radius,
        fill.unwrap_or(if raised {
            theme.tokens.glass_strong
        } else {
            theme.tokens.glass
        }),
    );
    if let Some(accent) = accent {
        let glow = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 18.0, rect.center().y),
            egui::vec2(16.0, (rect.height() - 12.0).max(8.0)),
        );
        ui.painter().add(
            egui::epaint::Shadow {
                offset: [0, 0],
                blur: 20,
                spread: 3,
                color: accent.gamma_multiply(0.28),
            }
            .as_shape(glow, 8.0),
        );
    }
    ui.painter().rect_stroke(
        rect.shrink(0.5),
        radius,
        egui::Stroke::new(1.0, theme.tokens.glass_border),
        egui::StrokeKind::Inside,
    );
    paint_glass_highlight(ui, rect, radius, theme);
}

fn paint_glass_highlight(
    ui: &egui::Ui,
    rect: egui::Rect,
    radius: impl Into<egui::CornerRadius>,
    theme: Theme,
) {
    let radius = radius.into();
    let inset = radius.nw as f32 + 7.0;
    if rect.width() > inset * 2.0 {
        ui.painter().line_segment(
            [
                egui::pos2(rect.left() + inset, rect.top() + 1.0),
                egui::pos2(rect.right() - inset, rect.top() + 1.0),
            ],
            egui::Stroke::new(1.0, theme.tokens.glass_highlight),
        );
    }
}

/// Paint the quiet Airwave atmosphere behind a page. It deliberately avoids
/// artwork or donor-specific decoration: the large glow and cropped rings are
/// FastCloud's own spatial motif and make otherwise dark pages feel layered.
pub fn canvas_backdrop(ui: &egui::Ui, theme: Theme) {
    let rect = ui.max_rect();
    let painter = ui.painter();

    let glow_center = egui::pos2(rect.right() - rect.width() * 0.18, rect.top() + 30.0);
    let glow_source = egui::Rect::from_center_size(glow_center, egui::vec2(72.0, 72.0));
    painter.add(
        egui::epaint::Shadow {
            offset: [0, 0],
            blur: 190,
            spread: 52,
            color: theme
                .accent
                .gamma_multiply(if theme.dark { 0.16 } else { 0.10 }),
        }
        .as_shape(glow_source, 36.0),
    );

    // A cool counter-light keeps the canvas from reading as one flat black
    // sheet and separates neutral glass from the warm Airwave accent.
    let counter_center = egui::pos2(
        rect.left() + rect.width() * 0.18,
        rect.bottom() - rect.height() * 0.08,
    );
    let counter_source = egui::Rect::from_center_size(counter_center, egui::vec2(52.0, 52.0));
    painter.add(
        egui::epaint::Shadow {
            offset: [0, 0],
            blur: 220,
            spread: 48,
            color: if theme.dark {
                egui::Color32::from_rgba_unmultiplied(64, 92, 180, 24)
            } else {
                egui::Color32::from_rgba_unmultiplied(82, 118, 210, 18)
            },
        }
        .as_shape(counter_source, 26.0),
    );

    let ring = theme
        .accent
        .gamma_multiply(if theme.dark { 0.13 } else { 0.10 });
    for (radius, width) in [(184.0, 1.0), (246.0, 0.7), (330.0, 0.5)] {
        painter.circle_stroke(glow_center, radius, egui::Stroke::new(width, ring));
    }

    let diagonal = theme.tokens.glass_border.gamma_multiply(0.55);
    painter.line_segment(
        [
            egui::pos2(rect.left() + rect.width() * 0.06, rect.bottom()),
            egui::pos2(rect.left() + rect.width() * 0.58, rect.top()),
        ],
        egui::Stroke::new(0.7, diagonal),
    );

    painter.hline(
        rect.x_range(),
        rect.top() + 0.5,
        egui::Stroke::new(1.0, theme.tokens.border_subtle),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionAction {
    Play,
    Queue,
    SelectAll,
    Clear,
}

pub fn selection_actions(
    ui: &mut egui::Ui,
    theme: Theme,
    count: usize,
    more: impl FnOnce(&mut egui::Ui),
) -> Option<SelectionAction> {
    let mut action = None;
    Surface::raised().show(ui, theme, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(Type::H4.rich(&format!("{count} selected (visible)"), theme.text));
            if action_button(ui, theme, ButtonVariant::Primary, "Play selected").clicked() {
                action = Some(SelectionAction::Play);
            }
            if action_button(ui, theme, ButtonVariant::Secondary, "Add to queue").clicked() {
                action = Some(SelectionAction::Queue);
            }
            ui.menu_button("More actions", more);
            if ui.button("Select all visible").clicked() {
                action = Some(SelectionAction::SelectAll);
            }
            if ui.button("Clear selection").clicked() {
                action = Some(SelectionAction::Clear);
            }
        });
    });
    action
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ButtonVisual {
    fill: egui::Color32,
    text: egui::Color32,
    border: egui::Color32,
}

fn button_visual(theme: Theme, variant: ButtonVariant) -> ButtonVisual {
    match variant {
        ButtonVariant::Primary => ButtonVisual {
            fill: theme.tokens.accent,
            text: theme.tokens.on_accent,
            border: egui::Color32::TRANSPARENT,
        },
        ButtonVariant::Secondary => ButtonVisual {
            fill: theme.tokens.glass,
            text: theme.tokens.text_primary,
            border: theme.tokens.glass_border,
        },
    }
}

pub fn action_button(
    ui: &mut egui::Ui,
    theme: Theme,
    variant: ButtonVariant,
    label: &str,
) -> egui::Response {
    let visual = button_visual(theme, variant);
    ui.add(
        egui::Button::new(Type::H4.rich(label, visual.text))
            .fill(visual.fill)
            .stroke(egui::Stroke::new(1.0, visual.border))
            .corner_radius(components::CONTROL_RADIUS)
            .min_size(egui::vec2(0.0, components::CONTROL_HEIGHT)),
    )
}

pub fn paint_focus_ring(
    ui: &egui::Ui,
    response: &egui::Response,
    color: egui::Color32,
    radius: impl Into<egui::CornerRadius>,
) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            radius,
            egui::Stroke::new(2.0, color),
            egui::StrokeKind::Inside,
        );
    }
}

pub fn segmented_tabs<T: Copy + Eq>(
    ui: &mut egui::Ui,
    theme: Theme,
    selected: T,
    tabs: &[(T, &str)],
) -> Option<T> {
    let mut changed = None;
    Surface::new(SurfaceTone::Default)
        .padding(Metrics::SP_HALF as i8)
        .radius(components::CONTROL_RADIUS)
        .border(false)
        .show(ui, theme, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(Metrics::SP_HALF, Metrics::SP_HALF);
                for &(value, label) in tabs {
                    let active = value == selected;
                    let response = ui.add(
                        egui::Button::new(Type::H4.rich(
                            label,
                            if active {
                                theme.tokens.text_primary
                            } else {
                                theme.tokens.text_secondary
                            },
                        ))
                        .fill(if active {
                            theme.tokens.accent_soft
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(components::CONTROL_RADIUS)
                        .min_size(egui::vec2(0.0, components::CONTROL_HEIGHT)),
                    );
                    if response.clicked() && !active {
                        changed = Some(value);
                    }
                }
            });
        });
    changed
}

pub enum ViewState<'a> {
    Loading(&'a str),
    Empty { title: &'a str, detail: &'a str },
    Error { title: &'a str, detail: &'a str },
}

pub fn state_panel(ui: &mut egui::Ui, theme: Theme, state: ViewState<'_>) {
    Surface::new(SurfaceTone::Default)
        .padding(Metrics::SP_2 as i8)
        .show(ui, theme, |ui| match state {
            ViewState::Loading(label) => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = Metrics::SP_1;
                    crate::ui::icons::spinner(ui, 16.0, theme.tokens.text_secondary);
                    ui.label(Type::BODY.rich(label, theme.tokens.text_secondary));
                });
                skeleton_rows(ui, theme, 2);
            }
            ViewState::Empty { title, detail } => {
                ui.label(Type::H4.rich(title, theme.tokens.text_primary));
                if !detail.is_empty() {
                    ui.label(Type::CAPTION.rich(detail, theme.tokens.text_secondary));
                }
            }
            ViewState::Error { title, detail } => {
                ui.label(Type::H4.rich(title, theme.tokens.danger));
                if !detail.is_empty() {
                    ui.label(Type::CAPTION.rich(detail, theme.tokens.text_secondary));
                }
            }
        });
}

pub fn skeleton_rows(ui: &mut egui::Ui, theme: Theme, rows: usize) {
    ui.add_space(Metrics::SP_1);
    for index in 0..rows {
        let width = ui.available_width() * if index % 2 == 0 { 0.86 } else { 0.62 };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 8.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, components::PILL_RADIUS, theme.tokens.surface_hover);
        ui.add_space(Metrics::SP_HALF);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_button_uses_accent_roles() {
        let theme = Theme::default();

        let visual = button_visual(theme, ButtonVariant::Primary);

        assert_eq!(visual.fill, theme.tokens.accent);
    }

    #[test]
    fn secondary_button_uses_glass_roles() {
        let theme = Theme::default();

        let visual = button_visual(theme, ButtonVariant::Secondary);

        assert_eq!(visual.fill, theme.tokens.glass);
        assert_eq!(visual.border, theme.tokens.glass_border);
    }

    #[test]
    fn shared_controls_use_the_component_height() {
        assert_eq!(components::CONTROL_HEIGHT, 36.0);
    }
}
