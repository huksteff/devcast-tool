//! Editing on the preview through the window: every gesture is one undo step and the same
//! project as `devcast` with the same command; the preview shows the draft while it goes on.

use dc_core::{OutputTime, Project};
use dc_fx_annotations::{AnnotId, Annotations};
use dc_fx_zoom::{Mode, Zoom, ZoomEffect, ZoomId};
use dc_ui_kit::strings::editor as text;
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, Rect, Vec2, pos2, vec2};
use egui_kittest::kittest::Queryable;

use crate::editor::Action;
use crate::editor::editing_tests::json;
use crate::editor::inspector::tests::{H, cli, pick, project_json, step, undo};
use crate::editor::selection::{Item, Selected};
use crate::editor::tests::harness;
use crate::editor::{SIZE, Transport};

/// Canvas of the test project, px.
pub(super) const CANVAS: Vec2 = vec2(2560.0, 1440.0);

pub(super) fn started() -> H {
    let mut harness = harness(SIZE);
    harness.run();
    harness
}

/// The output frame on screen.
pub(super) fn frame(harness: &H) -> Rect {
    harness.get_by_label(text::PREVIEW_FRAME).rect()
}

/// Canvas px `p` on screen (no camera: the tests have no GPU frame).
pub(super) fn on_screen(harness: &H, p: Pos2) -> Pos2 {
    let frame = frame(harness);
    frame.min + p.to_vec2() * (frame.width() / CANVAS.x)
}

/// Moves to `pos` and presses or releases there, `modifiers` held (they stay until changed: the
/// harness clock runs a quarter second per event, a click must not take long).
fn button(harness: &mut H, pos: Pos2, pressed: bool, modifiers: Modifiers) {
    harness.event(Event::ModifiersChanged(modifiers));
    harness.event(Event::PointerMoved(pos));
    let event = Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    };
    harness.event(event);
}

/// Presses at `from` and moves to `to` in steps, without releasing.
pub(super) fn hold(harness: &mut H, from: Pos2, to: Pos2, modifiers: Modifiers) {
    button(harness, from, true, modifiers);
    harness.run();
    for share in [0.25, 0.5, 0.75, 1.0] {
        let at = from + (to - from) * share;
        harness.event(Event::PointerMoved(at));
        harness.run();
    }
}

pub(super) fn release(harness: &mut H, at: Pos2, modifiers: Modifiers) {
    button(harness, at, false, modifiers);
    harness.event(Event::ModifiersChanged(Modifiers::NONE));
    harness.run();
    harness.run();
}

pub(super) fn drag(harness: &mut H, from: Pos2, to: Pos2, modifiers: Modifiers) {
    hold(harness, from, to, modifiers);
    release(harness, to, modifiers);
}

fn click(harness: &mut H, at: Pos2) {
    button(harness, at, true, Modifiers::NONE);
    button(harness, at, false, Modifiers::NONE);
    harness.run();
}

fn zoom(project: &Project, n: u32) -> Zoom {
    let id = ZoomId::from_number(n);
    let state = project.state::<ZoomEffect>().unwrap();
    state.zooms.into_iter().find(|z| z.id == id).unwrap()
}

fn annot_rect(project: &Project, n: u32) -> ([f64; 4], f64) {
    let id = AnnotId::from_number(n);
    let state = project.state::<Annotations>().unwrap();
    let a = state.annotations.into_iter().find(|a| a.id == id).unwrap();
    ([a.rect.x(), a.rect.y(), a.rect.w(), a.rect.h()], a.rotation)
}

/// The preview renders a project other than the document: the draft of a gesture.
pub(super) fn drafted(harness: &H) -> Option<Project> {
    let editor = &harness.state().editor;
    let (shown, _) = editor.preview.project();
    (*shown != *editor.doc.project()).then(|| (*shown).clone())
}

pub(super) fn at(harness: &mut H, secs: f64) {
    let time = OutputTime::from_secs_f64(secs);
    harness
        .state_mut()
        .editor
        .ask(Action::Transport(Transport::Scrub(time)));
    harness.run();
}

