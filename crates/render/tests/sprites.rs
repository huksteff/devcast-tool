mod common;

use std::error::Error;
use std::path::Path;
use std::process::Command;

use common::{TestResult, assert_near, gpu, pixel, size};
use dc_core::PixelSize;
use dc_media::decode_picture;
use dc_render::{
    Band, Bitmap, Camera, Compositor, Geometry, Gpu, Image, ImageKey, Layer, Paint, PxRect,
    RenderError, Rgba, Scene, Shape, Sprite,
};

const BLACK: [u8; 4] = [0, 0, 0, 255];

fn key(path: &str, frame: u32) -> ImageKey {
    ImageKey {
        path: path.into(),
        frame,
    }
}

fn sprite(image: ImageKey, rect: PxRect) -> Sprite {
    Sprite {
        image,
        rect,
        rotation: 0.0,
        opacity: 1.0,
        motion: None,
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

/// A `w`×`h` picture with `rgba(x, y)` per pixel.
fn bitmap(w: u32, h: u32, rgba: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .flat_map(|(x, y)| rgba(x, y))
        .collect()
}

fn draw(gpu: &Gpu, compositor: &mut Compositor, scene: &Scene) -> Result<Image, Box<dyn Error>> {
    let target = gpu.target(scene.size);
    compositor.draw(gpu, scene, &target)?;
    Ok(gpu.read(&target)?)
}

/// `pictures` uploaded, then a 64×64 scene of `layers` drawn.
fn render(
    pictures: &[(&str, PixelSize, Vec<u8>)],
    layers: Vec<Layer>,
) -> Result<Image, Box<dyn Error>> {
    let gpu = gpu()?;
    let mut compositor = Compositor::new(&gpu);
    for (path, size, rgba) in pictures {
        compositor.upload_image(&gpu, &key(path, 0), Bitmap::new(*size, rgba)?)?;
    }
    let mut scene = Scene::new(size(64, 64));
    for layer in layers {
        scene.push(Band::Annotations, layer);
    }
    draw(&gpu, &mut compositor, &scene)
}

fn quadrants() -> (&'static str, PixelSize, Vec<u8>) {
    let colours = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 255],
    ];
    (
        "q.png",
        size(2, 2),
        bitmap(2, 2, |x, y| colours[(y * 2 + x) as usize]),
    )
}

#[test]
fn picture_is_stretched_over_its_rectangle() -> TestResult {
    let layer = Layer::Sprite(sprite(key("q.png", 0), rect(16.0, 16.0, 32.0, 32.0)));
    let image = render(&[quadrants()], vec![layer])?;
    assert_near(pixel(&image, 18, 18), [255, 0, 0, 255], 0, "top left");
    assert_near(pixel(&image, 45, 18), [0, 255, 0, 255], 0, "top right");
    assert_near(pixel(&image, 18, 45), [0, 0, 255, 255], 0, "bottom left");
    assert_near(pixel(&image, 45, 45), [255; 4], 0, "bottom right");
    assert_near(pixel(&image, 10, 32), BLACK, 0, "outside");
    assert_near(pixel(&image, 15, 32), BLACK, 0, "just outside the edge");
    Ok(())
}

#[test]
fn straight_alpha_blends_in_linear_light_and_opacity_fades() -> TestResult {
    let white = Shape {
        geometry: Geometry::Rect {
            rect: rect(0.0, 0.0, 64.0, 64.0),
            radius: 0.0,
        },
        fill: Some(Paint::Solid(Rgba([255; 4]))),
        stroke: None,
        opacity: 1.0,
        rotation: 0.0,
        glow: None,
    };
    let half_red = ("r.png", size(4, 4), bitmap(4, 4, |_, _| [255, 0, 0, 128]));
    let red = ("s.png", size(4, 4), bitmap(4, 4, |_, _| [255, 0, 0, 255]));
    let faded = Sprite {
        opacity: 0.5,
        ..sprite(key("s.png", 0), rect(32.0, 0.0, 32.0, 64.0))
    };
    let layers = vec![
        Layer::Shapes(vec![white]),
        Layer::Sprite(sprite(key("r.png", 0), rect(0.0, 0.0, 32.0, 64.0))),
        Layer::Sprite(faded),
    ];
    let image = render(&[half_red, red], layers)?;
    assert_near(
        pixel(&image, 16, 32),
        [255, 188, 188, 255],
        2,
        "half-transparent red",
    );
    assert_near(
        pixel(&image, 48, 32),
        [255, 188, 188, 255],
        2,
        "half-faded red",
    );
    Ok(())
}

