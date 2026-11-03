//! `devcast silences [--min 0.5] [--db -45] [--track mic] [--from --to]`: quiet stretches of the
//! output, for cutting pauses (`devcast cut <from>-<to>`). Each clip is searched in its own
//! recording, by default with the threshold of that recording (`silence::threshold`); quiet
//! parts on both sides of a cut or a clip boundary join into one.

use std::collections::HashMap;

use dc_audio::silence::{self, DEFAULT_MIN};
use dc_audio::{SourceTrack, tracks};
use dc_core::{OutputTime, Project, TimeDelta, TimeMap};
use dc_project::ProjectDir;
use serde_json::json;

use crate::edit::{Ctx, answer};
use crate::error::CliError;
use crate::ls::time;
use crate::round::thousandths;
use crate::run::Answer;

#[derive(Debug, Clone, clap::Args)]
pub(crate) struct Args {
    #[arg(long, default_value_t = 0.5, help = "Самая короткая пауза, секунды")]
    min: f64,
    #[arg(
        long,
        allow_hyphen_values = true,
        help = "Порог, dBFS: тише — тишина; по умолчанию — посередине между шумом и речью записи"
    )]
    db: Option<f64>,
    #[arg(
        long,
        help = "Дорожка: mic (по умолчанию; у импортированного видео — первая), system, audio-1 …"
    )]
    track: Option<String>,
    #[arg(long, help = "Только паузы, которые идут после этого момента")]
    from: Option<String>,
    #[arg(long, help = "Только паузы, которые начинаются раньше этого момента")]
    to: Option<String>,
}

pub(crate) fn silences(ctx: &Ctx<'_>, query: &Args) -> Result<Answer, CliError> {
    if !(query.min > 0.0 && query.min.is_finite()) {
        return Err(CliError::Usage(format!(
            "--min {}: длительность в секундах больше 0, например --min {}",
            query.min,
            DEFAULT_MIN.as_secs_f64()
        )));
    }
    if let Some(db) = query.db.filter(|db| !(-120.0..0.0).contains(db)) {
        return Err(CliError::Usage(format!(
            "--db {db}: порог в dBFS от −120 до 0, например --db -45; без --db — порог самой записи"
        )));
    }
    let (from, to) = (
        time(query.from.as_ref(), "--from")?,
        time(query.to.as_ref(), "--to")?,
    );
    let mut workspace = ctx.open(false)?;
    let min = TimeDelta::from_secs_f64(query.min);
    let mut found = search(workspace.dir(), &workspace.project, query, min)?;
    found.retain(|&(start, end)| {
        end.since(start) >= min && from.is_none_or(|f| end > f) && to.is_none_or(|t| start < t)
    });
    let total: i64 = found.iter().map(|&(s, e)| e.since(s).micros()).sum();
    let mut lines: Vec<String> = found
        .iter()
        .map(|&(s, e)| format!("{s}–{e}  {:.2} с", e.since(s).as_secs_f64()))
        .collect();
    lines.push(match found.len() {
        0 => format!("тишины от {} с не найдено", query.min),
        n => format!("тишина: {n}, всего {:.2} с", total as f64 / 1e6),
    });
    let items: Vec<_> = found
        .iter()
        .map(|&(s, e)| json!({"from": secs(s), "to": secs(e), "duration": secs_of(e.since(s))}))
        .collect();
    let json = json!({"total": found.len(), "seconds": total as f64 / 1e6, "items": items});
    Ok(answer(&mut workspace, lines.join("\n"), json))
}

/// Quiet stretches of the whole output, joined across segment boundaries.
fn search(
    dir: &ProjectDir,
    project: &Project,
    query: &Args,
    min: TimeDelta,
) -> Result<Vec<(OutputTime, OutputTime)>, CliError> {
    let failed = |e: dc_audio::AudioError| CliError::Failed(e.to_string());
    let map = TimeMap::build(project);
    let mut chosen: HashMap<String, (SourceTrack, f64)> = HashMap::new();
    let mut found: Vec<(OutputTime, OutputTime)> = Vec::new();
    for segment in map.segments() {
        let Some(clip) = project.clip(segment.clip) else {
            continue;
        };
        if !chosen.contains_key(&segment.media) {
            let track = crate::speech::pick(
                tracks(dir, &clip.media).map_err(failed)?,
                query.track.as_ref(),
                &segment.media,
            )?;
            let db = match query.db {
                Some(db) => db,
                None => silence::threshold(&track).map_err(failed)?,
            };
            chosen.insert(segment.media.clone(), (track, db));
        }
        let Some((track, db)) = chosen.get(&segment.media) else {
            continue;
        };
        // The minimum is in output time; in source time a slowed segment needs less.
        let source_min = TimeDelta::from_micros((min.micros() as f64 * segment.speed) as i64);
        for quiet in silence::find(track, segment.source, *db, source_min).map_err(failed)? {
            let start = segment.output_at(quiet.start());
            let end = if quiet.end() == segment.source.end() {
                segment.output_end()
            } else {
                segment.output_at(quiet.end())
            };
            match found.last_mut() {
                Some(last) if start <= last.1 + TimeDelta::from_millis(1) => last.1 = end,
                _ => found.push((start, end)),
            }
        }
    }
    Ok(found)
}

fn secs(t: OutputTime) -> f64 {
    thousandths(t.as_secs_f64())
}

fn secs_of(d: TimeDelta) -> f64 {
    thousandths(d.as_secs_f64())
}
