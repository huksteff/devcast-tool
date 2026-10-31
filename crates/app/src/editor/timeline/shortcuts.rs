//! Shortcuts on the «Клавиши» row (7.16, mockup `.rg.keys`): a chip per plate the keys effect
//! shows — from its first press to when it is gone, «Ctrl+Shift+P ×2» when pressed again while
//! on screen. The placing is the effect's own (`dc_fx_keys::Prepared`, as the preview and the
//! export do): a press in a cut drops out, a faster part squeezes. The chips are pale while the
//! plate is off; they are not objects of the project, so they cannot be picked.

use std::collections::BTreeSet;

use dc_core::{KeyTracks, Project, Range, TimeMap};
use dc_fx_keys::{Keys, Prepared};
use dc_ui_kit::strings::timeline as text;

use super::tracks::{Region, Row, Tracks, region};

/// What the recordings hold for the panel «Показ клавиш».
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shortcuts {
    /// Different shortcuts pressed in the recordings, cut parts too.
    pub names: usize,
    /// The plate pressed most often on the output (the first of equals): the sample of the panel.
    pub sample: Option<Sample>,
}

/// A plate of the project: its shortcut and how many presses it counts.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub name: String,
    pub count: usize,
}

impl Tracks {
    /// The chips of the «Клавиши» row and the panel's facts from the shortcuts `keys` of the
    /// recordings of `project` read so far.
    pub fn place_keys(&mut self, project: &Project, keys: &KeyTracks) {
        // A broken keys state is a problem of the rows already: the chips follow the defaults.
        let state = project.state::<Keys>().unwrap_or_default();
        let map = TimeMap::build(project);
        let prepared = Prepared::new(state, keys, &map);
        let names: BTreeSet<&str> = keys.values().flatten().map(|k| k.name.as_str()).collect();
        let mut sample: Option<Sample> = None;
        let mut chips = Vec::new();
        for shown in prepared.shown() {
            let (Some(&first), count) = (shown.presses.first(), shown.presses.len()) else {
                continue;
            };
            if sample.as_ref().is_none_or(|s| count > s.count) {
                sample = Some(Sample {
                    name: shown.name.clone(),
                    count,
                });
            }
            let Ok(range) = Range::new(first, shown.end) else {
                continue;
            };
            chips.push(region(range, text::shortcut(&shown.name, count)));
        }
        self.shortcuts = Shortcuts {
            names: names.len(),
            sample,
        };
        if let Some(track) = self.rows.iter_mut().find(|t| t.row == Row::Keys) {
            let faded = track.off;
            track.regions = chips
                .into_iter()
                .map(|chip| Region { faded, ..chip })
                .collect();
        }
    }
}
