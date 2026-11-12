use std::sync::Arc;

use dc_ui_kit::tokens;
use eframe::egui::{self, pos2, vec2};

use super::*;

fn at(rgba: &[u8], x: u32, y: u32) -> &[u8] {
    let i = ((y * SIDE + x) * 4) as usize;
    &rgba[i..i + 4]
}

#[test]
fn the_icon_is_the_48_px_picture_of_the_exe_icon() {
    assert_eq!(ICON.len(), (SIDE * SIDE * 4) as usize);
    assert!(
        ICON.as_chunks::<4>().0.iter().any(|pixel| pixel[3] == 255),
        "opaque icon"
    );
    let icon = window_icon();
    assert_eq!(icon.rgba, ICON);
    assert!(
        Arc::ptr_eq(&icon, &window_icon()),
        "one icon for every frame"
    );
}

#[test]
fn recording_puts_a_red_dot_in_the_lower_right_corner() {
    let (rest, recording) = (pixels(false), pixels(true));
    assert_eq!(rest, ICON);
    let centre = DOT_CENTRE as u32;
    assert_eq!(at(&recording, centre, centre), &tokens::REC.to_array());
    assert_eq!(
        at(&recording, 8, 8),
        at(&rest, 8, 8),
        "the rest of the icon stays"
    );
    let gap = (DOT_CENTRE + DOT_RADIUS + DOT_GAP / 2.0) as u32;
    assert_eq!(
        at(&recording, gap, centre)[3],
        0,
        "a clear ring around the dot"
    );
}

/// Both states 2× on the dark theme and on white, as the tray shows them on either taskbar.
/// Each pixel is a rectangle: textures sampled by WARP differ from run to run.
#[test]
fn icon_states() {
    const CELL: f32 = 112.0;
    const SCALE: f32 = 2.0;
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(vec2(2.0 * CELL, 2.0 * CELL))
        .build_ui(|ui| {
            let painter = ui.painter();
            let margin = (CELL - SCALE * SIDE as f32) / 2.0;
            for (row, background) in [tokens::BG, egui::Color32::WHITE].into_iter().enumerate() {
                let top = CELL * row as f32;
                let band = egui::Rect::from_min_size(pos2(0.0, top), vec2(2.0 * CELL, CELL));
                painter.rect_filled(band, 0, background);
                for (column, recording) in [false, true].into_iter().enumerate() {
                    let origin = pos2(CELL * column as f32 + margin, top + margin);
                    let rgba = pixels(recording);
                    for (i, p) in rgba.as_chunks::<4>().0.iter().enumerate() {
                        let (x, y) = ((i as u32 % SIDE) as f32, (i as u32 / SIDE) as f32);
                        let min = origin + SCALE * vec2(x, y);
                        let rect = egui::Rect::from_min_size(min, vec2(SCALE, SCALE));
                        let colour = egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]);
                        painter.rect_filled(rect, 0, colour);
                    }
                }
            }
        });
    harness.run();
    harness.snapshot("tray_icon_states");
}
