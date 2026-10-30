//! wgpu compositor: video, SDF shapes, blur and glow, text, sprites, strokes, motion blur, offscreen.

mod block;
mod blur;
mod brush;
mod compositor;
mod error;
#[cfg(feature = "golden")]
pub mod golden;
mod gpu;
mod nv12_readback;
mod offscreen;
mod pipeline;
mod readback;
mod recorder;
pub mod scene;
mod shapes;
mod sprites;
mod text;
mod turn;
mod uniform;
mod video;

pub use compositor::Compositor;
pub use error::{GpuError, RenderError};
pub use gpu::{AdapterChoice, BACKEND, FORMAT, Gpu, SRGB_VIEW};
pub use nv12_readback::{Nv12Readback, Planes};
pub use offscreen::SHEET_GAP;
pub use readback::Image;
pub use scene::{
    Align, BLUE, Band, Beside, Blur, BrushStroke, Camera, DEFAULT_FAMILY, Drawing, FAMILIES, Font,
    Geometry, Glow, Gradient, GradientColors, ImageKey, Layer, Motion, Paint, ParseGradientError,
    Plate, PxRect, REFERENCE, Rgba, Scene, Shadow, Shape, Sprite, Stroke, Text, TextRun, Tool,
    Video, VideoSource,
};
pub use sprites::{Bitmap, IMAGE_BUDGET};
pub use text::{BUILTIN_FONTS, TextMeasure, builtin_dir as builtin_fonts_dir};
pub use video::Nv12;
