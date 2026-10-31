//! The render thread on the software adapter (like CI) with the fixture recording
//! `clicks.dcproj`: its frame is the frame of `devcast frame` byte for byte.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use dc_core::{OutputTime, PixelSize, Project};
use dc_encode::Sources;
use dc_ops::Evaluator;
use dc_project::ProjectDir;
use dc_render::{AdapterChoice, Compositor, Gpu, Image, TextMeasure};

use super::fetch::Want;
use super::worker::{Answer, Request, Worker};

/// A copy of `clicks.dcproj` in a folder with Cyrillic and a space; lives while the guard does.
fn fixture() -> (tempfile::TempDir, ProjectDir, Project) {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/clicks.dcproj");
    let source = ProjectDir::open(fixture).unwrap();
    let project = source.load().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let dir = source
        .save_as(&project, temp.path().join("превью проекта.dcproj"))
        .unwrap();
    (temp, dir, project)
}

fn start(gpu: &Arc<Gpu>, dir: &ProjectDir) -> (Worker, Receiver<()>) {
    let (woke, wakes) = channel();
    let worker = Worker::start(Arc::clone(gpu), dir.clone(), move || {
        // The test may stop listening first.
        let _ = woke.send(());
    })
    .unwrap();
    (worker, wakes)
}

/// Waits for the wake-up of the next answer.
fn next(worker: &Worker, wakes: &Receiver<()>) -> Answer {
    loop {
        wakes.recv().unwrap();
        if let Some(answer) = worker.answers().next() {
            return answer;
        }
    }
}

fn ask(worker: &Worker, generation: u64, project: &Arc<Project>, secs: f64, size: PixelSize) {
    let want = Want {
        revision: 1,
        time: OutputTime::from_secs_f64(secs),
        size,
    };
    let project = Arc::clone(project);
    assert!(worker.send(Request {
        generation,
        project,
        want
    }));
}

/// What `devcast frame --at <secs>` renders (`dc-cli` `scene.rs` and `still.rs`).
fn devcast_frame(gpu: &Gpu, dir: &ProjectDir, project: &Project, secs: f64) -> Image {
    let (cursors, keys) = dc_capture::project::telemetry_tracks(dir.root(), project).unwrap();
    let mut measure = TextMeasure::new();
    measure.load_builtin_fonts().unwrap();
    let evaluator = Evaluator::new(project, cursors, keys, &mut measure).unwrap();
    let scene = evaluator.evaluate(OutputTime::from_secs_f64(secs));
    let mut compositor = Compositor::new(gpu);
    compositor.load_builtin_fonts().unwrap();
    Sources::new(dir)
        .feed(gpu, &mut compositor, &scene)
        .unwrap();
    compositor.render(gpu, &scene).unwrap()
}

#[test]
fn preview_frame_is_the_frame_of_devcast_frame() {
    let gpu = Arc::new(Gpu::new(AdapterChoice::Software).unwrap());
    let (_temp, dir, project) = fixture();
    let canvas = PixelSize {
        width: 1280,
        height: 720,
    };
    let (worker, wakes) = start(&gpu, &dir);
    let shared = Arc::new(project.clone());
    ask(&worker, 1, &shared, 2.0, canvas);
    let (generation, answer) = next(&worker, &wakes);
    assert_eq!(generation, 1);
    let rendered = answer.unwrap();
    let preview = gpu.read(&rendered.texture).unwrap();
    let expected = devcast_frame(&gpu, &dir, &project, 2.0);
    assert_eq!(preview.size, canvas);
    assert!(
        preview.rgba == expected.rgba,
        "the preview differs from devcast frame"
    );
    assert!(
        preview
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[..3] != [0, 0, 0]),
        "the frame is not black"
    );
}

#[test]
fn the_texture_is_reused_while_the_size_stays() {
    let gpu = Arc::new(Gpu::new(AdapterChoice::Software).unwrap());
    let (_temp, dir, project) = fixture();
    let (worker, wakes) = start(&gpu, &dir);
    let project = Arc::new(project);
    let half = PixelSize {
        width: 640,
        height: 360,
    };
    ask(&worker, 1, &project, 1.0, half);
    let first = next(&worker, &wakes).1.unwrap();
    assert_eq!((first.texture.width(), first.texture.height()), (640, 360));
    ask(&worker, 2, &project, 3.0, half);
    let second = next(&worker, &wakes).1.unwrap();
    assert_eq!(second.serial, first.serial, "same size, same texture");
    let small = PixelSize {
        width: 320,
        height: 180,
    };
    ask(&worker, 3, &project, 3.0, small);
    let third = next(&worker, &wakes).1.unwrap();
    assert_ne!(third.serial, first.serial);
    assert_eq!(third.texture.width(), 320);
}

#[test]
fn a_project_without_clips_answers_why() {
    let gpu = Arc::new(Gpu::new(AdapterChoice::Software).unwrap());
    let (_temp, dir, mut project) = fixture();
    project.clips.clear();
    let (worker, wakes) = start(&gpu, &dir);
    let size = PixelSize {
        width: 64,
        height: 36,
    };
    ask(&worker, 1, &Arc::new(project), 0.0, size);
    let error = next(&worker, &wakes).1.unwrap_err();
    assert!(error.contains("нет клипов"), "{error}");
}
