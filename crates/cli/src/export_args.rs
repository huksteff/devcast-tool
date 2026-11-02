//! Arguments of `devcast export` and what they turn into: the settings of
//! `dc_ops::export` (shared with the editor) and the output file.

use std::path::{Component, Path, PathBuf};

use dc_ops::export::{self, Settings};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Format {
    Mp4,
    Gif,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum QualityArg {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Switch {
    On,
    Off,
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
        (Some(Format::Mp4), _) | (None, Some("mp4")) => export::Format::Mp4,
        (Some(Format::Gif), _) | (None, Some("gif")) => export::Format::Gif,
        _ => {
            return Err(usage(
                "формат не понятен по имени файла: -o урок.mp4, -o урок.gif или --format mp4|gif",
            ));
        }
    };
    let mut settings = Settings::default_for(format);
    if let Some(res) = args.res {
        settings.res = match res {
            Res::P720 => export::Res::P720,
            Res::P1080 => export::Res::P1080,
            Res::P1440 => export::Res::P1440,
            Res::P2160 => export::Res::P2160,
            Res::Source => export::Res::Source,
        };
    }
    if format == export::Format::Gif {
        if matches!(settings.res, export::Res::P1440 | export::Res::P2160) {
            return Err(usage("GIF — только 720p, 1080p или source: --res 720p"));
        }
        if args.quality.is_some() {
            return Err(usage("у GIF нет --quality: качество задают размер и fps"));
        }
    }
    if format == export::Format::Mp4 && args.looped.is_some() {
        return Err(usage("--loop — только для GIF"));
    }
    if let Some(fps) = args.fps {
        settings.fps = fps;
    }
    if let Some(quality) = args.quality {
        settings.quality = match quality {
            QualityArg::Low => export::Quality::Low,
            QualityArg::Medium => export::Quality::Medium,
            QualityArg::High => export::Quality::High,
        };
    }
    if let Some(looped) = args.looped {
        settings.looped = looped == Switch::On;
    }
    Ok((resolved(&cwd.join(out)), settings))
}

/// `path` without `.` and `..`: it is printed back to the user.
pub(crate) fn resolved(path: &Path) -> PathBuf {
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
