mod common;

use common::{TestResult, assert_near, gpu, pixel, render_shapes, size};
use dc_render::{
    Band, Blur, Camera, Geometry, Glow, Image, Layer, Paint, PxRect, Rgba, Scene, Shape, Stroke,
};

const WHITE: Rgba = Rgba([255, 255, 255, 255]);
const BLACK: [u8; 4] = [0, 0, 0, 255];
const NONE: Rgba = Rgba([0, 0, 0, 0]);

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn filled(rect: PxRect, color: Rgba) -> Shape {
    Shape {
        geometry: Geometry::Rect { rect, radius: 0.0 },
        fill: Some(Paint::Solid(color)),
        stroke: None,
        opacity: 1.0,
        rotation: 0.0,
        glow: None,
        drawn: 1.0,
    }
}

fn glow(color: Rgba, sigma: f32) -> Option<Glow> {
    Some(Glow { color, sigma })
}

/// A 256×64 frame white left of x = 128, black right of it, with `layers` on top.
fn half_white(layers: Vec<Layer>) -> Scene {
    let mut scene = Scene::new(size(256, 64));
    let white = filled(rect(0.0, 0.0, 128.0, 64.0), WHITE);
    scene.push(Band::Video, Layer::Shapes(vec![white]));
    for layer in layers {
        scene.push(Band::Annotations, layer);
    }
    scene
}

fn blur(rect: PxRect, sigma: f32, tint: Rgba, radius: f32) -> Layer {
    Layer::Blur(Blur {
        rect,
        radius,
        sigma,
        tint,
        rotation: 0.0,
        opacity: 1.0,
    })
}

/// Normal cumulative distribution (Abramowitz–Stegun 7.1.26, error < 1.5e-7).
fn phi(x: f64) -> f64 {
    let z = x.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * z);
    let poly = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erf = 1.0 - poly * (-z * z).exp();
    if x >= 0.0 {
        0.5 * (1.0 + erf)
    } else {
        0.5 * (1.0 - erf)
    }
}

fn srgb_byte(linear: f64) -> u8 {
    let c = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round() as u8
}

fn grey(linear: f64) -> [u8; 4] {
    let v = srgb_byte(linear);
    [v, v, v, 255]
}

fn render(scene: &Scene) -> Result<Image, Box<dyn std::error::Error>> {
    render_shapes(&gpu()?, scene)
}

/// A white/black edge blurred by σ follows the gaussian's cumulative curve in linear light.
#[test]
fn region_blur_is_gaussian_in_linear_light() -> TestResult {
    for sigma in [2.0, 6.0, 12.0, 40.0] {
        let scene = half_white(vec![blur(rect(32.0, 0.0, 192.0, 64.0), sigma, NONE, 0.0)]);
        let image = render(&scene)?;
        for dx in [-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0] {
            let x = (128.0 + dx * f64::from(sigma)).floor();
            let expected = phi((128.0 - (x + 0.5)) / f64::from(sigma));
            assert_near(
                pixel(&image, x as u32, 32),
                grey(expected),
                5,
                &format!("σ {sigma}, x {x}"),
            );
        }
        assert_near(pixel(&image, 10, 32), [255; 4], 0, "left of the region");
        assert_near(pixel(&image, 250, 32), BLACK, 0, "right of the region");
    }
    Ok(())
}

/// Next to the frame edge the blur repeats the edge pixels: no dark rim from outside the frame.
#[test]
fn blur_at_the_frame_edge_has_no_dark_rim() -> TestResult {
    let scene = half_white(vec![blur(rect(-20.0, -20.0, 80.0, 104.0), 12.0, NONE, 0.0)]);
    let image = render(&scene)?;
    for (x, y) in [(0, 0), (0, 32), (30, 0), (59, 63)] {
        assert_near(pixel(&image, x, y), [255; 4], 1, &format!("{x},{y}"));
    }
    Ok(())
}

