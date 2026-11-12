use std::time::Instant;

use dc_ops::Step;
use dc_project::dir::{PROJECT_FILE, SESSION_FILE};
use eframe::egui;

use dc_ui_kit::strings::projects as text;
use egui_kittest::kittest::Queryable;

use super::frame;
use crate::app::Shared;
use crate::editor::Leave;
use crate::editor::closing::Closing;
use crate::settings::Store;

/// A pass of the editor window in which the user closes it.
pub(in crate::editor) fn close(ctx: &egui::Context, shared: &mut Shared) -> egui::FullOutput {
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

/// Shared with the test editor after one more edit (`keys` shown), its folders and its root.
fn edited() -> (
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
    let root = editor.doc.dir().root().to_path_buf();
    let settings = tempfile::tempdir().unwrap();
    let store = Store::load(&settings.path().join("settings.json")).unwrap();
    let mut shared = Shared::new(egui::Context::default(), store);
    shared.editor = Some(editor);
    (project, settings, shared, root)
}

/// The saved project of `root` is the working state of the editor after the test edit.
fn keys_saved(root: &std::path::Path) -> bool {
    let saved = dc_project::ProjectDir::open(root).unwrap().load().unwrap();
    let keys = saved.state::<dc_fx_keys::Keys>().unwrap();
    keys.settings.show
}

/// The temporary folders of a test: the project and the settings.
type Folders = (tempfile::TempDir, tempfile::TempDir);

/// The window closed with unsaved changes asks; the question answered with `button`.
fn answer(
    button: &str,
) -> (
    egui_kittest::Harness<'static, Shared>,
    std::path::PathBuf,
    Folders,
) {
    let (project, settings, mut shared, root) = edited();
    let ctx = shared.ctx.clone();
    let output = close(&ctx, &mut shared);
    let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
    assert!(commands.contains(&egui::ViewportCommand::CancelClose));
    assert_eq!(
        shared.editor.as_ref().unwrap().closing,
        Closing::Asking(Leave::Close)
    );
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(crate::editor::SIZE)
        .build_ui_state(|ui, shared: &mut Shared| frame(ui, shared), shared);
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Button, button)
        .click();
    harness.run();
    (harness, root, (project, settings))
}

#[test]
fn closing_without_changes_does_not_ask() {
    let (_project, _settings, mut shared, _root) = edited();
    shared.editor.as_mut().unwrap().doc.save().unwrap();
    let ctx = shared.ctx.clone();
    close(&ctx, &mut shared);
    assert!(shared.editor.is_none());
}

#[test]
fn save_on_close_writes_the_project_and_closes() {
    let (harness, root, _folders) = answer(text::SAVE);
    assert!(harness.state().editor.is_none());
    assert!(keys_saved(&root));
    assert!(!root.join(SESSION_FILE).exists(), "nothing is left unsaved");
}

#[test]
fn dont_save_drops_the_working_state_and_closes() {
    let (harness, root, _folders) = answer(text::DONT_SAVE);
    assert!(harness.state().editor.is_none());
    assert!(!keys_saved(&root), "the saved project stays as it was");
    assert!(!root.join(SESSION_FILE).exists());
    let (reopened, problems) =
        crate::editor::Document::open(dc_project::ProjectDir::open(&root).unwrap()).unwrap();
    assert!(
        problems.is_empty(),
        "the history fits the project: {problems:?}"
    );
    assert!(!reopened.unsaved());
}

#[test]
fn cancel_keeps_the_window_and_the_changes() {
    let (harness, root, _folders) = answer(text::CANCEL);
    let editor = harness.state().editor.as_ref().unwrap();
    assert_eq!(editor.closing, Closing::Open);
    assert!(editor.doc.unsaved());
    assert!(!keys_saved(&root));
}

#[test]
fn a_save_that_fails_keeps_the_window_with_the_error() {
    let (_project, _settings, mut shared, root) = edited();
    std::fs::remove_file(root.join(PROJECT_FILE)).unwrap();
    std::fs::create_dir(root.join(PROJECT_FILE)).unwrap();
    shared.editor.as_mut().unwrap().closing = Closing::Asking(Leave::Close);
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(crate::editor::SIZE)
        .build_ui_state(|ui, shared: &mut Shared| frame(ui, shared), shared);
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Button, text::SAVE)
        .click();
    harness.run();
    assert!(
        harness.state().editor.is_some(),
        "the changes are not saved"
    );
    assert!(!harness.state().toasts.is_empty(), "the error is told");
}

#[test]
fn the_toasts_show_in_the_editor_window_while_the_hud_hides() {
    let (_project, _settings, shared, _root) = edited();
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(crate::editor::SIZE)
        .build_ui_state(|ui, shared: &mut Shared| frame(ui, shared), shared);
    let ctx = harness.ctx.clone();
    let told = "автозумов нет: в записях нет кликов";
    let state = harness.state_mut();
    state.ctx = ctx.clone();
    state
        .toasts
        .push(&ctx, dc_ui_kit::widgets::ToastKind::Error, told);
    harness.run();
    assert!(harness.query_by_label(told).is_some(), "the toast is seen");
}
