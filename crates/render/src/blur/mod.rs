//! Separable gaussian blur with downsampling (decision 004): region blur with tint and rounded
//! corners, and the glow of a figure (its mask blurred under it). Both work in a part of two
//! scratch textures of the target's size.
//!
//! The scratch holds the area 2^k times smaller, with k chosen so that sigma stays at least 3
//! texels: the kernel is 13–37 taps whatever the sigma, and a big blur costs as little as a small
//! one. For the default glow (σ 10.5 px at 1080p) that is half the frame, as in decision 004.

mod scratch;

use dc_core::PixelSize;

use crate::error::RenderError;
use crate::gpu::Gpu;
use crate::pipeline::pipeline;
use crate::recorder::Recorder;
use crate::scene::Blur;
use crate::shapes::{ShapesPass, View};
use crate::text::{ShadowArea, TextPass};
use crate::uniform::Slots;

use scratch::{ONE, Scratch, bind_group, scratch, scratch_pass};

/// Floats of `Params` of blur.wgsl.
pub(super) const PARAMS: usize = 24;
/// Largest downsample: sigma up to ~1000 px still fits the kernel.
const MAX_DOWN: f32 = 64.0;

/// One blur of a frame being drawn, in target px.
pub(crate) struct Op {
    /// Dynamic offset of its parameters.
    slot: u32,
    /// Used texels of the scratch.
    texels: [u32; 2],
    kind: Kind,
}

enum Kind {
    Region,
    /// Figure `instance` drawn as a mask through view `view` of the shape pass.
    Glow {
        instance: u32,
        view: u32,
    },
    /// Shadow `renderer` of the text pass drawn into the scratch texture.
    TextShadow {
        renderer: usize,
    },
}

/// The part of the target a blur reads, the downsample and sigma in scratch texels.
struct Area {
    origin: [f32; 2],
    texels: [u32; 2],
    down: f32,
    sigma: f32,
}

pub(crate) struct BlurPass {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    down: wgpu::RenderPipeline,
    horizontal: wgpu::RenderPipeline,
    vertical: wgpu::RenderPipeline,
    region: wgpu::RenderPipeline,
    glow: wgpu::RenderPipeline,
    params: Slots<PARAMS>,
    scratch: Scratch,
    /// Target px per frame px and the target size, for the frame being prepared.
    scale: f32,
    target: PixelSize,
}

