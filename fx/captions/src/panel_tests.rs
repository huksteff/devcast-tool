//! The captions panels on their own, as the editor places them: the tab's edits are `captions
//! style`, `clear`, `set`, `split`, `merge`, `rm`; a line of the list asks to pick its cue and to
//! seek to it, «Создать субтитры» and «Остановить» ask to start and stop the recognition, the
//! language and the tracks change the facts. The editor's snapshots and the path to the same
//! project as `devcast` are in dc-app (`inspector/captions_tests.rs`).

use std::collections::BTreeMap;

use dc_core::time::Output;
use dc_core::{OutputTime, Project, Range, Step};
use dc_ui_kit::egui;
use dc_ui_kit::panel::{Cx, Groups, Request};
use dc_ui_kit::strings::{captions as text, inspector as names, timeline};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use serde_json::{Value, json};

use crate::panel::{self, Language, List, Model, Phase, Recognition, Shown, Tracks};
use crate::tests::project;
use crate::{CaptionsEffect, CueId};

struct State {
    project: Project,
    defaults: Project,
    groups: BTreeMap<String, bool>,
    recognition: Recognition,
    list: List,
    /// The cue whose own panel is drawn; the rail tab when `None`.
    cue: Option<CueId>,
    playhead: OutputTime,
    edits: Vec<Vec<Step>>,
    requests: Vec<Request>,
}

fn o(s: f64) -> OutputTime {
    OutputTime::from_secs_f64(s)
}

fn at(a: f64, b: f64) -> Range<Output> {
    Range::new(o(a), o(b)).unwrap()
}

/// The cues of the test project where the timeline shows them: the second recording from 10 s.
fn list(picked: Option<u32>) -> List {
    let shown = [(1, 0.9, 3.1, "0:00.90"), (2, 4.9, 5.6, "0:04.90")]
        .into_iter()
        .chain([(3, 10.9, 12.1, "0:10.90")])
        .map(|(n, a, b, start)| Shown {
            id: CueId::from_number(n),
            at: at(a, b),
            start: start.to_owned(),
        })
        .collect();
    List {
        shown,
        picked: picked.map(CueId::from_number),
    }
}

fn idle() -> Recognition {
    Recognition {
        phase: Phase::Idle,
        model: Model::Fast,
        model_ready: false,
        language: Language::Auto,
        tracks: Tracks::Mic,
        system: false,
        auto_silence: None,
    }
}

fn harness(project: Project, recognition: Recognition, list: List) -> Harness<'static, State> {
    let state = State {
        project,
        defaults: Project::default(),
        groups: BTreeMap::new(),
        recognition,
        list,
        cue: None,
        playhead: o(1.5),
        edits: Vec::new(),
        requests: Vec::new(),
    };
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(340.0, 1100.0))
        .build_ui_state(draw, state);
    harness.run();
    harness
}

fn draw(ui: &mut egui::Ui, state: &mut State) {
    let fonts = ["Inter".to_owned()];
    let mut cx = Cx {
        project: &state.project,
        defaults: &state.defaults,
        playhead: state.playhead,
        auto_keys: false,
        fonts: &fonts,
        groups: Groups::new(&mut state.groups),
        edits: Vec::new(),
        adds: Vec::new(),
        requests: Vec::new(),
    };
    match state.cue {
        Some(id) => panel::cue(ui, &mut cx, id, at(0.9, 3.1)),
        None => {
            cx.groups.panel(panel::ID);
            panel::ui(ui, &mut cx, &mut state.recognition, &state.list);
        }
    }
    assert!(cx.adds.is_empty(), "the captions panels add nothing");
    let (edits, requests) = (cx.edits, cx.requests);
    state.edits.extend(edits);
    state.requests.extend(requests);
}

fn step(op: &str, args: Value) -> Vec<Step> {
    let Value::Object(args) = args else {
        panic!("arguments are an object");
    };
    vec![Step::new(op, args)]
}

fn click(harness: &mut Harness<'_, State>, role: egui::Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
    harness.run();
}

fn type_in(harness: &mut Harness<'_, State>, label: &str, value: f64) {
    harness
        .get_by_role_and_label(egui::Role::SpinButton, label)
        .set_value_accesskit(value);
    harness.run();
}

#[test]
fn a_line_asks_to_pick_its_cue_and_seek_to_it() {
    let mut harness = harness(project(), idle(), list(None));
    assert!(harness.query_by_label(text::CUE_TEXT).is_none());
    click(
        &mut harness,
        egui::Role::Button,
        &text::cue_line("0:04.90", "Пока!"),
    );
    let expected = vec![Request::Pick("s2".to_owned()), Request::Seek(o(4.9))];
    assert_eq!(harness.state().requests, expected);
    assert!(harness.state().edits.is_empty(), "a pick is no edit");
}

#[test]
fn the_picked_cue_of_the_list_is_edited_with_captions_commands() {
    let mut harness = harness(project(), idle(), list(Some(1)));
    assert!(harness.query_by_label(text::CUE_TEXT).is_some());
    click(&mut harness, egui::Role::Button, text::SPLIT);
    click(&mut harness, egui::Role::Button, text::MERGE);
    click(&mut harness, egui::Role::Button, names::DELETE);
    click(&mut harness, egui::Role::Button, text::CLEAR);
    let expected = vec![
        step("captions split", json!({"id": "s1", "at": 1.5})),
        step("captions merge", json!({"id": "s1"})),
        step("captions rm", json!({"id": "s1"})),
        step("captions clear", json!({})),
    ];
    assert_eq!(harness.state().edits, expected);
}

