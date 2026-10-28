//! `devcast events <clicks|keys|activity> [--from --to]`: input of the recordings on the output
//! timeline. Times are output times; events in cut-out parts are not listed, only counted.
//! Positions are fractions of the clip's picture after its crop, like a zoom focus: outside the
//! picture they leave 0..1.

use dc_capture::input::{Button, CursorEvent};
use dc_core::{OutputTime, Project, SourceTime, TimeDelta, TimeMap};
use serde_json::{Value, json};

use crate::edit::{Ctx, answer};
use crate::error::CliError;
use crate::ls::time;
use crate::run::Answer;
use crate::telemetry::{self, Recorded, fraction, place};

/// Input with pauses no longer than this belongs to one stretch of activity.
pub(crate) const IDLE: TimeDelta = TimeDelta::from_millis(1000);

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Kind {
    Clicks,
    Keys,
    Activity,
}

impl Kind {
    fn list(self) -> &'static str {
        match self {
            Self::Clicks => "clicks",
            Self::Keys => "keys",
            Self::Activity => "activity",
        }
    }
}

/// One line: where it starts and ends on the output, its text and JSON fields.
struct Row {
    from: OutputTime,
    to: OutputTime,
    text: String,
    fields: Value,
}

/// What a kind of event contributes to a stretch of activity.
#[derive(Default, Clone, Copy)]
struct Counts {
    moves: usize,
    clicks: usize,
    wheel: usize,
    keys: usize,
}

pub(crate) fn events(
    ctx: &Ctx<'_>,
    kind: Kind,
    from: Option<&String>,
    to: Option<&String>,
) -> Result<Answer, CliError> {
    let (from, to) = (time(from, "--from")?, time(to, "--to")?);
    let mut workspace = ctx.open(false)?;
    let project = &workspace.project;
    let recorded = telemetry::read(workspace.dir().root(), project)?;
    let map = TimeMap::build(project);
    let (mut rows, cut) = match kind {
        Kind::Clicks => clicks(&map, project, &recorded),
        Kind::Keys => keys(&map, project, &recorded),
        Kind::Activity => activity(&map, project, &recorded),
    };
    let none = recorded.is_empty();
    let from = from.unwrap_or(OutputTime::ZERO);
    let to = to.unwrap_or(OutputTime::from_micros(i64::MAX));
    rows.retain(|r| r.from < to && from <= r.to);
    let list = kind.list();
    let mut lines: Vec<String> = rows.iter().map(|r| r.text.clone()).collect();
    if lines.is_empty() {
        lines.push(format!("{list}: пусто"));
    }
    if cut > 0 {
        lines.push(format!("вырезано и не показано событий: {cut}"));
    }
    let json = json!({
        "list": list,
        "total": rows.len(),
        "cut": cut,
        "items": rows.into_iter().map(|r| r.fields).collect::<Vec<_>>(),
    });
    let mut answer = answer(&mut workspace, lines.join("\n"), json);
    if none {
        answer.warnings.push(
            "в проекте нет записей DevCast: у импортированных видео нет кликов и клавиш".to_owned(),
        );
    }
    Ok(answer)
}

fn secs(t: OutputTime) -> f64 {
    round(t.as_secs_f64())
}

/// Three decimals: milliseconds, or a pixel of a 1000-pixel frame.
fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn button(button: Button) -> &'static str {
    match button {
        Button::Left => "left",
        Button::Right => "right",
        Button::Middle => "middle",
        Button::X1 => "x1",
        Button::X2 => "x2",
    }
}

