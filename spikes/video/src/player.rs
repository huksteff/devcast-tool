//! Real-time playback in an egui window: decode thread → NV12 frames → two wgpu textures
//! (Y as R8, UV as RG8) → WGSL BT.709 limited-range conversion inside an egui paint callback.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::{Duration, Instant};

use eframe::{egui, egui_wgpu, wgpu};

use crate::decode::{Nv12Frame, VideoDecoder};

const SHADER: &str = r#"
override SRGB_TARGET: bool = false;
@group(0) @binding(0) var tex_y: texture_2d<f32>;
@group(0) @binding(1) var tex_uv: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VsOut;
    o.pos = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    o.uv = uv;
    return o;
}
fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}
@fragment fn fs(v: VsOut) -> @location(0) vec4<f32> {
    let y = (textureSample(tex_y, samp, v.uv).r - 16.0 / 255.0) * (255.0 / 219.0);
    let c = (textureSample(tex_uv, samp, v.uv).rg - vec2<f32>(128.0 / 255.0)) * (255.0 / 224.0);
    var rgb = clamp(vec3<f32>(y + 1.5748 * c.y, y - 0.1873 * c.x - 0.4681 * c.y, y + 1.8556 * c.x),
                    vec3<f32>(0.0), vec3<f32>(1.0));
    if SRGB_TARGET { rgb = to_linear(rgb); }
    return vec4<f32>(rgb, 1.0);
}
"#;

struct VideoResources {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    planes: Option<(wgpu::Texture, wgpu::Texture, wgpu::BindGroup)>,
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

impl VideoResources {
    fn new(rs: &egui_wgpu::RenderState) -> Self {
        let device = &rs.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("nv12"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("nv12"),
            entries: &[
                texture_entry(0),
                texture_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("nv12"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let srgb = if rs.target_format.is_srgb() { 1.0 } else { 0.0 };
        println!("target format: {:?}", rs.target_format);
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("nv12"),
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
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[("SRGB_TARGET", srgb)],
                    ..Default::default()
                },
                targets: &[Some(rs.target_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self { pipeline, layout, sampler, planes: None }
    }

    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &Nv12Frame) {
        let size_matches = self.planes.as_ref().is_some_and(|(y, _, _)| {
            y.width() == frame.width && y.height() == frame.height
        });
        if !size_matches {
            let make = |w, h, format| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("plane"),
                    size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                })
            };
            let y = make(frame.width, frame.height, wgpu::TextureFormat::R8Unorm);
            let uv = make(frame.width / 2, frame.height / 2, wgpu::TextureFormat::Rg8Unorm);
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("nv12"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&y.create_view(&Default::default())),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&uv.create_view(&Default::default())),
                    },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            self.planes = Some((y, uv, bind));
        }
        if let Some((y, uv, _)) = &self.planes {
            write_plane(queue, y, &frame.y, frame.width, frame.height);
            write_plane(queue, uv, &frame.uv, frame.width, frame.height / 2);
        }
    }
}

fn write_plane(queue: &wgpu::Queue, texture: &wgpu::Texture, data: &[u8], row_bytes: u32, rows: u32) {
    queue.write_texture(
        texture.as_image_copy(),
        data,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row_bytes), rows_per_image: Some(rows) },
        texture.size(),
    );
}

struct VideoCallback {
    frame: Option<Arc<Nv12Frame>>,
}

impl egui_wgpu::CallbackTrait for VideoCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let (Some(frame), Some(res)) = (&self.frame, resources.get_mut::<VideoResources>()) {
            res.upload(device, queue, frame);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(res) = resources.get::<VideoResources>()
            && let Some((_, _, bind)) = &res.planes
        {
            pass.set_pipeline(&res.pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

struct Player {
    frames: Receiver<Nv12Frame>,
    next: Option<Nv12Frame>,
    clock: Option<Instant>,
    seconds: f64,
    started: Instant,
    shown: u32,
    dropped: u32,
    to_upload: Option<Arc<Nv12Frame>>,
}

impl eframe::App for Player {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.started.elapsed().as_secs_f64() > self.seconds {
            println!("play: shown={} dropped={}", self.shown, self.dropped);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.next.is_none() {
            self.next = self.frames.try_recv().ok();
        }
        let Some(first) = &self.next else {
            ctx.request_repaint_after(Duration::from_millis(2));
            return;
        };
        let clock = *self.clock.get_or_insert_with(|| Instant::now() - Duration::from_secs_f64(first.pts));
        let now = clock.elapsed().as_secs_f64();
        let mut due = None;
        while self.next.as_ref().is_some_and(|f| f.pts <= now) {
            if due.is_some() {
                self.dropped += 1;
            }
            due = self.next.take();
            self.next = self.frames.try_recv().ok();
        }
        if let Some(frame) = due {
            self.shown += 1;
            self.to_upload = Some(Arc::new(frame));
        }
        let wait = self.next.as_ref().map_or(0.002, |f| (f.pts - now).max(0.0));
        ctx.request_repaint_after(Duration::from_secs_f64(wait));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let rect = ui.max_rect();
        let callback = VideoCallback { frame: self.to_upload.take() };
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(rect, callback));
    }
}

pub fn run(file: String, hw: bool, seconds: f64) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = sync_channel(6);
    std::thread::spawn(move || {
        let Ok(mut decoder) = VideoDecoder::open(&file, hw) else { return };
        while let Ok(Some(frame)) = decoder.next_frame() {
            if tx.send(frame).is_err() {
                return;
            }
        }
    });
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("spike-video")
            .with_inner_size([960.0, 540.0])
            .with_position([-1880.0, 200.0])
            .with_always_on_top(),
        ..Default::default()
    };
    eframe::run_native(
        "spike-video",
        options,
        Box::new(move |cc| {
            let rs = cc.wgpu_render_state.as_ref().ok_or("wgpu render state missing")?;
            let resources = VideoResources::new(rs);
            rs.renderer.write().callback_resources.insert(resources);
            Ok(Box::new(Player {
                frames: rx,
                next: None,
                clock: None,
                seconds,
                started: Instant::now(),
                shown: 0,
                dropped: 0,
                to_upload: None,
            }))
        }),
    )?;
    Ok(())
}
