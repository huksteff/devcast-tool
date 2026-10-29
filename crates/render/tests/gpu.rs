use dc_core::PixelSize;
use dc_render::{AdapterChoice, BACKEND, Gpu, GpuError, Image};

const COLOR: [u8; 4] = [51, 102, 204, 255];

fn clear(gpu: &Gpu, size: PixelSize) -> Result<Image, GpuError> {
    let target = gpu.target(size);
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let [r, g, b, a] = COLOR.map(|c| f64::from(c) / 255.0);
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    gpu.queue.submit([encoder.finish()]);
    gpu.read(&target)
}

fn assert_cleared(gpu: &Gpu) -> Result<(), GpuError> {
    // 67 * 4 bytes is not a multiple of the 256-byte row alignment, so rows come back padded.
    let size = PixelSize {
        width: 67,
        height: 5,
    };
    let image = clear(gpu, size)?;
    assert_eq!(image.size, size);
    assert_eq!(image.rgba, COLOR.repeat(67 * 5));
    Ok(())
}

#[test]
fn auto_device_clears_and_reads_back() -> Result<(), GpuError> {
    let gpu = Gpu::new(AdapterChoice::Auto)?;
    let info = gpu.info();
    eprintln!("auto adapter: {info:?}");
    assert_eq!(wgpu::Backends::from(info.backend), BACKEND);
    assert_cleared(&gpu)
}

#[test]
fn software_device_clears_and_reads_back() -> Result<(), GpuError> {
    let gpu = Gpu::new(AdapterChoice::Software)?;
    let info = gpu.info();
    eprintln!("software adapter: {info:?}");
    assert_eq!(info.device_type, wgpu::DeviceType::Cpu);
    assert_eq!(wgpu::Backends::from(info.backend), BACKEND);
    assert_cleared(&gpu)
}
