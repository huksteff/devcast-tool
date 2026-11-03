//! The tooltip of a key marker (7.25, mockup 2e): what the key changes, from the annotations
//! lane (`lanes::annotations::tip`, the kinds' fields by the registration `lanes::kind_fields`).

use dc_core::{OutputTime, PixelSize, Project};
use dc_ui_kit::tokens::TEXT_3;
use eframe::egui::{RichText, Ui};

pub use crate::editor::lanes::annotations::Tip;
use crate::editor::lanes::{self, annotations::AnnotId};

/// Widest tooltip, points (v1).
const WIDTH: f32 = 320.0;

/// The tooltip of the key of annotation `id` at output `at` on a `canvas` px frame.
pub fn tip(project: &Project, canvas: PixelSize, id: AnnotId, at: OutputTime) -> Option<Tip> {
    lanes::annotations::tip(project, canvas, (id, at), lanes::kind_fields)
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
