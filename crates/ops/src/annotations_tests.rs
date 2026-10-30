use std::path::Path;

use dc_core::{Command, CursorTracks, KeyTracks, OutputTime, Project};
use dc_fx_annotations::{AnnotAdd, AnnotAnim, AnnotSet, Annotations};
use dc_fx_text::TextSet;
use dc_render::{Band, Layer, PxRect, Rgba, Scene, Text, TextMeasure};
use serde_json::{Value, json};

use crate::evaluate_tests::two_clips;
use crate::{EvaluateError, Evaluator};

fn run<C: Command + for<'de> serde::Deserialize<'de>>(p: &mut Project, args: Value) {
    let command: C = serde_json::from_value(args).unwrap();
    command.apply(p).unwrap();
}

fn measure() -> TextMeasure {
    let mut measure = TextMeasure::new();
    let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/fonts");
    measure.load_fonts(&fonts).unwrap();
    measure
}

fn scene(p: &Project, at: f64) -> Result<Scene, EvaluateError> {
    let evaluator = Evaluator::new(p, CursorTracks::new(), KeyTracks::new(), &mut measure())?;
    Ok(evaluator.evaluate(OutputTime::from_secs_f64(at)))
}

/// Text layers of the annotations band, bottom first.
fn texts(scene: &Scene) -> Vec<&Text> {
    scene
        .painted()
        .filter_map(|(band, layer)| match (band, layer) {
            (Band::Annotations, Layer::Text(text)) => Some(text),
            _ => None,
        })
        .collect()
}

/// The recording with a cut at source 1–2 s (output 0–3 s), then a 4:3 one (output 3–5 s):
/// a1 «верх» on track 1, a2 «низ» on track 0 added later, a3 a shape, all from 0.2 s for 2 s.
fn layered() -> Project {
    let mut p = two_clips();
    for (kind, track) in [("text", 1), ("text", 0), ("shape", 0)] {
        let args = json!({"kind": kind, "at": 0.2, "duration": 2, "track": track,
            "rect": "0.1,0.2,0.5,0.25", "rotation": 12});
        run::<AnnotAdd>(&mut p, args);
    }
    run::<TextSet>(&mut p, json!({"id": "a1", "text": "верх"}));
    run::<TextSet>(&mut p, json!({"id": "a2", "text": "низ", "size": 64}));
    p
}

#[test]
fn texts_lie_in_their_layer_order_with_their_box_in_canvas_px() {
    let p = layered();
    let scene = scene(&p, 1.5).unwrap();
    let shown = texts(&scene);
    let words: Vec<&str> = shown.iter().map(|t| t.runs[0].text.as_str()).collect();
    assert_eq!(words, ["низ", "верх"], "track 0 under track 1");
    let kinds: Vec<&str> = scene
        .painted()
        .filter(|(band, _)| *band == Band::Annotations)
        .map(|(_, layer)| match layer {
            Layer::Text(_) => "text",
            Layer::Shapes(_) => "shape",
            _ => "other",
        })
        .collect();
    assert_eq!(
        kinds,
        ["text", "shape", "text"],
        "the shape a3 over a2, under a1"
    );
    assert_eq!(
        shown[0].rect,
        PxRect {
            x: 192.0,
            y: 216.0,
            w: 960.0,
            h: 270.0
        }
    );
    assert_eq!((shown[0].rotation, shown[0].font.size), (12.0, 64.0));
    let bands: Vec<Band> = scene.painted().map(|(band, _)| band).collect();
    assert!(bands.windows(2).all(|w| w[0] <= w[1]));
    assert!(Band::Annotations.under_camera());
}

#[test]
fn texts_show_in_their_source_span_through_the_cut() {
    let p = layered();
    // Output 0.2–2.2 s is source 0.2–1.0 and 2.0–3.2 s of the first recording.
    assert!(texts(&scene(&p, 0.1).unwrap()).is_empty());
    assert_eq!(texts(&scene(&p, 0.9).unwrap()).len(), 2);
    assert_eq!(texts(&scene(&p, 2.1).unwrap()).len(), 2);
    assert!(texts(&scene(&p, 2.3).unwrap()).is_empty());
    assert!(
        texts(&scene(&p, 3.5).unwrap()).is_empty(),
        "the second recording has none"
    );
}

