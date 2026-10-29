//! Reference frames of every pass (decision 012), rendered on the software adapter (WARP on
//! Windows) like CI. A frame that changes on purpose is updated with
//! `cargo xtask golden update "<почему>"`.

mod common;

use std::error::Error;
use std::num::NonZeroU32;
use std::path::Path;

use common::{Planes, TestResult, size};
use dc_core::{PixelSize, Rect};
use dc_render::golden::Golden;
use dc_render::{
    AdapterChoice, Align, Band, Bitmap, Blur, BrushStroke, Camera, Compositor, Drawing, Font,
    Geometry, Glow, Gpu, Gradient, ImageKey, Layer, Motion, Paint, Plate, PxRect, Rgba, Scene,
    Shape, Sprite, Stroke, Text, TextRun, Tool, Video,
};

const SIZE: PixelSize = PixelSize {
    width: 320,
    height: 180,
};
const TEAL: Rgba = Rgba([0x14, 0xb8, 0xc4, 255]);
const VIOLET: Rgba = Rgba([0xa8, 0x55, 0xf7, 255]);
const WHITE: Rgba = Rgba([255, 255, 255, 255]);

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn shape(geometry: Geometry, fill: Option<Paint>) -> Shape {
    Shape {
        geometry,
        fill,
        stroke: None,
        opacity: 1.0,
        rotation: 0.0,
        glow: None,
    }
}

fn gradient(colors: Vec<Rgba>, angle: f32) -> Paint {
    Paint::Gradient(Gradient {
        colors: colors.try_into().unwrap_or_else(|e| panic!("{e}")),
        angle,
        cycle: None,
    })
}

/// Colour bars over a luma ramp, 4:3, so the 16:9 frame gets fields.
fn bars() -> Planes {
    const YUV: [[u8; 3]; 6] = [
        [235, 128, 128],
        [63, 102, 240],
        [173, 42, 26],
        [32, 240, 118],
        [126, 128, 128],
        [16, 128, 128],
    ];
    Planes::new(size(160, 120), |x, y| match y {
        0..80 => YUV[(x / 27) as usize],
        _ => [(16 + x * 219 / 159) as u8, 128, 128],
    })
}

fn checker() -> Vec<Shape> {
    (0..8 * 5)
        .filter(|i| (i % 8 + i / 8) % 2 == 0)
        .map(|i| {
            let geometry = Geometry::Rect {
                rect: rect((i % 8 * 40) as f32, (i / 8 * 36) as f32, 40.0, 36.0),
                radius: 0.0,
            };
            shape(geometry, Some(Paint::Solid(WHITE)))
        })
        .collect()
}

/// A 32×32 disc fading from opaque white at the centre to transparent at the rim.
fn disc() -> Vec<u8> {
    (0..32 * 32)
        .flat_map(|i| {
            let (x, y) = ((i % 32) as f32 - 15.5, (i / 32) as f32 - 15.5);
            let alpha = (1.0 - x.hypot(y) / 16.0).max(0.0);
            [255, 200, 40, (alpha * 255.0) as u8]
        })
        .collect()
}

fn disc_key() -> ImageKey {
    ImageKey {
        path: "assets/disc.png".into(),
        frame: 0,
    }
}

