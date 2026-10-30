//! Blur annotations in the scene: the region over what lies below, keys and fades by output time.

use dc_core::{Command, CursorTracks, KeyTracks, OutputTime, Project};
use dc_fx_annotations::{AnnotAdd, AnnotAnim, AnnotMove};
use dc_fx_blur::BlurSet;
use dc_render::{Band, Blur, Layer, PxRect, Rgba, TextMeasure};
use serde_json::{Value, json};

use crate::Evaluator;
use crate::evaluate_tests::two_clips;

fn run<C: Command + for<'de> serde::Deserialize<'de>>(p: &mut Project, args: Value) {
    let command: C = serde_json::from_value(args).unwrap();
    command.apply(p).unwrap();
}

/// The layers of the annotations band at output `at`, bottom first.
fn layers(p: &Project, at: f64) -> Vec<Layer> {
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
        .filter(|(band, _)| *band == Band::Annotations)
        .map(|(_, layer)| layer.clone())
        .collect()
}

fn blur(p: &Project, at: f64) -> Blur {
    let found = layers(p, at).into_iter().find_map(|layer| match layer {
        Layer::Blur(blur) => Some(blur),
        _ => None,
    });
    found.unwrap()
}

/// The recording with a cut at source 1–2 s (output 0–3 s): blur a1 from output 0.2 s for 2 s
/// in box 0.1,0.2,0.5,0.25 (192,216 960×270 px of the 1920×1080 canvas), turned by 20°.
fn region() -> Project {
    let mut p = two_clips();
    let args = json!({"kind": "blur", "at": 0.2, "duration": 2, "rect": "0.1,0.2,0.5,0.25",
        "rotation": 20});
    run::<AnnotAdd>(&mut p, args);
    p
}

#[test]
fn the_region_is_the_box_with_v1_strength_and_corners() {
    let b = blur(&region(), 1.5);
    let rect = PxRect {
        x: 192.0,
        y: 216.0,
        w: 960.0,
        h: 270.0,
    };
    assert_eq!((b.rect, b.sigma, b.radius), (rect, 20.0, 8.0));
    assert_eq!(
        (b.tint, b.rotation, b.opacity),
        (Rgba([0, 0, 0, 0]), 20.0, 1.0)
    );
    assert!(layers(&region(), 0.1).is_empty() && layers(&region(), 2.3).is_empty());
}

/// A shape on a lower track is drawn first, so the blur covers it; on a higher one it stays sharp.
#[test]
fn the_blur_lies_over_the_lower_tracks_only() {
    let mut p = region();
    for track in [0, 5] {
        let args = json!({"kind": "shape", "at": 0.2, "duration": 2, "track": track});
        run::<AnnotAdd>(&mut p, args);
    }
    run::<AnnotMove>(&mut p, json!({"id": "a1", "track": 2}));
    let kinds: Vec<_> = layers(&p, 1.0)
        .iter()
        .map(|layer| matches!(layer, Layer::Blur(_)))
        .collect();
    assert_eq!(kinds, [false, true, false]);
}

/// The fade and the keys run through the cut: 0.4 s of a 0.8 s fade is half, the tint blends
/// between keys.
#[test]
fn fade_and_keys_go_by_output_time_through_the_cut() {
    let mut p = region();
    run::<AnnotAnim>(
        &mut p,
        json!({"id": "a1", "enter": "fade", "enter_duration": 0.8}),
    );
    assert!((blur(&p, 0.6).opacity - 0.5).abs() < 1e-5);
    assert_eq!(blur(&p, 1.5).opacity, 1.0);
    run::<BlurSet>(
        &mut p,
        json!({"id": "a1", "at": 2.0, "tint": "#ff0000ff", "strength": 60}),
    );
    let end = blur(&p, 2.1);
    assert_eq!((end.tint, end.sigma), (Rgba([255, 0, 0, 255]), 60.0));
    let between = blur(&p, 1.5);
    assert!(
        between.sigma > 20.0 && between.sigma < 60.0,
        "{}",
        between.sigma
    );
    assert!(between.tint.0[3] > 0 && between.tint.0[3] < 255);
}