impl BlurPass {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::include_wgsl!("blur.wgsl"));
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blur"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    1,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
                Slots::<PARAMS>::layout_entry(
                    2,
                    wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ),
            ],
        });
        let blend = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let make = |stages, blend| pipeline(device, &module, &layout, stages, blend);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blur"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let params = Slots::new(gpu, "blur");
        let scratch = scratch(gpu, &layout, &sampler, &params, ONE);
        Self {
            down: make(["vs_full", "fs_down"], None),
            horizontal: make(["vs_full", "fs_blur_h"], None),
            vertical: make(["vs_full", "fs_blur_v"], None),
            region: make(["vs_quad", "fs_region"], blend),
            glow: make(["vs_quad", "fs_glow"], blend),
            sampler,
            params,
            layout,
            scratch,
            scale: 1.0,
            target: ONE,
        }
    }

    /// Starts a `frame`-sized scene drawn into `target`.
    pub(crate) fn clear(&mut self, frame: PixelSize, target: PixelSize) {
        self.params.clear();
        self.scale = target.width as f32 / frame.width as f32;
        self.target = target;
    }

    /// Prepares the blur of `blur` (frame px, camera applied); `None` when it is off the frame.
    pub(crate) fn push_region(&mut self, blur: &Blur) -> Option<Op> {
        let k = self.scale;
        let r = blur.rect;
        let rect = [r.x * k, r.y * k, (r.x + r.w) * k, (r.y + r.h) * k];
        let pad = (3.0 * blur.sigma * k).ceil();
        let area = self.area(grow(rect, pad), blur.sigma * k)?;
        overlaps(rect, self.target)?;
        let half = [(rect[2] - rect[0]) / 2.0, (rect[3] - rect[1]) / 2.0];
        let center = [rect[0] + half[0], rect[1] + half[1]];
        let [tr, tg, tb, ta] = blur.tint.linear();
        let values = self.values(&area, rect, [tr, tg, tb, ta], center, half, blur.radius * k);
        Some(Op {
            slot: self.params.push(&values),
            texels: area.texels,
            kind: Kind::Region,
        })
    }

    /// Prepares the glow of figure `instance` reaching `bounds` (frame px) with `sigma` (frame
    /// px); its mask view goes to `shapes`. `None` when it is off the frame.
    pub(crate) fn push_glow(
        &mut self,
        shapes: &mut ShapesPass,
        instance: u32,
        bounds: [f32; 4],
        sigma: f32,
    ) -> Option<Op> {
        let k = self.scale;
        let area = self.area(bounds.map(|v| v * k), sigma * k)?;
        let [w, h] = area.texels.map(|t| t as f32 * area.down / k);
        let view = shapes.push_view(View {
            origin: area.origin.map(|v| v / k),
            size: [w, h],
            scale: area.down / k,
        });
        let quad = [
            area.origin[0],
            area.origin[1],
            area.origin[0] + area.texels[0] as f32 * area.down,
            area.origin[1] + area.texels[1] as f32 * area.down,
        ];
        let values = self.values(&area, quad, [0.0; 4], [0.0; 2], [0.0; 2], 0.0);
        Some(Op {
            slot: self.params.push(&values),
            texels: area.texels,
            kind: Kind::Glow { instance, view },
        })
    }

    /// A text shadow over target-px `bounds` blurred by `sigma` target px: `prepare` lays the
    /// glyphs out for the scratch area it is given and returns the shadow renderer. `None` when
    /// the shadow is outside the target.
    pub(crate) fn push_text_shadow(
        &mut self,
        bounds: [f32; 4],
        sigma: f32,
        prepare: impl FnOnce(&ShadowArea) -> Result<usize, RenderError>,
    ) -> Result<Option<Op>, RenderError> {
        let Some(area) = self.area(bounds, sigma) else {
            return Ok(None);
        };
        let renderer = prepare(&ShadowArea {
            origin: area.origin,
            down: area.down,
        })?;
        let quad = [
            area.origin[0],
            area.origin[1],
            area.origin[0] + area.texels[0] as f32 * area.down,
            area.origin[1] + area.texels[1] as f32 * area.down,
        ];
        let values = self.values(&area, quad, [0.0; 4], [0.0; 2], [0.0; 2], 0.0);
        Ok(Some(Op {
            slot: self.params.push(&values),
            texels: area.texels,
            kind: Kind::TextShadow { renderer },
        }))
    }

    /// Sends the prepared parameters; makes scratch textures of the target's size when the frame
    /// has blurs (`any`) and they are of another size.
    pub(crate) fn upload(&mut self, gpu: &Gpu, any: bool) {
        let rebind = self.params.upload(gpu);
        let size = if any && self.scratch.size != self.target {
            self.target
        } else {
            self.scratch.size
        };
        if rebind || size != self.scratch.size {
            self.scratch = scratch(gpu, &self.layout, &self.sampler, &self.params, size);
        }
    }

    /// Records `op` reading `source` (a bind group of [`Self::bind`] over the target), ending
    /// with its composite in the target's pass.
    pub(crate) fn record(
        &self,
        recorder: &mut Recorder<'_>,
        source: &wgpu::BindGroup,
        (shapes, text): (&ShapesPass, &TextPass),
        op: &Op,
    ) -> Result<(), RenderError> {
        let [a, b] = &self.scratch.views;
        let [bind_a, bind_b] = &self.scratch.binds;
        let encoder = recorder.encoder();
        let composite = match op.kind {
            Kind::Region => {
                self.step(encoder, a, &self.down, source, op);
                &self.region
            }
            Kind::Glow { instance, view } => {
                let mut pass = scratch_pass(encoder, a, op.texels, true);
                shapes.record_mask(&mut pass, instance, view);
                &self.glow
            }
            Kind::TextShadow { renderer } => {
                let mut pass = scratch_pass(encoder, a, op.texels, true);
                // glyphon places glyphs by the viewport of the whole texture (target size); the
                // scissor keeps them in the area.
                let (w, h) = (self.target.width as f32, self.target.height as f32);
                pass.set_viewport(0.0, 0.0, w, h, 0.0, 1.0);
                text.record_shadow(&mut pass, renderer)?;
                &self.glow
            }
        };
        self.step(encoder, b, &self.horizontal, bind_a, op);
        self.step(encoder, a, &self.vertical, bind_b, op);
        let pass = recorder.pass();
        pass.set_pipeline(composite);
        pass.set_bind_group(0, bind_a, &[op.slot]);
        pass.draw(0..4, 0..1);
        Ok(())
    }

    /// A bind group sampling `view` (the target) for [`Self::record`].
    pub(crate) fn bind(&self, gpu: &Gpu, view: &wgpu::TextureView) -> wgpu::BindGroup {
        bind_group(gpu, &self.layout, view, &self.sampler, &self.params)
    }

    fn step(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        pipeline: &wgpu::RenderPipeline,
        source: &wgpu::BindGroup,
        op: &Op,
    ) {
        let mut pass = scratch_pass(encoder, target, op.texels, false);
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, source, &[op.slot]);
        pass.draw(0..4, 0..1);
    }

    /// The part of the target around `rect` (target px) that a blur of `sigma` (target px)
    /// reads, clipped to the target; `None` when nothing of it is in the target.
    fn area(&self, rect: [f32; 4], sigma: f32) -> Option<Area> {
        let [x0, y0, x1, y1] = overlaps(rect, self.target)?;
        let [x0, y0, x1, y1] = [x0.floor(), y0.floor(), x1.ceil(), y1.ceil()];
        let down = (sigma / 3.0).max(1.0).log2().floor().exp2().min(MAX_DOWN);
        let texels = [
            ((x1 - x0) / down).ceil() as u32,
            ((y1 - y0) / down).ceil() as u32,
        ];
        Some(Area {
            origin: [x0, y0],
            texels,
            down,
            sigma: sigma / down,
        })
    }

    fn values(
        &self,
        area: &Area,
        quad: [f32; 4],
        tint: [f32; 4],
        center: [f32; 2],
        half: [f32; 2],
        radius: f32,
    ) -> [f32; PARAMS] {
        let [ox, oy] = area.origin;
        let [tx, ty] = area.texels.map(|t| t as f32);
        let (w, h) = (self.target.width as f32, self.target.height as f32);
        let rows = [
            [ox, oy, area.down, area.sigma],
            [tx, ty, 0.0, 0.0],
            quad,
            tint,
            [center[0], center[1], half[0], half[1]],
            [radius, w, h, 0.0],
        ];
        let mut out = [0.0; PARAMS];
        out.copy_from_slice(rows.as_flattened());
        out
    }
}

/// The part of `rect` (left, top, right, bottom) inside the target; `None` when it is empty.
fn overlaps(rect: [f32; 4], target: PixelSize) -> Option<[f32; 4]> {
    let (w, h) = (target.width as f32, target.height as f32);
    let clipped = [
        rect[0].max(0.0),
        rect[1].max(0.0),
        rect[2].min(w),
        rect[3].min(h),
    ];
    (clipped[0] < clipped[2] && clipped[1] < clipped[3]).then_some(clipped)
}

fn grow(rect: [f32; 4], by: f32) -> [f32; 4] {
    [rect[0] - by, rect[1] - by, rect[2] + by, rect[3] + by]
}
