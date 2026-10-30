//! Width of text on the CPU (decision 016): the same fonts and shaping as the text pass, so
//! `evaluate` can break caption lines and pages exactly as the frame will show them.

use std::path::Path;

use glyphon::{Buffer, FontSystem, Metrics, Shaping};

use super::fonts;
use crate::error::RenderError;
use crate::scene::Font;

pub struct TextMeasure {
    fonts: FontSystem,
    /// Made at the first measure: cosmic-text needs a font to make one.
    buffer: Option<Buffer>,
}

impl TextMeasure {
    /// Without fonts: load them with [`Self::load_fonts`] or [`Self::load_builtin_fonts`].
    pub fn new() -> Self {
        Self {
            fonts: fonts::font_system(),
            buffer: None,
        }
    }

    /// The same as [`crate::Compositor::load_fonts`].
    pub fn load_fonts(&mut self, dir: &Path) -> Result<(), RenderError> {
        fonts::load(&mut self.fonts, dir)
    }

    /// The same as [`crate::Compositor::load_builtin_fonts`].
    pub fn load_builtin_fonts(&mut self) -> Result<(), RenderError> {
        fonts::load(&mut self.fonts, &fonts::builtin_dir()?)
    }

    /// Whether font `family` is loaded: [`RenderError::NoFont`] says what to do if not.
    pub fn check(&self, family: &str) -> Result<(), RenderError> {
        fonts::check_family(&self.fonts, family)
    }

    /// Advance width of `text` set on one line in `font`, px; spaces at its ends count.
    pub fn width(&mut self, text: &str, font: &Font) -> Result<f32, RenderError> {
        fonts::check_family(&self.fonts, &font.family)?;
        let system = &mut self.fonts;
        let buffer = self
            .buffer
            .get_or_insert_with(|| Buffer::new(system, Metrics::new(1.0, 1.0)));
        buffer.set_metrics_and_size(Metrics::new(font.size, font.size), None, None);
        buffer.set_text(text, &fonts::attrs(font), Shaping::Advanced, None);
        buffer.shape_until_scroll(system, false);
        Ok(buffer
            .layout_runs()
            .flat_map(|run| run.glyphs.iter())
            .map(|glyph| glyph.w)
            .sum())
    }
}

impl Default for TextMeasure {
    fn default() -> Self {
        Self::new()
    }
}
