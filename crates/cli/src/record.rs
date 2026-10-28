//! `devcast record`: records the screen into a new project until `devcast record stop` from
//! another call (or `--duration` seconds of recording); `record pause|resume|cancel` control
//! it on the way ([`crate::record_control`]).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use dc_capture::win::{ProjectRecording, RecordedProject, VIDEO_TRACK};
use dc_capture::{Fps, RecordingConfig, Source};
use dc_core::SourceTime;
use dc_project::{Defaults, settings};
use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::record_control::{self, Control, Reply, Request, Server};
use crate::run::Answer;

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
    fn is_empty(&self) -> bool {
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

/// `devcast record [stop|pause|resume|cancel] …`.
pub(crate) fn record(
    ctx: &Ctx<'_>,
    control: Option<Control>,
    args: &RecordArgs,
) -> Result<Answer, CliError> {
    if ctx.globals.project.is_some() {
        return Err(CliError::Usage(
            "record пишет в новый проект: папку задаёт -o, а не --project".to_owned(),
        ));
    }
    match control {
        Some(control) if args.is_empty() => record_control::send(control, ctx.globals.dry_run),
        Some(control) => Err(CliError::Usage(format!(
            "record {} управляет идущей записью и не берёт настроек: devcast record {}",
            name(control),
            name(control)
        ))),
        None => start(ctx, args),
    }
}

fn name(control: Control) -> &'static str {
    match control {
        Control::Stop => "stop",
        Control::Pause => "pause",
        Control::Resume => "resume",
        Control::Cancel => "cancel",
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

fn duration(args: &RecordArgs) -> Result<Option<Duration>, CliError> {
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

fn start(ctx: &Ctx<'_>, args: &RecordArgs) -> Result<Answer, CliError> {
    let config = config(args)?;
    let duration = duration(args)?;
    let root = project_folder(ctx.cwd, args.out.as_deref())?;
    if ctx.globals.dry_run {
        let text = format!(
            "dry-run, запись не начата: {} {} fps{}{} → {}",
            config.source,
            config.fps.get(),
            if config.system_audio {
                ", системный звук"
            } else {
                ""
            },
            config
                .microphone
                .as_ref()
                .map(|m| format!(", микрофон «{m}»"))
                .unwrap_or_default(),
            root.display()
        );
        let json = json!({"dry_run": true, "config": config, "project": root});
        return Ok(Answer::new(text, json));
    }
    let defaults = Defaults::load(&settings::defaults_file()?)?;
    let server = Server::start()?;
    let started = ProjectRecording::start(&config, &root, "devcast record", defaults);
    let result = match started {
        Ok(recording) => run(recording, &server.requests, duration),
        Err(e) => Err(CliError::Failed(e.to_string())),
    };
    let warnings = server.close()?;
    let mut answer = result?;
    answer.warnings.extend(warnings);
    Ok(answer)
}

/// Serves requests until the stop, the cancel or the end of `duration` of recording.
fn run(
    recording: ProjectRecording,
    requests: &Receiver<Request>,
    duration: Option<Duration>,
) -> Result<Answer, CliError> {
    let mut clock = Recorded::default();
    loop {
        let left = duration.map(|d| d.saturating_sub(clock.total()));
        let request = match (left, clock.running) {
            (Some(left), Some(_)) => match requests.recv_timeout(left) {
                Ok(request) => request,
                Err(RecvTimeoutError::Timeout) => return stop(recording),
                Err(RecvTimeoutError::Disconnected) => return closed(recording),
            },
            _ => match requests.recv() {
                Ok(request) => request,
                Err(_) => return closed(recording),
            },
        };
        let result = match request.control {
            Control::Stop => {
                let answer = stop(recording);
                reply(&request, &answer);
                return answer;
            }
            Control::Cancel => {
                let answer = cancel(recording);
                reply(&request, &answer);
                return answer;
            }
            Control::Pause => recording
                .pause()
                .map(|_| clock.pause())
                .map(|recorded| at("пауза", recorded)),
            Control::Resume => recording
                .resume()
                .map(|_| clock.resume())
                .map(|recorded| at("продолжено", recorded)),
        };
        reply(
            &request,
            &result.map_err(|e| CliError::Failed(e.to_string())),
        );
    }
}

fn reply(request: &Request, answer: &Result<Answer, CliError>) {
    // The caller may have given up waiting; the recording goes on regardless.
    let _ = request.reply.send(Reply::of(answer));
}

/// How long was recorded, without pauses.
struct Recorded {
    before: Duration,
    running: Option<Instant>,
}

impl Recorded {
    fn total(&self) -> Duration {
        self.before + self.running.map_or(Duration::ZERO, |since| since.elapsed())
    }

    fn pause(&mut self) -> Duration {
        self.before = self.total();
        self.running = None;
        self.before
    }

    fn resume(&mut self) -> Duration {
        self.running = Some(Instant::now());
        self.before
    }
}

impl Default for Recorded {
    fn default() -> Self {
        Self {
            before: Duration::ZERO,
            running: Some(Instant::now()),
        }
    }
}

fn at(what: &str, recorded: Duration) -> Answer {
    let time = SourceTime::from_micros(recorded.as_micros() as i64);
    Answer::new(
        format!("{what}: записано {time}"),
        json!({"state": what, "recorded": recorded.as_secs_f64()}),
    )
}

fn stop(recording: ProjectRecording) -> Result<Answer, CliError> {
    let recorded = recording
        .stop()
        .map_err(|e| CliError::Failed(e.to_string()))?;
    summary(&recorded)
}

/// Nobody can reach the recording any more: it stops now.
fn closed(recording: ProjectRecording) -> Result<Answer, CliError> {
    let mut answer = stop(recording)?;
    answer
        .warnings
        .push("канал управления закрылся, запись остановлена".to_owned());
    Ok(answer)
}

fn cancel(recording: ProjectRecording) -> Result<Answer, CliError> {
    let root = recording.dir().root().to_path_buf();
    recording
        .cancel()
        .map_err(|e| CliError::Failed(e.to_string()))?;
    Ok(Answer::new(
        format!("запись отменена, папка удалена: {}", root.display()),
        json!({"cancelled": true, "project": root}),
    ))
}

/// `записано: recording-1.dcproj — 0:12.345, 1920x1080, 60 fps, звук: system, mic`.
pub(crate) fn summary(recorded: &RecordedProject) -> Result<Answer, CliError> {
    let manifest = &recorded.summary.manifest;
    let root = recorded.dir.root();
    let broken = || {
        CliError::Failed(format!(
            "в проекте {} нет клипа или видео записи: открой папку media/ и проверь файлы",
            root.display()
        ))
    };
    let media = &recorded.project.clips.first().ok_or_else(broken)?.media;
    let video = manifest.tracks.iter().find(|t| t.name == VIDEO_TRACK);
    let fps = video.ok_or_else(broken)?.rate;
    let (duration, width, height) = (media.duration, media.size.width, media.size.height);
    let sound: Vec<&str> = manifest
        .tracks
        .iter()
        .filter(|t| t.name != VIDEO_TRACK)
        .map(|t| t.name.as_str())
        .collect();
    let mut text = format!(
        "записано: {} — {}, {width}x{height}, {fps} fps, звук: {}",
        root.display(),
        SourceTime::ZERO + duration,
        if sound.is_empty() {
            "нет".to_owned()
        } else {
            sound.join(", ")
        }
    );
    if !manifest.pauses.is_empty() {
        text.push_str(&format!(", пауз: {}", manifest.pauses.len()));
    }
    let json = json!({
        "project": root,
        "duration": duration.as_secs_f64(),
        "size": [width, height],
        "fps": fps,
        "sound": sound,
        "pauses": manifest.pauses.len(),
        "frames": recorded.summary.video.frames,
    });
    Ok(Answer::new(text, json))
}
