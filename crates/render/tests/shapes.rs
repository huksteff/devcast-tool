mod common;

use common::{TestResult, assert_near, gpu, pixel, render_shapes, size};
use dc_render::{
    Band, Camera, Geometry, Gradient, Layer, Paint, PxRect, Rgba, Scene, Shape, Stroke,
};

const RED: Rgba = Rgba([255, 0, 0, 255]);
const BLUE: Rgba = Rgba([0, 0, 255, 255]);
const WHITE: Rgba = Rgba([255, 255, 255, 255]);
const BLACK: [u8; 4] = [0, 0, 0, 255];

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn filled(geometry: Geometry, color: Rgba) -> Shape {
    Shape {
        geometry,
        fill: Some(Paint::Solid(color)),
        stroke: None,
        opacity: 1.0,
        rotation: 0.0,
        glow: None,
    }
}

fn bar(width: f32, height: f32, paint: Paint) -> Shape {
    let geometry = Geometry::Rect {
        rect: rect(0.0, 0.0, width, height),
        radius: 0.0,
    };
    Shape {
        fill: Some(paint),
        ..filled(geometry, RED)
    }
}

fn scene_of(width: u32, height: u32, shapes: Vec<Shape>) -> Scene {
    let mut scene = Scene::new(size(width, height));
    scene.push(Band::Annotations, Layer::Shapes(shapes));
    scene
}

fn gradient(colors: Vec<Rgba>, angle: f32, cycle: Option<f32>) -> Paint {
    Paint::Gradient(Gradient {
        colors: colors.try_into().unwrap_or_else(|e| panic!("{e}")),
        angle,
        cycle,
    })
}

#[test]
fn rectangle_fills_inside_and_blends_its_edge_in_linear_light() -> TestResult {
    let shape = filled(
        Geometry::Rect {
            rect: rect(10.5, 10.0, 40.0, 20.0),
            radius: 0.0,
        },
        RED,
    );
    let image = render_shapes(&gpu()?, &scene_of(64, 40, vec![shape]))?;
    assert_near(pixel(&image, 30, 20), [255, 0, 0, 255], 0, "inside");
    assert_near(pixel(&image, 5, 5), BLACK, 0, "outside");
    assert_near(pixel(&image, 9, 20), BLACK, 0, "left of the edge");
    assert_near(
        pixel(&image, 10, 20),
        [188, 0, 0, 255],
        4,
        "half-covered edge pixel",
    );
    Ok(())
}

#[test]
fn rounded_corner_is_cut() -> TestResult {
    let shape = filled(
        Geometry::Rect {
            rect: rect(10.0, 10.0, 40.0, 40.0),
            radius: 12.0,
        },
        RED,
    );
    let image = render_shapes(&gpu()?, &scene_of(64, 64, vec![shape]))?;
    assert_near(pixel(&image, 11, 11), BLACK, 0, "corner");
    assert_near(pixel(&image, 11, 30), [255, 0, 0, 255], 0, "side");
    Ok(())
}

#[test]
fn ellipse_fills_its_box_without_the_corners() -> TestResult {
    let shape = filled(
        Geometry::Ellipse {
            rect: rect(0.0, 0.0, 64.0, 32.0),
        },
        RED,
    );
    let image = render_shapes(&gpu()?, &scene_of(64, 32, vec![shape]))?;
    assert_near(pixel(&image, 32, 16), [255, 0, 0, 255], 0, "centre");
    assert_near(
        pixel(&image, 61, 16),
        [255, 0, 0, 255],
        0,
        "end of the long axis",
    );
    assert_near(pixel(&image, 3, 3), BLACK, 0, "corner");
    Ok(())
}

#[test]
fn line_has_round_caps_and_its_width() -> TestResult {
    let line = Geometry::Line {
        from: [10.0, 20.0],
        to: [54.0, 20.0],
        width: 6.0,
    };
    let image = render_shapes(&gpu()?, &scene_of(64, 40, vec![filled(line, WHITE)]))?;
    assert_near(pixel(&image, 32, 20), [255; 4], 0, "on the line");
    assert_near(pixel(&image, 32, 24), BLACK, 0, "beside it");
    assert_near(pixel(&image, 8, 20), [255; 4], 0, "inside the round cap");
    assert_near(pixel(&image, 5, 20), BLACK, 0, "beyond the cap");
    Ok(())
}

#[test]
fn arrow_has_whiskers_at_45_degrees() -> TestResult {
    let arrow = Geometry::Arrow {
        from: [10.0, 32.0],
        to: [54.0, 32.0],
        width: 4.0,
        head: 20.0,
    };
    let image = render_shapes(&gpu()?, &scene_of(64, 64, vec![filled(arrow, WHITE)]))?;
    assert_near(pixel(&image, 30, 32), [255; 4], 0, "shaft");
    assert_near(pixel(&image, 46, 24), [255; 4], 0, "upper whisker");
    assert_near(pixel(&image, 46, 39), [255; 4], 0, "lower whisker");
    assert_near(pixel(&image, 46, 28), BLACK, 0, "between shaft and whisker");
    assert_near(pixel(&image, 30, 20), BLACK, 0, "away from the arrow");
    Ok(())
}

#[test]
fn outline_without_fill_leaves_the_inside_empty() -> TestResult {
    let shape = Shape {
        fill: None,
        stroke: Some(Stroke {
            width: 4.0,
            color: WHITE,
        }),
        ..filled(
            Geometry::Rect {
                rect: rect(10.0, 10.0, 44.0, 44.0),
                radius: 0.0,
            },
            RED,
        )
    };
    let image = render_shapes(&gpu()?, &scene_of(64, 64, vec![shape]))?;
    assert_near(pixel(&image, 32, 32), BLACK, 0, "inside");
    assert_near(pixel(&image, 10, 32), [255; 4], 0, "on the edge");
    assert_near(pixel(&image, 6, 32), BLACK, 0, "outside the outline");
    Ok(())
}

