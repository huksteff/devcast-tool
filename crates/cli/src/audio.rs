//! `devcast audio add <file> --at t [--track n] [--duration s]`: the sound file is read whole and
//! copied into the project's `assets/audio/`, then `dc_fx_audio_tracks::AudioAdd` places it on
//! the output timeline as one undo step (`dc_ops::import_sound`, shared with the editor). A
//! failed change deletes the file it copied.

use std::path::PathBuf;

use dc_core::TimeDelta;
use dc_fx_audio_tracks::AudioAdd;
use dc_ops::SoundPlace;

use crate::edit::{self, Ctx};
use crate::error::CliError;
use crate::run::Answer;

#[derive(Debug, clap::Args)]
pub(crate) struct AddArgs {
    #[arg(help = "Звук mp3, wav, aac, m4a, flac или ogg; копируется в assets/audio/ проекта")]
    file: PathBuf,
    #[arg(
        long,
        help = "Начало по готовому видео; вырезы и скорость его не сдвигают"
    )]
    at: String,
    #[arg(
        long,
        help = "Дорожка звука 0–31; без неё — первая, где клип помещается целиком"
    )]
    track: Option<u32>,
    #[arg(
        long,
        help = "Длина, секунды; без неё — файл до конца, но не дальше конца видео"
    )]
    duration: Option<f64>,
}

pub(crate) fn add(ctx: &Ctx<'_>, args: &AddArgs) -> Result<Answer, CliError> {
    let at = crate::ls::time(Some(&args.at), "--at")?.unwrap_or_default();
    let source = ctx.cwd.join(&args.file);
    let mut workspace = ctx.open(true)?;
    let place = SoundPlace {
        at,
        track: args.track,
        duration: args.duration.map(TimeDelta::from_secs_f64),
    };
    let imported = dc_ops::import_sound(workspace.dir(), &source, place, ctx.globals.dry_run)
        .map_err(|e| CliError::Failed(e.to_string()))?;
    let answer = edit::apply_commands(ctx, &mut workspace, AudioAdd::NAME, &[&imported.command]);
    if let Err(error) = &answer {
        imported
            .discard()
            .map_err(|e| CliError::Failed(format!("{error}; {e}")))?;
    }
    answer
}
