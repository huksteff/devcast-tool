//! Output time → the clips and source ranges it shows, through [`TimeMap`] only.

use dc_core::time::{Output, Source, Time, Timeline};
use dc_core::{ClipId, CommandError, OutputTime, Project, Range, SourceTime, TimeMap};

/// The source material shown in `range` of the output: one range per clip, in track order. A range
/// spans the clip's old cuts inside it, so cutting or speeding it up leaves no slivers between them.
pub fn source_spans(
    project: &Project,
    range: Range<Output>,
) -> Result<Vec<(ClipId, Range<Source>)>, CommandError> {
    let map = TimeMap::build(project);
    let video_end = OutputTime::ZERO + map.duration();
    if range.is_empty() {
        return Err(CommandError::new(format!(
            "пустой диапазон {}: конец должен быть позже начала",
            range.start()
        )));
    }
    if range.start() < OutputTime::ZERO || range.end() > video_end {
        return Err(CommandError::new(format!(
            "диапазон {}–{} выходит за видео 0:00.000–{video_end}; длина видео: devcast info",
            range.start(),
            range.end()
        )));
    }
    let mut spans: Vec<(ClipId, Range<Source>)> = Vec::new();
    let shown = map
        .segments()
        .iter()
        .filter(|s| s.output_start < range.end() && range.start() < s.output_end());
    for segment in shown {
        let start = if range.start() > segment.output_start {
            source_at(&map, range.start())?
        } else {
            segment.source.start()
        };
        let end = if range.end() < segment.output_end() {
            source_at(&map, range.end())?
        } else {
            segment.source.end()
        };
        match spans.last_mut() {
            Some((clip, span)) if *clip == segment.clip => *span = range_of(span.start(), end)?,
            _ => spans.push((segment.clip, range_of(start, end)?)),
        }
    }
    spans.retain(|(_, span)| !span.is_empty());
    Ok(spans)
}

fn source_at(map: &TimeMap, t: OutputTime) -> Result<SourceTime, CommandError> {
    map.to_source(t)
        .map(|(_, source)| source)
        .ok_or_else(|| CommandError::new(format!("внутренняя ошибка: {t} не найден на дорожке")))
}

pub fn range_of<K: Timeline>(start: Time<K>, end: Time<K>) -> Result<Range<K>, CommandError> {
    Range::new(start, end).map_err(|e| CommandError::new(e.to_string()))
}
