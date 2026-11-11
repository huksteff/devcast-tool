//! Where a popover of the HUD stands: 8 px above the bar under the button that opened it, or
//! under the bar when there is more room there (the HUD near the top of the screen); always
//! inside the work area of the HUD's monitor, as tall as the room lets it.

use dc_ui_kit::tokens::{CONTROL, S2};
use eframe::egui::{self, Rect, Vec2, pos2, vec2};

use super::popover::{Kind, MAX_HEIGHT, Open};
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

/// The work area (no taskbar) of the monitor the HUD whose window is `bar` stands on, points of
/// the main window; from the main window's pass. `None` when Windows does not tell it.
pub fn room(ctx: &egui::Context, bar: Rect) -> Option<Rect> {
    let ppp = ctx.pixels_per_point();
    let px = bar * ppp;
    let rect = (
        px.left().round() as i32,
        px.top().round() as i32,
        px.width().round() as u32,
        px.height().round() as u32,
    );
    let (x, y, width, height) = dc_capture::win::hud_window::work_area(rect)
        .inspect_err(|e| tracing::warn!("поповер может выйти за экран: {e}"))
        .ok()?;
    let corner = pos2(x as f32, y as f32);
    Some(Rect::from_min_size(corner, vec2(width as f32, height as f32)) / ppp)
}

/// Space for a panel above and under the HUD's bar in `room`, points.
fn spaces(bar: Rect, room: Rect) -> (f32, f32) {
    let hud = borderless::panel_rect(bar);
    let above = hud.top() - GAP - room.top();
    let under = room.bottom() - hud.bottom() - GAP;
    (above.max(0.0), under.max(0.0))
}

/// The tallest panel a popover may lay out: [`MAX_HEIGHT`], or less when neither above nor
/// under the bar has that much room.
pub fn tallest(bar: Rect, room: Option<Rect>) -> f32 {
    room.map_or(MAX_HEIGHT, |room| {
        let (above, under) = spaces(bar, room);
        above.max(under).min(MAX_HEIGHT)
    })
}

/// [`window_rect`] kept in `room`: above the bar when the panel fits there, else under it when
/// it fits there, else on the roomier side with the panel as tall as that side lets it. Then
/// moved sideways into `room`. Without `room` it is just [`window_rect`].
pub fn placed(open: Open, panel: Vec2, bar: Rect, room: Option<Rect>) -> Rect {
    let above = window_rect(open, panel, bar);
    let Some(room) = room else {
        return above;
    };
    let (space_above, space_under) = spaces(bar, room);
    let up = panel.y <= space_above || (panel.y > space_under && space_above >= space_under);
    let height = panel.y.min(if up { space_above } else { space_under });
    let hud = borderless::panel_rect(bar);
    let top = if up {
        hud.top() - GAP - height
    } else {
        hud.bottom() + GAP
    };
    let wanted = borderless::panel_rect(above).left();
    let left = wanted.min(room.right() - panel.x).max(room.left());
    Rect::from_min_size(pos2(left, top), vec2(panel.x, height)) + borderless::margin()
}

#[cfg(test)]
#[path = "place_tests.rs"]
mod tests;
