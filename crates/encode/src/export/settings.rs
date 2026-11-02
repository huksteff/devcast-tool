//! What the user picks for an export (SCOPE, «Экспорт»): resolution, frame rate, quality.

use dc_core::PixelSize;

use crate::codec::Quality;
use crate::error::EncodeError;

/// Size of the exported frame by its shorter side; the longer one keeps the canvas' aspect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resolution {
    P720,
    P1080,
    P1440,
    P2160,
    /// The canvas as it is.
    Source,
}

impl Resolution {
    /// The frame for `canvas`, both sides even (NV12 halves the colour planes).
    pub fn size(self, canvas: PixelSize) -> PixelSize {
        let short = match self {
            Self::P720 => 720,
            Self::P1080 => 1080,
            Self::P1440 => 1440,
            Self::P2160 => 2160,
            Self::Source => canvas.width.min(canvas.height),
        };
        let scale = f64::from(short) / f64::from(canvas.width.min(canvas.height));
        let even = |side: u32| ((f64::from(side) * scale / 2.0).round() as u32).max(1) * 2;
        PixelSize {
            width: even(canvas.width),
            height: even(canvas.height),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mp4Settings {
    pub resolution: Resolution,
    pub fps: u32,
    pub quality: Quality,
}

/// Frame rates of an MP4; 120 only when a recording has it.
pub const MP4_FPS: [u32; 3] = [30, 60, 120];

/// A recording "at 120 fps" whose average comes out a little lower still counts.
pub const FPS_120_FROM: f64 = 100.0;

impl Mp4Settings {
    pub(crate) fn check(&self, source_fps: f64) -> Result<(), EncodeError> {
        check_fps(self.fps, &MP4_FPS, "MP4")?;
        if self.fps == 120 && source_fps < FPS_120_FROM {
            return Err(EncodeError::Fps120 { source_fps });
        }
        Ok(())
    }
}

pub(crate) fn check_fps(
    fps: u32,
    allowed: &[u32],
    format: &'static str,
) -> Result<(), EncodeError> {
    if allowed.contains(&fps) {
        return Ok(());
    }
    let allowed = allowed
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    Err(EncodeError::Fps {
        fps,
        format,
        allowed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(width: u32, height: u32) -> PixelSize {
        PixelSize { width, height }
    }

    #[test]
    fn resolution_sets_the_shorter_side_and_keeps_the_aspect() {
        assert_eq!(Resolution::P1080.size(px(1280, 720)), px(1920, 1080));
        assert_eq!(Resolution::P2160.size(px(1280, 720)), px(3840, 2160));
        assert_eq!(Resolution::P720.size(px(1080, 1920)), px(720, 1280));
        assert_eq!(Resolution::Source.size(px(1281, 719)), px(1282, 720));
        assert_eq!(Resolution::P720.size(px(1000, 1000)), px(720, 720));
    }

    #[test]
    fn frame_rates_follow_scope() {
        let settings = |fps| Mp4Settings {
            resolution: Resolution::Source,
            fps,
            quality: Quality::High,
        };
        assert!(settings(60).check(30.0).is_ok());
        assert!(matches!(
            settings(25).check(30.0),
            Err(EncodeError::Fps { .. })
        ));
        assert!(matches!(
            settings(120).check(60.0),
            Err(EncodeError::Fps120 { .. })
        ));
        assert!(settings(120).check(119.6).is_ok());
    }
}
