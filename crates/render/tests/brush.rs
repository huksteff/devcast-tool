mod common;

use common::{TestResult, assert_near, gpu, pixel, render_shapes, size};
use dc_render::{
    Band, BrushStroke, Camera, Drawing, Geometry, Layer, Paint, PxRect, Rgba, Scene, Shape, Tool,
};

const RED: Rgba = Rgba([255, 0, 0, 255]);
const BLACK: [u8; 4] = [0, 0, 0, 255];

fn pen(points: &[[f32; 2]], width: f32, color: Rgba) -> BrushStroke {
    BrushStroke {
        points: points.to_vec(),
        width,
        tool: Tool::Pen(color),
    }
}

fn eraser(points: &[[f32; 2]], width: f32) -> BrushStroke {
    BrushStroke {
        points: points.to_vec(),
        width,
        tool: Tool::Eraser,
    }
}

fn drawing(strokes: Vec<BrushStroke>) -> Drawing {
    Drawing {
        strokes,
        drawn: 1.0,
        opacity: 1.0,
    }
}

fn scene_of(drawing: Drawing) -> Scene {
    let mut scene = Scene::new(size(100, 60));
    scene.push(Band::Annotations, Layer::Drawing(drawing));
    scene
}

#[test]
fn stroke_covers_its_width_with_round_caps_and_smooth_edges() -> TestResult {
    let line = pen(&[[20.0, 20.0], [80.0, 20.0]], 11.0, RED);
    let image = render_shapes(&gpu()?, &scene_of(drawing(vec![line])))?;
    assert_near(pixel(&image, 50, 20), [255, 0, 0, 255], 0, "on the line");
    assert_near(pixel(&image, 50, 30), BLACK, 0, "below the line");
    // The edge at y 14.5 halves row 14: half of the samples, 0.5 in linear light.
    assert_near(pixel(&image, 50, 14), [188, 0, 0, 255], 4, "edge row");
    assert_near(
        pixel(&image, 16, 20),
        [255, 0, 0, 255],
        0,
        "round cap past the end",
    );
    assert_near(pixel(&image, 15, 15), BLACK, 0, "outside the cap's corner");
    Ok(())
}

#[test]
fn a_single_point_is_a_dot() -> TestResult {
    let dot = pen(&[[50.0, 30.0]], 10.0, RED);
    let image = render_shapes(&gpu()?, &scene_of(drawing(vec![dot])))?;
    assert_near(pixel(&image, 50, 30), [255, 0, 0, 255], 0, "centre");
    assert_near(
        pixel(&image, 53, 27),
        [255, 0, 0, 255],
        0,
        "inside the disc",
    );
    assert_near(pixel(&image, 54, 25), BLACK, 0, "corner of its box");
    Ok(())
}

#[test]
fn eraser_clears_earlier_strokes_of_its_drawing_but_not_what_lies_below() -> TestResult {
    let mut scene = Scene::new(size(100, 60));
    let blue = Shape {
        geometry: Geometry::Rect {
            rect: PxRect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 60.0,
            },
            radius: 0.0,
        },
        fill: Some(Paint::Solid(Rgba([0, 0, 255, 255]))),
        stroke: None,
        opacity: 1.0,
        rotation: 0.0,
        glow: None,
    };
    scene.push(Band::Video, Layer::Shapes(vec![blue]));
    let strokes = vec![
        pen(&[[10.0, 30.0], [90.0, 30.0]], 10.0, RED),
        eraser(&[[50.0, 10.0], [50.0, 50.0]], 12.0),
        pen(&[[70.0, 10.0], [70.0, 50.0]], 6.0, RED),
    ];
    scene.push(Band::Annotations, Layer::Drawing(drawing(strokes)));
    let image = render_shapes(&gpu()?, &scene)?;
    assert_near(
        pixel(&image, 30, 30),
        [255, 0, 0, 255],
        0,
        "line left of the eraser",
    );
    assert_near(
        pixel(&image, 50, 30),
        [0, 0, 255, 255],
        0,
        "erased: the frame below",
    );
    assert_near(
        pixel(&image, 50, 15),
        [0, 0, 255, 255],
        0,
        "eraser off the line",
    );
    assert_near(
        pixel(&image, 70, 15),
        [255, 0, 0, 255],
        0,
        "stroke after the eraser",
    );
    Ok(())
}

