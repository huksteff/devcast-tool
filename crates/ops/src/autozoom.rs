//! Auto zooms from the clicks of the recordings (`dc_fx_zoom::suggest`), one path for
//! `devcast zoom suggest`, `devcast zoom add --at clicks`, the zooms after a recording and the
//! editor's «Подобрать зумы». The suggestion is not a registry command: a command is a pure
//! function of the project, and the clicks lie in the recordings' telemetry files, which the
//! caller's project folder gives.

use std::path::Path;

use dc_capture::input::{Button, CursorEvent};
use dc_capture::project::{Recorded, fraction, recorded};
use dc_core::Project;
use dc_fx_zoom::{Action, Mode, Pointer, Recording, Suggestions, ZoomAdd};

use crate::error::OpsError;

/// The value of `zoom add --at` that places the suggested zooms.
pub const CLICKS: &str = "clicks";

/// The zooms the clicks of the recordings of the project at `root` suggest.
pub fn click_zooms(root: &Path, project: &Project) -> Result<Suggestions, OpsError> {
    let recorded = recorded(root, project).map_err(|e| OpsError::Failed(e.to_string()))?;
    let recordings: Vec<Recording> = recorded.iter().map(pointer).collect();
    dc_fx_zoom::suggest(project, &recordings).map_err(|e| OpsError::Failed(e.to_string()))
}

/// The `zoom add` of every suggestion with `scale` and `mode`; applied as one undo step named
/// [`ZoomAdd`]'s name.
pub fn click_commands(found: &Suggestions, scale: f64, mode: Mode) -> Vec<ZoomAdd> {
    found.zooms.iter().map(|z| z.command(scale, mode)).collect()
}

/// Why there is nothing to suggest, or `None` if there is something.
pub fn no_click_zooms(found: &Suggestions) -> Option<String> {
    if !found.zooms.is_empty() {
        return None;
    }
    Some(match (found.clicks, found.taken) {
        (0, _) if found.hidden > 0 => {
            format!(
                "автозумов нет: все клики ({}) вырезаны или вне кадра",
                found.hidden
            )
        }
        (0, _) => "автозумов нет: в записях нет кликов".to_owned(),
        _ => format!(
            "автозумов нет: у всех групп кликов ({}) уже есть зумы",
            found.taken
        ),
    })
}

/// The moves, releases and left, right or middle presses of one recording, in time order.
fn pointer(r: &Recorded<'_>) -> Recording {
    let mut pointer: Vec<Pointer> = r
        .telemetry
        .cursor
        .iter()
        .filter_map(|e| {
            let (action, (x, y)) = match e.event {
                CursorEvent::Move { x, y } => (Action::Move, (x, y)),
                CursorEvent::Up { x, y, .. } => (Action::Release, (x, y)),
                _ => return None,
            };
            Some(Pointer {
                at: e.at,
                point: fraction(r.media, x, y),
                action,
            })
        })
        .collect();
    let presses = r
        .telemetry
        .clicks()
        .into_iter()
        .filter(|c| matches!(c.button, Button::Left | Button::Right | Button::Middle));
    pointer.extend(presses.map(|c| Pointer {
        at: c.at,
        point: fraction(r.media, c.x, c.y),
        action: Action::Press { double: c.double },
    }));
    pointer.sort_by_key(|p| p.at);
    Recording {
        media: r.media.path.clone(),
        pointer,
    }
}
