use std::path::PathBuf;

use dc_ui_kit::strings::{hud, kit};
use dc_ui_kit::widgets::panel_frame;
use eframe::egui;
use egui_kittest::kittest::Queryable;

use super::*;
use crate::settings::Store;

#[test]
fn the_buttons_of_a_failed_recording_fit_in_the_panel() {
    let dir = tempfile::tempdir().unwrap();
    let padding = Kind::Failed.padding();
    let inner = FAILED_WIDTH - padding.sum().x - 2.0;
    let mut shared = None;
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(FAILED_WIDTH + 40.0, 220.0))
        .build_ui(move |ui| {
            let shared = shared.get_or_insert_with(|| {
                let store = Store::load(&dir.path().join("settings.json")).unwrap();
                let mut shared = Shared::new(ui.ctx().clone(), store);
                let message = "не удалось создать «manifest.json.tmp»: файл занят".to_owned();
                shared.failure = Some((message, PathBuf::from("recording-5.devcast")));
                shared
            });
            panel_frame().inner_margin(padding).show(ui, |ui| {
                ui.set_width(inner);
                failed(ui, shared);
            });
        });
    harness.run();
    let left = harness.get_by_label(hud::PREPARE_FAILED).rect().left();
    let first = harness.get_by_label(hud::RECOVER).rect();
    let last = harness.get_by_label(kit::CLOSE).rect();
    assert!(first.left() >= left, "{first:?} starts before {left}");
    assert!(last.right() <= left + inner + 0.5, "{last:?}");
}