#[test]
fn dragging_the_zoom_frame_moves_the_focus_and_stands_the_camera() {
    let mut harness = started();
    pick(
        &mut harness,
        Some(Selected::Region(Item::of(ZoomId::from_number(1)))),
    );
    harness.snapshot("preview_zoom_frame");
    let centre = on_screen(&harness, pos2(1280.0, 720.0));
    let by = on_screen(&harness, pos2(256.0, 0.0)) - on_screen(&harness, Pos2::ZERO);
    hold(&mut harness, centre, centre + by, Modifiers::NONE);
    let draft = drafted(&harness).expect("the preview shows the draft");
    assert!((zoom(&draft, 1).focus.x - 0.6).abs() < 1e-3);
    release(&mut harness, centre + by, Modifiers::NONE);
    let z = zoom(harness.state().editor.doc.project(), 1);
    assert!((z.focus.x - 0.6).abs() < 1e-3 && z.focus.y == 0.5, "{z:?}");
    assert_eq!(z.mode, Mode::Fixed, "a following zoom stands on the point");
    let line = format!(
        "zoom set z1 --scale 1.5 --x {} --y 0.5 --mode fixed",
        z.focus.x
    );
    assert_eq!(project_json(&harness), json(&cli(&[&line])));
    assert!(
        drafted(&harness).is_none(),
        "the preview is back on the project"
    );
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&[])), "one step");
}

#[test]
fn a_corner_of_the_zoom_frame_sets_the_scale() {
    let mut harness = started();
    pick(
        &mut harness,
        Some(Selected::Region(Item::of(ZoomId::from_number(1)))),
    );
    // 1.5×: the frame is the middle two thirds; its bottom right corner goes to the centre.
    let corner = on_screen(&harness, pos2(2560.0 * 5.0 / 6.0, 1440.0 * 5.0 / 6.0));
    let to = on_screen(&harness, pos2(1280.0, 720.0));
    drag(&mut harness, corner, to, Modifiers::NONE);
    let z = zoom(harness.state().editor.doc.project(), 1);
    assert!((z.scale - 3.0).abs() < 0.01, "a third of the frame: {z:?}");
    assert_eq!(z.mode, Mode::Follow, "resizing keeps following");
    let line = format!(
        "zoom set z1 --scale {} --x {} --y {}",
        z.scale, z.focus.x, z.focus.y
    );
    assert_eq!(project_json(&harness), json(&cli(&[&line])));
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&[])), "one step");
}

#[test]
fn a_click_picks_the_topmost_annotation_and_nothing_drops_it() {
    let mut harness = started();
    // a3 (row 2) over a1 (row 1), both at 0.1, 0.1, 0.3 × 0.1 at 0:24.
    let moved = step("annot move a3 --at 23.5");
    harness.state_mut().editor.ask(Action::Apply(vec![moved]));
    harness.run();
    at(&mut harness, 24.0);
    let inside = on_screen(&harness, pos2(600.0, 200.0));
    click(&mut harness, inside);
    let picked = harness.state().editor.view.selection.single();
    let a3 = Selected::Region(Item::of(AnnotId::from_number(3)));
    assert_eq!(picked, Some(a3), "the upper row wins");
    harness.snapshot("preview_annotation_picked");
    let empty = on_screen(&harness, pos2(2000.0, 1200.0));
    click(&mut harness, empty);
    assert_eq!(harness.state().editor.view.selection.single(), None);
}

#[test]
fn dragging_an_annotation_moves_it_in_one_step() {
    let mut harness = started();
    at(&mut harness, 24.0);
    let from = on_screen(&harness, pos2(600.0, 200.0));
    let to = on_screen(&harness, pos2(700.0, 260.0));
    hold(&mut harness, from, to, Modifiers::NONE);
    let draft = drafted(&harness).expect("the preview shows the draft");
    assert!(annot_rect(&draft, 1).0[0] > 0.13);
    release(&mut harness, to, Modifiers::NONE);
    let ([x, y, ..], _) = annot_rect(harness.state().editor.doc.project(), 1);
    assert!(
        (x - 0.1391).abs() < 1e-3 && (y - 0.1417).abs() < 1e-3,
        "{x} {y}"
    );
    let line = format!("annot set a1 --x {x} --y {y}");
    assert_eq!(project_json(&harness), json(&cli(&[&line])));
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&[])), "one step");
}

#[test]
fn a_handle_at_a_key_resizes_that_key() {
    let mut harness = started();
    let key = step("annot key add a1 --at 24");
    harness.state_mut().editor.ask(Action::Apply(vec![key]));
    harness.run();
    at(&mut harness, 24.0);
    pick(
        &mut harness,
        Some(Selected::Region(Item::of(AnnotId::from_number(1)))),
    );
    // The right edge of 256..1024 × 144..288 pulled 200 px right.
    let edge = on_screen(&harness, pos2(1024.0, 216.0));
    let to = on_screen(&harness, pos2(1224.0, 230.0));
    drag(&mut harness, edge, to, Modifiers::NONE);
    let project = harness.state().editor.doc.project().clone();
    let state = project.state::<Annotations>().unwrap();
    let a1 = &state.annotations[0];
    assert_eq!(a1.rect.w(), 0.3, "the annotation keeps its box");
    let k = a1.keys.last().unwrap();
    assert!((k.rect.w() - 0.3781).abs() < 1e-3, "{:?}", k.rect);
    let line = format!(
        "annot set a1 --x {} --y {} --w {} --h {} --at 24",
        k.rect.x(),
        k.rect.y(),
        k.rect.w(),
        k.rect.h()
    );
    let lines = ["annot key add a1 --at 24", line.as_str()];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&lines[..1])), "one step");
}

