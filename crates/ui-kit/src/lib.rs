//! Dark theme, fonts, widgets and Russian UI strings shared by all windows
//! (docs/design/recording-windows.html is the reference).

mod error;
pub mod fonts;
pub mod icons;
#[cfg(feature = "kittest")]
pub mod testing;
pub mod theme;
pub mod tokens;
pub mod widgets;

pub use error::UiKitError;
pub use icons::Icon;

/// Sets our fonts and the dark theme on `ctx`; every window calls it once at its start.
pub fn install(ctx: &egui::Context) -> Result<(), UiKitError> {
    ctx.set_fonts(fonts::definitions(&dc_render::builtin_fonts_dir()?)?);
    theme::apply(ctx);
    Ok(())
}
