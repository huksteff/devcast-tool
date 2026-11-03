//! The panel of a callout event on its own, as the editor places it under its header: the
//! header's words from the event, edits are `callout set` and `callout style`, «Сбросить» of the
//! style brings the look and keeps the text of «Вкл». The editor's snapshot of it and the path to
//! the same project as `devcast` are in dc-app (`inspector/callout_tests.rs`).

use std::collections::BTreeMap;

use dc_core::{OutputTime, Project, Step};
use dc_ui_kit::egui;
use dc_ui_kit::panel::{Cx, Groups};
use dc_ui_kit::strings::inspector as names;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use serde_json::{Value, json};

use crate::EventId;
use crate::panel;
use crate::tests::{add, project, run, style};

struct State {
    project: Project,
    defaults: Project,
    groups: BTreeMap<String, bool>,
    event: EventId,
    edits: Vec<Vec<Step>>,
}

/// One 10 s recording with «Вкл» co1, «Текст» co2 (2 s of typing) and «Выкл» co3.
fn events() -> Project {
    let mut p = project(&[10.0]);
    for event in [
        json!({"action": "on", "at": 1}),
        json!({"action": "text", "at": 2, "text": "Привет", "duration": 2}),
        json!({"action": "off", "at": 6, "duration": 1}),
    ] {
        run(&mut p, &add(event));
    }
    p
}

fn harness(project: Project, number: u32) -> Harness<'static, State> {
    let state = State {
        project,
        defaults: Project::default(),
        groups: BTreeMap::new(),
        event: EventId::from_number(number),
        edits: Vec::new(),
    };
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(340.0, 700.0))
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
                };
                panel::ui(ui, &mut cx, state.event);
                let edits = cx.edits;
                state.edits.extend(edits);
            },
            state,
        );
    harness.run();
    harness
}

fn step(op: &str, args: Value) -> Vec<Step> {
    let Value::Object(args) = args else {
        panic!("arguments are an object");
    };
    vec![Step::new(op, args)]
}

#[test]
fn the_header_tells_the_event() {
    let p = events();
    let text = panel::about(&p, EventId::from_number(2), "0:02.00");
    let expected = (
        names::CALLOUT_TEXT,
        "событие 0:02.00 · печать 2,0 с".to_owned(),
    );
    assert_eq!(text, Some(expected));
    assert_eq!(panel::about(&p, EventId::from_number(9), "0:00.00"), None);
}

#[test]
fn event_edits_are_callout_set() {
    let mut harness = harness(events(), 2);
    assert!(harness.query_by_label(names::CALLOUT_TEXT_HINT).is_some());
    harness
        .get_by_role_and_label(egui::Role::RadioButton, names::CALLOUT_HIDE)
        .click();
    harness.run();
    let off = step("callout set", json!({"id": "co2", "action": "off"}));
    assert_eq!(harness.state().edits, vec![off]);
}

#[test]
fn a_new_text_event_takes_the_text_of_on_or_a_word() {
    let mut harness = harness(events(), 1);
    harness
        .get_by_role_and_label(egui::Role::RadioButton, names::CALLOUT_TYPE)
        .click();
    harness.run();
    let word = json!({"id": "co1", "action": "text", "text": names::CALLOUT_NEW_TEXT});
    assert_eq!(harness.state().edits, vec![step("callout set", word)]);

    let mut p = events();
    run(&mut p, &style(json!({"text": "Готово"})));
    let mut harness = self::harness(p, 3);
    harness
        .get_by_role_and_label(egui::Role::RadioButton, names::CALLOUT_TYPE)
        .click();
    harness.run();
    let on = json!({"id": "co3", "action": "text", "text": "Готово"});
    assert_eq!(harness.state().edits, vec![step("callout set", on)]);
}

#[test]
fn style_edits_are_callout_style_and_its_reset_keeps_the_text() {
    let mut harness = harness(events(), 1);
    let reset = names::reset_group(names::CALLOUT_STYLE);
    assert!(harness.get_by_label(&reset).accesskit_node().is_disabled());
    harness
        .get_by_role_and_label(egui::Role::Switch, names::CALLOUT_GRADIENT)
        .click();
    harness.run();
    let on = step("callout style", json!({"gradient": true}));
    assert_eq!(harness.state().edits, vec![on]);

    let mut p = events();
    run(&mut p, &style(json!({"text": "Готово", "size": 1.5})));
    let mut harness = self::harness(p, 1);
    harness.get_by_label(&reset).click();
    harness.run();
    let look = json!({
        "background": "#14b8c4ff",
        "text_color": "#ffffffff",
        "size": 1.0,
        "gradient": false,
        "colors": "#14b8c4ff,#a855f7ff",
        "glow": 0.0,
    });
    assert_eq!(harness.state().edits, vec![step("callout style", look)]);
}
