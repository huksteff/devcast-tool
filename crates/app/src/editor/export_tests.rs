//! Task 7.31: the export dialog, the queue with a stand-in job, and the job of the editor equal
//! to the job `devcast export` makes of the same project.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use dc_core::PixelSize;
use dc_encode::export::{Exported, Stages};
use dc_encode::{EncodeError, Job};
use dc_ops::Step;
use dc_ops::export::{Format, Settings, Spec};
use dc_ui_kit::strings::{editor as names, export as text};
use eframe::egui;
use egui_kittest::kittest::Queryable;

use super::SIZE;
use super::export::{ExportDialog, queue, spec};
use super::tests::{editor, harness};
use crate::exports::{Exports, Runner};

/// A queue whose jobs write nothing: each takes its spec into `taken` and is done at once, or
/// fails when its file is named `сбой.mp4`.
fn stand_in() -> (Exports, Arc<Mutex<Vec<Spec>>>) {
    let taken = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&taken);
    let runner: Runner = Arc::new(move |spec: Spec| -> Job {
        let seen = Arc::clone(&seen);
        Box::new(move |_progress| {
            seen.lock().unwrap().push(spec.clone());
            if spec.file_name() == "сбой.mp4" {
                return Err(EncodeError::NothingLeft);
            }
            Ok(Exported {
                path: spec.out.clone(),
                size: PixelSize {
                    width: 1920,
                    height: 1080,
                },
                fps: spec.settings.fps,
                frames: 1,
                duration: dc_core::TimeDelta::from_micros(1),
                codec: None,
                bytes: 1_048_576,
                stages: Stages::default(),
            })
        })
    });
    (Exports::new(runner, || {}).unwrap(), taken)
}

/// The editor exports its working state, unsaved edits included, with the defaults of the
/// owner: the same spec `devcast export -o <file>.mp4` makes of the project on disk.
#[test]
fn the_editor_job_is_the_cli_job() {
    let (_temp, mut editor) = editor();
    let step = Step::new(
        "keys",
        serde_json::Map::from_iter([("show".to_owned(), true.into())]),
    );
    editor.doc.apply(&[step], Instant::now()).unwrap();
    editor.doc.flush().unwrap();
    let out = editor.doc.dir().root().join("урок 1.mp4");
    let settings = ExportDialog::default().settings();
    let app = spec(&editor, settings, out.clone());
    let working = dc_project::Working::load(editor.doc.dir(), false).unwrap();
    let cli = Spec {
        root: editor.doc.dir().root().to_path_buf(),
        project: working.project,
        settings: Settings::default_for(Format::Mp4),
        out,
    };
    assert_eq!(app, cli);
    assert_eq!(app.settings.describe(), "mp4 1080p 60 fps high");
    let gif = ExportDialog {
        format: Format::Gif,
        ..ExportDialog::default()
    };
    assert_eq!(gif.settings().describe(), "gif 1080p 15 fps по кругу");
}

#[test]
fn export_opens_the_dialog_and_escape_closes_it() {
    let mut harness = harness(SIZE);
    let (exports, _) = stand_in();
    harness.state_mut().editor.exports = Some(exports);
    harness.run();
    harness.get_by_label(names::EXPORT).click();
    harness.run();
    harness.get_by_label(text::SAVE_AS);
    harness.snapshot("editor_export");
    harness.get_by_label(text::GIF).click();
    harness.run();
    harness.get_by_label(text::NO_SOUND);
    harness.snapshot("editor_export_gif");
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(harness.state().editor.export.is_none());
}

#[test]
fn queued_exports_run_and_end_in_a_toast_and_the_list() {
    let mut harness = harness(SIZE);
    let (exports, taken) = stand_in();
    harness.state_mut().errors = Some(Vec::new());
    harness.state_mut().editor.exports = Some(exports.clone());
    let folder = harness.state().editor.doc.dir().root().to_path_buf();
    let settings = Settings::default_for(Format::Mp4);
    let editor = &mut harness.state_mut().editor;
    queue(editor, settings, folder.join("урок.mp4"));
    queue(editor, settings, folder.join("сбой.mp4"));
    exports.wait_idle();
    harness.run();
    let outcome = harness.state().outcome.clone().unwrap();
    assert_eq!(outcome.done, [text::finished("урок.mp4")]);
    let errors = harness.state().errors.clone().unwrap();
    assert!(
        errors.iter().any(|e| e.starts_with(&text::failed("сбой.mp4", ""))),
        "{errors:?}"
    );
    let outs: Vec<PathBuf> = taken.lock().unwrap().iter().map(|s| s.out.clone()).collect();
    assert_eq!(outs, [folder.join("урок.mp4"), folder.join("сбой.mp4")]);
    harness.get_by_label(text::SHOW_IN_FOLDER);
    harness.get_by_label(text::COPY_ERROR);
    harness.snapshot("editor_export_queue");
    harness.run();
    assert!(
        harness.state().outcome.clone().unwrap().done.is_empty(),
        "each export is told once"
    );
}
