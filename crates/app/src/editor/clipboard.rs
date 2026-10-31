//! Delete, Ctrl+A and the clipboard of regions (mockup 2a). The clipboard lives in the window
//! (not the OS clipboard): a copied region is the registry command that adds it again — `zoom
//! add` and `callout add` carry all of a zoom and an event, annotations, cues and extra sound
//! go through `paste` with their JSON — and its offset from the first copied region. Ctrl+V adds
//! them at the playhead, keeping the offsets and the rows, in one undo step, and picks them.

use dc_core::{Project, TimeDelta};
use dc_fx_annotations::Annotations;
use dc_fx_audio_tracks::Audio;
use dc_fx_callout::{Action as CalloutAction, Callout};
use dc_fx_captions::CaptionsEffect;
use dc_fx_zoom::ZoomEffect;
use dc_ops::Step;
use serde::Serialize;
use serde_json::{Map, Value};

use super::montage::arg;
use super::regions::{remove, seconds};
use super::selection::{Item, Selected};
use super::{Action, EditorWindow};

/// A copied region: the command that adds it, without its `at`, and its offset from the start
/// of the first copied region.
#[derive(Debug, Clone, PartialEq)]
pub struct Copied {
    pub offset: TimeDelta,
    pub step: Step,
}

/// Delete: removes the picked regions and clips in one undo step.
pub fn delete(editor: &mut EditorWindow) {
    let selection = &editor.view.selection;
    let mut steps: Vec<Step> = selection.regions().map(remove).collect();
    if let Some(Action::Apply(clips)) = super::montage::remove(selection.clips()) {
        steps.extend(clips);
    }
    if !steps.is_empty() {
        editor.ask(Action::Apply(steps));
    }
}

/// Ctrl+C: the picked regions go to the clipboard; clips are not copied.
pub fn copy(editor: &mut EditorWindow) {
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
        editor.view.clipboard = copied;
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
    let steps: Vec<Step> = editor
        .view
        .clipboard
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
