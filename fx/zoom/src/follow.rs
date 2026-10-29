//! Where a zoom looks (docs/porting/zoom.md §6): a fixed zoom at its focus; a follow zoom starts
//! there and re-centres on the cursor whenever it leaves the middle half of the zoomed-in view,
//! one axis at a time. The jumps are smoothed by the camera spring. Points are fractions of the
//! project frame.

use dc_core::Point;

use crate::frame::reachable;
use crate::model::{Mode, Zoom};

/// The part of the zoomed-in view, from each edge, where the cursor makes the camera move (v1
/// `SNAP_TO_EDGES_RATIO_AUTO`): the middle half of the view is the safe zone.
const EDGE: f64 = 0.25;
/// From this strength on the zoom counts as fully in; once it was, the zoom-out keeps the focus
/// still (v1).
const FULL: f64 = 0.99;

/// The focus of the zoom being followed. It belongs to one zoom: another zoom starts over.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Follow {
    zoom: Option<usize>,
    focus: Point,
    full: bool,
}

impl Follow {
    /// The focus of zoom number `index` at `strength` (0..1) with the cursor at `cursor`, if
    /// the recording has one there.
    pub(crate) fn focus(
        &mut self,
        index: usize,
        zoom: &Zoom,
        strength: f64,
        cursor: Option<Point>,
    ) -> Point {
        let start = reachable(zoom.focus, zoom.scale);
        if self.zoom != Some(index) {
            *self = Self {
                zoom: Some(index),
                focus: start,
                full: false,
            };
        }
        if zoom.mode == Mode::Fixed {
            return start;
        }
        if strength >= FULL {
            self.full = true;
        }
        let Some(cursor) = cursor.filter(|_| !self.full || strength >= FULL) else {
            return self.focus;
        };
        let half = 0.5 / zoom.scale;
        let safe = half * (1.0 - 2.0 * EDGE);
        let axis = |focus: f64, cursor: f64| {
            if (cursor - focus).abs() > safe {
                cursor
            } else {
                focus
            }
        };
        self.focus = reachable(
            Point {
                x: axis(self.focus.x, cursor.x),
                y: axis(self.focus.y, cursor.y),
            },
            zoom.scale,
        );
        self.focus
    }

    /// Forgets the zoom: the next one starts at its own focus.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use dc_core::{Point, Range, SourceTime};

    use super::Follow;
    use crate::model::{Mode, Zoom, ZoomState};

    fn zoom(mode: Mode) -> Zoom {
        Zoom {
            id: ZoomState::default().ids.allocate(),
            media: "media/a.mp4".to_owned(),
            range: Range::new(SourceTime::ZERO, SourceTime::from_secs_f64(1.0)).unwrap(),
            scale: 2.0,
            focus: Point { x: 0.5, y: 0.5 },
            mode,
        }
    }

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[test]
    fn camera_stays_while_the_cursor_is_in_the_middle_half_of_the_view() {
        let z = zoom(Mode::Follow);
        let mut follow = Follow::default();
        assert_eq!(follow.focus(0, &z, 1.0, Some(p(0.62, 0.38))), p(0.5, 0.5));
        assert_eq!(follow.focus(0, &z, 1.0, Some(p(0.63, 0.5))), p(0.63, 0.5));
        assert_eq!(follow.focus(0, &z, 1.0, Some(p(0.7, 0.6))), p(0.63, 0.5));
        assert_eq!(follow.focus(0, &z, 1.0, None), p(0.63, 0.5));
    }

    #[test]
    fn focus_stops_at_the_edge_of_the_picture() {
        let z = zoom(Mode::Follow);
        let mut follow = Follow::default();
        assert_eq!(follow.focus(0, &z, 1.0, Some(p(1.0, 1.0))), p(0.75, 0.75));
    }

    #[test]
    fn zoom_out_keeps_the_focus_still() {
        let z = zoom(Mode::Follow);
        let mut follow = Follow::default();
        assert_eq!(follow.focus(0, &z, 0.5, Some(p(0.9, 0.5))), p(0.75, 0.5));
        assert_eq!(follow.focus(0, &z, 1.0, Some(p(0.2, 0.5))), p(0.25, 0.5));
        assert_eq!(follow.focus(0, &z, 0.6, Some(p(0.8, 0.8))), p(0.25, 0.5));
    }

    #[test]
    fn fixed_zoom_and_another_zoom_start_at_their_own_focus() {
        let mut follow = Follow::default();
        let fixed = zoom(Mode::Fixed);
        assert_eq!(follow.focus(0, &fixed, 1.0, Some(p(0.9, 0.9))), p(0.5, 0.5));
        let z = zoom(Mode::Follow);
        assert_eq!(follow.focus(1, &z, 1.0, Some(p(0.9, 0.5))), p(0.75, 0.5));
        assert_eq!(follow.focus(2, &z, 1.0, Some(p(0.55, 0.5))), p(0.5, 0.5));
        follow.reset();
        assert_eq!(follow.focus(2, &z, 1.0, None), p(0.5, 0.5));
    }
}
