//! What is selected on the timeline. The selection is not part of the project: undo does not
//! bring it back (mockup 2a), and what the project no longer has drops out of it. Clips are the
//! only objects for now; the regions of the effects and multi-select join in 7.9.

use dc_core::{ClipId, Project};

/// An object picked on the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selected {
    Clip(ClipId),
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

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn contains(&self, item: Selected) -> bool {
        self.items.contains(&item)
    }

    /// The selected clips in the order they were picked.
    pub fn clips(&self) -> impl Iterator<Item = ClipId> + '_ {
        self.items.iter().map(|item| match item {
            Selected::Clip(id) => *id,
        })
    }

    /// Drops what `project` does not have (a removed clip, an undone split).
    pub fn keep(&mut self, project: &Project) {
        self.items.retain(|item| match item {
            Selected::Clip(id) => project.clips.iter().any(|c| c.id == *id),
        });
    }
}
