//! The output frame: its size in pixels, the canvas every effect lays the scene out on.

use dc_core::{PixelSize, Project};

/// The output frame before scaling: the first clip's picture after its crop, sides even. `None`
/// without clips.
pub fn canvas(project: &Project) -> Option<PixelSize> {
    let clip = project.clips.first()?;
    let [_, _, w, h] = clip.crop.to_pixels(clip.media.size);
    Some(PixelSize {
        width: even(w),
        height: even(h),
    })
}

/// `side` px rounded to an even number of at least 2: the encoder needs even sides.
fn even(side: f64) -> u32 {
    ((side / 2.0).round() as u32).max(1) * 2
}
