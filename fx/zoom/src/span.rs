//! Times from commands are seconds of the output video; zooms are stored in a recording's source
//! time, so clips split from one recording share their zooms.

use dc_core::time::Source;
use dc_core::{ClipId, CommandError, OutputTime, Project, Range, SourceTime, TimeDelta, TimeMap};

/// A moment of one recording: `Media.path` and source time.
pub type Edge = (String, SourceTime);

/// The output timeline of a project, for turning command times into zoom edges.
pub struct Timeline {
    map: TimeMap,
    media: Vec<(ClipId, String)>,
}

impl Timeline {
    pub fn new(project: &Project) -> Result<Self, CommandError> {
        let map = TimeMap::build(project);
        if map.duration() <= TimeDelta::ZERO {
            return Err(CommandError::new(
                "в проекте нет клипов: сначала добавь видео (devcast clip add)",
            ));
        }
        Ok(Self {
            map,
            media: project
                .clips
                .iter()
                .map(|c| (c.id, c.media.path.clone()))
                .collect(),
        })
    }

    /// Recording of `clip`; every clip of the time map is in `media`.
    fn media_of(&self, clip: ClipId) -> &str {
        self.media
            .iter()
            .find(|(id, _)| *id == clip)
            .map_or("", |(_, path)| path.as_str())
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
    pub fn start(&self, t: OutputTime) -> Result<Edge, CommandError> {
        let t = Self::output(t)?;
        let (clip, source) = self.map.to_source(t).ok_or_else(|| {
            CommandError::new(format!(
                "начало зума {t} не раньше конца видео ({}): укажи время меньше",
                self.end_of_video()
            ))
        })?;
        Ok((self.media_of(clip).to_owned(), source))
    }

    /// The exclusive source end of what is shown before `t`: at the end of a segment it stays in
    /// that segment's clip instead of jumping to the next one.
    pub fn end(&self, t: OutputTime) -> Result<Edge, CommandError> {
        let t = Self::output(t)?;
        if t > self.end_of_video() {
            return Err(CommandError::new(format!(
                "конец зума {t} позже конца видео ({}): укороти зум",
                self.end_of_video()
            )));
        }
        if let Some(segment) = self.map.segments().iter().find(|s| s.output_end() == t) {
            return Ok((self.media_of(segment.clip).to_owned(), segment.source.end()));
        }
        self.start(t)
    }

    /// Range of one recording from `start` to `end`.
    pub fn span(&self, start: Edge, end: Edge) -> Result<(String, Range<Source>), CommandError> {
        let (media, from) = start;
        if end.0 != media {
            let joint = self
                .map
                .segments()
                .iter()
                .skip_while(|s| self.media_of(s.clip) != media || !s.source.contains(from))
                .find(|s| self.media_of(s.clip) != media)
                .map_or(self.end_of_video(), |s| s.output_start);
            return Err(CommandError::new(format!(
                "зум переходит через стык клипов разных записей в {joint}: раздели его на два зума"
            )));
        }
        if end.1 <= from {
            return Err(CommandError::new(
                "длительность зума должна быть больше нуля: конец позже начала",
            ));
        }
        Range::new(from, end.1)
            .map(|range| (media, range))
            .map_err(|e| CommandError::new(e.to_string()))
    }
}
