//! Settings of `devcast record` as typed and what they mean: the recording config, the time
//! limit and the new project folder.

use std::path::{Path, PathBuf};
use std::time::Duration;

use dc_capture::{Fps, RecordingConfig, Source};

use crate::error::CliError;

/// Settings of `devcast record` as typed.
#[derive(Debug, Default, clap::Args)]
pub(crate) struct RecordArgs {
    #[arg(
        long,
        help = "Что писать: screen:<номер>, window:<id> или area:x,y,w,h (список — devcast ls sources)"
    )]
    pub source: Option<String>,
    #[arg(long, help = "Кадров в секунду: 30, 60 или 120; по умолчанию 60")]
    pub fps: Option<u32>,
    #[arg(
        long,
        help = "Остановиться через столько секунд записи (паузы не считаются); без него — до devcast record stop"
    )]
    pub duration: Option<f64>,
    #[arg(long, help = "Системный звук: on или off; по умолчанию on")]
    pub audio: Option<Switch>,
    #[arg(
        long,
        help = "Микрофон по имени из devcast ls sources; без него — без микрофона"
    )]
    pub mic: Option<String>,
    #[arg(
        short = 'o',
        long,
        help = "Папка нового проекта *.dcproj; по умолчанию recording-N.dcproj в текущей папке"
    )]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Switch {
    On,
    Off,
}

impl RecordArgs {
    pub(crate) fn is_empty(&self) -> bool {
        let RecordArgs {
            source,
            fps,
            duration,
            audio,
            mic,
            out,
        } = self;
        source.is_none()
            && fps.is_none()
            && duration.is_none()
            && audio.is_none()
            && mic.is_none()
            && out.is_none()
    }
}

/// What to record, from the arguments.
pub(crate) fn config(args: &RecordArgs) -> Result<RecordingConfig, CliError> {
    let source: Source = args
        .source
        .as_deref()
        .ok_or_else(|| {
            CliError::Usage(
                "укажи, что писать: --source screen:1 (window:<id>, area:x,y,w,h; список — devcast ls sources)"
                    .to_owned(),
            )
        })?
        .parse()
        .map_err(|e: dc_capture::SourceError| CliError::Usage(e.to_string()))?;
    let fps = Fps::try_from(args.fps.unwrap_or(60))
        .map_err(|e| CliError::Usage(format!("--fps: {e}")))?;
    let config = RecordingConfig {
        fps,
        system_audio: args.audio != Some(Switch::Off),
        microphone: args.mic.clone(),
        ..RecordingConfig::new(source)
    };
    config
        .validate()
        .map_err(|e| CliError::Usage(format!("--mic: {e}")))?;
    Ok(config)
}

pub(crate) fn duration(args: &RecordArgs) -> Result<Option<Duration>, CliError> {
    args.duration
        .map(|seconds| {
            Duration::try_from_secs_f64(seconds)
                .ok()
                .filter(|d| !d.is_zero())
                .ok_or_else(|| {
                    CliError::Usage(format!(
                        "--duration {seconds}: нужно число секунд больше 0, например --duration 10"
                    ))
                })
        })
        .transpose()
}

/// The new project folder: `-o`, or the first free `recording-N.dcproj` in `cwd`.
pub(crate) fn project_folder(cwd: &Path, out: Option<&Path>) -> Result<PathBuf, CliError> {
    let Some(out) = out else {
        let mut n = 1;
        loop {
            let root = cwd.join(format!("recording-{n}.dcproj"));
            if !root.exists() {
                return Ok(root);
            }
            n += 1;
        }
    };
    if out.extension().is_none_or(|e| e != "dcproj") {
        return Err(CliError::Usage(format!(
            "-o {}: папка проекта должна называться *.dcproj, например -o урок.dcproj",
            out.display()
        )));
    }
    let root = cwd.join(out);
    if root.exists() {
        return Err(CliError::Failed(format!(
            "{} уже есть: запись создаёт новый проект, выбери другое имя",
            root.display()
        )));
    }
    Ok(root)
}
