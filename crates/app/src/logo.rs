//! The DevCast icon of `branding/devcast.ico` (the exe's icon too), with a red dot while
//! recording (owner's decision 2026-11-03). The tray icon and the title bar icon of every window
//! use it; `build.rs` decodes its 32 × 32 picture.

use std::sync::{Arc, OnceLock};

use dc_ui_kit::tokens;
use eframe::egui;

/// Side of the icon, pixels.
pub const SIDE: u32 = 32;

/// RGBA pixels of the 32 × 32 picture of `branding/devcast.ico`, rows from the top.
const ICON: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon.rgba"));

/// The recording dot in the lower right corner: centre and radius, px of the icon.
const DOT_CENTRE: f32 = 24.0;
const DOT_RADIUS: f32 = 7.0;
/// The transparent ring around the dot that separates it from the icon, px.
const DOT_GAP: f32 = 1.5;

/// RGBA pixels of the icon, `SIDE` × `SIDE`, rows from the top.
pub fn pixels(recording: bool) -> Vec<u8> {
    let mut rgba = ICON.to_vec();
    if recording {
        for (i, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x, y) = ((i as u32 % SIDE) as f32, (i as u32 / SIDE) as f32);
            let distance = (x + 0.5 - DOT_CENTRE).hypot(y + 0.5 - DOT_CENTRE);
            dot(pixel, distance);
        }
    }
    rgba
}

/// One pixel `distance` px from the dot's centre: the red dot over the icon, the icon cleared in
/// the gap around it; both edges antialiased over one pixel. RGBA is not premultiplied.
fn dot(pixel: &mut [u8], distance: f32) {
    let red = (DOT_RADIUS + 0.5 - distance).clamp(0.0, 1.0);
    let keep = (distance - DOT_RADIUS - DOT_GAP + 0.5).clamp(0.0, 1.0);
    let icon = f32::from(pixel[3]) / 255.0 * keep * (1.0 - red);
    let alpha = red + icon;
    if alpha == 0.0 {
        pixel.fill(0);
        return;
    }
    let rec = tokens::REC.to_array();
    for c in 0..3 {
        pixel[c] = ((f32::from(rec[c]) * red + f32::from(pixel[c]) * icon) / alpha).round() as u8;
    }
    pixel[3] = (alpha * 255.0).round() as u8;
}

/// The icon at rest for `ViewportBuilder::with_icon`: always the same `Arc`, since egui sends
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

#[cfg(test)]
#[path = "logo_tests.rs"]
mod tests;
