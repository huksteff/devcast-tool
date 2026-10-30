use dc_core::{PixelSize, TimeDelta};
use dc_fx_annotations::Placement;
use dc_render::PxRect;

use crate::tests::picture;
use crate::{ImageBody, PictureFile, evaluate};

fn placed(rect: PxRect) -> Placement {
    Placement {
        rect,
        rotation: 30.0,
        k: 1.0,
        opacity: 0.5,
        reveal: 1.0,
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn gif(durations: &[i64]) -> PictureFile {
    let durations: Vec<_> = durations
        .iter()
        .map(|&ms| TimeDelta::from_millis(ms))
        .collect();
    PictureFile::new(
        "assets/images/a.gif".into(),
        PixelSize {
            width: 4,
            height: 4,
        },
        &durations,
    )
}

#[test]
fn nothing_is_drawn_without_a_picture() {
    let place = placed(rect(0.0, 0.0, 100.0, 100.0));
    assert_eq!(
        evaluate(&ImageBody::default(), &place, TimeDelta::ZERO),
        None
    );
}

#[test]
fn the_picture_keeps_its_proportions_centred_in_the_box_and_turns_with_it() {
    let body = |w, h| ImageBody {
        picture: Some(picture("a.png", w, h)),
    };
    // A wide picture in a square box: full width, centred vertically.
    let place = placed(rect(100.0, 200.0, 400.0, 400.0));
    let sprite = evaluate(&body(200, 100), &place, TimeDelta::ZERO).unwrap();
    assert_eq!(sprite.rect, rect(100.0, 300.0, 400.0, 200.0));
    assert_eq!((sprite.rotation, sprite.opacity), (30.0, 0.5));
    assert_eq!(sprite.image.path, "assets/images/a.png");
    assert_eq!(sprite.image.frame, 0);
    // A tall picture in a wide box: full height, centred horizontally; small pictures grow.
    let place = placed(rect(0.0, 0.0, 300.0, 100.0));
    let sprite = evaluate(&body(10, 20), &place, TimeDelta::ZERO).unwrap();
    assert_eq!(sprite.rect, rect(125.0, 0.0, 50.0, 100.0));
}

#[test]
fn frames_of_equal_length_are_one_run_and_a_still_picture_has_none() {
    assert_eq!(gif(&[100, 100, 100, 50]).frames, [[3, 100], [1, 50]]);
    assert!(gif(&[0]).frames.is_empty());
}

#[test]
fn an_animation_shows_its_frames_by_their_lengths_and_loops() {
    let g = gif(&[100, 100, 300, 50]);
    let at = |ms| g.frame_at(TimeDelta::from_millis(ms));
    assert_eq!(
        [at(0), at(99), at(100), at(200), at(499), at(500), at(549)],
        [0, 0, 1, 2, 2, 3, 3]
    );
    assert_eq!(
        [at(550), at(650), at(1100)],
        [0, 1, 0],
        "loops every 550 ms"
    );
}

#[test]
fn an_animation_without_length_and_a_still_picture_show_frame_0() {
    assert_eq!(gif(&[0, 0]).frame_at(TimeDelta::from_millis(70)), 0);
    assert_eq!(gif(&[0]).frame_at(TimeDelta::from_secs_f64(3.0)), 0);
}
