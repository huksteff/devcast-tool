use std::path::Path;

use dc_core::{Command, CursorTracks, KeyTracks, OutputTime, Project};
use dc_fx_annotations::{AnnotAdd, Annotations};
use dc_fx_text::TextSet;
use dc_render::{Band, Layer, PxRect, Scene, Text, TextMeasure};
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
    assert_eq!(
        words,
        ["низ", "верх"],
        "track 0 under track 1; the shape draws nothing yet"
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
