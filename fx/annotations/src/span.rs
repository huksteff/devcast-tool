//! Annotations keep a recording's source time; command times come from the output video through
//! [`dc_core::span::Timeline`].

use dc_core::span::{Edge, Timeline};
use dc_core::time::{Output, Source};
use dc_core::{CommandError, Project, Range, TimeDelta};

use crate::model::{AnnotKind, Annotation, MIN_DURATION};

pub(crate) fn timeline(project: &Project) -> Result<Timeline, CommandError> {
    Timeline::new::<AnnotKind>(project)
}

/// The source range between two edges of one recording, at least `MIN_DURATION` long on the
/// output video.
pub(crate) fn span(
    timeline: &Timeline,
    start: Edge,
    end: Edge,
) -> Result<(String, Range<Source>), CommandError> {
    if let Some(joint) = timeline.joint(&start, &end) {
        return Err(CommandError::new(format!(
            "аннотация переходит через стык клипов разных записей в {joint}: закончи её до стыка или сделай две"
        )));
    }
    let (media, from) = start;
    let range = Range::new(from, end.1)
        .ok()
        .filter(|r| !r.is_empty())
        .ok_or_else(|| CommandError::new("конец аннотации должен быть позже начала"))?;
    let shown = timeline
        .map()
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

/// Where the annotation plays on the output video; `None` if all of it is cut out.
pub(crate) fn shown(timeline: &Timeline, annotation: &Annotation) -> Option<Range<Output>> {
    timeline
        .map()
        .output_span(&annotation.media, annotation.range)
}
