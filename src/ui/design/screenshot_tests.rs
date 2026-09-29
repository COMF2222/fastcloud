use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::widgets::{self, ButtonVariant, Surface, SurfaceTone, ViewState};
use crate::ui::theme::{AIRWAVE_ORANGE, Theme};

const CONTROLS_SIZE: egui::Vec2 = egui::vec2(420.0, 250.0);
const STATES_SIZE: egui::Vec2 = egui::vec2(520.0, 390.0);

#[derive(Clone, Copy)]
enum Scene {
    Controls,
    States,
    Selection,
}

struct SnapshotApp {
    theme: Theme,
    scene: Scene,
    selection_action: Option<widgets::SelectionAction>,
}

impl eframe::App for SnapshotApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, self.theme.tokens.canvas);
        ui.vertical(|ui| match self.scene {
            Scene::Controls => controls(ui, self.theme),
            Scene::States => view_states(ui, self.theme),
            Scene::Selection => {
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        let action = widgets::selection_actions(ui, self.theme, 128, |ui| {
                            ui.label("Additional selection actions");
                        });
                        if action.is_some() {
                            self.selection_action = action;
                        }
                    });
            }
        });
    }
}

fn controls(ui: &mut egui::Ui, theme: Theme) {
    egui::Frame::new()
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            Surface::new(SurfaceTone::Default).show(ui, theme, |ui| {
                ui.set_width(354.0);
                ui.heading("Airwave controls");
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    widgets::action_button(ui, theme, ButtonVariant::Primary, "Play");
                    widgets::action_button(ui, theme, ButtonVariant::Secondary, "Next up");
                });
                ui.add_space(16.0);
                let tabs = [(0_u8, "Overview"), (1, "Likes"), (2, "History")];
                let _ = widgets::segmented_tabs(ui, theme, 1, &tabs);
            });
        });
}

fn view_states(ui: &mut egui::Ui, theme: Theme) {
    egui::Frame::new()
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(470.0);
                widgets::state_panel(ui, theme, ViewState::Loading("Loading collection…"));
                ui.add_space(12.0);
                widgets::state_panel(
                    ui,
                    theme,
                    ViewState::Empty {
                        title: "Nothing here yet",
                        detail: "Play or save a track to start this collection.",
                    },
                );
                ui.add_space(12.0);
                widgets::state_panel(
                    ui,
                    theme,
                    ViewState::Error {
                        title: "That didn't load",
                        detail: "Check the connection and try again.",
                    },
                );
            });
        });
}

fn harness(theme: Theme, scene: Scene, size: egui::Vec2) -> Harness<'static, SnapshotApp> {
    Harness::builder()
        .with_size(size)
        .with_pixels_per_point(1.0)
        .build_eframe(move |creation| {
            crate::fonts::install(&creation.egui_ctx, None);
            theme.apply(&creation.egui_ctx);
            SnapshotApp {
                theme,
                scene,
                selection_action: None,
            }
        })
}

#[test]
fn dark_controls_match_the_golden_image() {
    let mut harness = harness(Theme::dark(AIRWAVE_ORANGE), Scene::Controls, CONTROLS_SIZE);
    harness.run();
    harness.snapshot("airwave/controls_dark");
}

#[test]
fn selection_dark_matches_the_golden_image() {
    let mut harness = harness(
        Theme::dark(AIRWAVE_ORANGE),
        Scene::Selection,
        egui::vec2(960.0, 120.0),
    );
    harness.run();
    harness.snapshot("airwave/selection_dark");
}

#[test]
fn selection_light_matches_the_golden_image() {
    let mut harness = harness(
        Theme::light(AIRWAVE_ORANGE),
        Scene::Selection,
        egui::vec2(960.0, 120.0),
    );
    harness.run();
    harness.snapshot("airwave/selection_light");
}

#[test]
fn selection_compact_matches_the_golden_image() {
    let mut harness = harness(
        Theme::dark(AIRWAVE_ORANGE),
        Scene::Selection,
        egui::vec2(420.0, 220.0),
    );
    harness.run();
    harness.snapshot("airwave/selection_compact");
}

#[test]
fn selection_buttons_emit_the_expected_actions() {
    for (label, expected) in [
        ("Play selected", widgets::SelectionAction::Play),
        ("Add to queue", widgets::SelectionAction::Queue),
        ("Select all visible", widgets::SelectionAction::SelectAll),
        ("Clear selection", widgets::SelectionAction::Clear),
    ] {
        let mut harness = harness(
            Theme::dark(AIRWAVE_ORANGE),
            Scene::Selection,
            egui::vec2(960.0, 120.0),
        );
        harness.run();
        harness.get_by_label(label).click();
        harness.run();
        assert_eq!(harness.state().selection_action, Some(expected), "{label}");
    }
}

#[test]
fn light_controls_match_the_golden_image() {
    let mut harness = harness(Theme::light(AIRWAVE_ORANGE), Scene::Controls, CONTROLS_SIZE);
    harness.run();
    harness.snapshot("airwave/controls_light");
}

#[test]
fn view_states_match_the_golden_image() {
    let mut harness = harness(Theme::dark(AIRWAVE_ORANGE), Scene::States, STATES_SIZE);
    harness.run_steps(2);
    harness.snapshot("airwave/view_states_dark");
}
