//! Shape annotations in the scene: the figure, draw-in by output time, keys, the gradient.

use dc_core::{Command, CursorTracks, KeyTracks, OutputTime, Project};
use dc_fx_annotations::{AnnotAdd, AnnotAnim};
use dc_fx_shape::ShapeSet;
use dc_render::{Band, Geometry, Layer, Paint, Shape, TextMeasure};
use serde_json::{Value, json};

use crate::Evaluator;
use crate::evaluate_tests::two_clips;

fn run<C: Command + for<'de> serde::Deserialize<'de>>(p: &mut Project, args: Value) {
    let command: C = serde_json::from_value(args).unwrap();
    command.apply(p).unwrap();
}

/// The shapes of the annotations band at output `at`, bottom first.
fn shapes(p: &Project, at: f64) -> Vec<Shape> {
    let evaluator = Evaluator::new(
        p,
        CursorTracks::new(),
        KeyTracks::new(),
        &mut TextMeasure::new(),
    )
    .unwrap();
    let scene = evaluator.evaluate(OutputTime::from_secs_f64(at));
    scene
        .painted()
        .filter_map(|(band, layer)| match (band, layer) {
            (Band::Annotations, Layer::Shapes(shapes)) => Some(shapes.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// The recording with a cut at source 1–2 s (output 0–3 s): arrow a1 from output 0.2 s for 2 s
/// in box 0.1,0.2,0.5,0.25 (192,216 960×270 px of the 1920×1080 canvas).
fn arrow() -> Project {
    let mut p = two_clips();
    let args = json!({"kind": "shape", "at": 0.2, "duration": 2, "rect": "0.1,0.2,0.5,0.25"});
    run::<AnnotAdd>(&mut p, args);
    p
}

#[test]
fn the_arrow_crosses_its_box_with_no_fonts_loaded() {
    let p = arrow();
    let [shape] = &shapes(&p, 1.5)[..] else {
        panic!("one shape")
    };
    let Geometry::Arrow {
        from, to, width, ..
    } = shape.geometry
    else {
        panic!("{shape:?}")
    };
    assert_eq!((from, to, width), ([192.0, 351.0], [1152.0, 351.0], 6.0));
    assert_eq!(shape.drawn, 1.0);
    assert!(shapes(&p, 0.1).is_empty() && shapes(&p, 2.3).is_empty());
}

#[test]
fn draw_in_reveals_the_figure_by_output_time_through_the_cut() {
    let mut p = arrow();
    run::<ShapeSet>(&mut p, json!({"id": "a1", "figure": "rectangle"}));
    run::<AnnotAnim>(
        &mut p,
        json!({"id": "a1", "enter": "draw", "enter_duration": 1.6, "exit": "draw", "exit_duration": 0.2}),
    );
    let drawn = |t| shapes(&p, t)[0].drawn;
    assert!(
        (drawn(1.0) - 0.5).abs() < 1e-6,
        "0.8 s of 1.6 s: {}",
        drawn(1.0)
    );
    assert_eq!(drawn(1.9), 1.0);
    assert!(
        (drawn(2.1) - 0.5).abs() < 1e-5,
        "undrawn on the way out: {}",
        drawn(2.1)
    );
}

#[test]
fn keys_blend_the_shape_fields_and_the_gradient_flows_by_output_time() {
    let mut p = arrow();
    run::<ShapeSet>(&mut p, json!({"id": "a1", "gradient": true, "cycle": 2}));
    run::<ShapeSet>(&mut p, json!({"id": "a1", "at": 2.0, "width": 16}));
    let first = &shapes(&p, 0.2)[0];
    let Some(Paint::Gradient(g)) = &first.fill else {
        panic!("{first:?}")
    };
    assert!((g.cycle.unwrap() - 0.1).abs() < 1e-5, "{:?}", g.cycle);
    let Geometry::Arrow { width, .. } = shapes(&p, 2.1)[0].geometry else {
        panic!()
    };
    assert_eq!(width, 16.0, "the last key holds");
    let Geometry::Arrow { width, .. } = shapes(&p, 1.0)[0].geometry else {
        panic!()
    };
    assert!(width > 6.0 && width < 16.0, "between the keys: {width}");
}
