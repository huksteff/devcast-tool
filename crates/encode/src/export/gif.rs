//! GIF export through gifski (ARCHITECTURE): frames drawn by the same `Compositor` as the MP4,
//! read back as RGBA and quantized by gifski on its own threads while the next frames render.

use std::fs::File;
use std::io::BufWriter;
use std::ops::ControlFlow;
use std::path::Path;

use dc_render::{Compositor, Gpu};

use crate::error::EncodeError;
use crate::still::Sources;

use super::settings::check_fps;
use super::{
    Exported, PartFile, Progress, ProgressFn, Resolution, Timeline, frame_count, frame_time,
};

/// Frame rates of a GIF (SCOPE): more is heavy and many players ignore it.
pub const GIF_FPS: [u32; 4] = [15, 20, 25, 30];

/// gifski quality 1–100: 90 keeps screen text clean at about two thirds the size of 100.
const QUALITY: u8 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GifSettings {
    /// 720p, 1080p or the canvas.
    pub resolution: Resolution,
    pub fps: u32,
    /// Plays again and again; otherwise once.
    pub looped: bool,
}

impl GifSettings {
    fn check(&self) -> Result<(), EncodeError> {
        check_fps(self.fps, &GIF_FPS, "GIF")?;
        match self.resolution {
            Resolution::P720 | Resolution::P1080 | Resolution::Source => Ok(()),
            Resolution::P1440 | Resolution::P2160 => Err(EncodeError::GifResolution),
        }
    }
}

/// Exports the picture of `timeline` (GIF has no sound) into `out`. `progress` hears of every
/// frame and may cancel; a cancelled or failed export leaves no file at `out`.
pub fn export_gif(
    gpu: &Gpu,
    compositor: &mut Compositor,
    sources: &mut Sources<'_>,
    timeline: &mut Timeline<'_>,
    settings: &GifSettings,
    out: &Path,
    progress: &mut ProgressFn<'_>,
) -> Result<Exported, EncodeError> {
    settings.check()?;
    if timeline.duration <= dc_core::TimeDelta::ZERO {
        return Err(EncodeError::NothingLeft);
    }
    let size = settings.resolution.size(timeline.canvas);
    let fps = settings.fps;
    let repeat = if settings.looped {
        gifski::Repeat::Infinite
    } else {
        gifski::Repeat::Finite(0)
    };
    let (collector, writer) = gifski::new(gifski::Settings {
        // Without a size gifski shrinks frames larger than 800×600 on its own.
        width: Some(size.width),
        height: Some(size.height),
        quality: QUALITY,
        fast: false,
        repeat,
    })
    .map_err(gif_error)?;
    let part = PartFile::new(out);
    let file = File::create(part.path()).map_err(|source| EncodeError::Io {
        action: "создать",
        path: part.path().to_path_buf(),
        source,
    })?;
    let target = gpu.target(size);
    let total = frame_count(timeline.duration, fps);
    std::thread::scope(|scope| {
        let written = scope.spawn(move || {
            writer.write(BufWriter::new(file), &mut gifski::progress::NoProgress {})
        });
        let drawn = (|| {
            for n in 0..total {
                let scene = (timeline.scene)(frame_time(n, fps));
                sources.feed(gpu, compositor, &scene)?;
                compositor.draw(gpu, &scene, &target)?;
                let image = gpu.read(&target)?;
                let pixels = image
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|&[r, g, b, a]| rgb::RGBA8 { r, g, b, a })
                    .collect();
                let frame = imgref::ImgVec::new(pixels, size.width as usize, size.height as usize);
                let seconds = n as f64 / f64::from(fps);
                collector
                    .add_frame_rgba(n as usize, frame, seconds)
                    .map_err(gif_error)?;
                if progress(Progress { done: n + 1, total }) == ControlFlow::Break(()) {
                    return Err(EncodeError::Cancelled);
                }
            }
            Ok(())
        })();
        // Closing the collector lets the writer finish (or stop after a failure).
        drop(collector);
        let written = written
            .join()
            .map_err(|_| EncodeError::Gif("поток gifski упал".to_owned()))?;
        drawn?;
        written.map_err(gif_error)
    })?;
    Ok(Exported {
        path: out.to_path_buf(),
        size,
        fps,
        frames: total,
        duration: timeline.duration,
        codec: None,
        bytes: part.commit()?,
    })
}

fn gif_error(error: gifski::Error) -> EncodeError {
    EncodeError::Gif(error.to_string())
}
