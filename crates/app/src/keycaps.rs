//! Key caps of a shortcut (`.kbd` of the mockups): mono text on a raised cap, «+» between caps.
//! The settings and the editor help draw shortcuts with it.

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::tokens::{BORDER, FS_S, S1, SURFACE_3, TEXT, TEXT_3};
use eframe::egui::{self, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

/// Height of a cap and its text padding, points (mockup `.kbd`).
const HEIGHT: f32 = 20.0;
const PAD: f32 = 6.0;

/// Width of `caps` drawn by [`paint`].
pub fn width(painter: &Painter, caps: &[&str]) -> f32 {
    let plus = painter
        .layout_no_wrap("+".to_owned(), fonts::text(FS_S, Weight::Regular), TEXT_3)
        .size()
        .x;
    let joints = caps.len().saturating_sub(1) as f32 * (plus + 2.0 * S1);
    caps.iter()
        .map(|cap| {
            let galley = painter.layout_no_wrap((*cap).to_owned(), fonts::mono(FS_S), TEXT);
            galley.size().x + 2.0 * PAD
        })
        .sum::<f32>()
        + joints
}

/// Draws `caps` from `left` (the middle of their left edge); returns the right edge.
pub fn paint(painter: &Painter, left: Pos2, caps: &[&str]) -> f32 {
    let mut x = left.x;
    for (i, part) in caps.iter().enumerate() {
        if i > 0 {
            let plus = painter.text(
                pos2(x + S1, left.y),
                egui::Align2::LEFT_CENTER,
                "+",
                fonts::text(FS_S, Weight::Regular),
                TEXT_3,
            );
            x = plus.right() + S1;
        }
        let galley = painter.layout_no_wrap((*part).to_owned(), fonts::mono(FS_S), TEXT);
        let cap = Rect::from_min_size(
            pos2(x, left.y - HEIGHT / 2.0),
            vec2(galley.size().x + 2.0 * PAD, HEIGHT),
        );
        painter.rect(
            cap,
            4,
            SURFACE_3,
            Stroke::new(1.0, BORDER),
            StrokeKind::Inside,
        );
        painter.galley(
            pos2(cap.left() + PAD, cap.center().y - galley.size().y / 2.0),
            galley,
            TEXT,
        );
        x = cap.right();
    }
    x
}
