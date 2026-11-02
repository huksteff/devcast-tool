//! Task 7.31: the export dialog, the queue with a stand-in job, and the job of the editor equal
//! to the job `devcast export` makes of the same project.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use dc_core::PixelSize;
use dc_encode::export::{Exported, Stages};
use dc_encode::{Codec, EncodeError, Job};
use dc_ops::Step;
use dc_ops::export::{Format, Res, Settings, Spec};
use dc_settings::Store;
use dc_ui_kit::strings::{editor as names, export as text};
use eframe::egui;
use egui_kittest::kittest::Queryable;

use super::SIZE;
use super::export::{ExportDialog, queue, recall, remember, spec};
use super::tests::{editor, harness, harness_of};
use crate::exports::{Encoder, Exports, Probe, Runner};

/// A queue whose jobs write nothing: each takes its spec into `taken` and is done at once, or
/// fails when its file is named `сбой.mp4`. Its encoder check finds NVENC.
fn stand_in() -> (Exports, Arc<Mutex<Vec<Spec>>>) {
    stand_in_with(Arc::new(|| Ok(Codec::Nvenc)))
}

fn stand_in_with(probe: Probe) -> (Exports, Arc<Mutex<Vec<Spec>>>) {
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
    (Exports::with_probe(runner, probe, || {}).unwrap(), taken)
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
    assert_eq!(gif.settings().describe(), "gif 720p 15 fps по кругу");
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
    harness.get_by_label(text::ESTIMATE_HINT);
    let exports = harness.state().editor.exports.clone().unwrap();
    assert_eq!(exports.wait_encoder(), Encoder::Ready(Codec::Nvenc));
    harness.run();
    harness.get_by_label_contains(text::CHECKED);
    harness.get_by_label(text::NVENC);
    harness.snapshot("editor_export");
    harness.get_by_label(text::GIF).click();
    harness.run();
    harness.get_by_label(text::NO_SOUND);
    assert!(
        harness.query_by_label_contains(text::ENCODES).is_none(),
        "GIF has no encoder row"
    );
    harness.snapshot("editor_export_gif");
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(harness.state().editor.export.is_none());
}

/// The check says «проверяю…» until it answers; a failed one shows what to do.
#[test]
fn the_encoder_row_waits_for_the_check_and_shows_its_failure() {
    let (release, open) = std::sync::mpsc::channel::<()>();
    let gate = Mutex::new(open);
    let probe: Probe = Arc::new(move || {
        gate.lock().unwrap().recv().unwrap();
        Err("ни один кодек H.264 не работает: обнови драйвер".to_owned())
    });
    let mut harness = harness(SIZE);
    let (exports, _) = stand_in_with(probe);
    harness.state_mut().editor.exports = Some(exports.clone());
    harness.run();
    harness.get_by_label(names::EXPORT).click();
    harness.run();
    harness.get_by_label_contains(text::CHECKING);
    release.send(()).unwrap();
    assert!(matches!(exports.wait_encoder(), Encoder::Failed(_)));
    harness.run();
    harness.get_by_label_contains(text::CHECK_FAILED);
    harness.get_by_label("ни один кодек H.264 не работает: обнови драйвер");
}

/// The last choices go to `settings.json` and the dialog of the next editor opens with them;
/// a file without them gives the defaults.
#[test]
fn the_last_choices_survive_closing_the_dialog_and_the_app() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder
        .path()
        .join("Настройки DevCast")
        .join("settings.json");
    let mut store = Store::load(&path).unwrap();
    assert_eq!(store.get().export, ExportDialog::default());
    let mut harness = harness(SIZE);
    let (exports, _) = stand_in();
    harness.state_mut().editor.exports = Some(exports.clone());
    recall(&mut harness.state_mut().editor, store.get());
    harness.run();
    harness.get_by_label(names::EXPORT).click();
    harness.run();
    harness.get_by_label(text::GIF).click();
    harness.run();
    harness.get_by_label(text::P1080).click();
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(harness.state().editor.export.is_none());
    remember(&harness.state().editor, &mut store).unwrap();
    drop(harness);

    let store = Store::load(&path).unwrap();
    let (temp, mut again) = editor();
    again.exports = Some(exports);
    recall(&mut again, store.get());
    let mut harness = harness_of(temp, again, SIZE, Default::default());
    harness.run();
    harness.get_by_label(names::EXPORT).click();
    harness.run();
    harness.get_by_label(text::NO_SOUND);
    let shown = harness.state().editor.export.unwrap();
    assert_eq!(shown.format, Format::Gif);
    assert_eq!(shown.gif.res, Res::P1080);
    assert_eq!(shown.mp4, Settings::default_for(Format::Mp4));
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
    assert_eq!(harness.state().done, [text::finished("урок.mp4")]);
    let errors = harness.state().errors.clone().unwrap();
    assert!(
        errors
            .iter()
            .any(|e| e.starts_with(&text::failed("сбой.mp4", ""))),
        "{errors:?}"
    );
    let outs: Vec<PathBuf> = taken
        .lock()
        .unwrap()
        .iter()
        .map(|s| s.out.clone())
        .collect();
    assert_eq!(outs, [folder.join("урок.mp4"), folder.join("сбой.mp4")]);
    harness.get_by_label(text::SHOW_IN_FOLDER);
    harness.get_by_label(text::COPY_ERROR);
    harness.snapshot("editor_export_queue");
    harness.run();
    assert_eq!(harness.state().done.len(), 1, "each export is told once");
}
