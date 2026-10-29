//! What the callout says at an output moment: the events of the recording on screen replayed in
//! order (docs/porting/clicks-callout.md §12). Nothing is carried between frames, so the preview,
//! the export and any fps agree.

use dc_core::{OutputTime, Range, SourceTime, TimeDelta, TimeMap};
use unicode_segmentation::UnicodeSegmentation;

use crate::model::{Action, CalloutEvent, CalloutState};

/// The typed part of the callout text at output moment `t`; `None` while the callout is off,
/// empty, or `t` is outside the video.
pub fn text_at(state: &CalloutState, map: &TimeMap, t: OutputTime) -> Option<String> {
    let segment = map.segment_at(t)?;
    let now = segment.source_at(t);
    let mut events: Vec<&CalloutEvent> = state
        .events
        .iter()
        .filter(|e| e.media == segment.media && e.at <= now)
        .collect();
    events.sort_by_key(|e| (e.at, e.id));
    let mut on = false;
    let mut text = "";
    for event in events {
        let q = progress(map, event, now, t);
        match &event.action {
            Action::On => {
                on = true;
                text = &state.settings.text;
            }
            Action::Text { text: target } if on && q > 0.0 => {
                let typed = (count(target) as f64 * q).round() as usize;
                text = head(target, typed.max(1));
            }
            Action::Off if on => {
                text = head(text, (count(text) as f64 * (1.0 - q)).round() as usize);
                if q >= 1.0 {
                    on = false;
                }
            }
            Action::Text { .. } | Action::Off => {}
        }
    }
    (on && !text.is_empty()).then(|| text.to_owned())
}

/// Part of `event` done at output moment `t` (source `now` of its recording), 0..1. Time runs on
/// the output from the first shown moment after its start, so cuts do not skip typing and a
/// sped-up part does not hurry it.
fn progress(map: &TimeMap, event: &CalloutEvent, now: SourceTime, t: OutputTime) -> f64 {
    let Some(shown) = Range::new(event.at, now)
        .ok()
        .and_then(|r| map.output_span(&event.media, r))
    else {
        return 0.0;
    };
    let elapsed = t.since(shown.start()).max(TimeDelta::ZERO);
    (elapsed.as_secs_f64() / event.duration.as_secs_f64()).min(1.0)
}

fn count(text: &str) -> usize {
    text.graphemes(true).count()
}

/// The first `n` letters (grapheme clusters, so an emoji is never cut in half) of `text`.
fn head(text: &str, n: usize) -> &str {
    text.grapheme_indices(true)
        .nth(n)
        .map_or(text, |(end, _)| &text[..end])
}
