//! Dark theme, fonts, widgets and Russian UI strings shared by all windows
//! (docs/design/recording-windows.html is the reference).

mod error;
pub mod fonts;
pub mod icons;
pub mod panel;
pub mod settings_rows;
pub mod strings;
#[cfg(feature = "kittest")]
pub mod testing;
pub mod theme;
pub mod tokens;
pub mod widgets;

/// The egui of all windows: the panels of the effects draw with it (one version for all).
pub use egui;
pub use error::UiKitError;
pub use icons::Icon;

/// Sets our fonts and the dark theme on `ctx`; every window calls it once at its start.
pub fn install(ctx: &egui::Context) -> Result<(), UiKitError> {
    ctx.set_fonts(fonts::definitions(&dc_render::builtin_fonts_dir()?)?);
    theme::apply(ctx);
    Ok(())
}
