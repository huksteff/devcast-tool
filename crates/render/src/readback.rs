//! Copies a rendered texture to the CPU.

use std::sync::mpsc;

use dc_core::PixelSize;

use crate::error::GpuError;
use crate::gpu::Gpu;

/// Frame in memory: RGBA8 in [`crate::FORMAT`], rows top to bottom without padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub size: PixelSize,
    pub rgba: Vec<u8>,
}

impl Gpu {
    /// Waits for the submitted work and returns the pixels of `texture` (format [`crate::FORMAT`]).
    pub fn read(&self, texture: &wgpu::Texture) -> Result<Image, GpuError> {
        let size = PixelSize {
            width: texture.width(),
            height: texture.height(),
        };
        let row = size.width * 4;
        let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded) * u64::from(size.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(size.height),
                },
            },
            texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (sender, receiver) = mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        receiver.recv().unwrap_or(Err(wgpu::BufferAsyncError))?;
        let data = buffer.get_mapped_range(..)?;
        let rgba = data
            .chunks_exact(padded as usize)
            .flat_map(|line| &line[..row as usize])
            .copied()
            .collect();
        Ok(Image { size, rgba })
    }
}
