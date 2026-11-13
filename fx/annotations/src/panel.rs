//! The panel of a picked annotation (mockup 3c and the main window «Текст»): the key strip under
//! the header, «Основное» of its kind (a [`Section`] in the kind's crate: `fx/text`, `shape`,
//! `image`, `drawing`, `blur`), «Вход и выход» (`annot anim`) and «Тонкая настройка»: the kind's
//! finer fields, the box and rotation (`annot set`), the time (`annot set --from --to`), the
//! row (`annot move --track`) and the layer (`annot layer`) — `panel_rows.rs`.
//!
//! This crate cannot name its kinds (they depend on it), so the editor picks the kind's
//! [`Section`] by [`kind`] and calls [`ui`] with it. The facts of the timeline — where the
//! annotation is shown, its keys on the output, the output frame — are a [`Region`].
//!
//! Keys (QUESTIONS «Ключи аннотаций» 2, the CLI rule): while the playhead stands on a key, the
//! kind's fields, the box and the rotation change in that key (`--at` the key); elsewhere they
//! change in the annotation and in every key, or with «Авто-ключи» (7.25, v1) in a new key at the
//! playhead ([`target`], the preview's edits go there too). The rows show what the playhead
//! shows; the strip names the key and steps the playhead to the previous or the next one.

use dc_core::OpName;
use dc_core::time::Output;
use dc_core::{OutputTime, PixelSize, Project, Range, Step, TimeDelta};
use dc_ui_kit::egui::Ui;
use dc_ui_kit::panel::{Cx, Group, padded, rows, time_arg};
use dc_ui_kit::strings::editor;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::panel_rows::{fine_rows, keys_strip, motion_group};
use crate::{AnnotId, Annotation, Annotations, Kind, key_offset, same_moment};

const ANNOT_SET: OpName = OpName::of::<crate::AnnotSet>();

/// Where the timeline shows the picked annotation; the editor fills it.
#[derive(Debug, Clone, Copy)]
pub struct Region<'a> {
    /// Its place on the output.
    pub range: Range<Output>,
    /// Its keys at these output times, in order.
    pub keys: &'a [OutputTime],
    /// The output frame the box is a part of (after the crop); `None` without clips.
    pub canvas: Option<PixelSize>,
}

/// Where an edit of the kind's fields, the box and the rotation goes: the key at the playhead,
/// or everywhere (`at` none).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edit {
    pub id: AnnotId,
    pub at: Option<OutputTime>,
}

