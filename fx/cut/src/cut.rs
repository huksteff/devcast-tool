//! `cut`: removes ranges of the output. Each becomes a source cut of every clip it touches; a clip
//! cut entirely leaves the track.

use dc_core::time::{Output, Source};
use dc_core::{ClipId, Command, CommandError, Diff, Op, Project, Range, RangesArg};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::track::{range_of, source_spans};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Cut {
    /// Что вырезать, время выхода: 10.2-12.5,20-21.
    pub ranges: RangesArg<Output>,
}

impl Command for Cut {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        cut(project, &self.ranges.0)
    }
}

impl Op for Cut {
    const NAME: &'static str = "cut";
    const ABOUT: &'static str = "Вырезать участки видео (время выхода, до выреза); в devcast ещё слова w12-w20 и паузы речи --silences [--pad 0.15]";
    const EXAMPLE: &'static str = "cut 10.2-12.5,20-21";
    const POSITIONAL: &'static [&'static str] = &["ranges"];
}

/// Cuts `ranges` (output time before the command) out of the project.
fn cut(project: &mut Project, ranges: &[Range<Output>]) -> Result<Diff, CommandError> {
    let mut spans = Vec::new();
    let mut diff = Diff::default();
    for &range in ranges {
        spans.extend(source_spans(project, range)?);
        diff = diff.and(Diff::note(format!(
            "вырез {}–{}",
            range.start(),
            range.end()
        )));
    }
    let touched: Vec<ClipId> = spans.iter().map(|(id, _)| *id).collect();
    for clip in &mut project.clips {
        for (_, span) in spans.iter().filter(|(id, _)| *id == clip.id) {
            add_cut(&mut clip.cuts, *span)?;
        }
    }
    let whole = |trim: Range<Source>, cuts: &[Range<Source>]| cuts == [trim];
    if project.clips.iter().all(|c| whole(c.trim, &c.cuts)) {
        return Err(CommandError::new(
            "нельзя вырезать всё видео: останется пустой проект. Сузь диапазон",
        ));
    }
    for clip in project.clips.iter().filter(|c| touched.contains(&c.id)) {
        diff = diff.and(if whole(clip.trim, &clip.cuts) {
            Diff::removed(clip.id)
        } else {
            Diff::changed(clip.id)
        });
    }
    project.clips.retain(|c| !whole(c.trim, &c.cuts));
    Ok(diff)
}

/// Adds `cut` to sorted, separate `cuts`, merging ranges that overlap or touch.
fn add_cut(cuts: &mut Vec<Range<Source>>, cut: Range<Source>) -> Result<(), CommandError> {
    cuts.push(cut);
    cuts.sort_by_key(Range::start);
    let mut merged: Vec<Range<Source>> = Vec::with_capacity(cuts.len());
    for &next in cuts.iter() {
        match merged.last_mut() {
            Some(last) if next.start() <= last.end() => {
                *last = range_of(last.start(), last.end().max(next.end()))?;
            }
            _ => merged.push(next),
        }
    }
    *cuts = merged;
    Ok(())
}

#[cfg(test)]
#[path = "cut_tests.rs"]
mod tests;
