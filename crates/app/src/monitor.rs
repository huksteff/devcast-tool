//! Where the windows besides the HUD open: centred on the HUD's monitor; the timeline window
//! on another one.

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

/// Room left at the bottom of a monitor for the taskbar, points.
const TASKBAR: f32 = 56.0;

/// Corner of the timeline window of `size` when it has no place of its own yet: centred on
/// another monitor than the HUD's, where the editor is (DaVinci's second screen), or on one
/// monitor at its bottom, over the editor's bottom where the timeline was docked.
pub fn for_timeline(ctx: &egui::Context, shared: &Shared, size: egui::Vec2) -> Option<egui::Pos2> {
    let monitors = &shared.sources.monitors;
    let hud = shared.store.get().hud.monitor(monitors)?.bounds;
    let ppp = ctx.native_pixels_per_point().unwrap_or(1.0);
    let others = monitors.iter().map(|m| m.bounds).filter(|b| *b != hud);
    Some(timeline_corner(hud, others, ppp, size))
}

/// [`for_timeline`] of the monitor `hud` and the `others`, physical desktop pixels.
fn timeline_corner(
    hud: dc_capture::Area,
    mut others: impl Iterator<Item = dc_capture::Area>,
    ppp: f32,
    size: egui::Vec2,
) -> egui::Pos2 {
    if let Some(other) = others.next() {
        return centred(other, ppp, size);
    }
    let centre = centred(hud, ppp, size);
    let bottom = (hud.y() + hud.height() as i32) as f32 / ppp;
    egui::pos2(centre.x, bottom - TASKBAR - size.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_timeline_opens_on_another_monitor_or_at_the_bottom() {
        let left = dc_capture::Area::new(-1920, 120, 1920, 1080).unwrap();
        let main = dc_capture::Area::new(0, 0, 2560, 1440).unwrap();
        let size = egui::vec2(1280.0, 460.0);
        let corner = timeline_corner(left, [main].into_iter(), 1.0, size);
        assert_eq!(
            corner,
            egui::pos2(640.0, 490.0),
            "centred on the other monitor"
        );
        let corner = timeline_corner(main, std::iter::empty(), 1.0, size);
        assert_eq!(
            corner,
            egui::pos2(640.0, 1440.0 - 56.0 - 460.0),
            "one monitor: its bottom"
        );
    }
}
