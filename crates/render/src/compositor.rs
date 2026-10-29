//! `draw`: paints a [`Scene`] into a target texture. Pipelines, fonts and per-media textures live
//! here between frames, so a frame only uploads new pixels.

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;

use dc_core::PixelSize;

use crate::blur::{BlurPass, Op};
use crate::brush::BrushPass;
use crate::error::RenderError;
use crate::gpu::{Gpu, SRGB_VIEW};
use crate::recorder::Recorder;
use crate::scene::{Blur, Camera, ImageKey, Layer, PxRect, Scene, Shape, Text, VideoSource};
use crate::shapes::{ShapesPass, glow_area};
use crate::sprites::{Bitmap, SpritesPass};
use crate::text::TextPass;
use crate::video::{Nv12, VideoFrame, VideoPass};

/// The renderer of preview, export and the CLI.
pub struct Compositor {
    video: VideoPass,
    shapes: ShapesPass,
    blur: BlurPass,
    text: TextPass,
    sprites: SpritesPass,
    brush: BrushPass,
    /// Decoded frames by `Media::path`; a frame the drawn scene does not show is dropped.
    frames: HashMap<String, VideoFrame>,
    /// What the frame being drawn records, in paint order.
    steps: Vec<Step>,
    ops: Vec<Op>,
}

/// One draw of a frame.
enum Step {
    Video(String),
    Shapes(Range<u32>),
    /// A region blur or a glow: `ops[i]`.
    Blur(usize),
    /// Glyphs of text renderer `i`.
    Text(usize),
    Sprite {
        image: ImageKey,
        slot: u32,
    },
    /// Drawing `i` of the brush pass.
    Drawing(usize),
}

impl Compositor {
    pub fn new(gpu: &Gpu) -> Self {
        Self {
            video: VideoPass::new(gpu),
            shapes: ShapesPass::new(gpu),
            blur: BlurPass::new(gpu),
            text: TextPass::new(gpu),
            sprites: SpritesPass::new(gpu),
            brush: BrushPass::new(gpu),
            frames: HashMap::new(),
            steps: Vec::new(),
            ops: Vec::new(),
        }
    }

    /// Loads the `.ttf` and `.otf` files of `dir`: the built-in fonts (`assets/fonts` of the
    /// program) and those of a project's `assets/`. No other fonts are used, the system's
    /// included (decision 005).
    pub fn load_fonts(&mut self, dir: &Path) -> Result<(), RenderError> {
        self.text.load_fonts(dir)
    }

    /// Hands over a picture (a frame of a decoded image file) that sprites show as `key`. It
    /// stays until the pictures exceed [`crate::IMAGE_BUDGET`]; [`Self::has_image`] tells whether
    /// it is still here.
    pub fn upload_image(
        &mut self,
        gpu: &Gpu,
        key: &ImageKey,
        bitmap: Bitmap<'_>,
    ) -> Result<(), RenderError> {
        self.sprites.upload(gpu, key, bitmap)
    }

    pub fn has_image(&self, key: &ImageKey) -> bool {
        self.sprites.contains(key)
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
        for step in &self.steps {
            match step {
                Step::Video(media) => self.video.record(recorder.pass(), &self.frames[media]),
                Step::Shapes(range) => self.shapes.record(recorder.pass(), range.clone()),
                Step::Blur(i) => {
                    self.blur
                        .record(&mut recorder, &source, &self.shapes, &self.ops[*i]);
                }
                Step::Text(i) => self.text.record(recorder.pass(), *i)?,
                Step::Sprite { image, slot } => {
                    self.sprites.record(recorder.pass(), image, *slot);
                }
                Step::Drawing(i) => self.brush.record(&mut recorder, *i),
            }
        }
        gpu.queue.submit([recorder.finish()]);
        self.text.trim();
        self.frames.retain(|media, _| {
            scene
                .painted()
                .any(|(_, layer)| matches!(layer, Layer::Video(v) if v.source.media == *media))
        });
        Ok(())
    }

    /// Writes what the layers need into GPU buffers and plans the draws.
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
        self.text.clear(gpu, scene.size, target);
        self.sprites.clear();
        self.brush.clear();
        self.steps.clear();
        self.ops.clear();
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
                    self.steps.push(Step::Video(video.source.media.clone()));
                }
                Layer::Shapes(shapes) => self.push_shapes(shapes, camera),
                Layer::Blur(blur) => {
                    if let Some(op) = self.blur.push_region(&through(blur, camera)) {
                        self.push_op(op);
                    }
                }
                Layer::Text(text) => self.push_text(gpu, text, camera)?,
                Layer::Sprite(sprite) => {
                    let slot = self.sprites.push(sprite, camera, scene.size, target)?;
                    let image = sprite.image.clone();
                    self.steps.push(Step::Sprite { image, slot });
                }
                Layer::Drawing(drawing) => {
                    if let Some(i) = self.brush.push(drawing, camera, scene.size, target)? {
                        self.steps.push(Step::Drawing(i));
                    }
                }
            }
        }
        self.shapes.upload(gpu);
        self.sprites.upload_slots(gpu);
        self.brush.upload(gpu, target);
        self.blur.upload(gpu, !self.ops.is_empty());
        Ok(())
    }

    /// Figures in runs; the glow of a figure goes between the run before it and the figure.
    fn push_shapes(&mut self, shapes: &[Shape], camera: Camera) {
        let mut run = self.shapes.push(shapes, camera);
        for (instance, shape) in run.clone().zip(shapes) {
            let Some((bounds, sigma)) = glow_area(shape, camera) else {
                continue;
            };
            if let Some(op) = self
                .blur
                .push_glow(&mut self.shapes, instance, bounds, sigma)
            {
                self.steps.push(Step::Shapes(run.start..instance));
                self.push_op(op);
                run.start = instance;
            }
        }
        self.steps.push(Step::Shapes(run));
    }

    fn push_text(&mut self, gpu: &Gpu, text: &Text, camera: Camera) -> Result<(), RenderError> {
        let (renderer, decor) = self.text.push(gpu, text, camera)?;
        let plates = self.shapes.push(&decor.plates, Camera::IDENTITY);
        self.steps.push(Step::Shapes(plates));
        self.steps.push(Step::Text(renderer));
        let underlines = self.shapes.push(&decor.underlines, Camera::IDENTITY);
        self.steps.push(Step::Shapes(underlines));
        Ok(())
    }

    fn push_op(&mut self, op: Op) {
        self.steps.push(Step::Blur(self.ops.len()));
        self.ops.push(op);
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
