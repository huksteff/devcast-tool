//! Command times are seconds of the output video; effects stay in a recording's source time
//! (`Media.path` + time), so clips split from one recording share them. [`Timeline`] turns an
//! output moment into such an edge, with errors that name the object (`начало зума …`).

use crate::command::CommandError;
use crate::id::IdKind;
use crate::project::Project;
use crate::time::{OutputTime, SourceTime, TimeDelta};
use crate::timemap::TimeMap;

/// A moment of one recording: `Media.path` and source time.
pub type Edge = (String, SourceTime);

/// The output timeline of a project with at least one clip.
pub struct Timeline {
    map: TimeMap,
    /// Genitive noun of what the edges belong to ([`IdKind::NOUN`]), for errors.
    noun: &'static str,
}

impl Timeline {
    /// The timeline for objects of kind `K`; an error if the project has no clips.
    pub fn new<K: IdKind>(project: &Project) -> Result<Self, CommandError> {
        let map = TimeMap::build(project);
        if map.duration() <= TimeDelta::ZERO {
            return Err(CommandError::new(
                "в проекте нет клипов: сначала добавь видео (devcast clip add)",
            ));
        }
        Ok(Self { map, noun: K::NOUN })
    }

    pub fn map(&self) -> &TimeMap {
        &self.map
    }

    pub fn end_of_video(&self) -> OutputTime {
        OutputTime::ZERO + self.map.duration()
    }

    fn output(t: OutputTime) -> Result<OutputTime, CommandError> {
        if t < OutputTime::ZERO {
            return Err(CommandError::new(format!(
                "время {t} раньше начала видео: укажи 0 или больше"
            )));
        }
        Ok(t)
    }

    /// The source moment shown at `t`.
    pub fn start(&self, t: OutputTime) -> Result<Edge, CommandError> {
        let t = Self::output(t)?;
        let segment = self.map.segment_at(t).ok_or_else(|| {
            CommandError::new(format!(
                "начало {} {t} не раньше конца видео ({}): укажи время меньше",
                self.noun,
                self.end_of_video()
            ))
        })?;
        Ok((segment.media.clone(), segment.source_at(t)))
    }

    /// The exclusive source end of what is shown before `t`: at the end of a segment it stays in
    /// that segment's clip instead of jumping to the next one.
    pub fn end(&self, t: OutputTime) -> Result<Edge, CommandError> {
        let t = Self::output(t)?;
        if t > self.end_of_video() {
            return Err(CommandError::new(format!(
                "конец {} {t} позже конца видео ({}): укороти или поставь раньше",
                self.noun,
                self.end_of_video()
            )));
        }
        match self.map.ending_at(t) {
            Some(segment) => Ok((segment.media.clone(), segment.source.end())),
            None => self.start(t),
        }
    }

    /// Where the clips of `start`'s recording give way to another recording, when `end` is in
    /// another recording; `None` when both edges are in one recording.
    pub fn joint(&self, start: &Edge, end: &Edge) -> Option<OutputTime> {
        let (media, from) = start;
        if &end.0 == media {
            return None;
        }
        let joint = self
            .map
            .segments()
            .iter()
            .skip_while(|s| &s.media != media || !s.source.contains(*from))
            .find(|s| &s.media != media)
            .map_or(self.end_of_video(), |s| s.output_start);
        Some(joint)
    }
}
