//! Spike 1.12: an egui panel driven through AccessKit (click by label) and compared to a PNG
//! snapshot rendered with wgpu (kittest prefers the CPU adapter: WARP on Windows).

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

struct State {
    record: bool,
    zoom: f32,
    clicks: u32,
}

#[test]
fn settings_panel_snapshot() {
    let state = State { record: false, zoom: 1.5, clicks: 0 };
    let mut harness = Harness::builder().with_size(egui::vec2(320.0, 180.0)).build_ui_state(
        |ui, state: &mut State| {
            ui.heading("Настройки зума");
            ui.checkbox(&mut state.record, "Следовать за курсором");
            ui.add(egui::Slider::new(&mut state.zoom, 1.0..=5.0).text("Масштаб"));
            if ui.button("Подобрать зумы").clicked() {
                state.clicks += 1;
            }
            ui.label(format!("Нажатий: {}", state.clicks));
        },
        state,
    );
    harness.get_by_label("Следовать за курсором").click();
    harness.get_by_label("Подобрать зумы").click();
    harness.run();
    assert!(harness.state().record, "checkbox must toggle through AccessKit");
    assert_eq!(harness.state().clicks, 1);
    harness.snapshot("settings_panel");
}
