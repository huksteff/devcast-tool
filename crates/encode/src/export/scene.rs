//! The scene of an output moment before effects have `evaluate` (5.15+): the video of the clip
//! playing there, its crop fitted into the canvas. `evaluate` of the project replaces
//! [`clip_scene`] as the [`super::Timeline::scene`] of the export.

use dc_core::{OutputTime, PixelSize, Project, TimeMap};
use dc_render::{Band, Layer, Scene, Video, VideoSource};

/// The output frame before scaling: the first clip's picture after its crop, sides even.
/// `None` without clips.
pub fn canvas(project: &Project) -> Option<PixelSize> {
    let clip = project.clips.first()?;
    let [_, _, w, h] = clip.crop.to_pixels(clip.media.size);
    let even = |side: f64| ((side / 2.0).round() as u32).max(1) * 2;
    Some(PixelSize {
        width: even(w),
        height: even(h),
    })
}

/// What output moment `t` shows: black outside `0..duration` of `map`.
pub fn clip_scene(project: &Project, map: &TimeMap, canvas: PixelSize, t: OutputTime) -> Scene {
    let mut scene = Scene::new(canvas);
    let Some(segment) = map.segment_at(t) else {
        return scene;
    };
    let Some(clip) = project.clip(segment.clip) else {
        return scene;
    };
    let source = VideoSource {
        media: segment.media.clone(),
        time: segment.source_at(t),
    };
    let video = Video::fitted(source, clip.media.size, clip.crop, canvas);
    scene.push(Band::Video, Layer::Video(video));
    scene
}