#[test]
fn tint_mixes_over_the_blur_and_corners_are_rounded() -> TestResult {
    let red_half = Rgba([255, 0, 0, 128]);
    let scene = half_white(vec![
        blur(
            rect(0.0, 0.0, 64.0, 64.0),
            4.0,
            Rgba([255, 0, 0, 255]),
            16.0,
        ),
        blur(rect(160.0, 0.0, 64.0, 64.0), 4.0, red_half, 0.0),
    ]);
    let image = render(&scene)?;
    assert_near(pixel(&image, 32, 32), [255, 0, 0, 255], 0, "solid tint");
    assert_near(pixel(&image, 1, 1), [255; 4], 0, "cut corner");
    // Half of black and red in linear light.
    let half = srgb_byte(128.0 / 255.0);
    assert_near(pixel(&image, 192, 32), [half, 0, 0, 255], 2, "half tint");
    Ok(())
}

#[test]
fn zero_sigma_leaves_the_picture_and_off_frame_blur_draws_nothing() -> TestResult {
    let plain = render(&half_white(vec![]))?;
    let sharp = render(&half_white(vec![blur(
        rect(64.0, 0.0, 128.0, 64.0),
        0.0,
        NONE,
        0.0,
    )]))?;
    assert_eq!(plain, sharp);
    let away = render(&half_white(vec![blur(
        rect(300.0, 0.0, 50.0, 64.0),
        8.0,
        NONE,
        0.0,
    )]))?;
    assert_eq!(plain, away);
    Ok(())
}

/// Under a ×2 camera the region and its sigma double: σ 3 of the canvas is σ 6 of the frame.
#[test]
fn camera_moves_and_scales_the_blur() -> TestResult {
    let mut scene = half_white(vec![blur(rect(64.0, 0.0, 96.0, 32.0), 3.0, NONE, 0.0)]);
    scene.camera = Camera {
        scale: 2.0,
        x: -128.0,
        y: 0.0,
    };
    let image = render(&scene)?;
    for x in [122.0, 128.0, 134.0] {
        let expected = phi((128.0 - (x + 0.5)) / 6.0);
        assert_near(
            pixel(&image, x as u32, 32),
            grey(expected),
            5,
            &format!("x {x}"),
        );
    }
    assert_near(pixel(&image, 230, 32), BLACK, 0, "right of the region");
    Ok(())
}

/// Glow light falls off around what is visible of the figure: an outline-only circle glows
/// along its ring and stays dark in the middle (decision 004).
#[test]
fn glow_follows_what_is_visible() -> TestResult {
    let ring = Shape {
        geometry: Geometry::Ellipse {
            rect: rect(32.0, 32.0, 192.0, 192.0),
        },
        fill: None,
        stroke: Some(Stroke {
            width: 4.0,
            color: WHITE,
        }),
        glow: glow(Rgba([0, 255, 0, 255]), 6.0),
        ..filled(rect(0.0, 0.0, 1.0, 1.0), WHITE)
    };
    let mut scene = Scene::new(size(256, 256));
    scene.push(Band::Annotations, Layer::Shapes(vec![ring]));
    let image = render(&scene)?;
    assert_near(pixel(&image, 128, 128), BLACK, 0, "middle");
    assert_near(pixel(&image, 128, 32), [255; 4], 0, "ring");
    let near = pixel(&image, 128, 22);
    assert!(
        near[1] > 60 && near[0] == 0,
        "glow outside the ring: {near:?}"
    );
    let inner = pixel(&image, 128, 42);
    assert!(
        inner[1] > 60 && inner[0] == 0,
        "glow inside the ring: {inner:?}"
    );
    Ok(())
}

/// A filled square's glow at distance d from a straight edge (far from the corners) is the
/// gaussian tail Φ(−d/σ) of the glow colour.
#[test]
fn glow_falls_off_as_a_gaussian_under_its_figure() -> TestResult {
    let sigma = 8.0;
    let square = Shape {
        glow: glow(WHITE, sigma),
        ..filled(rect(64.0, 64.0, 128.0, 128.0), Rgba([255, 0, 0, 255]))
    };
    let mut scene = Scene::new(size(256, 256));
    scene.push(Band::Annotations, Layer::Shapes(vec![square]));
    let image = render(&scene)?;
    assert_near(
        pixel(&image, 128, 128),
        [255, 0, 0, 255],
        0,
        "figure over its glow",
    );
    for d in [4.0, 8.0, 16.0] {
        let x = 192.0 + d;
        let expected = phi(-(d + 0.5) / f64::from(sigma));
        assert_near(
            pixel(&image, x as u32, 128),
            grey(expected),
            5,
            &format!("{d} px out"),
        );
    }
    Ok(())
}

