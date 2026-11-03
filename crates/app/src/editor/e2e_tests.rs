//! Task 7.33: the editor window end to end on a fixture recording, as the app opens it after a
//! stop: auto zooms, undo, «Подобрать зумы», a text annotation by T and the inspector, captions
//! by a fake recognition worker, undo, Ctrl+S, closing, opening again from the recent projects
//! and an export. After every step the project equals the one `devcast` makes of the same
//! recording with the same commands ([`Twin`]).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dc_core::{Command, History, OutputTime, Project, Range, SourceTime};
use dc_fx_annotations::Annotations;
use dc_fx_captions::{CaptionsEffect, CaptionsGen, NewCue, Word};
use dc_fx_zoom::ZoomEffect;
use dc_ops::export::{Format, Settings, Spec};
use dc_ops::{Registry, Step};
use dc_project::dir::SESSION_FILE;
use dc_project::{ProjectDir, ProjectLock, Working};
use dc_ui_kit::strings::{annotations as annot, timeline as rows};
use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::Queryable;

use super::EditorWindow;
use super::editing_tests::json;
use super::export_tests::stand_in;
use super::window::frame;
use crate::app::Shared;
use crate::downloads::Downloads;
use crate::models::Models;
use crate::recognition::Message;
use crate::recognition::job_tests::answering;
use crate::settings::Store;

pub(super) type H = egui_kittest::Harness<'static, Shared>;

/// The media of the fixture recording.
const MEDIA: &str = "media/recording-1/video.mp4";

/// The same recording edited by `devcast`: the CLI's parsing, its expansion of
/// `zoom add --at clicks` and apply, on a second copy of the fixture.
pub(super) struct Twin {
    root: PathBuf,
    registry: Registry,
    pub project: Project,
    history: History,
}

impl Twin {
    /// The copy `dir` just recorded by `devcast record`: its auto zooms.
    fn recorded(dir: &ProjectDir) -> Self {
        let lock = ProjectLock::acquire(dir, "devcast").unwrap();
        dc_ops::autozoom::after_recording(dir, &lock).unwrap();
        drop(lock);
        let working = Working::load(dir, false).unwrap();
        Self {
            root: dir.root().to_path_buf(),
            registry: Registry::devcast().unwrap(),
            project: working.project,
            history: working.history,
        }
    }

    /// `devcast <line>`.
    pub fn run(&mut self, line: &str) {
        let words: Vec<String> = line.split(' ').map(str::to_owned).collect();
        let (info, taken) = self.registry.resolve(&words).unwrap();
        let args = dc_ops::parse_args(info, &words[taken..]).unwrap();
        let steps = [Step::new(info.name, args)];
        let mut notes = Vec::new();
        let (root, project) = (&self.root, &self.project);
        let commands = dc_ops::build_with(&self.registry, &steps, |step| {
            dc_ops::autozoom::expand(root, project, step, &mut notes)
        })
        .unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        let commands: Vec<&dyn Command> = commands.iter().map(AsRef::as_ref).collect();
        self.apply(dc_ops::step_name(&steps), &commands);
    }

    /// Commands the CLI builds itself (`captions gen` of the cues found) as one step `name`.
    fn apply(&mut self, name: &str, commands: &[&dyn Command]) {
        let (project, history) = (&mut self.project, &mut self.history);
        dc_ops::apply_commands(project, history, name, commands, false).unwrap();
    }

    /// `devcast undo`.
    pub fn undo(&mut self) {
        self.history.undo(&mut self.project).unwrap();
    }
}

/// The folders of a scenario, the app's recording and its root, the CLI's twin.
pub(super) struct Recorded {
    pub temp: tempfile::TempDir,
    pub root: PathBuf,
    pub twin: Twin,
}

/// A copy of `clicks.devcast` saved under `parent/name.devcast`, as a stop leaves it.
fn copy(parent: &Path, name: &str) -> ProjectDir {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/clicks.devcast");
    let source = ProjectDir::open(fixture).unwrap();
    let project = source.load().unwrap();
    std::fs::create_dir_all(parent).unwrap();
    source
        .save_as(&project, parent.join(format!("{name}.devcast")))
        .unwrap()
}

