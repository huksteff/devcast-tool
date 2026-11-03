//! Adding regions (mockup 2e, «призрак»): a hovered empty spot of a row, Z and T at the playhead,
//! a sound file on an extra sound row; a picture
//! file into an image annotation. A new region has the length v1 gives (`LENGTH`), fits the
//! room it is in (zooms and extra sound do not overlap their neighbours, zooms, annotations and
//! callout events lie in one clip) and is added by the effect's own command, then picked. Tab
//! picks the next annotation under the playhead in the order they are painted.

use std::path::PathBuf;
use std::time::Instant;

use dc_core::time::Output;
use dc_core::{OutputTime, Range, TimeDelta};
use dc_fx_annotations::TRACKS;
use dc_fx_audio_tracks::AudioAdd;
use dc_ops::{SoundPlace, Step};
use dc_ui_kit::lane::seconds;
use serde_json::{Map, Value};

use super::lanes;
use super::montage::arg;
use super::selection::{Item, Selected};
use super::timeline::{Row, Tracks};
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
    pub id: dc_fx_annotations::AnnotId,
    pub at: Option<OutputTime>,
    pub path: PathBuf,
}

/// Regions can be added to `row` by hovering it.
pub fn addable(row: Row) -> bool {
    matches!(
        row,
        Row::Zooms | Row::Cursor | Row::Annotations(_) | Row::Audio(_)
    )
}

/// Where a region added on `row` at `at` lies; `None` when there is no room.
pub fn slot(tracks: &Tracks, row: Row, at: OutputTime) -> Option<Range<Output>> {
    let end = OutputTime::ZERO + tracks.duration;
    let (mut low, mut high) = (OutputTime::ZERO, end);
    if !matches!(row, Row::Audio(_)) {
        let clip = regions(tracks, Row::Clips).find(|r| r.contains(at))?;
        (low, high) = (clip.start(), clip.end());
    }
    if matches!(row, Row::Zooms | Row::Audio(_)) {
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

/// The command that adds a region at `range` of `row` (a text annotation, a callout «on»).
pub fn step(row: Row, range: Range<Output>) -> Option<Step> {
    match row {
        Row::Zooms => return Some(lanes::zoom::added(range)),
        Row::Cursor => return Some(lanes::callout::added(range)),
        _ => {}
    }
    let mut args = Map::new();
    args.insert("at".to_owned(), Value::String(arg(range.start())));
    let op = match row {
        Row::Annotations(track) => {
            args.insert("kind".to_owned(), Value::from("text"));
            args.insert("track".to_owned(), track.into());
            "annot add"
        }
        _ => return None,
    };
    args.insert("duration".to_owned(), seconds(range.duration()));
    Some(Step::new(op, args))
}

/// Z: a zoom at the playhead; where there is no room, the command says why.
pub fn zoom(editor: &mut EditorWindow) {
    let at = editor.view.playhead;
    let range = slot(&editor.tracks, Row::Zooms, at).or_else(|| Range::new(at, at + LENGTH).ok());
    if let Some(step) = range.and_then(|r| step(Row::Zooms, r)) {
        editor.ask(Action::Paste(vec![step]));
    }
}

/// T: a text annotation at the playhead on the lowest row free there, a new row when none is.
pub fn annotation(editor: &mut EditorWindow) {
    let at = editor.view.playhead;
    let range =
        slot(&editor.tracks, Row::Annotations(0), at).or_else(|| Range::new(at, at + LENGTH).ok());
    let Some(range) = range else { return };
    let rows: Vec<u32> = editor
        .tracks
        .rows
        .iter()
        .filter_map(|t| match t.row {
            Row::Annotations(track) => Some(track),
            _ => None,
        })
        .collect();
    let busy = |track: u32| {
        regions(&editor.tracks, Row::Annotations(track)).any(|r| r.intersect(&range).is_some())
    };
    let new = (rows.len() as u32).min(TRACKS - 1);
    let track = rows.iter().copied().find(|t| !busy(*t)).unwrap_or(new);
    if let Some(step) = step(Row::Annotations(track), range) {
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
        .filter(|t| matches!(t.row, Row::Annotations(_)))
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
        .apply_commands(AudioAdd::NAME, &[&imported.command], now)
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
    let name = dc_fx_image::ImageSet::NAME;
    match editor.doc.apply_commands(name, &[&imported.command], now) {
        Ok(_) => Ok(()),
        Err(error) => Err(match imported.discard() {
            Ok(()) => error.to_string(),
            Err(e) => format!("{error}; {e}"),
        }),
    }
}
