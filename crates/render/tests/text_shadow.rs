//! `Text::shadow`: the glyphs in the shadow colour, moved and blurred, under the text.

mod common;

use std::error::Error;
use std::path::Path;

use common::{TestResult, gpu, pixel, size};
use dc_render::{
    Align, Band, Camera, Compositor, Font, Geometry, Image, Layer, Paint, PxRect, Rgba, Scene,
    Shadow, Shape, Text, TextRun,
};

const RED: Rgba = Rgba([255, 0, 0, 255]);

fn render(shadow: Option<Shadow>, camera: Camera) -> Result<Image, Box<dyn Error>> {
    let gpu = gpu()?;
    let mut compositor = Compositor::new(&gpu);
    compositor.load_fonts(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/fonts"))?;
    let mut scene = Scene::new(size(400, 160));
    scene.camera = camera;
    let rect = PxRect {
        x: 0.0,
        y: 0.0,
        w: 400.0,
        h: 160.0,
    };
    let white = Shape {
        geometry: Geometry::Rect { rect, radius: 0.0 },
        fill: Some(Paint::Solid(Rgba([255; 4]))),
        stroke: None,
        opacity: 1.0,
        rotation: 0.0,
        glow: None,
    };
    scene.push(Band::Annotations, Layer::Shapes(vec![white]));
    let text = Text {
        rect,
        runs: vec![TextRun {
            text: "Тень".into(),
            color: RED,
        }],
        font: Font {
            family: "Inter".into(),
            size: 60.0,
            bold: true,
            italic: false,
        },
        underline: false,
        align: Align::Center,
        line_height: 1.4,
        padding: 0.0,
        plate: None,
        opacity: 1.0,
        beside: None,
        shadow,
    };
    scene.push(Band::Annotations, Layer::Text(text));
    let target = gpu.target(scene.size);
    compositor.draw(&gpu, &scene, &target)?;
    Ok(gpu.read(&target)?)
}

/// Pixels of the frame that are clearly grey (shadow on the white), not red letters.
fn grey(image: &Image) -> Vec<(u32, u32)> {
    let mut found = Vec::new();
    for y in 0..image.size.height {
        for x in 0..image.size.width {
            let [r, g, b, _] = pixel(image, x, y);
            if r < 200 && g < 200 && b < 200 && r.abs_diff(g) < 20 {
                found.push((x, y));
            }
        }
    }
    found
}

/// Lowest row of the red letters.
fn letters_bottom(image: &Image) -> u32 {
    let mut bottom = 0;
    for y in 0..image.size.height {
        for x in 0..image.size.width {
            let [r, g, _, _] = pixel(image, x, y);
            if r > 200 && g < 80 {
                bottom = bottom.max(y);
            }
        }
    }
    bottom
}

fn shadow() -> Shadow {
    Shadow {
        color: Rgba([0, 0, 0, 255]),
        sigma: 2.0,
        offset: [0.0, 8.0],
    }
}

#[test]
fn shadow_lies_under_the_letters_moved_down() -> TestResult {
    let plain = render(None, Camera::IDENTITY)?;
    assert!(grey(&plain).is_empty(), "no shadow without one");
    let shaded = render(Some(shadow()), Camera::IDENTITY)?;
    let grey = grey(&shaded);
    assert!(!grey.is_empty(), "a shadow");
    let bottom = letters_bottom(&shaded);
    let lowest = grey.iter().map(|p| p.1).max().ok_or("no shadow")?;
    assert!(
        lowest > bottom + 4 && lowest <= bottom + 8 + 3 * 2,
        "shadow reaches below the letters by the offset and the blur: {lowest} vs {bottom}"
    );
    assert_eq!(
        letters_bottom(&plain),
        bottom,
        "the letters themselves stay put"
    );
    Ok(())
}

#[test]
fn camera_moves_and_scales_the_shadow() -> TestResult {
    let camera = Camera {
        scale: 1.5,
        x: -100.0,
        y: -40.0,
    };
    let shaded = render(Some(shadow()), camera)?;
    let bottom = letters_bottom(&shaded);
    let lowest = grey(&shaded).iter().map(|p| p.1).max().ok_or("no shadow")?;
    assert!(
        lowest > bottom + 8 && lowest <= bottom + 12 + 9,
        "offset 8 × 1.5 below: {lowest} vs {bottom}"
    );
    Ok(())
}
