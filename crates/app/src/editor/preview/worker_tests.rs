//! The render thread on the software adapter (like CI) with the fixture recording
//! `clicks.devcast`: its frame is the frame of `devcast frame` byte for byte.

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

/// A copy of `clicks.devcast` in a folder with Cyrillic and a space; lives while the guard does.
fn fixture() -> (tempfile::TempDir, ProjectDir, Project) {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/clicks.devcast");
    let source = ProjectDir::open(fixture).unwrap();
    let project = source.load().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let dir = source
        .save_as(&project, temp.path().join("превью проекта.devcast"))
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
    ask_for(worker, generation, project, secs, (size, false));
}

/// Asks for the frame at `secs` of `size`, or only the source picture of the clip (`true`);
/// each request is a new revision of the project.
fn ask_for(
    worker: &Worker,
    generation: u64,
    project: &Arc<Project>,
    secs: f64,
    (size, source): (PixelSize, bool),
) {
    let want = Want {
        revision: generation,
        time: OutputTime::from_secs_f64(secs),
        size,
        scrub: false,
        source,
        camera: true,
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
    dc_ops::fonts::load(dir.root(), |d| measure.load_fonts(d)).unwrap();
    let evaluator = Evaluator::new(project, cursors, keys, &mut measure).unwrap();
    let scene = evaluator.evaluate(OutputTime::from_secs_f64(secs));
    let mut compositor = Compositor::new(gpu);
    dc_ops::fonts::load(dir.root(), |d| compositor.load_fonts(d)).unwrap();
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
    assert_eq!(second.texture, first.texture, "same size, same texture");
    let small = PixelSize {
        width: 320,
        height: 180,
    };
    ask(&worker, 3, &project, 3.0, small);
    let third = next(&worker, &wakes).1.unwrap();
    assert_ne!(third.texture, first.texture);
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

#[test]
fn the_source_picture_ignores_the_crop_and_what_lies_over_the_video() {
    let gpu = Arc::new(Gpu::new(AdapterChoice::Software).unwrap());
    let (_temp, dir, project) = fixture();
    let mut cropped = project.clone();
    cropped.clips[0].crop = dc_core::Rect::new(0.25, 0.25, 0.5, 0.5).unwrap();
    let source = cropped.clips[0].media.size;
    let size = PixelSize {
        width: source.width / 2,
        height: source.height / 2,
    };
    let (worker, wakes) = start(&gpu, &dir);
    let picture = |generation, project: Project, source| {
        ask_for(&worker, generation, &Arc::new(project), 2.0, (size, source));
        let (answered, frame) = next(&worker, &wakes);
        assert_eq!(answered, generation);
        gpu.read(&frame.unwrap().texture).unwrap().rgba
    };
    let whole = picture(1, project, true);
    let of_cropped = picture(2, cropped.clone(), true);
    let frame = picture(3, cropped, false);
    assert!(whole == of_cropped, "the crop does not change the picture");
    assert!(whole != frame, "the frame shows the crop and the cursor");
    assert!(whole.as_chunks::<4>().0.iter().any(|p| p[..3] != [0, 0, 0]));
}

/// `project` with one fixed 2× zoom from 1 s to 4 s of the output instead of its zooms.
fn zoomed(mut project: Project) -> Project {
    let mut state = project.state::<dc_fx_zoom::ZoomEffect>().unwrap();
    state.zooms.clear();
    project.set_state::<dc_fx_zoom::ZoomEffect>(&state).unwrap();
    let registry = dc_ops::Registry::devcast().unwrap();
    let words: Vec<String> = "zoom add --at 1 --duration 3 --scale 2 --mode fixed"
        .split(' ')
        .map(str::to_owned)
        .collect();
    let (info, taken) = registry.resolve(&words).unwrap();
    let args = dc_ops::parse_args(info, &words[taken..]).unwrap();
    let steps = [dc_ops::Step::new(info.name, args)];
    let mut history = dc_core::History::default();
    dc_ops::apply(&registry, &mut project, &mut history, &steps, false).unwrap();
    project
}

#[test]
fn without_the_camera_the_frame_is_the_whole_picture() {
    let gpu = Arc::new(Gpu::new(AdapterChoice::Software).unwrap());
    let (_temp, dir, project) = fixture();
    let project = Arc::new(zoomed(project));
    let (worker, wakes) = start(&gpu, &dir);
    let size = PixelSize {
        width: 320,
        height: 180,
    };
    let frame = |generation, camera| {
        let want = Want {
            revision: 1,
            time: OutputTime::from_secs_f64(2.5),
            size,
            scrub: false,
            source: false,
            camera,
        };
        let project = Arc::clone(&project);
        assert!(worker.send(Request {
            generation,
            project,
            want
        }));
        let rendered = next(&worker, &wakes).1.unwrap();
        (rendered.camera, gpu.read(&rendered.texture).unwrap().rgba)
    };
    let (camera, zoomed_in) = frame(1, true);
    assert!(camera.scale > 1.9, "the zoom is in: {camera:?}");
    let (still, whole) = frame(2, false);
    assert_eq!(still, dc_render::Camera::IDENTITY);
    assert!(zoomed_in != whole, "the camera is left out");
}

/// `project` with black words in family `font` over 1–4 s of the output (the fixture's screen is
/// white).
fn with_text(mut project: Project, font: &str) -> Project {
    let registry = dc_ops::Registry::devcast().unwrap();
    let lines: [&[&str]; 2] = [
        &["annot", "add", "text", "--at", "1", "--duration", "3"],
        &[
            "text",
            "set",
            "a1",
            "--text",
            "DEVCAST",
            "--size",
            "96",
            "--font",
            font,
            "--color",
            "#000000ff",
        ],
    ];
    let steps: Vec<_> = lines
        .iter()
        .map(|words| {
            let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
            let (info, taken) = registry.resolve(&words).unwrap();
            let args = dc_ops::parse_args(info, &words[taken..]).unwrap();
            dc_ops::Step::new(info.name, args)
        })
        .collect();
    let mut history = dc_core::History::default();
    dc_ops::apply(&registry, &mut project, &mut history, &steps, false).unwrap();
    project
}

/// A font added to the project after the thread started is drawn by the next revision, as
/// `devcast frame` draws it.
#[test]
fn a_font_added_later_is_loaded_with_the_next_revision() {
    let gpu = Arc::new(Gpu::new(AdapterChoice::Software).unwrap());
    let (_temp, dir, project) = fixture();
    let canvas = PixelSize {
        width: 1280,
        height: 720,
    };
    let (worker, wakes) = start(&gpu, &dir);
    ask(&worker, 1, &Arc::new(project.clone()), 2.0, canvas);
    let plain = gpu.read(&next(&worker, &wakes).1.unwrap().texture).unwrap();
    let font =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts/PressStart2P-Regular.ttf");
    dc_ops::import_font(&dir, &font, false).unwrap();
    let project = with_text(project, "Press Start 2P");
    ask(&worker, 2, &Arc::new(project.clone()), 2.0, canvas);
    let preview = gpu.read(&next(&worker, &wakes).1.unwrap().texture).unwrap();
    let expected = devcast_frame(&gpu, &dir, &project, 2.0);
    assert!(
        preview.rgba == expected.rgba,
        "the preview differs from devcast frame"
    );
    assert!(preview.rgba != plain.rgba, "the words are drawn");
}
