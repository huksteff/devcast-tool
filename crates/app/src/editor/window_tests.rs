use std::time::Instant;

use dc_ops::Step;
use eframe::egui;

use super::frame;
use crate::app::Shared;
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

#[test]
fn a_failed_write_on_close_keeps_the_editor_open() {
    let (_project, mut editor) = crate::editor::tests::editor();
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
    let ctx = egui::Context::default();
    let mut shared = Shared::new(ctx.clone(), store);
    shared.editor = Some(editor);

    let output = close(&ctx, &mut shared);
    assert!(shared.editor.is_some(), "the edits are not on disk");
    let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
    assert!(commands.contains(&egui::ViewportCommand::CancelClose));

    std::fs::remove_dir(&session).unwrap();
    close(&ctx, &mut shared);
    assert!(shared.editor.is_none());
    assert!(session.is_file(), "the second close writes the session");
}
