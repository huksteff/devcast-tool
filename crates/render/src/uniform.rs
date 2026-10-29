//! A small uniform buffer of `N` floats (a whole number of `vec4<f32>`), rewritten before each
//! draw.

use crate::gpu::Gpu;

pub(crate) struct Uniform<const N: usize> {
    buffer: wgpu::Buffer,
}

impl<const N: usize> Uniform<N> {
    pub(crate) fn new(gpu: &Gpu, label: &str) -> Self {
        const { assert!(N.is_multiple_of(4), "a uniform holds whole vec4<f32>") };
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (N * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { buffer }
    }

    pub(crate) fn bind_entry(&self, binding: u32) -> wgpu::BindGroupEntry<'_> {
        wgpu::BindGroupEntry {
            binding,
            resource: self.buffer.as_entire_binding(),
        }
    }

    pub(crate) fn write(&self, gpu: &Gpu, values: &[f32; N]) {
        let bytes: [[u8; 4]; N] = values.map(f32::to_le_bytes);
        gpu.queue
            .write_buffer(&self.buffer, 0, bytes.as_flattened());
    }
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
