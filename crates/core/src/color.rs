//! How the YUV samples of a video stream map to R'G'B': the matrix, the range of the values and
//! where the chroma samples sit. `dc-media` reads it from the file, `dc-render` converts with it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoColor {
    pub matrix: ColorMatrix,
    pub range: ColorRange,
    pub chroma: ChromaSiting,
}

impl VideoColor {
    /// H.264 from our recordings and most HD files.
    pub const BT709: Self = Self {
        matrix: ColorMatrix::Bt709,
        range: ColorRange::Limited,
        chroma: ChromaSiting::Left,
    };
}

/// YUV → R'G'B' matrix (ITU-R BT.601, BT.709, BT.2020 non-constant luminance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorMatrix {
    Bt601,
    Bt709,
    Bt2020,
}

impl ColorMatrix {
    /// Luma weights of red and blue, `Kr` and `Kb`.
    pub const fn weights(self) -> (f64, f64) {
        match self {
            Self::Bt601 => (0.299, 0.114),
            Self::Bt709 => (0.2126, 0.0722),
            Self::Bt2020 => (0.2627, 0.0593),
        }
    }
}

/// Limited (TV): luma 16–235, chroma 16–240. Full (PC, JPEG): 0–255.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorRange {
    Limited,
    Full,
}

/// Where a 4:2:0 chroma sample sits against the 2×2 luma block it covers: horizontally on the left
/// luma column or between the columns, vertically on the top row, between the rows or on the
/// bottom row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChromaSiting {
    /// H.264, H.265 and MPEG-2.
    Left,
    /// JPEG, VP8 and MPEG-1.
    Center,
    TopLeft,
    Top,
    BottomLeft,
    Bottom,
}

impl ChromaSiting {
    /// Offset of the chroma sample from the centre of its 2×2 luma block, in luma pixels.
    pub const fn offset(self) -> (f32, f32) {
        match self {
            Self::Left => (-0.5, 0.0),
            Self::Center => (0.0, 0.0),
            Self::TopLeft => (-0.5, -0.5),
            Self::Top => (0.0, -0.5),
            Self::BottomLeft => (-0.5, 0.5),
            Self::Bottom => (0.0, 0.5),
        }
    }
}
