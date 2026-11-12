//! `devcast silences [--min 0.5] [--db -45] [--track mic] [--from --to]`: quiet stretches of the
//! output, for cutting pauses (`devcast cut <from>-<to>`). Each clip is searched in its own
//! recording, by default with the threshold of that recording (`silence::threshold`); quiet
//! parts on both sides of a cut or a clip boundary join into one. `devcast cut --silences`
//! cuts them by one command ([`cut_args`]).

use std::collections::HashMap;

use dc_audio::silence::{self, DEFAULT_MIN};
use dc_audio::{SourceTrack, tracks};
use dc_core::{Op, OutputTime, Project, Step, TimeDelta, TimeMap};
use dc_fx_cut::Cut;
use dc_project::ProjectDir;
use serde_json::{Map, Value, json};

use crate::edit::{Ctx, answer};
use crate::error::CliError;
use crate::ls::time;
use crate::round::thousandths;
use crate::run::Answer;
use crate::voice::Mute;
use crate::workspace::Workspace;

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
    let looked = Looked {
        track: query.track.as_ref(),
        db: query.db,
        min,
    };
    let mut found = search(workspace.dir(), &workspace.project, &looked)?;
    if query.track.is_none() {
        for mute in mute(&workspace)? {
            workspace.warnings.push(format!(
                "{} — это тишина другого звука, а не паузы речи",
                mute.line()
            ));
        }
    }
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

/// The recordings of `workspace` without a voice.
fn mute(workspace: &Workspace) -> Result<Vec<Mute>, CliError> {
    crate::voice::mute(workspace.dir(), &workspace.project)
        .map_err(|e| CliError::Failed(e.to_string()))
}

/// How the silence is looked for.
struct Looked<'a> {
    /// The track; none — the microphone (of a video: its first track).
    track: Option<&'a String>,
    /// The threshold, dBFS; none — the recording's own.
    db: Option<f64>,
    /// The shortest silence on the output.
    min: TimeDelta,
}

/// The arguments of `cut --silences` besides the flag itself.
const CUT_ARGS: [&str; 4] = ["pad", "min", "db", "track"];
/// How much of each pause `cut --silences` leaves at both of its edges by default, seconds.
const PAD: f64 = 0.15;

/// `cut --silences [--pad 0.15] [--min 0.5] [--db -45] [--track mic]` (one op or a step of
/// `apply plan.json`): the arguments of `cut` with the pauses of the speech as its ranges, each
/// without `pad` seconds at both edges; `None` for any other step. A recording without a voice
/// is refused: every moment of it is a pause.
pub(crate) fn cut_args(
    workspace: &Workspace,
    step: &Step,
) -> Result<Option<Map<String, Value>>, CliError> {
    if step.op != Cut::NAME || !step.args.contains_key(SILENCES) {
        return Ok(None);
    }
    let usage = |text: String| CliError::Usage(text);
    let example = "devcast cut --silences --pad 0.15";
    if step.args.get(SILENCES) != Some(&Value::Bool(true)) {
        return Err(usage(format!("--silences — флаг без значения: {example}")));
    }
    if let Some(other) = step
        .args
        .keys()
        .find(|k| *k != SILENCES && !CUT_ARGS.contains(&k.as_str()))
    {
        return Err(usage(format!(
            "cut --silences не берёт «{other}»: либо диапазоны (devcast cut 10-12), либо паузы ({example}; ещё --min, --db, --track)"
        )));
    }
    let number = |name: &str, default: Option<f64>| match step.args.get(name) {
        None => Ok(default),
        Some(value) => value
            .as_f64()
            .map(Some)
            .ok_or_else(|| usage(format!("--{name} {value}: нужно число, например {example}"))),
    };
    let pad = number("pad", Some(PAD))?.unwrap_or(PAD);
    let min = number("min", Some(DEFAULT_MIN.as_secs_f64()))?.unwrap_or_default();
    if !(pad >= 0.0 && pad.is_finite()) || !(min > 0.0 && min.is_finite()) {
        return Err(usage(format!(
            "--pad — секунды от 0, --min — секунды больше 0: {example}"
        )));
    }
    let track = match step.args.get("track") {
        None => None,
        Some(Value::String(name)) => Some(name),
        Some(other) => return Err(usage(format!("--track {other}: имя дорожки, например mic"))),
    };
    if track.is_none() {
        let mute = mute(workspace)?;
        if !mute.is_empty() {
            let lines: Vec<String> = mute.iter().map(Mute::line).collect();
            return Err(CliError::Failed(format!(
                "в записи нет голоса ({}): пауз речи нет, вырез убрал бы всё видео. Ничего не вырезано. Тишину другого звука покажет devcast silences --track system, вырезать её — devcast cut <от>-<до>",
                lines.join("; ")
            )));
        }
    }
    let looked = Looked {
        track,
        db: number("db", None)?,
        min: TimeDelta::from_secs_f64(min),
    };
    let pad = TimeDelta::from_secs_f64(pad);
    let ranges: Vec<String> = search(workspace.dir(), &workspace.project, &looked)?
        .into_iter()
        .filter(|&(start, end)| end.since(start) >= looked.min)
        .map(|(start, end)| (start + pad, end - pad))
        .filter(|(start, end)| start < end)
        .map(|(start, end)| format!("{}-{}", time_arg(start), time_arg(end)))
        .collect();
    if ranges.is_empty() {
        return Err(CliError::Failed(format!(
            "пауз от {min} с не найдено: вырезать нечего (devcast silences покажет тишину короче — с --min)"
        )));
    }
    let mut args = Map::new();
    args.insert("ranges".to_owned(), ranges.join(",").into());
    Ok(Some(args))
}

/// The flag of `cut` that asks for the pauses.
const SILENCES: &str = "silences";

/// `t` as a time of a range: seconds with microseconds.
fn time_arg(t: OutputTime) -> String {
    let micros = t.micros().max(0);
    format!("{}.{:06}", micros / 1_000_000, micros % 1_000_000)
}

/// Quiet stretches of the whole output, joined across segment boundaries.
fn search(
    dir: &ProjectDir,
    project: &Project,
    looked: &Looked<'_>,
) -> Result<Vec<(OutputTime, OutputTime)>, CliError> {
    let min = looked.min;
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
                looked.track,
                &segment.media,
            )?;
            let db = match looked.db {
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
