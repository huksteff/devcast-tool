//! Image annotations in the scene: the picture fitted into its box, frames of an animation by
//! output time, the picture of a key, the entrance.

use dc_core::{Command, CursorTracks, KeyTracks, OutputTime, PixelSize, Project, TimeDelta};
use dc_fx_annotations::{AnnotAdd, AnnotAnim};
use dc_fx_image::{ImageSet, PictureFile};
use dc_render::{Band, Layer, PxRect, Sprite, TextMeasure};
use serde_json::{Value, json};

use crate::Evaluator;
use crate::evaluate_tests::two_clips;

fn run<C: Command + for<'de> serde::Deserialize<'de>>(p: &mut Project, args: Value) {
    let command: C = serde_json::from_value(args).unwrap();
    command.apply(p).unwrap();
}

/// The sprites of the annotations band at output `at`, bottom first.
fn sprites(p: &Project, at: f64) -> Vec<Sprite> {
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
            (Band::Annotations, Layer::Sprite(sprite)) => Some(sprite.clone()),
            _ => None,
        })
        .collect()
}

/// A 7-frame animation of 100 ms frames, 200×100 px.
fn gif() -> PictureFile {
    let size = PixelSize {
        width: 200,
        height: 100,
    };
    PictureFile::new(
        "assets/images/a.gif".into(),
        size,
        &[TimeDelta::from_millis(100); 7],
    )
}

fn set(p: &mut Project, picture: PictureFile, at: Option<f64>) {
    let set = ImageSet {
        id: "a1".parse().unwrap(),
        picture,
        at: at.map(OutputTime::from_secs_f64),
    };
    set.apply(p).unwrap();
}

/// The recording with a cut at source 1–2 s (output 0–3 s): image a1 from output 0.2 s for 2 s
/// in box 0.1,0.2,0.5,0.25 (192,216 960×270 px of the 1920×1080 canvas).
fn image() -> Project {
    let mut p = two_clips();
    let args = json!({"kind": "image", "at": 0.2, "duration": 2, "rect": "0.1,0.2,0.5,0.25"});
    run::<AnnotAdd>(&mut p, args);
    p
}

#[test]
fn an_image_without_a_picture_draws_nothing() {
    assert!(sprites(&image(), 1.0).is_empty());
}

#[test]
fn the_picture_fits_its_box_keeping_proportions() {
    let mut p = image();
    set(&mut p, gif(), None);
    let [sprite] = &sprites(&p, 0.5)[..] else {
        panic!("one sprite")
    };
    // 2:1 in a 960×270 box: 540×270 in the middle.
    let rect = PxRect {
        x: 192.0 + 210.0,
        y: 216.0,
        w: 540.0,
        h: 270.0,
    };
    assert_eq!(sprite.rect, rect);
    assert_eq!(
        (sprite.image.path.as_str(), sprite.opacity),
        ("assets/images/a.gif", 1.0)
    );
}

#[test]
fn frames_play_on_output_time_across_a_cut() {
    let mut p = image();
    set(&mut p, gif(), None);
    let frame = |at| sprites(&p, at)[0].image.frame;
    assert_eq!(frame(0.2), 0);
    assert_eq!(frame(0.55), 3);
    // Output 1.5 s shows source 2.5 s past the cut: 1.3 s on the output (frame 6 of the 0.7 s
    // loop), not the 2.3 s of the recording (frame 2).
    assert_eq!(frame(1.5), 6);
}

#[test]
fn a_key_changes_the_picture_and_the_entrance_fades_it() {
    let mut p = image();
    set(&mut p, gif(), None);
    let still = PictureFile::new(
        "assets/images/b.png".into(),
        PixelSize {
            width: 10,
            height: 10,
        },
        &[TimeDelta::ZERO],
    );
    set(&mut p, still, Some(1.5));
    run::<AnnotAnim>(
        &mut p,
        json!({"id": "a1", "enter": "fade", "enter_duration": 0.4}),
    );
    let early = &sprites(&p, 0.4)[0];
    assert_eq!(early.image.path, "assets/images/a.gif");
    assert!((early.opacity - 0.5).abs() < 1e-3, "{}", early.opacity);
    let late = &sprites(&p, 1.6)[0];
    assert_eq!(
        (late.image.path.as_str(), late.image.frame),
        ("assets/images/b.png", 0)
    );
    assert_eq!(late.rect.w, 270.0, "square in the 960×270 box");
}