/// The fixture recorded by the app (its auto zooms, `recorder::auto_zooms`) into
/// `Записи/Запись 1.devcast` and by `devcast record` into the twin's folder.
pub(super) fn recorded() -> Recorded {
    let temp = tempfile::tempdir().unwrap();
    let app = copy(&temp.path().join("Записи"), "Запись 1");
    crate::recorder::auto_zooms(&app).unwrap();
    let cli = copy(&temp.path().join("devcast"), "Запись 1");
    Recorded {
        root: app.root().to_path_buf(),
        twin: Twin::recorded(&cli),
        temp,
    }
}

/// The app's window with the editor, the specs its export queue took and the answer file of
/// its recognition worker.
pub(super) struct App {
    pub harness: H,
    pub taken: Arc<Mutex<Vec<Spec>>>,
    pub _answer: tempfile::TempDir,
}

/// The app with its settings in `folder`, the editor of `root` opened as after a stop
/// (`Shared::open_editor`), its window in a harness. Exports only take their spec
/// (`export_tests::stand_in`); recognition is a worker answering `cues`.
pub(super) fn opened(folder: &Path, root: &Path, cues: &[NewCue]) -> App {
    let store = Store::load(&folder.join("settings.json")).unwrap();
    let mut shared = Shared::new(egui::Context::default(), store);
    let (exports, taken) = stand_in();
    shared.exports = Some(exports);
    assert!(shared.open_project(root, true));
    let answer = worker(&mut shared, folder, cues);
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(1280.0, 1000.0))
        .build_ui_state(|ui, shared: &mut Shared| frame(ui, shared), shared);
    harness.run();
    App {
        harness,
        taken,
        _answer: answer,
    }
}

/// A fake recognition worker that finds `cues`, with the model of the settings on disk; the
/// folder of its answer.
fn worker(shared: &mut Shared, folder: &Path, cues: &[NewCue]) -> tempfile::TempDir {
    let models = folder.join("models");
    std::fs::create_dir_all(&models).unwrap();
    let file = shared.store.get().speech_model.model().file_name();
    std::fs::write(models.join(file), b"model").unwrap();
    let (answer, spawn) = answering(&[Message::Cues(cues.to_vec())]);
    let editor = shared.editor.as_mut().unwrap();
    let models = Models::new(Some(models), Downloads::default());
    editor.captions = super::generation::Generation::new(models, spawn);
    answer
}

pub(super) fn editor(harness: &H) -> &EditorWindow {
    harness.state().editor.as_ref().unwrap()
}

pub(super) fn editor_mut(harness: &mut H) -> &mut EditorWindow {
    harness.state_mut().editor.as_mut().unwrap()
}

/// The editor's project is the twin's.
#[track_caller]
pub(super) fn same(harness: &H, twin: &Twin, step: &str) {
    let app = json(editor(harness).doc.project());
    assert_eq!(app, json(&twin.project), "{step}");
    assert!(harness.state().toasts.is_empty(), "{step}: a toast");
}

pub(super) fn ctrl(harness: &mut H, key: Key) {
    harness.key_press_modifiers(Modifiers::COMMAND, key);
    harness.run();
}

/// T at 2 s of the output, then the size and the words in the inspector.
pub(super) fn text_annotation(harness: &mut H, twin: &mut Twin) {
    let view = &mut editor_mut(harness).view;
    view.selection.clear();
    view.playhead = OutputTime::from_secs_f64(2.0);
    harness.run();
    harness.key_press(Key::T);
    harness.run();
    twin.run("annot add text --at 2 --duration 1 --track 0");
    same(harness, twin, "T");
    let added = twin.project.state::<Annotations>().unwrap().annotations;
    assert_eq!(added.len(), 1);
    harness
        .get_by_role_and_label(egui::Role::SpinButton, annot::SIZE)
        .set_value_accesskit(40.0);
    harness.run();
    twin.run("text set a1 --size 40");
    same(harness, twin, "the size");
    harness.get_by_label(annot::TEXT_HINT).focus();
    harness.run();
    harness.get_by_label(annot::TEXT_HINT).type_text("Вход");
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::SpinButton, annot::SIZE)
        .focus();
    harness.run();
    twin.run("text set a1 --text ТекстВход");
    same(harness, twin, "the words");
}

