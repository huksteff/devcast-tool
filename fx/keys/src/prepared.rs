//! The shortcuts of a project placed on the output once by `dc_ops::Evaluator::new`, with fast
//! repeats merged.

use dc_core::{KeyTracks, OutputTime, TimeMap};

use crate::state::{KeysSettings, KeysState};

/// One shortcut on screen: pressed once or several times in a row.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub name: String,
    /// Output moments of its presses: the count grows with each (`×2` from the second).
    pub presses: Vec<OutputTime>,
    /// Gone from here on: `duration` after the last press, the next shortcut or the end of video.
    pub end: OutputTime,
    /// Appears from nothing: nothing was on screen just before.
    pub fade_in: bool,
    /// Leaves into nothing: no other shortcut replaces it.
    pub fade_out: bool,
}

/// The keys effect of a project, ready for every frame.
#[derive(Debug)]
pub struct Prepared {
    pub(crate) settings: KeysSettings,
    /// In output order, not overlapping.
    pub(crate) shown: Vec<Shown>,
}

impl Prepared {
    /// Every press that plays on the output (cut ones drop out), in output order: a press of the
    /// shortcut on screen counts into it, any other replaces it at once.
    pub fn new(state: KeysState, tracks: &KeyTracks, map: &TimeMap) -> Self {
        let settings = state.settings;
        let end = OutputTime::ZERO + map.duration();
        let presses = map.segments().iter().flat_map(|segment| {
            tracks
                .get(&segment.media)
                .into_iter()
                .flatten()
                .filter(|press| segment.source.contains(press.at))
                .map(|press| (segment.output_at(press.at), press.name.as_str()))
        });
        let mut shown: Vec<Shown> = Vec::new();
        for (at, name) in presses {
            let gone = (at + settings.duration).min(end);
            if let Some(last) = shown.last_mut().filter(|s| at < s.end) {
                if last.name == name {
                    last.presses.push(at);
                    last.end = gone;
                    continue;
                }
                last.end = at;
                last.fade_out = false;
            }
            let fade_in = shown.last().is_none_or(|s| s.end < at);
            shown.push(Shown {
                name: name.to_owned(),
                presses: vec![at],
                end: gone,
                fade_in,
                fade_out: true,
            });
        }
        Self { settings, shown }
    }

    /// The shortcuts on screen in output order: the plates of the «Клавиши» row of the editor.
    pub fn shown(&self) -> &[Shown] {
        &self.shown
    }
}
