//! Which zoom leads the camera at an output moment (docs/porting/zoom.md §4.3): the strongest
//! one, the later of equals. Close zooms connected by the `connect` setting hand over at full
//! strength instead of zooming out and in again. Seconds of the output video.

use dc_core::TimeMap;

use crate::model::{Mode, Zoom, ZoomState};
use crate::ramp::Ramp;

/// Zooms at most this far apart on the output are connected (v1 `CHAINED_ZOOM_PAN_GAP_MS`);
/// overlapping ones too.
pub const CONNECT_GAP: f64 = 1.35;
/// A connected zoom hands over to the next one this long after its end (v1).
const HANDOVER: f64 = 0.2;

/// A zoom on the output: its strength curve and the zoom it pulls towards.
pub(crate) struct Pull<'z> {
    pub ramp: Ramp,
    pub start: f64,
    end: f64,
    pub zoom: &'z Zoom,
    /// Connected to the next zoom: it hands over at this moment and does not zoom out before.
    gives: Option<f64>,
    /// Connected to the previous zoom: it takes over at this moment, already zoomed in.
    takes: Option<f64>,
    /// The first zoom of the run of connected follow zooms this one belongs to: they share
    /// one followed focus.
    pub chain: usize,
}

impl Pull<'_> {
    fn strength(&self, t: f64) -> f64 {
        if let Some(handover) = self.gives {
            if t >= handover {
                return 0.0;
            }
            if t >= self.ramp.exit_start() {
                return self.ramp.peak();
            }
        }
        if let Some(handover) = self.takes {
            if t < handover {
                return 0.0;
            }
            if t < self.ramp.exit_start() {
                return self.ramp.peak();
            }
        }
        self.ramp.strength(t)
    }

    /// From the first moment it pulls the camera to the moment it lets go.
    fn window(&self) -> (f64, f64) {
        let (from, to) = self.ramp.window();
        (from, self.gives.unwrap_or(to))
    }
}

/// The zooms shown on the output, in the order they start.
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
                    end,
                    zoom,
                    gives: None,
                    takes: None,
                    chain: 0,
                })
            })
            .collect();
        list.sort_by(|a, b| a.start.total_cmp(&b.start));
        for i in 0..list.len() {
            list[i].chain = i;
            if !state.settings.connect || i == 0 || list[i].start - list[i - 1].end > CONNECT_GAP {
                continue;
            }
            let handover = list[i - 1].end + HANDOVER;
            list[i - 1].gives = Some(handover);
            list[i].takes = Some(handover);
            if list[i - 1].zoom.mode == Mode::Follow && list[i].zoom.mode == Mode::Follow {
                list[i].chain = list[i - 1].chain;
            }
        }
        Self { list, done: 0 }
    }

    pub(crate) fn get(&self, index: usize) -> &Pull<'z> {
        &self.list[index]
    }

    /// The zoom leading at `t` (its index) and its strength; `t` never goes back between calls.
    pub(crate) fn lead(&mut self, t: f64) -> Option<(usize, f64)> {
        while self.list.get(self.done).is_some_and(|p| p.window().1 <= t) {
            self.done += 1;
        }
        self.list
            .iter()
            .enumerate()
            .skip(self.done)
            .take_while(|(_, p)| p.window().0 <= t)
            .map(|(i, p)| (i, p.strength(t)))
            .filter(|(_, strength)| *strength > 0.0)
            .max_by(|(i, a), (j, b)| {
                a.total_cmp(b)
                    .then(self.list[*i].start.total_cmp(&self.list[*j].start))
            })
    }
}