impl Edit {
    /// `<op> <id> --<field> <value> … [--at <key>]`.
    pub fn step(self, op: OpName, fields: impl IntoIterator<Item = (&'static str, Value)>) -> Step {
        let mut args = Map::new();
        args.insert("id".to_owned(), json!(self.id));
        for (name, value) in fields {
            args.insert(name.to_owned(), value);
        }
        if let Some(at) = self.at {
            args.insert("at".to_owned(), json!(time_arg(at)));
        }
        Step::of(op, args)
    }

    /// [`Edit::step`] as one undo step of the panel.
    pub fn apply(
        self,
        cx: &mut Cx<'_>,
        op: OpName,
        fields: impl IntoIterator<Item = (&'static str, Value)>,
    ) {
        cx.edits.push(vec![self.step(op, fields)]);
    }

    /// The step that brings the fields `now` to `default`; nothing when they are the same.
    pub fn reset(
        self,
        op: OpName,
        now: &[(&'static str, Value)],
        default: &[(&'static str, Value)],
    ) -> Option<Step> {
        (now != default).then(|| self.step(op, default.iter().cloned()))
    }

    /// The edit of the fields that keys do not hold (time, row, layer, animations).
    pub fn unkeyed(self) -> Self {
        Self { at: None, ..self }
    }
}

/// A kind's part of the panel over its fields `Self` (`body` of the annotation as the playhead
/// shows it).
pub trait Section: DeserializeOwned {
    /// First part of the keys of the remembered groups of this kind.
    const ID: &'static str;

    /// The rows of «Основное».
    fn basic(&self, ui: &mut Ui, cx: &mut Cx<'_>, edit: Edit);

    /// The steps of «Сбросить» of «Основное»; `None` for a kind without one.
    fn basic_reset(&self, edit: Edit) -> Option<Vec<Step>>;

    /// The kind's rows at the top of «Тонкая настройка».
    fn fine(&self, _ui: &mut Ui, _cx: &mut Cx<'_>, _edit: Edit) {}

    /// The kind's steps of «Сбросить» of «Тонкая настройка».
    fn fine_reset(&self, _edit: Edit) -> Vec<Step> {
        Vec::new()
    }
}

/// The annotation as the playhead shows it.
pub(crate) struct Shown {
    pub annotation: Annotation,
    /// Its place on the output.
    pub range: Range<Output>,
    pub rect: dc_core::Rect,
    pub rotation: f64,
    pub body: Map<String, Value>,
    /// Annotation tracks there are: the rows it can move to.
    pub tracks: u32,
    pub canvas: Option<PixelSize>,
}

/// The kind of annotation `id`: the editor gives [`ui`] its [`Section`].
pub fn kind(project: &Project, id: AnnotId) -> Option<Kind> {
    let state = project.state::<Annotations>().ok()?;
    state
        .annotations
        .iter()
        .find(|a| a.id == id)
        .map(|a| a.kind)
}

/// The strip, the groups of kind `B` and the common ones of annotation `id` shown at `region`.
pub fn ui<B: Section>(ui: &mut Ui, cx: &mut Cx<'_>, id: AnnotId, region: &Region<'_>) {
    let Some(shown) = read(cx, id, region) else {
        return;
    };
    let on = key_at(region.keys, cx.playhead);
    let at = target(region.keys, cx.playhead, region.range, cx.auto_keys);
    keys_strip(ui, cx, region.keys, on, at);
    section::<B>(ui, cx, &shown, Edit { id, at });
}

/// The key of `keys` (output times) at `playhead`.
pub fn key_at(keys: &[OutputTime], playhead: OutputTime) -> Option<OutputTime> {
    keys.iter().copied().find(|k| same_moment(*k, playhead))
}

/// Where an edit of an annotation shown at `range` with keys at `keys` goes: the key at the
/// playhead; with automatic keys (`auto`) the playhead while it shows the annotation (the command
/// makes a key there); else nowhere — the annotation and every key.
pub fn target(
    keys: &[OutputTime],
    playhead: OutputTime,
    range: Range<Output>,
    auto: bool,
) -> Option<OutputTime> {
    let inside = range.start() <= playhead && playhead < range.end();
    key_at(keys, playhead).or((auto && inside).then_some(playhead))
}

/// Annotation `id` with what the playhead shows of it: before it — its start, after — its end.
fn read(cx: &Cx<'_>, id: AnnotId, region: &Region<'_>) -> Option<Shown> {
    let state = cx.project.state::<Annotations>().ok()?;
    let tracks = state
        .annotations
        .iter()
        .map(|a| a.track + 1)
        .max()
        .unwrap_or(1);
    let annotation = state.annotations.into_iter().find(|a| a.id == id)?;
    let offset = match key_offset(cx.project, &annotation, cx.playhead) {
        Ok(offset) => offset,
        Err(_) if cx.playhead <= region.range.start() => TimeDelta::ZERO,
        Err(_) => annotation.range.duration(),
    };
    let (rect, rotation) = annotation.frame_at(offset);
    let body = annotation.body_at(offset);
    Some(Shown {
        annotation,
        range: region.range,
        rect,
        rotation,
        body,
        tracks,
        canvas: region.canvas,
    })
}

fn section<B: Section>(ui: &mut Ui, cx: &mut Cx<'_>, shown: &Shown, edit: Edit) {
    let body: B = match serde_json::from_value(Value::Object(shown.body.clone())) {
        Ok(body) => body,
        Err(error) => {
            padded(ui, |ui| rows::note(ui, &error.to_string()));
            return;
        }
    };
    cx.groups.panel(B::ID);
    let basic = Group {
        id: "basic",
        title: editor::BASIC,
        open: true,
        reset: body.basic_reset(edit),
    };
    cx.group(ui, basic, |ui, cx| body.basic(ui, cx, edit));
    motion_group(ui, cx, &shown.annotation, edit.unkeyed());
    let mut reset = body.fine_reset(edit);
    let turned = [("rotation", json!(shown.rotation))];
    reset.extend(edit.reset(ANNOT_SET, &turned, &[("rotation", json!(0.0))]));
    let fine = Group {
        id: "fine",
        title: editor::FINE,
        open: false,
        reset: Some(reset),
    };
    cx.group(ui, fine, |ui, cx| {
        body.fine(ui, cx, edit);
        fine_rows(ui, cx, shown, edit);
    });
}
