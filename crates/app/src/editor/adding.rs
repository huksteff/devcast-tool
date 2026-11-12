//! Adding regions (mockup 2e, «призрак»): a hovered empty spot of a row (an annotation of the
//! kind picked from its menu, `timeline/kinds.rs`), Z and T at the playhead, a sound file on an
//! extra sound row; a picture file into an image annotation or a new one. A new region has the
//! length v1 gives (`LENGTH`), fits the room it is in (zooms and extra sound do not overlap their
//! neighbours, zooms, annotations and callout events lie in one clip) and is added by the
//! effect's own command, then picked. Tab picks the next annotation under the playhead in the
//! order they are painted.

use std::path::PathBuf;
use std::time::Instant;

use dc_core::time::Output;
use dc_core::{OutputTime, Range, TimeDelta};
use dc_ops::{SoundPlace, Step};

use super::lanes::{
    self,
    annotations::{AnnotId, Kind, TRACKS},
};
use super::selection::{Item, Selected};
use super::timeline::{Adds, Row, Tracks};
use super::{Action, EditorWindow};

/// Length of a new zoom, annotation and callout event (v1: zoom 1 s, annotation 1 s).
pub const LENGTH: TimeDelta = TimeDelta::from_micros(1_000_000);
/// The least room a region is added to (the commands' least length).
const LEAST: TimeDelta = TimeDelta::from_micros(100_000);

/// A sound file to put on extra sound row `track` at `at`, not longer than `room`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundFile {
    pub path: PathBuf,
    pub at: OutputTime,
    pub track: u32,
    pub room: TimeDelta,
}

/// A picture file for image annotation `id`: in the key at `at` of the output, or everywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PictureFile {
    pub id: AnnotId,
    pub at: Option<OutputTime>,
    pub path: PathBuf,
}

/// Regions can be added to `row` by hovering it.
pub fn addable(row: Row) -> bool {
    !matches!(row.kind.adds, Adds::Nothing)
}

/// Where a region added on `row` at `at` lies; `None` when there is no room.
pub fn slot(tracks: &Tracks, row: Row, at: OutputTime) -> Option<Range<Output>> {
    let end = OutputTime::ZERO + tracks.duration;
    let (mut low, mut high) = (OutputTime::ZERO, end);
    if row.kind.in_clip {
        let clip = regions(tracks, Row::CLIPS).find(|r| r.contains(at))?;
        (low, high) = (clip.start(), clip.end());
    }
    if row.kind.apart {
        if regions(tracks, row).any(|r| r.contains(at)) {
            return None;
        }
        low = regions(tracks, row)
            .map(|r| r.end())
            .filter(|e| *e <= at)
            .fold(low, OutputTime::max);
        high = regions(tracks, row)
            .map(|r| r.start())
            .filter(|s| *s > at)
            .fold(high, OutputTime::min);
    }
    if !(low..end).contains(&at) || high.since(low) < LEAST {
        return None;
    }
    let start = at.min(high - LENGTH).max(low);
    Range::new(start, (start + LENGTH).min(high)).ok()
}

fn regions(tracks: &Tracks, row: Row) -> impl Iterator<Item = Range<Output>> + '_ {
    tracks
        .rows
        .iter()
        .filter(move |t| t.row == row)
        .flat_map(|t| &t.regions)
        .map(|r| r.range)
}

/// The command that adds a region at `range` of `row` (a text annotation, a callout «on»); the
/// ghost of an annotation row asks for the kind first (`timeline/kinds.rs`).
pub fn step(row: Row, range: Range<Output>) -> Option<Step> {
    match row.kind.adds {
        Adds::Step(added) => Some(added(range, row.track)),
        Adds::Annotation => Some(lanes::annotations::added(range, row.track, Kind::Text)),
        Adds::Sound | Adds::Nothing => None,
    }
}

/// Z: a zoom at the playhead; where there is no room, the command says why.
pub fn zoom(editor: &mut EditorWindow) {
    let at = editor.view.playhead;
    let range = slot(&editor.tracks, Row::ZOOMS, at).or_else(|| Range::new(at, at + LENGTH).ok());
    if let Some(step) = range.and_then(|r| step(Row::ZOOMS, r)) {
        editor.ask(Action::Paste(vec![step]));
    }
}

