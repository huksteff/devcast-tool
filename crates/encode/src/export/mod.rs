//! Export of a project (SCOPE, «Экспорт»): every output frame drawn by the same `Compositor` as
//! the preview and `devcast frame` (ARCHITECTURE, decision 4), then encoded.
//!
//! The export does not know effects: a [`Timeline`] hands it the scene of every output moment
//! and the mixed sound. Until effects have `evaluate` (5.15+), the scene is [`clip_scene`].

mod mp4;
mod scene;
mod settings;

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use dc_core::{OutputTime, PixelSize, TimeDelta};
use dc_render::Scene;

use crate::audio::AudioSettings;
use crate::codec::Codec;
use crate::error::EncodeError;

pub use mp4::export_mp4;
pub use scene::{canvas, clip_scene};
pub use settings::{Mp4Settings, Resolution};

/// Fills interleaved samples of [`Timeline::sound`] from output sample `start`.
pub type SoundFn<'a> =
    dyn FnMut(i64, &mut [f32]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + 'a;

/// What the export shows and plays.
pub struct Timeline<'a> {
    pub duration: TimeDelta,
    /// Size the scenes are laid out in; the export scales it to its resolution.
    pub canvas: PixelSize,
    /// The highest frame rate among the project's videos: 120 fps needs a 120 fps recording.
    pub source_fps: f64,
    pub scene: &'a mut dyn FnMut(OutputTime) -> Scene,
    /// Format of what `sound` gives.
    pub audio: AudioSettings,
    pub sound: &'a mut SoundFn<'a>,
}

/// How far the export is, in output frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

/// Called after every frame; `Break` cancels the export.
pub type ProgressFn<'a> = dyn FnMut(Progress) -> ControlFlow<()> + 'a;

/// A written file.
#[derive(Debug, Clone, PartialEq)]
pub struct Exported {
    pub path: PathBuf,
    pub size: PixelSize,
    pub fps: u32,
    pub frames: u64,
    pub duration: TimeDelta,
    /// Video codec of an MP4.
    pub codec: Option<Codec>,
    pub bytes: u64,
}

/// Output frames of `duration` at `fps`: the last one may show less than a full frame time.
pub(crate) fn frame_count(duration: TimeDelta, fps: u32) -> u64 {
    let micros = duration.micros().max(0) as u128 * u128::from(fps);
    micros.div_ceil(1_000_000) as u64
}

/// Output moment of frame `n` at `fps`.
pub(crate) fn frame_time(n: u64, fps: u32) -> OutputTime {
    let micros = u128::from(n) * 1_000_000 / u128::from(fps);
    OutputTime::from_micros(micros as i64)
}

/// The file is written next to `out` under a temporary name and renamed when complete, so a
/// failed or cancelled export neither leaves a broken file nor destroys an older one.
pub(crate) struct PartFile {
    part: PathBuf,
    out: PathBuf,
}

impl PartFile {
    pub(crate) fn new(out: &Path) -> Self {
        let mut name = out.file_name().unwrap_or_default().to_os_string();
        name.push(".part");
        Self {
            part: out.with_file_name(name),
            out: out.to_path_buf(),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.part
    }

    /// Renames the finished file to `out`; returns its size in bytes.
    pub(crate) fn commit(self) -> Result<u64, EncodeError> {
        let io = |action, path: &Path| {
            let path = path.to_path_buf();
            move |source| EncodeError::Io {
                action,
                path,
                source,
            }
        };
        std::fs::rename(&self.part, &self.out).map_err(io("переименовать в", &self.out))?;
        let bytes = std::fs::metadata(&self.out)
            .map_err(io("прочитать", &self.out))?
            .len();
        Ok(bytes)
    }
}

impl Drop for PartFile {
    fn drop(&mut self) {
        // After `commit` there is nothing here; after a failure the error is already reported.
        let _ = std::fs::remove_file(&self.part);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_cover_the_whole_duration() {
        let us = TimeDelta::from_micros;
        assert_eq!(frame_count(us(1_000_000), 60), 60);
        assert_eq!(frame_count(us(1_000_001), 60), 61);
        assert_eq!(frame_count(us(20_016_666), 30), 601);
        assert_eq!(frame_count(us(0), 30), 0);
        assert_eq!(frame_time(3, 60), OutputTime::from_micros(50_000));
    }
}
