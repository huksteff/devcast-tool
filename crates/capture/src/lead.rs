//! How far the time of a captured frame is ahead of the moment it arrives. WGC stamps a frame
//! with the moment it is to be shown; that lead is one or two vsyncs and changes between runs
//! and even within one (decision 007, «Задача 3.9»). Video is put on the host clock at the
//! moment the system composed the frame, like sound at the moment it was mixed: the frame time
//! minus the lead, measured on every frame and smoothed by [`LeadFilter`].

use std::collections::VecDeque;

use crate::event::HostTime;

use serde::{Deserialize, Serialize};

/// Spread of the lead over a recording, microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameLead {
    pub median: i64,
    /// 5th percentile.
    pub low: i64,
    /// 95th percentile.
    pub high: i64,
}

impl FrameLead {
    /// `None` without frames.
    pub fn measure(mut leads: Vec<i64>) -> Option<Self> {
        leads.sort_unstable();
        let last = leads.len().checked_sub(1)?;
        let at = |part: usize| leads.get(last * part / 100).copied();
        Some(Self {
            median: at(50)?,
            low: at(5)?,
            high: at(95)?,
        })
    }
}

/// Frames the lead is smoothed over: a step of the lead is followed after half of them.
const FILTER_FRAMES: usize = 9;

/// The median lead of the last [`FILTER_FRAMES`] frames: arrival times jitter by a
/// millisecond or two, the lead itself only jumps by whole vsyncs.
#[derive(Debug, Clone, Default)]
pub struct LeadFilter {
    recent: VecDeque<i64>,
    sorted: Vec<i64>,
}

impl LeadFilter {
    /// The host time of a frame stamped `shown` that arrived at `arrived`.
    pub fn place(&mut self, shown: HostTime, arrived: HostTime) -> HostTime {
        if self.recent.len() == FILTER_FRAMES {
            self.recent.pop_front();
        }
        self.recent.push_back(shown.micros() - arrived.micros());
        self.sorted.clear();
        self.sorted.extend(&self.recent);
        self.sorted.sort_unstable();
        HostTime::from_micros(shown.micros() - self.sorted[self.sorted.len() / 2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_placed_at_arrival_without_its_jitter() {
        let mut filter = LeadFilter::default();
        let mut placed = Vec::new();
        for frame in 0..30_i64 {
            let shown = 1_000_000 + frame * 16_667;
            // Shown 15 ms after arrival, then a vsync later from frame 15; ±1.5 ms jitter.
            let lead = if frame < 15 { 15_000 } else { 31_667 };
            let jitter = [0, 1_500, -1_500][frame as usize % 3];
            let arrived = HostTime::from_micros(shown - lead + jitter);
            let at = filter.place(HostTime::from_micros(shown), arrived);
            placed.push(at.micros() - (shown - lead));
        }
        assert_eq!(placed[0], 0);
        assert!(placed[1..15].iter().all(|p| *p == 0), "{placed:?}");
        // The step is followed after half the window (and the jitter after a few more).
        assert!(placed[15..19].iter().all(|p| *p > 15_000), "{placed:?}");
        assert!(placed[21..].iter().all(|p| *p == 0), "{placed:?}");
    }

    #[test]
    fn percentiles_of_the_leads() {
        assert_eq!(FrameLead::measure(Vec::new()), None);
        let leads: Vec<i64> = (0..=100).rev().map(|v| v * 10).collect();
        assert_eq!(
            FrameLead::measure(leads),
            Some(FrameLead {
                median: 500,
                low: 50,
                high: 950,
            })
        );
    }
}
