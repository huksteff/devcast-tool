use dc_ui_kit::strings::{kit, notices, settings};
use dc_ui_kit::widgets::panel_frame;
use eframe::egui;
use egui_kittest::kittest::Queryable;

use super::*;
use crate::settings::Store;

struct State {
    shared: Option<Shared>,
    dir: tempfile::TempDir,
}

fn harness() -> egui_kittest::Harness<'static, State> {
    let dir = tempfile::tempdir().unwrap();
    dc_ui_kit::testing::harness()
        .with_size(egui::vec2(376.0, 140.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let shared = state.shared.get_or_insert_with(|| {
                    let store = Store::load(&state.dir.path().join("settings.json")).unwrap();
                    let mut shared = Shared::new(ui.ctx().clone(), store);
                    shared.popover = Some(Open::new(popover::Kind::Crashed, 0.0));
                    shared
                });
                panel_frame().show(ui, |ui| {
                    ui.set_width(336.0);
                    crashed(ui, shared);
                });
            },
            State { shared: None, dir },
        )
}

#[test]
fn crashed_popover() {
    let mut harness = harness();
    harness.run();
    harness.get_by_label(settings::OPEN_LOGS);
    harness.get_by_label(notices::CRASHED);
    harness.snapshot("hud_crashed");
    harness.get_by_label(kit::CLOSE).click();
    harness.run();
    assert!(harness.state().shared.as_ref().unwrap().popover.is_none());
}

#[test]
fn a_new_crash_report_opens_the_popover_once() {
    let dir = tempfile::tempdir().unwrap();
    let crashes = dir.path().join("logs").join("crashes");
    let store = Store::load(&dir.path().join("settings.json")).unwrap();
    let mut shared = Shared::new(egui::Context::default(), store);
    at_start(&mut shared, &crashes).unwrap();
    assert!(shared.popover.is_none(), "no reports");
    crate::crashes::write_panic(&crashes, "паника").unwrap();
    at_start(&mut shared, &crashes).unwrap();
    assert_eq!(shared.popover.map(|o| o.kind), Some(popover::Kind::Crashed));
    shared.popover = None;
    let reread = Store::load(&dir.path().join("settings.json")).unwrap();
    assert!(reread.get().crash_seen > 0, "remembered in settings.json");
    at_start(&mut shared, &crashes).unwrap();
    assert!(shared.popover.is_none(), "told once");
}
