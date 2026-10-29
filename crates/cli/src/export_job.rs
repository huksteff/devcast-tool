//! One export job: the project snapshot taken by `devcast export`, rendered and encoded by
//! `dc_encode::export` with the project's sound mixed by `dc-audio`.

use std::path::PathBuf;
use std::sync::Arc;

use dc_audio::{CHANNELS, Levels, LoudnessCache, Mixer, RATE};
use dc_core::{Project, TimeMap};
use dc_encode::export::{
    Exported, ProgressFn, Timeline, canvas, clip_scene, export_gif, export_mp4,
};
use dc_encode::{AudioSettings, EncodeError, Sources};
use dc_media::cached_probe;
use dc_project::ProjectDir;
use dc_render::{Compositor, Gpu};
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
    let canvas = canvas(project).ok_or(EncodeError::Empty)?;
    let mut source_fps = 0.0_f64;
    for clip in &project.clips {
        source_fps = source_fps.max(cached_probe(&dir, &clip.media.path)?.fps);
    }
    let map = TimeMap::build(project);
    let mut loudness = LoudnessCache::default();
    let mut mixer = Mixer::new(&dir, project, &Levels::default(), &mut loudness)
        .map_err(|e| EncodeError::Sound(Box::new(e)))?;
    let mut scene = |t| clip_scene(project, &map, canvas, t);
    let mut sound = |start, out: &mut [f32]| {
        mixer
            .render(start, out)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
    };
    let mut timeline = Timeline {
        duration: map.duration(),
        canvas,
        source_fps,
        scene: &mut scene,
        audio: AudioSettings {
            rate: RATE,
            channels: CHANNELS as u16,
        },
        sound: &mut sound,
    };
    let mut compositor = Compositor::new(gpu);
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
