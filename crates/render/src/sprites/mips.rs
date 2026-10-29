//! A picture on the GPU: straight-alpha RGBA uploaded once, premultiplied in linear light and
//! given a full mip chain, so a big picture drawn small stays smooth (trilinear sampling).

use dc_core::PixelSize;

use crate::gpu::{FORMAT, Gpu, SRGB_VIEW};
use crate::pipeline::pipeline;

pub(super) struct Mips {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    premultiply: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
}

impl Mips {
    pub(super) fn new(gpu: &Gpu, texture_layout: &wgpu::BindGroupLayout) -> Self {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::include_wgsl!("mips.wgsl"));
        Self {
            premultiply: pipeline(
                device,
                &module,
                texture_layout,
                ["vs", "fs_premultiply"],
                None,
            ),
            down: pipeline(device, &module, texture_layout, ["vs", "fs_down"], None),
            layout: texture_layout.clone(),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("mips"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }

    /// A texture of `size` with every mip level made from `rgba` (straight alpha, tight rows).
    pub(super) fn texture(&self, gpu: &Gpu, size: PixelSize, rgba: &[u8]) -> wgpu::Texture {
        let levels = size.width.max(size.height).ilog2() + 1;
        let staging = texture(gpu, size, 1, wgpu::TextureUsages::COPY_DST);
        gpu.queue.write_texture(
            staging.as_image_copy(),
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.width * 4),
                rows_per_image: None,
            },
            staging.size(),
        );
        let picture = texture(gpu, size, levels, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mips"),
            });
        self.level(
            gpu,
            &mut encoder,
            &self.premultiply,
            (&staging, 0),
            (&picture, 0),
        );
        for level in 1..levels {
            self.level(
                gpu,
                &mut encoder,
                &self.down,
                (&picture, level - 1),
                (&picture, level),
            );
        }
        gpu.queue.submit([encoder.finish()]);
        picture
    }

    fn level(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        (source, from): (&wgpu::Texture, u32),
        (target, to): (&wgpu::Texture, u32),
    ) {
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mips"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&level_view(source, from)),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let view = level_view(target, to);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mips"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.draw(0..4, 0..1);
    }
}

/// One mip level of `texture` through the sRGB view.
fn level_view(texture: &wgpu::Texture, level: u32) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        format: Some(SRGB_VIEW),
        base_mip_level: level,
        mip_level_count: Some(1),
        ..Default::default()
    })
}

fn texture(gpu: &Gpu, size: PixelSize, levels: u32, usage: wgpu::TextureUsages) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sprite"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: usage | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[SRGB_VIEW],
    })
}
