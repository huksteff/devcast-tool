//! The panel of a picked annotation (mockup 3c and the main window «Текст»): the key strip under
//! the header, «Основное» of its kind (`annot_text.rs`, `annot_shape.rs`, `annot_image.rs`,
//! `annot_drawing.rs`, `annot_blur.rs`), «Вход и выход» (`annot anim`) and «Тонкая настройка»:
//! the kind's finer fields, the box and rotation (`annot set`), the time (`annot set --from
//! --to`), the row (`annot move --track`) and the layer (`annot layer`).
//!
//! Keys (QUESTIONS «Ключи аннотаций» 2, the CLI rule): while the playhead stands on a key, the
//! kind's fields, the box and the rotation change in that key (`--at` the key); elsewhere they
//! change in the annotation and in every key, or with «Авто-ключи» (7.25, v1) in a new key at the
//! playhead. The rows show what the playhead shows; the strip names the key and steps the
//! playhead to the previous or the next one.

use dc_core::time::Output;
use dc_core::{OutputTime, Range, TimeDelta};
use dc_fx_annotations::{AnnotId, Annotation, Annotations, Kind, key_offset};
use dc_ops::Step;
use dc_ui_kit::strings::editor;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use super::annotation_rows::{fine_rows, keys_strip, motion_group};
use super::{Cx, Group, padded, rows};
use crate::editor::montage;
use eframe::egui::Ui;

/// Where an edit of the kind's fields, the box and the rotation goes: the key at the playhead,
/// or everywhere (`at` none).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edit {
    pub id: AnnotId,
    pub at: Option<OutputTime>,
}

impl Edit {
    /// `<op> <id> --<field> <value> … [--at <key>]`.
    pub fn step(self, op: &str, fields: impl IntoIterator<Item = (&'static str, Value)>) -> Step {
        let mut args = Map::new();
        args.insert("id".to_owned(), json!(self.id));
        for (name, value) in fields {
            args.insert(name.to_owned(), value);
        }
        if let Some(at) = self.at {
            args.insert("at".to_owned(), json!(montage::arg(at)));
        }
        Step::new(op, args)
    }

    /// [`Edit::step`] as one undo step.
    pub fn apply(
        self,
        op: &str,
        fields: impl IntoIterator<Item = (&'static str, Value)>,
    ) -> crate::editor::Action {
        crate::editor::Action::Apply(vec![self.step(op, fields)])
    }

    /// The step that brings the fields `now` to `default`; nothing when they are the same.
    pub fn reset(
        self,
        op: &str,
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
pub struct Shown {
    pub annotation: Annotation,
    /// Its place on the output.
    pub range: Range<Output>,
    pub rect: dc_core::Rect,
    pub rotation: f64,
    pub body: Map<String, Value>,
    /// Annotation tracks there are: the rows it can move to.
    pub tracks: u32,
    /// The output frame the box is a part of; `None` without clips.
    pub canvas: Option<dc_core::PixelSize>,
}

/// The strip, the kind's groups and the common ones of annotation `id`, shown at `range` with
/// its keys at `keys` of the output.
pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>, id: AnnotId, range: Range<Output>, keys: &[OutputTime]) {
    let Some(shown) = read(cx, id, range) else {
        return;
    };
    let on = key_at(keys, cx.base.playhead);
    let at = target(keys, cx.base.playhead, range, cx.base.auto_keys);
    keys_strip(ui, cx, keys, on, at);
    let edit = Edit { id, at };
    match shown.annotation.kind {
        Kind::Text => panel::<dc_fx_text::TextBody>(ui, cx, &shown, edit),
        Kind::Shape => panel::<dc_fx_shape::ShapeBody>(ui, cx, &shown, edit),
        Kind::Image => panel::<dc_fx_image::ImageBody>(ui, cx, &shown, edit),
        Kind::Drawing => panel::<dc_fx_drawing::DrawingBody>(ui, cx, &shown, edit),
        Kind::Blur => panel::<dc_fx_blur::BlurBody>(ui, cx, &shown, edit),
    }
}

/// The key of `keys` (output times) at `playhead`.
pub fn key_at(keys: &[OutputTime], playhead: OutputTime) -> Option<OutputTime> {
    let same = |k: &OutputTime| crate::editor::selection::same_moment(*k, playhead);
    keys.iter().copied().find(same)
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
fn read(cx: &Cx<'_>, id: AnnotId, range: Range<Output>) -> Option<Shown> {
    let state = cx.base.project.state::<Annotations>().ok()?;
    let tracks = state
        .annotations
        .iter()
        .map(|a| a.track + 1)
        .max()
        .unwrap_or(1);
    let annotation = state.annotations.into_iter().find(|a| a.id == id)?;
    let offset = match key_offset(cx.base.project, &annotation, cx.base.playhead) {
        Ok(offset) => offset,
        Err(_) if cx.base.playhead <= range.start() => TimeDelta::ZERO,
        Err(_) => annotation.range.duration(),
    };
    let crop = cx.base.project.state::<dc_fx_crop::Crop>().ok()?;
    let canvas = dc_fx_crop::canvas(cx.base.project, &crop.settings);
    let (rect, rotation) = annotation.frame_at(offset);
    let body = annotation.body_at(offset);
    Some(Shown {
        annotation,
        range,
        rect,
        rotation,
        body,
        tracks,
        canvas,
    })
}

fn panel<B: Section>(ui: &mut Ui, cx: &mut Cx<'_>, shown: &Shown, edit: Edit) {
    let body: B = match serde_json::from_value(Value::Object(shown.body.clone())) {
        Ok(body) => body,
        Err(error) => {
            padded(ui, |ui| rows::note(ui, &error.to_string()));
            return;
        }
    };
    cx.base.groups.panel(B::ID);
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
    reset.extend(edit.reset("annot set", &turned, &[("rotation", json!(0.0))]));
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
