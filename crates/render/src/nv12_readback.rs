//! Frames for the encoder (decision 006): a rendered target converted to NV12 on the GPU and
//! read back through a ring of buffers, so the GPU draws the next frames while the CPU copies
//! and encodes an earlier one.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver};

use dc_core::PixelSize;

use crate::error::GpuError;
use crate::gpu::Gpu;

/// Frames in flight between the GPU and the encoder.
const RING: usize = 4;

/// The planes of one frame in a mapped buffer: `height` rows of Y, then `height / 2` rows of
/// interleaved U, V; every row starts `pitch` bytes after the previous one.
pub struct Planes<'a> {
    pub y: &'a [u8],
    pub uv: &'a [u8],
    pub pitch: usize,
}

/// Converts one target texture (of [`Gpu::target`], even sides) after every draw into it.
pub struct Nv12Readback {
    y_pipeline: wgpu::RenderPipeline,
    uv_pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    y: wgpu::Texture,
    uv: wgpu::Texture,
    size: PixelSize,
    pitch: u32,
    free: Vec<wgpu::Buffer>,
    pending: VecDeque<Pending>,
}

struct Pending {
    buffer: wgpu::Buffer,
    submission: wgpu::SubmissionIndex,
    mapped: Receiver<Result<(), wgpu::BufferAsyncError>>,
}

impl Nv12Readback {
    pub fn new(gpu: &Gpu, target: &wgpu::Texture) -> Self {
        let device = &gpu.device;
        let size = PixelSize {
            width: target.width(),
            height: target.height(),
        };
        let module = device.create_shader_module(wgpu::include_wgsl!("nv12.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("nv12"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("nv12"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &target.create_view(&wgpu::TextureViewDescriptor::default()),
                ),
            }],
        });
        let y_format = wgpu::TextureFormat::R8Unorm;
        let uv_format = wgpu::TextureFormat::Rg8Unorm;
        let half = PixelSize {
            width: size.width / 2,
            height: size.height / 2,
        };
        Self {
            y_pipeline: pipeline(gpu, &module, &layout, "fs_y", y_format),
            uv_pipeline: pipeline(gpu, &module, &layout, "fs_uv", uv_format),
            bind,
            y: plane(gpu, size, y_format),
            uv: plane(gpu, half, uv_format),
            pitch: size
                .width
                .next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
            size,
            free: Vec::new(),
            pending: VecDeque::new(),
        }
    }

    /// Converts what the target holds now and starts copying it to memory. With [`RING`]
    /// frames in flight, first waits for the oldest and hands its planes to `take`.
    pub fn push<E: From<GpuError>>(
        &mut self,
        gpu: &Gpu,
        take: &mut impl FnMut(Planes<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        if self.pending.len() == RING {
            self.take_oldest(gpu, take)?;
        }
        let buffer = self.free.pop().unwrap_or_else(|| self.buffer(gpu));
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("nv12"),
            });
        convert(&mut encoder, &self.y, &self.y_pipeline, &self.bind);
        convert(&mut encoder, &self.uv, &self.uv_pipeline, &self.bind);
        let rows = self.size.height;
        for (texture, offset, rows) in [(&self.y, 0, rows), (&self.uv, self.y_len(), rows / 2)] {
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset,
                        bytes_per_row: Some(self.pitch),
                        rows_per_image: Some(rows),
                    },
                },
                texture.size(),
            );
        }
        let submission = gpu.queue.submit([encoder.finish()]);
        let (sender, mapped) = mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });
        self.pending.push_back(Pending {
            buffer,
            submission,
            mapped,
        });
        Ok(())
    }

    /// Hands the frames still in flight to `take`, oldest first.
    pub fn finish<E: From<GpuError>>(
        &mut self,
        gpu: &Gpu,
        take: &mut impl FnMut(Planes<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        while !self.pending.is_empty() {
            self.take_oldest(gpu, take)?;
        }
        Ok(())
    }

    fn take_oldest<E: From<GpuError>>(
        &mut self,
        gpu: &Gpu,
        take: &mut impl FnMut(Planes<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        let Some(oldest) = self.pending.pop_front() else {
            return Ok(());
        };
        gpu.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(oldest.submission),
                timeout: None,
            })
            .map_err(GpuError::from)?;
        oldest
            .mapped
            .recv()
            .unwrap_or(Err(wgpu::BufferAsyncError))
            .map_err(GpuError::from)?;
        {
            let data = oldest.buffer.get_mapped_range(..).map_err(GpuError::from)?;
            let (y, uv) = data.split_at(self.y_len() as usize);
            take(Planes {
                y,
                uv,
                pitch: self.pitch as usize,
            })?;
        }
        oldest.buffer.unmap();
        self.free.push(oldest.buffer);
        Ok(())
    }

    fn y_len(&self) -> u64 {
        u64::from(self.pitch) * u64::from(self.size.height)
    }

    fn buffer(&self, gpu: &Gpu) -> wgpu::Buffer {
        gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("nv12 readback"),
            size: self.y_len() + u64::from(self.pitch) * u64::from(self.size.height / 2),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        })
    }
}

fn plane(gpu: &Gpu, size: PixelSize, format: wgpu::TextureFormat) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("nv12 plane"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn pipeline(
    gpu: &Gpu,
    module: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
    fs: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(fs),
            bind_group_layouts: &[Some(layout)],
            immediate_size: 0,
        });
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fs),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
}

fn convert(
    encoder: &mut wgpu::CommandEncoder,
    plane: &wgpu::Texture,
    pipeline: &wgpu::RenderPipeline,
    bind: &wgpu::BindGroup,
) {
    let view = plane.create_view(&wgpu::TextureViewDescriptor::default());
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("nv12"),
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
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind, &[]);
    pass.draw(0..3, 0..1);
}
