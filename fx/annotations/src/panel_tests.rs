//! The common part of the annotation panel on its own, over a kind with no rows of its own: the
//! key strip names the key at the playhead and asks to seek to its neighbours, edits of the box
//! and the rotation go into the key at the playhead (or a new one with «Авто-ключи»), the time,
//! row, layer and animations everywhere. The editor's snapshots of the whole panel, each kind and
//! the path to the same project as `devcast` are in dc-app (`inspector/annotation_tests.rs`,
//! `annot_kinds_tests.rs`).

use std::collections::BTreeMap;

use dc_core::time::Output;
use dc_core::{OutputTime, PixelSize, Project, Range, Step};
use dc_ui_kit::egui;
use dc_ui_kit::panel::{Cx, Groups, Request};
use dc_ui_kit::strings::{annotations as text, editor};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::panel::{self, Edit, Region, Section};
use crate::tests::{add, project, run};
use crate::{Animation, AnnotId, Kind};

/// A kind with no fields of its own: only the common rows.
#[derive(Deserialize)]
struct Plain {}

impl Section for Plain {
    const ID: &'static str = "plain";

    fn basic(&self, _ui: &mut egui::Ui, _cx: &mut Cx<'_>, _edit: Edit) {}

    fn basic_reset(&self, _edit: Edit) -> Option<Vec<Step>> {
        None
    }
}

struct State {
    project: Project,
    defaults: Project,
    groups: BTreeMap<String, bool>,
    playhead: OutputTime,
    auto_keys: bool,
    keys: Vec<OutputTime>,
    edits: Vec<Vec<Step>>,
    requests: Vec<Request>,
}

fn at(secs: f64) -> OutputTime {
    OutputTime::from_secs_f64(secs)
}

/// Where a1 is shown: 2–4 s of the output.
fn shown() -> Range<Output> {
    Range::new(at(2.0), at(4.0)).unwrap()
}

/// One 10 s clip with text a1 at 2–4 s; the panel with the playhead at `playhead` s and the
/// timeline's keys of a1 at `keys` s.
fn harness(playhead: f64, keys: &[f64], auto_keys: bool) -> Harness<'static, State> {
    let mut project = project(&[10.0]);
    run(
        &mut project,
        &add(json!({"kind": "text", "at": 2, "duration": 2})),
    );
    let state = State {
        project,
        defaults: Project::default(),
        groups: BTreeMap::new(),
        playhead: at(playhead),
        auto_keys,
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
    let mut cx = Cx {
        project: &state.project,
        defaults: &state.defaults,
        playhead: state.playhead,
        auto_keys: state.auto_keys,
        fonts: &[],
        groups: Groups::new(&mut state.groups),
        edits: Vec::new(),
        adds: Vec::new(),
        requests: Vec::new(),
    };
    let region = Region {
        range: shown(),
        keys: &state.keys,
        canvas: Some(PixelSize {
            width: 1920,
            height: 1080,
        }),
    };
    panel::ui::<Plain>(ui, &mut cx, AnnotId::from_number(1), &region);
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

fn fine(harness: &mut Harness<'_, State>) {
    harness.get_by_label(editor::FINE).click();
    harness.run();
}

fn type_in(harness: &mut Harness<'_, State>, label: &str, value: f64) {
    harness
        .get_by_role_and_label(egui::Role::SpinButton, label)
        .set_value_accesskit(value);
    harness.run();
}

#[test]
fn the_kind_of_an_annotation_picks_its_section() {
    let harness = harness(3.0, &[], false);
    let project = &harness.state().project;
    assert_eq!(
        panel::kind(project, AnnotId::from_number(1)),
        Some(Kind::Text)
    );
    assert_eq!(panel::kind(project, AnnotId::from_number(9)), None);
}

#[test]
fn edits_go_to_the_key_at_the_playhead_or_with_auto_keys_to_a_new_one() {
    let (keys, range) = ([at(2.5), at(3.5)], shown());
    assert_eq!(panel::target(&keys, at(3.5), range, false), Some(at(3.5)));
    assert_eq!(panel::target(&keys, at(3.0), range, false), None);
    assert_eq!(panel::target(&keys, at(3.0), range, true), Some(at(3.0)));
    assert_eq!(
        panel::target(&keys, at(5.0), range, true),
        None,
        "not shown"
    );
    assert_eq!(
        panel::key_at(&keys, at(3.5004)),
        Some(at(3.5)),
        "within 1 ms"
    );
}

#[test]
fn the_strip_names_the_key_and_seeks_to_its_neighbours() {
    let mut harness = harness(3.0, &[2.5, 3.0, 3.5], false);
    let said = text::key_of(2, 3, "0:03.00");
    assert!(harness.query_by_label(&said).is_some(), "«{said}»");
    harness.get_by_label(text::KEY_PREVIOUS).click();
    harness.run();
    harness.get_by_label(text::KEY_NEXT).click();
    harness.run();
    let expected = vec![Request::Seek(at(2.5)), Request::Seek(at(3.5))];
    assert_eq!(harness.state().requests, expected);
    let between = self::harness(2.7, &[2.5, 3.5], false);
    assert!(between.query_by_label(&text::no_key(2)).is_some());
    assert!(between.query_by_label(text::KEY_OFF_NOTE).is_some());
    let none = self::harness(2.7, &[], false);
    assert!(none.query_by_label(text::NO_KEYS).is_some());
}

#[test]
fn at_a_key_the_box_and_rotation_go_into_it_and_the_rest_everywhere() {
    let mut harness = harness(3.0, &[3.0], false);
    fine(&mut harness);
    type_in(&mut harness, text::ROTATION, 15.0);
    type_in(&mut harness, text::X, 960.0);
    harness
        .get_by_role_and_label(egui::Role::Button, text::LAYER_TOP)
        .click();
    harness.run();
    let key = "3.000000";
    let expected = vec![
        step(
            "annot set",
            json!({"id": "a1", "rotation": 15.0, "at": key}),
        ),
        step("annot set", json!({"id": "a1", "x": 0.5, "at": key})),
        step("annot layer", json!({"id": "a1", "step": "top"})),
    ];
    assert_eq!(harness.state().edits, expected);
}

#[test]
fn with_auto_keys_an_edit_makes_a_key_at_the_playhead() {
    let mut harness = harness(3.25, &[], true);
    assert!(harness.query_by_label(text::KEY_AUTO_NOTE).is_some());
    fine(&mut harness);
    type_in(&mut harness, text::ROTATION, -30.0);
    let expected = json!({"id": "a1", "rotation": -30.0, "at": "3.250000"});
    assert_eq!(harness.state().edits, vec![step("annot set", expected)]);
}

#[test]
fn an_entrance_is_annot_anim_and_its_reset_brings_both_stages_back() {
    let mut harness = harness(3.0, &[3.0], false);
    harness
        .get_by_role_and_label(egui::Role::ComboBox, text::ENTER)
        .click();
    harness.run();
    harness.get_by_label(text::ANIM_FADE).click();
    harness.run();
    let fade = json!({"id": "a1", "enter": Animation::Fade});
    assert_eq!(harness.state().edits, vec![step("annot anim", fade)]);
    let reset = dc_ui_kit::strings::inspector::reset_group(text::MOTION);
    let off = self::harness(3.0, &[], false);
    assert!(
        off.get_by_label(&reset).accesskit_node().is_disabled(),
        "the stages are the defaults"
    );
}
