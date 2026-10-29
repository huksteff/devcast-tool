//! What one output frame shows: the result of `evaluate`, the only input of `draw` (decision 4 of
//! ARCHITECTURE). Plain data without wgpu, serialized for the JSON snapshots of `evaluate` tests.
//!
//! Coordinates are pixels of the output frame (`size`), y down. A target of another size gets
//! the same picture scaled, so the preview may render smaller than the export.

mod blur;
mod shape;
mod video;

use std::collections::BTreeMap;

use dc_core::PixelSize;
use serde::{Deserialize, Serialize};

pub use blur::Blur;
pub use shape::{
    Geometry, Glow, Gradient, GradientColors, GradientCount, Paint, ParseRgbaError, Rgba, Shape,
    Stroke,
};
pub use video::{PxRect, Video, VideoSource};

/// Everything drawn in one output frame. The frame starts black (the fields around a clip, SCOPE
/// «Кадр»); layers are painted band by band, bottom to top, and in push order within a band.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub size: PixelSize,
    pub camera: Camera,
    pub layers: BTreeMap<Band, Vec<Layer>>,
}

impl Scene {
    /// An empty black frame with the camera at rest.
    pub fn new(size: PixelSize) -> Self {
        Self {
            size,
            camera: Camera::IDENTITY,
            layers: BTreeMap::new(),
        }
    }

    /// Puts `layer` on top of everything already in `band`.
    pub fn push(&mut self, band: Band, layer: Layer) {
        self.layers.entry(band).or_default().push(layer);
    }

    /// Layers in paint order, bottom first.
    pub fn painted(&self) -> impl Iterator<Item = (Band, &Layer)> {
        self.layers
            .iter()
            .flat_map(|(band, layers)| layers.iter().map(|layer| (*band, layer)))
    }
}

/// Paint order of the effects, bottom to top (v1: video, annotations and the cursor layer move
/// with the camera; captions and keys stay on top of the frame).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    Video,
    Annotations,
    /// Click effects, the cursor and its callout.
    Cursor,
    /// Captions and pressed keys.
    Overlay,
}

impl Band {
    /// Whether the zoom camera moves and scales this band.
    pub const fn under_camera(self) -> bool {
        !matches!(self, Self::Overlay)
    }
}

/// Zoom camera: a point `p` of the canvas lands in the frame at `p * scale + (x, y)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub scale: f32,
    pub x: f32,
    pub y: f32,
}

impl Camera {
    pub const IDENTITY: Self = Self {
        scale: 1.0,
        x: 0.0,
        y: 0.0,
    };

    /// Where canvas point `p` lands in the frame.
    pub fn apply(&self, p: [f32; 2]) -> [f32; 2] {
        [p[0] * self.scale + self.x, p[1] * self.scale + self.y]
    }
}

/// One drawable thing; each kind is drawn by its own pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Video(Video),
    /// Figures drawn in one instanced pass, later ones on top.
    Shapes(Vec<Shape>),
    Blur(Blur),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_scales_then_moves() {
        let camera = Camera {
            scale: 2.0,
            x: -960.0,
            y: -540.0,
        };
        assert_eq!(camera.apply([960.0, 540.0]), [960.0, 540.0]);
        assert_eq!(camera.apply([0.0, 0.0]), [-960.0, -540.0]);
        assert_eq!(Camera::IDENTITY.apply([3.0, 4.0]), [3.0, 4.0]);
    }

    #[test]
    fn bands_paint_bottom_to_top_and_overlay_ignores_camera() {
        let order = [Band::Overlay, Band::Video, Band::Cursor, Band::Annotations];
        let mut sorted = order;
        sorted.sort();
        assert_eq!(
            sorted,
            [Band::Video, Band::Annotations, Band::Cursor, Band::Overlay]
        );
        assert!(Band::Cursor.under_camera());
        assert!(!Band::Overlay.under_camera());
    }

    #[test]
    fn video_layer_round_trips_through_json() {
        let size = PixelSize {
            width: 1280,
            height: 720,
        };
        let mut scene = Scene::new(size);
        let source = VideoSource {
            media: "media/запись 1.mp4".into(),
            time: dc_core::SourceTime::from_micros(1_500_000),
        };
        scene.push(
            Band::Video,
            Layer::Video(Video::fitted(source, size, dc_core::Rect::FULL, size)),
        );
        let json = serde_json::to_string(&scene).unwrap();
        assert!(
            json.contains(r#""layers":{"video":[{"video":{"source":{"media":"media/запись 1.mp4","time":1500000}"#),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<Scene>(&json).unwrap(), scene);
    }

    #[test]
    fn empty_scene_round_trips_through_json() {
        let scene = Scene::new(PixelSize {
            width: 1920,
            height: 1080,
        });
        let json = serde_json::to_string(&scene).unwrap();
        assert_eq!(
            json,
            r#"{"size":{"width":1920,"height":1080},"camera":{"scale":1.0,"x":0.0,"y":0.0},"layers":{}}"#
        );
        assert_eq!(serde_json::from_str::<Scene>(&json).unwrap(), scene);
    }
}
