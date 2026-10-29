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

/// Uniform values of `N` floats for several draws of a frame, one slot each, bound with a dynamic
/// offset. Filled before the frame is recorded, sent in one write.
pub(crate) struct Slots<const N: usize> {
    buffer: wgpu::Buffer,
    stride: u64,
    bytes: Vec<u8>,
}

impl<const N: usize> Slots<N> {
    pub(crate) fn new(gpu: &Gpu, label: &str) -> Self {
        let stride =
            u64::from(gpu.device.limits().min_uniform_buffer_offset_alignment).max((N * 4) as u64);
        Self {
            buffer: slots_buffer(gpu, label, stride * 8),
            stride,
            bytes: Vec::new(),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.bytes.clear();
    }

    /// Adds a slot and returns its dynamic offset.
    pub(crate) fn push(&mut self, values: &[f32; N]) -> u32 {
        let offset = self.bytes.len();
        self.bytes
            .extend_from_slice(values.map(f32::to_le_bytes).as_flattened());
        self.bytes.resize(offset + self.stride as usize, 0);
        offset as u32
    }

    /// Sends the slots; `true` when the buffer was replaced and its bind groups must be made
    /// again.
    pub(crate) fn upload(&mut self, gpu: &Gpu) -> bool {
        let len = self.bytes.len() as u64;
        let grown = len > self.buffer.size();
        if grown {
            self.buffer = slots_buffer(gpu, "slots", len.next_power_of_two());
        }
        if len > 0 {
            gpu.queue.write_buffer(&self.buffer, 0, &self.bytes);
        }
        grown
    }

    pub(crate) fn bind_entry(&self, binding: u32) -> wgpu::BindGroupEntry<'_> {
        wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &self.buffer,
                offset: 0,
                size: wgpu::BufferSize::new((N * 4) as u64),
            }),
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
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new((N * 4) as u64),
            },
            count: None,
        }
    }
}

fn slots_buffer(gpu: &Gpu, label: &str, size: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
