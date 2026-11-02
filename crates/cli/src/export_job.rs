//! One export job: the project snapshot taken by `devcast export`, evaluated by
//! `dc_ops::Evaluator`, rendered and encoded by `dc_encode::export` with the project's sound mixed
//! by `dc-audio` at the levels of `fx/audio-tracks` (`crate::sound`).

use std::path::PathBuf;
use std::sync::Arc;

use dc_audio::{CHANNELS, LoudnessCache, Mixer, RATE};
use dc_core::Project;
use dc_encode::export::{Exported, ProgressFn, Timeline, export_gif, export_mp4};
use dc_encode::{AudioSettings, EncodeError, Sources};
use dc_media::cached_probe;
use dc_project::ProjectDir;
use dc_render::Gpu;
use serde::{Deserialize, Serialize};

use crate::export_args::{Format, Settings};

/// What the queue needs to run an export without the caller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Spec {
    pub root: PathBuf,
    /// The working state at the moment of `devcast export`: later edits do not change it.
    pub project: Project,
    pub settings: Settings,
    pub out: PathBuf,
}

impl Spec {
    /// What `export status` calls it: the file name and the settings.
    pub(crate) fn label(&self) -> String {
        let name = self.out.file_name().unwrap_or(self.out.as_os_str());
        format!("{} ({})", name.display(), self.settings.describe())
    }
}

pub(crate) fn run(
    gpu: &Arc<Gpu>,
    spec: &Spec,
    progress: &mut ProgressFn<'_>,
) -> Result<Exported, EncodeError> {
    let dir = ProjectDir::open(&spec.root)?;
    let project = &spec.project;
    let scene_error = |e: crate::error::CliError| EncodeError::Scene(Box::new(e));
    let evaluator = crate::scene::evaluator(&spec.root, project).map_err(scene_error)?;
    let mut source_fps = 0.0_f64;
    for clip in &project.clips {
        source_fps = source_fps.max(cached_probe(&dir, &clip.media.path)?.fps);
    }
    let levels =
        dc_ops::sound::levels(&spec.root, project).map_err(|e| EncodeError::Sound(Box::new(e)))?;
    let mut loudness = LoudnessCache::default();
    let mut mixer = Mixer::new(&dir, project, &levels, &mut loudness)
        .map_err(|e| EncodeError::Sound(Box::new(e)))?;
    let mut scene = |t| evaluator.evaluate(t);
    let mut sound = |start, out: &mut [f32]| {
        mixer
            .render(start, out)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
    };
    let mut timeline = Timeline {
        duration: evaluator.duration(),
        canvas: evaluator.canvas(),
        source_fps,
        scene: &mut scene,
        audio: AudioSettings {
            rate: RATE,
            channels: CHANNELS as u16,
        },
        sound: &mut sound,
    };
    let mut compositor = crate::scene::compositor(gpu, &spec.root).map_err(scene_error)?;
    let mut sources = Sources::new(&dir);
    let settings = &spec.settings;
    match settings.format {
        Format::Mp4 => export_mp4(
            gpu,
            &mut compositor,
            &mut sources,
            &mut timeline,
            &settings.mp4(),
            &spec.out,
            progress,
        ),
        Format::Gif => export_gif(
            gpu,
            &mut compositor,
            &mut sources,
            &mut timeline,
            &settings.gif(),
            &spec.out,
            progress,
        ),
    }
}
