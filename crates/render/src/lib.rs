//! wgpu compositor: video, SDF shapes, blur and glow, text, sprites, strokes, motion blur, offscreen.

mod block;
mod error;
mod gpu;
mod readback;

pub use error::GpuError;
pub use gpu::{AdapterChoice, BACKEND, FORMAT, Gpu};
pub use readback::Image;
