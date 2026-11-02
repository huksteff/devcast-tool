//! `devcast captions gen | export | import`: the caption commands that need Whisper or files. The
//! rest (`captions set`, `split`, `merge`, `rm`, `clear`) are ops of the registry.

use std::path::Path;
use std::rc::Rc;

use dc_core::CommandError;
use dc_fx_captions::{CaptionsGen, CaptionsImport, export_text, parse_text};
use serde_json::json;

use crate::edit::{self, Ctx, answer};
use crate::error::CliError;
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
        crate::speech::generate(workspace.dir(), &workspace.project, &speech, &silent)?
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
