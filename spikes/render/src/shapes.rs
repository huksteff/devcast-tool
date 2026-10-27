//! Instanced SDF shapes drawn from a storage buffer.

use bytemuck::{Pod, Zeroable};

use crate::gpu::{self, Gpu};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
pub struct Shape {
    pub center_half: [f32; 4],
    pub params: [f32; 4],
    pub kind: [u32; 4],
    pub grad: [f32; 4],
    pub stop_pos: [f32; 4],
    pub colors: [[f32; 4]; 4],
    pub stroke_color: [f32; 4],
    pub glow_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    time: f32,
    pad: f32,
}

pub struct Shapes {
    globals: wgpu::Buffer,
    storage: wgpu::Buffer,
    bind: wgpu::BindGroup,
    main: wgpu::RenderPipeline,
    glow_mask: wgpu::RenderPipeline,
    count: u32,
}

fn pipeline(device: &wgpu::Device, module: &wgpu::ShaderModule, layout: &wgpu::PipelineLayout, fs: &str) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fs),
        layout: Some(layout),
        vertex: wgpu::VertexState { module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fs),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: gpu::FORMAT,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

impl Shapes {
    pub fn new(gpu: &Gpu, shapes: &[Shape]) -> Self {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shapes"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shapes.wgsl").into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shapes"),
            entries: &[
                entry(0, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }),
                entry(1, wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                }),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shapes"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let storage = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shapes"),
            size: size_of_val(shapes) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&storage, 0, bytemuck::cast_slice(shapes));
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shapes"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: storage.as_entire_binding() },
            ],
        });
        Self {
            main: pipeline(device, &module, &pipeline_layout, "fs_main"),
            glow_mask: pipeline(device, &module, &pipeline_layout, "fs_glow_mask"),
            globals,
            storage,
            bind,
            count: shapes.len() as u32,
        }
    }

    /// `viewport` is the full-resolution frame size in px (shapes are positioned in it).
    pub fn set_frame(&self, gpu: &Gpu, viewport: [f32; 2], time: f32) {
        let g = Globals { viewport, time, pad: 0.0 };
        gpu.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&g));
        let _ = &self.storage;
    }

    fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::Texture, pipeline: &wgpu::RenderPipeline, clear: bool) {
        let view = gpu::view(target);
        let mut pass = gpu::pass(encoder, &view, clear.then_some(wgpu::Color::TRANSPARENT));
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..6, 0..self.count);
    }

    pub fn draw_main(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::Texture) {
        self.draw(encoder, target, &self.main, false);
    }

    /// Renders glow coverage into `mask` (may be lower resolution than the frame).
    pub fn draw_glow_mask(&self, encoder: &mut wgpu::CommandEncoder, mask: &wgpu::Texture) {
        self.draw(encoder, mask, &self.glow_mask, true);
    }
}
