//! Delete, Ctrl+A and the clipboard of regions (mockup 2a). The clipboard lives in the window
//! (not the OS clipboard): a copied region is the registry command that adds it again — `zoom
//! add` and `callout add` carry all of a zoom and an event, annotations, cues and extra sound
//! go through `paste` with their JSON — and its offset from the first copied region. Ctrl+V adds
//! them at the playhead, keeping the offsets and the rows, in one undo step, and picks them.
//! A picked key copies instead (7.25, v1): Ctrl+V is `annot key copy` of it to the playhead, into
//! the picked annotation (else its own), and picks the pasted key.

use dc_core::{OutputTime, Project, TimeDelta, TimeMap};
use dc_fx_annotations::{AnnotId, Annotations, TRACKS};
use dc_fx_audio_tracks::Audio;
use dc_fx_callout::{Action as CalloutAction, Callout};
use dc_fx_captions::CaptionsEffect;
use dc_fx_zoom::ZoomEffect;
use dc_ops::Step;
use serde::Serialize;
use serde_json::{Map, Value};

use super::montage::arg;
use super::regions::{key_remove, remove, seconds};
use super::selection::{Item, Selected};
use super::{Action, EditorWindow};

/// What Ctrl+V puts at the playhead.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Clipboard {
    #[default]
    Empty,
    Regions(Vec<Copied>),
    /// The key of annotation `id` at output `at`.
    Key {
        id: AnnotId,
        at: OutputTime,
    },
}

/// A copied region: the command that adds it, without its `at`, and its offset from the start
/// of the first copied region.
#[derive(Debug, Clone, PartialEq)]
pub struct Copied {
    pub offset: TimeDelta,
    pub step: Step,
}

/// Delete: removes the picked keys, regions and clips in one undo step; the keys of a removed
/// annotation go with it.
pub fn delete(editor: &mut EditorWindow) {
    let selection = &editor.view.selection;
    let gone = |id| selection.contains(Selected::Region(Item::Annotation(id)));
    let keys = selection.keys().filter(|(id, _)| !gone(*id));
    let mut steps: Vec<Step> = keys.map(|(id, at)| key_remove(id, at)).collect();
    steps.extend(selection.regions().map(remove));
    if let Some(Action::Apply(clips)) = super::montage::remove(selection.clips()) {
        steps.extend(clips);
    }
    if !steps.is_empty() {
        editor.ask(Action::Apply(steps));
    }
}

/// Ctrl+C: the picked regions go to the clipboard, or the picked key when no region is
/// picked; clips are not copied.
pub fn copy(editor: &mut EditorWindow) {
    let selection = &editor.view.selection;
    if selection.regions().next().is_none()
        && let Some((id, at)) = selection.keys().next()
    {
        editor.view.clipboard = Clipboard::Key { id, at };
        return;
    }
    let picked: Vec<(Item, dc_core::Range<dc_core::time::Output>)> = editor
        .view
        .selection
        .regions()
        .filter_map(|item| Some((item, region_range(editor, item)?)))
        .collect();
    let Some(first) = picked.iter().map(|(_, r)| r.start()).min() else {
        return;
    };
    let project = editor.doc.project();
    let copied: Vec<Copied> = picked
        .iter()
        .filter_map(|&(item, range)| {
            Some(Copied {
                offset: range.start().since(first),
                step: adding(project, item, range.duration())?,
            })
        })
        .collect();
    if !copied.is_empty() {
        editor.view.clipboard = Clipboard::Regions(copied);
    }
}

/// Ctrl+X: copies the picked regions and removes them in one undo step.
pub fn cut(editor: &mut EditorWindow) {
    copy(editor);
    let steps: Vec<Step> = editor.view.selection.regions().map(remove).collect();
    if !steps.is_empty() {
        editor.ask(Action::Apply(steps));
    }
}

/// Ctrl+V: the clipboard at the playhead.
pub fn paste(editor: &mut EditorWindow) {
    let at = editor.view.playhead;
    let copied = match &editor.view.clipboard {
        Clipboard::Empty => return,
        Clipboard::Key { id, at: key } => {
            let into = super::stage::picked(editor).unwrap_or(*id);
            let mut args = Map::new();
            args.insert("id".to_owned(), serde_json::json!(id));
            args.insert("at".to_owned(), Value::String(arg(*key)));
            args.insert("to".to_owned(), Value::String(arg(at)));
            if into != *id {
                args.insert("into".to_owned(), serde_json::json!(into));
            }
            let step = Step::new("annot key copy", args);
            editor.ask(Action::Keyed(vec![step], into));
            return;
        }
        Clipboard::Regions(copied) => copied,
    };
    let steps: Vec<Step> = copied
        .iter()
        .map(|copied| {
            let mut step = copied.step.clone();
            let start = Value::String(arg(at + copied.offset));
            step.args.insert("at".to_owned(), start);
            step
        })
        .collect();
    if !steps.is_empty() {
        editor.ask(Action::Paste(steps));
    }
}

