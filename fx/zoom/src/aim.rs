//! Which zoom leads the camera at an output moment (docs/porting/zoom.md §4.3): the strongest
//! one, the later of equals. Seconds of the output video.

use dc_core::TimeMap;

use crate::model::{Zoom, ZoomState};
use crate::ramp::Ramp;

/// A zoom on the output: its strength curve and the zoom it pulls towards.
pub(crate) struct Pull<'z> {
    pub ramp: Ramp,
    pub start: f64,
    pub zoom: &'z Zoom,
}

/// The zooms shown on the output, in the order their pull starts.
pub(crate) struct Pulls<'z> {
    list: Vec<Pull<'z>>,
    /// Zooms before this one have let go already.
    done: usize,
}

impl<'z> Pulls<'z> {
    /// The zooms of `state` placed by `map`; zoom-outs are over by `limit`. A zoom cut out
    /// entirely is left out; a zoom whose shown parts are apart plays over all of them.
    pub(crate) fn new(state: &'z ZoomState, map: &TimeMap, limit: f64) -> Self {
        let (enter, exit) = (
            state.settings.zoom_in.as_secs_f64(),
            state.settings.zoom_out.as_secs_f64(),
        );
        let mut list: Vec<Pull<'z>> = state
            .zooms
            .iter()
            .filter_map(|zoom| {
                let span = map.output_span(&zoom.media, zoom.range)?;
                let (start, end) = (span.start().as_secs_f64(), span.end().as_secs_f64());
                Some(Pull {
                    ramp: Ramp::new(start, end, enter, exit, limit),
                    start,
                    zoom,
                })
            })
            .collect();
        list.sort_by(|a, b| a.ramp.window().0.total_cmp(&b.ramp.window().0));
        Self { list, done: 0 }
    }

    pub(crate) fn get(&self, index: usize) -> &Pull<'z> {
        &self.list[index]
    }

    /// The zoom leading at `t` (its index) and its strength; `t` never goes back between calls.
    pub(crate) fn lead(&mut self, t: f64) -> Option<(usize, f64)> {
        while self
            .list
            .get(self.done)
            .is_some_and(|p| p.ramp.window().1 <= t)
        {
            self.done += 1;
        }
        self.list
            .iter()
            .enumerate()
            .skip(self.done)
            .take_while(|(_, p)| p.ramp.window().0 <= t)
            .map(|(i, p)| (i, p.ramp.strength(t)))
            .filter(|(_, strength)| *strength > 0.0)
            .max_by(|(i, a), (j, b)| {
                a.total_cmp(b)
                    .then(self.list[*i].start.total_cmp(&self.list[*j].start))
            })
    }
}
