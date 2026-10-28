//! `speed`: plays a range of the output faster or slower. Each touched clip gets a speed range over
//! the source shown there; `1` resets it.

use dc_core::project::{SPEED_MAX, SPEED_MIN, SpeedRange};
use dc_core::time::{Output, Source};
use dc_core::{Command, CommandError, Diff, Op, Project, Range, RangeArg};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::track::{range_of, source_spans};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Speed {
    /// Участок, время выхода: 10-20.
    pub range: RangeArg<Output>,
    /// Скорость 0.25–16; 2 — вдвое быстрее, 1 — обычная.
    pub factor: f64,
}

impl Command for Speed {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        speed(project, self.range.0, self.factor)
    }
}

impl Op for Speed {
    const NAME: &'static str = "speed";
    const ABOUT: &'static str = "Ускорить или замедлить участок видео (0.25–16×, 1 — сброс)";
    const EXAMPLE: &'static str = "speed 10-20 2";
    const POSITIONAL: &'static [&'static str] = &["range", "factor"];
}

/// Sets `factor` on `range` (output time before the command).
fn speed(project: &mut Project, range: Range<Output>, factor: f64) -> Result<Diff, CommandError> {
    if !(SPEED_MIN..=SPEED_MAX).contains(&factor) {
        return Err(CommandError::new(format!(
            "скорость {factor} вне {SPEED_MIN}–{SPEED_MAX}×; 1 — обычная скорость"
        )));
    }
    let spans = source_spans(project, range)?;
    let mut diff = Diff::default();
    for clip in &mut project.clips {
        let before = clip.speeds.clone();
        for (_, span) in spans.iter().filter(|(id, _)| *id == clip.id) {
            set_speed(&mut clip.speeds, *span, factor)?;
        }
        if clip.speeds != before {
            diff = diff.and(Diff::changed(clip.id));
        }
    }
    if diff.is_empty() {
        return Ok(diff);
    }
    let note = format!("скорость {factor}× {}–{}", range.start(), range.end());
    Ok(Diff::note(note).and(diff))
}

/// Puts `factor` on `span` over sorted, separate `speeds`; touching ranges of one factor merge.
fn set_speed(
    speeds: &mut Vec<SpeedRange>,
    span: Range<Source>,
    factor: f64,
) -> Result<(), CommandError> {
    let mut kept = Vec::with_capacity(speeds.len() + 2);
    for s in speeds.drain(..) {
        if s.range.intersect(&span).is_none() {
            kept.push(s);
            continue;
        }
        if s.range.start() < span.start() {
            let range = range_of(s.range.start(), span.start())?;
            kept.push(SpeedRange { range, ..s });
        }
        if s.range.end() > span.end() {
            let range = range_of(span.end(), s.range.end())?;
            kept.push(SpeedRange { range, ..s });
        }
    }
    if factor != 1.0 {
        kept.push(SpeedRange {
            range: span,
            factor,
        });
    }
    kept.sort_by_key(|s| s.range.start());
    for s in kept {
        match speeds.last_mut() {
            Some(last) if last.range.end() == s.range.start() && last.factor == s.factor => {
                last.range = range_of(last.range.start(), s.range.end())?;
            }
            _ => speeds.push(s),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "speed_tests.rs"]
mod tests;
