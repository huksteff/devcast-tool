//! The extra sound on the timeline (`dc_ui_kit::lane`): a moved sound is `audio set --at` (with
//! `--track` when it changes row), a dragged edge changes its `--duration` and keeps the sound
//! under its end (a moved start moves the offset into the file too), a copy is `audio paste`
//! with its JSON. «Дублировать» leaves the copy where it is, on the first row free there (sounds
//! of a row do not overlap).

use dc_core::time::Output;
use dc_core::{IdKind, Project, Range, Step, TimeDelta};
use dc_ui_kit::lane::{Item, Lane, Moved, Resize, id_args, seconds, time};

use crate::{Audio, AudioKind};

pub const LANE: Lane = Lane {
    prefix: AudioKind::PREFIX,
    op: "audio",
    moved,
    resized,
    copied,
    duplicated,
};

fn moved(moved: &Moved) -> Step {
    let mut args = id_args(moved.item);
    args.insert("at".to_owned(), time(moved.range.start()));
    if let Some(track) = moved.track {
        args.insert("track".to_owned(), track.into());
    }
    Step::new("audio set", args)
}

fn resized(project: &Project, resize: &Resize) -> Step {
    let mut args = id_args(resize.item);
    let (range, at) = (resize.range, resize.at);
    if resize.end {
        args.insert("duration".to_owned(), seconds(at.since(range.start())));
        return Step::new("audio set", args);
    }
    args.insert("at".to_owned(), time(at));
    args.insert("duration".to_owned(), seconds(range.end().since(at)));
    let offset = project
        .state::<Audio>()
        .ok()
        .zip(resize.item.id::<AudioKind>())
        .and_then(|(s, id)| s.clips.into_iter().find(|c| c.id == id))
        .map(|c| TimeDelta::from_micros(c.offset.micros() + at.since(range.start()).micros()));
    if let Some(offset) = offset {
        args.insert("offset".to_owned(), seconds(offset));
    }
    Step::new("audio set", args)
}

fn copied(project: &Project, item: Item, _: TimeDelta) -> Option<Step> {
    let id = item.id::<AudioKind>()?;
    let state = project.state::<Audio>().ok()?;
    let clip = state.clips.into_iter().find(|c| c.id == id)?;
    let mut args = serde_json::Map::new();
    args.insert("item".to_owned(), serde_json::to_value(&clip).ok()?);
    Some(Step::new("audio paste", args))
}

fn duplicated(project: &Project, item: Item, range: Range<Output>) -> Option<Step> {
    let mut step = copied(project, item, range.duration())?;
    let track = project.state::<Audio>().ok()?.free_track(range)?;
    step.args.insert("at".to_owned(), time(range.start()));
    step.args.insert("track".to_owned(), track.into());
    Some(step)
}
