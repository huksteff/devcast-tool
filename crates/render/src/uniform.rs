//! A small uniform buffer of three `vec4<f32>`, rewritten before each draw.

use crate::gpu::Gpu;

pub(crate) const UNIFORM_BYTES: usize = 48;

pub(crate) struct Uniform {
    buffer: wgpu::Buffer,
}

impl Uniform {
    pub(crate) fn new(gpu: &Gpu, label: &str) -> Self {
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: UNIFORM_BYTES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { buffer }
    }

    pub(crate) fn layout_entry(
        binding: u32,
        visibility: wgpu::ShaderStages,
    ) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }
    }

    pub(crate) fn bind_entry(&self, binding: u32) -> wgpu::BindGroupEntry<'_> {
        wgpu::BindGroupEntry {
            binding,
            resource: self.buffer.as_entire_binding(),
        }
    }

    pub(crate) fn write(&self, gpu: &Gpu, values: &[f32; UNIFORM_BYTES / 4]) {
        let bytes: [[u8; 4]; UNIFORM_BYTES / 4] = values.map(f32::to_le_bytes);
        gpu.queue
            .write_buffer(&self.buffer, 0, bytes.as_flattened());
    }
}
