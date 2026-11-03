//! What is selected on the timeline: clips and the regions of the effects, together (mockup 2a:
//! one selection over all rows). The selection is not part of the project: undo does not bring
//! it back, and what the project no longer has drops out of it.

use dc_core::{ClipId, OutputTime};
use dc_fx_annotations::AnnotId;
use dc_fx_annotations::KEY_TOLERANCE;
use dc_fx_audio_tracks::AudioId;
use dc_fx_callout::EventId;
use dc_fx_captions::CueId;
use dc_fx_zoom::ZoomId;

use super::timeline::Tracks;

/// The effect object behind a region of the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Item {
    Zoom(ZoomId),
    Cue(CueId),
    Callout(EventId),
    Annotation(AnnotId),
    Sound(AudioId),
}

impl Item {
    /// The item whose id `devcast` writes as `text` (`s3`); the prefixes of the kinds differ.
    pub fn parse(text: &str) -> Option<Self> {
        text.parse()
            .map(Self::Zoom)
            .or_else(|_| text.parse().map(Self::Cue))
            .or_else(|_| text.parse().map(Self::Callout))
            .or_else(|_| text.parse().map(Self::Annotation))
            .or_else(|_| text.parse().map(Self::Sound))
            .ok()
    }

    /// Items of one kind lie on the same rows: Ctrl+A picks them all.
    pub fn same_kind(self, other: Self) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }
}

/// An object picked on the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selected {
    Clip(ClipId),
    Region(Item),
    /// A press of a recording on the «Курсор» row at its output time (7.15).
    Click(OutputTime),
    /// A key of an annotation at its output time (7.12).
    Key {
        id: AnnotId,
        at: OutputTime,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    items: Vec<Selected>,
}

impl Selection {
    /// Just `item` (a click).
    pub fn only(&mut self, item: Selected) {
        self.items = vec![item];
    }

    /// `item` too, unless it is picked already.
    pub fn add(&mut self, item: Selected) {
        if !self.contains(item) {
            self.items.push(item);
        }
    }

    /// Ctrl+click: adds `item` or takes it out.
    pub fn toggle(&mut self, item: Selected) {
        match self.items.iter().position(|i| *i == item) {
            Some(at) => {
                self.items.remove(at);
            }
            None => self.items.push(item),
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn contains(&self, item: Selected) -> bool {
        self.items.contains(&item)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// The picked object when exactly one is picked: the inspector opens its panel.
    pub fn single(&self) -> Option<Selected> {
        match self.items.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    /// The selected clips in the order they were picked.
    pub fn clips(&self) -> impl Iterator<Item = ClipId> + '_ {
        self.items.iter().filter_map(|item| match item {
            Selected::Clip(id) => Some(*id),
            Selected::Region(_) | Selected::Key { .. } | Selected::Click(_) => None,
        })
    }

    /// The selected regions in the order they were picked.
    pub fn regions(&self) -> impl Iterator<Item = Item> + '_ {
        self.items.iter().filter_map(|item| match item {
            Selected::Region(item) => Some(*item),
            Selected::Clip(_) | Selected::Key { .. } | Selected::Click(_) => None,
        })
    }

    /// The selected keys: annotation and output time.
    pub fn keys(&self) -> impl Iterator<Item = (AnnotId, OutputTime)> + '_ {
        self.items.iter().filter_map(|item| match item {
            Selected::Key { id, at } => Some((*id, *at)),
            _ => None,
        })
    }

    /// Drops what the rows of the project no longer show (a removed clip or zoom, an undone
    /// split).
    pub fn keep(&mut self, tracks: &Tracks) {
        let regions = || tracks.rows.iter().flat_map(|t| &t.regions);
        self.items.retain(|item| match item {
            Selected::Clip(id) => regions().any(|r| r.clip == Some(*id)),
            Selected::Region(item) => regions().any(|r| r.item == Some(*item)),
            Selected::Key { id, at } => regions().any(|r| {
                r.item == Some(Item::Annotation(*id)) && r.keys.iter().any(|k| same_moment(*k, *at))
            }),
            Selected::Click(at) => tracks.clicks.iter().any(|d| same_moment(d.at, *at)),
        });
    }
}

/// Two output times of one key: closer than `KEY_TOLERANCE`.
pub fn same_moment(a: OutputTime, b: OutputTime) -> bool {
    a.since(b).micros().abs() < KEY_TOLERANCE.micros()
}
