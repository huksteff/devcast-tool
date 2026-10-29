//! `devcast captions gen | export | import`: the caption commands that need Whisper or files. The
//! rest (`captions set`, `split`, `merge`, `rm`, `clear`) are ops of the registry.

use std::path::Path;

use dc_core::time::Source;
use dc_core::{CommandError, Range, SourceTime};
use dc_fx_captions::{
    CaptionsGen, CaptionsImport, NewCue, Word, export_text, merge_tracks, parse_text,
};
use dc_transcribe::Cue;
use serde_json::json;

use crate::edit::{self, Ctx, answer};
use crate::error::CliError;
use crate::run::Answer;
use crate::transcript::{MediaWords, Quality, media_cues, project_words};

#[derive(Debug, Clone, clap::Args)]
pub(crate) struct GenArgs {
    #[arg(long, value_enum, default_value_t = Quality::Fast, help = "fast — small на процессоре; accurate — large-v3-turbo, нужна видеокарта")]
    model: Quality,
    #[arg(long, help = "Язык речи: ru, en …; без флага определяется сам")]
    language: Option<String>,
    #[arg(
        long,
        value_delimiter = ',',
        help = "Дорожки через запятую, где речь перекрывается — главнее первая: mic (по умолчанию; у импортированного видео — первая), system"
    )]
    tracks: Vec<String>,
}

fn failed(error: CommandError) -> CliError {
    CliError::Failed(error.to_string())
}

/// `captions gen`: recognizes the chosen tracks of every recording (cached like `transcript`)
/// and replaces all cues, one undo step.
pub(crate) fn generate(ctx: &Ctx<'_>, args: &GenArgs) -> Result<Answer, CliError> {
    let names: Vec<Option<&String>> = if args.tracks.is_empty() {
        vec![None]
    } else {
        args.tracks.iter().map(Some).collect()
    };
    let mut tracks = Vec::with_capacity(names.len());
    {
        let workspace = ctx.open(false)?;
        for name in names {
            let all = project_words(
                workspace.dir(),
                &workspace.project,
                args.model.model(),
                args.language.as_deref(),
                name,
                true,
            )?;
            let mut cues = Vec::new();
            for media in &all {
                for cue in media_cues(media, &media.words)? {
                    cues.push(new_cue(media, &cue).map_err(failed)?);
                }
            }
            tracks.push(cues);
        }
    }
    let cues = merge_tracks(tracks).map_err(failed)?;
    if cues.is_empty() {
        return Err(CliError::Failed(
            "речи не найдено: субтитры не изменены; проверь дорожку (--tracks) и язык (--language)"
                .to_owned(),
        ));
    }
    let mut workspace = ctx.open(true)?;
    edit::apply_commands(
        ctx,
        &mut workspace,
        CaptionsGen::NAME,
        &[&CaptionsGen { cues }],
    )
}

fn new_cue(media: &MediaWords, cue: &Cue) -> Result<NewCue, CommandError> {
    let range = |start: i64, end: i64| -> Result<Range<Source>, CommandError> {
        Range::new(SourceTime::from_micros(start), SourceTime::from_micros(end))
            .map_err(|e| CommandError::new(e.to_string()))
    };
    let words = cue
        .words
        .iter()
        .filter_map(|&i| media.words.get(i))
        .map(|w| {
            Ok(Word {
                text: w.text.clone(),
                range: range(w.start, w.end)?,
            })
        })
        .collect::<Result<Vec<Word>, CommandError>>()?;
    Ok(NewCue {
        media: media.media.clone(),
        range: range(cue.start, cue.end)?,
        words,
    })
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
