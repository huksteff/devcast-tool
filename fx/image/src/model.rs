//! The fields of an image annotation, kept in `Annotation::body` (fx/annotations).

use dc_core::{PixelSize, TimeDelta};
use dc_fx_annotations::{Blend, step};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Folder of the project where pictures of image annotations are copied.
pub const IMAGES_DIR: &str = "assets/images";

/// What an image annotation shows. A new one shows nothing until `devcast image set --file`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ImageBody {
    /// The picture, copied into the project; none for a new annotation.
    pub picture: Option<PictureFile>,
}

/// A picture file of the project as the scene needs it, read once when it was copied in. Its
/// fields are lists and text, so a key snapshot between two pictures takes one of them whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PictureFile {
    /// Path in the project, `assets/images/logo.png`.
    pub path: String,
    /// Width and height, px of the picture.
    pub size: [u32; 2],
    /// Frames of an animation in order, as runs `[count, ms]`: `count` frames shown `ms` each.
    /// Empty for a still picture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<[u32; 2]>,
}

impl PictureFile {
    /// The picture at `path` of `size` whose frames show for `durations` (one zero for a still
    /// picture, as `dc_media::decode_picture` gives them); durations are kept in whole ms.
    pub fn new(path: String, size: PixelSize, durations: &[TimeDelta]) -> Self {
        let mut frames: Vec<[u32; 2]> = Vec::new();
        if durations.len() > 1 {
            for duration in durations {
                let ms = u32::try_from((duration.micros() + 500) / 1000).unwrap_or(0);
                match frames.last_mut() {
                    Some([count, last]) if *last == ms => *count += 1,
                    _ => frames.push([1, ms]),
                }
            }
        }
        Self {
            path,
            size: [size.width, size.height],
            frames,
        }
    }

    /// The frame shown `elapsed` after the picture appears; the animation loops.
    pub fn frame_at(&self, elapsed: TimeDelta) -> u32 {
        let total: i64 = self
            .frames
            .iter()
            .map(|[count, ms]| i64::from(*count) * i64::from(*ms) * 1000)
            .sum();
        if total <= 0 {
            return 0;
        }
        let mut at = elapsed.micros().rem_euclid(total);
        let mut frame = 0;
        for [count, ms] in &self.frames {
            let each = i64::from(*ms) * 1000;
            let run = i64::from(*count) * each;
            if at < run {
                return frame + u32::try_from(at / each.max(1)).unwrap_or(0);
            }
            at -= run;
            frame += count;
        }
        frame.saturating_sub(1)
    }
}

/// Between two keys the picture steps at the right key.
impl Blend for ImageBody {
    fn blend(&self, to: &Self, e: f64) -> Self {
        Self {
            picture: step(&self.picture, &to.picture, e),
        }
    }
}