/// Places every event of every recording on the output; returns the placed ones in time order
/// and how many were cut out.
fn placed<E>(
    map: &TimeMap,
    project: &Project,
    recorded: &[Recorded<'_>],
    events: impl Fn(&Recorded<'_>) -> Vec<(SourceTime, E)>,
) -> (Vec<(OutputTime, E)>, usize) {
    let mut shown = Vec::new();
    let mut cut = 0;
    for recording in recorded {
        for (at, event) in events(recording) {
            match place(map, project, recording.media, at) {
                Some((output, _)) => shown.push((output, event)),
                None => cut += 1,
            }
        }
    }
    shown.sort_by_key(|(at, _)| *at);
    (shown, cut)
}

fn clicks(map: &TimeMap, project: &Project, recorded: &[Recorded<'_>]) -> (Vec<Row>, usize) {
    let mut rows = Vec::new();
    let mut cut = 0;
    for recording in recorded {
        for click in recording.telemetry.clicks() {
            let Some((at, clip)) = place(map, project, recording.media, click.at) else {
                cut += 1;
                continue;
            };
            let p = clip
                .crop
                .to_local(fraction(recording.media, click.x, click.y));
            let (x, y) = (round(p.x), round(p.y));
            let name = button(click.button);
            let double = if click.double { " double" } else { "" };
            rows.push(Row {
                from: at,
                to: at,
                text: format!("{at}  {name}{double}  x={x}  y={y}"),
                fields: json!({
                    "at": secs(at), "clip": clip.id.to_string(), "button": name,
                    "double": click.double, "x": x, "y": y,
                }),
            });
        }
    }
    rows.sort_by_key(|r| r.from);
    (rows, cut)
}

fn keys(map: &TimeMap, project: &Project, recorded: &[Recorded<'_>]) -> (Vec<Row>, usize) {
    let (shown, cut) = placed(map, project, recorded, |r| {
        r.telemetry.keys.iter().map(|k| (k.at, k.event)).collect()
    });
    let rows = shown
        .into_iter()
        .map(|(at, key)| {
            let name = key.name();
            Row {
                from: at,
                to: at,
                text: format!("{at}  {name}"),
                fields: json!({"at": secs(at), "keys": name}),
            }
        })
        .collect();
    (rows, cut)
}

/// Stretches of input separated by more than [`IDLE`] without any. The first position of a
/// recording is where the pointer was at the start, not a move.
fn activity(map: &TimeMap, project: &Project, recorded: &[Recorded<'_>]) -> (Vec<Row>, usize) {
    let (shown, cut) = placed(map, project, recorded, |r| {
        let start = r
            .telemetry
            .cursor
            .iter()
            .position(|e| e.event.position().is_some());
        let mouse = r
            .telemetry
            .cursor
            .iter()
            .enumerate()
            .filter_map(move |(i, e)| {
                let counts = match e.event {
                    CursorEvent::Move { .. } if Some(i) == start => return None,
                    CursorEvent::Move { .. } | CursorEvent::Up { .. } => Counts {
                        moves: 1,
                        ..Counts::default()
                    },
                    CursorEvent::Down { .. } => Counts {
                        clicks: 1,
                        ..Counts::default()
                    },
                    CursorEvent::Wheel { .. } | CursorEvent::HWheel { .. } => Counts {
                        wheel: 1,
                        ..Counts::default()
                    },
                    CursorEvent::Image { .. } | CursorEvent::Shape { .. } => return None,
                };
                Some((e.at, counts))
            });
        let keys = r.telemetry.keys.iter().map(|k| {
            let counts = Counts {
                keys: 1,
                ..Counts::default()
            };
            (k.at, counts)
        });
        mouse.chain(keys).collect()
    });
    let mut stretches: Vec<(OutputTime, OutputTime, Counts)> = Vec::new();
    for (at, counts) in shown {
        match stretches.last_mut() {
            Some((_, end, total)) if at.since(*end) <= IDLE => {
                *end = at;
                total.moves += counts.moves;
                total.clicks += counts.clicks;
                total.wheel += counts.wheel;
                total.keys += counts.keys;
            }
            _ => stretches.push((at, at, counts)),
        }
    }
    let rows = stretches
        .into_iter()
        .map(|(from, to, c)| {
            let counted = [
                ("moves", c.moves),
                ("clicks", c.clicks),
                ("wheel", c.wheel),
                ("keys", c.keys),
            ];
            let mut text = format!("{from}–{to}");
            for (name, n) in counted.iter().filter(|(_, n)| *n > 0) {
                text.push_str(&format!("  {name}={n}"));
            }
            Row {
                from,
                to,
                text,
                fields: json!({
                    "from": secs(from), "to": secs(to), "moves": c.moves,
                    "clicks": c.clicks, "wheel": c.wheel, "keys": c.keys,
                }),
            }
        })
        .collect();
    (rows, cut)
}