#[test]
fn outline_is_drawn_over_the_fill_and_opacity_fades_both() -> TestResult {
    let shape = Shape {
        stroke: Some(Stroke {
            width: 4.0,
            color: WHITE,
        }),
        opacity: 0.5,
        ..filled(
            Geometry::Rect {
                rect: rect(8.0, 8.0, 48.0, 48.0),
                radius: 0.0,
            },
            RED,
        )
    };
    let image = render_shapes(&gpu()?, &scene_of(64, 64, vec![shape]))?;
    assert_near(pixel(&image, 32, 32), [188, 0, 0, 255], 2, "half red fill");
    assert_near(
        pixel(&image, 8, 32),
        [188, 188, 188, 255],
        2,
        "half white outline",
    );
    Ok(())
}

#[test]
fn rotation_turns_the_figure_around_its_centre() -> TestResult {
    let shape = Shape {
        rotation: 90.0,
        ..filled(
            Geometry::Rect {
                rect: rect(12.0, 29.0, 40.0, 6.0),
                radius: 0.0,
            },
            WHITE,
        )
    };
    let image = render_shapes(&gpu()?, &scene_of(64, 64, vec![shape]))?;
    assert_near(pixel(&image, 32, 15), [255; 4], 0, "now vertical");
    assert_near(pixel(&image, 15, 32), BLACK, 0, "no longer horizontal");
    Ok(())
}

#[test]
fn gradient_runs_edge_to_edge_mixing_in_srgb() -> TestResult {
    let gpu = gpu()?;
    let two = bar(64.0, 8.0, gradient(vec![RED, BLUE], 0.0, None));
    let image = render_shapes(&gpu, &scene_of(64, 8, vec![two]))?;
    assert_near(pixel(&image, 0, 4), [255, 0, 0, 255], 4, "left");
    assert_near(pixel(&image, 63, 4), [0, 0, 255, 255], 4, "right");
    assert_near(
        pixel(&image, 32, 4),
        [126, 0, 130, 255],
        4,
        "middle, sRGB mix",
    );
    let four = bar(64.0, 8.0, gradient(vec![RED, WHITE, BLUE, RED], 0.0, None));
    let image = render_shapes(&gpu, &scene_of(64, 8, vec![four]))?;
    assert_near(pixel(&image, 21, 4), [255; 4], 8, "second of four at 1/3");
    Ok(())
}

#[test]
fn gradient_angle_turns_its_direction() -> TestResult {
    let shape = bar(16.0, 64.0, gradient(vec![RED, BLUE], 90.0, None));
    let image = render_shapes(&gpu()?, &scene_of(16, 64, vec![shape]))?;
    assert_near(pixel(&image, 8, 0), [255, 0, 0, 255], 4, "top");
    assert_near(pixel(&image, 8, 63), [0, 0, 255, 255], 4, "bottom");
    Ok(())
}

#[test]
fn looping_gradient_shifts_with_phase_and_wraps_seamlessly() -> TestResult {
    let gpu = gpu()?;
    let at = |phase: f32| {
        let shape = bar(64.0, 8.0, gradient(vec![RED, BLUE], 0.0, Some(phase)));
        render_shapes(&gpu, &scene_of(64, 8, vec![shape]))
    };
    let start = at(0.0)?;
    assert_near(pixel(&start, 0, 4), [255, 0, 0, 255], 8, "red at the left");
    assert_near(
        pixel(&start, 32, 4),
        [0, 0, 255, 255],
        8,
        "blue in the middle",
    );
    assert_near(
        pixel(&start, 63, 4),
        [255, 0, 0, 255],
        8,
        "back to red: seamless",
    );
    let half = at(0.5)?;
    assert_near(
        pixel(&half, 0, 4),
        [0, 0, 255, 255],
        8,
        "pattern moved left",
    );
    let full = at(1.0)?;
    for x in [0, 16, 32, 48, 63] {
        assert_near(
            pixel(&full, x, 4),
            pixel(&start, x, 4),
            1,
            "a whole cycle later",
        );
    }
    Ok(())
}

#[test]
fn camera_moves_annotations_but_not_the_overlay() -> TestResult {
    let dot = || {
        filled(
            Geometry::Ellipse {
                rect: rect(8.0, 8.0, 8.0, 8.0),
            },
            WHITE,
        )
    };
    let mut scene = scene_of(64, 64, vec![dot()]);
    scene.push(Band::Overlay, Layer::Shapes(vec![dot()]));
    scene.camera = Camera {
        scale: 2.0,
        x: 16.0,
        y: 16.0,
    };
    let image = render_shapes(&gpu()?, &scene)?;
    assert_near(pixel(&image, 12, 12), [255; 4], 0, "overlay dot stays");
    assert_near(
        pixel(&image, 40, 40),
        [255; 4],
        0,
        "annotation dot at 12*2+16",
    );
    assert_near(pixel(&image, 33, 40), [255; 4], 0, "and twice as large");
    Ok(())
}

#[test]
fn bands_paint_in_order_whatever_the_push_order() -> TestResult {
    let mut scene = Scene::new(size(16, 16));
    scene.push(
        Band::Overlay,
        Layer::Shapes(vec![bar(16.0, 16.0, Paint::Solid(BLUE))]),
    );
    scene.push(
        Band::Annotations,
        Layer::Shapes(vec![bar(16.0, 16.0, Paint::Solid(RED))]),
    );
    let image = render_shapes(&gpu()?, &scene)?;
    assert_near(pixel(&image, 8, 8), [0, 0, 255, 255], 0, "overlay on top");
    Ok(())
}
