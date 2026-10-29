//! Shape pass: every figure of a frame in one storage buffer, each layer one instanced draw. The
//! same instances drawn by `fs_mask` into a smaller texture make the masks that glows blur.

mod pack;

use std::ops::Range;

use dc_core::PixelSize;

use crate::gpu::Gpu;
use crate::pipeline::pipeline;
use crate::scene::{Camera, Shape};
use crate::uniform::Slots;

use pack::{FLOATS, glow_bounds, pack};

const BYTES: u64 = FLOATS as u64 * 4;
/// Floats of `Globals` of shapes.wgsl.
const GLOBALS: usize = 8;

/// The part of the frame a draw covers and how it maps to its render target.
#[derive(Debug, Clone, Copy)]
pub(crate) struct View {
    /// Top-left of the covered part, frame px.
    pub(crate) origin: [f32; 2],
    /// Size of the covered part, frame px; it fills the viewport.
    pub(crate) size: [f32; 2],
    /// Frame px per pixel of the render target.
    pub(crate) scale: f32,
}

pub(crate) struct ShapesPass {
    pipeline: wgpu::RenderPipeline,
    mask: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    /// Slot 0: the whole frame; then one per glow mask.
    views: Slots<GLOBALS>,
    storage: wgpu::Buffer,
    bind: wgpu::BindGroup,
    /// Figures of the frame being prepared, reused between frames.
    bytes: Vec<u8>,
}

impl ShapesPass {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::include_wgsl!("shapes.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shapes"),
            entries: &[
                Slots::<GLOBALS>::layout_entry(0, wgpu::ShaderStages::VERTEX),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let blend = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let views = Slots::new(gpu, "shapes");
        let storage = storage(gpu, BYTES * 64);
        let bind = bind(gpu, &layout, &views, &storage);
        Self {
            pipeline: pipeline(device, &module, &layout, ["vs", "fs"], blend),
            mask: pipeline(device, &module, &layout, ["vs", "fs_mask"], blend),
            layout,
            views,
            storage,
            bind,
            bytes: Vec::new(),
        }
    }

    /// Starts a `frame`-sized scene drawn into `target`.
    pub(crate) fn clear(&mut self, frame: PixelSize, target: PixelSize) {
        let (w, h) = (frame.width as f32, frame.height as f32);
        let whole = View {
            origin: [0.0; 2],
            size: [w, h],
            scale: w / target.width as f32,
        };
        self.bytes.clear();
        self.views.clear();
        self.views.push(&globals(whole));
    }

    /// Adds `shapes` seen through `camera` as the next instances; returns them.
    pub(crate) fn push(&mut self, shapes: &[Shape], camera: Camera) -> Range<u32> {
        let first = (self.bytes.len() as u64 / BYTES) as u32;
        for shape in shapes {
            let floats = pack(shape, camera);
            self.bytes
                .extend_from_slice(floats.map(f32::to_le_bytes).as_flattened());
        }
        first..first + shapes.len() as u32
    }

    /// Adds a view for a glow mask; returns its slot.
    pub(crate) fn push_view(&mut self, view: View) -> u32 {
        self.views.push(&globals(view))
    }

    /// Sends the pushed figures and views to the GPU.
    pub(crate) fn upload(&mut self, gpu: &Gpu) {
        let len = self.bytes.len() as u64;
        let mut rebind = self.views.upload(gpu);
        if len > self.storage.size() {
            self.storage = storage(gpu, len.next_power_of_two());
            rebind = true;
        }
        if rebind {
            self.bind = bind(gpu, &self.layout, &self.views, &self.storage);
        }
        if len > 0 {
            gpu.queue.write_buffer(&self.storage, 0, &self.bytes);
        }
    }

    pub(crate) fn record(&self, pass: &mut wgpu::RenderPass<'_>, instances: Range<u32>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[0]);
        pass.draw(0..4, instances);
    }

    /// Draws what is visible of figure `instance` in its glow colour through view `slot`.
    pub(crate) fn record_mask(&self, pass: &mut wgpu::RenderPass<'_>, instance: u32, slot: u32) {
        pass.set_pipeline(&self.mask);
        pass.set_bind_group(0, &self.bind, &[slot]);
        pass.draw(0..4, instance..instance + 1);
    }
}

/// Frame px that the glow of `shape` seen through `camera` reaches, left, top, right, bottom, and
/// its sigma in frame px; `None` without a glow.
pub(crate) fn glow_area(shape: &Shape, camera: Camera) -> Option<([f32; 4], f32)> {
    let glow = shape.glow.as_ref()?;
    let sigma = glow.sigma * camera.scale;
    Some((glow_bounds(&pack(shape, camera), sigma), sigma))
}

fn globals(view: View) -> [f32; GLOBALS] {
    let [w, h] = view.size;
    let [x, y] = view.origin;
    [w, h, view.scale, 0.0, x, y, 0.0, 0.0]
}

fn storage(gpu: &Gpu, size: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("shapes"),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn bind(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    views: &Slots<GLOBALS>,
    storage: &wgpu::Buffer,
) -> wgpu::BindGroup {
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shapes"),
        layout,
        entries: &[
            views.bind_entry(0),
            wgpu::BindGroupEntry {
                binding: 1,
                resource: storage.as_entire_binding(),
            },
        ],
    })
}
