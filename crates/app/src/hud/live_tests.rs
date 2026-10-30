use dc_ui_kit::strings::hud;
use dc_ui_kit::tokens::SURFACE;
use eframe::egui;
use egui_kittest::kittest::Queryable;

use super::super::bar::Clicked;

fn live_harness(view: super::LiveView) -> egui_kittest::Harness<'static, Option<Clicked>> {
    dc_ui_kit::testing::harness()
        .with_size(egui::vec2(380.0, 48.0))
        .build_ui_state(
            move |ui, clicked: &mut Option<Clicked>| {
                let border = if view.paused {
                    dc_ui_kit::tokens::PAUSE
                } else {
                    dc_ui_kit::tokens::BORDER
                };
                ui.painter().rect(
                    ui.max_rect(),
                    dc_ui_kit::tokens::R3,
                    SURFACE,
                    egui::Stroke::new(1.0, border),
                    egui::StrokeKind::Inside,
                );
                let inner = ui.horizontal_centered(|ui| super::live_bar(ui, &view));
                if let Some((c, _)) = inner.inner {
                    *clicked = Some(c);
                }
            },
            None,
        )
}

#[test]
fn recording_bar() {
    let mut harness = live_harness(super::LiveView {
        timer: "12:47".to_owned(),
        paused: false,
        mic: Some(0.6),
    });
    harness.run();
    harness.snapshot("hud_recording");
    for (label, clicked) in [
        (hud::STOP, Clicked::Stop),
        (hud::PAUSE, Clicked::Pause),
        (hud::CANCEL, Clicked::Cancel),
        (hud::HIDE, Clicked::Hide),
    ] {
        harness.get_by_label(label).click();
        harness.run();
        assert_eq!(*harness.state(), Some(clicked));
    }
}

#[test]
fn paused_bar() {
    let mut harness = live_harness(super::LiveView {
        timer: "12:47".to_owned(),
        paused: true,
        mic: None,
    });
    harness.run();
    harness.snapshot("hud_paused");
    harness.get_by_label(hud::RESUME).click();
    harness.run();
    assert_eq!(*harness.state(), Some(Clicked::Pause));
}

#[test]
fn cancel_question() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(280.0, 150.0))
        .build_ui_state(
            |ui, answer: &mut Option<dc_ui_kit::widgets::Answer>| {
                dc_ui_kit::widgets::panel_frame().show(ui, |ui| {
                    let text = hud::cancel_text("12:47");
                    let question = dc_ui_kit::widgets::Confirm {
                        title: hud::CANCEL_TITLE,
                        text: &text,
                        cancel: hud::KEEP_RECORDING,
                        confirm: hud::DELETE_RECORDING,
                        kind: dc_ui_kit::widgets::Kind::Danger,
                    };
                    if let Some(a) = question.show(ui) {
                        *answer = Some(a);
                    }
                });
            },
            None,
        );
    harness.run();
    harness.snapshot("hud_cancel");
    harness.get_by_label(hud::DELETE_RECORDING).click();
    harness.run();
    assert_eq!(*harness.state(), Some(dc_ui_kit::widgets::Answer::Confirm));
}

#[test]
fn preparing_bar() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(super::PREPARING_WIDTH, 48.0))
        .build_ui(|ui| {
            ui.painter().rect_filled(ui.max_rect(), 0, SURFACE);
            ui.horizontal_centered(super::preparing);
        });
    harness.run();
    harness.snapshot("hud_preparing");
}

#[test]
fn countdown_window() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(crate::countdown::SIDE, crate::countdown::SIDE))
        .build_ui_state(
            |ui, cancelled: &mut bool| {
                if crate::countdown::face(ui, 3) {
                    *cancelled = true;
                }
            },
            false,
        );
    harness.run();
    harness.snapshot("countdown");
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(*harness.state());
}
