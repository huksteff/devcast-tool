//! Windows that draw their own frame: the HUD, its popovers, the countdown, the area selection.

use eframe::egui::ViewportBuilder;

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

#[cfg(test)]
mod tests {
    use eframe::egui::WindowLevel;

    #[test]
    fn no_frame_no_shadow_on_top() {
        let builder = super::window("Звук");
        assert_eq!(builder.decorations, Some(false));
        assert_eq!(builder.has_shadow, Some(false));
        assert_eq!(builder.window_level, Some(WindowLevel::AlwaysOnTop));
        assert_eq!(builder.taskbar, Some(false));
        assert_eq!(builder.title.as_deref(), Some("Звук"));
    }
}
