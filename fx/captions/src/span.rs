//! Times from commands are seconds of the output video; cues stay in the source time of their
//! recording, so a cue's edges and split points must fall into clips of that recording.

use dc_core::{CommandError, OutputTime, Project, SourceTime, TimeMap};

use crate::model::Cue;

/// The output timeline of a project, for turning command times into source moments of a cue.
pub(crate) struct Timeline {
    map: TimeMap,
}

impl Timeline {
    pub(crate) fn new(project: &Project) -> Self {
        Self {
            map: TimeMap::build(project),
        }
    }

    fn outside(&self, cue: &Cue, t: OutputTime) -> CommandError {
        CommandError::new(format!(
            "в {t} идёт не запись реплики {} ({}): укажи время внутри её клипов (devcast ls captions, devcast ls clips)",
            cue.id, cue.media
        ))
    }

    /// The recording shown at `t`.
    pub(crate) fn media_at(&self, t: OutputTime) -> Result<String, CommandError> {
        self.map
            .segment_at(t)
            .map(|s| s.media.clone())
            .ok_or_else(|| {
                CommandError::new(format!(
                    "в {t} нет видео: реплика начинается внутри клипов (devcast ls clips)"
                ))
            })
    }

    /// The source moment of `cue`'s recording shown at `t`.
    pub(crate) fn start(&self, cue: &Cue, t: OutputTime) -> Result<SourceTime, CommandError> {
        self.map
            .segment_at(t)
            .filter(|s| s.media == cue.media)
            .map(|s| s.source_at(t))
            .ok_or_else(|| self.outside(cue, t))
    }

    /// The exclusive source end of what `cue`'s recording shows before `t`: at the end of a
    /// segment it stays in that segment.
    pub(crate) fn end(&self, cue: &Cue, t: OutputTime) -> Result<SourceTime, CommandError> {
        match self.map.ending_at(t).filter(|s| s.media == cue.media) {
            Some(segment) => Ok(segment.source.end()),
            None => self.start(cue, t),
        }
    }
}
