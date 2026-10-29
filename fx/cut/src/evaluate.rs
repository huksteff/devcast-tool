//! The main track in the scene: the source frame of the clip playing at an output moment. Cuts,
//! clip order and speed reach the picture only through the time map.

use dc_core::{OutputTime, Project, TimeMap};
use dc_render::{Band, Layer, Scene, Video, VideoSource};

/// Puts the video of output moment `t` into `scene`: the clip's crop fitted into the frame. Outside
/// `0..duration` of `map` nothing is added and the frame stays black.
pub fn evaluate(project: &Project, map: &TimeMap, t: OutputTime, scene: &mut Scene) {
    let Some(segment) = map.segment_at(t) else {
        return;
    };
    let Some(clip) = project.clip(segment.clip) else {
        return;
    };
    let source = VideoSource {
        media: segment.media.clone(),
        time: segment.source_at(t),
    };
    let video = Video::fitted(source, clip.media.size, clip.crop, scene.size);
    scene.push(Band::Video, Layer::Video(video));
}

#[cfg(test)]
#[path = "evaluate_tests.rs"]
mod tests;
