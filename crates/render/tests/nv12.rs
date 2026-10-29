//! Frames for the encoder: a rendered target to NV12 (BT.709 limited, chroma sited left) on the
//! GPU, read back through the ring; and back through the video pass to the same picture.

mod common;

use common::{TestResult, gpu, size};
use dc_core::VideoColor;
use dc_render::{
    Band, Compositor, Geometry, Gpu, GpuError, Gradient, Image, Layer, Nv12, Nv12Readback, Paint,
    PxRect, Rgba, Scene, Shape, Video,
};

const W: u32 = 96;
const H: u32 = 54;

fn bar(x: f32, w: f32, paint: Paint) -> Shape {
    Shape {
        geometry: Geometry::Rect {
            rect: PxRect {
                x,
                y: 0.0,
                w,
                h: H as f32,
            },
            radius: 0.0,
        },
        fill: Some(paint),
        stroke: None,
        opacity: 1.0,
        rotation: 0.0,
        glow: None,
    }
}

/// Hard colour edges on odd and even columns, and a gradient.
fn scene() -> Scene {
    let mut scene = Scene::new(size(W, H));
    let gradient = Gradient {
        colors: vec![Rgba([255, 200, 0, 255]), Rgba([0, 60, 255, 255])]
            .try_into()
            .unwrap_or_else(|e| panic!("{e}")),
        angle: 30.0,
        cycle: None,
    };
    let shapes = vec![
        bar(0.0, 21.0, Paint::Solid(Rgba([255, 0, 0, 255]))),
        bar(21.0, 20.0, Paint::Solid(Rgba([0, 255, 0, 255]))),
        bar(41.0, 15.0, Paint::Solid(Rgba([255, 255, 255, 255]))),
        bar(56.0, 40.0, Paint::Gradient(gradient)),
    ];
    scene.push(Band::Annotations, Layer::Shapes(shapes));
    scene
}

/// The rendered frame and its NV12 planes (tight rows) from the readback.
fn convert(gpu: &Gpu, scene: &Scene, frames: usize) -> Result<(Image, Vec<u8>, Vec<u8>), GpuError> {
    let target = gpu.target(scene.size);
    let mut compositor = Compositor::new(gpu);
    compositor
        .draw(gpu, scene, &target)
        .unwrap_or_else(|e| panic!("{e}"));
    let image = gpu.read(&target)?;
    let mut readback = Nv12Readback::new(gpu, &target);
    let mut got = Vec::new();
    let w = W as usize;
    let mut take = |planes: dc_render::Planes<'_>| -> Result<(), GpuError> {
        let rows = |plane: &[u8], n: usize| -> Vec<u8> {
            plane
                .chunks(planes.pitch)
                .take(n)
                .flat_map(|row| &row[..w])
                .copied()
                .collect()
        };
        got.push((rows(planes.y, H as usize), rows(planes.uv, H as usize / 2)));
        Ok(())
    };
    for _ in 0..frames {
        readback.push(gpu, &mut take)?;
    }
    readback.finish(gpu, &mut take)?;
    assert_eq!(got.len(), frames, "every pushed frame comes back once");
    let (y, uv) = got.swap_remove(0);
    Ok((image, y, uv))
}

fn rgb(image: &Image, x: i64, y: i64) -> [f64; 3] {
    let x = x.clamp(0, i64::from(W) - 1) as usize;
    let y = y.clamp(0, i64::from(H) - 1) as usize;
    let at = (y * W as usize + x) * 4;
    [0, 1, 2].map(|i| f64::from(image.rgba[at + i]) / 255.0)
}

fn luma([r, g, b]: [f64; 3]) -> f64 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

#[test]
fn planes_follow_bt709_limited_with_chroma_on_the_left_column() -> TestResult {
    let gpu = gpu()?;
    let (image, y, uv) = convert(&gpu, &scene(), 6)?;
    let mut worst = 0.0_f64;
    for row in 0..i64::from(H) {
        for col in 0..i64::from(W) {
            let expected = 16.0 + 219.0 * luma(rgb(&image, col, row));
            let actual = f64::from(y[(row * i64::from(W) + col) as usize]);
            worst = worst.max((actual - expected).abs());
        }
    }
    assert!(worst <= 0.6, "luma off by {worst}");
    let mut worst = 0.0_f64;
    for j in 0..i64::from(H) / 2 {
        for i in 0..i64::from(W) / 2 {
            let mut c = [0.0; 3];
            for (dx, weight) in [(-1, 1.0), (0, 2.0), (1, 1.0)] {
                for dy in 0..2 {
                    let p = rgb(&image, 2 * i + dx, 2 * j + dy);
                    (0..3).for_each(|k| c[k] += p[k] * weight / 8.0);
                }
            }
            let l = luma(c);
            let cb = 128.0 + 224.0 * (c[2] - l) / 1.8556;
            let cr = 128.0 + 224.0 * (c[0] - l) / 1.5748;
            let at = (j * i64::from(W) + 2 * i) as usize;
            worst = worst
                .max((f64::from(uv[at]) - cb).abs())
                .max((f64::from(uv[at + 1]) - cr).abs());
        }
    }
    assert!(worst <= 0.6, "chroma off by {worst}");
    Ok(())
}

#[test]
fn nv12_drawn_back_by_the_video_pass_gives_the_frame() -> TestResult {
    let gpu = gpu()?;
    let (image, y, uv) = convert(&gpu, &scene(), 1)?;
    let mut back = Scene::new(size(W, H));
    let source = common::source("media/export.mp4");
    let video = Video::fitted(source.clone(), size(W, H), dc_core::Rect::FULL, size(W, H));
    back.push(Band::Video, Layer::Video(video));
    let mut compositor = Compositor::new(&gpu);
    compositor.upload(
        &gpu,
        &source,
        Nv12::new(size(W, H), &y, &uv, VideoColor::BT709)?,
    );
    let shown = compositor.render(&gpu, &back)?;
    // Inside flat areas the colour survives up to rounding; at the edges 4:2:0 smears chroma.
    for (x, what) in [(10, "red"), (31, "green"), (48, "white"), (80, "gradient")] {
        for row in [2, H / 2, H - 3] {
            common::assert_near(
                common::pixel(&shown, x, row),
                common::pixel(&image, x, row),
                3,
                what,
            );
        }
    }
    Ok(())
}