#[test]
fn a_missing_font_or_broken_fields_stop_before_any_frame() {
    let mut p = layered();
    run::<TextSet>(&mut p, json!({"id": "a1", "font": "Comic Sans"}));
    let error = scene(&p, 1.0).unwrap_err();
    assert!(matches!(error, EvaluateError::Text(_)), "{error}");
    assert!(error.to_string().contains("Comic Sans"), "{error}");
    let mut p = layered();
    let mut state = p.state::<Annotations>().unwrap();
    state.annotations[1]
        .body
        .insert("size".into(), json!("большой"));
    p.set_state::<Annotations>(&state).unwrap();
    let error = scene(&p, 1.0).unwrap_err();
    assert!(error.to_string().contains("annot rm a2"), "{error}");
}

#[test]
fn animations_change_the_text_layer_by_output_time() {
    let mut p = layered();
    run::<AnnotAnim>(
        &mut p,
        json!({"id": "a2", "enter": "pop", "enter_duration": 1.6}),
    );
    run::<AnnotAnim>(
        &mut p,
        json!({"id": "a1", "exit": "typewriter", "exit_duration": 1}),
    );
    // Output 1.0 s is right after the cut: 0.8 s into the pop of a2, all of a1 typed.
    let frame = scene(&p, 1.0).unwrap();
    let [low, high] = texts(&frame)[..] else {
        panic!("two texts")
    };
    assert_eq!((low.opacity, low.font.size), (0.5, 64.0 * 0.93));
    let centre = |t: &Text| [t.rect.x + t.rect.w / 2.0, t.rect.y + t.rect.h / 2.0];
    assert_eq!(centre(low), [672.0, 351.0], "scaled around the box centre");
    assert_eq!(high.runs.len(), 1);
    let frame = scene(&p, 1.7).unwrap();
    let typed: Vec<&str> = texts(&frame)[1]
        .runs
        .iter()
        .map(|r| r.text.as_str())
        .collect();
    assert_eq!(typed, ["ве", "рх"], "0.5 s of the 1 s exit left");
}

#[test]
fn keys_move_the_box_and_blend_the_text_fields() {
    let mut p = layered();
    // a2 shows output 0.2–2.2 s; a change at 2.0 s keys it from its start (output 0.2 s).
    run::<AnnotSet>(
        &mut p,
        json!({"id": "a2", "at": 2.0, "x": 0.3, "rotation": -12}),
    );
    run::<TextSet>(
        &mut p,
        json!({"id": "a2", "at": 2.0, "size": 32, "color": "#000000ff"}),
    );
    // Output 0.6 s is source 0.6 s and 2.0 s is source 3.0 s (after the cut 1–2 s): 0.4 of the
    // 2.8 s from the start key at source 0.2 s.
    let s: f32 = 0.4 / 2.8;
    let e = s * s * (3.0 - 2.0 * s);
    let frame = scene(&p, 0.6).unwrap();
    let low = texts(&frame)[0];
    let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
    assert!(close(low.rect.x, 192.0 + 384.0 * e), "{}", low.rect.x);
    assert!(close(low.rotation, 12.0 - 24.0 * e));
    assert!(close(low.font.size, 64.0 - 32.0 * e));
    let end = scene(&p, 2.1).unwrap();
    let low = texts(&end)[0];
    assert_eq!(
        (low.rect.x, low.rotation, low.font.size),
        (576.0, -12.0, 32.0)
    );
    assert_eq!(
        low.runs[0].color,
        Rgba([0, 0, 0, 255]),
        "the last key holds"
    );
    run::<TextSet>(&mut p, json!({"id": "a2", "at": 1.0, "font": "Comic Sans"}));
    let error = scene(&p, 0.3).unwrap_err();
    assert!(
        error.to_string().contains("Comic Sans"),
        "a key's font is checked too"
    );
}
