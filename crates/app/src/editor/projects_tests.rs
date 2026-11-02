use dc_ui_kit::strings::{editor as editor_text, projects as text};
use egui_kittest::kittest::Queryable;

use crate::editor::tests::harness;
use crate::editor::{Leave, SIZE};

#[test]
fn the_menu_lists_the_recent_projects_and_marks_the_open_one() {
    let mut harness = harness(SIZE);
    let open = harness.state().editor.doc.dir().root().to_path_buf();
    let other = open.with_file_name("Урок 2.devcast");
    std::fs::create_dir(&other).unwrap();
    let gone = open.with_file_name("Удалённый.devcast");
    harness.state_mut().editor.projects.recent = vec![open, other.clone(), gone];
    harness.run();
    harness.get_by_label(editor_text::PROJECTS).click();
    harness.run();
    harness.snapshot("editor_projects_menu");
    assert!(
        harness.query_by_label("Удалённый").is_none(),
        "a gone folder is left out"
    );
    harness.get_by_label("Урок 2").click();
    harness.run();
    let editor = &harness.state().editor;
    assert_eq!(
        editor.closing,
        crate::editor::closing::Closing::Asking(Leave::Open(other)),
        "the unsaved changes are asked about first"
    );
}

#[test]
fn the_save_question_offers_save_dont_save_and_cancel() {
    let mut harness = harness(SIZE);
    harness.state_mut().editor.leave(Leave::Close);
    harness.run();
    harness.snapshot("editor_save_question");
    harness.get_by_label(text::CANCEL).click();
    harness.run();
    assert!(harness.state().outcome.as_ref().unwrap().leave.is_none());
}

#[test]
fn save_as_copies_the_project_and_opens_the_copy() {
    let mut harness = harness(SIZE);
    super::run(
        &mut harness.state_mut().editor,
        crate::keymap::EditorKey::SaveAs,
    );
    harness.run();
    harness.snapshot("editor_save_as");
    harness.get_by_label(text::SAVE).click();
    harness.step();
    harness.state_mut().editor.projects.wait_copy();
    harness.step();
    let open = harness.state().editor.doc.dir().root().to_path_buf();
    let copy = open.with_file_name("Демо-запись — копия.devcast");
    assert_eq!(
        harness.state().outcome.as_ref().unwrap().leave,
        Some(Leave::Copied(copy.clone()))
    );
    let saved = dc_project::ProjectDir::open(&copy).unwrap().load().unwrap();
    assert_eq!(
        &saved,
        harness.state().editor.doc.project(),
        "the working state is in the copy"
    );
}
