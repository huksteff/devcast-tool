use std::time::Instant;

use dc_ops::Step;
use eframe::egui;

use dc_ui_kit::strings::editor as text;
use egui_kittest::kittest::Queryable;

use super::frame;
use crate::app::Shared;
use crate::editor::closing::Closing;
use crate::settings::Store;

/// A pass of the editor window in which the user closes it.
fn close(ctx: &egui::Context, shared: &mut Shared) -> egui::FullOutput {
    let mut input = egui::RawInput::default();
    input.viewports.insert(
        egui::ViewportId::ROOT,
        egui::ViewportInfo {
            events: vec![egui::ViewportEvent::Close],
            ..Default::default()
        },
    );
    let mut output = ctx.run_ui(input, |ui| frame(ui, shared));
    output.textures_delta.clear();
    output
}

/// Shared with an editor whose edits cannot be written (`session.json` is a folder), and that
/// `session.json`.
fn unwritable() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Shared,
    std::path::PathBuf,
) {
    let (project, mut editor) = crate::editor::tests::editor();
    let step = Step::new(
        "keys",
        serde_json::Map::from_iter([("show".to_owned(), true.into())]),
    );
    editor.doc.apply(&[step], Instant::now()).unwrap();
    let session = editor.doc.dir().file(dc_project::dir::SESSION_FILE);
    if session.is_file() {
        std::fs::remove_file(&session).unwrap();
    }
    std::fs::create_dir(&session).unwrap();
    let settings = tempfile::tempdir().unwrap();
    let store = Store::load(&settings.path().join("settings.json")).unwrap();
    let mut shared = Shared::new(egui::Context::default(), store);
    shared.editor = Some(editor);
    (project, settings, shared, session)
}

#[test]
fn a_failed_write_on_close_keeps_the_editor_open() {
    let (_project, _settings, mut shared, session) = unwritable();
    let ctx = shared.ctx.clone();
    let output = close(&ctx, &mut shared);
    assert!(shared.editor.is_some(), "the edits are not on disk");
    let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
    assert!(commands.contains(&egui::ViewportCommand::CancelClose));

    std::fs::remove_dir(&session).unwrap();
    close(&ctx, &mut shared);
    assert!(shared.editor.is_none());
    assert!(session.is_file(), "the second close writes the session");
}

#[test]
fn a_second_failed_close_asks_and_closes_without_the_edits_on_yes() {
    let (_project, _settings, mut shared, session) = unwritable();
    let ctx = shared.ctx.clone();
    close(&ctx, &mut shared);
    assert!(!shared.toasts.is_empty(), "the first close shows the error");
    shared.toasts = Default::default();
    let output = close(&ctx, &mut shared);
    let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
    assert!(commands.contains(&egui::ViewportCommand::CancelClose));
    assert!(shared.toasts.is_empty(), "the second close asks instead");
    assert_eq!(shared.editor.as_ref().unwrap().closing, Closing::Asking);

    let mut harness = dc_ui_kit::testing::harness()
        .with_size(crate::editor::SIZE)
        .build_ui_state(|ui, shared: &mut Shared| frame(ui, shared), shared);
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Button, text::CLOSE_UNSAVED_CANCEL)
        .click();
    harness.run();
    let editor = harness.state().editor.as_ref().unwrap();
    assert_eq!(
        editor.closing,
        Closing::Failed,
        "«Остаться» keeps the window"
    );
    harness.state_mut().editor.as_mut().unwrap().closing = Closing::Asking;
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Button, text::CLOSE_UNSAVED_CONFIRM)
        .click();
    harness.run();
    assert!(harness.state().editor.is_none(), "closed without the edits");
    assert!(session.is_dir(), "nothing was written");
    std::fs::remove_dir(&session).unwrap();
}
