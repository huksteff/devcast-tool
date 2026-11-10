//! Where a popover of the HUD stands: 8 px above the bar under the button that opened it, or
//! under the bar when there is no room above (the HUD near the top of the screen).

use dc_ui_kit::tokens::{CONTROL, S2};
use eframe::egui::{self, Rect, Vec2, pos2, vec2};

use super::popover::{Kind, Open};
use crate::app::Shared;
use crate::borderless;

/// Between the bar and the popover's panel, points.
const GAP: f32 = 8.0;

/// The transparent window of `open` around a panel of `panel` size, its shadow margin
/// included: the panel stands `GAP` above the HUD's panel, under the button that opened it;
/// `bar` is the HUD's window rectangle.
pub fn window_rect(open: Open, panel: Vec2, bar: Rect) -> Rect {
    let top = borderless::panel_rect(bar).top();
    // «Ещё» is the last button: its menu ends where the button ends, as menus at the right
    // edge do, instead of reaching past the HUD.
    let left = match open.kind {
        Kind::More => open.anchor + CONTROL + S2 - panel.x,
        _ => open.anchor - S2,
    };
    let corner = pos2(bar.left() + left, top - GAP - panel.y);
    Rect::from_min_size(corner, panel) + borderless::margin()
}

/// The top edge of the HUD's monitor, points of the main window; `None` while the monitors
/// are unknown.
pub(super) fn ceiling(ctx: &egui::Context, shared: &Shared) -> Option<f32> {
    let settings = shared.store.get();
    let bounds = settings.hud.monitor(&shared.sources.monitors)?.bounds;
    Some(bounds.y() as f32 / ctx.native_pixels_per_point().unwrap_or(1.0))
}

/// [`window_rect`] on a monitor whose top edge is `ceiling`: a popover too tall to stand above
/// the HUD (the HUD is near the top of the screen) opens under it instead of over it.
pub fn placed(open: Open, panel: Vec2, bar: Rect, ceiling: Option<f32>) -> Rect {
    let above = window_rect(open, panel, bar);
    let top = borderless::panel_rect(above).top();
    match ceiling {
        Some(edge) if top < edge => {
            let under = borderless::panel_rect(bar).bottom() + GAP;
            above.translate(vec2(0.0, under - top))
        }
        _ => above,
    }
}

#[cfg(test)]
#[path = "place_tests.rs"]
mod tests;
