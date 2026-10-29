use dc_core::PixelSize;
use dc_render::{AdapterChoice, Gpu, GpuError, SRGB_VIEW, Scene, draw};

const SIZE: PixelSize = PixelSize {
    width: 64,
    height: 36,
};

#[test]
fn empty_scene_is_an_opaque_black_frame() -> Result<(), GpuError> {
    let gpu = Gpu::new(AdapterChoice::Auto)?;
    let target = gpu.target(SIZE);
    draw(&gpu, &Scene::new(SIZE), &target);
    let image = gpu.read(&target)?;
    assert_eq!(image.rgba, [0, 0, 0, 255].repeat(64 * 36));
    Ok(())
}

/// Passes see linear light through the sRGB view: linear 0.5 is stored as sRGB byte 188, not 128.
#[test]
fn srgb_view_stores_linear_light_as_srgb_bytes() -> Result<(), GpuError> {
    let gpu = Gpu::new(AdapterChoice::Auto)?;
    let target = gpu.target(SIZE);
    let view = target.create_view(&wgpu::TextureViewDescriptor {
        format: Some(SRGB_VIEW),
        ..Default::default()
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    gpu.queue.submit([encoder.finish()]);
    let image = gpu.read(&target)?;
    assert_eq!(&image.rgba[..4], &[188, 188, 188, 255]);
    Ok(())
}
