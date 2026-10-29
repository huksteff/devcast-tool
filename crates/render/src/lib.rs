//! wgpu compositor: video, SDF shapes, blur and glow, text, sprites, strokes, motion blur, offscreen.

mod block;
mod draw;
mod error;
mod gpu;
mod readback;
pub mod scene;

pub use draw::draw;
pub use error::GpuError;
pub use gpu::{AdapterChoice, BACKEND, FORMAT, Gpu, SRGB_VIEW};
pub use readback::Image;
pub use scene::{Band, Camera, Layer, Scene};
