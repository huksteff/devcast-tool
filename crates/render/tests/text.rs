mod common;

use std::error::Error;
use std::path::Path;

use common::{TestResult, gpu, pixel, size};
use dc_render::{
    Align, Band, Camera, Compositor, Font, Image, Layer, Plate, PxRect, RenderError, Rgba, Scene,
    Text, TextRun,
};

const WHITE: Rgba = Rgba([255, 255, 255, 255]);

fn text(line: &str) -> Text {
    Text {
        rect: PxRect {
            x: 20.0,
            y: 20.0,
            w: 600.0,
            h: 120.0,
        },
        runs: vec![TextRun {
            text: line.into(),
            color: WHITE,
        }],
        font: Font {
            family: "Inter".into(),
            size: 40.0,
            bold: false,
            italic: false,
        },
        underline: false,
        align: Align::Left,
        line_height: 1.4,
        padding: 8.0,
        plate: None,
        opacity: 1.0,
    }
}

fn render(texts: Vec<Text>, camera: Camera) -> Result<Image, Box<dyn Error>> {
    let gpu = gpu()?;
    let mut compositor = Compositor::new(&gpu);
    compositor.load_fonts(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/fonts"))?;
    let mut scene = Scene::new(size(640, 160));
    scene.camera = camera;
    for text in texts {
        scene.push(Band::Annotations, Layer::Text(text));
    }
    let target = gpu.target(scene.size);
    compositor.draw(&gpu, &scene, &target)?;
    Ok(gpu.read(&target)?)
}

fn one(text: Text) -> Result<Image, Box<dyn Error>> {
    render(vec![text], Camera::IDENTITY)
}

/// Columns and rows that have any lit pixel: left, top, right, bottom (inclusive).
fn ink(image: &Image, lit: impl Fn([u8; 4]) -> bool) -> Option<[u32; 4]> {
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

fn bright(p: [u8; 4]) -> bool {
    p[0] > 128 && p[1] > 128 && p[2] > 128
}

/// Sum of brightness: how much ink a text has.
fn weight(image: &Image) -> u64 {
    image.rgba.chunks(4).map(|p| u64::from(p[0])).sum()
}

#[test]
fn cyrillic_text_is_drawn_inside_its_box_centred_vertically() -> TestResult {
    let image = one(text("Съешь ещё этих булок"))?;
    let [left, top, right, bottom] = ink(&image, bright).ok_or("no ink")?;
    assert!(
        (28..=34).contains(&left),
        "starts after the padding: {left}"
    );
    assert!(right > 300 && right < 620, "{right}");
    let middle = (top + bottom) / 2;
    assert!(
        (70..=90).contains(&middle),
        "centred in 20..140: {top}..{bottom}"
    );
    Ok(())
}

#[test]
fn box_cuts_the_text_and_long_lines_wrap() -> TestResult {
    let mut long = text("Очень длинная строка текста, которая не помещается в одну строку бокса");
    long.rect.h = 60.0;
    let image = one(long)?;
    let [_, top, right, bottom] = ink(&image, bright).ok_or("no ink")?;
    assert!(top >= 20 && bottom < 80, "cut by the box: {top}..{bottom}");
    assert!(right < 620, "wrapped: {right}");
    Ok(())
}

#[test]
fn bold_and_italic_change_the_glyphs() -> TestResult {
    let regular = one(text("Жирный и курсив"))?;
    let mut bold = text("Жирный и курсив");
    bold.font.bold = true;
    let bold = one(bold)?;
    assert!(
        weight(&bold) > weight(&regular) * 5 / 4,
        "bold has more ink: {} vs {}",
        weight(&bold),
        weight(&regular)
    );
    let mut italic = text("Жирный и курсив");
    italic.font.italic = true;
    assert_ne!(one(italic)?, regular);
    Ok(())
}

#[test]
fn alignment_moves_the_line() -> TestResult {
    let at = |align| -> Result<[u32; 4], Box<dyn Error>> {
        let mut t = text("Текст");
        t.align = align;
        Ok(ink(&one(t)?, bright).ok_or("no ink")?)
    };
    let [l, _, r, _] = at(Align::Left)?;
    let [cl, _, cr, _] = at(Align::Center)?;
    let [rl, _, rr, _] = at(Align::Right)?;
    assert!((28..=34).contains(&l), "{l}");
    assert!(
        (cl + cr) / 2 >= 315 && (cl + cr) / 2 <= 325,
        "centre {cl}..{cr}"
    );
    assert!((606..=612).contains(&rr), "{rr}");
    assert!((r - l).abs_diff(rr - rl) <= 1, "same width");
    Ok(())
}

#[test]
fn plate_sits_behind_each_line_and_underline_under_it() -> TestResult {
    let mut plated = text("Плашка");
    plated.plate = Some(Plate {
        color: Rgba([0, 0, 255, 255]),
        radius: 6.0,
        pad: [8.0, 4.0],
    });
    plated.underline = true;
    let image = one(plated)?;
    let blue = |p: [u8; 4]| p[2] > 200 && p[0] < 50;
    let [pl, pt, pr, pb] = ink(&image, blue).ok_or("no plate")?;
    let [tl, tt, tr, tb] = ink(&image, bright).ok_or("no text")?;
    assert!(
        pl + 6 <= tl && tr + 6 <= pr,
        "plate wider than the ink: {pl}..{pr} vs {tl}..{tr}"
    );
    assert!(pt < tt && tb < pb, "plate taller: {pt}..{pb} vs {tt}..{tb}");
    assert!(blue(pixel(&image, pl + 3, (pt + pb) / 2)), "plate edge");
    assert!(!blue(pixel(&image, pl, pt)), "rounded corner");
    // The underline: a white row across the whole word below the letters.
    let row = (tt..=tb)
        .rev()
        .find(|y| (tl + 2..tr - 2).all(|x| bright(pixel(&image, x, *y))))
        .ok_or("no underline row")?;
    assert!(row > (tt + tb) / 2, "underline at {row}, text {tt}..{tb}");
    Ok(())
}

#[test]
fn emoji_are_in_colour() -> TestResult {
    let image = one(text("🔥👍"))?;
    let coloured = |p: [u8; 4]| {
        let max = p[0].max(p[1]).max(p[2]);
        let min = p[0].min(p[1]).min(p[2]);
        max - min > 100
    };
    let [l, _, r, _] = ink(&image, coloured).ok_or("no colour")?;
    assert!(r - l > 40, "both emoji are coloured: {l}..{r}");
    Ok(())
}

#[test]
fn runs_keep_their_colours_and_opacity_fades_the_text() -> TestResult {
    let mut two = text("");
    two.runs = vec![
        TextRun {
            text: "Красный ".into(),
            color: Rgba([255, 0, 0, 255]),
        },
        TextRun {
            text: "зелёный".into(),
            color: Rgba([0, 255, 0, 255]),
        },
    ];
    let image = one(two.clone())?;
    let red = ink(&image, |p| p[0] > 200 && p[1] < 50).ok_or("no red")?;
    let green = ink(&image, |p| p[1] > 200 && p[0] < 50).ok_or("no green")?;
    assert!(red[2] < green[0], "red word first: {red:?} {green:?}");
    two.opacity = 0.5;
    let faded = one(two)?;
    assert!(
        ink(&faded, |p| p[0] > 200).is_none(),
        "half-transparent red is darker"
    );
    assert!(ink(&faded, |p| p[0] > 100).is_some());
    Ok(())
}

#[test]
fn camera_scales_the_text() -> TestResult {
    let small = ink(&one(text("Зум"))?, bright).ok_or("no ink")?;
    let zoomed = render(
        vec![text("Зум")],
        Camera {
            scale: 2.0,
            x: -20.0,
            y: -80.0,
        },
    )?;
    let big = ink(&zoomed, bright).ok_or("no ink")?;
    let (w, zw) = (small[2] - small[0], big[2] - big[0]);
    assert!(zw >= w * 2 - 4 && zw <= w * 2 + 4, "{w} → {zw}");
    Ok(())
}

#[test]
fn unknown_family_is_an_error() -> TestResult {
    let mut t = text("Текст");
    t.font.family = "Comic Sans MS".into();
    let error = one(t).err().ok_or("drawn")?;
    let error = error.downcast_ref::<RenderError>().ok_or("other error")?;
    assert!(matches!(error, RenderError::NoFont { family } if family == "Comic Sans MS"));
    Ok(())
}