/// Ctrl+A: every region of the kinds picked now (all annotations for a picked annotation);
/// with no region picked, all zooms (v1).
pub fn select_all(editor: &mut EditorWindow) {
    let picked: Vec<Item> = editor.view.selection.regions().collect();
    let wanted = |item: Item| match picked.is_empty() {
        true => matches!(item, Item::Zoom(_)),
        false => picked.iter().any(|p| p.same_kind(item)),
    };
    let all: Vec<Item> = editor
        .tracks
        .rows
        .iter()
        .flat_map(|t| &t.regions)
        .filter_map(|r| r.item)
        .filter(|&item| wanted(item))
        .collect();
    if all.is_empty() {
        return;
    }
    editor.view.selection.clear();
    for item in all {
        editor.view.selection.add(Selected::Region(item));
    }
}

/// Where `item` lies on the output.
pub fn region_range(
    editor: &EditorWindow,
    item: Item,
) -> Option<dc_core::Range<dc_core::time::Output>> {
    editor
        .tracks
        .rows
        .iter()
        .flat_map(|t| &t.regions)
        .find(|r| r.item == Some(item))
        .map(|r| r.range)
}

/// The command that adds `item` again, `length` long on the output, without its `at`.
fn adding(project: &Project, item: Item, length: TimeDelta) -> Option<Step> {
    let mut args = Map::new();
    let op = match item {
        Item::Zoom(id) => {
            let state = project.state::<ZoomEffect>().ok()?;
            let zoom = state.zooms.into_iter().find(|z| z.id == id)?;
            args.insert("duration".to_owned(), seconds(length));
            args.insert("scale".to_owned(), zoom.scale.into());
            args.insert("mode".to_owned(), serde_json::to_value(zoom.mode).ok()?);
            args.insert("x".to_owned(), zoom.focus.x.into());
            args.insert("y".to_owned(), zoom.focus.y.into());
            "zoom add"
        }
        Item::Callout(id) => {
            let state = project.state::<Callout>().ok()?;
            let event = state.events.into_iter().find(|e| e.id == id)?;
            args.insert("action".to_owned(), event.action.name().into());
            if let CalloutAction::Text { text } = &event.action {
                args.insert("text".to_owned(), text.clone().into());
            }
            args.insert("duration".to_owned(), seconds(event.duration));
            "callout add"
        }
        Item::Annotation(id) => {
            let state = project.state::<Annotations>().ok()?;
            let annotation = state.annotations.into_iter().find(|a| a.id == id)?;
            args.insert("item".to_owned(), json(&annotation)?);
            args.insert("duration".to_owned(), seconds(length));
            "annot paste"
        }
        Item::Cue(id) => {
            let state = project.state::<CaptionsEffect>().ok()?;
            let cue = state.cues.into_iter().find(|c| c.id == id)?;
            args.insert("item".to_owned(), json(&cue)?);
            args.insert("duration".to_owned(), seconds(length));
            "captions paste"
        }
        Item::Sound(id) => {
            let state = project.state::<Audio>().ok()?;
            let clip = state.clips.into_iter().find(|c| c.id == id)?;
            args.insert("item".to_owned(), json(&clip)?);
            "audio paste"
        }
    };
    Some(Step::new(op, args))
}

fn json(value: &impl Serialize) -> Option<Value> {
    serde_json::to_value(value).ok()
}

/// «Дублировать» of a panel: the command that adds `item` again. An annotation goes right after
/// itself (`range` on the output; past a joint of recordings or the end the command refuses), on
/// its row when that row is free there, else on the first free row, else on its row on top (as
/// Ctrl+V); a sound stays where it is, on the first row free there (sounds of a row do not
/// overlap).
pub fn duplicate(
    project: &Project,
    item: Item,
    range: dc_core::Range<dc_core::time::Output>,
) -> Option<Step> {
    let mut step = adding(project, item, range.duration())?;
    let (place, track) = match item {
        Item::Annotation(id) => {
            let after = dc_core::Range::new(range.end(), range.end() + range.duration()).ok()?;
            (after, Some(annotation_row(project, id, after)?))
        }
        Item::Sound(_) => (
            range,
            Some(project.state::<Audio>().ok()?.free_track(range)?),
        ),
        _ => (range, None),
    };
    step.args
        .insert("at".to_owned(), Value::String(arg(place.start())));
    if let Some(track) = track {
        step.args.insert("track".to_owned(), track.into());
    }
    Some(step)
}

/// The row for a copy of annotation `id` at `place`: its own when free there, else the first
/// free one, else its own.
fn annotation_row(
    project: &Project,
    id: AnnotId,
    place: dc_core::Range<dc_core::time::Output>,
) -> Option<u32> {
    let state = project.state::<Annotations>().ok()?;
    let own = state.annotations.iter().find(|a| a.id == id)?.track;
    let map = TimeMap::build(project);
    let free = |track: u32| {
        state.annotations.iter().all(|a| {
            a.track != track
                || map
                    .output_span(&a.media, a.range)
                    .is_none_or(|shown| shown.intersect(&place).is_none())
        })
    };
    let first = (0..TRACKS).find(|&t| free(t));
    Some(if free(own) { own } else { first.unwrap_or(own) })
}
