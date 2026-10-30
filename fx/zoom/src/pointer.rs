//! The cursor of the recordings as a follow zoom sees it: where it is at an output moment, in
//! fractions of the project frame. Telemetry comes from the caller (`dc_core::CursorTracks`,
//! decision 015); the clip shown at that moment maps it through its crop and its place in the
//! frame.

use std::collections::BTreeMap;

use dc_core::{ClipId, CursorTracks, OutputTime, PixelSize, Point, Project, Rect, TimeMap};
use dc_render::{PxRect, Video, VideoSource};

/// Where each clip's picture lies: its crop of the recording and its rectangle in the frame.
pub(crate) struct Pointer<'a> {
    map: &'a TimeMap,
    cursors: &'a CursorTracks,
    clips: BTreeMap<ClipId, (Rect, PxRect)>,
    canvas: PixelSize,
}

impl<'a> Pointer<'a> {
    pub(crate) fn new(
        project: &Project,
        map: &'a TimeMap,
        cursors: &'a CursorTracks,
        canvas: PixelSize,
    ) -> Self {
        let clips = project
            .clips
            .iter()
            .map(|clip| {
                let source = VideoSource {
                    media: clip.media.path.clone(),
                    time: clip.trim.start(),
                };
                let video = Video::fitted(source, clip.media.size, clip.crop, canvas);
                (clip.id, (clip.crop, video.rect))
            })
            .collect();
        Self {
            map,
            cursors,
            clips,
            canvas,
        }
    }

    /// The cursor at output second `t`: the last position recorded up to the moment shown (the
    /// first one before it); `None` outside the video or without telemetry.
    pub(crate) fn at(&self, t: f64) -> Option<Point> {
        let segment = self.map.segment_at(OutputTime::from_secs_f64(t))?;
        let moves = &self.cursors.get(&segment.media)?.moves;
        let source = segment.source_at(OutputTime::from_secs_f64(t));
        let index = moves.partition_point(|m| m.at <= source).saturating_sub(1);
        let pos = moves.get(index)?.pos;
        let (crop, rect) = self.clips.get(&segment.clip)?;
        let local = crop.to_local(pos);
        Some(Point {
            x: (f64::from(rect.x) + local.x * f64::from(rect.w)) / f64::from(self.canvas.width),
            y: (f64::from(rect.y) + local.y * f64::from(rect.h)) / f64::from(self.canvas.height),
        })
    }
}

#[cfg(test)]
mod tests {
    use dc_core::project::{Media, MediaKind};
    use dc_core::{
        Clip, CursorPoint, CursorTrack, CursorTracks, PixelSize, Point, Project, Range, Rect,
        SourceTime, TimeDelta, TimeMap,
    };

    use super::Pointer;

    const HD: PixelSize = PixelSize {
        width: 1920,
        height: 1080,
    };

    fn secs(s: f64) -> SourceTime {
        SourceTime::from_secs_f64(s)
    }

    /// A 10 s recording cropped to its right half, cut at 2–4 s; the pointer at (0.6, 0.2) from
    /// 1 s and at (0.9, 0.8) from 5 s.
    fn setup() -> (Project, CursorTracks) {
        let mut p = Project::default();
        let id = p.clip_ids.allocate();
        let duration = TimeDelta::from_secs_f64(10.0);
        p.clips.push(Clip {
            id,
            media: Media {
                path: "media/recording-1/video.mp4".to_owned(),
                kind: MediaKind::Recording,
                duration,
                size: HD,
            },
            trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
            cuts: vec![Range::new(secs(2.0), secs(4.0)).unwrap()],
            speeds: Vec::new(),
            crop: Rect::new(0.5, 0.0, 0.5, 1.0).unwrap(),
            audio: Default::default(),
        });
        let moves = vec![
            CursorPoint {
                at: secs(1.0),
                pos: Point { x: 0.6, y: 0.2 },
            },
            CursorPoint {
                at: secs(5.0),
                pos: Point { x: 0.9, y: 0.8 },
            },
        ];
        let track = CursorTrack {
            moves,
            ..CursorTrack::default()
        };
        let cursors = CursorTracks::from([("media/recording-1/video.mp4".to_owned(), track)]);
        (p, cursors)
    }

    fn close(a: Point, x: f64, y: f64) -> bool {
        (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9
    }

    #[test]
    fn cursor_goes_through_the_crop_and_the_cut() {
        let (p, cursors) = setup();
        let map = TimeMap::build(&p);
        let pointer = Pointer::new(&p, &map, &cursors, HD);
        // The crop (960×1080) lies in the middle of the 1920×1080 frame: x 480..1440.
        let first = pointer.at(0.5).unwrap();
        assert!(close(first, 0.35, 0.2), "{first:?}");
        assert!(close(pointer.at(2.5).unwrap(), 0.35, 0.2));
        assert!(close(pointer.at(3.0).unwrap(), 0.65, 0.8));
        assert_eq!(pointer.at(9.0), None);
        assert_eq!(
            Pointer::new(&p, &map, &CursorTracks::new(), HD).at(1.0),
            None
        );
    }
}
