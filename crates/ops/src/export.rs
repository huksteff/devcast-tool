//! One export, the same for `devcast export` and the editor's «Экспорт» (task 7.31): the
//! settings the user picks, the project snapshot, and the job that evaluates it
//! (`Evaluator`), renders and encodes it (`dc_encode::export`) with the sound mixed by
//! `dc-audio` at the levels of `fx/audio-tracks` (`sound`). Both callers build a [`Spec`] and
//! run [`run`] in `dc_encode::ExportQueue`, so they write the same file.

use std::path::PathBuf;

use dc_audio::{CHANNELS, LoudnessCache, Mixer, RATE};
use dc_core::Project;
use dc_encode::export::{Exported, ProgressFn, Timeline, export_gif, export_mp4};
use dc_encode::{AudioSettings, EncodeError, Sources};
use dc_media::cached_probe;
use dc_project::ProjectDir;
use dc_render::Gpu;
use serde::{Deserialize, Serialize};

/// The user's choices live with the settings the editor remembers (`dc-settings`), so the
/// dialog's last choices are the same type as the job's.
pub use dc_settings::export::{Format, Quality, Res, Settings};

/// What the queue needs to run an export without the caller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub root: PathBuf,
    /// The working state when the export was asked for, unsaved edits included: later edits
    /// do not change it.
    pub project: Project,
    pub settings: Settings,
    /// Absolute: the queue may run in another process with another folder.
    pub out: PathBuf,
}

impl Spec {
    /// The file name and the settings: `урок.mp4 (mp4 1080p 60 fps high)`.
    pub fn label(&self) -> String {
        format!("{} ({})", self.file_name(), self.settings.describe())
    }

    /// The name of the output file.
    pub fn file_name(&self) -> String {
        let name = self.out.file_name().unwrap_or(self.out.as_os_str());
        name.to_string_lossy().into_owned()
    }
}

/// Renders and encodes `spec` on `gpu`; `progress` hears of every frame and may cancel.
pub fn run(gpu: &Gpu, spec: &Spec, progress: &mut ProgressFn<'_>) -> Result<Exported, EncodeError> {
    let dir = ProjectDir::open(&spec.root)?;
    let project = &spec.project;
    let scene_error = |e: crate::scene::SceneError| EncodeError::Scene(Box::new(e));
    let evaluator = crate::scene::evaluator(&spec.root, project).map_err(scene_error)?;
    let mut source_fps = 0.0_f64;
    for clip in &project.clips {
        source_fps = source_fps.max(cached_probe(&dir, &clip.media.path)?.fps);
    }
    let levels =
        crate::sound::levels(&spec.root, project).map_err(|e| EncodeError::Sound(Box::new(e)))?;
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
