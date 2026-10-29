//! Video pass: NV12 frames on the GPU, converted to RGB while drawing with the matrix, range and
//! chroma siting the file marks (`dc_media::Probe::color`).

use dc_core::{ColorRange, PixelSize, SourceTime, VideoColor};

use crate::error::RenderError;
use crate::gpu::{Gpu, SRGB_VIEW};
use crate::scene::{Camera, Motion, Video};
use crate::uniform::{self, Uniform};

/// Floats of `Video` of video.wgsl.
const FLOATS: usize = 40;

/// A decoded frame as `dc_media::Frame` gives it: the Y plane, then interleaved UV of
/// `ceil(width/2)` pairs per row and `ceil(height/2)` rows; rows without padding. `color` is how
/// its file stores colour.
#[derive(Debug, Clone, Copy)]
pub struct Nv12<'a> {
    size: PixelSize,
    y: &'a [u8],
    uv: &'a [u8],
    color: VideoColor,
}

impl<'a> Nv12<'a> {
    /// Checks that the planes have the lengths of a `size` frame.
    pub fn new(
        size: PixelSize,
        y: &'a [u8],
        uv: &'a [u8],
        color: VideoColor,
    ) -> Result<Self, RenderError> {
        let half = chroma_size(size);
        let y_len = size.width as usize * size.height as usize;
        let uv_len = half.width as usize * 2 * half.height as usize;
        if y.len() != y_len || uv.len() != uv_len || y_len == 0 {
            return Err(RenderError::Nv12 {
                width: size.width,
                height: size.height,
                y: y.len(),
                uv: uv.len(),
            });
        }
        Ok(Self { size, y, uv, color })
    }
}

/// Pipeline and sampler shared by all video layers.
pub(crate) struct VideoPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

/// Textures of one media file, kept between frames and rewritten by every upload.
pub(crate) struct VideoFrame {
    pub(crate) time: SourceTime,
    size: PixelSize,
    color: VideoColor,
    luma: wgpu::Texture,
    chroma: wgpu::Texture,
    uniform: Uniform<FLOATS>,
    bind: wgpu::BindGroup,
}

