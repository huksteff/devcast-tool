//! The regions of the effect rows in the editor window (7.9): click, Ctrl+click, the rubber
//! band, Delete, dragging regions and edges with snapping, the clipboard, Ctrl+A. Each gesture
//! gives the project `devcast` gives for the same commands, in one undo step.

use dc_core::{History, OutputTime, Project};
use dc_ops::{Registry, Step};
use dc_ui_kit::strings::timeline as text;
use eframe::egui::{Event, Key, Modifiers, Pos2, pos2};
use egui_kittest::kittest::Queryable;

use super::clips_tests::{
    H, button, move_to, press, project_json, started, undo_is_one_step, x_of,
};
use crate::editor::editing_tests::json;
use crate::editor::selection::{Item, Selected};
use crate::editor::tests::project;

/// The project `devcast` makes of the test project with each line (its words) as one step of a
/// plan: the CLI's parsing and apply.
pub(super) fn by_cli(lines: &[Vec<String>]) -> Project {
    let registry = Registry::devcast().unwrap();
    let steps = steps(lines);
    let mut project = project();
    dc_ops::apply(
        &registry,
        &mut project,
        &mut History::default(),
        &steps,
        false,
    )
    .unwrap();
    project
}

/// The steps of the lines (their words) as `devcast` parses them.
pub(super) fn steps(lines: &[Vec<String>]) -> Vec<Step> {
    let registry = Registry::devcast().unwrap();
    lines
        .iter()
        .map(|words| {
            let (info, taken) = registry.resolve(words).unwrap();
            Step::new(
                info.name,
                dc_ops::parse_args(info, &words[taken..]).unwrap(),
            )
        })
        .collect()
}

pub(super) fn words(line: &str) -> Vec<String> {
    line.split(' ').map(str::to_owned).collect()
}

/// The middle of the region named `label` (start `secs`) in its row.
fn region(harness: &H, label: &str) -> Pos2 {
    harness.get_by_label(label).rect().center()
}

/// Output second `secs` on the row headed `row`.
pub(super) fn on_row(harness: &H, row: &str, secs: f64) -> Pos2 {
    let y = harness.get_by_label(row).rect().center().y;
    pos2(x_of(harness, secs), y)
}

/// A drag with the primary button from `from` to `to` in a few steps, released there.
pub(super) fn drag(harness: &mut H, from: Pos2, to: Pos2, modifiers: Modifiers) {
    button(harness, from, true, modifiers);
    harness.run();
    for share in [0.25, 0.5, 0.75, 1.0] {
        move_to(harness, from + (to - from) * share);
    }
    button(harness, to, false, modifiers);
    harness.run();
}

pub(super) fn click(harness: &mut H, at: Pos2, modifiers: Modifiers) {
    harness.event(Event::ModifiersChanged(modifiers));
    button(harness, at, true, modifiers);
    button(harness, at, false, modifiers);
    harness.run();
    harness.event(Event::ModifiersChanged(Modifiers::NONE));
    harness.run();
}

pub(super) fn picked(harness: &H) -> Vec<Item> {
    harness.state().editor.view.selection.regions().collect()
}

pub(super) fn zoom(n: u32) -> Item {
    Item::Zoom(dc_core::Id::from_number(n))
}

pub(super) fn annotation(n: u32) -> Item {
    Item::Annotation(dc_core::Id::from_number(n))
}

pub(super) fn at(harness: &mut H, secs: f64) {
    harness.state_mut().editor.view.playhead = OutputTime::from_secs_f64(secs);
    harness.run();
}

#[test]
fn click_ctrl_click_and_delete_remove_the_picked_regions() {
    let mut harness = started();
    click_on(&mut harness, "2×", Modifiers::NONE);
    assert_eq!(picked(&harness), [zoom(2)]);
    click_on(&mut harness, text::SHAPE, Modifiers::COMMAND);
    assert_eq!(picked(&harness), [zoom(2), annotation(3)]);
    click_on(&mut harness, "1,5×", Modifiers::COMMAND);
    click_on(&mut harness, "1,5×", Modifiers::COMMAND);
    assert_eq!(
        picked(&harness),
        [zoom(2), annotation(3)],
        "Ctrl+click takes out"
    );
    press(&mut harness, Key::Delete, Modifiers::NONE);
    let expected = by_cli(&[words("zoom rm z2"), words("annot rm a3")]);
    assert_eq!(project_json(&harness), json(&expected));
    assert!(picked(&harness).is_empty());
    undo_is_one_step(&mut harness);
}

