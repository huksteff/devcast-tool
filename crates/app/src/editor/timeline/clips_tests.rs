//! The main track in the editor window (7.8): picking a clip, Delete, C, the blade, dragging a
//! clip to another place, the range on the ruler with Shift+Delete and its speed. Each gesture
//! gives the project `devcast` gives for the same command, in one undo step.

use dc_core::time::Output;
use dc_core::{ClipId, OutputTime, Range, TimeDelta};
use dc_ui_kit::strings::{keys as key_names, timeline as text};
use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::clips::slot;
use super::viewport::Lanes;
use crate::editor::editing_tests::{by_cli, json};
use crate::editor::montage::range_arg;
use crate::editor::selection::Selected;
use crate::editor::tests::{State, harness, project};
use crate::editor::view::{RangeMenu, Tool};
use crate::editor::{SIZE, Way};

type H = Harness<'static, State>;

fn started() -> H {
    let mut harness = harness(SIZE);
    harness.run();
    harness
}

fn lanes(harness: &H) -> Lanes {
    let ruler = harness.get_by_label(text::RULER).rect();
    Lanes {
        left: ruler.left(),
        width: ruler.width(),
        duration: TimeDelta::from_secs_f64(105.0),
        fps: 30.0,
    }
}

fn x_of(harness: &H, secs: f64) -> f32 {
    let lanes = lanes(harness);
    let view = &harness.state().editor.view.timeline;
    view.scale(&lanes).x(OutputTime::from_secs_f64(secs))
}

/// The point of output second `secs` on the clips row.
fn on_clips(harness: &H, secs: f64) -> Pos2 {
    let y = harness.get_by_label(text::CLIPS).rect().center().y;
    pos2(x_of(harness, secs), y)
}

fn on_ruler(harness: &H, secs: f64) -> Pos2 {
    let y = harness.get_by_label(text::RULER).rect().center().y;
    pos2(x_of(harness, secs), y)
}

fn button(harness: &H, pos: Pos2, pressed: bool, modifiers: Modifiers) {
    harness.event(Event::PointerMoved(pos));
    harness.event(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    });
}

fn move_to(harness: &mut H, pos: Pos2) {
    harness.event(Event::PointerMoved(pos));
    harness.run();
}

/// A key by its physical key, as egui-winit sends it.
fn press(harness: &mut H, key: Key, modifiers: Modifiers) {
    harness.event(Event::ModifiersChanged(modifiers));
    harness.event(Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    });
    harness.event(Event::ModifiersChanged(Modifiers::NONE));
    harness.run();
}

fn project_json(harness: &H) -> serde_json::Value {
    json(harness.state().editor.doc.project())
}

/// Undo takes the gesture back in one step.
fn undo_is_one_step(harness: &mut H) {
    harness
        .state_mut()
        .editor
        .doc
        .travel(Way::Undo, std::time::Instant::now());
    assert_eq!(json(harness.state().editor.doc.project()), json(&project()));
}

/// Shift+drag on the ruler from second `from` to `to`; the range picked.
fn pick_range(harness: &mut H, from: f64, to: f64) -> Range<Output> {
    harness.event(Event::ModifiersChanged(Modifiers::SHIFT));
    let start = on_ruler(harness, from);
    button(harness, start, true, Modifiers::SHIFT);
    move_to(harness, on_ruler(harness, (from + to) / 2.0));
    move_to(harness, on_ruler(harness, to));
    button(harness, on_ruler(harness, to), false, Modifiers::SHIFT);
    harness.event(Event::ModifiersChanged(Modifiers::NONE));
    harness.run();
    harness.state().editor.view.range.unwrap()
}

fn c(n: u32) -> ClipId {
    ClipId::from_number(n)
}

