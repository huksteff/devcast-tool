//! The panel of the object picked on the timeline: a zoom, a cue, a callout event, an annotation
//! (also through one of its keys), an extra sound, a clip or a click (its own panel, `click.rs`).
//! The frame gives each its header —
//! the colour of its row, its name, where it is — and «Удалить» (the same command as Delete);
//! a zoom gets its parameters (`zoom_region.rs`), a callout event its own (`callout.rs`), a cue
//! its text and times (`cue.rs`), the
//! others name the task that brings theirs.
//! An annotation's panel puts its keyframe strip under the header (7.18, 7.25).

use dc_core::Range;
use dc_core::time::Output;
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::{editor as text, inspector as names, timeline as rows_text};

use super::{Cx, Header, callout, click, cue, header, header_button, padded, rows, zoom_region};
use crate::editor::selection::{Item, Selected, same_moment};
use crate::editor::timeline::{self, Region, Row, Tracks, timecode};
use crate::editor::{Action, montage, regions};
use eframe::egui::Ui;

pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>, tracks: &Tracks, picked: Selected) {
    let picked = match picked {
        Selected::Key { id, .. } => Selected::Region(Item::Annotation(id)),
        Selected::Click(at) => {
            let dot = tracks.clicks.iter().find(|d| same_moment(d.at, at));
            if let Some(dot) = dot {
                click::ui(ui, cx, dot);
            }
            return;
        }
        other => other,
    };
    // The selection keeps only what the rows show, so the object is there.
    let Some((row, region)) = find(tracks, picked) else {
        return;
    };
    let mut about = describe(picked, row, region);
    if let Selected::Region(Item::Callout(id)) = picked
        && let Some((title, subtitle)) = callout::about(cx, id, &timecode(region.range.start()))
    {
        about.title = title.to_owned();
        about.subtitle = subtitle;
    }
    let head = Header {
        hue: timeline::hue(row),
        title: &about.title,
        subtitle: &about.subtitle,
    };
    header(ui, &head, |ui| {
        if header_button(ui, Icon::Trash, names::DELETE, true)
            && let Some(action) = delete(picked)
        {
            cx.actions.push(action);
        }
    });
    match picked {
        Selected::Region(Item::Zoom(id)) => zoom_region::ui(ui, cx, id),
        Selected::Region(Item::Callout(id)) => callout::ui(ui, cx, id),
        Selected::Region(Item::Cue(id)) => cue_panel(ui, cx, id, region.range),
        _ => padded(ui, |ui| rows::note(ui, &names::later(about.task))),
    }
}

/// The row and the region of `picked`.
fn find(tracks: &Tracks, picked: Selected) -> Option<(Row, &Region)> {
    tracks.rows.iter().find_map(|track| {
        let region = track.regions.iter().find(|r| match picked {
            Selected::Clip(id) => r.clip == Some(id),
            Selected::Region(item) => r.item == Some(item),
            Selected::Key { .. } | Selected::Click(_) => false,
        })?;
        Some((track.row, region))
    })
}

/// What the header says of an object and the task of its parameters.
struct About {
    title: String,
    subtitle: String,
    task: &'static str,
}

fn describe(picked: Selected, row: Row, region: &Region) -> About {
    let span = span(region.range);
    let length = rows_text::seconds(region.range.duration().as_secs_f64());
    let timed = |title: &str, task| About {
        title: title.to_owned(),
        subtitle: names::with_length(&span, &length),
        task,
    };
    let layered = |task| About {
        title: region.label.clone(),
        subtitle: names::on_row(&timeline::name(row), &span),
        task,
    };
    match picked {
        Selected::Clip(_) => About {
            title: region.label.clone(),
            subtitle: region.detail.clone().unwrap_or(length),
            task: "7.19",
        },
        Selected::Region(Item::Zoom(_)) => timed(text::ZOOM, "7.14"),
        Selected::Region(Item::Callout(_)) => timed(names::CALLOUT, ""),
        Selected::Region(Item::Cue(_)) => timed(names::CUE, ""),
        Selected::Region(Item::Annotation(_)) | Selected::Key { .. } => layered("7.18"),
        Selected::Click(_) => timed(names::CLICK, "7.15"),
        Selected::Region(Item::Sound(_)) => layered("7.19"),
    }
}

/// «0:41.00 – 0:48.00».
fn span(range: Range<Output>) -> String {
    rows_text::span(&timecode(range.start()), &timecode(range.end()))
}

/// The command that removes `picked`, as Delete does.
fn delete(picked: Selected) -> Option<Action> {
    match picked {
        Selected::Clip(id) => montage::remove([id]),
        Selected::Region(item) => Some(Action::Apply(vec![regions::remove(item)])),
        Selected::Key { .. } | Selected::Click(_) => None,
    }
}

/// The panel of a cue picked on the timeline: its text and times, split, merge (`cue.rs`).
fn cue_panel(ui: &mut Ui, cx: &mut Cx<'_>, id: dc_fx_captions::CueId, at: Range<Output>) {
    let Ok(state) = cx.project.state::<dc_fx_captions::CaptionsEffect>() else {
        return;
    };
    let Some(found) = state.cues.iter().find(|c| c.id == id) else {
        return;
    };
    padded(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        cue::editor(ui, cx, (&state, found), at, false);
        rows::note(ui, dc_ui_kit::strings::captions::CUE_NOTE);
    });
}
