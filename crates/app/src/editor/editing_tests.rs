//! Editing through the window: a control runs a registry command, the result equals what
//! `devcast` gives, Ctrl+Z / Ctrl+Y walk the history by the physical key, autosave writes the
//! session and its history.

use std::time::{Duration, Instant};

use dc_core::{History, Project};
use dc_fx_keys::Keys;
use dc_ops::{Registry, Step};
use dc_project::Working;
use dc_ui_kit::strings::{editor as text, timeline as names};
use egui::{Key, Modifiers};
use egui_kittest::kittest::{NodeT as _, Queryable};

use super::tests::{State, editor, harness, project};
use super::*;

/// The project `devcast <line>` makes of the test project: the CLI's parsing and apply.
pub(crate) fn by_cli(line: &str) -> Project {
    let registry = Registry::devcast().unwrap();
    let words: Vec<String> = line.split(' ').map(str::to_owned).collect();
    let (info, taken) = registry.resolve(&words).unwrap();
    let args = dc_ops::parse_args(info, &words[taken..]).unwrap();
    let mut project = project();
    let steps = [Step::new(info.name, args)];
    dc_ops::apply(
        &registry,
        &mut project,
        &mut History::default(),
        &steps,
        false,
    )
    .unwrap();
    project
}

pub(crate) fn json(project: &Project) -> serde_json::Value {
    serde_json::to_value(project).unwrap()
}

fn keys_shown(harness: &egui_kittest::Harness<'_, State>) -> bool {
    let project = harness.state().editor.doc.project();
    project.state::<Keys>().unwrap().settings.show
}

fn press(harness: &mut egui_kittest::Harness<'_, State>, key: Key, physical: Key, shift: bool) {
    let modifiers = Modifiers {
        shift,
        ..Modifiers::COMMAND
    };
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

#[test]
fn the_keys_eye_gives_the_project_of_the_cli() {
    let mut harness = harness(SIZE);
    harness.run();
    assert!(!keys_shown(&harness));
    harness.get_by_label(names::HIDDEN).click();
    harness.run();
    assert!(keys_shown(&harness));
    let editor = &harness.state().editor;
    assert_eq!(
        json(editor.doc.project()),
        json(&by_cli("keys --show true"))
    );
    let row = editor
        .tracks
        .rows
        .iter()
        .find(|t| t.row == timeline::Row::Keys);
    assert!(!row.unwrap().off, "the row shows the new state");
    assert!(
        !harness
            .get_by_label(text::UNDO)
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .get_by_label(text::REDO)
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn the_captions_eye_hides_them_as_the_cli_does() {
    let mut harness = harness(SIZE);
    harness.run();
    harness.get_by_label(names::SHOWN).click();
    harness.run();
    let project = harness.state().editor.doc.project();
    assert_eq!(json(project), json(&by_cli("captions style --show false")));
}

#[test]
fn undo_and_redo_by_buttons_and_physical_keys() {
    let mut harness = harness(SIZE);
    harness.run();
    let original = json(&project());
    harness.get_by_label(names::HIDDEN).click();
    harness.run();
    let shown = json(harness.state().editor.doc.project());
    let state = |h: &egui_kittest::Harness<'_, State>| json(h.state().editor.doc.project());

    harness.get_by_label(text::UNDO).click();
    harness.run();
    assert_eq!(state(&harness), original, "undo button");
    harness.get_by_label(text::REDO).click();
    harness.run();
    assert_eq!(state(&harness), shown, "redo button");

    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(state(&harness), original, "Ctrl+Z without a physical key");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Y);
    harness.run();
    assert_eq!(state(&harness), shown, "Ctrl+Y");
    // Russian layout: «Я» has no egui key, egui-winit sends the physical one.
    press(&mut harness, Key::Z, Key::Z, false);
    assert_eq!(state(&harness), original, "Ctrl+Я");
    press(&mut harness, Key::Z, Key::Z, true);
    assert_eq!(state(&harness), shown, "Ctrl+Shift+Я");
    // A layout whose letter on the Z key is another one (Dvorak «;»): the place decides.
    press(&mut harness, Key::Semicolon, Key::Z, false);
    assert_eq!(state(&harness), original, "the key in the place of Z");
    press(&mut harness, Key::Z, Key::Y, false);
    assert_eq!(state(&harness), shown, "the key in the place of Y");
    assert!(keys_shown(&harness));
}

#[test]
fn nothing_to_undo_disables_the_buttons() {
    let mut harness = harness(SIZE);
    harness.run();
    for label in [text::UNDO, text::REDO] {
        assert!(harness.get_by_label(label).accesskit_node().is_disabled());
    }
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(json(harness.state().editor.doc.project()), json(&project()));
}

#[test]
fn autosave_writes_the_session_and_its_history() {
    let (temp, mut editor) = editor();
    let start = Instant::now();
    let step = |show: bool| {
        Step::new(
            "keys",
            serde_json::Map::from_iter([("show".to_owned(), show.into())]),
        )
    };
    editor.doc.apply(&[step(true)], start).unwrap();
    editor.doc.apply(&[step(false)], start).unwrap();
    editor.doc.travel(Way::Undo, start);
    let root = editor.doc.dir().root().to_path_buf();
    let session = root.join("session.json");
    std::fs::remove_file(&session).unwrap();
    let wait = editor.doc.autosave(start).unwrap();
    assert!(wait.is_some() && !session.exists(), "waits for a pause");
    let wait = editor.doc.autosave(start + Duration::from_secs(3)).unwrap();
    assert!(wait.is_none() && session.exists());
    assert!(editor.doc.unsaved());
    let edited = json(editor.doc.project());
    drop(editor);
    let dir = ProjectDir::open(&root).unwrap();
    let working = Working::load(&dir, false).unwrap();
    assert!(
        working.unsaved && working.warnings.is_empty(),
        "{:?}",
        working.warnings
    );
    assert_eq!(json(&working.project), edited);
    assert_eq!(working.history.changes().len(), 1);
    assert_eq!(working.history.redo_changes().len(), 1);
    drop(temp);
}

#[test]
fn closing_writes_what_autosave_has_not() {
    let (_temp, mut editor) = editor();
    let step = Step::new(
        "keys",
        serde_json::Map::from_iter([("show".to_owned(), true.into())]),
    );
    editor.doc.apply(&[step], Instant::now()).unwrap();
    editor.doc.flush().unwrap();
    let dir = editor.doc.dir().clone();
    let edited = json(editor.doc.project());
    drop(editor);
    let working = Working::load(&dir, false).unwrap();
    assert_eq!(json(&working.project), edited);
    assert_eq!(working.history.changes().len(), 1);
}

#[test]
fn a_second_writer_is_refused() {
    let (_temp, editor) = editor();
    let error = Document::open(editor.doc.dir().clone()).unwrap_err();
    assert!(matches!(
        error,
        DocumentError::Project(dc_project::ProjectError::Locked { .. })
    ));
}
