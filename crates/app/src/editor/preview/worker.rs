//! The render thread of the preview: the scene of the asked moment from `dc_ops::Evaluator`, its
//! video frames and pictures decoded by `dc_encode::Sources`, drawn by `Compositor::draw` into a
//! texture of the shared device — the path of `devcast frame` and the export, so the preview and
//! the export show the same frame. The UI never waits: it sends requests and takes answers. Like
//! `dc_media::Seeker`, the thread takes only the newest request waiting; the evaluator is built
//! once per project revision, the telemetry is read again only when the recordings change. A request
//! may ask for the source picture of the clip alone (the crop dialog, `Want::source`).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use dc_core::{CursorTracks, KeyTracks, PixelSize, Project, Rect};
use dc_encode::{Seek, Sources};
use dc_ops::Evaluator;
use dc_project::ProjectDir;
use dc_render::{Band, Compositor, Gpu, Layer, PxRect, Scene, TextMeasure, Video};
use dc_ui_kit::strings::crop as text;
use eframe::wgpu;

use super::fetch::Want;

pub struct Request {
    pub generation: u64,
    /// The project of `want.revision`.
    pub project: Arc<Project>,
    pub want: Want,
}

/// A frame drawn into `texture`; the thread draws into the same texture while the size stays,
/// `serial` names the texture.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub texture: wgpu::Texture,
    pub serial: u64,
}

/// The generation of a request and its frame or why there is none.
pub type Answer = (u64, Result<Rendered, String>);

#[derive(Debug)]
pub struct Worker {
    requests: Sender<Request>,
    answers: Receiver<Answer>,
}

impl Worker {
    /// Starts the thread of the project in `dir`; `wake` is called after every answer.
    pub fn start(
        gpu: Arc<Gpu>,
        dir: ProjectDir,
        wake: impl Fn() + Send + 'static,
    ) -> std::io::Result<Self> {
        let (requests, inbox) = channel::<Request>();
        let (outbox, answers) = channel::<Answer>();
        std::thread::Builder::new()
            .name("devcast-preview".to_owned())
            .spawn(move || {
                let reply = |answer: Answer| {
                    let sent = outbox.send(answer).is_ok();
                    wake();
                    sent
                };
                serve(&gpu, &dir, &inbox, &reply);
            })?;
        Ok(Self { requests, answers })
    }

    /// Asks for a frame; `false` if the thread is gone.
    pub fn send(&self, request: Request) -> bool {
        self.requests.send(request).is_ok()
    }

    /// The answers that came since the last call, oldest first.
    pub fn answers(&self) -> impl Iterator<Item = Answer> + '_ {
        self.answers.try_iter()
    }
}

/// Answers requests until the worker is dropped; a new revision rebuilds the evaluator.
fn serve(gpu: &Gpu, dir: &ProjectDir, inbox: &Receiver<Request>, reply: &dyn Fn(Answer) -> bool) {
    let Some(mut request) = newest(inbox) else {
        return;
    };
    // Built at the first request: the thread of the crop dialog may never get one.
    let mut painter = Painter::new(gpu, dir);
    loop {
        let project = Arc::clone(&request.project);
        let revision = request.want.revision;
        let evaluator = match &mut painter {
            Ok(painter) => painter.evaluator(&project),
            Err(error) => Err(error.clone()),
        };
        loop {
            let answer = match (&mut painter, &evaluator) {
                (Ok(painter), Ok(evaluator)) => painter.draw(gpu, evaluator, &request.want),
                (Err(error), _) => Err(error.clone()),
                (_, Err(error)) => Err(error.clone()),
            };
            if !reply((request.generation, answer)) {
                return;
            }
            let Some(next) = newest(inbox) else {
                return;
            };
            request = next;
            if request.want.revision != revision {
                break;
            }
        }
    }
}

/// Waits for a request and skips to the newest one waiting; `None` once the worker is dropped.
fn newest(inbox: &Receiver<Request>) -> Option<Request> {
    let mut request = inbox.recv().ok()?;
    while let Ok(newer) = inbox.try_recv() {
        request = newer;
    }
    Some(request)
}

