//! The cursor panels on their own, as the editor places them: the tab's edits are `cursor` and
//! `clicks`, a picked click's are `click set --at` and `click reset` on the press of the facts
//! ([`Press`]). The editor's snapshots of them and the path to the same project as `devcast` are
//! in dc-app (`inspector/cursor_tests.rs`).

use std::collections::BTreeMap;

use dc_core::{Command, OutputTime, Project, Step};
use dc_ui_kit::egui;
use dc_ui_kit::panel::{Cx, Groups};
use dc_ui_kit::strings::{editor as text, inspector as names};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use serde_json::{Value, json};

use crate::panel::{self, Press, PressButton};
use crate::test_util::{RECORDING, o, project, s};
use crate::{ClickKind, ClickSet};

struct State {
    project: Project,
    defaults: Project,
    groups: BTreeMap<String, bool>,
    /// The press whose panel is drawn; the rail tab when `None`.
    press: Option<Press>,
    edits: Vec<Vec<Step>>,
}

fn harness(project: Project, press: Option<Press>) -> Harness<'static, State> {
    let state = State {
        project,
        defaults: Project::default(),
        groups: BTreeMap::new(),
        press,
        edits: Vec::new(),
    };
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(340.0, 1100.0))
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
    };
    match &state.press {
        Some(press) => panel::click(ui, &mut cx, press, "0:01.50"),
        None => {
            cx.groups.panel(panel::ID);
            panel::ui(ui, &mut cx);
        }
    }
    assert!(cx.adds.is_empty(), "the cursor panels add nothing");
    let edits = cx.edits;
    state.edits.extend(edits);
}

fn step(op: &str, args: Value) -> Vec<Step> {
    let Value::Object(args) = args else {
        panic!("arguments are an object");
    };
    vec![Step::new(op, args)]
}

/// The left press at 1.5 s of the recording.
fn press(double: bool) -> Press {
    Press {
        at: o(1.5),
        media: RECORDING.to_owned(),
        source: s(1.5),
        button: PressButton::Left,
        double,
    }
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
fn tab_edits_are_cursor_and_clicks() {
    let mut harness = harness(project(), None);
    click(&mut harness, egui::Role::Switch, names::CURSOR_SHOW);
    type_in(&mut harness, names::CURSOR_SIZE, 3.0);
    click(&mut harness, egui::Role::RadioButton, names::CLICK_RIPPLE);
    harness.get_by_label(text::FINE).click();
    harness.run();
    type_in(&mut harness, names::CLICK_OPACITY, 50.0);
    click(&mut harness, egui::Role::Switch, names::CURSOR_LOOP);
    let ripple = json!(ClickKind::Ripple);
    let expected = vec![
        step("cursor", json!({"show": false})),
        step("cursor", json!({"size": 3.0})),
        step("clicks", json!({"style": ripple})),
        step("clicks", json!({"opacity": 0.5})),
        step("cursor", json!({"loop": true})),
    ];
    assert_eq!(harness.state().edits, expected);
}

#[test]
fn tab_reset_brings_the_cursor_group_back_in_one_step() {
    let mut project = project();
    let bigger: crate::CursorSet = serde_json::from_value(json!({"size": 3.0})).unwrap();
    bigger.apply(&mut project).unwrap();
    let mut harness = harness(project, None);
    harness
        .get_by_label(&names::reset_group(names::CURSOR_GROUP))
        .click();
    harness.run();
    let [reset] = harness.state().edits.as_slice() else {
        panic!("one step: {:?}", harness.state().edits);
    };
    assert_eq!(reset.len(), 1);
    assert_eq!(reset[0].op, "cursor");
    let default = crate::CursorSettings::default().size;
    assert_eq!(reset[0].args["size"], json!(default));
}

#[test]
fn a_click_edits_are_click_set_at_its_output_time() {
    let mut harness = harness(project(), Some(press(true)));
    let head = harness.get_by_role_and_label(egui::Role::Heading, names::CLICK);
    let about = names::click_about("0:01.50", names::LEFT_BUTTON, true);
    assert_eq!(head.value().as_deref(), Some(about.as_str()));
    let reset = names::reset_group(names::CLICK_ONE);
    assert!(harness.get_by_label(&reset).accesskit_node().is_disabled());
    click(&mut harness, egui::Role::RadioButton, names::CLICK_ECHO);
    type_in(&mut harness, names::CLICK_SIZE, 2.0);
    let echo = json!(ClickKind::Echo);
    let expected = vec![
        step("click set", json!({"at": "1.500000", "style": echo})),
        step("click set", json!({"at": "1.500000", "scale": 2.0})),
    ];
    assert_eq!(harness.state().edits, expected);
}

#[test]
fn a_changed_click_shows_its_change_and_resets_with_click_reset() {
    let mut project = project();
    let args = json!({"at": "1.500000", "style": ClickKind::Echo});
    let set: ClickSet = serde_json::from_value(args).unwrap();
    set.apply(&mut project).unwrap();
    let mut harness = harness(project, Some(press(false)));
    let reset = names::reset_group(names::CLICK_ONE);
    assert!(!harness.get_by_label(&reset).accesskit_node().is_disabled());
    harness.get_by_label(&reset).click();
    harness.run();
    let back = step("click reset", json!({"at": "1.500000"}));
    assert_eq!(harness.state().edits, vec![back]);
}
