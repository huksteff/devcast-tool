//! The recording of the running app (owner's rule 2026-11-02): `devcast record --app` is
//! «Запись» of the HUD — the app's settings, its countdown, the editor after the stop — and
//! answers once the first frame is recorded; `record stop|pause|resume|cancel` reach it when no
//! `devcast record` process runs. A cancel from here is the answer to «Отменить запись?»
//! itself: the app does not ask it.

use std::path::Path;
use std::time::Duration;

use dc_ops::live::{Answer as Live, RecordAnswer, RecordControl, RecordingState, Request};
use serde_json::json;

use crate::app_settings::switch;
use crate::edit::Ctx;
use crate::error::CliError;
use crate::live::{app_channel, ask_app, refused};
use crate::record::at;
use crate::record_args::RecordArgs;
use crate::record_control::Control;
use crate::run::Answer;

/// `devcast record --app`.
pub(crate) fn start(ctx: &Ctx<'_>, args: &RecordArgs) -> Result<Answer, CliError> {
    if !args.is_empty() {
        return Err(CliError::Usage(
            "record --app пишет с настройками программы, которые показывает её HUD, без --source, --fps, --audio, --mic, --duration и -o: поменяй их — devcast config set recording.source=screen:2 (надолго), или пиши без программы — devcast record --source screen:2"
                .to_owned(),
        ));
    }
    if ctx.globals.dry_run {
        let settings = ctx.app_settings()?;
        let recording = &settings.recording;
        let text = format!(
            "dry-run, запись не начата: программа запишет {} {} fps, системный звук {}, микрофон {}, отсчёт {} с → {}",
            recording.source,
            recording.fps.get(),
            switch(recording.system_audio),
            recording.microphone.as_deref().unwrap_or("нет"),
            settings.countdown.seconds(),
            settings.recordings_dir.display()
        );
        let json = json!({"dry_run": true, "config": recording, "countdown": settings.countdown.seconds(), "folder": settings.recordings_dir});
        return Ok(Answer::new(text, json));
    }
    match ask(ctx, &Request::Record)? {
        RecordAnswer::Started { root } => Ok(Answer::new(
            format!(
                "запись идёт в программе: {}; остановить — devcast record stop",
                root.display()
            ),
            json!({"state": "recording", "project": root, "app": true}),
        )),
        RecordAnswer::CountdownCancelled => Err(CliError::Failed(
            "отсчёт отменили, запись не начата: запусти devcast record --app ещё раз".to_owned(),
        )),
        RecordAnswer::Cancelled { root } => Err(CliError::Failed(format!(
            "запись отменили сразу после старта, папка удалена: {}",
            root.display()
        ))),
        answer => Err(unexpected(answer, None)),
    }
}

/// `devcast record stop|pause|resume|cancel` to the app's recording.
pub(crate) fn control(ctx: &Ctx<'_>, control: Control) -> Result<Answer, CliError> {
    let asked = match control {
        Control::Stop => RecordControl::Stop,
        Control::Pause => RecordControl::Pause,
        Control::Resume => RecordControl::Resume,
        Control::Cancel => RecordControl::Cancel,
    };
    match ask(ctx, &Request::RecordControl(asked))? {
        RecordAnswer::Paused { recorded } => Ok(at("пауза", seconds(recorded))),
        RecordAnswer::Resumed { recorded } => Ok(at("продолжено", seconds(recorded))),
        RecordAnswer::Stopped { root } => Ok(Answer::new(
            format!(
                "записано: {}; программа открыла проект в редакторе",
                root.display()
            ),
            json!({"project": root, "app": true}),
        )),
        RecordAnswer::Cancelled { root } => Ok(cancelled(&root)),
        RecordAnswer::CountdownCancelled => Ok(Answer::new(
            "отсчёт отменён, запись не начата".to_owned(),
            json!({"countdown_cancelled": true, "app": true}),
        )),
        answer => Err(unexpected(answer, Some(control))),
    }
}

fn cancelled(root: &Path) -> Answer {
    Answer::new(
        format!("запись отменена, папка удалена: {}", root.display()),
        json!({"cancelled": true, "project": root, "app": true}),
    )
}

fn seconds(recorded: f64) -> Duration {
    Duration::try_from_secs_f64(recorded).unwrap_or_default()
}

/// The app's answer about its recording; not running — the recording is not going on.
fn ask(ctx: &Ctx<'_>, request: &Request) -> Result<RecordAnswer, CliError> {
    match ask_app(&app_channel(ctx.local.app_channel)?, request)? {
        Some(Live::Record(answer)) => Ok(answer),
        Some(answer) => Err(refused(answer)),
        None => Err(CliError::Failed(match request {
            Request::Record => {
                "DevCast не запущен: запусти программу или пиши без неё — devcast record --source screen:1"
            }
            _ => {
                "запись не идёт: начни её — devcast record --app (в программе) или devcast record --source screen:1"
            }
        }
        .to_owned())),
    }
}

/// A refusal or a failure: why, and what to do.
fn unexpected(answer: RecordAnswer, control: Option<Control>) -> CliError {
    CliError::Failed(match answer {
        RecordAnswer::Failed(message) => format!("запись в программе не удалась: {message}"),
        RecordAnswer::NotNow(state) => busy(&state, control),
        other => format!(
            "программа ответила не по делу ({other:?}): обнови DevCast, чтобы программа и devcast были одной версии"
        ),
    })
}

/// Why `control` (`None`: the start) does not fit the recording's `state`.
fn busy(state: &RecordingState, control: Option<Control>) -> String {
    match (state, control) {
        (RecordingState::Idle, _) => {
            "запись не идёт: начни её — devcast record --app".to_owned()
        }
        (RecordingState::Countdown { left }, _) => format!(
            "идёт отсчёт перед записью (осталось {left} с): пауза — после старта, отменить — devcast record cancel"
        ),
        (RecordingState::Starting { .. }, _) => {
            "запись только начинается: повтори команду, когда devcast app status покажет recording"
                .to_owned()
        }
        (RecordingState::Recording { asking: true, .. }, _) => {
            "программа спрашивает «Отменить запись?»: ответь devcast record cancel или devcast record stop".to_owned()
        }
        (RecordingState::Recording { .. }, None) => {
            "программа уже записывает: останови — devcast record stop".to_owned()
        }
        (RecordingState::Recording { paused: true, .. }, Some(Control::Pause)) => {
            "запись уже на паузе: продолжить — devcast record resume".to_owned()
        }
        (RecordingState::Recording { .. }, _) => {
            "запись не на паузе: пауза — devcast record pause".to_owned()
        }
        (RecordingState::Preparing { root }, _) => format!(
            "запись уже остановлена и сохраняется: {}",
            root.display()
        ),
        (RecordingState::Cancelling { root }, _) => {
            format!("запись уже отменяется: {}", root.display())
        }
    }
}
