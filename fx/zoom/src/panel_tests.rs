//! The zoom panels on their own, as the editor places them: edits are `zoom motion` and
//! `zoom set` in `edits`, «Подобрать зумы по кликам» is `zoom add --at clicks` in `adds`. The
//! editor's snapshots of them and the path to the same project as `devcast` are in dc-app
//! (`inspector/zoom_tests.rs`).

use std::collections::BTreeMap;

use dc_core::{OutputTime, Project, Step};
use dc_ui_kit::egui;
use dc_ui_kit::panel::{Cx, Groups};
use dc_ui_kit::strings::{editor as text, inspector as names};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

use crate::tests::{add, project, run};
use crate::{Mode, Preset, ZoomEffect, ZoomId, by_clicks, panel};

struct State {
    project: Project,
    defaults: Project,
    groups: BTreeMap<String, bool>,
    /// The zoom whose panel is drawn; the rail tab when `None`.
    region: Option<ZoomId>,
    edits: Vec<Vec<Step>>,
    adds: Vec<Vec<Step>>,
}

fn harness(project: Project, region: Option<ZoomId>) -> Harness<'static, State> {
    let state = State {
        project,
        defaults: Project::default(),
        groups: BTreeMap::new(),
        region,
        edits: Vec::new(),
        adds: Vec::new(),
    };
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(340.0, 900.0))
        .build_ui_state(draw, state);
    harness.run();
    harness
}

fn draw(ui: &mut egui::Ui, state: &mut State) {
    let mut cx = Cx {
        project: &state.project,
        defaults: &state.defaults,
        playhead: OutputTime::ZERO,
        auto_keys: false,
        fonts: &[],
        groups: Groups::new(&mut state.groups),
        edits: Vec::new(),
        adds: Vec::new(),
        requests: Vec::new(),
    };
    match state.region {
        Some(id) => panel::region(ui, &mut cx, id),
        None => {
            cx.groups.panel(panel::ID);
            panel::ui(ui, &mut cx);
        }
    }
    let (edits, adds) = (cx.edits, cx.adds);
    state.edits.extend(edits);
    state.adds.extend(adds);
}

fn step(op: &str, args: Value) -> Vec<Step> {
    let Value::Object(args) = args else {
        panic!("arguments are an object");
    };
    vec![Step::new(op, args)]
}

/// A project of one 10 s clip with one zoom at 2 s; its id.
fn with_zoom() -> (Project, ZoomId) {
    let mut project = project(&[10.0]);
    run(&mut project, &add(2.0, 1.0));
    let id = project.state::<ZoomEffect>().unwrap().zooms[0].id;
    (project, id)
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
fn tab_edits_are_zoom_motion_and_suggest_is_a_step_that_adds() {
    let mut harness = harness(project(&[10.0]), None);
    click(&mut harness, egui::Role::RadioButton, names::ZOOM_SMOOTH);
    click(&mut harness, egui::Role::Switch, names::ZOOM_AUTO);
    let smooth = serde_json::to_value(Preset::Smooth).unwrap();
    let expected = vec![
        step("zoom motion", json!({"preset": smooth})),
        step("zoom motion", json!({"auto": false})),
    ];
    assert_eq!(harness.state().edits, expected);
    assert!(harness.state().adds.is_empty());
    click(&mut harness, egui::Role::Button, names::ZOOM_SUGGEST);
    assert_eq!(harness.state().adds, vec![vec![by_clicks()]]);
    assert_eq!(harness.state().edits.len(), 2, "suggest is no edit");
}

#[test]
fn fine_tuning_is_zoom_motion() {
    let mut harness = harness(project(&[10.0]), None);
    harness.get_by_label(text::FINE).click();
    harness.run();
    type_in(&mut harness, names::ZOOM_BLUR, 1.0);
    type_in(&mut harness, names::ZOOM_IN, 0.5);
    let expected = vec![
        step("zoom motion", json!({"blur": 1.0})),
        step("zoom motion", json!({"zoom_in": 0.5})),
    ];
    assert_eq!(harness.state().edits, expected);
}

#[test]
fn a_picked_zoom_edits_are_zoom_set() {
    let (project, id) = with_zoom();
    let mut harness = harness(project, Some(id));
    click(&mut harness, egui::Role::RadioButton, names::ZOOM_FIXED);
    type_in(&mut harness, names::ZOOM_SCALE, 2.0);
    type_in(&mut harness, names::ZOOM_X, 25.0);
    let (fixed, id) = (json!(Mode::Fixed), json!(id));
    let expected = vec![
        step("zoom set", json!({"id": id, "mode": fixed})),
        step("zoom set", json!({"id": id, "scale": 2.0})),
        step("zoom set", json!({"id": id, "x": 0.25})),
    ];
    assert_eq!(harness.state().edits, expected);
    click(&mut harness, egui::Role::Button, names::ZOOM_SUGGEST);
    assert_eq!(harness.state().adds, vec![vec![by_clicks()]]);
}

#[test]
fn reset_of_a_picked_zoom_is_what_zoom_add_gives_in_one_step() {
    let (mut project, id) = with_zoom();
    let set: crate::ZoomSet =
        serde_json::from_value(json!({"id": id, "scale": 3.0, "mode": Mode::Fixed})).unwrap();
    run(&mut project, &set);
    let mut harness = harness(project, Some(id));
    harness
        .get_by_label(&names::reset_group(text::BASIC))
        .click();
    harness.run();
    let (follow, id) = (json!(Mode::Follow), json!(id));
    let back = json!({"id": id, "scale": 1.5, "mode": follow, "x": 0.5, "y": 0.5});
    assert_eq!(harness.state().edits, vec![step("zoom set", back)]);
}

#[test]
fn a_zoom_that_is_gone_draws_nothing() {
    let harness = harness(project(&[10.0]), Some(ZoomId::from_number(7)));
    assert!(harness.query_by_label(names::ZOOM_SCALE).is_none());
    assert!(harness.state().edits.is_empty());
}
