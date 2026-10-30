use dc_fx_annotations::Placement;
use dc_render::{PxRect, Rgba, Tool};

use crate::{DrawingBody, Stroke, StrokeTool, evaluate};

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

fn stroke(tool: StrokeTool, width: f64, points: &[[f64; 2]]) -> Stroke {
    Stroke {
        tool,
        color: Rgba([255, 0, 0, 255]),
        width,
        points: points.to_vec(),
    }
}

/// A line across the box and an eraser dot in its middle.
fn body() -> DrawingBody {
    DrawingBody {
        strokes: vec![
            stroke(StrokeTool::Pencil, 4.0, &[[0.0, 0.0], [1.0, 1.0]]),
            stroke(StrokeTool::Eraser, 12.0, &[[0.5, 0.5]]),
        ],
        opacity: 0.5,
    }
}

fn near(a: [f32; 2], b: [f32; 2]) {
    assert!(
        (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3,
        "{a:?} {b:?}"
    );
}

#[test]
fn an_empty_drawing_draws_nothing() {
    assert_eq!(evaluate(&DrawingBody::default(), &placed()), None);
}

#[test]
fn strokes_take_box_points_tools_and_reference_widths() {
    let d = evaluate(&body(), &placed()).unwrap();
    let [line, eraser] = &d.strokes[..] else {
        panic!("{d:?}")
    };
    near(line.points[0], [100.0, 50.0]);
    near(line.points[1], [500.0, 250.0]);
    assert_eq!(line.tool, Tool::Pen(Rgba([255, 0, 0, 255])));
    assert_eq!(line.width, 4.0, "ref px, whatever the box");
    near(eraser.points[0], [300.0, 150.0]);
    assert_eq!((eraser.tool, eraser.width), (Tool::Eraser, 12.0));
    assert_eq!((d.drawn, d.opacity), (1.0, 0.5));
}

#[test]
fn the_box_turns_the_points_and_k_scales_the_widths() {
    let turned = Placement {
        rotation: 90.0,
        k: 2.0,
        ..placed()
    };
    let d = evaluate(&body(), &turned).unwrap();
    near(d.strokes[0].points[0], [400.0, -50.0]);
    near(d.strokes[0].points[1], [200.0, 350.0]);
    near(d.strokes[1].points[0], [300.0, 150.0]);
    assert_eq!((d.strokes[0].width, d.strokes[1].width), (8.0, 24.0));
    let tiny = Placement { k: 0.1, ..placed() };
    let d = evaluate(&body(), &tiny).unwrap();
    assert_eq!(d.strokes[0].width, 1.0, "never thinner than a pixel (v1)");
}

#[test]
fn animations_reveal_and_fade_the_drawing() {
    let posed = Placement {
        opacity: 0.5,
        reveal: 0.25,
        ..placed()
    };
    let d = evaluate(&body(), &posed).unwrap();
    assert_eq!((d.drawn, d.opacity), (0.25, 0.25));
}
