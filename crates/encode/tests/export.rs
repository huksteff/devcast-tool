//! Task 5.12: MP4 export. ffprobe sees H.264 + AAC, the frame rate, the BT.709 limited tags and
//! matching lengths of picture and sound; a frame of the file, decoded and drawn by the video
//! pass, matches what `frame_png` renders for the same moment (ꟻLIP).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use common::{AUDIO, canvas, check_streams, clip_frame, speech, synthetic, tone};
use dc_core::{OutputTime, PixelSize, Project, Rect, SourceTime, TimeMap};
use dc_encode::export::{Mp4Settings, Resolution, Timeline, export_mp4};
use dc_encode::{EncodeError, Quality, Sources};
use dc_media::{Decoder, FramePool, nv12_len, probe};
use dc_project::ProjectDir;
use dc_render::golden::flip_error;
use dc_render::{
    AdapterChoice, Band, Compositor, Gpu, Image, Layer, Nv12, Scene, Video, VideoSource,
};

struct Exported {
    file: PathBuf,
    result: Result<dc_encode::export::Exported, EncodeError>,
}

fn export(
    gpu: &Gpu,
    dir: &Path,
    project_dir: &ProjectDir,
    project: &Project,
    settings: Mp4Settings,
    cancel_at: Option<u64>,
) -> Exported {
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
    let file = dir.join("готовое видео.mp4");
    let mut progress = |p: dc_encode::export::Progress| match cancel_at {
        Some(n) if p.done == n => ControlFlow::Break(()),
        _ => ControlFlow::Continue(()),
    };
    let mut compositor = Compositor::new(gpu);
    let mut sources = Sources::new(project_dir);
    let result = export_mp4(
        gpu,
        &mut compositor,
        &mut sources,
        &mut timeline,
        &settings,
        &file,
        &mut progress,
    );
    Exported { file, result }
}

/// Frame `t` of `file` as the preview would show the file: decoded and drawn by the video pass.
fn decoded(gpu: &Gpu, file: &Path, t: SourceTime) -> Image {
    let probe = probe(file).unwrap();
    let size = probe.size;
    let color = probe.color;
    let mut decoder = Decoder::open(file, probe, FramePool::new(nv12_len(size))).unwrap();
    let frame = decoder.frame_at(t).unwrap();
    let source = VideoSource {
        media: "export.mp4".into(),
        time: t,
    };
    let mut scene = Scene::new(size);
    scene.push(
        Band::Video,
        Layer::Video(Video::fitted(source.clone(), size, Rect::FULL, size)),
    );
    let mut compositor = Compositor::new(gpu);
    compositor.upload(
        gpu,
        &source,
        Nv12::new(size, frame.y(), frame.uv(), color).unwrap(),
    );
    compositor.render(gpu, &scene).unwrap()
}

/// What `frame_png` draws at output moment `t`, scaled to the export's `size`.
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

/// ꟻLIP allowed between an exported frame and its preview: H.264 loses detail, most of all in
/// the noise block of `testsrc2` (measured at high quality, NVENC: mean 0.022–0.033, 99 % ≤
/// 0.19–0.22). A frame half a second off must fail it.
const EXPORT_FLIP_MEAN_MAX: f32 = 0.05;
const EXPORT_FLIP_P99_MAX: f32 = 0.3;

struct Check<'a> {
    gpu: &'a Gpu,
    project_dir: &'a ProjectDir,
    project: &'a Project,
    file: &'a Path,
    size: PixelSize,
    fps: u32,
}