#[test]
fn a_cue_picked_on_the_timeline_has_its_editor_without_the_bin() {
    let mut harness = harness(project(), idle(), list(None));
    harness.state_mut().cue = Some(CueId::from_number(1));
    harness.run();
    assert!(harness.query_by_label(text::RECOGNITION).is_none());
    assert!(
        harness.query_by_label(names::DELETE).is_none(),
        "the frame has it"
    );
    click(&mut harness, egui::Role::Button, text::SPLIT);
    let expected = vec![step("captions split", json!({"id": "s1", "at": 1.5}))];
    assert_eq!(harness.state().edits, expected);
}

#[test]
fn recognition_is_asked_for_and_its_choices_are_the_facts() {
    let mut harness = harness(Project::default(), idle(), list(None));
    assert!(harness.query_by_label(text::EMPTY).is_some());
    click(&mut harness, egui::Role::Button, timeline::MAKE_CAPTIONS);
    assert_eq!(harness.state().requests, vec![Request::Recognize]);
    click(&mut harness, egui::Role::ComboBox, text::LANGUAGE);
    harness.get_by_label(text::RUSSIAN).click();
    harness.run();
    assert_eq!(harness.state().recognition.language, Language::Russian);
    click(&mut harness, egui::Role::ComboBox, text::TRACKS);
    harness.get_by_label(text::BOTH).click();
    harness.run();
    assert_eq!(
        harness.state().recognition.tracks,
        Tracks::Mic,
        "no system sound in the recordings"
    );
    harness.state_mut().recognition.system = true;
    harness.run();
    click(&mut harness, egui::Role::ComboBox, text::TRACKS);
    harness.get_by_label(text::BOTH).click();
    harness.run();
    assert_eq!(harness.state().recognition.tracks, Tracks::Both);
    assert!(harness.state().edits.is_empty(), "the choices are no edits");
}

#[test]
fn a_running_recognition_shows_its_progress_and_can_be_stopped() {
    let running = Recognition {
        phase: Phase::Recognizing(Some(0.64)),
        ..idle()
    };
    let mut harness = harness(project(), running, list(None));
    let line = text::recognizing(Some(64), dc_ui_kit::strings::settings::MODEL_FAST);
    assert!(harness.query_by_label(&line).is_some());
    let remake = harness.get_by_role_and_label(egui::Role::Button, text::REMAKE);
    assert!(
        remake.accesskit_node().is_disabled(),
        "one recognition at a time"
    );
    click(&mut harness, egui::Role::Button, text::CANCEL);
    assert_eq!(harness.state().requests, vec![Request::StopRecognizing]);
}

#[test]
fn the_look_is_captions_style_and_a_font_file_is_asked_for() {
    let mut harness = harness(project(), idle(), list(None));
    harness.get_by_label(text::STYLE).click();
    harness.run();
    click(&mut harness, egui::Role::Switch, text::SHOW);
    click(&mut harness, egui::Role::RadioButton, text::RISE);
    type_in(&mut harness, text::SIZE, 40.0);
    click(&mut harness, egui::Role::Button, names::ADD_FONT);
    let rise = json!(crate::Animation::Rise);
    let expected = vec![
        step("captions style", json!({"show": false})),
        step("captions style", json!({"animation": rise})),
        step("captions style", json!({"size": 40.0})),
    ];
    assert_eq!(harness.state().edits, expected);
    let choose = Step::new("captions style", serde_json::Map::new());
    assert_eq!(harness.state().requests, vec![Request::AddFont(choose)]);
    let state = harness.state().project.state::<CaptionsEffect>().unwrap();
    assert!(state.settings.show, "the panel only asks");
}

#[test]
fn the_silence_threshold_leaves_auto_at_the_recordings_own() {
    let told = Recognition {
        auto_silence: Some(-41.6),
        ..idle()
    };
    let mut harness = harness(project(), told, list(None));
    assert!(harness.query_by_label(&text::silence_auto(-41.6)).is_some());
    assert!(
        harness.query_by_label(text::SILENCE_LEVEL).is_none(),
        "auto: no level"
    );
    click(&mut harness, egui::Role::Switch, text::SILENCE);
    let leave = step("captions style", json!({"silence": -42.0}));
    assert_eq!(harness.state().edits, vec![leave]);
}

#[test]
fn a_threshold_by_hand_has_its_level_and_goes_back_to_auto() {
    let mut by_hand = project();
    let mut state = by_hand.state::<CaptionsEffect>().unwrap();
    state.settings.silence = crate::Silence::Db(-50.0);
    by_hand.set_state::<CaptionsEffect>(&state).unwrap();
    let mut harness = harness(by_hand, idle(), list(None));
    type_in(&mut harness, text::SILENCE_LEVEL, -55.0);
    click(&mut harness, egui::Role::Switch, text::SILENCE);
    let expected = vec![
        step("captions style", json!({"silence": -55.0})),
        step("captions style", json!({"silence": "auto"})),
    ];
    assert_eq!(harness.state().edits, expected);
}

#[test]
fn without_a_recognition_the_level_starts_at_minus_45() {
    let mut harness = harness(project(), idle(), list(None));
    assert!(harness.query_by_label(text::SILENCE_UNKNOWN).is_some());
    click(&mut harness, egui::Role::Switch, text::SILENCE);
    let leave = step("captions style", json!({"silence": -45.0}));
    assert_eq!(harness.state().edits, vec![leave]);
}