impl VideoPass {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::include_wgsl!("video.wgsl"));
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video"),
            entries: &[
                uniform::layout_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                texture(1),
                texture(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("video"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("video"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SRGB_VIEW,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("video"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            layout,
            sampler,
        }
    }

    /// Writes `frame` into the textures of `old` (new ones when there are none of its size).
    pub(crate) fn upload(
        &self,
        gpu: &Gpu,
        old: Option<VideoFrame>,
        time: SourceTime,
        frame: Nv12<'_>,
    ) -> VideoFrame {
        let textures = match old {
            Some(old) if old.size == frame.size => old,
            _ => self.create(gpu, frame.size),
        };
        write(gpu, &textures.luma, frame.y, frame.size.width);
        write(
            gpu,
            &textures.chroma,
            frame.uv,
            chroma_size(frame.size).width * 2,
        );
        VideoFrame {
            time,
            color: frame.color,
            ..textures
        }
    }

    fn create(&self, gpu: &Gpu, size: PixelSize) -> VideoFrame {
        let luma = plane(gpu, size, wgpu::TextureFormat::R8Unorm);
        let chroma = plane(gpu, chroma_size(size), wgpu::TextureFormat::Rg8Unorm);
        let uniform = Uniform::new(gpu, "video");
        let view = |texture: &wgpu::Texture| texture.create_view(&Default::default());
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("video"),
            layout: &self.layout,
            entries: &[
                uniform.bind_entry(0),
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view(&luma)),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&view(&chroma)),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        VideoFrame {
            time: SourceTime::ZERO,
            size,
            color: VideoColor::BT709,
            luma,
            chroma,
            uniform,
            bind,
        }
    }

    /// Places `video` in a `frame`-sized output drawn into `target` through `camera`, smeared
    /// along `motion` of the camera.
    pub(crate) fn prepare(
        gpu: &Gpu,
        textures: &VideoFrame,
        video: &Video,
        camera: Camera,
        motion: Option<Motion<Camera>>,
        [frame, target]: [PixelSize; 2],
    ) {
        let from = motion.map_or(camera, |m| m.from);
        let samples = motion.map_or(1, |m| m.samples.get());
        let step = motion.map_or(0.0, |m| m.step());
        let r = video.rect;
        let (fw, fh) = (frame.width as f32, frame.height as f32);
        let corners = [camera, from].map(|c| {
            let [left, top] = c.apply([r.x, r.y]);
            let [right, bottom] = c.apply([r.x + r.w, r.y + r.h]);
            [left, top, right, bottom]
        });
        let quad = [
            corners[0][0].min(corners[1][0]) / fw * 2.0 - 1.0,
            1.0 - corners[0][1].min(corners[1][1]) / fh * 2.0,
            corners[0][2].max(corners[1][2]) / fw * 2.0 - 1.0,
            1.0 - corners[0][3].max(corners[1][3]) / fh * 2.0,
        ];
        let c = video.crop;
        let crop = [c.x(), c.y(), c.x() + c.w(), c.y() + c.h()].map(|v| v as f32);
        // Chroma sample i covers luma pixels 2i and 2i + 1, shifted by the siting; odd sizes have
        // one chroma sample more than half the luma.
        let half = chroma_size(textures.size);
        let (cw, ch) = ((half.width * 2) as f32, (half.height * 2) as f32);
        let (w, h) = (textures.size.width as f32, textures.size.height as f32);
        let (dx, dy) = textures.color.chroma.offset();
        let [red, green, blue] = yuv_to_rgb(textures.color);
        let rows: [[f32; 4]; 10] = [
            [r.x, r.y, r.w, r.h],
            crop,
            [w / cw, h / ch, -dx / cw, -dy / ch],
            red,
            green,
            blue,
            [camera.scale, camera.x, camera.y, fw / target.width as f32],
            [
                from.scale - camera.scale,
                from.x - camera.x,
                from.y - camera.y,
                samples as f32,
            ],
            quad,
            [step, 0.0, 0.0, 0.0],
        ];
        let mut values = [0.0; FLOATS];
        values.copy_from_slice(rows.as_flattened());
        textures.uniform.write(gpu, &values);
    }

    pub(crate) fn record(&self, pass: &mut wgpu::RenderPass<'_>, textures: &VideoFrame) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &textures.bind, &[]);
        pass.draw(0..4, 0..1);
    }
}

/// Rows of R', G' and B' as `(y, u, v, 1) · row` of the sampled texture values 0..1.
fn yuv_to_rgb(color: VideoColor) -> [[f32; 4]; 3] {
    let (kr, kb) = color.matrix.weights();
    let kg = 1.0 - kr - kb;
    // Luma and chroma 0..1 → Y' 0..1 and Cb, Cr −0.5..0.5.
    let (ys, yo, cs) = match color.range {
        ColorRange::Limited => (255.0 / 219.0, -16.0 / 219.0, 255.0 / 224.0),
        ColorRange::Full => (1.0, 0.0, 1.0),
    };
    let co = -128.0 / 255.0 * cs;
    let (rv, bu) = (2.0 * (1.0 - kr), 2.0 * (1.0 - kb));
    let (gu, gv) = (-bu * kb / kg, -rv * kr / kg);
    let row = |u: f64, v: f64| [ys, u * cs, v * cs, yo + (u + v) * co].map(|x| x as f32);
    [row(0.0, rv), row(gu, gv), row(bu, 0.0)]
}

fn chroma_size(size: PixelSize) -> PixelSize {
    PixelSize {
        width: size.width.div_ceil(2),
        height: size.height.div_ceil(2),
    }
}

fn plane(gpu: &Gpu, size: PixelSize, format: wgpu::TextureFormat) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("video plane"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn write(gpu: &Gpu, texture: &wgpu::Texture, data: &[u8], row_bytes: u32) {
    gpu.queue.write_texture(
        texture.as_image_copy(),
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(row_bytes),
            rows_per_image: None,
        },
        texture.size(),
    );
}
