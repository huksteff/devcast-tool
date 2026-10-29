#![allow(
    dead_code,
    reason = "each test binary uses its own part of the helpers"
)]

use std::error::Error;

use dc_core::{PixelSize, Rect, SourceTime};
use dc_render::{
    AdapterChoice, Band, Compositor, Gpu, GpuError, Image, Layer, Nv12, RenderError, Scene, Video,
    VideoSource,
};

pub type TestResult = Result<(), Box<dyn Error>>;

pub fn size(width: u32, height: u32) -> PixelSize {
    PixelSize { width, height }
}

pub fn pixel(image: &Image, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * image.size.width + x) * 4) as usize;
    [0, 1, 2, 3].map(|i| image.rgba[at + i])
}

/// Each channel of `actual` is within `tolerance` of `expected`.
pub fn assert_near(actual: [u8; 4], expected: [u8; 4], tolerance: u8, what: &str) {
    let near = actual
        .iter()
        .zip(expected)
        .all(|(a, e)| a.abs_diff(e) <= tolerance);
    assert!(
        near,
        "{what}: {actual:?}, expected {expected:?} ±{tolerance}"
    );
}

/// Planes of an NV12 frame; `yuv(x, y)` gives the luma of a pixel and the chroma of the 2×2
/// block whose top-left pixel it is.
pub struct Planes {
    pub size: PixelSize,
    pub y: Vec<u8>,
    pub uv: Vec<u8>,
}

impl Planes {
    pub fn new(size: PixelSize, yuv: impl Fn(u32, u32) -> [u8; 3]) -> Self {
        let (w, h) = (size.width, size.height);
        let y = (0..h)
            .flat_map(|r| (0..w).map(move |c| (r, c)))
            .map(|(r, c)| yuv(c, r)[0]);
        let uv = (0..h.div_ceil(2))
            .flat_map(|r| (0..w.div_ceil(2)).map(move |c| (r, c)))
            .flat_map(|(r, c)| {
                let [_, u, v] = yuv(c * 2, r * 2);
                [u, v]
            });
        Self {
            size,
            y: y.collect(),
            uv: uv.collect(),
        }
    }

    pub fn nv12(&self) -> Result<Nv12<'_>, RenderError> {
        Nv12::new(self.size, &self.y, &self.uv)
    }
}

pub fn source(media: &str) -> VideoSource {
    VideoSource {
        media: media.into(),
        time: SourceTime::ZERO,
    }
}

/// A scene of `frame` size showing `planes` cropped to `crop` and fitted into it.
pub fn video_scene(frame: PixelSize, planes: &Planes, crop: Rect) -> Scene {
    let mut scene = Scene::new(frame);
    let video = Video::fitted(source("media/a.mp4"), planes.size, crop, frame);
    scene.push(Band::Video, Layer::Video(video));
    scene
}

/// Draws `scene` into a target of its size after uploading `planes` for every video layer.
pub fn render(gpu: &Gpu, scene: &Scene, planes: &Planes) -> Result<Image, Box<dyn Error>> {
    let mut compositor = Compositor::new(gpu);
    for (_, layer) in scene.painted() {
        if let Layer::Video(video) = layer {
            compositor.upload(gpu, &video.source, planes.nv12()?);
        }
    }
    let target = gpu.target(scene.size);
    compositor.draw(gpu, scene, &target)?;
    Ok(gpu.read(&target)?)
}

pub fn gpu() -> Result<Gpu, GpuError> {
    Gpu::new(AdapterChoice::Auto)
}

/// Draws a scene without video into a target of its size.
pub fn render_shapes(gpu: &Gpu, scene: &Scene) -> Result<Image, Box<dyn Error>> {
    let target = gpu.target(scene.size);
    Compositor::new(gpu).draw(gpu, scene, &target)?;
    Ok(gpu.read(&target)?)
}
