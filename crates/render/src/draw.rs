//! `draw`: paints a [`Scene`] into a target texture on the GPU.

use crate::gpu::{Gpu, SRGB_VIEW};
use crate::scene::Scene;

/// Paints `scene` into `target` (a texture of [`Gpu::target`]) in one render pass: the frame is
/// cleared to black, then the layers go bottom to top. Blending happens in linear light.
pub fn draw(gpu: &Gpu, scene: &Scene, target: &wgpu::Texture) {
    let view = target.create_view(&wgpu::TextureViewDescriptor {
        format: Some(SRGB_VIEW),
        ..Default::default()
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("draw"),
        });
    let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("scene"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    if let Some((_band, layer)) = scene.painted().next() {
        match *layer {}
    }
    drop(pass);
    gpu.queue.submit([encoder.finish()]);
}
