//! Spike 1.4: overlays on wgpu with our own WGSL — SDF shapes (gradient, stroke, rotation,
//! opacity), separable gaussian region blur with tint, glow. Renders PNGs and benchmarks 1080p/4K.
//!
//! Usage: spike-render <out_dir> [--warp]

mod blur;
mod gpu;
mod shapes;

use std::time::Instant;

use blur::{Blur, Region};
use gpu::Gpu;
use shapes::{Shape, Shapes};

fn srgb(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    let lin = |c: u8| {
        let c = f32::from(c) / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    [lin(r), lin(g), lin(b), a]
}

/// Scene laid out for 1920×1080 and scaled by `k`.
fn scene(k: f32) -> (Vec<Shape>, Region) {
    let none = [0.0; 4];
    let rect = |cx: f32, cy: f32, hx: f32, hy: f32| [cx * k, cy * k, hx * k, hy * k];
    let shapes = vec![
        // Rounded rectangle: static 4-stop gradient, white stroke, orange glow.
        Shape {
            center_half: rect(520.0, 330.0, 230.0, 110.0),
            params: [0.0, 18.0 * k, 6.0 * k, 1.0],
            kind: [0, 4, 1, 0],
            grad: [1.0, 0.0, 0.0, 0.0],
            stop_pos: [0.0, 0.33, 0.66, 1.0],
            colors: [srgb(235, 64, 52, 1.0), srgb(250, 204, 21, 1.0), srgb(34, 197, 94, 1.0), srgb(59, 130, 246, 1.0)],
            stroke_color: srgb(255, 255, 255, 1.0),
            glow_color: srgb(255, 140, 30, 0.9),
        },
        // Rotated ellipse: animated 3-colour gradient, 80% opacity.
        Shape {
            center_half: rect(520.0, 760.0, 190.0, 110.0),
            params: [0.3, 0.0, 0.0, 0.8],
            kind: [1, 3, 1, 1],
            grad: [0.0, 1.0, 0.5, 0.0],
            colors: [srgb(168, 85, 247, 1.0), srgb(236, 72, 153, 1.0), srgb(14, 165, 233, 1.0), none],
            ..Default::default()
        },
        // Circle: stroke only, cyan glow.
        Shape {
            center_half: rect(1450.0, 760.0, 120.0, 120.0),
            params: [0.0, 0.0, 10.0 * k, 1.0],
            kind: [1, 1, 0, 0],
            stroke_color: srgb(34, 211, 238, 1.0),
            glow_color: srgb(34, 211, 238, 0.8),
            ..Default::default()
        },
        // Arrow: rotated, animated gradient, magenta glow.
        Shape {
            center_half: rect(1050.0, 560.0, 260.0, 40.0),
            params: [-0.5, 0.0, 0.0, 1.0],
            kind: [2, 3, 1, 1],
            grad: [1.0, 0.0, 0.4, 0.0],
            colors: [srgb(244, 63, 94, 1.0), srgb(250, 204, 21, 1.0), srgb(168, 85, 247, 1.0), none],
            glow_color: srgb(217, 70, 239, 0.7),
            ..Default::default()
        },
        // Highlight: translucent yellow box.
        Shape {
            center_half: rect(1380.0, 300.0, 200.0, 50.0),
            params: [0.0, 6.0 * k, 0.0, 1.0],
            kind: [0, 1, 1, 0],
            colors: [srgb(250, 204, 21, 0.35), none, none, none],
            ..Default::default()
        },
    ];
    let region = Region {
        rect: [1100.0 * k, 420.0 * k, 560.0 * k, 200.0 * k],
        sigma: 18.0 * k,
        tint: srgb(20, 20, 30, 0.15),
        radius: 8.0 * k,
    };
    (shapes, region)
}

struct Frame {
    scene: wgpu::Texture,
    region_a: wgpu::Texture,
    region_b: wgpu::Texture,
    glow_a: wgpu::Texture,
    glow_b: wgpu::Texture,
    shapes: Shapes,
    region: Region,
    k: f32,
}

const REGION_DOWN: f32 = 4.0;
const GLOW_DOWN: u32 = 4;

impl Frame {
    fn new(gpu: &Gpu, w: u32, h: u32) -> Self {
        let k = w as f32 / 1920.0;
        let (shapes, region) = scene(k);
        let rw = (region.rect[2] / REGION_DOWN) as u32;
        let rh = (region.rect[3] / REGION_DOWN) as u32;
        Self {
            scene: gpu.target(w, h, "scene"),
            region_a: gpu.target(rw, rh, "region a"),
            region_b: gpu.target(rw, rh, "region b"),
            glow_a: gpu.target(w / GLOW_DOWN, h / GLOW_DOWN, "glow a"),
            glow_b: gpu.target(w / GLOW_DOWN, h / GLOW_DOWN, "glow b"),
            shapes: Shapes::new(gpu, &shapes),
            region,
            k,
        }
    }

    fn render(&self, gpu: &Gpu, blur: &Blur, time: f32) {
        blur.begin_frame();
        let (w, h) = (self.scene.width() as f32, self.scene.height() as f32);
        self.shapes.set_frame(gpu, [w, h], time);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let view = gpu::view(&self.scene);
            let mut pass = gpu::pass(&mut encoder, &view, Some(wgpu::Color::BLACK));
            pass.set_pipeline(&blur.background);
            pass.draw(0..3, 0..1);
        }
        blur.region(gpu, &mut encoder, &self.scene, &self.region_a, &self.region_b, &self.region, REGION_DOWN);
        self.shapes.draw_glow_mask(&mut encoder, &self.glow_a);
        blur.glow(gpu, &mut encoder, &self.scene, &self.glow_a, &self.glow_b, 14.0 * self.k / GLOW_DOWN as f32, 1.6);
        self.shapes.draw_main(&mut encoder, &self.scene);
        gpu.queue.submit([encoder.finish()]);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args.first().cloned().unwrap_or_else(|| ".".to_owned());
    let warp = args.iter().any(|a| a == "--warp");
    let gpu = Gpu::new(warp)?;
    println!("adapter: {}", gpu.adapter);
    let blur = Blur::new(&gpu);
    for (w, h, name) in [(1920, 1080, "1080p"), (3840, 2160, "4k")] {
        let frame = Frame::new(&gpu, w, h);
        frame.render(&gpu, &blur, 0.25);
        gpu.save_png(&frame.scene, &format!("{out}/render-{name}.png"))?;
        let n = if warp { 10 } else { 240 };
        gpu.wait();
        let started = Instant::now();
        for i in 0..n {
            frame.render(&gpu, &blur, i as f32 / 60.0);
        }
        gpu.wait();
        let ms = started.elapsed().as_secs_f64() * 1000.0 / f64::from(n);
        println!("{name}: {ms:.2} ms/frame ({:.0} fps) over {n} frames", 1000.0 / ms);
    }
    Ok(())
}
