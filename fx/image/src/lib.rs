//! Image annotation: a picture file copied into the project's `assets/` and drawn in the
//! annotation box keeping its proportions; an animated GIF or WebP plays
//! (docs/porting/annotations.md §7, decision 000).
//!
//! Copying and decoding the file is the caller's (`devcast image set`, dc-cli): this crate keeps
//! what the scene needs of the picture (path, size, frame timing) in the annotation body.

mod commands;
mod evaluate;
pub mod look;
mod model;
pub mod panel;

pub use commands::ImageSet;
pub use evaluate::evaluate;
pub use model::{IMAGES_DIR, ImageBody, PictureFile};

#[cfg(test)]
mod evaluate_tests;
#[cfg(test)]
mod panel_tests;
#[cfg(test)]
mod tests;
