//! The DevCast logo drawn in code: a blue disc with a white ▶ at rest (mockup 6), a red dot
//! while recording. The tray icon and the title bar icon of every window use it.

use std::sync::{Arc, OnceLock};

use eframe::egui;

/// Side of the icon, pixels.
pub const SIDE: u32 = 32;

/// RGBA pixels of the icon, `SIDE` × `SIDE`, rows from the top.
pub fn pixels(recording: bool) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    let c = SIDE as f32 / 2.0;
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let inside = (dx * dx + dy * dy).sqrt() <= c - 1.0;
            let pixel = if !inside {
                [0, 0, 0, 0]
            } else if recording {
                [0xf4, 0x3f, 0x5e, 0xff]
            } else if play(dx, dy, c) {
                [0xff, 0xff, 0xff, 0xff]
            } else {
                [0x25, 0x63, 0xeb, 0xff]
            };
            rgba.extend_from_slice(&pixel);
        }
    }
    rgba
}

/// The logo at rest for `ViewportBuilder::with_icon`: always the same `Arc`, since egui sends
/// the icon to the window again whenever the `Arc` differs from last frame's.
pub fn window_icon() -> Arc<egui::IconData> {
    static ICON: OnceLock<Arc<egui::IconData>> = OnceLock::new();
    Arc::clone(ICON.get_or_init(|| {
        Arc::new(egui::IconData {
            rgba: pixels(false),
            width: SIDE,
            height: SIDE,
        })
    }))
}

/// The ▶ inside the disc of radius `r`, offsets from its centre.
fn play(dx: f32, dy: f32, r: f32) -> bool {
    let (left, right, half) = (-0.3 * r, 0.45 * r, 0.4 * r);
    dx >= left && dx <= right && dy.abs() <= half * (right - dx) / (right - left)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_sign_sits_inside_the_disc() {
        let r = SIDE as f32 / 2.0;
        assert!(play(0.0, 0.0, r));
        assert!(!play(-0.5 * r, 0.0, r));
        assert!(!play(0.0, 0.45 * r, r));
        let side = SIDE as usize;
        for recording in [false, true] {
            assert_eq!(pixels(recording).len(), side * side * 4);
        }
        let icon = window_icon();
        let centre = (side / 2 * side + side / 2) * 4;
        assert_eq!(&icon.rgba[centre..centre + 4], &[0xff, 0xff, 0xff, 0xff]);
        assert_eq!(icon.rgba[3], 0, "the corner is transparent");
        assert!(
            Arc::ptr_eq(&icon, &window_icon()),
            "one icon for every frame"
        );
    }
}
