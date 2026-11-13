//! A kind of annotation as the editor registers it (docs/ARCHITECTURE.md «Панель эффекта»,
//! «Аннотации»): this crate does not know its kinds' fields, so each kind's crate gives an
//! [`Entry`] of its fields type in its `panel` module, and the editor's registration lists them.

use dc_ui_kit::egui::Ui;
use dc_ui_kit::panel::Cx;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::panel::{Region, Section, ui};
use crate::{AnnotId, Kind, lane};

/// The fields of a body with the kind's defaults; none when they do not read as the kind's.
pub type Fields = fn(&Map<String, Value>) -> Option<Map<String, Value>>;

/// What the editor does with a kind: its panel section and its fields with their defaults.
#[derive(Clone, Copy)]
pub struct Entry {
    pub kind: Kind,
    /// The panel of an annotation of this kind (`panel::ui` with the kind's section).
    pub ui: fn(&mut Ui, &mut Cx<'_>, AnnotId, &Region<'_>),
    /// The fields of a body with the kind's defaults (the key tips, `lane::tip`).
    pub fields: Fields,
}

impl Entry {
    /// The entry of kind `kind` whose fields are `B`.
    pub const fn of<B: Section + Serialize>(kind: Kind) -> Self {
        Self {
            kind,
            ui: ui::<B>,
            fields: lane::fields::<B>,
        }
    }
}
