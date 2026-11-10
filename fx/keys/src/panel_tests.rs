//! The panel «Показ клавиш» on its own, as the editor places it: its facts from the shortcuts,
//! its edits are the `keys` command, «Сбросить» brings the defaults. The editor's snapshots of it
//! are in dc-app (`inspector_keys*.png`).

use std::collections::BTreeMap;

use dc_core::{OutputTime, Project, Step, TimeMap};
use dc_ui_kit::egui;
use dc_ui_kit::panel::{Cx, Groups};
use dc_ui_kit::strings::{editor as text, inspector as names};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;

use crate::Keys;
use crate::panel::{self, Sample, Shortcuts};
use crate::test_util::{project, shown, tracks};

struct State {
    project: Project,
    defaults: Project,
    groups: BTreeMap<String, bool>,
    shortcuts: Shortcuts,
    edits: Vec<Vec<Step>>,
}

fn harness(project: Project, shortcuts: Shortcuts) -> Harness<'static, State> {
    let state = State {
        project,
        defaults: Project::default(),
        groups: BTreeMap::new(),
        shortcuts,
        edits: Vec::new(),
    };
    dc_ui_kit::testing::harness()
        .with_size(egui::vec2(340.0, 600.0))
        .build_ui_state(
            |ui, state: &mut State| {
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
                panel::ui(ui, &mut cx, &state.shortcuts);
                let edits = cx.edits;
                state.edits.extend(edits);
            },
            state,
        )
}

fn keys(args: serde_json::Value) -> Vec<Step> {
    let serde_json::Value::Object(args) = args else {
        panic!("arguments are an object");
    };
    vec![Step::new("keys", args)]
}

#[test]
fn the_facts_come_from_the_plates_on_the_output() {
    let project = project();
    let map = TimeMap::build(&project);
    let prepared = crate::Prepared::new(shown(), &tracks(), &map);
    let facts = Shortcuts::of(&prepared, &tracks());
    let sample = Sample {
        name: "Ctrl+Z".to_owned(),
        count: 3,
    };
    assert_eq!(facts.names, 5);
    assert_eq!(facts.sample, Some(sample));
}

#[test]
fn edits_are_the_keys_command() {
    let shortcuts = Shortcuts {
        names: 3,
        sample: Some(Sample {
            name: "Ctrl+Shift+P".to_owned(),
            count: 2,
        }),
    };
    let mut harness = harness(project(), shortcuts);
    harness.run();
    let head = harness.get_by_role_and_label(egui::Role::Heading, text::KEYS);
    assert_eq!(head.value().as_deref(), Some("3 клавиши в записях"));
    assert!(harness.query_by_label("Ctrl + Shift + P ×2").is_some());
    harness
        .get_by_role_and_label(egui::Role::Switch, names::KEYS_SHOW)
        .click();
    harness.run();
    assert_eq!(harness.state().edits, vec![keys(json!({"show": true}))]);
}

#[test]
fn reset_brings_the_defaults_of_the_group() {
    let mut project = project();
    project.set_state::<Keys>(&shown()).unwrap();
    let mut harness = harness(project, Shortcuts::default());
    harness.run();
    let reset = names::reset_group(text::BASIC);
    harness.get_by_label(&reset).click();
    harness.run();
    let default = json!({"show": false, "position": "bottom", "size": 1.0});
    assert_eq!(harness.state().edits, vec![keys(default)]);
    assert!(harness.query_by_label("Ctrl + Shift + P ×2").is_none());
}
