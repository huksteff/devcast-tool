//! The «Клавиши» row (7.16, `dc_ui_kit::lane`): a chip per plate the keys effect shows — from its
//! first press to when it is gone, «Ctrl+Shift+P ×2» when pressed again while on screen. The
//! placing is the effect's own (`Prepared`, as the preview and the export do): a press in a cut
//! drops out, a faster part squeezes. The chips are pale while the plate is off; they are not
//! objects of the project, so they cannot be picked.

use dc_core::{KeyTracks, Project, Range, StateError, TimeMap};
use dc_ui_kit::lane::Region;
use dc_ui_kit::strings::timeline as text;

pub use crate::panel::Shortcuts;
use crate::{Keys, Prepared};

/// The plates show (the row is not off).
pub fn shown(project: &Project) -> Result<bool, StateError> {
    Ok(project.state::<Keys>()?.settings.show)
}

/// The chips of the row and the panel's facts from the shortcuts `keys` of the recordings of
/// `project` read so far.
pub fn chips(project: &Project, keys: &KeyTracks) -> (Vec<Region>, Shortcuts) {
    // A broken keys state is a problem of the rows already: the chips follow the defaults.
    let state = project.state::<Keys>().unwrap_or_default();
    let faded = !state.settings.show;
    let map = TimeMap::build(project);
    let prepared = Prepared::new(state, keys, &map);
    let mut chips = Vec::new();
    for shown in prepared.shown() {
        let Some(&first) = shown.presses.first() else {
            continue;
        };
        let Ok(range) = Range::new(first, shown.end) else {
            continue;
        };
        let label = text::shortcut(&shown.name, shown.presses.len());
        chips.push(Region {
            faded,
            ..Region::new(range, label)
        });
    }
    (chips, Shortcuts::of(&prepared, keys))
}
