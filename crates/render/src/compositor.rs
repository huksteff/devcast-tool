//! `draw`: paints a [`Scene`] into a target texture. Pipelines and per-media textures live here
//! between frames, so a frame only uploads new pixels.

use std::collections::HashMap;

use dc_core::PixelSize;

use crate::blur::{BlurPass, Op};
use crate::error::RenderError;
use crate::gpu::{Gpu, SRGB_VIEW};
use crate::recorder::Recorder;
use crate::scene::{Blur, Camera, Layer, PxRect, Scene, Shape, VideoSource};
use crate::shapes::{ShapesPass, glow_area};
use crate::video::{Nv12, VideoFrame, VideoPass};

/// The renderer of preview, export and the CLI.
pub struct Compositor {
    video: VideoPass,
    shapes: ShapesPass,
    blur: BlurPass,
    /// Decoded frames by `Media::path`; a frame the drawn scene does not show is dropped.
    frames: HashMap<String, VideoFrame>,
    /// Blurs of the frame being drawn in paint order (region blurs and glows); `None` for one
    /// that is off the frame.
    ops: Vec<Option<Op>>,
}

impl Compositor {
    pub fn new(gpu: &Gpu) -> Self {
        Self {
            video: VideoPass::new(gpu),
            shapes: ShapesPass::new(gpu),
            blur: BlurPass::new(gpu),
            frames: HashMap::new(),
            ops: Vec::new(),
        }
    }

    /// Hands over the decoded frame of `source`, before [`Self::draw`] of a scene showing it.
    pub fn upload(&mut self, gpu: &Gpu, source: &VideoSource, frame: Nv12<'_>) {
        let old = self.frames.remove(&source.media);
        let new = self.video.upload(gpu, old, source.time, frame);
        self.frames.insert(source.media.clone(), new);
    }

    /// Paints `scene` into `target` (a texture of [`Gpu::target`], any size: the scene is
    /// scaled to it): the frame is cleared to black, then the layers go bottom to top. Blending
    /// happens in linear light. One render pass, left only for the passes of a blur or a glow.
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
        let source = self.blur.bind(gpu, &view);
        let mut recorder = Recorder::new(gpu, &view);
        let mut ops = self.ops.iter();
        // Shape layers were pushed in paint order, so their instances follow one another.
        let mut next = 0;
        for (_band, layer) in scene.painted() {
            match layer {
                Layer::Video(video) => {
                    let frame = self.frame(&video.source)?;
                    self.video.record(recorder.pass(), frame);
                }
                Layer::Shapes(shapes) => {
                    // A glow goes under its figure: the figures before it are drawn first.
                    let end = next + shapes.len() as u32;
                    let mut run = next;
                    for (i, shape) in (next..).zip(shapes) {
                        if shape.glow.is_some() {
                            self.shapes.record(recorder.pass(), run..i);
                            run = i;
                            self.record_op(&mut recorder, &source, ops.next());
                        }
                    }
                    self.shapes.record(recorder.pass(), run..end);
                    next = end;
                }
                Layer::Blur(_) => self.record_op(&mut recorder, &source, ops.next()),
            }
        }
        gpu.queue.submit([recorder.finish()]);
        self.frames.retain(|media, _| {
            scene
                .painted()
                .any(|(_, layer)| matches!(layer, Layer::Video(v) if v.source.media == *media))
        });
        Ok(())
    }

    fn record_op(
        &self,
        recorder: &mut Recorder<'_>,
        source: &wgpu::BindGroup,
        op: Option<&Option<Op>>,
    ) {
        if let Some(Some(op)) = op {
            self.blur.record(recorder, source, &self.shapes, op);
        }
    }

    /// Writes what the layers need into GPU buffers before the render pass.
    fn prepare(
        &mut self,
        gpu: &Gpu,
        scene: &Scene,
        target: &wgpu::Texture,
    ) -> Result<(), RenderError> {
        let target = PixelSize {
            width: target.width(),
            height: target.height(),
        };
        self.shapes.clear(scene.size, target);
        self.blur.clear(scene.size, target);
        self.ops.clear();
        let mut instance = 0;
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
                Layer::Shapes(shapes) => {
                    self.shapes.push(shapes, camera);
                    for shape in shapes {
                        self.push_glow(shape, camera, instance);
                        instance += 1;
                    }
                }
                Layer::Blur(blur) => {
                    let op = self.blur.push_region(&through(blur, camera));
                    self.ops.push(op);
                }
            }
        }
        self.shapes.upload(gpu);
        self.blur.upload(gpu, self.ops.iter().any(Option::is_some));
        Ok(())
    }

    fn push_glow(&mut self, shape: &Shape, camera: Camera, instance: u32) {
        if let Some((bounds, sigma)) = glow_area(shape, camera) {
            let op = self
                .blur
                .push_glow(&mut self.shapes, instance, bounds, sigma);
            self.ops.push(op);
        }
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

/// `blur` seen through `camera`: the rectangle, corner radius and sigma move and scale with it.
fn through(blur: &Blur, camera: Camera) -> Blur {
    let r = blur.rect;
    let [x, y] = camera.apply([r.x, r.y]);
    let k = camera.scale;
    Blur {
        rect: PxRect {
            x,
            y,
            w: r.w * k,
            h: r.h * k,
        },
        radius: blur.radius * k,
        sigma: blur.sigma * k,
        tint: blur.tint,
    }
}
