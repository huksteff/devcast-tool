//! Projects of test files: imported clips with cuts and speeds, the speech fixture, and the
//! whole output of a mixer.

use std::path::{Path, PathBuf};

use dc_audio::{CHANNELS, Levels, LoudnessCache, Mixer};
use dc_core::geom::PixelSize;
use dc_core::project::{Clip, Media, MediaKind, SpeedRange};
use dc_core::time::Source;
use dc_core::{Project, Range, Rect, SourceTime, TimeDelta};
use dc_project::ProjectDir;

pub fn speech_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/speech.devcast")
}

/// The project folder `dir`; a folder of test files becomes one with an empty `project.json`.
pub fn project_dir(dir: &Path) -> ProjectDir {
    let file = dir.join("project.json");
    if !file.exists() {
        std::fs::write(&file, serde_json::to_string(&Project::default()).unwrap()).unwrap();
    }
    ProjectDir::open(dir).unwrap()
}

pub fn speech_project() -> Project {
    let text = std::fs::read_to_string(speech_dir().join("project.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

pub fn range(from: f64, to: f64) -> Range<Source> {
    Range::new(
        SourceTime::from_secs_f64(from),
        SourceTime::from_secs_f64(to),
    )
    .unwrap()
}

/// Appends a clip of the whole imported file `path` (relative to the project folder),
/// `seconds` long, and returns it for cuts and speeds.
pub fn add_clip<'a>(project: &'a mut Project, path: &str, seconds: f64) -> &'a mut Clip {
    let duration = TimeDelta::from_secs_f64(seconds);
    let id = project.clip_ids.allocate();
    project.clips.push(Clip {
        id,
        media: Media {
            path: path.to_owned(),
            kind: MediaKind::Imported,
            duration,
            size: PixelSize {
                width: 2,
                height: 2,
            },
        },
        trim: range(0.0, seconds),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
        audio: Default::default(),
    });
    project.clips.last_mut().unwrap()
}

pub fn speed(from: f64, to: f64, factor: f64) -> SpeedRange {
    SpeedRange {
        range: range(from, to),
        factor,
    }
}

pub fn mixer(dir: &Path, project: &Project, levels: &Levels) -> Mixer {
    Mixer::new(
        &project_dir(dir),
        project,
        levels,
        &mut LoudnessCache::default(),
    )
    .unwrap()
}

/// The whole output of `mixer` (and a second past it), rendered in order in `chunk` frames.
pub fn render_all(mixer: &mut Mixer, chunk: usize) -> Vec<f32> {
    let frames = mixer.frames() as usize + 48_000;
    let mut out = vec![0.0; frames * CHANNELS];
    for (n, part) in out.chunks_mut(chunk * CHANNELS).enumerate() {
        mixer.render((n * chunk) as i64, part).unwrap();
    }
    out
}

/// The largest second difference of `mono` in `from..to`: how sharp the worst corner is. A
/// sine of amplitude `a` and frequency `f` has `a · (2π f / RATE)²`; a click is far above.
pub fn sharpest(mono: &[f32], from: usize, to: usize) -> f32 {
    (from.max(1)..to.min(mono.len() - 1))
        .map(|i| (mono[i + 1] - 2.0 * mono[i] + mono[i - 1]).abs())
        .fold(0.0, f32::max)
}
