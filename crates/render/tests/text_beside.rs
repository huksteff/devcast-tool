//! A line put next to a point (`Text::beside`, the cursor callout): its box sits after the point,
//! goes to the other side at an edge of `rect`, and is pushed inside when it fits neither way;
//! the plate takes a gradient and a glow.

mod common;

use std::error::Error;
use std::path::Path;

use common::{TestResult, gpu, pixel, size};
use dc_render::{
    Align, Band, Beside, Camera, Compositor, Font, Glow, Gradient, GradientColors, Image, Layer,
    Paint, Plate, PxRect, Rgba, Scene, Text, TextRun,
};

const W: f32 = 640.0;
const H: f32 = 160.0;
const BLUE: Rgba = Rgba([0, 0, 255, 255]);
const PAD: [f32; 2] = [10.0, 6.0];
const OFFSET: [f32; 2] = [20.0, 10.0];
const MARGIN: f32 = 8.0;

fn bubble(line: &str, point: [f32; 2], paint: Paint, glow: Option<Glow>) -> Text {
    Text {
        rect: PxRect {
            x: 0.0,
            y: 0.0,
            w: W,
            h: H,
        },
        runs: vec![TextRun {
            text: line.into(),
            color: Rgba([255, 255, 255, 255]),
        }],
        font: Font {
            family: "Inter".into(),
            size: 30.0,
            bold: true,
            italic: false,
        },
        underline: false,
        align: Align::Left,
        line_height: 1.4,
        padding: 0.0,
        plate: Some(Plate {
            paint,
            radius: 100.0,
            pad: PAD,
            glow,
        }),
        opacity: 1.0,
        beside: Some(Beside {
            point,
            offset: OFFSET,
            margin: MARGIN,
        }),
    }
}

fn render(text: Text, camera: Camera) -> Result<Image, Box<dyn Error>> {
    let gpu = gpu()?;
    let mut compositor = Compositor::new(&gpu);
    compositor.load_fonts(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/fonts"))?;
    let mut scene = Scene::new(size(W as u32, H as u32));
    scene.camera = camera;
    scene.push(Band::Cursor, Layer::Text(text));
    let target = gpu.target(scene.size);
    compositor.draw(&gpu, &scene, &target)?;
    Ok(gpu.read(&target)?)
}

/// Left, top, right, bottom (inclusive) of the pixels that are `lit`.
fn bounds(image: &Image, lit: impl Fn([u8; 4]) -> bool) -> Option<[u32; 4]> {
    let mut found: Option<[u32; 4]> = None;
    for y in 0..image.size.height {
        for x in 0..image.size.width {
            if lit(pixel(image, x, y)) {
                let b = found.get_or_insert([x, y, x, y]);
                *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
            }
        }
    }
    found
}

fn blue(p: [u8; 4]) -> bool {
    p[2] > 200 && p[0] < 60
}

/// The plate of a solid blue bubble at `point`.
fn plate_at(line: &str, point: [f32; 2]) -> Result<[u32; 4], Box<dyn Error>> {
    let image = render(
        bubble(line, point, Paint::Solid(BLUE), None),
        Camera::IDENTITY,
    )?;
    Ok(bounds(&image, blue).ok_or("no plate")?)
}

fn near(a: u32, b: f32) -> bool {
    (f64::from(a) - f64::from(b)).abs() <= 1.5
}

#[test]
fn box_starts_after_the_point() -> TestResult {
    let [l, t, r, b] = plate_at("Облачко", [100.0, 40.0])?;
    assert!(near(l, 120.0) && near(t, 50.0), "corner {l},{t}");
    assert!(
        r > l + 80 && b > t + 30,
        "a line with its pad: {l},{t}..{r},{b}"
    );
    Ok(())
}

#[test]
fn box_goes_left_and_up_at_the_edges() -> TestResult {
    let [l, t, r, _] = plate_at("Облачко", [600.0, 40.0])?;
    assert!(
        near(r + 1, 580.0),
        "right edge at the point minus offset: {l}..{r}"
    );
    assert!(near(t, 50.0), "still below: {t}");
    let [l, _, _, b] = plate_at("Облачко", [100.0, 140.0])?;
    assert!(near(b + 1, 130.0), "bottom at the point minus offset: {b}");
    assert!(near(l, 120.0), "still right of it: {l}");
    Ok(())
}

#[test]
fn box_that_fits_neither_way_is_pushed_inside() -> TestResult {
    let [l, _, r, _] = plate_at("Очень длинное облачко у курсора", [320.0, 40.0])?;
    assert!(
        near(r + 1, W - MARGIN),
        "pressed to the right margin: {l}..{r}"
    );
    Ok(())
}

#[test]
fn camera_moves_the_point_and_scales_the_box() -> TestResult {
    let plain = plate_at("Облачко", [100.0, 40.0])?;
    let camera = Camera {
        scale: 1.5,
        x: -50.0,
        y: -20.0,
    };
    let image = render(
        bubble("Облачко", [100.0, 40.0], Paint::Solid(BLUE), None),
        camera,
    )?;
    let [l, t, r, _] = bounds(&image, blue).ok_or("no plate")?;
    // Point at 100·1.5 − 50 = 100, offset 20·1.5.
    assert!(near(l, 130.0) && near(t, 55.0), "corner {l},{t}");
    let (w0, w1) = (plain[2] - plain[0], r - l);
    assert!(
        (f64::from(w1) / f64::from(w0) - 1.5).abs() < 0.05,
        "{w0} → {w1}"
    );
    Ok(())
}

#[test]
fn plate_takes_a_gradient_and_a_glow() -> TestResult {
    let colors = GradientColors::try_from(vec![Rgba([255, 0, 0, 255]), BLUE])?;
    let paint = Paint::Gradient(Gradient {
        colors,
        angle: 0.0,
        cycle: None,
    });
    let glow = Glow {
        color: Rgba([255, 255, 255, 255]),
        sigma: 6.0,
    };
    let image = render(
        bubble("Облачко", [100.0, 40.0], paint, Some(glow)),
        Camera::IDENTITY,
    )?;
    let red = |p: [u8; 4]| p[0] > 150 && p[2] < 120;
    let [rl, ..] = bounds(&image, red).ok_or("no red")?;
    let [bl, ..] = bounds(&image, blue).ok_or("no blue")?;
    assert!(rl < bl, "red on the left, blue on the right: {rl} {bl}");
    let above = pixel(&image, 160, 46);
    assert!(above[0] > 20, "glow above the plate: {above:?}");
    let far = pixel(&image, 160, 10);
    assert_eq!(far[..3], [0, 0, 0], "no glow far away");
    Ok(())
}
