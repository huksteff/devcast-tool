use dc_ui_kit::strings::{hotkeys as text, keys as names};
use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::Queryable;

use super::HotkeyPage;
use crate::hotkeys::Command;
use crate::keymap::{Binding, EditorKey, Title};
use crate::settings::{AppSettings, Hotkeys};

struct State {
    page: HotkeyPage,
    settings: AppSettings,
}

fn harness(taken: Vec<Command>) -> egui_kittest::Harness<'static, State> {
    sized(taken, 380.0)
}

fn sized(taken: Vec<Command>, height: f32) -> egui_kittest::Harness<'static, State> {
    dc_ui_kit::testing::harness()
        .with_size(egui::vec2(560.0, height))
        .build_ui_state(
            move |ui, state: &mut State| {
                egui::Frame::new()
                    .fill(dc_ui_kit::tokens::SURFACE)
                    .inner_margin(dc_ui_kit::tokens::S5)
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        state.page.ui(ui, &mut state.settings, &taken);
                    });
            },
            State {
                page: HotkeyPage::default(),
                settings: AppSettings::default(),
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
    let hotkeys = &harness.state().settings.hotkeys;
    assert_eq!(hotkeys.cancel.as_deref(), Some("Ctrl+Shift+R"));
    assert_eq!(hotkeys.start_stop.as_deref(), Some("Ctrl+Shift+X"));
}

#[test]
fn capture_needs_a_modifier_and_backspace_clears() {
    let mut harness = harness(Vec::new());
    harness.run();
    harness
        .get_by_label("Пауза / продолжить: Ctrl+Shift+P")
        .click();
    harness.run();
    harness.key_press_modifiers(Modifiers::SHIFT, Key::K);
    harness.run();
    let pause = Binding::Global(Command::Pause);
    assert_eq!(harness.state().page.capture, Some(pause));
    harness.key_press_modifiers(Modifiers::ALT, Key::F5);
    harness.run();
    let hotkeys = |h: &egui_kittest::Harness<'_, State>| h.state().settings.hotkeys.clone();
    assert_eq!(hotkeys(&harness).pause.as_deref(), Some("Alt+F5"));
    harness.get_by_label("Пауза / продолжить: Alt+F5").click();
    harness.run();
    harness.key_press(Key::Backspace);
    harness.run();
    assert_eq!(hotkeys(&harness).pause, None);
    harness
        .get_all_by_label(text::RESET_ALL)
        .next()
        .unwrap()
        .click();
    harness.run();
    assert_eq!(hotkeys(&harness), Hotkeys::default());
}

/// The physical key of the Russian layout: «И» is not an egui key, the place of B is.
fn press_physical(harness: &mut egui_kittest::Harness<'_, State>, modifiers: Modifiers, key: Key) {
    let event = egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    };
    harness.event_modifiers(event, modifiers);
    harness.run();
}

#[test]
fn an_editor_action_takes_a_bare_key_and_conflicts_across_groups() {
    let mut harness = harness(Vec::new());
    harness.state_mut().page.search = names::BLADE.to_owned();
    harness.run();
    harness.get_by_label("Лезвие: B").click();
    harness.run();
    press_physical(&mut harness, Modifiers::NONE, Key::K);
    let keys = |h: &egui_kittest::Harness<'_, State>| h.state().settings.editor_keys.clone();
    assert_eq!(keys(&harness).keys(EditorKey::Blade).as_deref(), Some("K"));
    // Ctrl+Shift+R is the global start: a conflict, answered «Отмена».
    harness.get_by_label("Лезвие: K").click();
    harness.run();
    press_physical(&mut harness, CTRL_SHIFT, Key::R);
    let conflict = harness.state().page.conflict.clone().unwrap();
    assert_eq!(conflict.owner, Binding::Global(Command::StartStop));
    harness.snapshot("settings_hotkeys_editor_conflict");
    harness.get_by_label(text::CANCEL_CAPTURE).click();
    harness.run();
    assert_eq!(keys(&harness).keys(EditorKey::Blade).as_deref(), Some("K"));
    // The editor group resets only its own shortcuts.
    harness.state_mut().settings.hotkeys.pause = None;
    harness.state_mut().page.search.clear();
    harness.run();
    harness
        .get_all_by_label(text::RESET_ALL)
        .nth(1)
        .unwrap()
        .click();
    harness.run();
    assert_eq!(keys(&harness), crate::keymap::EditorKeys::default());
    assert_eq!(harness.state().settings.hotkeys.pause, None);
}

#[test]
fn the_editor_group_lists_every_action() {
    let mut harness = sized(Vec::new(), 1640.0);
    harness.run();
    harness.snapshot("settings_hotkeys_editor");
    let settings = AppSettings::default();
    for key in EditorKey::ALL {
        let keys = settings.editor_keys.keys(key).unwrap();
        harness.get_by_label(&format!("{}: {keys}", key.title()));
    }
}
