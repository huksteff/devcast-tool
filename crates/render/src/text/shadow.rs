//! Shadow of a text layer (`Text::shadow`): its glyphs in the shadow colour, drawn into a scratch
//! texture of the blur pass and blurred there like the glow of a figure (decision 004).

use glyphon::{Buffer, Color, Metrics, TextArea, TextBounds, TextRenderer};

use super::{Placed, TextPass, fill};
use crate::error::RenderError;
use crate::gpu::Gpu;
use crate::scene::{Shadow, Text};

/// Where a shadow is drawn: the scratch area of the blur pass, target px.
pub(crate) struct ShadowArea {
    /// Target px of texel (0, 0).
    pub(crate) origin: [f32; 2],
    /// Target px per texel.
    pub(crate) down: f32,
}

impl TextPass {
    /// Target px the glyphs of `text` placed at `placed` cover, moved by the shadow's offset:
    /// left, top, right, bottom. The blur pass grows it by the blur.
    pub(crate) fn shadow_bounds(&self, index: usize, shadow: &Shadow, placed: &Placed) -> [f32; 4] {
        let buffer = &self.buffers[index];
        let [mut x0, mut y0, mut x1, mut y1] = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                x0 = x0.min(glyph.x);
                x1 = x1.max(glyph.x + glyph.w);
            }
            y0 = y0.min(run.line_top);
            y1 = y1.max(run.line_top + run.line_height);
        }
        let [x, y] = placed.origin;
        let [dx, dy] = shadow.offset.map(|v| v * placed.k);
        let clip = placed.clip;
        [
            (x + x0).max(clip.x) + dx,
            (y + y0).max(clip.y) + dy,
            (x + x1).min(clip.x + clip.w) + dx,
            (y + y1).min(clip.y + clip.h) + dy,
        ]
    }

    /// Lays out `text` again in its shadow colour and prepares it for drawing into `area`;
    /// returns the index of the shadow renderer for [`Self::record_shadow`].
    pub(crate) fn push_shadow(
        &mut self,
        gpu: &Gpu,
        text: &Text,
        shadow: &Shadow,
        placed: &Placed,
        area: &ShadowArea,
    ) -> Result<usize, RenderError> {
        let index = self.shadows_used;
        self.shadows_used += 1;
        if self.shadow_buffers.len() <= index {
            self.shadow_buffers
                .push(Buffer::new(&mut self.fonts, Metrics::new(1.0, 1.0)));
            self.shadows.push(TextRenderer::new(
                &mut self.atlas,
                &gpu.device,
                wgpu::MultisampleState::default(),
                None,
            ));
        }
        let buffer = &mut self.shadow_buffers[index];
        fill(
            &mut self.fonts,
            buffer,
            text,
            placed.clip,
            placed.k,
            Some(shadow.color),
        );
        let to_scratch = |v: f32, origin: f32| (v - origin) / area.down;
        let [dx, dy] = shadow.offset.map(|v| v * placed.k);
        let clip = placed.clip;
        let text_area = TextArea {
            buffer,
            left: to_scratch(placed.origin[0] + dx, area.origin[0]),
            top: to_scratch(placed.origin[1] + dy, area.origin[1]),
            scale: 1.0 / area.down,
            bounds: TextBounds {
                left: to_scratch(clip.x + dx, area.origin[0]).floor() as i32,
                top: to_scratch(clip.y + dy, area.origin[1]).floor() as i32,
                right: to_scratch(clip.x + clip.w + dx, area.origin[0]).ceil() as i32,
                bottom: to_scratch(clip.y + clip.h + dy, area.origin[1]).ceil() as i32,
            },
            default_color: Color::rgba(0, 0, 0, 0),
            custom_glyphs: &[],
        };
        self.shadows[index]
            .prepare(
                &gpu.device,
                &gpu.queue,
                &mut self.fonts,
                &mut self.atlas,
                &self.viewport,
                [text_area],
                &mut self.swash,
            )
            .map_err(RenderError::Text)?;
        Ok(index)
    }

    /// Draws shadow `index` into the scratch pass.
    pub(crate) fn record_shadow(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        index: usize,
    ) -> Result<(), RenderError> {
        self.shadows[index]
            .render(&self.atlas, &self.viewport, pass)
            .map_err(RenderError::TextDraw)
    }
}
