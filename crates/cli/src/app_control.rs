//! `devcast app show|quit|status`: the running app as a whole (owner's rule 2026-11-02). `show`
//! is a second start of the app (the HUD comes back); `quit` is the tray's «Выход» without its
//! question — a recording is stopped and saved, the editor writes its working state; `status`
//! is one place to poll: the recording, the editor, the exports of both queues.

use dc_ops::live::{
    Answer as Live, AppStatus, CaptionsState, EditorState, RecordingState, Request,
};
use serde_json::{Value, json};

use crate::edit::Ctx;
use crate::error::CliError;
use crate::export;
use crate::live::{NOT_RUNNING, app_channel, ask_app, refused};
use crate::run::Answer;

/// `devcast app show`.
pub(crate) fn show(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    if ctx.globals.dry_run {
        let text = "dry-run: HUD программы не тронут".to_owned();
        return Ok(Answer::new(text, json!({"dry_run": true})));
    }
    match ask_app(&app_channel(ctx.local.app_channel)?, &Request::Show)? {
        Some(Live::Shown) => Ok(Answer::new(
            "HUD программы показан".to_owned(),
            json!({"shown": true}),
        )),
        Some(answer) => Err(refused(answer)),
        None => Err(CliError::Failed(NOT_RUNNING.to_owned())),
    }
}

/// `devcast app quit`.
pub(crate) fn quit(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let channel = app_channel(ctx.local.app_channel)?;
    if ctx.globals.dry_run {
        let status = match ask_app(&channel, &Request::Status)? {
            Some(Live::Status(status)) => status,
            Some(answer) => return Err(refused(answer)),
            None => return Ok(not_running()),
        };
        let recording = !matches!(status.recording, RecordingState::Idle);
        let text = match recording {
            true => "dry-run: программа не закрыта; quit остановит и сохранит запись",
            false => "dry-run: программа не закрыта; записи нет",
        };
        let json = json!({"dry_run": true, "recording": recording});
        return Ok(Answer::new(text.to_owned(), json));
    }
    match ask_app(&channel, &Request::Quit)? {
        Some(Live::Quit { recorded }) => {
            let text = match &recorded {
                Some(root) => format!("DevCast закрыт; запись сохранена: {}", root.display()),
                None => "DevCast закрыт".to_owned(),
            };
            Ok(Answer::new(
                text,
                json!({"quit": true, "recorded": recorded}),
            ))
        }
        Some(answer) => Err(refused(answer)),
        None => Ok(not_running()),
    }
}

fn not_running() -> Answer {
    Answer::new("DevCast не запущен".to_owned(), json!({"running": false}))
}

/// `devcast app status`.
pub(crate) fn status(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let status = match ask_app(&app_channel(ctx.local.app_channel)?, &Request::Status)? {
        Some(Live::Status(status)) => Some(status),
        Some(answer) => return Err(refused(answer)),
        None => None,
    };
    let exports = export::all_jobs(ctx)?;
    let mut lines = vec![match &status {
        Some(status) => format!("запись: {}", recording(&status.recording)),
        None => "DevCast не запущен".to_owned(),
    }];
    if let Some(AppStatus {
        editor: Some(editor),
        ..
    }) = status.as_deref()
    {
        lines.push(format!("редактор: {}", opened(editor)));
    }
    lines.extend(
        exports
            .iter()
            .map(|job| format!("экспорт: {}", export::line(job))),
    );
    let jobs: Vec<Value> = exports
        .iter()
        .map(|(q, job)| export::listed(*q, job))
        .collect();
    let json = match status {
        Some(status) => json!({
            "running": true,
            "recording": status.recording,
            "editor": status.editor,
            "exports": jobs,
        }),
        None => json!({"running": false, "exports": jobs}),
    };
    Ok(Answer::new(lines.join("\n"), json))
}

fn recording(state: &RecordingState) -> String {
    match state {
        RecordingState::Idle => "нет".to_owned(),
        RecordingState::Countdown { left } => format!("отсчёт, осталось {left} с"),
        RecordingState::Starting { .. } => "начинается".to_owned(),
        RecordingState::Recording {
            root,
            recorded,
            paused,
            asking,
        } => {
            let time = dc_core::SourceTime::from_micros((recorded * 1_000_000.0) as i64);
            let state = match (paused, asking) {
                (_, true) => ", спрашивает «Отменить запись?»",
                (true, false) => ", пауза",
                (false, false) => "",
            };
            format!("идёт {time}{state} — {}", root.display())
        }
        RecordingState::Preparing { root } => format!("сохраняется — {}", root.display()),
        RecordingState::Cancelling { root } => format!("отменяется — {}", root.display()),
    }
}

fn opened(editor: &EditorState) -> String {
    let unsaved = match editor.unsaved {
        true => ", есть несохранённое",
        false => "",
    };
    let share = |done: Option<f32>| {
        done.map(|d| format!(" {:.0}%", d * 100.0))
            .unwrap_or_default()
    };
    let captions = match editor.captions {
        CaptionsState::Idle => String::new(),
        CaptionsState::Downloading { done } => {
            format!(", субтитры: качается модель{}", share(done))
        }
        CaptionsState::Recognizing { done } => format!(", субтитры: распознаётся{}", share(done)),
    };
    format!("{}{unsaved}{captions}", editor.root.display())
}
