//! The output frame: its size in pixels, the canvas every effect lays the scene out on. Clips of
//! another aspect are fitted into it with black fields by their video layer.

use dc_core::{PixelSize, Project};

use crate::state::{Aspect, CropSettings};

/// The output frame before scaling, sides even; `None` without clips. `free` is the first clip's
/// picture after its crop, `source` its whole source frame; a fixed aspect is the smallest frame
/// of that aspect holding the first clip's crop at its own pixels (no detail lost, fields added).
pub fn canvas(project: &Project, settings: &CropSettings) -> Option<PixelSize> {
    let clip = project.clips.first()?;
    let [_, _, w, h] = clip.crop.to_pixels(clip.media.size);
    let source = clip.media.size;
    let fixed = |ratio: f64| {
        if w / h > ratio {
            (w, w / ratio)
        } else {
            (h * ratio, h)
        }
    };
    let (width, height) = match settings.aspect {
        Aspect::Free => (w, h),
        Aspect::Source => (f64::from(source.width), f64::from(source.height)),
        Aspect::Wide16x9 => fixed(16.0 / 9.0),
        Aspect::Wide16x10 => fixed(16.0 / 10.0),
        Aspect::Standard4x3 => fixed(4.0 / 3.0),
    };
    Some(PixelSize {
        width: even(width),
        height: even(height),
    })
}

/// `side` px rounded to an even number of at least 2: the encoder needs even sides.
fn even(side: f64) -> u32 {
    ((side / 2.0).round() as u32).max(1) * 2
}

#[cfg(test)]
#[path = "canvas_tests.rs"]
mod tests;