/// A one-pixel checkerboard drawn 16 times smaller averages to grey (mip chain), not to a
/// moiré of black and white.
#[test]
fn big_picture_drawn_small_is_smooth() -> TestResult {
    let checker = bitmap(
        256,
        256,
        |x, y| if (x + y) % 2 == 0 { [255; 4] } else { BLACK },
    );
    let layer = Layer::Sprite(sprite(key("c.png", 0), rect(16.0, 16.0, 16.0, 16.0)));
    let image = render(&[("c.png", size(256, 256), checker)], vec![layer])?;
    for (x, y) in [(18, 18), (24, 24), (29, 20)] {
        assert_near(
            pixel(&image, x, y),
            [188, 188, 188, 255],
            4,
            &format!("{x},{y}"),
        );
    }
    Ok(())
}

#[test]
fn rotation_and_camera_move_the_picture() -> TestResult {
    let turned = Sprite {
        rotation: 90.0,
        ..sprite(key("q.png", 0), rect(16.0, 16.0, 32.0, 32.0))
    };
    let image = render(&[quadrants()], vec![Layer::Sprite(turned)])?;
    // Clockwise: the top-left red quadrant goes to the top right.
    assert_near(
        pixel(&image, 45, 18),
        [255, 0, 0, 255],
        0,
        "red after the turn",
    );
    assert_near(
        pixel(&image, 18, 18),
        [0, 0, 255, 255],
        0,
        "blue after the turn",
    );

    let gpu = gpu()?;
    let mut compositor = Compositor::new(&gpu);
    let (path, picture_size, rgba) = quadrants();
    compositor.upload_image(&gpu, &key(path, 0), Bitmap::new(picture_size, &rgba)?)?;
    let mut scene = Scene::new(size(64, 64));
    scene.camera = Camera {
        scale: 2.0,
        x: -32.0,
        y: -32.0,
    };
    let layer = Layer::Sprite(sprite(key(path, 0), rect(16.0, 16.0, 32.0, 32.0)));
    scene.push(Band::Annotations, layer);
    let zoomed = draw(&gpu, &mut compositor, &scene)?;
    assert_near(pixel(&zoomed, 4, 4), [255, 0, 0, 255], 0, "zoomed red");
    assert_near(pixel(&zoomed, 60, 60), [255; 4], 0, "zoomed white");
    Ok(())
}

#[test]
fn missing_picture_and_wrong_length_are_errors() -> TestResult {
    let layer = Layer::Sprite(sprite(key("нет.png", 3), rect(0.0, 0.0, 8.0, 8.0)));
    let error = render(&[], vec![layer]).err().ok_or("drawn")?;
    let error = error.downcast_ref::<RenderError>().ok_or("other error")?;
    assert!(
        matches!(error, RenderError::NoImage { path, frame: 3 } if path == "нет.png"),
        "{error}"
    );
    assert!(Bitmap::new(size(2, 2), &[0; 15]).is_err());
    assert!(Bitmap::new(size(0, 0), &[]).is_err());
    Ok(())
}

/// Frames of an animated GIF decoded by `dc-media` are separate pictures of one file.
#[test]
fn frames_of_an_animated_gif_are_drawn_by_number() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("анимация.gif");
    let ffmpeg = Path::new(env!("FFMPEG_DIR"))
        .join("bin")
        .join(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX));
    let filter = r"color=s=8x8:r=10,format=gbrp,geq=r=255*eq(N\,0):g=255*eq(N\,1):b=255*eq(N\,2)";
    let status = Command::new(ffmpeg)
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            filter,
            "-frames:v",
            "3",
        ])
        .arg(&path)
        .status()?;
    assert!(status.success());
    let picture = decode_picture(&path)?;
    let gpu = gpu()?;
    let mut compositor = Compositor::new(&gpu);
    for (i, frame) in (0..).zip(&picture.frames) {
        let bitmap = Bitmap::new(picture.size, &frame.rgba)?;
        compositor.upload_image(&gpu, &key("assets/анимация.gif", i), bitmap)?;
    }
    assert!(compositor.has_image(&key("assets/анимация.gif", 2)));
    let expected = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
    for (i, colour) in (0..).zip(expected) {
        let mut scene = Scene::new(size(16, 16));
        let layer = sprite(key("assets/анимация.gif", i), rect(0.0, 0.0, 16.0, 16.0));
        scene.push(Band::Annotations, Layer::Sprite(layer));
        let image = draw(&gpu, &mut compositor, &scene)?;
        assert_near(pixel(&image, 8, 8), colour, 8, &format!("frame {i}"));
    }
    Ok(())
}
