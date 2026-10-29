//! Text pass: glyphon over a font system that holds only the fonts loaded from our folders
//! (decision 005), so a frame is the same on every PC. Plates and underlines are figures of the
//! shape pass; each text layer has its own glyphon renderer, so layers keep their paint order.

mod fonts;
mod layout;
mod measure;
mod shadow;

pub(crate) use shadow::ShadowArea;

pub use fonts::BUILTIN_FONTS;
pub(crate) use fonts::builtin_dir;
pub use measure::TextMeasure;

use std::path::Path;

use dc_core::PixelSize;
use glyphon::cosmic_text::UnderlineStyle;
use glyphon::{
    Attrs, Buffer, Cache, Color, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer, Viewport, Wrap,
};

use crate::error::RenderError;
use crate::gpu::{Gpu, SRGB_VIEW};
use crate::scene::{Align, Camera, PxRect, Rgba, Shape, Text};

/// Where a text layer was laid out, target px: the top-left of its buffer, the box that cuts it
/// and target px per px of its own lengths.
pub(crate) struct Placed {
    pub(crate) origin: [f32; 2],
    pub(crate) clip: PxRect,
    pub(crate) k: f32,
}

/// Figures of a laid-out text layer: plates go under its glyphs, underlines over them.
pub(crate) struct Decor {
    pub(crate) plates: Vec<Shape>,
    pub(crate) underlines: Vec<Shape>,
}

pub(crate) struct TextPass {
    fonts: FontSystem,
    swash: SwashCache,
    atlas: TextAtlas,
    viewport: Viewport,
    /// One per text layer of a frame, reused between frames.
    renderers: Vec<TextRenderer>,
    buffers: Vec<Buffer>,
    used: usize,
    /// The same for the shadows of a frame.
    shadows: Vec<TextRenderer>,
    shadow_buffers: Vec<Buffer>,
    shadows_used: usize,
    /// Target px per frame px.
    scale: f32,
}

impl TextPass {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        let cache = Cache::new(&gpu.device);
        Self {
            fonts: fonts::font_system(),
            swash: SwashCache::new(),
            atlas: TextAtlas::new(&gpu.device, &gpu.queue, &cache, SRGB_VIEW),
            viewport: Viewport::new(&gpu.device, &cache),
            renderers: Vec::new(),
            buffers: Vec::new(),
            used: 0,
            shadows: Vec::new(),
            shadow_buffers: Vec::new(),
            shadows_used: 0,
            scale: 1.0,
        }
    }

    /// Loads every `.ttf` and `.otf` file of `dir` (not its subfolders).
    pub(crate) fn load_fonts(&mut self, dir: &Path) -> Result<(), RenderError> {
        fonts::load(&mut self.fonts, dir)
    }

    /// Starts a `frame`-sized scene drawn into `target`.
    pub(crate) fn clear(&mut self, gpu: &Gpu, frame: PixelSize, target: PixelSize) {
        self.used = 0;
        self.shadows_used = 0;
        self.scale = target.width as f32 / frame.width as f32;
        self.viewport.update(
            &gpu.queue,
            Resolution {
                width: target.width,
                height: target.height,
            },
        );
    }

    /// Lays out `text` seen through `camera` and prepares its glyphs; returns the index of its
    /// renderer, its plates and underlines (frame px, camera applied) and where it went (for
    /// its shadow, [`Self::push_shadow`]).
    pub(crate) fn push(
        &mut self,
        gpu: &Gpu,
        text: &Text,
        camera: Camera,
    ) -> Result<(usize, Decor, Placed), RenderError> {
        fonts::check_family(&self.fonts, &text.font.family)?;
        let k = self.scale * camera.scale;
        let [x, y] = camera.apply([text.rect.x, text.rect.y]);
        let rect = PxRect {
            x: x * self.scale,
            y: y * self.scale,
            w: text.rect.w * k,
            h: text.rect.h * k,
        };
        let index = self.used;
        self.used += 1;
        if self.buffers.len() <= index {
            self.buffers
                .push(Buffer::new(&mut self.fonts, Metrics::new(1.0, 1.0)));
            self.renderers.push(TextRenderer::new(
                &mut self.atlas,
                &gpu.device,
                wgpu::MultisampleState::default(),
                None,
            ));
        }
        let buffer = &mut self.buffers[index];
        fill(&mut self.fonts, buffer, text, rect, k, None);
        let [left, top] = match &text.beside {
            None => layout::centred(buffer, text, rect, k),
            Some(beside) => {
                let point = camera.apply(beside.point).map(|v| v * self.scale);
                layout::beside_point(buffer, text, beside, point, rect, [k, self.scale])
            }
        };
        let decor = layout::decor(buffer, text, [left, top], rect, k, self.scale);
        let area = TextArea {
            buffer,
            left,
            top,
            scale: 1.0,
            bounds: TextBounds {
                left: rect.x.floor() as i32,
                top: rect.y.floor() as i32,
                right: (rect.x + rect.w).ceil() as i32,
                bottom: (rect.y + rect.h).ceil() as i32,
            },
            default_color: Color::rgba(255, 255, 255, 255),
            custom_glyphs: &[],
        };
        self.renderers[index]
            .prepare(
                &gpu.device,
                &gpu.queue,
                &mut self.fonts,
                &mut self.atlas,
                &self.viewport,
                [area],
                &mut self.swash,
            )
            .map_err(RenderError::Text)?;
        Ok((
            index,
            decor,
            Placed {
                origin: [left, top],
                clip: rect,
                k,
            },
        ))
    }

    pub(crate) fn record(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        index: usize,
    ) -> Result<(), RenderError> {
        self.renderers[index]
            .render(&self.atlas, &self.viewport, pass)
            .map_err(RenderError::TextDraw)
    }

    /// Frees atlas space of glyphs the last frame did not use.
    pub(crate) fn trim(&mut self) {
        self.atlas.trim();
    }
}

