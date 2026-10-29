//! Clicks of the recording playing, by their age on the output timeline: animations of a click
//! run at normal speed on sped-up parts (decision 000, «Ускоренные участки»).

use dc_core::{CursorPoint, OutputTime, TimeDelta, TimeMap};

use crate::prepared::Place;

/// Clicks of `place`'s recording not later than its moment and shown on the output at most
/// `window` before `t`, newest first, with their age. Cut-out clicks are skipped.
pub(crate) fn recent<'a>(
    place: &Place<'a>,
    map: &'a TimeMap,
    t: OutputTime,
    window: TimeDelta,
) -> impl Iterator<Item = (&'a CursorPoint, TimeDelta)> + 'a {
    let clicks = &place.recording.clicks;
    let media = place.media;
    let shown = clicks.partition_point(|c| c.at <= place.source);
    clicks[..shown]
        .iter()
        .rev()
        .filter_map(move |click| {
            let (_, at) = map.locate(media, click.at)?;
            Some((click, t.since(at)))
        })
        .filter(|(_, age)| *age >= TimeDelta::ZERO)
        .take_while(move |(_, age)| *age <= window)
}
