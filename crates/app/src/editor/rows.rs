//! Rows of the timeline as registry commands (mockup 1, 2e): removing an annotation or extra
//! sound row removes its regions and moves the rows above it one down; dragging an annotation
//! row to another place moves its annotations (the row order is the layer order); the sound
//! button of a recording track mutes it for every clip. No new command: `annot rm`, `annot move
//! --track`, `audio rm`, `audio set --track`, `audio channel --mute` as one undo step.

use dc_core::Project;
use dc_fx_audio_tracks::Audio;
use dc_ops::Step;
use serde_json::{Map, Value};

use super::Action;
use super::lanes;
use super::regions::remove;
use super::selection::Item;
use super::timeline::Row;

/// The commands that remove annotation track `track` and move the tracks above one down.
pub fn remove_annotations(project: &Project, track: u32) -> Vec<Step> {
    retrack(project, |t| match t {
        t if t == track => None,
        t if t > track => Some(t - 1),
        t => Some(t),
    })
}

/// The commands that put annotation track `from` at place `to`; the tracks between move one
/// towards `from`.
pub fn move_annotations(project: &Project, from: u32, to: u32) -> Vec<Step> {
    retrack(project, |t| {
        Some(match t {
            t if t == from => to,
            t if from < to && (from + 1..=to).contains(&t) => t - 1,
            t if to < from && (to..from).contains(&t) => t + 1,
            t => t,
        })
    })
}

/// Each annotation to the track `to` gives (`None` removes it).
fn retrack(project: &Project, to: impl Fn(u32) -> Option<u32>) -> Vec<Step> {
    lanes::annotations::retrack(project, to)
}

/// The commands that remove extra sound track `track` and move the tracks above one down,
/// lowest first (clips of a track do not overlap: the track below is free by then).
pub fn remove_sound(project: &Project, track: u32) -> Vec<Step> {
    let Ok(state) = project.state::<Audio>() else {
        return Vec::new();
    };
    let mut clips: Vec<_> = state.clips.iter().filter(|c| c.track >= track).collect();
    clips.sort_by_key(|c| c.track);
    clips
        .into_iter()
        .filter_map(|c| match c.track == track {
            true => remove(Item::of(c.id)),
            false => Some(step("audio set", c.id.to_string(), c.track - 1)),
        })
        .collect()
}

fn step(op: &str, id: String, track: u32) -> Step {
    let mut args = Map::new();
    args.insert("id".to_owned(), Value::String(id));
    args.insert("track".to_owned(), track.into());
    Step::new(op, args)
}

/// The sound button of a recording track row: mutes its sound in every clip that does not set
/// its own (`audio channel`), or sounds it again.
pub fn mute(row: Row, mute: bool) -> Option<Action> {
    let channel = match row {
        Row::Mic => dc_capture::audio::MIC_TRACK,
        Row::System => dc_capture::audio::SYSTEM_TRACK,
        _ => return None,
    };
    Some(Action::op(
        "audio channel",
        [
            ("channel", Value::from(channel)),
            ("mute", Value::Bool(mute)),
        ],
    ))
}