/// Sets `text` into `buffer` for a target-px `rect` at `k` target px per text px: wrapped at the
/// box width minus padding (one line for `beside`), each run in its colour or all in `colour`
/// (a shadow), faded by the text's opacity.
fn fill(
    fonts: &mut FontSystem,
    buffer: &mut Buffer,
    text: &Text,
    rect: PxRect,
    k: f32,
    colour: Option<Rgba>,
) {
    let size = text.font.size * k;
    let width = match text.beside {
        Some(_) => None,
        None => Some((rect.w - 2.0 * text.padding * k).max(0.0)),
    };
    buffer.set_metrics_and_size(Metrics::new(size, size * text.line_height), width, None);
    buffer.set_wrap(Wrap::WordOrGlyph);
    let base = attrs(text);
    let spans = text.runs.iter().map(|run| {
        let [r, g, b, a] = colour.unwrap_or(run.color).0;
        let alpha = (f32::from(a) * text.opacity).round() as u8;
        (
            run.text.as_str(),
            base.clone().color(Color::rgba(r, g, b, alpha)),
        )
    });
    buffer.set_rich_text(spans, &base, Shaping::Advanced, Some(align(text.align)));
    buffer.shape_until_scroll(fonts, false);
}

fn attrs(text: &Text) -> Attrs<'_> {
    let mut attrs = fonts::attrs(&text.font);
    if text.underline {
        attrs = attrs.underline(UnderlineStyle::Single);
    }
    attrs
}

fn align(align: Align) -> glyphon::cosmic_text::Align {
    match align {
        Align::Left => glyphon::cosmic_text::Align::Left,
        Align::Center => glyphon::cosmic_text::Align::Center,
        Align::Right => glyphon::cosmic_text::Align::Right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Font;
    use glyphon::fontdb;

    const FAMILIES: [&str; 5] = [
        "Inter",
        "Montserrat",
        "Roboto",
        "PT Serif",
        "JetBrains Mono",
    ];
    const PANGRAMS: &str = "Съешь же ещё этих мягких французских булок, да выпей чаю.         ЁЖЩЪЫЭЮЯ ёжщъыэюя The quick brown fox jumps over the lazy dog 0123456789 «»—–…№";

    fn fonts() -> FontSystem {
        let mut db = fontdb::Database::new();
        db.load_fonts_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/fonts"));
        FontSystem::new_with_locale_and_db("ru-RU".into(), db)
    }

    /// Text, glyph id and font family of every glyph of `line` set in `font`.
    fn glyphs(fonts: &mut FontSystem, font: Font, line: &str) -> Vec<(String, u16, String)> {
        let text = Text {
            rect: PxRect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            runs: Vec::new(),
            font,
            underline: false,
            align: Align::Left,
            line_height: 1.4,
            padding: 0.0,
            plate: None,
            opacity: 1.0,
            beside: None,
            shadow: None,
        };
        let mut buffer = Buffer::new(fonts, Metrics::new(32.0, 44.0));
        buffer.set_text(line, &attrs(&text), Shaping::Advanced, None);
        buffer.shape_until_scroll(fonts, false);
        let mut out = Vec::new();
        for run in buffer.layout_runs() {
            for g in run.glyphs {
                let face = fonts.db().face(g.font_id).unwrap();
                let family = face.families[0].0.clone();
                out.push((run.text[g.start..g.end].to_owned(), g.glyph_id, family));
            }
        }
        out
    }

    /// No «tofu»: every letter of the Cyrillic and Latin pangrams has a real glyph of the
    /// requested family in each style.
    #[test]
    fn built_in_fonts_have_every_letter_in_every_style() {
        let mut fonts = fonts();
        for family in FAMILIES {
            for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
                let font = Font {
                    family: family.into(),
                    size: 32.0,
                    bold,
                    italic,
                };
                for (text, id, face) in glyphs(&mut fonts, font, PANGRAMS) {
                    let what = format!("{family} bold {bold} italic {italic}: «{text}»");
                    assert_ne!(id, 0, "{what} is .notdef");
                    assert_eq!(face, family, "{what} came from another font");
                }
            }
        }
    }

    #[test]
    fn emoji_come_from_noto_color_emoji() {
        let mut fonts = fonts();
        let font = Font {
            family: "Inter".into(),
            size: 32.0,
            bold: false,
            italic: false,
        };
        let emoji: Vec<_> = glyphs(&mut fonts, font, "🎬🎮✅🔥👍😀")
            .into_iter()
            .filter(|(text, _, _)| !text.trim().is_empty())
            .collect();
        assert_eq!(emoji.len(), 6, "{emoji:?}");
        for (text, id, face) in emoji {
            assert_ne!(id, 0, "«{text}» is .notdef");
            assert_eq!(face, "Noto Color Emoji", "«{text}»");
        }
    }

    #[test]
    fn built_in_families_have_the_names_scenes_use() {
        let fonts = fonts();
        let mut families: Vec<_> = fonts
            .db()
            .faces()
            .map(|f| f.families[0].0.clone())
            .collect();
        families.sort();
        families.dedup();
        assert_eq!(
            families,
            [
                "Inter",
                "JetBrains Mono",
                "Montserrat",
                "Noto Color Emoji",
                "PT Serif",
                "Roboto"
            ]
        );
    }
}
