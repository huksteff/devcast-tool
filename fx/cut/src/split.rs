//! `clip split`: cuts a clip in two at an output moment, like the blade in an editor. Both halves
//! keep the clip's settings (crop, speed, cuts on their side); effects are anchored to the
//! recording, not to the clip, so nothing on screen changes.

use dc_core::time::{Output, Source};
use dc_core::{
    Clip, Command, CommandError, Diff, Op, OutputTime, Project, Range, TimeArg, TimeMap,
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
        let t = self.at.0;
        let map = TimeMap::build(project);
        let end = OutputTime::ZERO + map.duration();
        let segment = map
            .segment_at(t)
            .filter(|_| t > OutputTime::ZERO)
            .ok_or_else(|| {
                CommandError::new(format!(
                    "разрезать можно только внутри видео 0:00.000–{end}, а не в {t}"
                ))
            })?;
        let first = map.segments().iter().find(|s| s.clip == segment.clip);
        if first.is_some_and(|s| s.output_start == t) {
            return Err(CommandError::new(format!(
                "в {t} уже стык клипов: разрезать нечего; стыки: devcast ls clips"
            )));
        }
        let (id, at) = (segment.clip, segment.source_at(t));
        let index = dc_core::index_of(&project.clips, id, |c| c.id)?;
        let clip = &project.clips[index];
        let left = part(clip, range_of(clip.trim.start(), at)?);
        let mut right = part(clip, range_of(at, clip.trim.end())?);
        right.id = project.clip_ids.allocate();
        let added = right.id;
        project.clips[index] = left;
        project.clips.insert(index + 1, right);
        Ok(Diff::changed(id).and(Diff::added(added)))
    }
}

/// `clip` limited to `trim`: cuts and speed ranges outside it are dropped, the rest is kept.
fn part(clip: &Clip, trim: Range<Source>) -> Clip {
    let mut part = clip.clone();
    part.trim = trim;
    part.cuts = clip
        .cuts
        .iter()
        .filter_map(|c| c.intersect(&trim))
        .collect();
    part.speeds = clip
        .speeds
        .iter()
        .filter_map(|s| {
            let mut speed = *s;
            speed.range = s.range.intersect(&trim)?;
            Some(speed)
        })
        .collect();
    part
}

impl Op for ClipSplit {
    const NAME: &'static str = "clip split";
    const ABOUT: &'static str =
        "Разрезать клип в момент выхода на два; настройки клипа и эффекты остаются на месте";
    const EXAMPLE: &'static str = "clip split --at 12.5";
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod tests;
