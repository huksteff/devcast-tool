//! The captions panel (7.17): the cue list picks and seeks, the cue editor and the list buttons
//! are `captions set/split/merge/rm/clear`, the look is `captions style` — each edit one undo
//! step, the same project as `devcast`; a cue picked on the timeline opens the same editor.

use dc_core::{History, OutputTime, Project};
use dc_fx_captions::CueId;
use dc_ops::{Registry, Step};
use dc_ui_kit::strings::{captions as text, editor, inspector as names};
use eframe::egui;
use egui_kittest::kittest::Queryable;
use serde_json::json;

use super::tests::{H, cli, pick, project_json, undo};
use super::zoom_tests::{click, tall, type_in};
use crate::editor::editing_tests::{json, undone};
use crate::editor::selection::{Item, Selected};
use crate::editor::tests::project;
use crate::editor::view::Panel;

/// The editor on the captions tab.
pub(crate) fn captions_tab() -> H {
    let mut harness = tall();
    harness.state_mut().editor.view.panel = Panel::Captions;
    harness.run();
    harness
}

/// The test project after `steps`, each its own plan.
fn after(steps: &[Step]) -> Project {
    let registry = Registry::devcast().unwrap();
    let mut project = project();
    for step in steps {
        let mut history = History::default();
        dc_ops::apply(
            &registry,
            &mut project,
            &mut history,
            std::slice::from_ref(step),
            false,
        )
        .unwrap();
    }
    project
}

fn set(id: &str, field: &str, value: serde_json::Value) -> Step {
    let args = json!({"id": id, field: value});
    Step::new("captions set", args.as_object().unwrap().clone())
}

/// Clicks the line of the list at `at` saying `said`.
fn click_line(harness: &mut H, at: &str, said: &str) {
    let name = text::cue_line(at, said);
    click(harness, egui::Role::Button, &name);
}

/// Replaces the text of the field named `hint` with `typed` and presses Enter.
fn retype(harness: &mut H, hint: &str, typed: &str) {
    harness.get_by_label(hint).focus();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.get_by_label(hint).type_text(typed);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
}

#[test]
fn a_line_of_the_list_picks_the_cue_and_seeks_to_it() {
    let mut harness = captions_tab();
    let head = harness.get_by_role_and_label(egui::Role::Heading, editor::CAPTIONS);
    assert_eq!(head.value().as_deref(), Some("3 реплики"));
    click_line(&mut harness, "0:09.00", "Открываем страницу логина");
    let view = &harness.state().editor.view;
    let cue = Selected::Region(Item::of(CueId::from_number(2)));
    assert_eq!(view.selection.single(), Some(cue));
    assert_eq!(view.object, None, "the list stays open");
    assert_eq!(view.playhead, OutputTime::from_secs_f64(9.0));
    assert!(harness.query_by_label(text::CUE_TEXT).is_some());
    harness.snapshot("inspector_captions");
}

#[test]
fn cue_edits_are_captions_commands_one_step_each() {
    let mut harness = captions_tab();
    click_line(&mut harness, "0:09.00", "Открываем страницу логина");
    let said = "Открываем страницу входа в профиль";
    retype(&mut harness, text::CUE_TEXT, said);
    let mut steps = vec![set("s2", "text", json!(said))];
    assert_eq!(project_json(&harness), json(&after(&steps)));
    retype(&mut harness, text::CUE_START, "0:08.500");
    steps.push(set("s2", "from", json!("0:08.500")));
    assert_eq!(project_json(&harness), json(&after(&steps)));
    let at = harness.state().editor.view.playhead.micros() as f64 / 1e6;
    click(&mut harness, egui::Role::Button, text::SPLIT);
    let split = json!({"id": "s2", "at": at});
    steps.push(Step::new(
        "captions split",
        split.as_object().unwrap().clone(),
    ));
    assert_eq!(project_json(&harness), json(&after(&steps)));
    click(&mut harness, egui::Role::Button, text::MERGE);
    let merge = json!({"id": "s2"});
    steps.push(Step::new(
        "captions merge",
        merge.as_object().unwrap().clone(),
    ));
    assert_eq!(project_json(&harness), json(&after(&steps)));
    click(&mut harness, egui::Role::Button, names::DELETE);
    steps.push(Step::new("captions rm", merge.as_object().unwrap().clone()));
    assert_eq!(project_json(&harness), json(&after(&steps)));
    for done in (1..steps.len()).rev() {
        undo(&mut harness);
        let now = harness.state().editor.doc.project();
        assert_eq!(json(now), undone(&after(&steps[..done]), now), "one step");
    }
}

