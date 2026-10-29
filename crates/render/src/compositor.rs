//! `draw`: paints a [`Scene`] into a target texture. Pipelines and per-media textures live here
//! between frames, so a frame only uploads new pixels.

use std::collections::HashMap;

use dc_core::PixelSize;

use crate::error::RenderError;
use crate::gpu::{Gpu, SRGB_VIEW};
use crate::scene::{Camera, Layer, Scene, VideoSource};
use crate::shapes::ShapesPass;
use crate::video::{Nv12, VideoFrame, VideoPass};

/// The renderer of preview, export and the CLI.
pub struct Compositor {
    video: VideoPass,
    shapes: ShapesPass,
    /// Decoded frames by `Media::path`; a frame the drawn scene does not show is dropped.
    frames: HashMap<String, VideoFrame>,
}

impl Compositor {
    pub fn new(gpu: &Gpu) -> Self {
        Self {
            video: VideoPass::new(gpu),
            shapes: ShapesPass::new(gpu),
            frames: HashMap::new(),
        }
    }

    /// Hands over the decoded frame of `source`, before [`Self::draw`] of a scene showing it.
    pub fn upload(&mut self, gpu: &Gpu, source: &VideoSource, frame: Nv12<'_>) {
        let old = self.frames.remove(&source.media);
        let new = self.video.upload(gpu, old, source.time, frame);
        self.frames.insert(source.media.clone(), new);
    }

    /// Paints `scene` into `target` (a texture of [`Gpu::target`], any size: the scene is
    /// scaled to it) in one render pass: the frame is cleared to black, then the layers go
    /// bottom to top. Blending happens in linear light.
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        scene: &Scene,
        target: &wgpu::Texture,
    ) -> Result<(), RenderError> {
        self.prepare(gpu, scene, target)?;
        let view = target.create_view(&wgpu::TextureViewDescriptor {
            format: Some(SRGB_VIEW),
            ..Default::default()
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("draw"),
            });
        let mut pass = begin(&mut encoder, &view);
        // Shape layers were pushed in paint order, so their instances follow one another.
        let mut first = 0;
        for (_band, layer) in scene.painted() {
            match layer {
                Layer::Video(video) => self.video.record(&mut pass, self.frame(&video.source)?),
                Layer::Shapes(shapes) => {
                    let end = first + shapes.len() as u32;
                    self.shapes.record(&mut pass, first..end);
                    first = end;
                }
            }
        }
        drop(pass);
        gpu.queue.submit([encoder.finish()]);
        self.frames.retain(|media, _| {
            scene
                .painted()
                .any(|(_, layer)| matches!(layer, Layer::Video(v) if v.source.media == *media))
        });
        Ok(())
    }

    /// Writes what the layers need into GPU buffers before the render pass.
    fn prepare(
        &mut self,
        gpu: &Gpu,
        scene: &Scene,
        target: &wgpu::Texture,
    ) -> Result<(), RenderError> {
        self.shapes.clear();
        for (band, layer) in scene.painted() {
            let camera = if band.under_camera() {
                scene.camera
            } else {
                Camera::IDENTITY
            };
            match layer {
                Layer::Video(video) => {
                    let frame = self.frame(&video.source)?;
                    VideoPass::prepare(gpu, frame, video, camera, scene.size);
                }
                Layer::Shapes(shapes) => self.shapes.push(shapes, camera),
            }
        }
        let target = PixelSize {
            width: target.width(),
            height: target.height(),
        };
        self.shapes.upload(gpu, scene.size, target);
        Ok(())
    }

    fn frame(&self, source: &VideoSource) -> Result<&VideoFrame, RenderError> {
        self.frames
            .get(&source.media)
            .filter(|frame| frame.time == source.time)
            .ok_or_else(|| RenderError::NoFrame {
                media: source.media.clone(),
                time: source.time,
            })
    }
}

fn begin<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("scene"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    })
}
