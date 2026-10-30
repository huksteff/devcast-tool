//! Times from commands are seconds of the output video; annotations are stored in a recording's
//! source time, so clips split from one recording share them.

use dc_core::time::Source;
use dc_core::{CommandError, OutputTime, Project, Range, SourceTime, TimeDelta, TimeMap};

use crate::model::{Annotation, MIN_DURATION};

/// A moment of one recording: `Media.path` and source time.
pub(crate) type Edge = (String, SourceTime);

/// The output timeline of a project, for turning command times into annotation edges.
pub(crate) struct Timeline {
    map: TimeMap,
}

impl Timeline {
    pub(crate) fn new(project: &Project) -> Result<Self, CommandError> {
        let map = TimeMap::build(project);
        if map.duration() <= TimeDelta::ZERO {
            return Err(CommandError::new(
                "в проекте нет клипов: сначала добавь видео (devcast clip add)",
            ));
        }
        Ok(Self { map })
    }

    fn end_of_video(&self) -> OutputTime {
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
    pub(crate) fn start(&self, t: OutputTime) -> Result<Edge, CommandError> {
        let t = Self::output(t)?;
        let segment = self.map.segment_at(t).ok_or_else(|| {
            CommandError::new(format!(
                "начало аннотации {t} не раньше конца видео ({}): укажи время меньше",
                self.end_of_video()
            ))
        })?;
        Ok((segment.media.clone(), segment.source_at(t)))
    }

    /// The exclusive source end of what is shown before `t`: at the end of a segment it stays in
    /// that segment's clip instead of jumping to the next one.
    pub(crate) fn end(&self, t: OutputTime) -> Result<Edge, CommandError> {
        let t = Self::output(t)?;
        if t > self.end_of_video() {
            return Err(CommandError::new(format!(
                "конец аннотации {t} позже конца видео ({}): укороти её или поставь раньше",
                self.end_of_video()
            )));
        }
        if let Some(segment) = self.map.segments().iter().find(|s| s.output_end() == t) {
            return Ok((segment.media.clone(), segment.source.end()));
        }
        self.start(t)
    }

    /// The source range between two edges of one recording, at least `MIN_DURATION` long on the
    /// output video.
    pub(crate) fn span(
        &self,
        start: Edge,
        end: Edge,
    ) -> Result<(String, Range<Source>), CommandError> {
        let (media, from) = start;
        if end.0 != media {
            let joint = self
                .map
                .segments()
                .iter()
                .skip_while(|s| s.media != media || !s.source.contains(from))
                .find(|s| s.media != media)
                .map_or(self.end_of_video(), |s| s.output_start);
            return Err(CommandError::new(format!(
                "аннотация переходит через стык клипов разных записей в {joint}: закончи её до стыка или сделай две"
            )));
        }
        let range = Range::new(from, end.1)
            .ok()
            .filter(|r| !r.is_empty())
            .ok_or_else(|| CommandError::new("конец аннотации должен быть позже начала"))?;
        let shown = self
            .map
            .output_span(&media, range)
            .map_or(TimeDelta::ZERO, |r| r.duration());
        if shown < MIN_DURATION {
            return Err(CommandError::new(format!(
                "аннотация короче {} с на готовом видео: сделай её длиннее",
                MIN_DURATION.as_secs_f64()
            )));
        }
        Ok((media, range))
    }

    /// Output moment of source moment `t` of recording `media`; `None` if it is cut out.
    pub(crate) fn output_of(&self, media: &str, t: SourceTime) -> Option<OutputTime> {
        self.map.locate(media, t).map(|(_, at)| at)
    }

    /// Where the annotation plays on the output video; `None` if all of it is cut out.
    pub(crate) fn shown(&self, annotation: &Annotation) -> Option<Range<dc_core::time::Output>> {
        self.map.output_span(&annotation.media, annotation.range)
    }
}
