use dc_core::{OutputTime, TimeDelta};
use dc_fx_annotations::Placement;
use dc_render::{Geometry, Paint, PxRect, Rgba};

use crate::{BoxPoint, Figure, ShapeBody, evaluate};

/// A 400×200 box at (100, 50) of a 1080p canvas, as it stands.
fn placed() -> Placement {
    Placement {
        rect: PxRect {
            x: 100.0,
            y: 50.0,
            w: 400.0,
            h: 200.0,
        },
        rotation: 0.0,
        k: 1.0,
        opacity: 1.0,
        reveal: 1.0,
    }
}

fn of(figure: Figure) -> ShapeBody {
    ShapeBody {
        figure,
        ..ShapeBody::default()
    }
}

fn at(secs: f64) -> OutputTime {
    OutputTime::ZERO + TimeDelta::from_secs_f64(secs)
}

fn near(a: [f32; 2], b: [f32; 2]) {
    assert!(
        (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3,
        "{a:?} {b:?}"
    );
}

#[test]
fn arrow_runs_between_its_box_points_with_a_head_that_fits() {
    let shape = evaluate(&of(Figure::Arrow), &placed(), at(0.0));
    let Geometry::Arrow {
        from,
        to,
        width,
        head,
    } = shape.geometry
    else {
        panic!("{shape:?}")
    };
    near(from, [100.0, 150.0]);
    near(to, [500.0, 150.0]);
    assert_eq!(width, 6.0, "ref px, whatever the box");
    assert!(
        (head - 5.3 * 6.0).abs() < 1e-3,
        "limited by the thickness: {head}"
    );
    assert_eq!(shape.fill, Some(Paint::Solid(ShapeBody::default().color)));
    assert!(shape.stroke.is_none() && shape.rotation == 0.0);
    let short = ShapeBody {
        to: BoxPoint { x: 0.1, y: 0.5 },
        ..of(Figure::Arrow)
    };
    let Geometry::Arrow { head, .. } = evaluate(&short, &placed(), at(0.0)).geometry else {
        panic!()
    };
    assert!(
        (head - 0.354 * 40.0).abs() < 1e-3,
        "a short arrow is not all head: {head}"
    );
}

#[test]
fn turned_box_turns_the_line_around_its_centre() {
    let line = ShapeBody { ..of(Figure::Line) };
    let turned = Placement {
        rotation: 90.0,
        ..placed()
    };
    let Geometry::Line { from, to, .. } = evaluate(&line, &turned, at(0.0)).geometry else {
        panic!()
    };
    near(from, [300.0, -50.0]);
    near(to, [300.0, 350.0]);
}

#[test]
fn closed_figures_keep_the_outline_inside_the_box() {
    let k2 = Placement {
        k: 2.0,
        rotation: 30.0,
        ..placed()
    };
    let rect = evaluate(&of(Figure::Rectangle), &k2, at(0.0));
    let inner = PxRect {
        x: 106.0,
        y: 56.0,
        w: 388.0,
        h: 188.0,
    };
    assert_eq!(
        rect.geometry,
        Geometry::Rect {
            rect: inner,
            radius: 16.0
        }
    );
    assert_eq!(rect.stroke.as_ref().map(|s| s.width), Some(12.0));
    assert_eq!(rect.fill, None, "alpha 00 is no fill");
    assert_eq!(rect.rotation, 30.0);
    let circle = evaluate(&of(Figure::Circle), &placed(), at(0.0));
    let square = PxRect {
        x: 203.0,
        y: 53.0,
        w: 194.0,
        h: 194.0,
    };
    assert_eq!(circle.geometry, Geometry::Ellipse { rect: square });
    let bare = ShapeBody {
        outline: false,
        fill: Rgba([1, 2, 3, 255]),
        ..of(Figure::Ellipse)
    };
    let ellipse = evaluate(&bare, &placed(), at(0.0));
    assert_eq!(
        ellipse.geometry,
        Geometry::Ellipse {
            rect: placed().rect
        }
    );
    assert_eq!(ellipse.fill, Some(Paint::Solid(Rgba([1, 2, 3, 255]))));
    assert!(ellipse.stroke.is_none());
}

#[test]
fn gradient_flows_by_output_time_or_stands_still() {
    let flowing = ShapeBody {
        gradient: true,
        cycle: 2.0,
        angle: 45.0,
        ..of(Figure::Highlight)
    };
    let phase = |body: &ShapeBody, t| match evaluate(body, &placed(), at(t)).fill {
        Some(Paint::Gradient(g)) => (g.cycle, g.angle),
        other => panic!("{other:?}"),
    };
    assert_eq!(phase(&flowing, 0.5), (Some(0.25), 45.0));
    assert_eq!(phase(&flowing, 2.5), (Some(0.25), 45.0), "a seamless loop");
    let still = ShapeBody {
        cycle: 0.0,
        ..flowing
    };
    assert_eq!(phase(&still, 0.5), (None, 45.0));
}

#[test]
fn glow_takes_the_gradient_or_the_paint_colour() {
    let glowing = ShapeBody {
        glow: 10.0,
        ..of(Figure::Arrow)
    };
    let k2 = Placement { k: 2.0, ..placed() };
    let glow = evaluate(&glowing, &k2, at(0.0)).glow.unwrap();
    assert_eq!((glow.color, glow.sigma), (glowing.color, 15.0));
    let filled = ShapeBody {
        fill: Rgba([9, 9, 9, 255]),
        ..of(Figure::Rectangle)
    };
    let glow = evaluate(
        &ShapeBody {
            glow: 4.0,
            ..filled.clone()
        },
        &k2,
        at(0.0),
    )
    .glow;
    assert_eq!(glow.unwrap().color, Rgba([9, 9, 9, 255]), "the fill");
    let gradient = ShapeBody {
        gradient: true,
        glow: 4.0,
        ..filled
    };
    let glow = evaluate(&gradient, &k2, at(0.0)).glow.unwrap();
    assert_eq!(glow.color, gradient.colors.colors()[0]);
    assert!(evaluate(&of(Figure::Arrow), &k2, at(0.0)).glow.is_none());
}

#[test]
fn opacity_and_draw_in_come_with_the_placement() {
    let faded = ShapeBody {
        opacity: 0.5,
        ..of(Figure::Ellipse)
    };
    let half = Placement {
        opacity: 0.5,
        reveal: 0.4,
        ..placed()
    };
    let shape = evaluate(&faded, &half, at(0.0));
    assert_eq!((shape.opacity, shape.drawn), (0.25, 0.4));
}
