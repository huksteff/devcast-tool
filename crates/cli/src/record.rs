//! `devcast record`: records the screen into a new project until `devcast record stop` from
//! another call (or `--duration` seconds of recording); `record pause|resume|cancel` control
//! it on the way ([`crate::record_control`]).

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use dc_capture::VIDEO_TRACK;
use dc_capture::win::{ProjectRecording, RecordedProject, on_console_stop};
use dc_core::SourceTime;
use dc_project::Defaults;
use serde_json::json;

use crate::autozoom;
use crate::edit::Ctx;
use crate::error::CliError;
use crate::record_args::{RecordArgs, config, duration, project_folder};
use crate::record_control::{self, Control, Reply, Request, Server};
use crate::run::Answer;
use crate::workspace::Workspace;

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
        Some(control) if args.is_empty() => {
            record_control::send(&record_control::channel()?, control, ctx.globals.dry_run)
        }
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
    let defaults = Defaults::load(&ctx.defaults_file()?)?;
    let server = Server::start(&record_control::channel()?)?;
    if let Err(e) = on_console_stop(server.stopper()) {
        server.close()?;
        return Err(CliError::Failed(e.to_string()));
    }
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

/// Stops the recording and, if the zoom setting `auto` of the project is on, adds auto zooms.
fn stop(recording: ProjectRecording) -> Result<Answer, CliError> {
    let recorded = recording
        .stop()
        .map_err(|e| CliError::Failed(e.to_string()))?;
    let mut answer = summary(&recorded)?;
    let zooms = Workspace::open(recorded.dir.clone(), true, false)
        .and_then(|mut workspace| autozoom::after_recording(&mut workspace));
    match zooms {
        Ok(Some(applied)) => {
            answer.text.push_str(&format!("; автозумы: {applied}"));
            answer.json["zooms"] = applied.diff.to_string().into();
        }
        Ok(None) => {}
        Err(e) => answer.warnings.push(format!("автозумы не поставлены: {e}")),
    }
    Ok(answer)
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

/// `записано: recording-1.devcast — 0:12.345, 1920x1080, 60 fps, звук: system, mic`.
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
    let mut answer = Answer::new(text, json);
    answer.warnings.extend(recorded.summary.problems());
    Ok(answer)
}