#[test]
fn a_click_picks_a_clip_and_delete_removes_it() {
    let mut harness = started();
    harness.get_by_label("Клип 2").click();
    harness.run();
    let view = &harness.state().editor.view;
    assert!(view.selection.contains(Selected::Clip(c(2))));
    press(&mut harness, Key::Delete, Modifiers::NONE);
    assert_eq!(project_json(&harness), json(&by_cli("clip rm c2")));
    assert!(
        !harness
            .state()
            .editor
            .view
            .selection
            .contains(Selected::Clip(c(2)))
    );
    undo_is_one_step(&mut harness);
}

#[test]
fn c_cuts_the_clip_at_the_playhead() {
    let mut harness = started();
    harness.state_mut().editor.view.playhead = OutputTime::from_secs_f64(30.0);
    press(&mut harness, Key::C, Modifiers::NONE);
    assert_eq!(project_json(&harness), json(&by_cli("clip split --at 30")));
    assert_eq!(harness.state().editor.doc.project().clips.len(), 4);
}

#[test]
fn the_blade_sticks_to_the_playhead_and_cuts_there() {
    let mut harness = started();
    harness.state_mut().editor.view.playhead = OutputTime::from_secs_f64(70.0);
    press(&mut harness, Key::B, Modifiers::NONE);
    assert_eq!(harness.state().editor.view.tool, Tool::Blade);
    let near = on_clips(&harness, 70.0) + eframe::egui::vec2(5.0, 0.0);
    move_to(&mut harness, near);
    harness.run();
    harness.snapshot("editor_blade");
    button(&harness, near, true, Modifiers::NONE);
    button(&harness, near, false, Modifiers::NONE);
    harness.run();
    assert_eq!(project_json(&harness), json(&by_cli("clip split --at 70")));
    press(&mut harness, Key::Escape, Modifiers::NONE);
    assert_eq!(harness.state().editor.view.tool, Tool::Select);
}

#[test]
fn the_blade_cuts_under_the_pointer_without_the_magnet() {
    let mut harness = started();
    harness.state_mut().editor.view.tool = Tool::Blade;
    harness.state_mut().editor.view.magnet = false;
    harness.run();
    let at = on_clips(&harness, 95.0);
    button(&harness, at, true, Modifiers::NONE);
    button(&harness, at, false, Modifiers::NONE);
    harness.run();
    let lanes = lanes(&harness);
    let t = harness
        .state()
        .editor
        .view
        .timeline
        .scale(&lanes)
        .time(at.x);
    assert!((t.as_secs_f64() - 95.0).abs() < 0.1, "{t}");
    let line = format!("clip split --at {}", crate::editor::montage::arg(t));
    assert_eq!(project_json(&harness), json(&by_cli(&line)));
}

#[test]
fn a_dragged_clip_shows_a_ghost_and_moves_by_its_middle() {
    let mut harness = started();
    let grab = on_clips(&harness, 96.0);
    button(&harness, grab, true, Modifiers::NONE);
    for secs in [90.0, 70.0, 55.0, 45.0] {
        let at = on_clips(&harness, secs);
        move_to(&mut harness, at);
    }
    assert!(harness.state().editor.view.drag.is_some());
    harness.snapshot("editor_clip_drag");
    let at = on_clips(&harness, 45.0);
    button(&harness, at, false, Modifiers::NONE);
    harness.run();
    assert_eq!(harness.state().editor.view.drag, None);
    assert_eq!(project_json(&harness), json(&by_cli("clip move c3 --to 2")));
    let view = &harness.state().editor.view;
    assert!(view.selection.contains(Selected::Clip(c(3))));
    undo_is_one_step(&mut harness);
}

#[test]
fn esc_drops_a_dragged_clip_where_it_was() {
    let mut harness = started();
    let at = on_clips(&harness, 96.0);
    button(&harness, at, true, Modifiers::NONE);
    let at = on_clips(&harness, 60.0);
    move_to(&mut harness, at);
    let at = on_clips(&harness, 10.0);
    move_to(&mut harness, at);
    press(&mut harness, Key::Escape, Modifiers::NONE);
    let at = on_clips(&harness, 10.0);
    button(&harness, at, false, Modifiers::NONE);
    harness.run();
    assert_eq!(project_json(&harness), json(&project()));
}

