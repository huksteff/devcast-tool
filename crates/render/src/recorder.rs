//! Commands of one frame: the render pass into the target, left for the passes of a blur and
//! resumed after them.

use crate::gpu::Gpu;

pub(crate) struct Recorder<'v> {
    encoder: wgpu::CommandEncoder,
    target: &'v wgpu::TextureView,
    pass: Option<wgpu::RenderPass<'static>>,
}

impl<'v> Recorder<'v> {
    /// Starts the frame: `target` is cleared to black.
    pub(crate) fn new(gpu: &Gpu, target: &'v wgpu::TextureView) -> Self {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("draw"),
            });
        let pass = begin(
            &mut encoder,
            target,
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
        Self {
            pass: Some(pass.forget_lifetime()),
            encoder,
            target,
        }
    }

    /// The pass into the target, resumed with what is drawn so far.
    pub(crate) fn pass(&mut self) -> &mut wgpu::RenderPass<'static> {
        let Self {
            encoder,
            target,
            pass,
        } = self;
        pass.get_or_insert_with(|| begin(encoder, target, wgpu::LoadOp::Load).forget_lifetime())
    }

    /// The encoder for passes into other textures; the pass into the target ends.
    pub(crate) fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        self.pass = None;
        &mut self.encoder
    }

    pub(crate) fn finish(mut self) -> wgpu::CommandBuffer {
        self.pass = None;
        self.encoder.finish()
    }
}

fn begin<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("scene"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    })
}
