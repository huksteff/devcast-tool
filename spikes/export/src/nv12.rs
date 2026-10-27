//! GPU side of export: render an animated test scene (sRGB), convert to NV12 (BT.709 limited)
//! in two passes (Y full-res R8, UV half-res RG8), read back through a ring of mapped buffers.

const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

const SHADER: &str = r#"
struct Globals { time: f32, pad0: f32, pad1: f32, pad2: f32 };
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VsOut;
    o.pos = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    o.uv = uv;
    return o;
}
// Animated scene: moving bars over a gradient.
@fragment fn fs_scene(v: VsOut) -> @location(0) vec4<f32> {
    let bar = fract(v.uv.x * 6.0 + g.time * 0.5);
    let base = vec3<f32>(v.uv.x, v.uv.y, 1.0 - v.uv.x) * 0.8;
    let c = select(base, vec3<f32>(0.9, 0.3, 0.1), bar < 0.12);
    let box = abs(v.uv - vec2<f32>(0.5 + 0.3 * sin(g.time), 0.5)) < vec2<f32>(0.08, 0.12);
    return vec4<f32>(select(c, vec3<f32>(1.0), box.x && box.y), 1.0);
}
fn encode_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}
@fragment fn fs_y(v: VsOut) -> @location(0) vec4<f32> {
    let c = encode_srgb(textureSampleLevel(scene, samp, v.uv, 0.0).rgb);
    let y = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec4<f32>((16.0 + 219.0 * y) / 255.0, 0.0, 0.0, 1.0);
}
// Half-res target: bilinear sampling at the 2x2 block centre averages the block.
@fragment fn fs_uv(v: VsOut) -> @location(0) vec4<f32> {
    let c = encode_srgb(textureSampleLevel(scene, samp, v.uv, 0.0).rgb);
    let y = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let cb = (c.b - y) / 1.8556;
    let cr = (c.r - y) / 1.5748;
    return vec4<f32>((128.0 + 224.0 * cb) / 255.0, (128.0 + 224.0 * cr) / 255.0, 0.0, 1.0);
}
"#;

pub struct Converter {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub w: u32,
    pub h: u32,
    globals: wgpu::Buffer,
    scene: wgpu::Texture,
    y: wgpu::Texture,
    uv: wgpu::Texture,
    scene_pipe: wgpu::RenderPipeline,
    y_pipe: wgpu::RenderPipeline,
    uv_pipe: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    /// Globals only: the scene pass renders into the scene texture, so it must not bind it.
    scene_bind: wgpu::BindGroup,
    ring: Vec<wgpu::Buffer>,
    y_row: u32,
    uv_row: u32,
}

fn texture(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat, sampled: bool) -> wgpu::Texture {
    let usage = wgpu::TextureUsages::RENDER_ATTACHMENT
        | if sampled { wgpu::TextureUsages::TEXTURE_BINDING } else { wgpu::TextureUsages::COPY_SRC };
    device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fs: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fs),
        layout: Some(layout),
        vertex: wgpu::VertexState { module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
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

pub const RING: usize = 4;

impl Converter {
    pub fn new(w: u32, h: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::DX12;
        let instance = wgpu::Instance::new(desc);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("nv12"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scene = texture(&device, w, h, SCENE_FORMAT, true);
        let y = texture(&device, w, h, wgpu::TextureFormat::R8Unorm, false);
        let uv = texture(&device, w / 2, h / 2, wgpu::TextureFormat::Rg8Unorm, false);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&scene.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let scene_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene"),
            layout: &scene_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let scene_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&scene_layout)],
            immediate_size: 0,
        });
        let y_row = w.div_ceil(256) * 256;
        let uv_row = w.div_ceil(256) * 256; // w/2 texels × 2 bytes
        let size = u64::from(y_row * h + uv_row * (h / 2));
        let ring = (0..RING)
            .map(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("nv12 readback"),
                    size,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                })
            })
            .collect();
        Ok(Self {
            scene_pipe: pipeline(&device, &module, &scene_pl, "fs_scene", SCENE_FORMAT),
            y_pipe: pipeline(&device, &module, &pl, "fs_y", wgpu::TextureFormat::R8Unorm),
            uv_pipe: pipeline(&device, &module, &pl, "fs_uv", wgpu::TextureFormat::Rg8Unorm),
            device,
            queue,
            w,
            h,
            globals,
            scene,
            y,
            uv,
            bind,
            scene_bind,
            ring,
            y_row,
            uv_row,
        })
    }

    fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::Texture, pipe: &wgpu::RenderPipeline, bind: &wgpu::BindGroup) {
        let view = target.create_view(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        });
        pass.set_pipeline(pipe);
        pass.set_bind_group(0, bind, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Renders frame `time` and queues its NV12 copy into ring slot `slot`, mapping it when done.
    pub fn submit(&self, time: f32, slot: usize) {
        self.queue.write_buffer(&self.globals, 0, bytemuck::cast_slice(&[time, 0.0, 0.0, 0.0]));
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.draw(&mut encoder, &self.scene, &self.scene_pipe, &self.scene_bind);
        self.draw(&mut encoder, &self.y, &self.y_pipe, &self.bind);
        self.draw(&mut encoder, &self.uv, &self.uv_pipe, &self.bind);
        let buffer = &self.ring[slot];
        encoder.copy_texture_to_buffer(
            self.y.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.y_row), rows_per_image: Some(self.h) },
            },
            self.y.size(),
        );
        encoder.copy_texture_to_buffer(
            self.uv.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: u64::from(self.y_row * self.h),
                    bytes_per_row: Some(self.uv_row),
                    rows_per_image: Some(self.h / 2),
                },
            },
            self.uv.size(),
        );
        self.queue.submit([encoder.finish()]);
        buffer.map_async(wgpu::MapMode::Read, .., |_| {});
    }

    /// Waits for ring slot `slot` and copies its planes into an NV12 frame.
    pub fn read(&self, slot: usize, frame: &mut ffmpeg_next::frame::Video) -> Result<(), Box<dyn std::error::Error>> {
        let buffer = &self.ring[slot];
        loop {
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
            if buffer.get_mapped_range(..).is_ok() {
                break;
            }
        }
        {
            let data = buffer.get_mapped_range(..)?;
            let (w, h) = (self.w as usize, self.h as usize);
            let stride = frame.stride(0);
            let plane = frame.data_mut(0);
            for row in 0..h {
                let src = row * self.y_row as usize;
                plane[row * stride..row * stride + w].copy_from_slice(&data[src..src + w]);
            }
            let base = (self.y_row * self.h) as usize;
            let stride = frame.stride(1);
            let plane = frame.data_mut(1);
            for row in 0..h / 2 {
                let src = base + row * self.uv_row as usize;
                plane[row * stride..row * stride + w].copy_from_slice(&data[src..src + w]);
            }
        }
        buffer.unmap();
        Ok(())
    }
}
