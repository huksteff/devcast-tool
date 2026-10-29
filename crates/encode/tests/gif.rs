//! Task 5.13: GIF export through gifski. ffprobe sees a GIF of the size and frame count;
//! the loop flag is in the file; a frame of the GIF looks like the preview of its moment.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use std::ops::ControlFlow;
use std::path::Path;

use common::{AUDIO, canvas, clip_frame, ffprobe, synthetic, tone};
use dc_core::{OutputTime, PixelSize, Project, TimeMap};
use dc_encode::export::{GifSettings, Resolution, Timeline, export_gif};
use dc_encode::{EncodeError, Sources};
use dc_media::decode_picture;
use dc_project::ProjectDir;
use dc_render::golden::flip_error;
use dc_render::{AdapterChoice, Compositor, Gpu, Image};

fn export(
    gpu: &Gpu,
    project_dir: &ProjectDir,
    project: &Project,
    settings: GifSettings,
    out: &Path,
) -> Result<dc_encode::export::Exported, EncodeError> {
    let map = TimeMap::build(project);
    let canvas = canvas(project);
    let mut scene = |t| clip_frame(project, &map, t);
    let mut sound = |start, out: &mut [f32]| {
        tone(start, out);
        Ok(())
    };
    let mut timeline = Timeline {
        duration: map.duration(),
        canvas,
        source_fps: 30.0,
        scene: &mut scene,
        audio: AUDIO,
        sound: &mut sound,
    };
    let mut progress = |_| ControlFlow::Continue(());
    export_gif(
        gpu,
        &mut Compositor::new(gpu),
        &mut Sources::new(project_dir),
        &mut timeline,
        &settings,
        out,
        &mut progress,
    )
}

/// Loop count of the NETSCAPE2.0 block (0 — forever); `None` without the block: plays once.
fn loops(file: &Path) -> Option<u16> {
    let bytes = std::fs::read(file).unwrap();
    let at = bytes.windows(11).position(|w| w == b"NETSCAPE2.0")?;
    let block = &bytes[at + 11..at + 15];
    assert_eq!(block[..2], [3, 1]);
    Some(u16::from_le_bytes([block[2], block[3]]))
}

fn previewed(
    gpu: &Gpu,
    project_dir: &ProjectDir,
    project: &Project,
    t: OutputTime,
    size: PixelSize,
) -> Image {
    let map = TimeMap::build(project);
    let scene = clip_frame(project, &map, t);
    let mut compositor = Compositor::new(gpu);
    Sources::new(project_dir)
        .feed(gpu, &mut compositor, &scene)
        .unwrap();
    let target = gpu.target(size);
    compositor.draw(gpu, &scene, &target).unwrap();
    gpu.read(&target).unwrap()
}

#[test]
fn gif_has_the_size_frames_and_loop_and_shows_the_preview() {
    let dir = tempfile::tempdir().unwrap();
    let (project_dir, project) = synthetic(dir.path());
    let gpu = Gpu::new(AdapterChoice::Auto).unwrap();
    let out = dir.path().join("анимация.gif");
    let settings = GifSettings {
        resolution: Resolution::Source,
        fps: 20,
        looped: true,
    };
    let done = export(&gpu, &project_dir, &project, settings, &out).unwrap();
    let size = PixelSize {
        width: 640,
        height: 360,
    };
    assert_eq!((done.size, done.frames), (size, 50));
    println!("{} bytes", done.bytes);
    let info = ffprobe(&out);
    let video = &info["streams"][0];
    assert_eq!(video["codec_name"], "gif");
    assert_eq!(
        (video["width"].clone(), video["height"].clone()),
        (640.into(), 360.into())
    );
    let frames: u64 = video["nb_read_frames"].as_str().unwrap().parse().unwrap();
    assert!(
        (40..=50).contains(&frames),
        "gifski merges identical frames only: {frames}"
    );
    let length: f64 = info["format"]["duration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((length - 2.5).abs() < 0.06, "{length}");
    assert_eq!(loops(&out), Some(0));
    let picture = decode_picture(&out).unwrap();
    let frame = 10;
    let shown = Image {
        size,
        rgba: picture.frames[frame].rgba.clone(),
    };
    let preview = previewed(
        &gpu,
        &project_dir,
        &project,
        OutputTime::from_micros(500_000),
        size,
    );
    let (mean, p99) = flip_error(&preview, &shown);
    println!("gif frame {frame}: ꟻLIP mean {mean:.4}, 99 % ≤ {p99:.4}");
    assert!(mean < 0.05 && p99 < 0.3, "{mean} {p99}");
}

#[test]
fn gif_plays_once_without_loop_and_refuses_what_scope_does_not_have() {
    let dir = tempfile::tempdir().unwrap();
    let (project_dir, project) = synthetic(dir.path());
    let gpu = Gpu::new(AdapterChoice::Auto).unwrap();
    let out = dir.path().join("once.gif");
    let once = GifSettings {
        resolution: Resolution::P720,
        fps: 15,
        looped: false,
    };
    let done = export(&gpu, &project_dir, &project, once, &out).unwrap();
    assert_eq!(
        done.size,
        PixelSize {
            width: 1280,
            height: 720
        }
    );
    assert_eq!(loops(&out), None, "plays once");
    let video = &ffprobe(&out)["streams"][0];
    assert_eq!(
        (video["width"].clone(), video["height"].clone()),
        (1280.into(), 720.into())
    );
    for (resolution, fps) in [(Resolution::P2160, 15), (Resolution::P720, 60)] {
        let wrong = GifSettings {
            resolution,
            fps,
            looped: true,
        };
        let error = export(&gpu, &project_dir, &project, wrong, &out).unwrap_err();
        assert!(
            matches!(error, EncodeError::Fps { .. } | EncodeError::GifResolution),
            "{error}"
        );
    }
}
