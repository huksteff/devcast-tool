//! «Создать субтитры» with fake worker processes (no Whisper, `recognition::job_tests`): the
//! cues replace all cues as `captions gen` does, one undo step; the progress shows and
//! «Отменить» stops it; a failure is a toast; without the model folder nothing starts.

use dc_core::History;
use dc_fx_captions::CaptionsGen;
use dc_ui_kit::strings::{captions as text, timeline as names};
use eframe::egui;
use egui_kittest::kittest::{NodeT as _, Queryable};

use super::generation::{Generation, Phase};
use super::inspector::captions_tests::captions_tab;
use super::tests::project;
use crate::downloads::Downloads;
use crate::editor::editing_tests::json;
use crate::models::Models;
use crate::recognition::Message;
use crate::recognition::job::Spawn;
use crate::recognition::job_tests::{answering, cue, endless};
use crate::settings::SpeechModel;

type H = egui_kittest::Harness<'static, super::tests::State>;

/// The captions tab with `spawn` as the worker and the fast model on disk; the folders live
/// while the returned guard does.
fn with_worker(spawn: Spawn) -> (tempfile::TempDir, H) {
    let models = tempfile::tempdir().unwrap();
    let file = SpeechModel::Fast.model().file_name();
    std::fs::write(models.path().join(file), b"model").unwrap();
    let mut harness = captions_tab();
    let generation = Generation::new(
        Models::new(Some(models.path().to_path_buf()), Downloads::default()),
        spawn,
    );
    harness.state_mut().editor.captions = generation;
    harness.state_mut().errors = Some(Vec::new());
    harness.run();
    (models, harness)
}

fn make(harness: &mut H) {
    harness
        .get_by_role_and_label(egui::Role::Button, names::MAKE_CAPTIONS)
        .click();
    harness.run();
}

#[test]
fn the_cues_found_replace_all_cues_in_one_step() {
    let found = vec![cue("Привет", 1.0, 2.0), cue("мир", 2.5, 3.0)];
    let answer = [Message::Progress(0.5), Message::Cues(found.clone())];
    let (_answer, spawn) = answering(&answer);
    let (_models, mut harness) = with_worker(spawn);
    make(&mut harness);
    harness.state_mut().editor.captions.settle();
    harness.run();
    let mut expected = project();
    let command = CaptionsGen { cues: found };
    let mut history = History::default();
    dc_ops::apply_commands(
        &mut expected,
        &mut history,
        CaptionsGen::NAME,
        &[&command],
        false,
    )
    .unwrap();
    let editor = &harness.state().editor;
    assert_eq!(json(editor.doc.project()), json(&expected));
    assert_eq!(editor.captions.phase(), Phase::Idle);
    assert_eq!(harness.state().errors, Some(Vec::new()));
    let doc = &mut harness.state_mut().editor.doc;
    doc.travel(super::Way::Undo, std::time::Instant::now());
    history.undo(&mut expected);
    assert_eq!(json(doc.project()), json(&expected), "one step");
}

#[test]
fn the_progress_shows_and_cancel_stops_the_generation() {
    let (_models, mut harness) = with_worker(endless());
    make(&mut harness);
    assert_eq!(harness.state_mut().editor.captions.next(), None);
    harness.run();
    let phase = harness.state().editor.captions.phase();
    assert_eq!(phase, Phase::Recognizing(Some(0.25)));
    let button = harness.get_by_role_and_label(egui::Role::Button, names::MAKE_CAPTIONS);
    assert!(
        button.accesskit_node().is_disabled(),
        "one generation at a time"
    );
    harness.snapshot("inspector_captions_generating");
    harness
        .get_by_role_and_label(egui::Role::Button, text::CANCEL)
        .click();
    harness.run();
    assert_eq!(harness.state().editor.captions.phase(), Phase::Idle);
    assert_eq!(json(harness.state().editor.doc.project()), json(&project()));
}

#[test]
fn a_failed_recognition_is_a_toast_and_changes_nothing() {
    let why = "речи не найдено: субтитры не изменены";
    let (_answer, spawn) = answering(&[Message::Failed(why.to_owned())]);
    let (_models, mut harness) = with_worker(spawn);
    make(&mut harness);
    harness.state_mut().editor.captions.settle();
    harness.run();
    assert_eq!(harness.state().errors, Some(vec![why.to_owned()]));
    assert_eq!(json(harness.state().editor.doc.project()), json(&project()));
}

#[test]
fn without_the_model_folder_nothing_starts() {
    let mut harness = captions_tab();
    harness.state_mut().errors = Some(Vec::new());
    make(&mut harness);
    assert_eq!(
        harness.state().errors,
        Some(vec![text::NO_MODELS.to_owned()])
    );
    assert_eq!(harness.state().editor.captions.phase(), Phase::Idle);
    harness
        .get_by_role_and_label(egui::Role::Button, text::CLEAR)
        .click();
    harness.run();
    let hint = text::model_missing("Быстрая", text::FAST_SIZE);
    assert!(
        harness.query_by_label(&hint).is_some(),
        "the empty card says what it costs"
    );
}

#[test]
fn starting_opens_the_captions_tab() {
    let (_models, mut harness) = with_worker(endless());
    let editor = &mut harness.state_mut().editor;
    editor.view.tab = super::inspector::KEYS.id;
    editor.actions.push(super::actions::Action::Generate);
    harness.run();
    let view = &harness.state().editor.view;
    assert_eq!(view.tab, super::inspector::CAPTIONS.id);
    assert_eq!(view.object, None);
}