fn cue(said: &str, start: f64, end: f64) -> NewCue {
    let range = Range::new(
        SourceTime::from_secs_f64(start),
        SourceTime::from_secs_f64(end),
    )
    .unwrap();
    let words = vec![Word {
        text: said.to_owned(),
        range,
    }];
    NewCue {
        media: MEDIA.to_owned(),
        range,
        words,
    }
}

/// The spec the export queue took is the one `devcast export -o <out>` makes of the project on
/// disk once autosave has written the working state, with the settings' defaults.
pub(super) fn exported(harness: &mut H, taken: &Arc<Mutex<Vec<Spec>>>, out: PathBuf) -> Spec {
    let settings = harness.state().store.get().export.settings();
    let window = editor_mut(harness);
    super::export::queue(window, settings, out.clone());
    window.exports.clone().unwrap().wait_idle();
    window.doc.flush().unwrap();
    harness.run();
    let dir = editor(harness).doc.dir().clone();
    let cli = Spec {
        root: dir.root().to_path_buf(),
        project: Working::load(&dir, false).unwrap().project,
        settings: Settings::default_for(Format::Mp4),
        out,
    };
    let app = taken.lock().unwrap().pop().unwrap();
    assert_eq!(app, cli);
    app
}

#[test]
fn a_recording_edited_saved_opened_and_exported_is_the_cli_project() {
    let Recorded {
        temp,
        root,
        mut twin,
    } = recorded();
    let found = [cue("Нажимаем", 1.0, 2.0), cue("сюда", 2.5, 3.5)];
    let App {
        mut harness,
        taken,
        _answer,
    } = opened(temp.path(), &root, &found);
    assert!(!editor(&harness).recovery, "a fresh recording is saved");
    same(&harness, &twin, "the auto zooms of the recording");

    ctrl(&mut harness, Key::Z);
    twin.undo();
    same(&harness, &twin, "undo of the auto zooms");
    harness
        .get_by_role_and_label(egui::Role::Button, rows::SUGGEST_ZOOMS)
        .click();
    harness.run();
    twin.run("zoom add --at clicks");
    same(&harness, &twin, "«Подобрать зумы»");
    let zooms = twin.project.state::<ZoomEffect>().unwrap().zooms;
    assert!(!zooms.is_empty());

    text_annotation(&mut harness, &mut twin);

    editor_mut(&mut harness).view.selection.clear();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Button, rows::MAKE_CAPTIONS)
        .click();
    harness.run();
    editor_mut(&mut harness).captions.settle();
    harness.run();
    let command = CaptionsGen {
        cues: found.to_vec(),
    };
    twin.apply(CaptionsGen::NAME, &[&command]);
    same(&harness, &twin, "captions");
    let cues = twin.project.state::<CaptionsEffect>().unwrap().cues;
    assert_eq!(cues.len(), found.len());

    ctrl(&mut harness, Key::Z);
    twin.undo();
    same(&harness, &twin, "undo of the captions");

    ctrl(&mut harness, Key::S);
    assert!(!editor(&harness).doc.unsaved());
    assert!(!root.join(SESSION_FILE).exists(), "nothing left unsaved");
    let saved = ProjectDir::open(&root).unwrap().load().unwrap();
    assert_eq!(json(&saved), json(&twin.project), "project.json");

    let ctx = harness.state().ctx.clone();
    super::window::tests::close(&ctx, harness.state_mut());
    assert!(harness.state().editor.is_none(), "saved: closes at once");
    let recent = harness.state().store.get().recent_projects.clone();
    assert_eq!(recent, std::slice::from_ref(&root));
    let answer = harness.state_mut().open_asked(&recent[0]);
    assert_eq!(answer, dc_ops::live::Answer::Opened);
    harness.run();
    same(&harness, &twin, "opened again");
    ctrl(&mut harness, Key::Z);
    twin.undo();
    same(&harness, &twin, "the history survives closing");

    let spec = exported(&mut harness, &taken, root.join("урок 1.mp4"));
    assert_eq!(json(&spec.project), json(&twin.project));
}
