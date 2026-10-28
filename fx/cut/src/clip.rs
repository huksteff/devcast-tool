//! Clips on the magnetic main track: split, remove, reorder. The track has no gaps, so removing
//! or moving a clip only changes `project.clips`.

use dc_core::project::SpeedRange;
use dc_core::time::{Output, Source};
use dc_core::{
    ClipId, Command, CommandError, Diff, Op, OutputTime, Project, Range, TimeArg, TimeMap,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::track::range_of;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClipSplit {
    /// Где разрезать, время выхода: 12.5 или 1:02.5.
    pub at: TimeArg<Output>,
}

impl Command for ClipSplit {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        split(project, self.at.0)
    }
}

impl Op for ClipSplit {
    const NAME: &'static str = "clip split";
    const ABOUT: &'static str = "Разрезать клип на два в точке (время выхода)";
    const EXAMPLE: &'static str = "clip split --at 12.5";
}

/// Splits the clip shown at output time `at` into two clips of the same media.
fn split(project: &mut Project, at: OutputTime) -> Result<Diff, CommandError> {
    let map = TimeMap::build(project);
    let refuse = || {
        CommandError::new(format!(
            "в {at} нечего разрезать: точка должна быть внутри клипа, не на стыке и не в конце видео"
        ))
    };
    let (id, t) = map.to_source(at).ok_or_else(refuse)?;
    let clip_start = map.segments().iter().find(|s| s.clip == id);
    if clip_start.is_some_and(|s| s.output_start == at) {
        return Err(refuse());
    }
    let index = project
        .clips
        .iter()
        .position(|c| c.id == id)
        .ok_or_else(refuse)?;
    let new_id = project.clip_ids.allocate();
    let left = &mut project.clips[index];
    let (left_half, right_half) = (
        range_of(left.trim.start(), t)?,
        range_of(t, left.trim.end())?,
    );
    let mut right = left.clone();
    right.id = new_id;
    right.trim = right_half;
    right.cuts = within(&left.cuts, right_half);
    right.speeds = speeds_within(&left.speeds, right_half);
    left.trim = left_half;
    left.cuts = within(&left.cuts, left_half);
    left.speeds = speeds_within(&left.speeds, left_half);
    project.clips.insert(index + 1, right);
    Ok(Diff::changed(id).and(Diff::added(new_id)))
}

fn within(ranges: &[Range<Source>], bounds: Range<Source>) -> Vec<Range<Source>> {
    ranges.iter().filter_map(|r| r.intersect(&bounds)).collect()
}

fn speeds_within(speeds: &[SpeedRange], bounds: Range<Source>) -> Vec<SpeedRange> {
    speeds
        .iter()
        .filter_map(|s| {
            s.range
                .intersect(&bounds)
                .map(|range| SpeedRange { range, ..*s })
        })
        .collect()
}

fn index_of(project: &Project, id: ClipId) -> Result<usize, CommandError> {
    project
        .clips
        .iter()
        .position(|c| c.id == id)
        .ok_or_else(|| CommandError::not_found(id, project.clips.iter().map(|c| c.id)))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClipRm {
    /// Клип, например c2.
    pub id: ClipId,
}

impl Command for ClipRm {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let index = index_of(project, self.id)?;
        if project.clips.len() == 1 {
            return Err(CommandError::new(format!(
                "{} — последний клип, без него проект пуст. Добавь другой клип или вырежи часть: devcast cut",
                self.id
            )));
        }
        project.clips.remove(index);
        Ok(Diff::removed(self.id))
    }
}

impl Op for ClipRm {
    const NAME: &'static str = "clip rm";
    const ABOUT: &'static str = "Удалить клип с главной дорожки; следующие сдвигаются без дыры";
    const EXAMPLE: &'static str = "clip rm c2";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClipMove {
    /// Клип, например c2.
    pub id: ClipId,
    /// Новое место на дорожке, с 1.
    pub to: usize,
}

impl Command for ClipMove {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let index = index_of(project, self.id)?;
        let count = project.clips.len();
        if !(1..=count).contains(&self.to) {
            return Err(CommandError::new(format!(
                "место {} вне дорожки: клипов {count}, место 1–{count}",
                self.to
            )));
        }
        if index + 1 == self.to {
            return Ok(Diff::default());
        }
        let clip = project.clips.remove(index);
        project.clips.insert(self.to - 1, clip);
        Ok(Diff::changed(self.id))
    }
}

impl Op for ClipMove {
    const NAME: &'static str = "clip move";
    const ABOUT: &'static str = "Переставить клип на другое место главной дорожки";
    const EXAMPLE: &'static str = "clip move c2 --to 1";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[cfg(test)]
#[path = "clip_tests.rs"]
mod tests;