/// Renders `scene` on the software adapter with the bars as every video, the disc as every
/// sprite and the built-in fonts, and checks it against reference `render/<name>`.
fn check(name: &str, scene: &Scene) -> TestResult {
    let gpu = Gpu::new(AdapterChoice::Software)?;
    let mut compositor = Compositor::new(&gpu);
    compositor.load_fonts(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/fonts"))?;
    let planes = bars();
    for (_, layer) in scene.painted() {
        if let Layer::Video(video) = layer {
            compositor.upload(&gpu, &video.source, planes.nv12()?);
        }
    }
    let disc = disc();
    compositor.upload_image(&gpu, &disc_key(), Bitmap::new(size(32, 32), &disc)?)?;
    let image = compositor.render(&gpu, scene)?;
    Golden::new(&gpu)?
        .check(&format!("render/{name}"), &image)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn video_scene() -> Scene {
    let mut scene = Scene::new(SIZE);
    let video = Video::fitted(
        common::source("media/a.mp4"),
        size(160, 120),
        Rect::FULL,
        SIZE,
    );
    scene.push(Band::Video, Layer::Video(video));
    scene
}

#[test]
fn golden_video() -> TestResult {
    let mut scene = video_scene();
    scene.camera = Camera {
        scale: 1.25,
        x: -40.0,
        y: -22.5,
    };
    check("video", &scene)
}

#[test]
fn golden_shapes() -> TestResult {
    let mut scene = Scene::new(SIZE);
    let card = Shape {
        stroke: Some(Stroke {
            width: 4.0,
            color: WHITE,
        }),
        ..shape(
            Geometry::Rect {
                rect: rect(16.0, 16.0, 120.0, 70.0),
                radius: 14.0,
            },
            Some(gradient(
                vec![TEAL, VIOLET, Rgba([255, 120, 60, 255])],
                30.0,
            )),
        )
    };
    let ellipse = Shape {
        opacity: 0.7,
        rotation: 20.0,
        ..shape(
            Geometry::Ellipse {
                rect: rect(170.0, 20.0, 120.0, 60.0),
            },
            Some(Paint::Solid(VIOLET)),
        )
    };
    let line = shape(
        Geometry::Line {
            from: [30.0, 150.0],
            to: [140.0, 110.0],
            width: 6.0,
        },
        Some(Paint::Solid(WHITE)),
    );
    let arrow = Shape {
        glow: Some(Glow {
            color: TEAL,
            sigma: 6.0,
        }),
        ..shape(
            Geometry::Arrow {
                from: [180.0, 150.0],
                to: [290.0, 110.0],
                width: 6.0,
                head: 22.0,
            },
            Some(Paint::Solid(TEAL)),
        )
    };
    scene.push(
        Band::Annotations,
        Layer::Shapes(vec![card, ellipse, line, arrow]),
    );
    check("shapes", &scene)
}

#[test]
fn golden_blur() -> TestResult {
    let mut scene = Scene::new(SIZE);
    scene.push(Band::Video, Layer::Shapes(checker()));
    for (x, tint) in [
        (20.0, Rgba([0, 0, 0, 0])),
        (170.0, Rgba([20, 40, 120, 110])),
    ] {
        let blur = Blur {
            rect: rect(x, 30.0, 130.0, 120.0),
            radius: 12.0,
            sigma: 8.0,
            tint,
        };
        scene.push(Band::Annotations, Layer::Blur(blur));
    }
    check("blur", &scene)
}

#[test]
fn golden_text() -> TestResult {
    let mut scene = Scene::new(SIZE);
    let text = Text {
        rect: rect(10.0, 30.0, 300.0, 120.0),
        runs: vec![
            TextRun {
                text: "Привет, DevCast! ".into(),
                color: WHITE,
            },
            TextRun {
                text: "Кадр 🎬".into(),
                color: TEAL,
            },
        ],
        font: Font {
            family: "Inter".into(),
            size: 30.0,
            bold: true,
            italic: false,
        },
        underline: true,
        align: Align::Center,
        line_height: 1.4,
        padding: 8.0,
        plate: Some(Plate {
            paint: Paint::Solid(Rgba([40, 40, 60, 220])),
            radius: 8.0,
            pad: [8.0, 4.0],
            glow: None,
        }),
        opacity: 1.0,
        beside: None,
    };
    scene.push(Band::Overlay, Layer::Text(text));
    check("text", &scene)
}

#[test]
fn golden_sprites() -> TestResult {
    let mut scene = Scene::new(SIZE);
    scene.push(Band::Video, Layer::Shapes(checker()));
    for (x, w, rotation, opacity) in [
        (20.0, 64.0, 0.0, 1.0),
        (120.0, 96.0, 30.0, 1.0),
        (240.0, 12.0, 0.0, 0.5),
    ] {
        let sprite = Sprite {
            image: disc_key(),
            rect: rect(x, 50.0, w, w),
            rotation,
            opacity,
            motion: None,
        };
        scene.push(Band::Annotations, Layer::Sprite(sprite));
    }
    check("sprites", &scene)
}

#[test]
fn golden_brush() -> TestResult {
    let mut scene = video_scene();
    let wave = (0..=40)
        .map(|i| [20.0 + i as f32 * 7.0, 90.0 + 40.0 * (i as f32 * 0.35).sin()])
        .collect();
    let strokes = vec![
        BrushStroke {
            points: wave,
            width: 10.0,
            tool: Tool::Pen(Rgba([0x25, 0x63, 0xeb, 255])),
        },
        BrushStroke {
            points: vec![[160.0, 10.0], [160.0, 170.0]],
            width: 20.0,
            tool: Tool::Eraser,
        },
        BrushStroke {
            points: vec![[280.0, 30.0]],
            width: 24.0,
            tool: Tool::Pen(Rgba([250, 220, 40, 255])),
        },
        BrushStroke {
            points: vec![[30.0, 30.0], [290.0, 160.0], [290.0, 30.0], [30.0, 160.0]],
            width: 8.0,
            tool: Tool::Pen(Rgba([255, 80, 80, 150])),
        },
    ];
    // The eraser clears only the wave; the last stroke crosses its gap, cut by draw-in.
    let drawing = Drawing {
        strokes,
        drawn: 0.92,
        opacity: 0.9,
    };
    scene.push(Band::Annotations, Layer::Drawing(drawing));
    check("brush", &scene)
}

#[test]
fn golden_motion_blur() -> TestResult {
    let mut scene = video_scene();
    scene.camera = Camera {
        scale: 1.5,
        x: -120.0,
        y: -45.0,
    };
    scene.camera_motion = Some(Motion {
        from: Camera {
            scale: 1.2,
            x: -40.0,
            y: -18.0,
        },
        samples: NonZeroU32::new(12).ok_or("12 samples")?,
    });
    let cursor = Sprite {
        image: disc_key(),
        rect: rect(150.0, 70.0, 24.0, 24.0),
        rotation: 0.0,
        opacity: 1.0,
        motion: Some(Motion {
            from: [110.0, 90.0],
            samples: NonZeroU32::new(16).ok_or("16 samples")?,
        }),
    };
    scene.push(Band::Cursor, Layer::Sprite(cursor));
    check("motion_blur", &scene)
}

#[test]
fn golden_frames_come_from_one_adapter() -> Result<(), Box<dyn Error>> {
    let gpu = Gpu::new(AdapterChoice::Software)?;
    println!(
        "adapter of the reference frames: {}",
        Golden::new(&gpu)?.adapter()
    );
    Ok(())
}
