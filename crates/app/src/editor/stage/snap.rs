//! Snapping while a box is dragged on the preview (7.23): its edges and centre stick to the
//! edges and centre of the frame and to those of the other annotations shown, closer than a
//! threshold; a guide line shows each line it stuck to. Canvas px; the caller turns the
//! on-screen threshold into px (`Mapping::length`).

use eframe::egui::{Pos2, Rect, Vec2, vec2};

/// How near on screen a line pulls an edge or a centre, points.
pub const PULL: f32 = 6.0;

/// A line something sticks to: vertical at `x = at`, or horizontal at `y = at`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guide {
    pub vertical: bool,
    pub at: f32,
    /// A line of the frame (its edge or centre); otherwise of another annotation.
    pub frame: bool,
}

/// The lines to stick to, per axis: position and whether it is the frame's.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lines {
    xs: Vec<(f32, bool)>,
    ys: Vec<(f32, bool)>,
}

impl Lines {
    /// The edges and centres of the canvas (`canvas` px large) and of `others` (upright bounds of
    /// the other annotations shown).
    pub fn new(canvas: Vec2, others: &[Rect]) -> Self {
        let thirds = |lo: f32, hi: f32| [lo, (lo + hi) / 2.0, hi];
        let mut lines = Self::default();
        for x in thirds(0.0, canvas.x) {
            lines.xs.push((x, true));
        }
        for y in thirds(0.0, canvas.y) {
            lines.ys.push((y, true));
        }
        for other in others {
            for x in thirds(other.left(), other.right()) {
                lines.xs.push((x, false));
            }
            for y in thirds(other.top(), other.bottom()) {
                lines.ys.push((y, false));
            }
        }
        lines
    }

    /// The shift that brings the nearest of `marks` onto a line of `lines`, if one is within
    /// `pull`; with the lines it lands on (several may coincide).
    fn nearest(lines: &[(f32, bool)], marks: &[f32], pull: f32) -> Option<(f32, Vec<(f32, bool)>)> {
        let mut best: Option<f32> = None;
        for &(line, _) in lines {
            for &mark in marks {
                let d = line - mark;
                if d.abs() <= pull && best.is_none_or(|b| d.abs() < b.abs()) {
                    best = Some(d);
                }
            }
        }
        let shift = best?;
        let hit = lines
            .iter()
            .copied()
            .filter(|(line, _)| marks.iter().any(|m| (m + shift - line).abs() < 0.01))
            .collect();
        Some((shift, hit))
    }

    /// The shift that makes a moved box (upright bounds `bounds`) stick: its left, centre and
    /// right to the vertical lines, its top, middle and bottom to the horizontal ones; and
    /// the guides it sticks to.
    pub fn moved(&self, bounds: Rect, pull: f32) -> (Vec2, Vec<Guide>) {
        let xs = [bounds.left(), bounds.center().x, bounds.right()];
        let ys = [bounds.top(), bounds.center().y, bounds.bottom()];
        let mut shift = Vec2::ZERO;
        let mut guides = Vec::new();
        if let Some((dx, hit)) = Self::nearest(&self.xs, &xs, pull) {
            shift.x = dx;
            guides.extend(hit.into_iter().map(|(at, frame)| Guide {
                vertical: true,
                at,
                frame,
            }));
        }
        if let Some((dy, hit)) = Self::nearest(&self.ys, &ys, pull) {
            shift.y = dy;
            guides.extend(hit.into_iter().map(|(at, frame)| Guide {
                vertical: false,
                at,
                frame,
            }));
        }
        (shift, guides)
    }

    /// The pointer dragging a handle of an upright box sticks to the lines on the axes the
    /// handle moves (`side`: -1, 0, 1 per axis, as `Handle::side`).
    pub fn pointer(&self, p: Pos2, side: Vec2, pull: f32) -> (Pos2, Vec<Guide>) {
        let mut snapped = p;
        let mut guides = Vec::new();
        if side.x != 0.0
            && let Some((dx, hit)) = Self::nearest(&self.xs, &[p.x], pull)
        {
            snapped.x += dx;
            guides.extend(hit.into_iter().map(|(at, frame)| Guide {
                vertical: true,
                at,
                frame,
            }));
        }
        if side.y != 0.0
            && let Some((dy, hit)) = Self::nearest(&self.ys, &[p.y], pull)
        {
            snapped.y += dy;
            guides.extend(hit.into_iter().map(|(at, frame)| Guide {
                vertical: false,
                at,
                frame,
            }));
        }
        (snapped, guides)
    }
}

/// The canvas axes a handle on `side` (as `Handle::side`) moves when the box is turned `angle`
/// degrees: only for a box whose edges lie along the axes (a quarter turn swaps them); `None`
/// for a box turned otherwise, whose handles do not snap.
pub fn axes(angle: f32, side: Vec2) -> Option<Vec2> {
    let quarters = angle / 90.0;
    if (quarters - quarters.round()).abs() > 1e-3 {
        return None;
    }
    let odd = (quarters.round() as i64).rem_euclid(2) == 1;
    Some(if odd { vec2(side.y, side.x) } else { side })
}
