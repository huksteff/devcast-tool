//! Brush pass: a drawing's strokes, tessellated by lyon, painted into a layer of the target's size
//! and laid over the frame. The layer lets the eraser clear earlier strokes of the same drawing
//! only and gives the whole drawing one opacity.
//!
//! The layer is 4× multisampled (lyon triangles have hard edges). A stencil value per stroke
//! keeps each sample painted once by a stroke, so a translucent stroke does not darken where it
//! overlaps itself.

mod tessellate;

use dc_core::PixelSize;

use crate::error::RenderError;
use crate::gpu::{Gpu, SRGB_VIEW};
use crate::pipeline::pipeline;
use crate::recorder::Recorder;
use crate::scene::{Camera, Drawing};
use crate::uniform::Slots;

use tessellate::{Mesh, Piece, Space, Vertex};

const SAMPLES: u32 = 4;
const STENCIL: wgpu::TextureFormat = wgpu::TextureFormat::Stencil8;

pub(crate) struct BrushPass {
    pen: wgpu::RenderPipeline,
    eraser: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    opacity: Slots<4>,
    mesh: Mesh,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    layer: Option<Layer>,
    drawings: Vec<Planned>,
}

/// Textures of the target's size the strokes go into, and the bind group reading the result.
struct Layer {
    size: PixelSize,
    samples: wgpu::TextureView,
    stencil: wgpu::TextureView,
    resolved: wgpu::TextureView,
    bind: wgpu::BindGroup,
}

struct Planned {
    pieces: Vec<Piece>,
    slot: u32,
}

impl BrushPass {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::include_wgsl!("brush.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("brush"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                Slots::<4>::layout_entry(1, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let over = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let erase = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let erase = wgpu::BlendState {
            color: erase,
            alpha: erase,
        };
        Self {
            pen: stroke_pipeline(
                device,
                &module,
                wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            ),
            eraser: stroke_pipeline(device, &module, erase),
            composite: pipeline(device, &module, &layout, ["vs_layer", "fs_layer"], over),
            layout,
            opacity: Slots::new(gpu, "brush"),
            mesh: Mesh::default(),
            vertices: buffer(gpu, wgpu::BufferUsages::VERTEX, 1024),
            indices: buffer(gpu, wgpu::BufferUsages::INDEX, 1024),
            layer: None,
            drawings: Vec::new(),
        }
    }

    /// Starts a frame.
    pub(crate) fn clear(&mut self) {
        self.mesh.clear();
        self.opacity.clear();
        self.drawings.clear();
    }

    /// Adds `drawing` seen through `camera` in a `frame`-sized scene drawn into `target`; returns
    /// its index, `None` when nothing of it is shown yet.
    pub(crate) fn push(
        &mut self,
        drawing: &Drawing,
        camera: Camera,
        frame: PixelSize,
        target: PixelSize,
    ) -> Result<Option<usize>, RenderError> {
        let space = Space {
            camera,
            frame: [frame.width as f32, frame.height as f32],
            per_target: frame.width as f32 / target.width as f32,
        };
        let pieces = self
            .mesh
            .push(drawing, space)
            .map_err(RenderError::Stroke)?;
        if pieces.is_empty() {
            return Ok(None);
        }
        let slot = self.opacity.push(&[drawing.opacity, 0.0, 0.0, 0.0]);
        self.drawings.push(Planned { pieces, slot });
        Ok(Some(self.drawings.len() - 1))
    }

    /// Sends the frame's triangles; makes the layer when the target size changed.
    pub(crate) fn upload(&mut self, gpu: &Gpu, target: PixelSize) {
        if self.drawings.is_empty() {
            return;
        }
        let vertices = self.mesh.buffers.vertices.as_flattened();
        let indices = &self.mesh.buffers.indices;
        let usage = wgpu::BufferUsages::VERTEX;
        write(gpu, &mut self.vertices, usage, vertices, f32::to_le_bytes);
        let usage = wgpu::BufferUsages::INDEX;
        write(gpu, &mut self.indices, usage, indices, u32::to_le_bytes);
        let grown = self.opacity.upload(gpu);
        if grown || self.layer.as_ref().is_none_or(|l| l.size != target) {
            self.layer = Some(self.layer(gpu, target));
        }
    }

    pub(crate) fn record(&self, recorder: &mut Recorder<'_>, drawing: usize) {
        let Some(layer) = &self.layer else {
            return;
        };
        let planned = &self.drawings[drawing];
        let encoder = recorder.encoder();
        for (batch, pieces) in planned.pieces.chunks(255).enumerate() {
            let mut pass = layer_pass(encoder, layer, batch == 0);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            for (reference, piece) in (1..).zip(pieces) {
                pass.set_pipeline(if piece.eraser {
                    &self.eraser
                } else {
                    &self.pen
                });
                pass.set_stencil_reference(reference);
                pass.draw_indexed(piece.indices.clone(), 0, 0..1);
            }
        }
        let pass = recorder.pass();
        pass.set_pipeline(&self.composite);
        pass.set_bind_group(0, &layer.bind, &[planned.slot]);
        pass.draw(0..4, 0..1);
    }

    fn layer(&self, gpu: &Gpu, size: PixelSize) -> Layer {
        let texture = |format, samples, usage| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("brush layer"),
                    size: wgpu::Extent3d {
                        width: size.width,
                        height: size.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let attachment = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let resolved = texture(
            SRGB_VIEW,
            1,
            attachment | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("brush"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&resolved),
                },
                self.opacity.bind_entry(1),
            ],
        });
        Layer {
            size,
            samples: texture(SRGB_VIEW, SAMPLES, attachment),
            stencil: texture(STENCIL, SAMPLES, attachment),
            resolved,
            bind,
        }
    }
}

/// A pass into the layer: cleared for the first batch of strokes, the stencil for every batch
/// (references 1..=255 restart).
fn layer_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    layer: &Layer,
    first: bool,
) -> wgpu::RenderPass<'e> {
    let load = if first {
        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
    } else {
        wgpu::LoadOp::Load
    };
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("brush"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &layer.samples,
            depth_slice: None,
            resolve_target: Some(&layer.resolved),
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &layer.stencil,
            depth_ops: None,
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0),
                store: wgpu::StoreOp::Discard,
            }),
        }),
        ..Default::default()
    })
}

fn stroke_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    blend: wgpu::BlendState,
) -> wgpu::RenderPipeline {
    let once = wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::NotEqual,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Replace,
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("brush"),
        layout: None,
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_stroke"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs_stroke"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: SRGB_VIEW,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: STENCIL,
            depth_write_enabled: None,
            depth_compare: None,
            stencil: wgpu::StencilState {
                front: once,
                back: once,
                read_mask: 0xff,
                write_mask: 0xff,
            },
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: SAMPLES,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}

fn buffer(gpu: &Gpu, usage: wgpu::BufferUsages, size: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("brush"),
        size,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Writes the little-endian bytes of `data` into `buffer`, replacing it with a bigger one when
/// they do not fit.
fn write<T: Copy>(
    gpu: &Gpu,
    buffer: &mut wgpu::Buffer,
    usage: wgpu::BufferUsages,
    data: &[T],
    bytes: impl Fn(T) -> [u8; 4],
) {
    let bytes: Vec<u8> = data.iter().flat_map(|v| bytes(*v)).collect();
    let len = bytes.len() as u64;
    if len > buffer.size() {
        *buffer = self::buffer(gpu, usage, len.next_power_of_two());
    }
    gpu.queue.write_buffer(buffer, 0, &bytes);
}
