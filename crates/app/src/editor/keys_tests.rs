//! Editor shortcuts from the table: physical keys, reassigning, the help, tooltips.

use dc_ui_kit::strings::{editor as text, keys as names};
use egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;

use super::tests::{State, harness};
use super::view::Tool;
use super::*;
use crate::keymap::{EditorKey, EditorKeys};

/// A press of the key at the place of `physical` that the layout calls `key`.
fn press(
    harness: &mut egui_kittest::Harness<'_, State>,
    modifiers: Modifiers,
    key: Key,
    physical: Key,
) {
    let event = egui::Event::Key {
        key,
        physical_key: Some(physical),
        pressed: true,
        repeat: false,
        modifiers,
    };
    harness.event_modifiers(event, modifiers);
    harness.run();
}

fn tool(harness: &egui_kittest::Harness<'_, State>) -> Tool {
    harness.state().editor.view.tool
}

#[test]
fn tools_and_snap_go_by_the_physical_key() {
    let mut harness = harness(SIZE);
    harness.run();
    // Russian «И» has no egui key: egui-winit sends the physical B.
    press(&mut harness, Modifiers::NONE, Key::B, Key::B);
    assert_eq!(tool(&harness), Tool::Blade);
    // Dvorak writes X on the place of A: the place decides.
    press(&mut harness, Modifiers::NONE, Key::X, Key::A);
    assert_eq!(tool(&harness), Tool::Select);
    assert!(harness.state().editor.view.magnet);
    press(&mut harness, Modifiers::NONE, Key::N, Key::N);
    assert!(!harness.state().editor.view.magnet);
    // Ctrl+B is not B.
    press(&mut harness, Modifiers::COMMAND, Key::B, Key::B);
    assert_eq!(tool(&harness), Tool::Select);
}

#[test]
fn a_reassigned_shortcut_replaces_the_default() {
    let mut harness = harness(SIZE);
    let mut keys = EditorKeys::default();
    keys.set(EditorKey::Blade, Some("Ctrl+B".to_owned()));
    // The settings would have asked to take it from «Вырезать участок».
    keys.set(EditorKey::CutRange, None);
    keys.set(EditorKey::Undo, Some("Shift+Delete".to_owned()));
    harness.state_mut().keys = keys;
    harness.run();
    press(&mut harness, Modifiers::NONE, Key::B, Key::B);
    assert_eq!(tool(&harness), Tool::Select, "B is free now");
    press(&mut harness, Modifiers::COMMAND, Key::B, Key::B);
    assert_eq!(tool(&harness), Tool::Blade);

    let original = serde_json::to_value(harness.state().editor.doc.project()).unwrap();
    harness
        .get_by_label(dc_ui_kit::strings::timeline::HIDDEN)
        .click();
    harness.run();
    press(&mut harness, Modifiers::COMMAND, Key::Z, Key::Z);
    assert!(
        harness.state().editor.doc.next(Way::Undo).is_some(),
        "Ctrl+Z is free now"
    );
    // egui-winit gives Shift+Delete as a clipboard «cut» without a key event.
    // Shift is held as in a real press.
    harness.event(egui::Event::ModifiersChanged(Modifiers::SHIFT));
    harness.run();
    harness.event(egui::Event::Cut);
    harness.run();
    harness.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    harness.run();
    let undone = serde_json::to_value(harness.state().editor.doc.project()).unwrap();
    assert_eq!(undone, original);
}

#[test]
fn tooltips_name_the_shortcut_of_the_table() {
    let mut keys = EditorKeys::default();
    assert_eq!(
        keys::hint(text::SELECT, EditorKey::Select, &keys),
        "Выбор · A"
    );
    assert_eq!(
        keys::hint(&text::undo_step("зум 1,5×"), EditorKey::Undo, &keys),
        "Отменить: зум 1,5× · Ctrl+Z"
    );
    assert_eq!(
        keys::hint(text::PLAY, EditorKey::PlayPause, &keys),
        "Воспроизвести · Space"
    );
    assert_eq!(
        keys::hint(text::PREV_FRAME, EditorKey::PrevFrame, &keys),
        "Кадр назад · ←"
    );
    keys.set(EditorKey::Select, None);
    keys.set(EditorKey::Redo, Some("Ctrl+Shift+Y".to_owned()));
    assert_eq!(keys::hint(text::SELECT, EditorKey::Select, &keys), "Выбор");
    assert_eq!(
        keys::hint(&text::redo_step("шаг"), EditorKey::Redo, &keys),
        "Повторить: шаг · Ctrl+Shift+Y"
    );
}

#[test]
fn f1_opens_the_help_with_every_action() {
    let mut harness = harness(SIZE);
    harness.run();
    harness.key_press(Key::F1);
    harness.run();
    assert!(harness.state().editor.help.is_some());
    harness.snapshot("editor_help");
    let keys = EditorKeys::default();
    for key in EditorKey::ALL {
        let label = keys.label(key).unwrap();
        harness.get_by_label(&format!("{}: {label}", key.title()));
    }
    // While the help is open other shortcuts wait.
    press(&mut harness, Modifiers::NONE, Key::B, Key::B);
    assert_eq!(tool(&harness), Tool::Select);
    harness.key_press(Key::F1);
    harness.run();
    assert!(harness.state().editor.help.is_none());
}

#[test]
fn the_help_searches_and_leads_to_the_settings() {
    let mut harness = harness(SIZE);
    harness.run();
    harness.get_by_label(text::HELP).click();
    harness.run();
    harness.get_by_label(names::SEARCH).click();
    harness.run();
    harness.get_by_label(names::SEARCH).type_text("ctrl+z");
    harness.run();
    // Typing into the search is no shortcut.
    press(&mut harness, Modifiers::NONE, Key::B, Key::B);
    assert_eq!(tool(&harness), Tool::Select);
    harness.get_by_label("Отменить: Ctrl+Z");
    assert!(harness.query_by_label("Выбор: A").is_none());
    harness.snapshot("editor_help_search");
    harness.get_by_label(names::EDIT_IN_SETTINGS).click();
    harness.run();
    assert!(harness.state().editor.help.is_none());
    assert_eq!(
        harness.state().settings,
        Some(crate::settings_window::Page::Hotkeys)
    );
}

#[test]
fn the_help_shows_the_users_shortcuts() {
    let mut keys = EditorKeys::default();
    keys.set(EditorKey::Blade, Some("Ctrl+Alt+B".to_owned()));
    keys.set(EditorKey::AddZoom, None);
    let mut harness = harness(SIZE);
    harness.state_mut().keys = keys;
    harness.state_mut().editor.toggle_help();
    harness.run();
    harness.get_by_label("Лезвие: Ctrl+Alt+B");
    harness.get_by_label("Добавить зум: Нет");
}

#[test]
fn the_help_fits_the_least_window() {
    let mut harness = harness(MIN_SIZE);
    harness.state_mut().editor.toggle_help();
    harness.run();
    harness.snapshot("editor_help_min");
}