#[test]
fn a_dragged_zoom_sticks_to_the_playhead() {
    let mut harness = started();
    at(&mut harness, 20.0);
    let from = region(&harness, "2×");
    let to = pos2(from.x + x_of(&harness, 22.1) - x_of(&harness, 15.0), from.y);
    button(&harness, from, true, Modifiers::NONE);
    harness.run();
    for share in [0.25, 0.5, 0.75, 1.0] {
        move_to(&mut harness, from + (to - from) * share);
    }
    harness.snapshot("editor_regions_snap");
    button(&harness, to, false, Modifiers::NONE);
    harness.run();
    let expected = by_cli(&[words("zoom set z2 --from 20.000000 --to 24.000000")]);
    assert_eq!(project_json(&harness), json(&expected));
    undo_is_one_step(&mut harness);
}

#[test]
fn esc_puts_dragged_regions_back() {
    let mut harness = started();
    let from = region(&harness, "2×");
    button(&harness, from, true, Modifiers::NONE);
    harness.run();
    move_to(&mut harness, from + eframe::egui::vec2(40.0, 0.0));
    move_to(&mut harness, from + eframe::egui::vec2(80.0, 0.0));
    press(&mut harness, Key::Escape, Modifiers::NONE);
    let to = from + eframe::egui::vec2(80.0, 0.0);
    button(&harness, to, false, Modifiers::NONE);
    harness.run();
    assert_eq!(project_json(&harness), json(&project()));
}

#[test]
fn an_annotation_edge_sticks_to_a_zoom_edge() {
    let mut harness = started();
    let row = text::annotations(2);
    let from = on_row(&harness, &row, 14.0) - eframe::egui::vec2(2.0, 0.0);
    let to = on_row(&harness, &row, 17.1);
    button(&harness, from, true, Modifiers::NONE);
    harness.run();
    for share in [0.25, 0.5, 0.75, 1.0] {
        move_to(&mut harness, from + (to - from) * share);
    }
    harness.snapshot("editor_regions_edge");
    button(&harness, to, false, Modifiers::NONE);
    harness.run();
    let expected = by_cli(&[words("annot set a3 --to 17.000000")]);
    assert_eq!(project_json(&harness), json(&expected));
}

#[test]
fn an_annotation_dragged_to_another_row_changes_track() {
    let mut harness = started();
    let from = region(&harness, text::SHAPE);
    let up = harness
        .get_by_label(&text::annotations(1))
        .rect()
        .center()
        .y;
    drag(&mut harness, from, pos2(from.x, up), Modifiers::NONE);
    let expected = by_cli(&[words("annot move a3 --at 10.000000 --track 0")]);
    assert_eq!(project_json(&harness), json(&expected));
}

#[test]
fn a_speed_band_edge_takes_more_of_the_clip() {
    let mut harness = started();
    at(&mut harness, 30.0);
    let clips = harness.get_by_label(text::CLIPS).rect();
    let y = clips.bottom() - 11.0;
    let from = pos2(x_of(&harness, 27.0) - 2.0, y);
    let to = pos2(x_of(&harness, 30.2), y);
    drag(&mut harness, from, to, Modifiers::NONE);
    let expected = by_cli(&[words("speed 27.000000-30.000000 4")]);
    assert_eq!(project_json(&harness), json(&expected));
}

#[test]
fn a_refused_move_is_a_toast_and_changes_nothing() {
    let mut harness = started();
    harness.state_mut().errors = Some(Vec::new());
    at(&mut harness, 40.0);
    let from = region(&harness, "Теперь настройки профиля");
    let to = pos2(from.x + x_of(&harness, 40.1) - x_of(&harness, 54.0), from.y);
    drag(&mut harness, from, to, Modifiers::NONE);
    let errors = harness.state().errors.clone().unwrap_or_default();
    assert!(
        errors.iter().any(|e| e.contains("не запись реплики")),
        "{errors:?}"
    );
    assert_eq!(project_json(&harness), json(&project()));
}

