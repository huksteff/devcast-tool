use dc_core::project::{Media, MediaKind};
use dc_core::{Clip, Range, Rect, SourceTime, TimeDelta};
use serde_json::json;

use super::*;

fn clip_of(width: u32, height: u32, crop: Rect) -> Project {
    let mut project = Project::default();
    let id = project.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(5.0);
    project.clips.push(Clip {
        id,
        media: Media {
            path: "media/a.mp4".to_owned(),
            kind: MediaKind::Recording,
            duration,
            size: PixelSize { width, height },
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop,
        audio: Default::default(),
    });
    project
}

fn settings(aspect: Aspect) -> CropSettings {
    CropSettings { aspect }
}

const ASPECTS: [Aspect; 5] = [
    Aspect::Free,
    Aspect::Source,
    Aspect::Wide16x9,
    Aspect::Wide16x10,
    Aspect::Standard4x3,
];

#[test]
fn canvas_of_every_aspect_for_typical_first_clips() {
    let clips = [
        ("1920x1080", clip_of(1920, 1080, Rect::FULL)),
        ("1440x1080", clip_of(1440, 1080, Rect::FULL)),
        (
            "2560x1080 left half",
            clip_of(2560, 1080, Rect::new(0.0, 0.0, 0.5, 1.0).unwrap()),
        ),
    ];
    let table: Vec<_> = clips
        .iter()
        .flat_map(|(name, project)| {
            ASPECTS.iter().map(move |&aspect| {
                let canvas = canvas(project, &settings(aspect)).unwrap();
                json!({ "clip": name, "aspect": aspect, "canvas": canvas })
            })
        })
        .collect();
    insta::assert_json_snapshot!(table);
}

#[test]
fn fixed_aspect_holds_the_crop_at_its_own_pixels() {
    let crops = [
        Rect::FULL,
        Rect::new(0.1, 0.2, 0.3, 0.7).unwrap(),
        Rect::new(0.0, 0.45, 1.0, 0.05).unwrap(),
        Rect::new(0.5, 0.0, 0.05, 1.0).unwrap(),
    ];
    for crop in crops {
        let project = clip_of(1366, 768, crop);
        let [_, _, w, h] = crop.to_pixels(project.clips[0].media.size);
        for (aspect, ratio) in [
            (Aspect::Wide16x9, 16.0 / 9.0),
            (Aspect::Wide16x10, 1.6),
            (Aspect::Standard4x3, 4.0 / 3.0),
        ] {
            let c = canvas(&project, &settings(aspect)).unwrap();
            let (cw, ch) = (f64::from(c.width), f64::from(c.height));
            assert!(
                c.width.is_multiple_of(2) && c.height.is_multiple_of(2),
                "{c:?}"
            );
            assert!(
                cw >= w - 1.0 && ch >= h - 1.0,
                "{aspect}: {c:?} holds {w}×{h}"
            );
            assert!(
                cw <= w + 2.0 || ch <= h + 2.0,
                "{aspect}: {c:?} is the smallest"
            );
            assert!((cw / ch - ratio).abs() < 0.03, "{aspect}: {c:?}");
        }
    }
}

#[test]
fn only_the_first_clip_sets_the_frame_and_no_clip_has_none() {
    let mut project = clip_of(1440, 1080, Rect::FULL);
    let other = clip_of(3840, 2160, Rect::FULL).clips.remove(0);
    project.clips.push(other);
    let size = |width, height| PixelSize { width, height };
    assert_eq!(
        canvas(&project, &settings(Aspect::Source)),
        Some(size(1440, 1080))
    );
    assert_eq!(canvas(&Project::default(), &settings(Aspect::Source)), None);
}
