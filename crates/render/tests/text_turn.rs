//! Turned text (`Text::rotation`): drawn upright into a layer and laid over the frame turned
//! around its box centre.

mod common;

use std::error::Error;
use std::path::Path;

use common::{TestResult, gpu, pixel, size};
use dc_render::{
    Align, Band, Camera, Compositor, Font, Image, Layer, Paint, Plate, PxRect, Rgba, Scene, Text,
    TextRun,
};

const SIDE: u32 = 320;

/// A white line on a red plate in a 200×60 box centred in a 320×320 frame.
fn text(line: &str, rotation: f32) -> Text {
    Text {
        rect: PxRect {
            x: 60.0,
            y: 130.0,
            w: 200.0,
            h: 60.0,
        },
        runs: vec![TextRun {
            text: line.into(),
            color: Rgba([255, 255, 255, 255]),
        }],
        font: Font {
            family: "Inter".into(),
            size: 28.0,
            bold: true,
            italic: false,
        },
        underline: false,
        align: Align::Center,
        line_height: 1.4,
        padding: 8.0,
        plate: Some(Plate {
            paint: Paint::Solid(Rgba([220, 0, 0, 255])),
            radius: 4.0,
            pad: [6.0, 3.0],
            glow: None,
        }),
        opacity: 1.0,
        beside: None,
        shadow: None,
        rotation,
    }
}

fn render(text: Text, camera: Camera) -> Result<Image, Box<dyn Error>> {
    let gpu = gpu()?;
    let mut compositor = Compositor::new(&gpu);
    compositor.load_fonts(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/fonts"))?;
    let mut scene = Scene::new(size(SIDE, SIDE));
    scene.camera = camera;
    scene.push(Band::Annotations, Layer::Text(text));
    let target = gpu.target(scene.size);
    compositor.draw(&gpu, &scene, &target)?;
    Ok(gpu.read(&target)?)
}

fn one(text: Text) -> Result<Image, Box<dyn Error>> {
    render(text, Camera::IDENTITY)
}

fn red(p: [u8; 4]) -> bool {
    p[0] > 100 && p[1] < 60
}

/// Left, top, right, bottom (inclusive) of the pixels that pass `lit`.
fn bounds(image: &Image, lit: impl Fn([u8; 4]) -> bool) -> Option<[u32; 4]> {
    let mut bounds: Option<[u32; 4]> = None;
    for y in 0..image.size.height {
        for x in 0..image.size.width {
            if lit(pixel(image, x, y)) {
                let b = bounds.get_or_insert([x, y, x, y]);
                *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
            }
        }
    }
    bounds
}

/// Whether `b` is `a` turned half round the frame centre. Letters may differ at a few edge
/// pixels: a glyph drawn at another place of the layer can fall into a neighbouring subpixel bin
/// of the glyph cache (a quarter pixel), so up to 0.1 % of the channels may be off by more than 1.
fn upside_down(a: &Image, b: &Image) -> bool {
    let mut off = 0;
    for y in 0..SIDE {
        for x in 0..SIDE {
            let p = pixel(a, x, y);
            let q = pixel(b, SIDE - 1 - x, SIDE - 1 - y);
            off += (0..4).filter(|i| p[*i].abs_diff(q[*i]) > 1).count();
        }
    }
    off * 1000 <= (SIDE * SIDE * 4) as usize
}

#[test]
fn a_quarter_turn_stands_the_plate_up_around_the_box_centre() -> TestResult {
    let [l, t, r, b] = bounds(&one(text("Привет", 0.0))?, red).ok_or("no plate")?;
    let [tl, tt, tr, tb] = bounds(&one(text("Привет", 90.0))?, red).ok_or("no turned plate")?;
    let near = |a: u32, b: u32| a.abs_diff(b) <= 2;
    assert!(
        near(tr - tl, b - t) && near(tb - tt, r - l),
        "sides swap: {l},{t},{r},{b} → {tl},{tt},{tr},{tb}"
    );
    assert!(near(tl + tr, l + r) && near(tt + tb, t + b), "same centre");
    Ok(())
}

#[test]
fn a_half_turn_is_the_upright_text_upside_down() -> TestResult {
    let upright = one(text("Поворот 180°", 0.0))?;
    let turned = one(text("Поворот 180°", 180.0))?;
    assert!(upside_down(&upright, &turned));
    Ok(())
}

#[test]
fn whole_turns_draw_the_text_upright() -> TestResult {
    let upright = one(text("Ровно", 0.0))?;
    assert_eq!(one(text("Ровно", 360.0))?.rgba, upright.rgba);
    assert_eq!(one(text("Ровно", -720.0))?.rgba, upright.rgba);
    Ok(())
}

#[test]
fn a_clockwise_turn_lowers_the_right_end() -> TestResult {
    let image = one(text("Наклон", 20.0))?;
    let column = |x: u32| (0..SIDE).filter(|y| red(pixel(&image, x, *y))).min();
    let left = column(125).ok_or("no plate at x 125")?;
    let right = column(195).ok_or("no plate at x 195")?;
    assert!(
        right > left + 15,
        "top of the plate: {left} at left, {right} at right"
    );
    Ok(())
}

#[test]
fn a_box_off_the_frame_turns_into_it() -> TestResult {
    // The box lies right of the frame with its centre on the right edge; a quarter turn stands
    // its left half up inside the frame.
    let mut off = text("Сбоку", 90.0);
    off.rect = PxRect {
        x: 220.0,
        y: 145.0,
        w: 200.0,
        h: 30.0,
    };
    let upright = one(Text {
        rotation: 0.0,
        ..off.clone()
    })?;
    let [l, ..] = bounds(&upright, red).ok_or("no plate")?;
    let [tl, tt, tr, tb] = bounds(&one(off)?, red).ok_or("no turned plate")?;
    assert!(l > 230, "upright, the plate starts at {l}");
    assert!(tr == SIDE - 1 && tl > 290, "{tl}..{tr}");
    assert!(tt < 125 && tb > 195, "stands up: {tt}..{tb}");
    Ok(())
}

#[test]
fn the_camera_moves_and_scales_the_turned_text() -> TestResult {
    let camera = Camera {
        scale: 2.0,
        x: -160.0,
        y: -160.0,
    };
    let [l, t, r, b] = bounds(&render(text("Зум", 90.0), camera)?, red).ok_or("no plate")?;
    let [ul, ut, ur, ub] = bounds(&one(text("Зум", 90.0))?, red).ok_or("no plate")?;
    let near = |a: u32, b: u32| a.abs_diff(b) <= 3;
    assert!(near(l + r, SIDE) && near(t + b, SIDE), "centre stays");
    assert!(near(r - l, 2 * (ur - ul)) || r - l >= SIDE - 1, "{l}..{r}");
    assert!(near(b - t, 2 * (ub - ut)), "{t}..{b} twice {ut}..{ub}");
    Ok(())
}

#[test]
fn a_box_bigger_than_the_frame_is_drawn_in_tiles_without_seams() -> TestResult {
    // The box is three frames wide; its huge plate covers the frame, and the letters fill it.
    let mut huge = text("ЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖ", 180.0);
    huge.rect = PxRect {
        x: -320.0,
        y: -320.0,
        w: 960.0,
        h: 960.0,
    };
    huge.font.size = 140.0;
    let upright = one(Text {
        rotation: 0.0,
        ..huge.clone()
    })?;
    let turned = one(huge)?;
    assert!(bounds(&upright, red).is_some());
    assert!(upside_down(&upright, &turned));
    Ok(())
}