#[test]
fn a_rubber_band_picks_and_the_clipboard_pastes_at_the_playhead() {
    let mut harness = started();
    let from = on_row(&harness, text::ZOOMS, 3.0);
    let to = on_row(&harness, &text::annotations(2), 16.0);
    button(&harness, from, true, Modifiers::NONE);
    harness.run();
    for share in [0.25, 0.5, 0.75, 1.0] {
        move_to(&mut harness, from + (to - from) * share);
    }
    harness.snapshot("editor_regions_band");
    button(&harness, to, false, Modifiers::NONE);
    harness.run();
    assert_eq!(picked(&harness), [zoom(1), zoom(2), annotation(3)]);
    harness.snapshot("editor_regions_picked");
    press(&mut harness, Key::C, Modifiers::COMMAND);
    at(&mut harness, 60.0);
    press(&mut harness, Key::V, Modifiers::COMMAND);
    let shape = harness_item(&project(), 2);
    let expected = by_cli(&[
        words("zoom add --at 60.000000 --duration 7 --scale 1.5 --mode follow --x 0.5 --y 0.5"),
        vec![
            "annot".into(),
            "paste".into(),
            "--item".into(),
            shape,
            "--at".into(),
            "65.000000".into(),
            "--duration".into(),
            "4".into(),
        ],
        words("zoom add --at 68.000000 --duration 4 --scale 2 --mode follow --x 0.5 --y 0.5"),
    ]);
    assert_eq!(project_json(&harness), json(&expected));
    assert_eq!(
        picked(&harness),
        [zoom(4), zoom(5), annotation(4)],
        "the copies are picked"
    );
    undo_is_one_step(&mut harness);
}

/// Annotation number `index` of the test project as JSON text.
fn harness_item(project: &Project, index: usize) -> String {
    let state = project.state::<dc_fx_annotations::Annotations>().unwrap();
    serde_json::to_string(&state.annotations[index]).unwrap()
}

#[test]
fn cut_and_paste_move_a_zoom_through_the_clipboard() {
    let mut harness = started();
    click_on(&mut harness, "2×", Modifiers::NONE);
    press(&mut harness, Key::X, Modifiers::COMMAND);
    assert_eq!(
        project_json(&harness),
        json(&by_cli(&[words("zoom rm z2")]))
    );
    at(&mut harness, 30.0);
    press(&mut harness, Key::V, Modifiers::COMMAND);
    let expected = by_cli(&[
        words("zoom rm z2"),
        words("zoom add --at 30.000000 --duration 4 --scale 2 --mode follow --x 0.5 --y 0.5"),
    ]);
    assert_eq!(project_json(&harness), json(&expected));
}

#[test]
fn ctrl_a_picks_every_region_of_the_picked_kind_or_all_zooms() {
    let mut harness = started();
    press(&mut harness, Key::A, Modifiers::COMMAND);
    assert_eq!(picked(&harness), [zoom(1), zoom(2), zoom(3)]);
    click_on(&mut harness, text::TEXT, Modifiers::NONE);
    press(&mut harness, Key::A, Modifiers::COMMAND);
    assert_eq!(
        picked(&harness),
        [annotation(1), annotation(2), annotation(3)],
        "all tracks of annotations"
    );
    let empty = on_row(&harness, text::CLIPS, 49.5);
    click(&mut harness, empty, Modifiers::NONE);
    assert!(
        picked(&harness).is_empty(),
        "a click on an empty lane drops it"
    );
    assert!(
        !harness
            .state()
            .editor
            .view
            .selection
            .contains(Selected::Region(zoom(1)))
    );
}

fn click_on(harness: &mut H, label: &str, modifiers: Modifiers) {
    let at = region(harness, label);
    click(harness, at, modifiers);
}
