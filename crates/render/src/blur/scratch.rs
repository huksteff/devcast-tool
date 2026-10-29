//! Two scratch textures of the target's size, their bind groups and render passes into them.

use dc_core::PixelSize;

use crate::gpu::{Gpu, SRGB_VIEW};
use crate::uniform::Slots;

use super::PARAMS;

/// Two textures of the target's size (1×1 until a frame has blurs) and their bind groups.
pub(super) struct Scratch {
    pub(super) size: PixelSize,
    pub(super) views: [wgpu::TextureView; 2],
    pub(super) binds: [wgpu::BindGroup; 2],
}

pub(super) const ONE: PixelSize = PixelSize {
    width: 1,
    height: 1,
};

pub(super) fn scratch_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    [w, h]: [u32; 2],
    clear: bool,
) -> wgpu::RenderPass<'e> {
    let load = if clear {
        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
    } else {
        wgpu::LoadOp::Load
    };
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("blur"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    pass.set_viewport(0.0, 0.0, w as f32, h as f32, 0.0, 1.0);
    pass.set_scissor_rect(0, 0, w, h);
    pass
}

pub(super) fn scratch(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    params: &Slots<PARAMS>,
    size: PixelSize,
) -> Scratch {
    let views = [0, 1].map(|_| {
        gpu.target(size).create_view(&wgpu::TextureViewDescriptor {
            format: Some(SRGB_VIEW),
            ..Default::default()
        })
    });
    let binds = [0, 1].map(|i| bind_group(gpu, layout, &views[i], sampler, params));
    Scratch { size, views, binds }
}

pub(super) fn bind_group(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    params: &Slots<PARAMS>,
) -> wgpu::BindGroup {
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("blur"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            params.bind_entry(2),
        ],
    })
}
