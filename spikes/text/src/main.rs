//! Spike 1.5: text with glyphon 0.12 (cosmic-text) on wgpu 30, headless → PNG.
//! Checks: Cyrillic in bundled OFL fonts, colour emoji, crisp 1×→3× scaling, a .ttf loaded from file.
//!
//! Usage: spike-text <fonts_dir> <out.png>

use std::time::Instant;

use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea, TextAtlas,
    TextBounds, TextRenderer, Viewport,
};

const W: u32 = 1600;
const H: u32 = 900;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

struct Line {
    text: &'static str,
    family: &'static str,
    size: f32,
    scale: f32,
    top: f32,
    color: Color,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let fonts = args.first().cloned().unwrap_or_default();
    let out = args.get(1).cloned().unwrap_or_else(|| "text.png".to_owned());

    let started = Instant::now();
    let mut font_system = FontSystem::new_with_fonts([]);
    for file in ["Inter.ttf", "Montserrat.ttf", "Roboto.ttf", "PTSerif-Regular.ttf", "JetBrainsMono.ttf"] {
        font_system.db_mut().load_font_file(format!("{fonts}/{file}"))?;
    }
    font_system.db_mut().load_font_file("C:/Windows/Fonts/seguiemj.ttf")?;
    println!("fonts loaded in {:.1} ms: {}", started.elapsed().as_secs_f64() * 1000.0, font_system.db().len());
    for face in font_system.db().faces() {
        if let Some((name, _)) = face.families.first() {
            println!("  family: {name}");
        }
    }

    let white = Color::rgb(240, 240, 245);
    let lines = [
        Line { text: "Inter — Съешь же ещё этих мягких французских булок, да выпей чаю. 0123", family: "Inter", size: 30.0, scale: 1.0, top: 30.0, color: white },
        Line { text: "Montserrat — Широкая электрификация южных губерний", family: "Montserrat", size: 30.0, scale: 1.0, top: 80.0, color: white },
        Line { text: "Roboto — Эй, жлоб! Где туз? Прячь юных съёмщиц в шкаф.", family: "Roboto", size: 30.0, scale: 1.0, top: 130.0, color: white },
        Line { text: "PT Serif — В чащах юга жил бы цитрус? Да, но фальшивый экземпляр!", family: "PT Serif", size: 30.0, scale: 1.0, top: 180.0, color: white },
        Line { text: "JetBrains Mono (из файла) — fn main() { println!(\"Привет\"); }", family: "JetBrains Mono", size: 26.0, scale: 1.0, top: 230.0, color: Color::rgb(130, 220, 140) },
        Line { text: "Эмодзи и символы: 🎬 🎮 ✅ 🔥 👍 ● ▶ ■ ⏸", family: "Inter", size: 36.0, scale: 1.0, top: 290.0, color: white },
        Line { text: "Масштаб ×1: Запись", family: "Inter", size: 24.0, scale: 1.0, top: 370.0, color: Color::rgb(250, 204, 21) },
        Line { text: "Метрики ×3: Запись", family: "Inter", size: 72.0, scale: 1.0, top: 420.0, color: Color::rgb(250, 204, 21) },
        Line { text: "TextArea.scale ×3: Запись", family: "Inter", size: 24.0, scale: 3.0, top: 530.0, color: Color::rgb(250, 204, 21) },
        Line { text: "Мелкий текст 12 px: субтитры, подсказки, подписи на таймлайне — читается?", family: "Inter", size: 12.0, scale: 1.0, top: 680.0, color: white },
    ];
    let mut buffers = Vec::new();
    for line in &lines {
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(line.size, line.size * 1.3));
        buffer.set_size(Some(W as f32 / line.scale), None);
        buffer.set_text(line.text, &Attrs::new().family(Family::Name(line.family)), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut font_system, false);
        buffers.push(buffer);
    }

    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

    let mut swash = SwashCache::new();
    let cache = Cache::new(&device);
    let mut viewport = Viewport::new(&device, &cache);
    let mut atlas = TextAtlas::new(&device, &queue, &cache, FORMAT);
    let mut renderer = TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
    viewport.update(&queue, Resolution { width: W, height: H });

    let areas = lines.iter().zip(&buffers).map(|(line, buffer)| TextArea {
        buffer,
        left: 30.0,
        top: line.top,
        scale: line.scale,
        bounds: TextBounds { left: 0, top: 0, right: W as i32, bottom: H as i32 },
        default_color: line.color,
        custom_glyphs: &[],
    });
    let prepare_started = Instant::now();
    renderer.prepare(&device, &queue, &mut font_system, &mut atlas, &viewport, areas, &mut swash)?;
    println!("prepare (shaping + rasterising glyphs) {:.1} ms", prepare_started.elapsed().as_secs_f64() * 1000.0);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.01, g: 0.012, b: 0.02, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        renderer.render(&atlas, &viewport, &mut pass)?;
    }
    let row = (W * 4).div_ceil(256) * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(row * H),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(H) },
        },
        target.size(),
    );
    queue.submit([encoder.finish()]);
    readback.map_async(wgpu::MapMode::Read, .., |_| {});
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let data = readback.get_mapped_range(..)?;
    let mut pixels = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H as usize {
        pixels.extend_from_slice(&data[y * row as usize..y * row as usize + (W * 4) as usize]);
    }
    image::save_buffer(&out, &pixels, W, H, image::ExtendedColorType::Rgba8)?;
    println!("saved {out}");
    Ok(())
}