impl Check<'_> {
    fn flip(&self, exported_frame: u64, preview_frame: u64) -> (f32, f32) {
        let at = |frame: u64| (frame * 1_000_000 / u64::from(self.fps)) as i64;
        let exported = decoded(
            self.gpu,
            self.file,
            SourceTime::from_micros(at(exported_frame)),
        );
        let preview = previewed(
            self.gpu,
            self.project_dir,
            self.project,
            OutputTime::from_micros(at(preview_frame)),
            self.size,
        );
        flip_error(&preview, &exported)
    }

    fn same_frame(&self, frame: u64) {
        let (mean, p99) = self.flip(frame, frame);
        println!("frame {frame}: ꟻLIP mean {mean:.4}, 99 % ≤ {p99:.4}");
        assert!(
            mean <= EXPORT_FLIP_MEAN_MAX && p99 <= EXPORT_FLIP_P99_MAX,
            "frame {frame}: {mean} {p99}"
        );
        let off = frame
            .checked_sub(u64::from(self.fps) / 2)
            .unwrap_or(frame + u64::from(self.fps) / 2);
        let (mean, p99) = self.flip(frame, off);
        println!("frame {frame} against {off}: ꟻLIP mean {mean:.4}, 99 % ≤ {p99:.4}");
        assert!(mean > EXPORT_FLIP_MEAN_MAX || p99 > EXPORT_FLIP_P99_MAX);
    }
}

#[test]
fn synthetic_project_exports_h264_aac_with_tags_and_the_previewed_frames() {
    let dir = tempfile::tempdir().unwrap();
    let (project_dir, project) = synthetic(dir.path());
    let gpu = Gpu::new(AdapterChoice::Auto).unwrap();
    let settings = Mp4Settings {
        resolution: Resolution::P720,
        fps: 60,
        quality: Quality::High,
    };
    let exported = export(&gpu, dir.path(), &project_dir, &project, settings, None);
    let done = exported.result.unwrap();
    let size = PixelSize {
        width: 1280,
        height: 720,
    };
    assert_eq!(done.size, size);
    assert_eq!(done.frames, 150);
    println!("{:?}, {} bytes", done.codec, done.bytes);
    check_streams(&exported.file, 60, size, 2.5);
    // Before the cut, right after it (source 1.5 s) and the last frame.
    let check = Check {
        gpu: &gpu,
        project_dir: &project_dir,
        project: &project,
        file: &exported.file,
        size,
        fps: 60,
    };
    for frame in [20, 61, 149] {
        check.same_frame(frame);
    }
}

#[test]
fn cancelled_export_leaves_no_file_and_keeps_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let (project_dir, project) = synthetic(dir.path());
    let gpu = Gpu::new(AdapterChoice::Auto).unwrap();
    let settings = Mp4Settings {
        resolution: Resolution::Source,
        fps: 30,
        quality: Quality::Low,
    };
    std::fs::write(dir.path().join("готовое видео.mp4"), b"old").unwrap();
    let exported = export(&gpu, dir.path(), &project_dir, &project, settings, Some(10));
    assert!(matches!(exported.result, Err(EncodeError::Cancelled)));
    assert_eq!(std::fs::read(&exported.file).unwrap(), b"old");
    let left: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(left.len(), 2, "only the project and the old file: {left:?}");
}

#[test]
fn speech_recording_exports_with_the_previewed_frames() {
    let dir = tempfile::tempdir().unwrap();
    let (project_dir, project) = speech(dir.path());
    let gpu = Gpu::new(AdapterChoice::Auto).unwrap();
    let settings = Mp4Settings {
        resolution: Resolution::Source,
        fps: 30,
        quality: Quality::High,
    };
    let exported = export(&gpu, dir.path(), &project_dir, &project, settings, None);
    let done = exported.result.unwrap();
    let size = PixelSize {
        width: 1280,
        height: 720,
    };
    println!("{:?}, {} bytes", done.codec, done.bytes);
    check_streams(&exported.file, 30, size, 20.016666);
    let check = Check {
        gpu: &gpu,
        project_dir: &project_dir,
        project: &project,
        file: &exported.file,
        size,
        fps: 30,
    };
    for frame in [60, 450] {
        check.same_frame(frame);
    }
}
