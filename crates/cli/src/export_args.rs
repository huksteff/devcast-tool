//! Arguments of `devcast export` and what they turn into: the format, the settings of
//! `dc_encode::export` and the output file.

use std::path::{Component, Path, PathBuf};

use dc_encode::Quality;
use dc_encode::export::{GifSettings, Mp4Settings, Resolution};
use serde::{Deserialize, Serialize};

use crate::error::CliError;

#[derive(Debug, Clone, Default, clap::Args)]
pub(crate) struct ExportArgs {
    #[arg(
        short = 'o',
        long,
        help = "Файл .mp4 или .gif; формат берётся из расширения"
    )]
    pub out: Option<PathBuf>,
    #[arg(long, value_enum, help = "mp4 или gif, если расширение файла другое")]
    pub format: Option<Format>,
    #[arg(
        long,
        value_enum,
        help = "Размер по короткой стороне: 720p, 1080p, 1440p, 4k (только mp4) или source — как в проекте [по умолчанию 1080p]"
    )]
    pub res: Option<Res>,
    #[arg(
        long,
        help = "Кадров в секунду: mp4 — 30, 60 или 120 (для записей 120) [60]; gif — 15, 20, 25 или 30 [15]"
    )]
    pub fps: Option<u32>,
    #[arg(long, value_enum, help = "Качество mp4: low, medium, high [high]")]
    pub quality: Option<QualityArg>,
    #[arg(long = "loop", value_enum, help = "Зацикливание gif: on или off [on]")]
    pub looped: Option<Switch>,
}

impl ExportArgs {
    pub(crate) fn is_empty(&self) -> bool {
        self.out.is_none()
            && self.format.is_none()
            && self.res.is_none()
            && self.fps.is_none()
            && self.quality.is_none()
            && self.looped.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Format {
    Mp4,
    Gif,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
pub(crate) enum Res {
    #[value(name = "720p")]
    P720,
    #[value(name = "1080p")]
    P1080,
    #[value(name = "1440p")]
    P1440,
    #[value(name = "4k")]
    P2160,
    Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
pub(crate) enum QualityArg {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
pub(crate) enum Switch {
    On,
    Off,
}

/// One export as it travels to the process that runs the queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Settings {
    pub format: Format,
    pub res: Res,
    pub fps: u32,
    pub quality: QualityArg,
    pub looped: bool,
}

impl Settings {
    pub(crate) fn resolution(&self) -> Resolution {
        match self.res {
            Res::P720 => Resolution::P720,
            Res::P1080 => Resolution::P1080,
            Res::P1440 => Resolution::P1440,
            Res::P2160 => Resolution::P2160,
            Res::Source => Resolution::Source,
        }
    }

    pub(crate) fn mp4(&self) -> Mp4Settings {
        Mp4Settings {
            resolution: self.resolution(),
            fps: self.fps,
            quality: match self.quality {
                QualityArg::Low => Quality::Low,
                QualityArg::Medium => Quality::Medium,
                QualityArg::High => Quality::High,
            },
        }
    }

    pub(crate) fn gif(&self) -> GifSettings {
        GifSettings {
            resolution: self.resolution(),
            fps: self.fps,
            looped: self.looped,
        }
    }

    /// One line: `mp4 1080p 60 fps high`.
    pub(crate) fn describe(&self) -> String {
        let res = match self.res {
            Res::P720 => "720p",
            Res::P1080 => "1080p",
            Res::P1440 => "1440p",
            Res::P2160 => "4k",
            Res::Source => "source",
        };
        match self.format {
            Format::Mp4 => format!("mp4 {res} {} fps {}", self.fps, quality_name(self.quality)),
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

fn quality_name(quality: QualityArg) -> &'static str {
    match quality {
        QualityArg::Low => "low",
        QualityArg::Medium => "medium",
        QualityArg::High => "high",
    }
}

/// The output file (absolute: the queue may run in another process with another folder) and
/// the settings; mistakes of the command line are `Usage`.
pub(crate) fn settings(args: &ExportArgs, cwd: &Path) -> Result<(PathBuf, Settings), CliError> {
    let usage = |text: &str| CliError::Usage(text.to_owned());
    let out = args
        .out
        .as_deref()
        .ok_or_else(|| usage("куда писать? Задай файл: devcast export -o урок.mp4"))?;
    let extension = out
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);
    let format = match (args.format, extension.as_deref()) {
        (Some(format), _) => format,
        (None, Some("mp4")) => Format::Mp4,
        (None, Some("gif")) => Format::Gif,
        _ => {
            return Err(usage(
                "формат не понятен по имени файла: -o урок.mp4, -o урок.gif или --format mp4|gif",
            ));
        }
    };
    let res = args.res.unwrap_or(Res::P1080);
    if format == Format::Gif {
        if matches!(res, Res::P1440 | Res::P2160) {
            return Err(usage("GIF — только 720p, 1080p или source: --res 720p"));
        }
        if args.quality.is_some() {
            return Err(usage("у GIF нет --quality: качество задают размер и fps"));
        }
    }
    if format == Format::Mp4 && args.looped.is_some() {
        return Err(usage("--loop — только для GIF"));
    }
    let fps = args.fps.unwrap_or(match format {
        Format::Mp4 => 60,
        Format::Gif => 15,
    });
    let settings = Settings {
        format,
        res,
        fps,
        quality: args.quality.unwrap_or(QualityArg::High),
        looped: args.looped != Some(Switch::Off),
    };
    Ok((resolved(&cwd.join(out)), settings))
}

/// `path` without `.` and `..`: it is printed back to the user.
fn resolved(path: &Path) -> PathBuf {
    let mut clean = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other),
        }
    }
    clean
}
