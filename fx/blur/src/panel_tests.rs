//! The blur section of the annotation panel on its own (`dc_fx_annotations::panel::ui` with
//! `BlurBody`): strength, the tint's colour and cover (its alpha) and the radius are `blur set`;
//! «Сбросить» brings them back in one step. The editor's snapshot and the path to the same
//! project as `devcast` are in dc-app (`inspector/annot_kinds_tests.rs`).

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

use crate::BlurBody;

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
    panel::ui::<BlurBody>(ui, &mut cx, AnnotId::from_number(1), &region);
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
fn edits_are_blur_set() {
    let mut harness = harness(crate::tests::project(), 1.5, &[]);
    type_in(&mut harness, text::STRENGTH, 40.0);
    type_in(&mut harness, text::TINT_COVER, 50.0);
    type_in(&mut harness, text::RADIUS, 0.0);
    let expected = vec![
        step("blur set", json!({"id": "a1", "strength": 40.0})),
        step("blur set", json!({"id": "a1", "tint": "#00000080"})),
        step("blur set", json!({"id": "a1", "radius": 0.0})),
    ];
    assert_eq!(harness.state().edits, expected);
}

#[test]
fn reset_brings_the_blur_back_in_one_step() {
    let reset = dc_ui_kit::strings::inspector::reset_group(dc_ui_kit::strings::editor::BASIC);
    let fresh = harness(crate::tests::project(), 1.5, &[]);
    assert!(
        fresh.get_by_label(&reset).accesskit_node().is_disabled(),
        "the defaults already"
    );
    let mut project = crate::tests::project();
    let set: crate::BlurSet = serde_json::from_value(json!({"id": "a1", "strength": 60})).unwrap();
    dc_core::Command::apply(&set, &mut project).unwrap();
    let mut harness = harness(project, 1.5, &[]);
    click(&mut harness, egui::Role::Button, &reset);
    let d = BlurBody::default();
    let back = json!({"id": "a1", "strength": d.strength, "tint": d.tint, "radius": d.radius});
    assert_eq!(harness.state().edits, vec![step("blur set", back)]);
}