#[test]
fn translucent_stroke_does_not_darken_where_it_crosses_itself() -> TestResult {
    let half = Rgba([255, 0, 0, 128]);
    let zigzag = pen(
        &[[10.0, 10.0], [90.0, 50.0], [90.0, 10.0], [10.0, 50.0]],
        6.0,
        half,
    );
    let image = render_shapes(&gpu()?, &scene_of(drawing(vec![zigzag])))?;
    let single = pixel(&image, 30, 20);
    assert_near(single, [188, 0, 0, 255], 4, "one layer of half red");
    assert_near(pixel(&image, 50, 30), single, 0, "the crossing");
    assert_near(pixel(&image, 90, 30), single, 0, "the turn");
    Ok(())
}

#[test]
fn opacity_fades_the_drawing_as_a_whole() -> TestResult {
    let mut cross = drawing(vec![
        pen(&[[10.0, 30.0], [90.0, 30.0]], 8.0, RED),
        pen(&[[50.0, 5.0], [50.0, 55.0]], 8.0, RED),
    ]);
    cross.opacity = 0.5;
    let image = render_shapes(&gpu()?, &scene_of(cross))?;
    assert_near(pixel(&image, 30, 30), [188, 0, 0, 255], 4, "one stroke");
    assert_near(pixel(&image, 50, 30), pixel(&image, 30, 30), 0, "overlap");
    Ok(())
}

#[test]
fn draw_in_shows_the_first_part_of_the_path() -> TestResult {
    let mut line = drawing(vec![pen(&[[10.0, 30.0], [90.0, 30.0]], 6.0, RED)]);
    line.drawn = 0.5;
    let image = render_shapes(&gpu()?, &scene_of(line.clone()))?;
    assert_near(pixel(&image, 40, 30), [255, 0, 0, 255], 0, "drawn half");
    assert_near(pixel(&image, 60, 30), BLACK, 0, "not drawn yet");
    line.drawn = 0.0;
    let image = render_shapes(&gpu()?, &scene_of(line))?;
    assert!(image.rgba.chunks(4).all(|p| p == BLACK), "nothing at 0");
    Ok(())
}

#[test]
fn camera_moves_and_scales_the_strokes() -> TestResult {
    let line = pen(&[[10.0, 10.0], [30.0, 10.0]], 4.0, RED);
    let mut scene = scene_of(drawing(vec![line]));
    scene.camera = Camera {
        scale: 2.0,
        x: 0.0,
        y: 10.0,
    };
    let image = render_shapes(&gpu()?, &scene)?;
    assert_near(pixel(&image, 40, 30), [255, 0, 0, 255], 0, "moved line");
    assert_near(pixel(&image, 40, 33), [255, 0, 0, 255], 0, "width doubled");
    assert_near(pixel(&image, 20, 10), BLACK, 0, "old place");
    Ok(())
}

#[test]
fn more_than_255_strokes_all_show() -> TestResult {
    let dots = (0..300)
        .map(|i| {
            pen(
                &[[(i % 50) as f32 * 2.0 + 1.0, (i / 50) as f32 * 10.0 + 5.0]],
                1.5,
                RED,
            )
        })
        .collect();
    let image = render_shapes(&gpu()?, &scene_of(drawing(dots)))?;
    assert!(pixel(&image, 1, 5)[0] > 100, "first dot");
    assert!(pixel(&image, 99, 55)[0] > 100, "last dot");
    Ok(())
}
