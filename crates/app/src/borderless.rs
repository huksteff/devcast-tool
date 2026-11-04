//! Windows that draw their own frame: the HUD, its popovers, the countdown, the area selection.
//! The floating ones (HUD, popovers, countdown) are transparent windows with their rounded
//! panel, border and shadow drawn by egui, as Cap does it (decision 020): the window is the
//! panel plus [`margin`] for the shadow.

use dc_ui_kit::theme::WINDOW_SHADOW;
use dc_ui_kit::tokens::SURFACE;
use eframe::egui::epaint::{Color32, RectShape, Shape, Stroke, StrokeKind};
use eframe::egui::{Margin, Rect, Ui, Vec2, ViewportBuilder};

/// A window without the system frame and the system shadow, above all windows, with no taskbar
/// button. No shadow: the windows have their own shape, and the shadow of a popover 8 px above
/// the HUD darkened the HUD; without it the HUD also has no light 1 px caption row on top.
/// egui-winit honours `has_shadow` on Windows only with its patch (decision 019).
pub fn window(title: &str) -> ViewportBuilder {
    ViewportBuilder::default()
        .with_icon(crate::logo::window_icon())
        .with_title(title)
        .with_decorations(false)
        .with_has_shadow(false)
        .with_always_on_top()
        .with_taskbar(false)
}

/// [`window`] whose pixels outside the drawn panel show what is below. The patched egui-winit
/// creates it without a redirection bitmap (decision 020).
pub fn floating(title: &str) -> ViewportBuilder {
    window(title).with_transparent(true)
}

/// Room around the panel of a floating window for its shadow, points.
pub fn margin() -> Margin {
    let m = WINDOW_SHADOW.margin();
    Margin {
        left: m.left.ceil() as i8,
        right: m.right.ceil() as i8,
        top: m.top.ceil() as i8,
        bottom: m.bottom.ceil() as i8,
    }
}

/// Size of the floating window around a panel of `panel` size, points.
pub fn outer_size(panel: Vec2) -> Vec2 {
    panel + margin().sum()
}

/// The panel in a floating window whose rectangle is `window`.
pub fn panel_rect(window: Rect) -> Rect {
    window - margin()
}

/// The panel at `rect` with corners of `radius`: its shadow, fill and 1 px border inside the
/// edge.
pub fn panel(rect: Rect, radius: u8, fill: Color32, border: Color32) -> Shape {
    Shape::Vec(vec![
        WINDOW_SHADOW.as_shape(rect, radius).into(),
        RectShape::new(
            rect,
            radius,
            fill,
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        )
        .into(),
    ])
}

/// What every window starts its frame with: nothing. eframe has one clear colour for all
/// windows, and outside its drawn panel a floating window (HUD, popover, countdown) must show
/// the desktop (decision 020). The opaque windows paint their background with [`opaque`].
pub fn clear_color() -> [f32; 4] {
    Color32::TRANSPARENT.to_normalized_gamma_f32()
}

/// Background of an opaque window (settings, editor, area selection) over the whole window:
/// the clear colour is transparent, and a pixel left at it shows the window's white redirection
/// bitmap through the composition swapchain.
pub fn opaque(ui: &Ui) {
    ui.painter().rect_filled(ui.max_rect(), 0, SURFACE);
}

/// Shows a window made hidden (`with_visible(false)`) once its first frame is drawn: winit shows a
/// new window before it gives it its size and place, a flash at the system's default place on
/// the main screen. egui counts frames per window from zero again after it closes.
pub fn show_when_drawn(ctx: &eframe::egui::Context) {
    if ctx.cumulative_frame_nr() == 0 {
        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(true));
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Rect, WindowLevel, pos2, vec2};

    #[test]
    fn no_frame_no_shadow_on_top() {
        let builder = super::window("Звук");
        assert_eq!(builder.decorations, Some(false));
        assert_eq!(builder.has_shadow, Some(false));
        assert_eq!(builder.window_level, Some(WindowLevel::AlwaysOnTop));
        assert_eq!(builder.taskbar, Some(false));
        assert_eq!(builder.title.as_deref(), Some("Звук"));
        assert_eq!(builder.transparent, None);
    }

    #[test]
    fn floating_windows_are_transparent() {
        let builder = super::floating("Звук");
        assert_eq!(builder.transparent, Some(true));
        assert_eq!(builder.has_shadow, Some(false));
        assert_eq!(builder.decorations, Some(false));
    }

    /// The shadow fits the margin, and the margin below is the 8 px gap above the HUD: the
    /// popover's window does not reach over the bar.
    #[test]
    fn margin_holds_the_shadow_and_ends_at_the_gap() {
        let margin = super::margin();
        let shadow = dc_ui_kit::theme::WINDOW_SHADOW.margin();
        assert!(f32::from(margin.left) >= shadow.left);
        assert!(f32::from(margin.right) >= shadow.right);
        assert!(f32::from(margin.top) >= shadow.top);
        assert!(f32::from(margin.bottom) >= shadow.bottom);
        assert_eq!(margin.bottom, 8);
        let window = Rect::from_min_size(pos2(100.0, 200.0), super::outer_size(vec2(300.0, 48.0)));
        let panel = super::panel_rect(window);
        assert_eq!(panel.size(), vec2(300.0, 48.0));
        assert_eq!(
            panel.min,
            window.min + vec2(margin.left.into(), margin.top.into())
        );
    }

    /// Outside their drawn panel the floating windows show the desktop (decision 020).
    #[test]
    fn windows_are_cleared_to_transparent() {
        assert_eq!(super::clear_color(), [0.0; 4]);
    }
}
