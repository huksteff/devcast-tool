//! Test projects (a synthetic clip, the speech recording), FFmpeg tools and ffprobe checks for
//! the export tests.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]
#![allow(
    dead_code,
    reason = "each test binary uses its own part of the helpers"
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use dc_core::{OutputTime, PixelSize, Project, Rect, SourceTime, TimeMap};
use dc_encode::AudioSettings;
use dc_media::probe;
use dc_project::ProjectDir;
use dc_render::{Band, Layer, Scene, Video, VideoSource};
use serde_json::Value;

pub const AUDIO: AudioSettings = AudioSettings {
    rate: 48_000,
    channels: 2,
};
pub fn tool(name: &str) -> PathBuf {
    Path::new(env!("FFMPEG_DIR"))
        .join("bin")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

pub fn run(name: &str, args: &[&str]) -> String {
    let output = Command::new(tool(name)).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// The frame of the export tests' projects (one uncropped clip): its source size.
pub fn canvas(project: &Project) -> PixelSize {
    project.clips[0].media.size
}

/// What output moment `t` of a one-clip test project shows: its source frame filling the canvas.
/// The real scenes of a project come from `dc_ops::Evaluator`, a layer above this crate.
pub fn clip_frame(project: &Project, map: &TimeMap, t: OutputTime) -> Scene {
    let mut scene = Scene::new(canvas(project));
    if let Some(segment) = map.segment_at(t) {
        let source = VideoSource {
            media: segment.media.clone(),
            time: segment.source_at(t),
        };
        let video = Video::fitted(source, canvas(project), Rect::FULL, scene.size);
        scene.push(Band::Video, Layer::Video(video));
    }
    scene
}

/// A project of one moving `testsrc2` clip, 640×360 at 30 fps, 3 s, with a cut at 1–1.5 s.
pub fn synthetic(dir: &Path) -> (ProjectDir, Project) {
    let project_dir = ProjectDir::create(dir.join("экспорт проекта.dcproj")).unwrap();
    let video = project_dir.media_dir().join("clip.mp4");
    run(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=640x360:r=30:d=3",
            "-vf",
            "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p",
            "-colorspace",
            "bt709",
            "-c:v",
            "libx264",
            "-g",
            "30",
            video.to_str().unwrap(),
        ],
    );
    let probe = probe(&video).unwrap();
    let mut project = Project::default();
    let id = project.clip_ids.allocate();
    let clip = dc_core::Clip {
        id,
        media: dc_core::project::Media {
            path: "media/clip.mp4".into(),
            kind: dc_core::project::MediaKind::Imported,
            duration: probe.duration,
            size: probe.size,
        },
        trim: dc_core::Range::new(SourceTime::ZERO, SourceTime::ZERO + probe.duration).unwrap(),
        cuts: vec![
            dc_core::Range::new(
                SourceTime::from_secs_f64(1.0),
                SourceTime::from_secs_f64(1.5),
            )
            .unwrap(),
        ],
        speeds: Vec::new(),
        crop: Rect::FULL,
    };
    project.clips.push(clip);
    (project_dir, project)
}

/// The speech recording, copied: the export writes probe caches into the project.
pub fn speech(dir: &Path) -> (ProjectDir, Project) {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/speech.dcproj");
    let root = dir.join("speech.dcproj");
    let media = root.join("media").join("recording-1");
    std::fs::create_dir_all(&media).unwrap();
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::copy(fixture.join("project.json"), root.join("project.json")).unwrap();
    std::fs::copy(
        fixture.join("media/recording-1/video.mp4"),
        media.join("video.mp4"),
    )
    .unwrap();
    let project_dir = ProjectDir::open(&root).unwrap();
    let project = project_dir.load().unwrap();
    (project_dir, project)
}

/// A 440 Hz tone at −12 dBFS from output sample `start`.
pub fn tone(start: i64, out: &mut [f32]) {
    for (i, pair) in out.as_chunks_mut::<2>().0.iter_mut().enumerate() {
        let t = (start + i as i64) as f32 / 48_000.0;
        let v = 0.25 * (2.0 * std::f32::consts::PI * 440.0 * t).sin();
        pair.fill(v);
    }
}

pub fn ffprobe(file: &Path) -> Value {
    let text = run(
        "ffprobe",
        &[
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-count_frames",
            "-of",
            "json",
            file.to_str().unwrap(),
        ],
    );
    serde_json::from_str(&text).unwrap()
}

pub fn seconds(value: &Value) -> f64 {
    value.as_str().unwrap().parse().unwrap()
}

/// Checks codecs, rate, tags and lengths; returns the video stream.
pub fn check_streams(file: &Path, fps: u32, size: PixelSize, duration: f64) -> Value {
    let info = ffprobe(file);
    let streams = info["streams"].as_array().unwrap();
    let video = streams.iter().find(|s| s["codec_type"] == "video").unwrap();
    let audio = streams.iter().find(|s| s["codec_type"] == "audio").unwrap();
    assert_eq!(video["codec_name"], "h264");
    assert_eq!(video["width"], size.width);
    assert_eq!(video["height"], size.height);
    assert_eq!(video["r_frame_rate"], format!("{fps}/1"));
    assert_eq!(video["pix_fmt"], "yuv420p");
    for (tag, value) in [
        ("color_range", "tv"),
        ("color_space", "bt709"),
        ("color_primaries", "bt709"),
        ("color_transfer", "bt709"),
        ("chroma_location", "left"),
    ] {
        assert_eq!(video[tag], value, "{tag}");
    }
    assert_eq!(audio["codec_name"], "aac");
    assert_eq!(audio["profile"], "LC");
    assert_eq!(audio["sample_rate"], "48000");
    assert_eq!(audio["channels"], 2);
    let frames: u64 = video["nb_read_frames"].as_str().unwrap().parse().unwrap();
    assert_eq!(frames, (duration * f64::from(fps)).ceil() as u64);
    let video_length = seconds(&video["duration"]);
    let audio_length = seconds(&audio["duration"]);
    println!(
        "{}: video {video_length:.4} s, audio {audio_length:.4} s, project {duration:.4} s",
        file.display()
    );
    assert!(
        seconds(&audio["start_time"]).abs() < 1e-3,
        "sound starts at 0"
    );
    assert!(
        (audio_length - duration).abs() < 0.002,
        "sound {audio_length} vs {duration}"
    );
    assert!(video_length >= duration && video_length - duration < 1.0 / f64::from(fps) + 1e-3);
    video.clone()
}