#[test]
fn clear_all_is_one_step_and_leaves_the_empty_card() {
    let mut harness = captions_tab();
    click(&mut harness, egui::Role::Button, text::CLEAR);
    assert_eq!(project_json(&harness), json(&cli(&["captions clear"])));
    assert!(harness.query_by_label(text::EMPTY).is_some());
    harness.snapshot("inspector_captions_empty");
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&project()));
}

#[test]
fn style_edits_are_captions_style_and_reset_is_one_step() {
    let mut harness = captions_tab();
    harness.get_by_label(text::STYLE).click();
    harness.run();
    harness.get_by_label(editor::FINE).click();
    harness.run();
    click(&mut harness, egui::Role::RadioButton, text::RISE);
    click(&mut harness, egui::Role::Switch, text::SHADOW);
    type_in(&mut harness, text::SIZE, 40.0);
    click(&mut harness, egui::Role::RadioButton, "2");
    type_in(&mut harness, text::WIDTH, 70.0);
    type_in(&mut harness, text::PLATE, 50.0);
    let lines = [
        "captions style --animation rise",
        "captions style --shadow false",
        "captions style --size 40",
        "captions style --rows 2",
        "captions style --width 70",
        "captions style --background 0.5",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    // Closed, so the snapshot shows both groups of the look.
    harness.get_by_label(text::RECOGNITION).click();
    harness.run();
    harness.get_by_label(text::CUES).click();
    harness.run();
    harness.snapshot("inspector_captions_style");
    click(&mut harness, egui::Role::Switch, text::SHOW);
    let mut shown = lines.to_vec();
    shown.push("captions style --show false");
    assert_eq!(project_json(&harness), json(&cli(&shown)));
    let reset = names::reset_group(text::STYLE);
    click(&mut harness, egui::Role::Button, &reset);
    let kept = [lines[3], lines[4], lines[5], "captions style --show false"];
    assert_eq!(project_json(&harness), json(&cli(&kept)));
    undo(&mut harness);
    assert_eq!(
        project_json(&harness),
        json(&cli(&shown)),
        "reset is one step"
    );
}

#[test]
fn a_cue_picked_on_the_timeline_opens_its_editor() {
    let mut harness = tall();
    let cue = Selected::Region(Item::of(CueId::from_number(1)));
    pick(&mut harness, Some(cue));
    let head = harness.get_by_role_and_label(egui::Role::Heading, names::CUE);
    assert_eq!(head.value().as_deref(), Some("0:01.00 – 0:08.00 · 7,0 с"));
    retype(&mut harness, text::CUE_END, "0:07.500");
    let steps = [set("s1", "to", json!("0:07.500"))];
    assert_eq!(project_json(&harness), json(&after(&steps)));
    harness.snapshot("inspector_cue");
}

#[test]
fn esc_in_a_cue_field_drops_the_draft() {
    let mut harness = captions_tab();
    click_line(&mut harness, "0:09.00", "Открываем страницу логина");
    let untouched = project_json(&harness);
    for hint in [text::CUE_TEXT, text::CUE_START] {
        let before = harness.get_by_label(hint).value();
        harness.get_by_label(hint).focus();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.get_by_label(hint).type_text("0:05.000 черновик");
        harness.run();
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert_eq!(
            project_json(&harness),
            untouched,
            "{hint}: Esc applies nothing"
        );
        assert_eq!(
            harness.get_by_label(hint).value(),
            before,
            "{hint}: the text is back"
        );
        let cue = Selected::Region(Item::of(CueId::from_number(2)));
        let view = &harness.state().editor.view;
        assert_eq!(
            view.selection.single(),
            Some(cue),
            "{hint}: the cue stays picked"
        );
    }
    click_line(&mut harness, "0:01.00", "Сегодня покажу вход");
    assert_eq!(
        project_json(&harness),
        untouched,
        "nothing left to apply later"
    );
}
