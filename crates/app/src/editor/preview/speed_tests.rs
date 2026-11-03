//! How fast the render thread draws frames one after another, as playback asks for them: the
//! fixture recording (720p60 with its telemetry) and synthetic 1080p60 and 4K60 videos, on the
//! real GPU.
//! A measurement, not a check: `cargo nextest run -p dc-app --run-ignored only playback_speed
//! --no-capture` (and with `--release`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::Instant;

use dc_core::project::{Clip, Media, MediaKind};
use dc_core::{OutputTime, PixelSize, Project, Range, Rect, SourceTime, TimeDelta};
use dc_project::ProjectDir;
use dc_render::{AdapterChoice, Gpu};

use super::fetch::Want;
use super::worker::{Request, Worker};

/// Frames drawn in a row: 5 s of 60 fps.
const FRAMES: u32 = 300;

fn ffmpeg(args: &[&str]) {
    let tool = PathBuf::from(env!("FFMPEG_DIR"))
        .join("bin")
        .join("ffmpeg.exe");
    let status = std::process::Command::new(tool)
        .args(["-v", "error", "-y"])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg {args:?}");
}

/// A project of one imported 60 fps `testsrc2` video of `size`, GOP 1 s like our recordings.
fn synthetic(root: &Path, size: PixelSize) -> (ProjectDir, Project) {
    let name = format!("{}p.devcast", size.height);
    let dir = ProjectDir::create(root.join(name)).unwrap();
    std::fs::create_dir_all(dir.root().join("media")).unwrap();
    let video = dir.root().join("media").join("screen.mp4");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        &format!("testsrc2=s={}x{}:r=60:d=8", size.width, size.height),
        "-c:v",
        "libx264",
        "-g",
        "60",
        "-bf",
        "0",
        "-pix_fmt",
        "yuv420p",
        video.to_str().unwrap(),
    ]);
    let secs = 8.0;
    let range = Range::new(SourceTime::ZERO, SourceTime::from_secs_f64(secs)).unwrap();
    let mut project = Project::default();
    let id = project.clip_ids.allocate();
    project.clips.push(Clip {
        id,
        media: Media {
            path: "media/screen.mp4".to_owned(),
            kind: MediaKind::Imported,
            duration: TimeDelta::from_secs_f64(secs),
            size,
        },
        trim: range,
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
        audio: Default::default(),
    });
    dir.save(&project).unwrap();
    (dir, project)
}

/// Draws [`FRAMES`] frames 1/60 s apart from 1 s on, each after the last one came; frames per
/// second.
fn measure(gpu: &Arc<Gpu>, dir: &ProjectDir, project: Project, size: PixelSize) -> f64 {
    let (woke, wakes) = channel();
    let worker = Worker::start(Arc::clone(gpu), dir.clone(), move || {
        let _ = woke.send(());
    })
    .unwrap();
    let project = Arc::new(project);
    let ask = |generation: u32| {
        let want = Want {
            revision: 1,
            time: OutputTime::from_secs_f64(1.0 + f64::from(generation) / 60.0),
            size,
            scrub: false,
            source: false,
            camera: true,
        };
        let project = Arc::clone(&project);
        assert!(worker.send(Request {
            generation: u64::from(generation),
            project,
            want,
        }));
        wakes.recv().unwrap();
        let (_, answer) = worker.answers().next().unwrap();
        answer.unwrap()
    };
    // The first frame opens the decoder and loads the fonts.
    ask(0);
    let start = Instant::now();
    let mut last = None;
    for generation in 1..=FRAMES {
        last = Some(ask(generation));
    }
    // Waits for the GPU to finish the last frame.
    gpu.read(&last.unwrap().texture).unwrap();
    f64::from(FRAMES) / start.elapsed().as_secs_f64()
}

#[test]
#[ignore = "a measurement on the real GPU"]
fn playback_speed() {
    let gpu = Arc::new(Gpu::new(AdapterChoice::Auto).unwrap());
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/clicks.devcast");
    let source = ProjectDir::open(fixture).unwrap();
    let project = source.load().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let dir = source
        .save_as(&project, temp.path().join("clicks.devcast"))
        .unwrap();
    let hd = PixelSize {
        width: 1280,
        height: 720,
    };
    let fps = measure(&gpu, &dir, project, hd);
    println!("clicks.devcast 720p60 with cursor: {fps:.0} frames/s");
    let full = PixelSize {
        width: 1920,
        height: 1080,
    };
    let (dir, project) = synthetic(temp.path(), full);
    let fps = measure(&gpu, &dir, project, full);
    println!("testsrc2 1080p60: {fps:.0} frames/s");
    let uhd = PixelSize {
        width: 3840,
        height: 2160,
    };
    let (dir, project) = synthetic(temp.path(), uhd);
    let fps = measure(&gpu, &dir, project, full);
    println!("testsrc2 4K60 into a 1080p preview: {fps:.0} frames/s");
}
