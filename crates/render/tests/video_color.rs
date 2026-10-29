//! A file's colour marks reach the video pass: synthetic H.264 in BT.601 and BT.709, limited and
//! full range, decoded by `dc-media` and drawn, matches FFmpeg's own RGB of the same file.

mod common;

use std::error::Error;
use std::path::Path;
use std::process::Command;

use common::{TestResult, assert_near, gpu, pixel, size, source};
use dc_core::{Rect, SourceTime};
use dc_media::{Decoder, FramePool, nv12_len, probe};
use dc_render::{Band, Compositor, Image, Layer, Nv12, Scene, Video};

const W: u32 = 64;
const H: u32 = 32;

fn ffmpeg(args: &[&str]) -> TestResult {
    let exe = Path::new(env!("FFMPEG_DIR"))
        .join("bin")
        .join(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX));
    let result = Command::new(exe)
        .args(["-v", "error", "-y"])
        .args(args)
        .output()?;
    assert!(
        result.status.success(),
        "ffmpeg {args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}

/// A lossless one-frame H.264 of saturated smooth gradients, marked `matrix` / `range`.
fn encode(path: &Path, matrix: &str, range: &str) -> TestResult {
    let (space, tag) = match matrix {
        "bt601" => ("bt470bg", "smpte170m"),
        _ => ("bt709", "bt709"),
    };
    let scale = format!("scale=out_color_matrix={matrix}:out_range={range},format=yuv420p");
    let source = format!("gradients=s={W}x{H}:c0=red:c1=lime:c2=blue:c3=yellow:nb_colors=4");
    let path = path.to_str().ok_or("path")?;
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        &source,
        "-frames:v",
        "1",
        "-vf",
        &scale,
        "-colorspace",
        space,
        "-color_primaries",
        tag,
        "-color_trc",
        tag,
        "-color_range",
        range,
        "-chroma_sample_location",
        "left",
        "-c:v",
        "libx264",
        "-qp",
        "0",
        path,
    ])
}

/// FFmpeg's RGB of the file, told the same colour space explicitly.
fn reference(path: &Path, matrix: &str, range: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let raw = path.with_extension("rgb");
    let scale = format!(
        "scale=in_color_matrix={matrix}:in_range={range}:in_chroma_loc=left:out_range=pc:flags=bilinear+accurate_rnd+full_chroma_int,format=rgb24"
    );
    ffmpeg(&[
        "-i",
        path.to_str().ok_or("path")?,
        "-vf",
        &scale,
        "-f",
        "rawvideo",
        raw.to_str().ok_or("path")?,
    ])?;
    Ok(std::fs::read(raw)?)
}

fn render(path: &Path) -> Result<Image, Box<dyn Error>> {
    let probe = probe(path)?;
    let color = probe.color;
    let mut decoder = Decoder::open(path, probe, FramePool::new(nv12_len(size(W, H))))?;
    let frame = decoder.frame_at(SourceTime::ZERO)?;
    let gpu = gpu()?;
    let mut scene = Scene::new(size(W, H));
    let video = Video::fitted(source("media/a.mp4"), size(W, H), Rect::FULL, size(W, H));
    let mut compositor = Compositor::new(&gpu);
    compositor.upload(
        &gpu,
        &video.source,
        Nv12::new(frame.size, frame.y(), frame.uv(), color)?,
    );
    scene.push(Band::Video, Layer::Video(video));
    let target = gpu.target(scene.size);
    compositor.draw(&gpu, &scene, &target)?;
    Ok(gpu.read(&target)?)
}

#[test]
fn colour_marks_of_the_file_match_ffmpeg() -> TestResult {
    let dir = tempfile::tempdir()?;
    for (matrix, range) in [
        ("bt601", "tv"),
        ("bt601", "pc"),
        ("bt709", "tv"),
        ("bt709", "pc"),
    ] {
        let path = dir.path().join(format!("{matrix} {range}.mp4"));
        encode(&path, matrix, range)?;
        let expected = reference(&path, matrix, range)?;
        let image = render(&path)?;
        for y in 0..H {
            for x in 0..W {
                let at = ((y * W + x) * 3) as usize;
                let [r, g, b] = [0, 1, 2].map(|i| expected[at + i]);
                assert_near(
                    pixel(&image, x, y),
                    [r, g, b, 255],
                    2,
                    &format!("{matrix} {range} at {x},{y}"),
                );
            }
        }
    }
    Ok(())
}
