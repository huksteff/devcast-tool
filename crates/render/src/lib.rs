//! wgpu compositor: video, SDF shapes, blur and glow, text, sprites, strokes, motion blur, offscreen.

mod block;
mod blur;
mod brush;
mod compositor;
mod error;
mod gpu;
mod offscreen;
mod pipeline;
mod readback;
mod recorder;
pub mod scene;
mod shapes;
mod sprites;
mod text;
mod uniform;
mod video;

pub use compositor::Compositor;
pub use error::{GpuError, RenderError};
pub use gpu::{AdapterChoice, BACKEND, FORMAT, Gpu, SRGB_VIEW};
pub use offscreen::SHEET_GAP;
pub use readback::Image;
pub use scene::{
    Align, Band, Blur, BrushStroke, Camera, Drawing, Font, Geometry, Glow, Gradient,
    GradientColors, ImageKey, Layer, Motion, Paint, Plate, PxRect, Rgba, Scene, Shape, Sprite,
    Stroke, Text, TextRun, Tool, Video, VideoSource,
};
pub use sprites::{Bitmap, IMAGE_BUDGET};
pub use video::Nv12;
