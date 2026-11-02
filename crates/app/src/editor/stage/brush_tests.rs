//! The brush on the preview: a stroke is one `drawing add`, the same project as `devcast`, one
//! undo step; Shift erases with a circle as big as the stroke it leaves, Alt draws a line, a key
//! at the playhead takes the stroke.

use dc_core::{PixelSize, Project};
use dc_fx_annotations::{AnnotId, Annotation, Annotations, Placement};
use dc_fx_drawing::{DrawingBody, Stroke, StrokeTool};
use dc_render::{BLUE, Camera};
use eframe::egui::{Event, Modifiers, pos2};

use super::brush::diameter;
use super::geom::Mapping;
use super::tests::{at, drafted, drag, frame, hold, on_screen, release, started};
use crate::editor::Action;
use crate::editor::editing_tests::json;
use crate::editor::inspector::tests::{H, cli, pick, project_json, step, undo};
use crate::editor::selection::{Item, Selected};

/// A drawing over the frame's middle half at 0:23.5–0:25.5, on top.
const ADD: &str = "annot add drawing --at 23.5 --duration 2 --track 3 --rect 0.25,0.25,0.5,0.5";

fn last(project: &Project) -> Annotation {
    let state = project.state::<Annotations>().unwrap();
    state.annotations.into_iter().last().unwrap()
}

fn strokes(harness: &H) -> Vec<Stroke> {
    let body: DrawingBody = last(harness.state().editor.doc.project()).body().unwrap();
    body.strokes
}

/// The test project with the drawing, picked, the playhead at 0:24; its id.
fn drawing(harness: &mut H) -> AnnotId {
    harness
        .state_mut()
        .editor
        .ask(Action::Apply(vec![step(ADD)]));
    harness.run();
    let id = last(harness.state().editor.doc.project()).id;
    at(harness, 24.0);
    pick(harness, Some(Selected::Region(Item::Annotation(id))));
    id
}

/// `drawing add` of `stroke` as typed in a shell.
fn line(id: AnnotId, stroke: &Stroke, extra: &str) -> String {
    let points: Vec<String> = stroke.points.iter().flatten().map(f64::to_string).collect();
    let paint = match stroke.tool {
        StrokeTool::Pencil => format!("--color {}", stroke.color),
        StrokeTool::Eraser => "--tool eraser".to_owned(),
    };
    format!(
        "drawing add {id} --points {} --width {} {paint}{extra}",
        points.join(","),
        stroke.width
    )
}

#[test]
fn a_drag_draws_a_pencil_stroke_in_one_step() {
    let mut harness = started();
    let id = drawing(&mut harness);
    let from = on_screen(&harness, pos2(800.0, 500.0));
    let to = on_screen(&harness, pos2(1500.0, 900.0));
    hold(&mut harness, from, to, Modifiers::NONE);
    assert!(drafted(&harness).is_some(), "the preview shows the stroke");
    harness.snapshot("preview_brush");
    release(&mut harness, to, Modifiers::NONE);
    let drawn = strokes(&harness);
    assert_eq!(drawn.len(), 1);
    let stroke = &drawn[0];
    assert_eq!(
        (stroke.tool, stroke.color, stroke.width),
        (StrokeTool::Pencil, BLUE, 4.0)
    );
    assert!(stroke.points.len() >= 3, "{:?}", stroke.points);
    let [x, y] = stroke.points[0];
    assert!(
        (x - 0.125).abs() < 2e-3 && (y - 0.1944).abs() < 2e-3,
        "{x} {y}"
    );
    let lines = [ADD, &line(id, stroke, "")];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    assert!(drafted(&harness).is_none());
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&[ADD])), "one step");
}

#[test]
fn shift_erases_with_the_circle_as_wide_as_its_stroke() {
    let mut harness = started();
    let id = drawing(&mut harness);
    let a = on_screen(&harness, pos2(800.0, 500.0));
    let b = on_screen(&harness, pos2(1500.0, 900.0));
    drag(&mut harness, a, b, Modifiers::NONE);
    drag(&mut harness, b, a, Modifiers::SHIFT);
    let drawn = strokes(&harness);
    assert_eq!(drawn[1].tool, StrokeTool::Eraser);
    assert_eq!(drawn[1].width, dc_fx_drawing::ERASER_WIDTH);
    let lines = [ADD, &line(id, &drawn[0], ""), &line(id, &drawn[1], "")];
    assert_eq!(project_json(&harness), json(&cli(&lines)));

    let canvas = PixelSize {
        width: 2560,
        height: 1440,
    };
    let map = Mapping::new(frame(&harness), canvas, Camera::IDENTITY);
    let annotation = last(harness.state().editor.doc.project());
    let body: DrawingBody = annotation.body().unwrap();
    let scene = dc_fx_drawing::evaluate(&body, &Placement::of(&annotation, canvas)).unwrap();
    let on_screen = scene.strokes[1].width * map.k;
    let circle = diameter(drawn[1].width, map);
    assert!((circle - on_screen).abs() < 1e-3, "{circle} vs {on_screen}");

    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&lines[..2])), "one step");
}

#[test]
fn alt_draws_a_straight_line() {
    let mut harness = started();
    drawing(&mut harness);
    let from = on_screen(&harness, pos2(800.0, 500.0));
    let to = on_screen(&harness, pos2(1440.0, 720.0));
    drag(&mut harness, from, to, Modifiers::ALT);
    let drawn = strokes(&harness);
    assert_eq!(drawn[0].points.len(), 2, "{:?}", drawn[0].points);
    let [x, y] = drawn[0].points[1];
    assert!(
        (x - 0.625).abs() < 2e-3 && (y - 0.5).abs() < 2e-3,
        "{x} {y}"
    );
}

#[test]
fn a_stroke_at_a_key_goes_into_that_key() {
    let mut harness = started();
    let id = drawing(&mut harness);
    let key = format!("annot key add {id} --at 24");
    harness
        .state_mut()
        .editor
        .ask(Action::Apply(vec![step(&key)]));
    harness.run();
    let from = on_screen(&harness, pos2(800.0, 500.0));
    let to = on_screen(&harness, pos2(1000.0, 600.0));
    drag(&mut harness, from, to, Modifiers::NONE);
    let annotation = last(harness.state().editor.doc.project());
    assert!(
        strokes(&harness).is_empty(),
        "the annotation keeps its body"
    );
    let keyed: DrawingBody = serde_json::from_value(serde_json::Value::Object(
        annotation.keys.last().unwrap().body.clone(),
    ))
    .unwrap();
    let lines = [ADD, &key, &line(id, &keyed.strokes[0], " --at 24")];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
}

#[test]
fn a_right_drag_moves_the_drawing() {
    let mut harness = started();
    drawing(&mut harness);
    let from = on_screen(&harness, pos2(1280.0, 720.0));
    let to = on_screen(&harness, pos2(1380.0, 720.0));
    let press = |pressed| Event::PointerButton {
        pos: if pressed { from } else { to },
        button: eframe::egui::PointerButton::Secondary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    harness.event(Event::PointerMoved(from));
    harness.event(press(true));
    harness.run();
    for share in [0.25, 0.5, 0.75, 1.0] {
        harness.event(Event::PointerMoved(from + (to - from) * share));
        harness.run();
    }
    harness.event(press(false));
    harness.run();
    harness.run();
    assert!(strokes(&harness).is_empty(), "no stroke");
    let rect = last(harness.state().editor.doc.project()).rect;
    assert!(rect.x() > 0.25, "{rect:?}");
}
