//! Where the windows besides the HUD open: centred on the HUD's monitor.

use eframe::egui;

use crate::app::Shared;

/// Corner of a window of `size` centred on the HUD's monitor, in points of the main window as
/// the other windows are placed; `None` while the monitors are unknown.
pub fn on_hud_monitor(
    ctx: &egui::Context,
    shared: &Shared,
    size: egui::Vec2,
) -> Option<egui::Pos2> {
    let bounds = shared
        .store
        .get()
        .hud
        .monitor(&shared.sources.monitors)?
        .bounds;
    let ppp = ctx.native_pixels_per_point().unwrap_or(1.0);
    Some(centred(bounds, ppp, size))
}

/// Corner of a window of `size` points centred on `bounds` (physical desktop pixels).
pub fn centred(bounds: dc_capture::Area, ppp: f32, size: egui::Vec2) -> egui::Pos2 {
    let center = egui::pos2(
        (bounds.x() + bounds.width() as i32 / 2) as f32 / ppp,
        (bounds.y() + bounds.height() as i32 / 2) as f32 / ppp,
    );
    center - size / 2.0
}
