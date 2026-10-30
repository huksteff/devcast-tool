//! Zooms keep a recording's source time; command times come from the output video through
//! [`dc_core::span::Timeline`].

use dc_core::span::{Edge, Timeline};
use dc_core::time::Source;
use dc_core::{CommandError, Project, Range};

use crate::model::ZoomKind;

pub(crate) fn timeline(project: &Project) -> Result<Timeline, CommandError> {
    Timeline::new::<ZoomKind>(project)
}

/// The source range between two edges of one recording, not empty.
pub(crate) fn span(
    timeline: &Timeline,
    start: Edge,
    end: Edge,
) -> Result<(String, Range<Source>), CommandError> {
    if let Some(joint) = timeline.joint(&start, &end) {
        return Err(CommandError::new(format!(
            "зум переходит через стык клипов разных записей в {joint}: раздели его на два зума"
        )));
    }
    let (media, from) = start;
    if end.1 <= from {
        return Err(CommandError::new(
            "длительность зума должна быть больше нуля: конец позже начала",
        ));
    }
    Range::new(from, end.1)
        .map(|range| (media, range))
        .map_err(|e| CommandError::new(e.to_string()))
}
