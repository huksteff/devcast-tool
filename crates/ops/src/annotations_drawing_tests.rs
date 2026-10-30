//! Drawing annotations in the scene: strokes in the box, draw-in by output time, keys.

use dc_core::{Command, CursorTracks, KeyTracks, OutputTime, Project};
use dc_fx_annotations::{AnnotAdd, AnnotAnim};
use dc_fx_drawing::{DrawingAdd, DrawingSet};
use dc_render::{Band, Drawing, Layer, TextMeasure, Tool};
use serde_json::{Value, json};

use crate::Evaluator;
use crate::evaluate_tests::two_clips;

fn run<C: Command + for<'de> serde::Deserialize<'de>>(p: &mut Project, args: Value) {
    let command: C = serde_json::from_value(args).unwrap();
    command.apply(p).unwrap();
}

/// The drawings of the annotations band at output `at`, bottom first.
fn drawings(p: &Project, at: f64) -> Vec<Drawing> {
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
            (Band::Annotations, Layer::Drawing(drawing)) => Some(drawing.clone()),
            _ => None,
        })
        .collect()
}

/// The recording with a cut at source 1–2 s (output 0–3 s): drawing a1 from output 0.2 s for
/// 2 s in box 0.1,0.2,0.5,0.25 (192,216 960×270 px of the 1920×1080 canvas).
fn drawing() -> Project {
    let mut p = two_clips();
    let args = json!({"kind": "drawing", "at": 0.2, "duration": 2, "rect": "0.1,0.2,0.5,0.25"});
    run::<AnnotAdd>(&mut p, args);
    p
}

#[test]
fn an_empty_drawing_puts_no_layer_and_strokes_cross_the_box() {
    let mut p = drawing();
    assert!(drawings(&p, 1.0).is_empty(), "no strokes yet");
    run::<DrawingAdd>(&mut p, json!({"id": "a1", "points": "0,0,1,1"}));
    run::<DrawingAdd>(
        &mut p,
        json!({"id": "a1", "points": "0.5,0.5", "tool": "eraser"}),
    );
    let [d] = &drawings(&p, 1.5)[..] else {
        panic!("one drawing")
    };
    assert_eq!(d.strokes[0].points, [[192.0, 216.0], [1152.0, 486.0]]);
    assert_eq!(d.strokes[0].width, 4.0, "ref px at 1080");
    assert_eq!(d.strokes[1].tool, Tool::Eraser);
    assert_eq!((d.drawn, d.opacity), (1.0, 1.0));
    assert!(drawings(&p, 0.1).is_empty() && drawings(&p, 2.3).is_empty());
}

#[test]
fn draw_in_reveals_the_strokes_by_output_time_and_keys_fade_them() {
    let mut p = drawing();
    run::<DrawingAdd>(&mut p, json!({"id": "a1", "points": "0,0,1,1"}));
    run::<AnnotAnim>(
        &mut p,
        json!({"id": "a1", "enter": "draw", "enter_duration": 1.6, "exit": "draw", "exit_duration": 0.2}),
    );
    let drawn = |p: &Project, t| drawings(p, t)[0].drawn;
    assert!((drawn(&p, 1.0) - 0.5).abs() < 1e-6, "0.8 s of 1.6 s");
    assert_eq!(drawn(&p, 1.9), 1.0);
    assert!(
        (drawn(&p, 2.1) - 0.5).abs() < 1e-5,
        "undrawn on the way out"
    );
    run::<DrawingSet>(&mut p, json!({"id": "a1", "at": 2.0, "opacity": 0.2}));
    let opacity = drawings(&p, 1.0)[0].opacity;
    assert!(
        opacity > 0.2 && opacity < 1.0,
        "between the keys: {opacity}"
    );
    assert!(
        (drawings(&p, 2.1)[0].opacity - 0.2).abs() < 1e-6,
        "the last key holds"
    );
}
