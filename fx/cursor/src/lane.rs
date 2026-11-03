//! The presses on the «Курсор» row (7.15, `dc_ui_kit::lane`): a dot at the output time of every
//! press of the recordings the editor read; a press in a cut or past a trimmed edge does not
//! show. A click whose own change turns its effect off is a hidden dot. A dot is no object of
//! the project: its panel works on the press (`panel::click`).

use dc_core::{Project, SourceTime, TimeMap};

use crate::panel::{Press, PressButton};
use crate::{ClickKind, Cursor};

/// A press of a recording on its own time, as the editor read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pressed {
    pub at: SourceTime,
    pub button: PressButton,
    /// The second press of a double click.
    pub double: bool,
}

/// A press as it shows on the output.
#[derive(Debug, Clone, PartialEq)]
pub struct Dot {
    pub press: Press,
    /// Its own change turns the effect of this click off.
    pub hidden: bool,
}

/// The dots of the presses of each recording (`Media.path` and its presses), in time order.
pub fn dots(project: &Project, recordings: Vec<(String, Vec<Pressed>)>) -> Vec<Dot> {
    let map = TimeMap::build(project);
    // A broken cursor state is a problem of the rows already: the dots show as plain.
    let changes = project
        .state::<Cursor>()
        .map(|s| s.clicks)
        .unwrap_or_default();
    let mut dots = Vec::new();
    for (media, presses) in recordings {
        for pressed in presses {
            let Some((_, at)) = map.locate(&media, pressed.at) else {
                continue;
            };
            let change = changes.iter().find(|c| c.is_for(&media, pressed.at));
            dots.push(Dot {
                press: Press {
                    at,
                    media: media.clone(),
                    source: pressed.at,
                    button: pressed.button,
                    double: pressed.double,
                },
                hidden: change.is_some_and(|c| c.kind == Some(ClickKind::Off)),
            });
        }
    }
    dots.sort_by_key(|d| d.press.at);
    dots
}
