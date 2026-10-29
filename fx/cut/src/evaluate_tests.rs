use dc_core::{PixelSize, Rect};
use dc_render::Layer;
use serde_json::json;

use super::*;
use crate::test_util::{o, project, speed, sr};

const HD: PixelSize = PixelSize {
    width: 1920,
    height: 1080,
};

/// c1: 10 s, cuts 2–3 and 5–5.5, 2× on 6–8 (output 0–7.5); c2: 4 s, 4:3, cropped to its left
/// half (output 7.5–11.5).
fn edited() -> Project {
    let mut p = project(&[10.0, 4.0]);
    p.clips[0].cuts = vec![sr(2.0, 3.0), sr(5.0, 5.5)];
    p.clips[0].speeds = vec![speed(6.0, 8.0, 2.0)];
    p.clips[1].media.size = PixelSize {
        width: 1440,
        height: 1080,
    };
    p.clips[1].crop = Rect::new(0.0, 0.0, 0.5, 1.0).unwrap();
    p
}

fn scene_at(p: &Project, map: &TimeMap, secs: f64) -> Scene {
    let mut scene = Scene::new(HD);
    evaluate(p, map, o(secs), &mut scene);
    scene
}

#[test]
fn joins_cuts_and_speed_pick_the_right_source_frame() {
    let p = edited();
    let map = TimeMap::build(&p);
    let moments = [
        0.0, 1.9, 2.0, 3.999_999, 4.0, 4.4, 4.5, 5.0, 5.499_999, 5.5, 7.499_999, 7.5, 9.0,
        11.499_999, 11.5,
    ];
    let scenes: Vec<_> = moments
        .iter()
        .map(|&t| json!({ "t": t, "scene": scene_at(&p, &map, t) }))
        .collect();
    insta::assert_json_snapshot!(scenes);
}

#[test]
fn source_time_follows_the_map_at_every_moment() {
    let p = edited();
    let map = TimeMap::build(&p);
    for n in 0..=115 {
        let t = o(f64::from(n) / 10.0);
        let mut scene = Scene::new(HD);
        evaluate(&p, &map, t, &mut scene);
        let shown = scene.painted().find_map(|(_, layer)| match layer {
            Layer::Video(video) => Some((video.source.media.clone(), video.source.time)),
            _ => None,
        });
        let expected = map.segment_at(t).map(|s| (s.media.clone(), s.source_at(t)));
        assert_eq!(shown, expected, "{t:?}");
        assert!(scene.painted().count() <= 1);
    }
}
