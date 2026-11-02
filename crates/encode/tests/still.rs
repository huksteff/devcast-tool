//! Frames of a project as PNG: a video clip and a picture from the project folder, drawn without
//! a window and read back from the PNG.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

use std::num::NonZeroU32;
use std::path::Path;
use std::process::Command;

use dc_core::{PixelSize, Rect, SourceTime};
use dc_encode::{Sources, frame_png, sheet_png};
use dc_media::{Picture, decode_picture, encode_png};
use dc_project::ProjectDir;
use dc_render::{
    AdapterChoice, Band, Compositor, Gpu, ImageKey, Layer, PxRect, SHEET_GAP, Scene, Sprite, Video,
    VideoSource,
};

const SIZE: PixelSize = PixelSize {
    width: 64,
    height: 36,
};

/// Half a second of red, then half a second of blue, BT.709 limited range.
fn red_then_blue(path: &Path) {
    let exe = Path::new(env!("FFMPEG_DIR"))
        .join("bin")
        .join(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX));
    let graph = "color=c=red:s=64x36:r=10:d=0.5[a];color=c=blue:s=64x36:r=10:d=0.5[b];[a][b]concat=n=2:v=1,scale=out_color_matrix=bt709:out_range=tv,format=yuv420p[v]";
    let output = Command::new(exe)
        .args(["-v", "error", "-y", "-filter_complex", graph, "-map", "[v]"])
        .args(["-colorspace", "bt709", "-c:v", "libx264", "-g", "1"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn project(dir: &Path) -> ProjectDir {
    let project = ProjectDir::create(dir.join("проект с пробелом.dcproj")).unwrap();
    red_then_blue(&project.media_dir().join("clip.mp4"));
    let green = [0, 255, 0, 255].repeat(8 * 8);
    let png = encode_png(
        PixelSize {
            width: 8,
            height: 8,
        },
        &green,
    )
    .unwrap();
    std::fs::write(project.assets_dir().join("точка.png"), png).unwrap();
    project
}

fn scene(seconds: f64) -> Scene {
    let mut scene = Scene::new(SIZE);
    let source = VideoSource {
        media: "media/clip.mp4".into(),
        time: SourceTime::from_micros((seconds * 1e6) as i64),
    };
    let video = Video::fitted(source, SIZE, Rect::FULL, SIZE);
    scene.push(Band::Video, Layer::Video(video));
    let sprite = Sprite {
        image: ImageKey {
            path: "assets/точка.png".into(),
            frame: 0,
        },
        rect: PxRect {
            x: 4.0,
            y: 4.0,
            w: 8.0,
            h: 8.0,
        },
        rotation: 0.0,
        opacity: 1.0,
        motion: None,
    };
    scene.push(Band::Annotations, Layer::Sprite(sprite));
    scene
}

fn read_png(dir: &Path, png: &[u8]) -> Picture {
    let path = dir.join("кадр.png");
    std::fs::write(&path, png).unwrap();
    decode_picture(&path).unwrap()
}

fn pixel(picture: &Picture, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * picture.size.width + x) * 4) as usize;
    let rgba = &picture.frames[0].rgba;
    [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
}

fn near(actual: [u8; 4], expected: [u8; 4], what: &str) {
    let close = actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 8);
    assert!(close, "{what}: {actual:?}, expected {expected:?} ±8");
}

#[test]
fn frame_shows_the_clip_at_its_time_and_the_picture() {
    let dir = tempfile::tempdir().unwrap();
    let project = project(dir.path());
    let gpu = Gpu::new(AdapterChoice::Auto).unwrap();
    let mut compositor = Compositor::new(&gpu);
    let mut sources = Sources::new(&project);
    for (seconds, color) in [(0.2, [255, 0, 0, 255]), (0.7, [0, 0, 255, 255])] {
        let png = frame_png(&gpu, &mut compositor, &mut sources, &scene(seconds), SIZE).unwrap();
        let picture = read_png(dir.path(), &png);
        assert_eq!(picture.size, SIZE);
        near(
            pixel(&picture, 40, 20),
            color,
            &format!("video at {seconds} s"),
        );
        near(pixel(&picture, 8, 8), [0, 255, 0, 255], "picture");
    }
}

#[test]
fn sheet_puts_moments_side_by_side() {
    let dir = tempfile::tempdir().unwrap();
    let project = project(dir.path());
    let gpu = Gpu::new(AdapterChoice::Auto).unwrap();
    let mut compositor = Compositor::new(&gpu);
    let mut sources = Sources::new(&project);
    let scenes = [scene(0.2), scene(0.7)];
    let columns = NonZeroU32::new(2).unwrap();
    let png = sheet_png(&gpu, &mut compositor, &mut sources, &scenes, columns, SIZE).unwrap();
    let picture = read_png(dir.path(), &png);
    let cell = SIZE.width + SHEET_GAP;
    assert_eq!(picture.size.width, 2 * cell + SHEET_GAP);
    assert_eq!(picture.size.height, SIZE.height + 2 * SHEET_GAP);
    near(
        pixel(&picture, SHEET_GAP + 40, 20),
        [255, 0, 0, 255],
        "first",
    );
    near(
        pixel(&picture, cell + SHEET_GAP + 40, 20),
        [0, 0, 255, 255],
        "second",
    );
    near(pixel(&picture, cell + 2, 20), [128, 128, 128, 255], "gap");
}

#[test]
fn a_missing_picture_frame_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let project = project(dir.path());
    let gpu = Gpu::new(AdapterChoice::Auto).unwrap();
    let mut scene = scene(0.2);
    if let Some(Layer::Sprite(sprite)) = scene
        .layers
        .get_mut(&Band::Annotations)
        .and_then(|l| l.first_mut())
    {
        sprite.image.frame = 3;
    }
    let error = frame_png(
        &gpu,
        &mut Compositor::new(&gpu),
        &mut Sources::new(&project),
        &scene,
        SIZE,
    )
    .unwrap_err();
    assert!(error.to_string().contains("кадра 3 нет"), "{error}");
}