/// Telemetry of the recordings named in `paths` (in clip order).
struct Telemetry {
    paths: Vec<String>,
    cursors: CursorTracks,
    keys: KeyTracks,
}

/// Everything the thread keeps between frames.
struct Painter<'d> {
    root: PathBuf,
    measure: TextMeasure,
    compositor: Compositor,
    sources: Sources<'d>,
    telemetry: Option<Telemetry>,
    target: Option<Rendered>,
    serial: u64,
}

impl<'d> Painter<'d> {
    /// The fonts of `devcast frame`: built-in ones, loaded once.
    fn new(gpu: &Gpu, dir: &'d ProjectDir) -> Result<Self, String> {
        let mut measure = TextMeasure::new();
        measure.load_builtin_fonts().map_err(|e| e.to_string())?;
        let mut compositor = Compositor::new(gpu);
        compositor.load_builtin_fonts().map_err(|e| e.to_string())?;
        Ok(Self {
            root: dir.root().to_path_buf(),
            measure,
            compositor,
            sources: Sources::new(dir),
            telemetry: None,
            target: None,
            serial: 0,
        })
    }

    fn evaluator<'p>(&mut self, project: &'p Project) -> Result<Evaluator<'p>, String> {
        let paths = recordings(project);
        let telemetry = match self.telemetry.take() {
            Some(known) if known.paths == paths => known,
            _ => {
                let (cursors, keys) = dc_capture::project::telemetry_tracks(&self.root, project)
                    .map_err(|e| e.to_string())?;
                Telemetry {
                    paths,
                    cursors,
                    keys,
                }
            }
        };
        let (cursors, keys) = (telemetry.cursors.clone(), telemetry.keys.clone());
        self.telemetry = Some(telemetry);
        Evaluator::new(project, cursors, keys, &mut self.measure).map_err(|e| e.to_string())
    }

    fn draw(
        &mut self,
        gpu: &Gpu,
        evaluator: &Evaluator<'_>,
        want: &Want,
    ) -> Result<Rendered, String> {
        let mut scene = evaluator.evaluate(want.time);
        if want.source {
            scene = source_scene(&scene, want.size)?;
        }
        let seek = if want.scrub {
            Seek::Keyframe
        } else {
            Seek::Exact
        };
        self.sources
            .feed_at(gpu, &mut self.compositor, &scene, seek)
            .map_err(|e| e.to_string())?;
        let target = self.target(gpu, want.size);
        self.compositor
            .draw(gpu, &scene, &target.texture)
            .map_err(|e| e.to_string())?;
        Ok(target)
    }

    /// The texture of `size`: the last one while the size stays.
    fn target(&mut self, gpu: &Gpu, size: PixelSize) -> Rendered {
        match &self.target {
            Some(target)
                if target.texture.width() == size.width
                    && target.texture.height() == size.height =>
            {
                target.clone()
            }
            _ => {
                self.serial += 1;
                let target = Rendered {
                    texture: gpu.target(size),
                    serial: self.serial,
                };
                self.target = Some(target.clone());
                target
            }
        }
    }
}

/// Media paths of the recordings on the track, in clip order: telemetry is read for them.
fn recordings(project: &Project) -> Vec<String> {
    project
        .clips
        .iter()
        .filter(|c| c.media.kind == dc_core::project::MediaKind::Recording)
        .map(|c| c.media.path.clone())
        .collect()
}

/// The scene of the crop dialog: the video of `scene`'s clip alone, its whole source frame
/// filling a `size` target (the size has the aspect of the source).
fn source_scene(scene: &Scene, size: PixelSize) -> Result<Scene, String> {
    let video = scene
        .painted()
        .find_map(|(_, layer)| match layer {
            Layer::Video(video) => Some(video),
            _ => None,
        })
        .ok_or_else(|| text::NO_PICTURE.to_owned())?;
    let mut only = Scene::new(size);
    let rect = PxRect {
        x: 0.0,
        y: 0.0,
        w: size.width as f32,
        h: size.height as f32,
    };
    let video = Video {
        source: video.source.clone(),
        crop: Rect::FULL,
        rect,
    };
    only.push(Band::Video, Layer::Video(video));
    Ok(only)
}
