//! Key markers (7.12): the ◆ of the keys of «Текст» (a1: output 23–29 s, its middle in the 4×
//! range), picking, Delete, moving a key with snapping and inside the annotation, hiding. Each
//! change gives the project `devcast` gives for the same commands, in one undo step.

use dc_core::OutputTime;
use dc_ui_kit::strings::timeline as text;
use eframe::egui::{Key, Modifiers, Pos2};
use egui_kittest::kittest::Queryable;

use super::clips_tests::{H, press, project_json, started};
use super::regions_tests::{annotation, by_cli, click, drag, on_row, picked, words};
use super::scale::timecode;
use crate::editor::editing_tests::json;
use crate::editor::selection::Selected;
use crate::editor::{Action, Way};

/// Keys of a1 at output 24 s (source 29 s), 26 s (source 34 s, in 4×) and 28 s.
const KEYS: [&str; 3] = [
    "annot key add a1 --at 24",
    "annot key add a1 --at 26",
    "annot key add a1 --at 28",
];

fn lines(extra: &[&str]) -> Vec<Vec<String>> {
    KEYS.iter().chain(extra).map(|l| words(l)).collect()
}

/// The editor with the keys of [`KEYS`] added as `devcast` adds them.
fn keyed() -> H {
    let mut harness = started();
    let steps = super::regions_tests::steps(&lines(&[]));
    harness.state_mut().editor.ask(Action::Apply(steps));
    harness.run();
    harness.run();
    harness
}

fn marker(harness: &H, secs: f64) -> Pos2 {
    let name = text::key_marker(&timecode(OutputTime::from_secs_f64(secs)));
    harness.get_by_label(&name).rect().center()
}

fn key(secs: f64) -> Selected {
    Selected::Key {
        id: dc_core::Id::from_number(1),
        at: OutputTime::from_secs_f64(secs),
    }
}

fn selected(harness: &H) -> bool {
    !harness
        .state()
        .editor
        .view
        .selection
        .keys()
        .collect::<Vec<_>>()
        .is_empty()
}

/// The project after `extra`, and one undo back to the keys alone.
fn expect_then_undo(harness: &mut H, extra: &[&str]) {
    assert_eq!(project_json(harness), json(&by_cli(&lines(extra))));
    let now = std::time::Instant::now();
    harness.state_mut().editor.doc.travel(Way::Undo, now);
    assert_eq!(project_json(harness), json(&by_cli(&lines(&[]))));
}

#[test]
fn markers_stand_at_the_keys_through_the_speed_range() {
    let harness = keyed();
    let rows = &harness.state().editor.tracks.rows;
    let mut region = rows.iter().flat_map(|t| &t.regions);
    let a1 = region.find(|r| r.item == Some(annotation(1))).unwrap();
    let secs: Vec<f64> = a1.keys.iter().map(|k| k.as_secs_f64()).collect();
    assert_eq!(secs, [24.0, 26.0, 28.0]);
    for at in [24.0, 26.0, 28.0] {
        let x = marker(&harness, at).x;
        assert!((x - on_row(&harness, &text::annotations(1), at).x).abs() < 0.5);
    }
}

#[test]
fn a_click_picks_a_key_and_delete_removes_the_picked_keys() {
    let mut harness = keyed();
    let at = marker(&harness, 26.0);
    click(&mut harness, at, Modifiers::NONE);
    let selection = &harness.state().editor.view.selection;
    assert!(selection.contains(key(26.0)) && selection.len() == 1);
    assert!(picked(&harness).is_empty(), "not the annotation");
    let playhead = |h: &H| h.state().editor.view.playhead;
    assert_eq!(
        playhead(&harness),
        OutputTime::from_secs_f64(26.0),
        "the playhead goes to the key"
    );
    harness.snapshot("editor_key_markers");
    let at = marker(&harness, 28.0);
    click(&mut harness, at, Modifiers::COMMAND);
    assert_eq!(harness.state().editor.view.selection.len(), 2);
    assert_eq!(
        playhead(&harness),
        OutputTime::from_secs_f64(26.0),
        "Ctrl+click leaves the playhead"
    );
    press(&mut harness, Key::Delete, Modifiers::NONE);
    let rm = ["annot key rm a1 --at 26", "annot key rm a1 --at 28"];
    assert!(!selected(&harness), "removed keys drop out");
    expect_then_undo(&mut harness, &rm);
}

#[test]
fn a_dragged_key_sticks_to_the_start_of_its_annotation() {
    let mut harness = keyed();
    let from = marker(&harness, 24.0);
    let to = on_row(&harness, &text::annotations(1), 23.25);
    drag(&mut harness, from, to, Modifiers::NONE);
    expect_then_undo(&mut harness, &["annot key move a1 --at 24 --to 23"]);
    assert!(harness.state().editor.view.selection.contains(key(23.0)));
}

#[test]
fn a_key_stays_inside_its_annotation() {
    let mut harness = keyed();
    let from = marker(&harness, 28.0);
    let to = on_row(&harness, &text::annotations(1), 36.0);
    super::clips_tests::button(&harness, from, true, Modifiers::NONE);
    harness.run();
    super::clips_tests::move_to(&mut harness, from + (to - from) * 0.5);
    super::clips_tests::move_to(&mut harness, to);
    harness.snapshot("editor_key_drag");
    super::clips_tests::button(&harness, to, false, Modifiers::NONE);
    harness.run();
    expect_then_undo(&mut harness, &["annot key move a1 --at 28 --to 29"]);
}

#[test]
fn hidden_markers_are_not_there_and_the_click_picks_the_annotation() {
    let mut harness = keyed();
    let at = marker(&harness, 26.0);
    harness.state_mut().editor.view.markers = false;
    harness.run();
    let name = text::key_marker(&timecode(OutputTime::from_secs_f64(26.0)));
    assert!(harness.query_by_label(&name).is_none());
    click(&mut harness, at, Modifiers::NONE);
    assert_eq!(picked(&harness), [annotation(1)]);
    assert!(!selected(&harness));
}
