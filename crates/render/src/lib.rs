//! wgpu compositor: video, SDF shapes, blur and glow, text, sprites, strokes, motion blur, offscreen.

mod block;
mod compositor;
mod error;
mod gpu;
mod readback;
pub mod scene;
mod uniform;
mod video;

pub use compositor::Compositor;
pub use error::{GpuError, RenderError};
pub use gpu::{AdapterChoice, BACKEND, FORMAT, Gpu, SRGB_VIEW};
pub use readback::Image;
pub use scene::{Band, Camera, Layer, PxRect, Scene, Video, VideoSource};
pub use video::Nv12;
