//! One export, the same for `devcast export` and the editor's «Экспорт» (task 7.31): the
//! settings the user picks, the project snapshot, and the job that evaluates it
//! (`Evaluator`), renders and encodes it (`dc_encode::export`) with the sound mixed by
//! `dc-audio` at the levels of `fx/audio-tracks` (`sound`). Both callers build a [`Spec`] and
//! run [`run`] in `dc_encode::ExportQueue`, so they write the same file.

use std::path::PathBuf;

use dc_audio::{CHANNELS, LoudnessCache, Mixer, RATE};
use dc_core::Project;
use dc_encode::export::{
    Exported, GifSettings, Mp4Settings, ProgressFn, Resolution, Timeline, export_gif, export_mp4,
};
use dc_encode::{AudioSettings, EncodeError, Sources};
use dc_media::cached_probe;
use dc_project::ProjectDir;
use dc_render::Gpu;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Mp4,
    Gif,
}

/// Size of the exported frame by its shorter side; `Source` is the canvas as it is. GIF has
/// no 1440p and 4K.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Res {
    P720,
    P1080,
    P1440,
    P2160,
    Source,
}

/// Constant quality of the MP4 codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Quality {
    Low,
    Medium,
    High,
}

/// What the user picks for one export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Settings {
    pub format: Format,
    pub res: Res,
    /// MP4: 30, 60 or 120 (a recording of ≥ 100 fps); GIF: 15, 20, 25 or 30.
    pub fps: u32,
    /// MP4 only.
    pub quality: Quality,
    /// GIF only: plays again and again.
    pub looped: bool,
}

impl Settings {
    /// What an export of `format` starts with (owner decision): MP4 1080p, 60 fps, high
    /// quality; GIF 1080p, 15 fps, looped.
    pub fn default_for(format: Format) -> Self {
        Self {
            format,
            res: Res::P1080,
            fps: match format {
                Format::Mp4 => 60,
                Format::Gif => 15,
            },
            quality: Quality::High,
            looped: true,
        }
    }

    pub fn resolution(&self) -> Resolution {
        match self.res {
            Res::P720 => Resolution::P720,
            Res::P1080 => Resolution::P1080,
            Res::P1440 => Resolution::P1440,
            Res::P2160 => Resolution::P2160,
            Res::Source => Resolution::Source,
        }
    }

    pub fn mp4(&self) -> Mp4Settings {
        Mp4Settings {
            resolution: self.resolution(),
            fps: self.fps,
            quality: match self.quality {
                Quality::Low => dc_encode::Quality::Low,
                Quality::Medium => dc_encode::Quality::Medium,
                Quality::High => dc_encode::Quality::High,
            },
        }
    }

    pub fn gif(&self) -> GifSettings {
        GifSettings {
            resolution: self.resolution(),
            fps: self.fps,
            looped: self.looped,
        }
    }

    /// One line: `mp4 1080p 60 fps high`.
    pub fn describe(&self) -> String {
        let res = match self.res {
            Res::P720 => "720p",
            Res::P1080 => "1080p",
            Res::P1440 => "1440p",
            Res::P2160 => "4k",
            Res::Source => "source",
        };
        match self.format {
            Format::Mp4 => {
                let quality = match self.quality {
                    Quality::Low => "low",
                    Quality::Medium => "medium",
                    Quality::High => "high",
                };
                format!("mp4 {res} {} fps {quality}", self.fps)
            }
            Format::Gif => {
                let looped = if self.looped {
                    "по кругу"
                } else {
                    "один раз"
                };
                format!("gif {res} {} fps {looped}", self.fps)
            }
        }
    }
}

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
pub fn run(
    gpu: &Gpu,
    spec: &Spec,
    progress: &mut ProgressFn<'_>,
) -> Result<Exported, EncodeError> {
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
