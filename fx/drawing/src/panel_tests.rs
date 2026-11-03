//! The drawing section of the annotation panel on its own (`dc_fx_annotations::panel::ui` with
//! `DrawingBody`): the pencil and the opacity are `drawing set`, «Очистить рисунок» is
//! `drawing clear`, off without strokes. The editor's snapshot and the path to the same project
//! as `devcast` are in dc-app (`inspector/annot_kinds_tests.rs`).

use std::collections::BTreeMap;

use dc_core::{OutputTime, PixelSize, Project, Range, Step};
use dc_fx_annotations::AnnotId;
use dc_fx_annotations::panel::{self, Region};
use dc_ui_kit::egui;
use dc_ui_kit::panel::{Cx, Groups, Request};
use dc_ui_kit::strings::annotations as text;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use serde_json::{Value, json};

use crate::DrawingBody;

struct State {
    project: Project,
    defaults: Project,
    groups: BTreeMap<String, bool>,
    playhead: OutputTime,
    /// The timeline's keys of a1, output times.
    keys: Vec<OutputTime>,
    edits: Vec<Vec<Step>>,
    requests: Vec<Request>,
}

fn at(secs: f64) -> OutputTime {
    OutputTime::from_secs_f64(secs)
}

/// The panel of a1 of `project` (shown at 1–2 s of the output) with the playhead at
/// `playhead` s and the timeline's keys of a1 at `keys` s.
fn harness(project: Project, playhead: f64, keys: &[f64]) -> Harness<'static, State> {
    let state = State {
        project,
        defaults: Project::default(),
        groups: BTreeMap::new(),
        playhead: at(playhead),
        keys: keys.iter().map(|k| at(*k)).collect(),
        edits: Vec::new(),
        requests: Vec::new(),
    };
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(340.0, 1400.0))
        .build_ui_state(draw, state);
    harness.run();
    harness
}

fn draw(ui: &mut egui::Ui, state: &mut State) {
    let fonts = ["Inter".to_owned(), "Montserrat".to_owned()];
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
    let region = Region {
        range: Range::new(at(1.0), at(2.0)).unwrap(),
        keys: &state.keys,
        canvas: Some(PixelSize {
            width: 1920,
            height: 1080,
        }),
    };
    panel::ui::<DrawingBody>(ui, &mut cx, AnnotId::from_number(1), &region);
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
fn edits_are_drawing_set_and_clear() {
    let mut project = crate::tests::project();
    let points = json!({"id": "a1", "points": "0.1,0.1,0.9,0.9"});
    crate::tests::run::<crate::DrawingAdd>(&mut project, points);
    let mut harness = harness(project, 1.5, &[]);
    assert!(harness.query_by_label(&text::strokes(1)).is_some());
    type_in(&mut harness, text::THICKNESS, 8.0);
    type_in(&mut harness, text::OPACITY, 50.0);
    click(&mut harness, egui::Role::Button, text::CLEAR);
    let expected = vec![
        step("drawing set", json!({"id": "a1", "width": 8.0})),
        step("drawing set", json!({"id": "a1", "opacity": 0.5})),
        step("drawing clear", json!({"id": "a1"})),
    ];
    assert_eq!(harness.state().edits, expected);
}

#[test]
fn clear_is_off_without_strokes_and_reset_is_off_at_the_defaults() {
    let harness = harness(crate::tests::project(), 1.5, &[]);
    assert!(
        harness
            .get_by_label(text::CLEAR)
            .accesskit_node()
            .is_disabled()
    );
    let reset = dc_ui_kit::strings::inspector::reset_group(dc_ui_kit::strings::editor::BASIC);
    assert!(harness.get_by_label(&reset).accesskit_node().is_disabled());
}
