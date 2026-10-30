use dc_fx_annotations::Placement;
use dc_render::{PxRect, Rgba};

use crate::{BlurBody, evaluate};

/// A 400×200 box at (100, 50), turned by 30°, of a 4K canvas (2 px per reference px).
fn placed() -> Placement {
    Placement {
        rect: PxRect {
            x: 100.0,
            y: 50.0,
            w: 400.0,
            h: 200.0,
        },
        rotation: 30.0,
        k: 2.0,
        opacity: 1.0,
        reveal: 1.0,
    }
}

#[test]
fn strength_and_corners_are_reference_px_and_the_box_is_the_placement() {
    let body = BlurBody {
        strength: 25.0,
        tint: Rgba([10, 20, 30, 128]),
        radius: 8.0,
    };
    let blur = evaluate(&body, &placed());
    assert_eq!(blur.rect, placed().rect);
    assert_eq!((blur.sigma, blur.radius), (50.0, 16.0));
    assert_eq!(blur.tint, body.tint);
    assert_eq!((blur.rotation, blur.opacity), (30.0, 1.0));
}

/// A pop scales σ and the corners with the box; a fade fades the whole region.
#[test]
fn entrance_scales_and_fades_the_region() {
    let half = Placement {
        k: 1.0,
        opacity: 0.25,
        ..placed()
    };
    let blur = evaluate(&BlurBody::default(), &half);
    assert_eq!((blur.sigma, blur.radius, blur.opacity), (20.0, 8.0, 0.25));
}
