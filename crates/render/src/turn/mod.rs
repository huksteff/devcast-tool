//! Turned text (`Text::rotation`): glyphon lays glyphs out upright only, so a turned text is drawn
//! upright into a layer of the target's size and laid over the frame turned around its box
//! centre, sampled bilinearly. The part of the upright text that can land in the frame may be
//! bigger than the target (a box larger than the frame, a deep zoom); it is then drawn in several
//! tiles, each into the layer and over the frame in turn.

use dc_core::PixelSize;

use crate::gpu::{Gpu, SRGB_VIEW};
use crate::pipeline::pipeline;
use crate::recorder::Recorder;
use crate::scene::{Camera, Text};
use crate::uniform::Slots;

/// Floats of `Turn` of turn.wgsl.
const UNIFORM: usize = 12;
/// Texels kept around a tile in the layer, so its bilinear samples at the edge read drawn text.
const APRON: f32 = 2.0;
/// Reach of a gaussian in sigmas: what lies further is invisible (as the blur pass).
const SIGMAS: f32 = 3.0;

/// One upright piece of a turned text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Tile {
    /// Upright target px of layer texel (0, 0); whole numbers, so glyphs land on the same
    /// pixels in every tile.
    pub(crate) origin: [f32; 2],
    /// Left, top, right, bottom of the upright text this tile shows, target px.
    pub(crate) area: [f32; 4],
}

pub(crate) struct TurnPass {
    composite: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    slots: Slots<UNIFORM>,
    layer: Option<Layer>,
    used: bool,
}

struct Layer {
    size: PixelSize,
    view: wgpu::TextureView,
    bind: wgpu::BindGroup,
}

impl TurnPass {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::include_wgsl!("turn.wgsl"));
        let fragment = wgpu::ShaderStages::FRAGMENT;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turn"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: fragment,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: fragment,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                Slots::<UNIFORM>::layout_entry(2, fragment),
            ],
        });
        let over = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        Self {
            composite: pipeline(device, &module, &layout, ["vs", "fs"], over),
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("turn"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            slots: Slots::new(gpu, "turn"),
            layer: None,
            used: false,
        }
    }

    /// Starts a frame.
    pub(crate) fn clear(&mut self) {
        self.slots.clear();
        self.used = false;
    }

    /// Plans `tile` of a text turned by `degrees` clockwise around `centre` (target px); returns
    /// its slot for [`Self::end`].
    pub(crate) fn push(&mut self, centre: [f32; 2], degrees: f32, tile: Tile) -> u32 {
        self.used = true;
        let (sin, cos) = degrees.to_radians().sin_cos();
        let [x, y] = tile.origin;
        let [l, t, r, b] = tile.area;
        let values = [centre[0], centre[1], cos, sin, x, y, 0.0, 0.0, l, t, r, b];
        self.slots.push(&values)
    }

    /// Sends the frame's slots; makes the layer when the target size changed.
    pub(crate) fn upload(&mut self, gpu: &Gpu, target: PixelSize) {
        if !self.used {
            return;
        }
        let grown = self.slots.upload(gpu);
        if grown || self.layer.as_ref().is_none_or(|l| l.size != target) {
            self.layer = Some(self.layer(gpu, target));
        }
    }

    /// What is recorded next goes into the layer, cleared.
    pub(crate) fn begin<'v>(&'v self, recorder: &mut Recorder<'v>) {
        if let Some(layer) = &self.layer {
            recorder.retarget(&layer.view, true);
        }
    }

    /// Back to `frame`; lays the tile of `slot` over it.
    pub(crate) fn end<'v>(
        &'v self,
        recorder: &mut Recorder<'v>,
        frame: &'v wgpu::TextureView,
        slot: u32,
    ) {
        let Some(layer) = &self.layer else {
            return;
        };
        recorder.retarget(frame, false);
        let pass = recorder.pass();
        pass.set_pipeline(&self.composite);
        pass.set_bind_group(0, &layer.bind, &[slot]);
        pass.draw(0..4, 0..1);
    }

    fn layer(&self, gpu: &Gpu, size: PixelSize) -> Layer {
        let view = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("turn layer"),
                size: wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SRGB_VIEW,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turn"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                self.slots.bind_entry(2),
            ],
        });
        Layer { size, view, bind }
    }
}

/// Whether `text` is turned at all: a whole number of turns draws it upright.
pub(crate) fn turned(text: &Text) -> bool {
    text.rotation.rem_euclid(360.0) != 0.0
}

/// Upright target px `text` seen through `camera` may paint — its box grown by the reach of the
/// plates' glow and of the shadow — and the centre of its box. `scale`: target px per frame px.
pub(crate) fn bounds(text: &Text, camera: Camera, scale: f32) -> ([f32; 4], [f32; 2]) {
    let k = scale * camera.scale;
    let [x, y] = camera.apply([text.rect.x, text.rect.y]).map(|v| v * scale);
    let [w, h] = [text.rect.w * k, text.rect.h * k];
    let glow = text
        .plate
        .as_ref()
        .and_then(|plate| plate.glow.as_ref())
        .map_or(0.0, |glow| SIGMAS * glow.sigma);
    let shadow = text.shadow.as_ref().map_or(0.0, |shadow| {
        SIGMAS * shadow.sigma + shadow.offset[0].abs().max(shadow.offset[1].abs())
    });
    let reach = glow.max(shadow) * k + APRON;
    let area = [x - reach, y - reach, x + w + reach, y + h + reach];
    (area, [x + w / 2.0, y + h / 2.0])
}

/// Tiles covering the part of upright `area` (target px) that lands in a `target`-sized frame
/// when turned by `degrees` clockwise around `centre`; none when nothing of it does. Each tile
/// with its apron fits the layer.
pub(crate) fn tiles(
    area: [f32; 4],
    centre: [f32; 2],
    degrees: f32,
    target: PixelSize,
) -> Vec<Tile> {
    let (w, h) = (target.width as f32, target.height as f32);
    let (sin, cos) = degrees.to_radians().sin_cos();
    let upright = |[x, y]: [f32; 2]| {
        let [dx, dy] = [x - centre[0], y - centre[1]];
        [
            centre[0] + cos * dx + sin * dy,
            centre[1] + cos * dy - sin * dx,
        ]
    };
    let corners = [[0.0, 0.0], [w, 0.0], [0.0, h], [w, h]].map(upright);
    let lo = |i: usize| corners.iter().map(|c| c[i]).fold(f32::MAX, f32::min) - APRON;
    let hi = |i: usize| corners.iter().map(|c| c[i]).fold(f32::MIN, f32::max) + APRON;
    let [l, t] = [area[0].max(lo(0)), area[1].max(lo(1))];
    let [r, b] = [area[2].min(hi(0)), area[3].min(hi(1))];
    if l >= r || t >= b {
        return Vec::new();
    }
    let step = [
        (w - 2.0 * APRON - 1.0).max(1.0),
        (h - 2.0 * APRON - 1.0).max(1.0),
    ];
    let columns = ((r - l) / step[0]).ceil() as usize;
    let rows = ((b - t) / step[1]).ceil() as usize;
    let mut tiles = Vec::with_capacity(columns * rows);
    for row in 0..rows {
        let y0 = t + step[1] * row as f32;
        for column in 0..columns {
            let x0 = l + step[0] * column as f32;
            tiles.push(Tile {
                origin: [x0.floor() - APRON, y0.floor() - APRON],
                area: [x0, y0, (x0 + step[0]).min(r), (y0 + step[1]).min(b)],
            });
        }
    }
    tiles
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
