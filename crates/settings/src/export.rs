//! What the user picks for one export, the same for `devcast export` and the editor's dialog
//! (`dc_ops::export` re-exports it), and the dialog's last choices kept in `settings.json`
//! (owner decision 2026-11-02): the format and the settings of each format.

use dc_encode::export::{GifSettings, Mp4Settings, Resolution};
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
    /// What an export of `format` starts with (owner decisions): MP4 1080p, 60 fps, high
    /// quality; GIF 720p, 15 fps, looped.
    pub fn default_for(format: Format) -> Self {
        let (res, fps) = match format {
            Format::Mp4 => (Res::P1080, 60),
            Format::Gif => (Res::P720, 15),
        };
        Self {
            format,
            res,
            fps,
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

/// The last choices of the editor's export dialog: it opens with them. Each format keeps its
/// own settings, so going MP4 → GIF → MP4 loses nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportChoices {
    pub format: Format,
    pub mp4: Settings,
    pub gif: Settings,
}

impl Default for ExportChoices {
    fn default() -> Self {
        Self {
            format: Format::Mp4,
            mp4: Settings::default_for(Format::Mp4),
            gif: Settings::default_for(Format::Gif),
        }
    }
}

impl ExportChoices {
    /// The settings of the chosen format.
    pub fn settings(&self) -> Settings {
        match self.format {
            Format::Mp4 => self.mp4,
            Format::Gif => self.gif,
        }
    }
}
