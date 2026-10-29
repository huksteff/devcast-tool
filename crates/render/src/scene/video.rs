//! Video layer: a decoded frame of a media file, cropped and placed on the canvas.

use dc_core::{PixelSize, Rect, SourceTime};
use serde::{Deserialize, Serialize};

/// A frame of a media file, cropped and fitted into the frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    /// The frame to show: the caller decodes it and hands it to `Compositor::upload`.
    pub source: VideoSource,
    /// Part of the source frame shown, in fractions of it (the clip's crop).
    pub crop: Rect,
    /// Where the cropped part lands on the canvas, px.
    pub rect: PxRect,
}

/// Which decoded frame a video layer shows.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoSource {
    /// `Media::path` of the clip.
    pub media: String,
    /// Source time of the frame (`Decoder::frame_at`).
    pub time: SourceTime,
}

/// Rectangle in pixels: top-left corner and size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PxRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Video {
    /// `crop` of a `source`-sized video fitted into a `frame`-sized frame: as large as fits,
    /// centred, proportions kept; the rest of the frame stays black (SCOPE «Кадр»).
    pub fn fitted(source: VideoSource, size: PixelSize, crop: Rect, frame: PixelSize) -> Self {
        let [_, _, w, h] = crop.to_pixels(size);
        let (fw, fh) = (f64::from(frame.width), f64::from(frame.height));
        let scale = (fw / w).min(fh / h);
        let (w, h) = (w * scale, h * scale);
        let rect = PxRect {
            x: ((fw - w) / 2.0) as f32,
            y: ((fh - h) / 2.0) as f32,
            w: w as f32,
            h: h as f32,
        };
        Self { source, crop, rect }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HD: PixelSize = PixelSize {
        width: 1920,
        height: 1080,
    };

    fn fit(width: u32, height: u32, crop: Rect) -> PxRect {
        let source = VideoSource {
            media: "media/a.mp4".into(),
            time: SourceTime::ZERO,
        };
        Video::fitted(source, PixelSize { width, height }, crop, HD).rect
    }

    #[test]
    fn same_proportions_fill_the_frame() {
        let full = PxRect {
            x: 0.0,
            y: 0.0,
            w: 1920.0,
            h: 1080.0,
        };
        assert_eq!(fit(1920, 1080, Rect::FULL), full);
        assert_eq!(fit(3840, 2160, Rect::FULL), full);
        assert_eq!(fit(1280, 720, Rect::FULL), full);
    }

    #[test]
    fn narrower_clip_gets_fields_left_and_right() {
        let rect = fit(1440, 1080, Rect::FULL);
        assert_eq!(
            rect,
            PxRect {
                x: 240.0,
                y: 0.0,
                w: 1440.0,
                h: 1080.0
            }
        );
    }

    #[test]
    fn wider_crop_gets_fields_top_and_bottom() {
        // The top half of a 16:9 frame is 32:9.
        let crop = Rect::new(0.0, 0.0, 1.0, 0.5).unwrap();
        let rect = fit(1920, 1080, crop);
        assert_eq!(
            rect,
            PxRect {
                x: 0.0,
                y: 270.0,
                w: 1920.0,
                h: 540.0
            }
        );
    }
}