#[test]
fn the_middle_of_the_ghost_chooses_the_place() {
    let r = |a: f64, b: f64| {
        Range::new(OutputTime::from_secs_f64(a), OutputTime::from_secs_f64(b)).unwrap()
    };
    let others = [r(0.0, 49.0), r(49.0, 87.0)];
    let at = OutputTime::from_secs_f64;
    assert_eq!(slot(&others, at(10.0)), 0);
    assert_eq!(slot(&others, at(30.0)), 1);
    assert_eq!(slot(&others, at(70.0)), 2);
    assert_eq!(slot(&[], at(70.0)), 0);
}

#[test]
fn shift_drag_on_the_ruler_picks_a_range_and_shift_delete_cuts_it() {
    let mut harness = started();
    let range = pick_range(&mut harness, 10.0, 15.0);
    assert!(
        (range.start().as_secs_f64() - 10.0).abs() < 0.1,
        "{range:?}"
    );
    assert!((range.end().as_secs_f64() - 15.0).abs() < 0.1, "{range:?}");
    let playhead = harness.state().editor.view.playhead.as_secs_f64();
    assert!(
        (playhead - 33.4).abs() < 1e-6,
        "the ruler did not move the playhead"
    );
    harness.snapshot("editor_range");
    press(&mut harness, Key::Delete, Modifiers::SHIFT);
    let line = format!("cut {}", range_arg(range));
    assert_eq!(project_json(&harness), json(&by_cli(&line)));
    assert_eq!(harness.state().editor.view.range, None);
    undo_is_one_step(&mut harness);
}

#[test]
fn r_opens_the_speed_of_the_range_and_a_preset_sets_it() {
    let mut harness = started();
    let range = pick_range(&mut harness, 60.0, 70.0);
    press(&mut harness, Key::R, Modifiers::NONE);
    harness.run();
    assert!(matches!(
        harness.state().editor.view.menu,
        Some(RangeMenu::Speed { .. })
    ));
    harness.snapshot("editor_range_speed");
    harness.get_by_label("4×").click();
    harness.run();
    let line = format!("speed {} 4", range_arg(range));
    assert_eq!(project_json(&harness), json(&by_cli(&line)));
    assert_eq!(harness.state().editor.view.menu, None);
}

#[test]
fn the_range_menu_cuts_the_range() {
    let mut harness = started();
    let range = pick_range(&mut harness, 60.0, 70.0);
    harness.get_by_label(text::RULER).hover();
    let inside = on_ruler(&harness, 65.0);
    harness.event(Event::PointerMoved(inside));
    for pressed in [true, false] {
        harness.event(Event::PointerButton {
            pos: inside,
            button: PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run();
    harness.run();
    assert_eq!(harness.state().editor.view.menu, Some(RangeMenu::Actions));
    harness.get_by_label(key_names::CUT_RANGE).click();
    harness.run();
    let line = format!("cut {}", range_arg(range));
    assert_eq!(project_json(&harness), json(&by_cli(&line)));
}

#[test]
fn a_click_on_a_speed_band_opens_its_speed() {
    let mut harness = started();
    harness.get_by_label(&text::speed_band(4.0)).click();
    harness.run();
    let band = harness.state().editor.tracks.speeds[0].region.range;
    let view = &harness.state().editor.view;
    assert_eq!(view.menu, Some(RangeMenu::Speed { own: 4.0 }));
    assert_eq!(view.range, Some(band));
    harness.run();
    harness.get_by_label("1×").click();
    harness.run();
    let line = format!("speed {} 1", range_arg(band));
    let cli = by_cli(&line);
    assert_eq!(project_json(&harness), json(&cli));
    assert!(cli.clips[0].speeds.is_empty(), "1× resets the speed");
}
