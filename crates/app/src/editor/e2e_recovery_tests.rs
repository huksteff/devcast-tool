//! Task 7.33, focused scenarios on a fixture recording: the working state a crash left in
//! `session.json` restored or discarded on open, renaming and then exporting. The project is
//! the one `devcast` makes of the same recording (`e2e_tests::Twin`).

use std::time::{Duration, Instant};

use dc_project::dir::SESSION_FILE;
use dc_ui_kit::strings::projects as text;
use eframe::egui::{self, Key};
use egui_kittest::kittest::Queryable;

use super::e2e_tests::{
    App, Recorded, ctrl, editor, editor_mut, exported, opened, recorded, same, text_annotation,
};
use super::editing_tests::json;

/// The app crashes after a text annotation that autosave has written; the app opens the
/// recording again as it does from the recent projects.
fn crashed() -> (Recorded, App, serde_json::Value) {
    let mut recorded = recorded();
    let as_recorded = json(&recorded.twin.project);
    let mut app = opened(recorded.temp.path(), &recorded.root, &[]);
    text_annotation(&mut app.harness, &mut recorded.twin);
    let later = Instant::now() + Duration::from_secs(10);
    let wait = editor_mut(&mut app.harness).doc.autosave(later).unwrap();
    assert!(wait.is_none(), "written");
    assert!(recorded.root.join(SESSION_FILE).is_file());
    drop(app.harness.state_mut().editor.take());
    assert!(app.harness.state_mut().open_project(&recorded.root, true));
    app.harness.run();
    assert!(editor(&app.harness).recovery, "asks what to do");
    (recorded, app, as_recorded)
}

#[test]
fn work_a_crash_left_is_restored_with_its_history() {
    let (mut recorded, mut app, _) = crashed();
    let harness = &mut app.harness;
    harness
        .get_by_role_and_label(egui::Role::Button, text::RESTORE)
        .click();
    harness.run();
    assert!(!editor(harness).recovery);
    assert!(editor(harness).doc.unsaved());
    same(harness, &recorded.twin, "restored");
    ctrl(harness, Key::Z);
    recorded.twin.undo();
    same(harness, &recorded.twin, "undo after the restore");
}

#[test]
fn discarding_the_work_a_crash_left_goes_back_to_the_recording() {
    let (recorded, mut app, as_recorded) = crashed();
    let harness = &mut app.harness;
    harness
        .get_by_role_and_label(egui::Role::Button, text::DISCARD)
        .click();
    harness.run();
    let editor = editor(harness);
    assert!(!editor.recovery && !editor.doc.unsaved());
    assert_eq!(json(editor.doc.project()), as_recorded);
    assert!(!recorded.root.join(SESSION_FILE).exists());
}

/// The header's name renames the folder; the export of the renamed project is the CLI's.
#[test]
fn a_renamed_recording_exports_from_its_new_folder() {
    let Recorded {
        temp,
        root,
        mut twin,
    } = recorded();
    let mut app = opened(temp.path(), &root, &[]);
    let harness = &mut app.harness;
    text_annotation(harness, &mut twin);
    harness.get_by_label("Запись 1").click();
    harness.run();
    harness.event(egui::Event::Text("Урок 2".to_owned()));
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    let renamed = root.with_file_name("Урок 2.devcast");
    assert!(!root.exists());
    assert_eq!(editor(harness).doc.dir().root(), renamed);
    assert_eq!(editor(harness).name, "Урок 2");
    same(harness, &twin, "renamed");
    let recent = &harness.state().store.get().recent_projects;
    assert_eq!(recent.first(), Some(&renamed));
    let spec = exported(harness, &app.taken, renamed.join("урок.mp4"));
    assert_eq!(spec.root, renamed);
    assert_eq!(json(&spec.project), json(&twin.project));
}
