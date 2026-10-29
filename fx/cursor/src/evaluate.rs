//! The cursor in the scene: the picture the system showed during the recording («Системный»
//! set), at the drawn path, scaled to the clip (docs/porting/cursor.md §4, §7, §8).

use std::collections::BTreeMap;

use dc_core::{
    CursorPicture, CursorShape, CursorTracks, OutputTime, Point, Project, SourceTime, TimeMap,
};
use dc_render::{Band, ImageKey, Layer, PxRect, Scene, Sprite, Video, VideoSource};

use crate::path::Path;
use crate::state::CursorSettings;

/// Arrow height in px at size 1 in a clip 1920 px wide (v1 `h = 28 · size · W / 1920`).
const ARROW_PX: f64 = 28.0;
const REFERENCE_WIDTH: f64 = 1920.0;
/// Picture height over the height of the arrow in it: a Windows arrow is 19.0625 px of its 32 px
/// box (v1 `k_style`), so every picture is drawn this many times taller than `h`.
const PICTURE_PER_ARROW: f64 = 32.0 / 19.0625;
/// v1 `dotAlpha`.
const OPACITY: f32 = 0.95;
/// A pointer this far outside the crop still counts as inside it (v1, fractions of the crop).
const EDGE: f64 = 1e-6;

/// The cursor of one recording, ready for every frame.
#[derive(Debug)]
struct Recording {
    path: Path,
    shapes: Vec<CursorShape>,
}

/// The cursor of a project, prepared once by `dc_ops::Evaluator::new`.
#[derive(Debug)]
pub struct Prepared {
    settings: CursorSettings,
    recordings: BTreeMap<String, Recording>,
}

impl Prepared {
    /// Paths of every recording on the track that has mouse events; imported videos have none
    /// and show no cursor.
    pub fn new(project: &Project, settings: CursorSettings, tracks: CursorTracks) -> Self {
        let spring = settings.smoothing.spring();
        let mut recordings = BTreeMap::new();
        for (media, track) in tracks {
            let Some(clip) = project.clips.iter().find(|c| c.media.path == media) else {
                continue;
            };
            if track.moves.is_empty() {
                continue;
            }
            let length = clip.media.duration;
            let recording = Recording {
                path: Path::new(track.moves, spring.as_ref(), length),
                shapes: track.shapes,
            };
            recordings.insert(media, recording);
        }
        Self {
            settings,
            recordings,
        }
    }
}

/// Puts the cursor of output moment `t` into `scene`, over everything under the camera.
pub fn evaluate(
    prepared: &Prepared,
    project: &Project,
    map: &TimeMap,
    t: OutputTime,
    scene: &mut Scene,
) {
    if !prepared.settings.show {
        return;
    }
    let Some(segment) = map.segment_at(t) else {
        return;
    };
    let (Some(recording), Some(clip)) = (
        prepared.recordings.get(&segment.media),
        project.clip(segment.clip),
    ) else {
        return;
    };
    let source = segment.source_at(t);
    let Some(picture) = picture_at(&recording.shapes, source) else {
        return;
    };
    let content = Video::fitted(
        VideoSource {
            media: segment.media.clone(),
            time: source,
        },
        clip.media.size,
        clip.crop,
        scene.size,
    )
    .rect;
    let Some(tip) = on_frame(clip.crop.to_local(recording.path.at(source)), content) else {
        return;
    };
    let arrow = ARROW_PX * prepared.settings.size * f64::from(content.w) / REFERENCE_WIDTH;
    scene.push(Band::Cursor, Layer::Sprite(sprite(picture, tip, arrow)));
}

/// The picture shown at `t`: of the last shape change not after it, or of the first one;
/// `None` while the cursor is hidden.
fn picture_at(shapes: &[CursorShape], t: SourceTime) -> Option<&CursorPicture> {
    let index = shapes.partition_point(|s| s.at <= t);
    shapes.get(index.saturating_sub(1))?.picture.as_ref()
}

/// Frame px of point `local` of the crop shown in `content`; `None` outside the crop.
fn on_frame(local: Point, content: PxRect) -> Option<[f64; 2]> {
    let inside = |v: f64| (-EDGE..=1.0 + EDGE).contains(&v);
    (inside(local.x) && inside(local.y)).then(|| {
        [
            f64::from(content.x) + local.x * f64::from(content.w),
            f64::from(content.y) + local.y * f64::from(content.h),
        ]
    })
}

/// `picture` with its hotspot at `tip`, its arrow `arrow` px tall.
fn sprite(picture: &CursorPicture, tip: [f64; 2], arrow: f64) -> Sprite {
    let (pw, ph) = (
        f64::from(picture.size.width),
        f64::from(picture.size.height),
    );
    let h = arrow * PICTURE_PER_ARROW;
    let w = h * pw / ph;
    let rect = PxRect {
        x: (tip[0] - picture.hotspot.x / pw * w) as f32,
        y: (tip[1] - picture.hotspot.y / ph * h) as f32,
        w: w as f32,
        h: h as f32,
    };
    Sprite {
        image: ImageKey {
            path: picture.path.clone(),
            frame: 0,
        },
        rect,
        rotation: 0.0,
        opacity: OPACITY,
        motion: None,
    }
}

#[cfg(test)]
#[path = "evaluate_tests.rs"]
mod tests;
