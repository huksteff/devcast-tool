//! Delete, Ctrl+A and the clipboard of regions (mockup 2a). The clipboard lives in the window
//! (not the OS clipboard): a copied region is the registry command that adds it again — `zoom
//! add` and `callout add` carry all of a zoom and an event, annotations, cues and extra sound
//! go through `paste` with their JSON — and its offset from the first copied region. Ctrl+V adds
//! them at the playhead, keeping the offsets and the rows, in one undo step, and picks them.
//! A picked key copies instead (7.25, v1): Ctrl+V is `annot key copy` of it to the playhead, into
//! the picked annotation (else its own), and picks the pasted key.

use dc_core::{OutputTime, Project, TimeDelta};
use dc_fx_annotations::AnnotId;
use dc_fx_zoom::ZoomKind;
use dc_ops::Step;
use serde_json::{Map, Value};

use super::lanes;
use super::montage::arg;
use super::regions::{key_remove, remove};
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
    let gone = |id| selection.contains(Selected::Region(Item::of(id)));
    let keys = selection.keys().filter(|(id, _)| !gone(*id));
    let mut steps: Vec<Step> = keys.filter_map(|(id, at)| key_remove(id, at)).collect();
    steps.extend(selection.regions().filter_map(remove));
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
                step: (lanes::of(item)?.copied)(project, item, range.duration())?,
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
    let steps: Vec<Step> = editor.view.selection.regions().filter_map(remove).collect();
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
        true => item.is::<ZoomKind>(),
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

/// «Дублировать» of a panel: the command that adds `item` lying at `range` again, placed by its
/// effect.
pub fn duplicate(
    project: &Project,
    item: Item,
    range: dc_core::Range<dc_core::time::Output>,
) -> Option<Step> {
    (lanes::of(item)?.duplicated)(project, item, range)
}
