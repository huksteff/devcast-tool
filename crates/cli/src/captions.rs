//! `devcast captions gen | export | import | stop`: the caption commands that need Whisper, files or
//! the editor. The rest (`captions set`, `split`, `merge`, `rm`, `clear`) are ops of the registry.

use std::path::Path;
use std::rc::Rc;

use dc_core::CommandError;
use dc_fx_captions::{CaptionsGen, CaptionsImport, export_text, parse_text};
use dc_ops::live::{Answer as Live, Request};
use serde_json::json;

use crate::edit::{self, Ctx, answer};
use crate::error::CliError;
use crate::find::find_project;
use crate::live::{ask_app, held_by_app, refused};
use crate::open::absolute;
use crate::run::Answer;
use crate::speech::OnProgress;

#[derive(Debug, Clone, clap::Args)]
pub(crate) struct GenArgs {
    #[arg(long, value_parser = crate::config::parse_model, help = "fast — small на процессоре; accurate — large-v3-turbo, нужна видеокарта; без флага — captions.model из devcast config")]
    model: Option<dc_settings::SpeechModel>,
    #[arg(
        long,
        help = "Язык речи: ru, en …, auto — определить; без флага — captions.language из devcast config"
    )]
    language: Option<String>,
    #[arg(
        long,
        value_delimiter = ',',
        help = "Дорожки через запятую, где речь перекрывается — главнее первая: mic (у импортированного видео — первая), system; без флага — captions.tracks из devcast config"
    )]
    tracks: Vec<String>,
}

fn failed(error: CommandError) -> CliError {
    CliError::Failed(error.to_string())
}

/// `captions gen`: recognizes the chosen tracks of every recording (cached like `transcript`)
/// and replaces all cues, one undo step; the same path as the editor's «Создать субтитры».
pub(crate) fn generate(ctx: &Ctx<'_>, args: &GenArgs) -> Result<Answer, CliError> {
    let speech = crate::speech::chosen(ctx, args.model, args.language.as_deref(), &args.tracks)?;
    let silent: OnProgress = Rc::new(|_| {});
    let command = {
        let workspace = ctx.open(false)?;
        crate::speech::generate(workspace.dir(), &workspace.project, &speech, &silent)?.command
    };
    let mut workspace = ctx.open(true)?;
    edit::apply_commands(ctx, &mut workspace, CaptionsGen::NAME, &[&command])
}

/// `captions export -o t.txt`: the text of all cues, one `<id> <text>` line each.
pub(crate) fn export(ctx: &Ctx<'_>, out: &Path) -> Result<Answer, CliError> {
    let mut workspace = ctx.open(false)?;
    let (text, count) = export_text(&workspace.project).map_err(failed)?;
    if count == 0 {
        return Err(CliError::Failed(
            "реплик нет: сначала devcast captions gen".to_owned(),
        ));
    }
    let path = ctx.cwd.join(out);
    if !ctx.globals.dry_run {
        std::fs::write(&path, text).map_err(|e| {
            CliError::Failed(format!(
                "не удалось записать «{}» ({e}): проверь путь и права",
                path.display()
            ))
        })?;
    }
    let line = format!("{}реплик {count} → {}", ctx.prefix(), out.display());
    let json = json!({"count": count, "path": path, "dry_run": ctx.globals.dry_run});
    Ok(answer(&mut workspace, line, json))
}

/// `captions import t.txt`: new texts of the listed cues, one undo step; times stay.
pub(crate) fn import(ctx: &Ctx<'_>, file: &Path) -> Result<Answer, CliError> {
    let path = ctx.cwd.join(file);
    let text = std::fs::read_to_string(&path).map_err(|e| {
        CliError::Failed(format!(
            "не удалось прочитать «{}» ({e}): проверь путь; файл — из devcast captions export",
            path.display()
        ))
    })?;
    let texts = parse_text(&text).map_err(failed)?;
    let mut workspace = ctx.open(true)?;
    edit::apply_commands(
        ctx,
        &mut workspace,
        CaptionsImport::NAME,
        &[&CaptionsImport { texts }],
    )
}

/// `captions stop`: «Отменить» of the captions panel in the editor holding the project; the
/// cues stay as they were. A `captions gen` of this console stops with Ctrl+C.
pub(crate) fn stop(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let dir = find_project(ctx.globals.project.as_deref(), ctx.cwd)?;
    let not_open = || {
        CliError::Failed(format!(
            "«{}» не открыт в редакторе DevCast: останавливать нечего (devcast captions gen в консоли останавливается Ctrl+C)",
            dir.name()
        ))
    };
    let channel = held_by_app(&dir, ctx.local.app_channel)?.ok_or_else(not_open)?;
    if ctx.globals.dry_run {
        let text = format!("{}распознавание в редакторе не тронуто", ctx.prefix());
        return Ok(Answer::new(text, json!({"dry_run": true})));
    }
    let request = Request::StopCaptions {
        root: absolute(dir.root()),
    };
    let stopped = match ask_app(&channel, &request)? {
        Some(Live::CaptionsStopped { stopped }) => stopped,
        Some(Live::NotOpen) | None => return Err(not_open()),
        Some(answer) => return Err(refused(answer)),
    };
    let text = match stopped {
        true => "распознавание субтитров в редакторе остановлено",
        false => "распознавание субтитров в редакторе не шло",
    };
    Ok(Answer::new(text.to_owned(), json!({"stopped": stopped})))
}
