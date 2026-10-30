//! Draw-in of figures (`Shape::drawn`): the drawn part of the path, its round ends, the fill
//! fading in.

mod common;

use common::{TestResult, assert_near, gpu, pixel, render_shapes, size};
use dc_render::{Band, Camera, Geometry, Layer, Paint, PxRect, Rgba, Scene, Shape, Stroke};

const RED: Rgba = Rgba([255, 0, 0, 255]);
const WHITE: Rgba = Rgba([255, 255, 255, 255]);
const BLACK: [u8; 4] = [0, 0, 0, 255];

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn drawn(geometry: Geometry, drawn: f32) -> Shape {
    Shape {
        geometry,
        fill: Some(Paint::Solid(WHITE)),
        stroke: None,
        opacity: 1.0,
        rotation: 0.0,
        glow: None,
        drawn,
    }
}

fn outlined(geometry: Geometry, part: f32) -> Shape {
    Shape {
        fill: None,
        stroke: Some(Stroke {
            width: 4.0,
            color: WHITE,
        }),
        ..drawn(geometry, part)
    }
}

fn scene_of(shapes: Vec<Shape>) -> Scene {
    let mut scene = Scene::new(size(64, 64));
    scene.push(Band::Annotations, Layer::Shapes(shapes));
    scene
}

#[test]
fn half_a_rectangle_is_its_top_and_right_sides_and_half_its_fill() -> TestResult {
    let square = Geometry::Rect {
        rect: rect(10.0, 10.0, 44.0, 44.0),
        radius: 0.0,
    };
    let shape = Shape {
        fill: Some(Paint::Solid(RED)),
        ..outlined(square, 0.5)
    };
    let image = render_shapes(&gpu()?, &scene_of(vec![shape]))?;
    assert_near(pixel(&image, 32, 10), [255; 4], 0, "top side");
    assert_near(pixel(&image, 54, 32), [255; 4], 0, "right side");
    assert_near(pixel(&image, 32, 55), BLACK, 0, "bottom side not yet");
    assert_near(pixel(&image, 9, 32), BLACK, 0, "left side not yet");
    assert_near(pixel(&image, 32, 32), [188, 0, 0, 255], 2, "fill at half");
    Ok(())
}

#[test]
fn nothing_is_drawn_at_zero() -> TestResult {
    let line = Geometry::Line {
        from: [10.0, 20.0],
        to: [54.0, 20.0],
        width: 6.0,
    };
    let image = render_shapes(&gpu()?, &scene_of(vec![drawn(line, 0.0)]))?;
    assert_near(pixel(&image, 10, 20), BLACK, 0, "start");
    assert_near(pixel(&image, 8, 20), BLACK, 0, "start cap");
    Ok(())
}

#[test]
fn line_grows_from_its_start_with_a_round_head() -> TestResult {
    let line = Geometry::Line {
        from: [10.0, 20.0],
        to: [54.0, 20.0],
        width: 6.0,
    };
    let image = render_shapes(&gpu()?, &scene_of(vec![drawn(line, 0.5)]))?;
    assert_near(pixel(&image, 8, 20), [255; 4], 0, "start cap");
    assert_near(pixel(&image, 28, 20), [255; 4], 0, "drawn");
    assert_near(pixel(&image, 33, 20), [255; 4], 0, "inside the round head");
    assert_near(
        pixel(&image, 34, 20),
        [255; 4],
        24,
        "the head is round: ahead on the axis",
    );
    assert!(
        pixel(&image, 34, 22)[0] < 128,
        "the head is round: no corner"
    );
    assert_near(pixel(&image, 40, 20), BLACK, 0, "not yet");
    Ok(())
}

#[test]
fn arrow_draws_its_shaft_before_the_whiskers() -> TestResult {
    let arrow = |part| {
        let geometry = Geometry::Arrow {
            from: [10.0, 32.0],
            to: [54.0, 32.0],
            width: 4.0,
            head: 20.0,
        };
        scene_of(vec![drawn(geometry, part)])
    };
    let gpu = gpu()?;
    // Shaft 44 px, whiskers 20 px: 64 px of path.
    let shaft = render_shapes(&gpu, &arrow(44.0 / 64.0))?;
    assert_near(pixel(&shaft, 50, 32), [255; 4], 0, "shaft");
    assert_near(pixel(&shaft, 46, 24), BLACK, 0, "no upper whisker");
    assert_near(pixel(&shaft, 46, 39), BLACK, 0, "no lower whisker");
    let half = render_shapes(&gpu, &arrow(54.0 / 64.0))?;
    assert_near(pixel(&half, 50, 28), [255; 4], 0, "upper whisker begins");
    assert_near(pixel(&half, 50, 35), [255; 4], 0, "lower whisker begins");
    assert_near(pixel(&half, 42, 20), BLACK, 0, "upper whisker end not yet");
    Ok(())
}

#[test]
fn circle_is_traced_clockwise_from_the_top() -> TestResult {
    let circle = Geometry::Ellipse {
        rect: rect(8.0, 8.0, 48.0, 48.0),
    };
    let image = render_shapes(&gpu()?, &scene_of(vec![outlined(circle, 0.25)]))?;
    assert_near(pixel(&image, 32, 8), [255; 4], 0, "top");
    assert_near(pixel(&image, 49, 15), [255; 4], 0, "between top and right");
    assert_near(pixel(&image, 56, 40), BLACK, 0, "below the right");
    assert_near(pixel(&image, 32, 56), BLACK, 0, "bottom");
    assert_near(
        pixel(&image, 15, 15),
        BLACK,
        0,
        "top-left, the last quarter",
    );
    Ok(())
}

#[test]
fn the_path_turns_with_the_figure() -> TestResult {
    let square = Geometry::Rect {
        rect: rect(12.0, 12.0, 40.0, 40.0),
        radius: 0.0,
    };
    let shape = Shape {
        rotation: 90.0,
        ..outlined(square, 0.25)
    };
    let image = render_shapes(&gpu()?, &scene_of(vec![shape]))?;
    assert_near(
        pixel(&image, 52, 32),
        [255; 4],
        0,
        "the top side turned right",
    );
    assert_near(pixel(&image, 32, 12), BLACK, 0, "the left side turned up");
    Ok(())
}

#[test]
fn camera_scales_the_drawn_length() -> TestResult {
    let line = Geometry::Line {
        from: [5.0, 10.0],
        to: [27.0, 10.0],
        width: 3.0,
    };
    let mut scene = scene_of(vec![drawn(line, 0.5)]);
    scene.camera = Camera {
        scale: 2.0,
        x: 0.0,
        y: 0.0,
    };
    let image = render_shapes(&gpu()?, &scene)?;
    assert_near(pixel(&image, 28, 20), [255; 4], 0, "drawn");
    assert_near(pixel(&image, 40, 20), BLACK, 0, "not yet");
    Ok(())
}

#[test]
fn whole_figure_does_not_write_drawn() {
    let line = Geometry::Line {
        from: [0.0, 0.0],
        to: [1.0, 0.0],
        width: 1.0,
    };
    let json = serde_json::to_string(&drawn(line.clone(), 1.0)).unwrap_or_default();
    assert!(!json.contains("drawn"), "{json}");
    let json = serde_json::to_string(&drawn(line, 0.5)).unwrap_or_default();
    assert!(json.contains(r#""drawn":0.5"#), "{json}");
}
