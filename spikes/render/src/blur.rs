//! Separable gaussian blur: region blur with tint and rounded corners, and glow composite.

use bytemuck::{Pod, Zeroable};

use crate::gpu::{self, Gpu};

/// Uniform slot size (min dynamic offset alignment is 256 bytes).
const SLOT: u64 = 256;
const SLOTS: u64 = 32;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
struct Params {
    src_rect: [f32; 4],
    step: [f32; 4],
    tint: [f32; 4],
    rect: [f32; 4],
}

pub struct Blur {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params: wgpu::Buffer,
    copy: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    region: wgpu::RenderPipeline,
    glow: wgpu::RenderPipeline,
    pub background: wgpu::RenderPipeline,
    next_slot: std::cell::Cell<u64>,
}

/// A region blur: rectangle in pixels, sigma in pixels, tint (linear rgb + strength), corner radius.
pub struct Region {
    pub rect: [f32; 4],
    pub sigma: f32,
    pub tint: [f32; 4],
    pub radius: f32,
}

impl Blur {
    pub fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blur"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blur.wgsl").into()),
        });
        let tex = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blur"),
            entries: &[
                entry(0, tex),
                entry(1, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
                entry(2, wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(size_of::<Params>() as u64),
                }),
            ],
        });
        let with_group = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blur"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let empty = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("background"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let premul = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        Self {
            copy: gpu::fullscreen_pipeline(device, &module, &with_group, "fs_copy", None),
            blur: gpu::fullscreen_pipeline(device, &module, &with_group, "fs_blur", None),
            region: gpu::fullscreen_pipeline(device, &module, &with_group, "fs_region", premul),
            glow: gpu::fullscreen_pipeline(device, &module, &with_group, "fs_glow", premul),
            background: gpu::fullscreen_pipeline(device, &module, &empty, "fs_background", None),
            sampler: gpu::linear_sampler(device),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("blur params"),
                size: SLOT * SLOTS,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            layout,
            next_slot: std::cell::Cell::new(0),
        }
    }

    /// Call at the start of every frame: parameter slots are reused.
    pub fn begin_frame(&self) {
        self.next_slot.set(0);
    }

    fn draw(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        src: &wgpu::Texture,
        dst: &wgpu::Texture,
        params: Params,
        viewport: Option<[f32; 4]>,
    ) {
        let slot = self.next_slot.get();
        self.next_slot.set((slot + 1) % SLOTS);
        gpu.queue.write_buffer(&self.params, slot * SLOT, bytemuck::bytes_of(&params));
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&gpu::view(src)) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.params,
                        offset: 0,
                        size: wgpu::BufferSize::new(size_of::<Params>() as u64),
                    }),
                },
            ],
        });
        let dst_view = gpu::view(dst);
        let mut pass = gpu::pass(encoder, &dst_view, viewport.is_none().then_some(wgpu::Color::TRANSPARENT));
        if let Some([x, y, w, h]) = viewport {
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind, &[(slot * SLOT) as u32]);
        pass.draw(0..3, 0..1);
    }

    /// Horizontal then vertical blur of `a` (result back in `a`, `b` is scratch of the same size).
    fn blur_two_pass(&self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder, a: &wgpu::Texture, b: &wgpu::Texture, sigma: f32) {
        let (w, h) = (a.width() as f32, a.height() as f32);
        let horizontal = Params { step: [1.0 / w, 0.0, sigma, 0.0], ..Default::default() };
        let vertical = Params { step: [0.0, 1.0 / h, sigma, 0.0], ..Default::default() };
        self.draw(gpu, encoder, &self.blur, a, b, horizontal, None);
        self.draw(gpu, encoder, &self.blur, b, a, vertical, None);
    }

    /// Blurs `region` of `scene` in place. `a`/`b` are scratch textures of region size / `down`.
    pub fn region(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::Texture,
        a: &wgpu::Texture,
        b: &wgpu::Texture,
        region: &Region,
        down: f32,
    ) {
        let (sw, sh) = (scene.width() as f32, scene.height() as f32);
        let [x, y, w, h] = region.rect;
        let copy = Params { src_rect: [x / sw, y / sh, (x + w) / sw, (y + h) / sh], ..Default::default() };
        self.draw(gpu, encoder, &self.copy, scene, a, copy, None);
        self.blur_two_pass(gpu, encoder, a, b, region.sigma / down);
        let composite = Params { tint: region.tint, rect: [w, h, region.radius, 0.0], ..Default::default() };
        self.draw(gpu, encoder, &self.region, a, scene, composite, Some(region.rect));
    }

    /// Blurs the glow mask in `a` (scratch `b`) and composites it over `scene`.
    pub fn glow(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::Texture,
        a: &wgpu::Texture,
        b: &wgpu::Texture,
        sigma: f32,
        strength: f32,
    ) {
        self.blur_two_pass(gpu, encoder, a, b, sigma);
        let composite = Params { rect: [0.0, 0.0, 0.0, strength], ..Default::default() };
        let full = [0.0, 0.0, scene.width() as f32, scene.height() as f32];
        self.draw(gpu, encoder, &self.glow, a, scene, composite, Some(full));
    }
}
