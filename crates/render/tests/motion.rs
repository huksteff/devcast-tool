mod common;

use std::error::Error;
use std::num::NonZeroU32;

use common::{Planes, TestResult, assert_near, gpu, pixel, render, size, video_scene};
use dc_core::Rect;
use dc_render::{
    Band, Bitmap, Camera, Compositor, Geometry, Gpu, Image, ImageKey, Layer, Motion, Paint, PxRect,
    Rgba, Scene, Shape, Sprite,
};

const BLACK: [u8; 3] = [16, 128, 128];
const WHITE: [u8; 3] = [235, 128, 128];

fn samples(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap_or(NonZeroU32::MIN)
}

/// A white 8×8 sprite at (`x`, 4) in a 64×16 frame, smeared from `from`.
fn white_square(x: f32, motion: Option<Motion<[f32; 2]>>) -> Scene {
    let mut scene = Scene::new(size(64, 16));
    let sprite = Sprite {
        image: ImageKey {
            path: "white.png".into(),
            frame: 0,
        },
        rect: PxRect {
            x,
            y: 4.0,
            w: 8.0,
            h: 8.0,
        },
        rotation: 0.0,
        opacity: 1.0,
        motion,
    };
    scene.push(Band::Cursor, Layer::Sprite(sprite));
    scene
}

fn render_sprites(gpu: &Gpu, scene: &Scene) -> Result<Image, Box<dyn Error>> {
    let mut compositor = Compositor::new(gpu);
    let white = vec![255; 8 * 8 * 4];
    let key = ImageKey {
        path: "white.png".into(),
        frame: 0,
    };
    compositor.upload_image(gpu, &key, Bitmap::new(size(8, 8), &white)?)?;
    let target = gpu.target(scene.size);
    compositor.draw(gpu, scene, &target)?;
    Ok(gpu.read(&target)?)
}

fn linear(byte: u8) -> f64 {
    let c = f64::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Opaque sRGB grey of `light` in linear light.
fn gray(light: f64) -> [u8; 4] {
    let byte = ((light.powf(1.0 / 2.4) * 1.055 - 0.055) * 255.0).round() as u8;
    [byte, byte, byte, 255]
}

#[test]
fn a_still_sprite_with_motion_blur_is_exactly_the_sharp_one() -> TestResult {
    let gpu = gpu()?;
    let sharp = render_sprites(&gpu, &white_square(20.25, None))?;
    let still = Motion {
        from: [20.25, 4.0],
        samples: samples(16),
    };
    let blurred = render_sprites(&gpu, &white_square(20.25, Some(still)))?;
    assert_eq!(sharp, blurred);
    Ok(())
}

#[test]
fn moving_sprite_is_smeared_evenly_along_its_path_without_losing_light() -> TestResult {
    let gpu = gpu()?;
    // 41 samples 1 px apart: from x 10 to x 50, the square covers 10..58.
    let motion = Motion {
        from: [10.0, 4.0],
        samples: samples(41),
    };
    let image = render_sprites(&gpu, &white_square(50.0, Some(motion)))?;
    // Column 30 lies under 8 of the 41 squares (those starting at 23..=30).
    assert_near(pixel(&image, 30, 8), gray(8.0 / 41.0), 2, "middle");
    assert_near(pixel(&image, 30, 2), [0, 0, 0, 255], 0, "above the path");
    assert_near(pixel(&image, 8, 8), [0, 0, 0, 255], 0, "behind the start");
    let light: f64 = image.rgba.chunks(4).map(|p| linear(p[0])).sum();
    assert!(
        (light - 64.0).abs() < 0.64,
        "light {light}, the square has 64"
    );
    Ok(())
}

fn stripe() -> Planes {
    Planes::new(size(64, 16), |x, _| {
        if (28..36).contains(&x) { WHITE } else { BLACK }
    })
}

#[test]
fn a_still_camera_with_motion_blur_is_exactly_the_sharp_one() -> TestResult {
    let gpu = gpu()?;
    let planes = stripe();
    let mut scene = video_scene(size(64, 16), &planes, Rect::FULL);
    scene.camera = Camera {
        scale: 1.5,
        x: -16.3,
        y: -4.0,
    };
    let sharp = render(&gpu, &scene, &planes)?;
    scene.camera_motion = Some(Motion {
        from: scene.camera,
        samples: samples(9),
    });
    assert_eq!(render(&gpu, &scene, &planes)?, sharp);
    Ok(())
}

#[test]
fn camera_pan_smears_the_video_but_not_the_layers_over_it() -> TestResult {
    let gpu = gpu()?;
    let planes = stripe();
    let mut scene = video_scene(size(64, 16), &planes, Rect::FULL);
    // The stripe (28..36) seen 16 px further left at the opening: samples 2 px apart.
    scene.camera_motion = Some(Motion {
        from: Camera {
            scale: 1.0,
            x: -16.0,
            y: 0.0,
        },
        samples: samples(9),
    });
    let dot = Shape {
        geometry: Geometry::Rect {
            rect: PxRect {
                x: 4.0,
                y: 4.0,
                w: 4.0,
                h: 4.0,
            },
            radius: 0.0,
        },
        fill: Some(Paint::Solid(Rgba([255, 0, 0, 255]))),
        stroke: None,
        opacity: 1.0,
        rotation: 0.0,
        glow: None,
    };
    scene.push(Band::Annotations, Layer::Shapes(vec![dot]));
    let image = render(&gpu, &scene, &planes)?;
    // Column 21 lies under the stripe of 4 of the 9 samples (shifts 8..=14 px back).
    assert_near(pixel(&image, 21, 8), gray(4.0 / 9.0), 2, "smear");
    assert_near(
        pixel(&image, 33, 8),
        gray(2.0 / 9.0),
        2,
        "right end of the stripe",
    );
    assert_near(pixel(&image, 5, 5), [255, 0, 0, 255], 0, "sharp shape");
    assert_near(pixel(&image, 3, 5), [0, 0, 0, 255], 0, "next to the shape");
    Ok(())
}
