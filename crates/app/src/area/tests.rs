use dc_ui_kit::strings::area as text;
use eframe::egui;
use egui_kittest::kittest::Queryable;

use super::model::{Preset, Rect};
use super::overlay::{Action, Selection, overlay};

struct State {
    selection: Selection,
    action: Option<Action>,
}

fn harness(rect: Option<Rect>) -> egui_kittest::Harness<'static, State> {
    dc_ui_kit::testing::harness()
        .with_size(egui::vec2(960.0, 540.0))
        .build_ui_state(
            |ui, state: &mut State| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(0x33, 0x41, 0x55))
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        if let Some(action) = overlay(ui, None, &mut state.selection) {
                            state.action = Some(action);
                        }
                    });
            },
            State {
                selection: Selection {
                    rect,
                    preset: Preset::Free,
                    drag: None,
                    size: (960, 540),
                },
                action: None,
            },
        )
}

#[test]
fn selection_with_size_and_buttons() {
    let mut harness = harness(Some(Rect {
        x: 200,
        y: 90,
        w: 480,
        h: 270,
    }));
    harness.run();
    harness.snapshot("area_selected");
    harness.get_by_label(text::WIDE).click();
    harness.run();
    let rect = harness.state().selection.rect.unwrap();
    assert_eq!((rect.w, rect.h), (480, 270), "already 16:9");
    harness.get_by_label(text::START).click();
    harness.run();
    assert_eq!(harness.state().action, Some(Action::Start));
}

#[test]
fn small_area_cannot_start_and_esc_cancels() {
    let mut harness = harness(Some(Rect {
        x: 100,
        y: 100,
        w: 120,
        h: 80,
    }));
    harness.run();
    harness.snapshot("area_too_small");
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(harness.state().action, None);
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert_eq!(harness.state().action, Some(Action::Cancel));
}

#[test]
fn nothing_selected_yet() {
    let mut harness = harness(None);
    harness.run();
    harness.snapshot("area_empty");
}
