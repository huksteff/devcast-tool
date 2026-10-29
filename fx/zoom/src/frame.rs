//! The zoom frame on the preview (docs/porting/zoom.md §3): the part of the project frame that
//! fills the output when zoomed in. Its proportions are the frame's, its size `1/scale` of it;
//! dragging it moves the focus, dragging a corner sets the scale. Fractions of the project frame.

use dc_core::{Point, Rect};

use crate::model::{SCALE_MAX, SCALE_MIN};

/// A corner of the zoom frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// The focus actually used at `scale`: as close to `focus` as the frame stays inside the picture.
pub fn reachable(focus: Point, scale: f64) -> Point {
    let margin = 0.5 / scale;
    Point {
        x: focus.x.clamp(margin, 1.0 - margin),
        y: focus.y.clamp(margin, 1.0 - margin),
    }
}

/// The frame of a zoom with `scale` (1..5) and `focus`.
pub fn frame(scale: f64, focus: Point) -> Rect {
    let size = 1.0 / scale;
    let centre = reachable(focus, scale);
    Rect::new(centre.x - size / 2.0, centre.y - size / 2.0, size, size).unwrap_or(Rect::FULL)
}

/// Scale and focus after dragging `corner` of `frame` to `pointer`: the opposite corner stays
/// (moved inwards if the frame could not grow to the smallest size there), the size follows the
/// pointer on both axes on average, so the frame keeps its proportions.
pub fn resize(frame: Rect, corner: Corner, pointer: Point) -> (f64, Point) {
    let (sx, sy) = match corner {
        Corner::TopLeft => (-1.0, -1.0),
        Corner::TopRight => (1.0, -1.0),
        Corner::BottomLeft => (-1.0, 1.0),
        Corner::BottomRight => (1.0, 1.0),
    };
    let smallest = 1.0 / SCALE_MAX;
    let anchor = |start: f64, size: f64, sign: f64| {
        if sign > 0.0 {
            start.min(1.0 - smallest)
        } else {
            (start + size).max(smallest)
        }
    };
    let ax = anchor(frame.x(), frame.w(), sx);
    let ay = anchor(frame.y(), frame.h(), sy);
    let room = |anchor: f64, sign: f64| if sign > 0.0 { 1.0 - anchor } else { anchor };
    let largest = room(ax, sx).min(room(ay, sy)).min(1.0 / SCALE_MIN);
    let size = (((pointer.x - ax) * sx + (pointer.y - ay) * sy) / 2.0)
        .min(largest)
        .max(smallest);
    let focus = Point {
        x: ax + sx * size / 2.0,
        y: ay + sy * size / 2.0,
    };
    (1.0 / size, focus)
}

#[cfg(test)]
mod tests {
    use dc_core::{Point, Rect};

    use super::{Corner, frame, reachable, resize};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn frame_is_a_scaled_copy_of_the_picture_kept_inside() {
        let f = frame(2.0, Point { x: 0.3, y: 0.4 });
        let expected = [0.05, 0.15, 0.5, 0.5];
        let got = [f.x(), f.y(), f.w(), f.h()];
        assert!(
            got.iter().zip(expected).all(|(a, b)| close(*a, b)),
            "{got:?}"
        );
        let corner = frame(2.0, Point { x: 1.0, y: 0.0 });
        assert_eq!([corner.x(), corner.y()], [0.5, 0.0]);
        assert_eq!(frame(1.0, Point { x: 0.9, y: 0.1 }), Rect::FULL);
        assert_eq!(
            reachable(Point { x: 1.0, y: 1.0 }, 2.0),
            Point { x: 0.75, y: 0.75 }
        );
    }

    #[test]
    fn dragging_a_corner_keeps_the_opposite_one() {
        let f = frame(2.0, Point { x: 0.5, y: 0.5 });
        let (scale, focus) = resize(f, Corner::BottomRight, Point { x: 0.5, y: 0.5 });
        assert!(close(scale, 4.0));
        assert!(close(focus.x, 0.375) && close(focus.y, 0.375));
        let grown = frame(scale, focus);
        assert!(close(grown.x(), 0.25) && close(grown.y(), 0.25));
        let (scale, _) = resize(f, Corner::TopLeft, Point { x: -1.0, y: -1.0 });
        assert!(close(scale, 1.0 / 0.75));
    }

    #[test]
    fn frame_never_leaves_the_picture_nor_exceeds_the_scale_limits() {
        let f = Rect::new(0.85, 0.85, 0.1, 0.1).unwrap();
        let (scale, focus) = resize(f, Corner::BottomRight, Point { x: 2.0, y: 2.0 });
        assert!(close(scale, 5.0));
        let moved = frame(scale, focus);
        assert!(moved.x() + moved.w() <= 1.0 + 1e-9 && moved.y() + moved.h() <= 1.0 + 1e-9);
        let (tiny, _) = resize(f, Corner::TopLeft, Point { x: 0.94, y: 0.94 });
        assert!(close(tiny, 5.0));
        let full = Rect::FULL;
        let (whole, _) = resize(full, Corner::BottomRight, Point { x: 3.0, y: 3.0 });
        assert!(close(whole, 1.0));
    }
}
