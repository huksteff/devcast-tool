//! The tooltip of a key marker (7.25, mockup 2e, v1 `keyframeChanges`): «Ключ 2 · 0:33.40» and
//! what the key changes against the previous one (the first key against the annotation without
//! keys), a line «Параметр: было → стало» each: the box in px of the frame, the rotation, the
//! kind's fields.

use dc_core::{OutputTime, PixelSize, Project, Rect};
use dc_fx_annotations::{AnnotId, Annotations, Kind, key_offset};
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::TEXT_3;
use eframe::egui::{RichText, Ui};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use super::scale::timecode;

/// Widest tooltip, points (v1).
const WIDTH: f32 = 320.0;
/// Longest text value shown whole, chars (v1).
const TEXT_MAX: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub struct Tip {
    pub title: String,
    pub since: String,
    pub lines: Vec<String>,
}

/// One state of the annotation: its box, rotation and kind's fields.
struct Look<'a> {
    rect: Rect,
    rotation: f64,
    body: &'a Map<String, Value>,
}

/// The tooltip of the key of annotation `id` at output `at` on a `canvas` px frame.
pub fn tip(project: &Project, canvas: PixelSize, id: AnnotId, at: OutputTime) -> Option<Tip> {
    let state = project.state::<Annotations>().ok()?;
    let annotation = state.annotations.into_iter().find(|a| a.id == id)?;
    let offset = key_offset(project, &annotation, at).ok()?;
    let index = annotation.key_index(offset)?;
    let key = &annotation.keys[index];
    let (before, since) = match index.checked_sub(1) {
        Some(i) => {
            let k = &annotation.keys[i];
            let look = Look {
                rect: k.rect,
                rotation: k.rotation,
                body: &k.body,
            };
            (look, text::key_since(index))
        }
        None => {
            let look = Look {
                rect: annotation.rect,
                rotation: annotation.rotation,
                body: &annotation.body,
            };
            (look, text::KEY_SINCE_BASE.to_owned())
        }
    };
    let after = Look {
        rect: key.rect,
        rotation: key.rotation,
        body: &key.body,
    };
    let mut lines = changes(annotation.kind, &before, &after, canvas)?;
    if lines.is_empty() {
        lines.push(text::KEY_SAME.to_owned());
    }
    Some(Tip {
        title: text::key_tip(index + 1, &timecode(at)),
        since,
        lines,
    })
}

/// The lines of what changes from `before` to `after`.
fn changes(kind: Kind, before: &Look, after: &Look, canvas: PixelSize) -> Option<Vec<String>> {
    let (w, h) = (f64::from(canvas.width), f64::from(canvas.height));
    let place = |r: Rect| format!("{}, {}", number(r.x() * w), number(r.y() * h));
    let size = |r: Rect| format!("{} × {}", number(r.w() * w), number(r.h() * h));
    let degrees = |v: f64| format!("{}°", number(v));
    let mut lines = Vec::new();
    for (name, shown) in [
        (text::KEY_POSITION, &place as &dyn Fn(Rect) -> String),
        (text::KEY_SIZE, &size),
    ] {
        let (from, to) = (shown(before.rect), shown(after.rect));
        if from != to {
            lines.push(text::key_change(name, &from, &to));
        }
    }
    if before.rotation != after.rotation {
        let (from, to) = (degrees(before.rotation), degrees(after.rotation));
        lines.push(text::key_change(text::KEY_ROTATION, &from, &to));
    }
    let (from, to) = (full(kind, before.body)?, full(kind, after.body)?);
    for (name, value) in &to {
        let old = from.get(name).unwrap_or(&Value::Null);
        if old == value {
            continue;
        }
        let field = text::key_field(name);
        lines.push(match (old, value) {
            (Value::Array(_) | Value::Object(_), _) | (_, Value::Array(_) | Value::Object(_)) => {
                text::key_changed(field)
            }
            _ => text::key_change(field, &shown(old), &shown(value)),
        });
    }
    Some(lines)
}

/// The kind's fields with the defaults of the missing ones, as the kind reads them.
fn full(kind: Kind, body: &Map<String, Value>) -> Option<Map<String, Value>> {
    match kind {
        Kind::Text => through::<dc_fx_text::TextBody>(body),
        Kind::Shape => through::<dc_fx_shape::ShapeBody>(body),
        Kind::Image => through::<dc_fx_image::ImageBody>(body),
        Kind::Drawing => through::<dc_fx_drawing::DrawingBody>(body),
        Kind::Blur => through::<dc_fx_blur::BlurBody>(body),
    }
}

fn through<B: DeserializeOwned + Serialize>(
    body: &Map<String, Value>,
) -> Option<Map<String, Value>> {
    let kind: B = serde_json::from_value(Value::Object(body.clone())).ok()?;
    match serde_json::to_value(kind).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

/// A value as the tooltip shows it: whole numbers whole, others to 2 decimals, switches on/off,
/// long text cut; lists and groups are not shown, only that they changed.
fn shown(value: &Value) -> String {
    match value {
        Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), number),
        Value::Bool(true) => text::KEY_ON.to_owned(),
        Value::Bool(false) => text::KEY_OFF.to_owned(),
        Value::String(s) if s.chars().count() > TEXT_MAX => {
            let cut: String = s.chars().take(TEXT_MAX - 3).collect();
            format!("{cut}…")
        }
        Value::String(s) => s.clone(),
        Value::Null | Value::Array(_) | Value::Object(_) => text::KEY_NONE.to_owned(),
    }
}

/// `v` whole when it is, else to 2 decimals with a comma.
fn number(v: f64) -> String {
    let rounded = (v * 100.0).round() / 100.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.2}").replace('.', ",")
    }
}

/// The tooltip's contents.
pub fn ui(ui: &mut Ui, tip: &Tip) {
    ui.set_max_width(WIDTH);
    ui.label(RichText::new(&tip.title).strong());
    ui.label(RichText::new(&tip.since).color(TEXT_3));
    for line in &tip.lines {
        ui.label(line);
    }
}