/// The glow lies over the figures painted before its own figure.
#[test]
fn glow_covers_earlier_figures_only() -> TestResult {
    let blue = Rgba([0, 0, 255, 255]);
    let under = filled(rect(0.0, 0.0, 128.0, 64.0), blue);
    let glowing = Shape {
        glow: glow(Rgba([0, 255, 0, 255]), 8.0),
        ..filled(rect(100.0, 16.0, 56.0, 32.0), Rgba([255, 0, 0, 255]))
    };
    let over = filled(rect(156.0, 0.0, 100.0, 64.0), WHITE);
    let mut scene = Scene::new(size(256, 64));
    scene.push(Band::Annotations, Layer::Shapes(vec![under, glowing, over]));
    let image = render(&scene)?;
    let lit = pixel(&image, 96, 32);
    assert!(
        lit[1] > 60 && lit[2] > 100,
        "glow over the blue figure: {lit:?}"
    );
    assert_near(
        pixel(&image, 160, 32),
        [255; 4],
        0,
        "later figure over the glow",
    );
    Ok(())
}

fn turned(rect: PxRect, tint: Rgba, rotation: f32, opacity: f32) -> Layer {
    Layer::Blur(Blur {
        rect,
        radius: 0.0,
        sigma: 4.0,
        tint,
        rotation,
        opacity,
    })
}

/// A square turned by 45° around its centre is a diamond: its corners reach past the straight
/// square along the axes, and the straight square's corners are left out.
#[test]
fn turned_region_is_turned_around_its_centre() -> TestResult {
    let red = Rgba([255, 0, 0, 255]);
    let mut scene = Scene::new(size(256, 256));
    let square = rect(78.0, 78.0, 100.0, 100.0);
    scene.push(Band::Annotations, turned(square, red, 45.0, 1.0));
    let image = render(&scene)?;
    for (x, y) in [(128, 128), (128, 66), (66, 128), (190, 128), (128, 190)] {
        assert_near(
            pixel(&image, x, y),
            [255, 0, 0, 255],
            0,
            &format!("{x},{y}"),
        );
    }
    for (x, y) in [(82, 82), (174, 82), (82, 174), (174, 174)] {
        assert_near(pixel(&image, x, y), BLACK, 0, &format!("corner {x},{y}"));
    }
    Ok(())
}

/// A quarter turn of a wide region gives the tall one; a camera keeps the turn.
#[test]
fn quarter_turn_swaps_the_sides_under_the_camera() -> TestResult {
    let tint = Rgba([0, 255, 0, 160]);
    let frame = |layer: Layer, camera: Camera| -> Result<Image, Box<dyn std::error::Error>> {
        let mut scene = half_white(vec![layer]);
        scene.camera = camera;
        render(&scene)
    };
    let zoom = Camera {
        scale: 2.0,
        x: -128.0,
        y: -32.0,
    };
    for camera in [Camera::IDENTITY, zoom] {
        let wide = frame(
            turned(rect(96.0, 24.0, 64.0, 16.0), tint, 90.0, 1.0),
            camera,
        )?;
        let tall = frame(turned(rect(120.0, 0.0, 16.0, 64.0), tint, 0.0, 1.0), camera)?;
        let differ = wide
            .rgba
            .chunks(4)
            .zip(tall.rgba.chunks(4))
            .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 1))
            .count();
        assert_eq!(differ, 0, "pixels off by more than 1 under {camera:?}");
    }
    Ok(())
}

/// The opacity fades the blur and its tint together: half of a solid red over black is half red
/// in linear light, none leaves the frame as it was.
#[test]
fn opacity_fades_the_whole_region() -> TestResult {
    let red = Rgba([255, 0, 0, 255]);
    let square = rect(160.0, 0.0, 64.0, 64.0);
    let image = render(&half_white(vec![turned(square, red, 0.0, 0.5)]))?;
    let half = srgb_byte(0.5);
    assert_near(pixel(&image, 192, 32), [half, 0, 0, 255], 1, "half faded");
    let plain = render(&half_white(vec![]))?;
    let gone = render(&half_white(vec![turned(square, red, 30.0, 0.0)]))?;
    assert_eq!(plain, gone);
    Ok(())
}
