use dc_ui_kit::strings::hotkeys as text;
use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::Queryable;

use super::HotkeyPage;
use crate::hotkeys::Command;
use crate::settings::Hotkeys;

struct State {
    page: HotkeyPage,
    hotkeys: Hotkeys,
}

fn harness(taken: Vec<Command>) -> egui_kittest::Harness<'static, State> {
    dc_ui_kit::testing::harness()
        .with_size(egui::vec2(560.0, 380.0))
        .build_ui_state(
            move |ui, state: &mut State| {
                egui::Frame::new()
                    .fill(dc_ui_kit::tokens::SURFACE)
                    .inner_margin(dc_ui_kit::tokens::S5)
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        state.page.ui(ui, &mut state.hotkeys, &taken);
                    });
            },
            State {
                page: HotkeyPage::default(),
                hotkeys: Hotkeys::default(),
            },
        )
}

const CTRL_SHIFT: Modifiers = Modifiers {
    alt: false,
    ctrl: true,
    shift: true,
    mac_cmd: false,
    command: true,
};

#[test]
fn a_taken_shortcut_is_swapped_on_replace() {
    let mut harness = harness(vec![Command::Pause]);
    harness.run();
    harness
        .get_by_label("Отменить запись: Ctrl+Shift+X")
        .click();
    harness.run();
    harness.key_press_modifiers(CTRL_SHIFT, Key::R);
    harness.run();
    harness.snapshot("settings_hotkeys_conflict");
    harness.get_by_label(text::REPLACE).click();
    harness.run();
    let hotkeys = &harness.state().hotkeys;
    assert_eq!(hotkeys.cancel.as_deref(), Some("Ctrl+Shift+R"));
    assert_eq!(hotkeys.start_stop.as_deref(), Some("Ctrl+Shift+X"));
}

#[test]
fn capture_needs_a_modifier_and_delete_clears() {
    let mut harness = harness(Vec::new());
    harness.run();
    harness
        .get_by_label("Пауза / продолжить: Ctrl+Shift+P")
        .click();
    harness.run();
    harness.key_press_modifiers(Modifiers::SHIFT, Key::K);
    harness.run();
    assert_eq!(harness.state().page.capture, Some(Command::Pause));
    harness.key_press_modifiers(Modifiers::ALT, Key::F5);
    harness.run();
    assert_eq!(harness.state().hotkeys.pause.as_deref(), Some("Alt+F5"));
    harness.get_by_label("Пауза / продолжить: Alt+F5").click();
    harness.run();
    harness.key_press(Key::Delete);
    harness.run();
    assert_eq!(harness.state().hotkeys.pause, None);
    harness.get_by_label(text::RESET_ALL).click();
    harness.run();
    assert_eq!(harness.state().hotkeys, Hotkeys::default());
}