/// T: a text annotation at the playhead on the lowest row free there, a new row when none is.
pub fn annotation(editor: &mut EditorWindow) {
    let at = editor.view.playhead;
    let range =
        slot(&editor.tracks, Row::annotations(0), at).or_else(|| Range::new(at, at + LENGTH).ok());
    let Some(range) = range else { return };
    let rows: Vec<u32> = editor
        .tracks
        .rows
        .iter()
        .filter_map(|t| t.row.annotation_track())
        .collect();
    let busy = |track: u32| {
        regions(&editor.tracks, Row::annotations(track)).any(|r| r.intersect(&range).is_some())
    };
    let new = (rows.len() as u32).min(TRACKS - 1);
    let track = rows.iter().copied().find(|t| !busy(*t)).unwrap_or(new);
    if let Some(step) = step(Row::annotations(track), range) {
        editor.ask(Action::Paste(vec![step]));
    }
}

/// Tab: the annotation under the playhead after the picked one, in the order they are painted
/// (track by track from the bottom, v1); the first when none of them is picked.
pub fn next_annotation(editor: &mut EditorWindow) {
    let at = editor.view.playhead;
    let shown: Vec<Item> = editor
        .tracks
        .rows
        .iter()
        .filter(|t| t.row.annotation_track().is_some())
        .flat_map(|t| &t.regions)
        .filter(|r| r.range.contains(at))
        .filter_map(|r| r.item)
        .collect();
    let picked = shown
        .iter()
        .position(|i| editor.view.selection.contains(Selected::Region(*i)));
    let next = picked.map_or(0, |i| (i + 1) % shown.len().max(1));
    if let Some(item) = shown.get(next) {
        editor.view.selection.only(Selected::Region(*item));
    }
}

/// Copies the sound `file` into the project and places it, as one undo step; a refused
/// command deletes the copy. The message of what failed.
pub fn sound(editor: &mut EditorWindow, file: &SoundFile, now: Instant) -> Result<(), String> {
    let place = SoundPlace {
        at: file.at,
        track: Some(file.track),
        duration: None,
    };
    let mut imported = dc_ops::import_sound(editor.doc.dir(), &file.path, place, false)
        .map_err(|e| e.to_string())?;
    let command = &mut imported.command;
    command.duration = Some(command.file.duration.min(file.room));
    match editor
        .doc
        .apply_commands(dc_ops::SoundImport::NAME, &[&imported.command], now)
    {
        Ok(_) => Ok(()),
        Err(error) => Err(match imported.discard() {
            Ok(()) => error.to_string(),
            Err(e) => format!("{error}; {e}"),
        }),
    }
}

/// Copies the picture `file` into the project and shows it in its annotation, as one undo step
/// (`devcast image set`); a refused command deletes the copy. The message of what failed.
pub fn picture(editor: &mut EditorWindow, file: &PictureFile, now: Instant) -> Result<(), String> {
    let imported = dc_ops::import_picture(editor.doc.dir(), &file.path, (file.id, file.at), false)
        .map_err(|e| e.to_string())?;
    let name = dc_ops::PictureImport::NAME;
    match editor.doc.apply_commands(name, &[&imported.command], now) {
        Ok(_) => Ok(()),
        Err(error) => Err(match imported.discard() {
            Ok(()) => error.to_string(),
            Err(e) => format!("{error}; {e}"),
        }),
    }
}

/// A new image annotation `add` (`annot add image`) showing the picture file `path`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewPicture {
    pub add: Step,
    pub path: PathBuf,
}

/// Adds the image annotation of `file` and shows its picture copied into the project, as one
/// undo step (`devcast annot add image` and `image set`); a refused command deletes the copy.
/// The message of what failed.
pub fn new_picture(
    editor: &mut EditorWindow,
    file: &NewPicture,
    now: Instant,
) -> Result<(), String> {
    let id = lanes::annotations::next_id(editor.doc.project()).map_err(|e| e.to_string())?;
    let add = editor.doc.command(&file.add).map_err(|e| e.to_string())?;
    let imported = dc_ops::import_picture(editor.doc.dir(), &file.path, (id, None), false)
        .map_err(|e| e.to_string())?;
    let commands: [&dyn dc_core::Command; 2] = [add.as_ref(), &imported.command];
    match editor.doc.apply_commands(&file.add.op, &commands, now) {
        Ok(_) => Ok(()),
        Err(error) => Err(match imported.discard() {
            Ok(()) => error.to_string(),
            Err(e) => format!("{error}; {e}"),
        }),
    }
}