#[test]
fn the_knob_turns_and_shift_snaps_to_15() {
    let mut harness = started();
    at(&mut harness, 24.0);
    pick(
        &mut harness,
        Some(Selected::Region(Item::of(AnnotId::from_number(1)))),
    );
    let frame = frame(&harness);
    let k = frame.width() / CANVAS.x;
    // The knob 22 points above the top middle (640, 144); to 30° right of the centre's vertical.
    let centre = on_screen(&harness, pos2(640.0, 216.0));
    let knob = on_screen(&harness, pos2(640.0, 144.0)) - vec2(0.0, 22.0);
    let reach = centre.y - knob.y;
    let turn = 32.0_f32.to_radians();
    let to = centre + vec2(turn.sin(), -turn.cos()) * reach;
    drag(&mut harness, knob, to, Modifiers::SHIFT);
    let (_, rotation) = annot_rect(harness.state().editor.doc.project(), 1);
    assert_eq!(rotation, 30.0, "k {k}");
    assert_eq!(
        project_json(&harness),
        json(&cli(&["annot set a1 --rotation 30"]))
    );
}

#[test]
fn moving_sticks_to_the_frame_edge_with_a_guide_unless_snapping_is_off() {
    let mut harness = started();
    at(&mut harness, 24.0);
    // The left edge from 256 to 6 px: within the pull of the frame's left edge.
    let from = on_screen(&harness, pos2(600.0, 200.0));
    let to = on_screen(&harness, pos2(350.0, 200.0));
    hold(&mut harness, from, to, Modifiers::NONE);
    harness.snapshot("preview_guides");
    release(&mut harness, to, Modifiers::NONE);
    let ([x, ..], _) = annot_rect(harness.state().editor.doc.project(), 1);
    assert_eq!(x, 0.0, "stuck to the edge");
    undo(&mut harness);
    harness.state_mut().editor.view.magnet = false;
    drag(&mut harness, from, to, Modifiers::NONE);
    let ([x, ..], _) = annot_rect(harness.state().editor.doc.project(), 1);
    assert!(x > 0.001, "snapping off: {x}");
    undo(&mut harness);
    drag(&mut harness, from, to, Modifiers::COMMAND);
    harness.state_mut().editor.view.magnet = true;
    let ([x, ..], _) = annot_rect(harness.state().editor.doc.project(), 1);
    assert!(x > 0.001, "Ctrl lets go too: {x}");
}

#[test]
fn esc_ends_a_drag_without_a_change() {
    let mut harness = started();
    at(&mut harness, 24.0);
    let from = on_screen(&harness, pos2(600.0, 200.0));
    let to = on_screen(&harness, pos2(900.0, 400.0));
    hold(&mut harness, from, to, Modifiers::NONE);
    assert!(drafted(&harness).is_some());
    harness.key_press(eframe::egui::Key::Escape);
    harness.run();
    release(&mut harness, to, Modifiers::NONE);
    assert!(drafted(&harness).is_none());
    assert_eq!(project_json(&harness), json(&cli(&[])));
}

#[test]
fn auto_keys_put_a_drag_into_a_new_key_at_the_playhead() {
    let mut harness = started();
    harness.state_mut().editor.view.auto_keys = true;
    at(&mut harness, 24.0);
    let from = on_screen(&harness, pos2(600.0, 200.0));
    let to = on_screen(&harness, pos2(700.0, 260.0));
    drag(&mut harness, from, to, Modifiers::NONE);
    let project = harness.state().editor.doc.project().clone();
    let state = project.state::<Annotations>().unwrap();
    let a1 = &state.annotations[0];
    assert_eq!(a1.rect.x(), 0.1, "the annotation keeps its box");
    let k = a1.keys.last().unwrap();
    let line = format!("annot set a1 --x {} --y {} --at 24", k.rect.x(), k.rect.y());
    assert_eq!(project_json(&harness), json(&cli(&[&line])));
    at(&mut harness, 25.0);
    let note = dc_ui_kit::strings::annotations::KEY_AUTO_NOTE;
    harness.get_by_label(note);
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&[])), "one step");
}
