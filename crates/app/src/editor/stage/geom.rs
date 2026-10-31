//! Geometry of editing on the preview: where px of the output frame (the canvas, before the
//! camera) land on screen, the box of an annotation with its rotation, hit tests, and what a drag
//! of the box, a handle or the rotation knob makes of it.
//!
//! Units: canvas px, y down; angles in degrees clockwise (as `Annotation::rotation`). The camera
//! scales evenly and the frame keeps the canvas aspect, so the screen is a scaled and shifted
//! copy of the canvas: lengths and angles carry over, only the scale `k` changes.

use dc_core::{PixelSize, Rect as Fractions};
use dc_render::Camera;
use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};

/// Points the rotation knob stands above the top edge (mockup: 22).
pub const KNOB: f32 = 22.0;
/// How near a handle or the knob the pointer grabs it, points.
pub const GRAB: f32 = 8.0;
/// Rotation steps with Shift, degrees.
pub const SNAP_ANGLE: f32 = 15.0;
/// The rotation an annotation may have, degrees either way (`annot set --rotation`).
const TURN_LIMIT: f32 = 360.0;

/// Canvas px → screen points: `origin + p · k`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mapping {
    pub origin: Pos2,
    /// Screen points per canvas px.
    pub k: f32,
    /// Canvas size, px.
    pub canvas: Vec2,
}

impl Mapping {
    /// The canvas shown in `frame` (screen) through `camera` (px of the canvas).
    pub fn new(frame: Rect, canvas: PixelSize, camera: Camera) -> Self {
        let size = vec2(canvas.width.max(1) as f32, canvas.height.max(1) as f32);
        let s = frame.width() / size.x;
        Self {
            origin: frame.min + vec2(camera.x, camera.y) * s,
            k: camera.scale * s,
            canvas: size,
        }
    }

    pub fn screen(&self, p: Pos2) -> Pos2 {
        self.origin + p.to_vec2() * self.k
    }

    pub fn canvas(&self, p: Pos2) -> Pos2 {
        Pos2::ZERO + (p - self.origin) / self.k
    }

    /// `points` on screen as canvas px.
    pub fn length(&self, points: f32) -> f32 {
        points / self.k
    }
}

/// A handle of the box: a corner or the middle of an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl Handle {
    pub const ALL: [Self; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Right,
        Self::BottomRight,
        Self::Bottom,
        Self::BottomLeft,
        Self::Left,
    ];

    /// Where it sits on the unturned box: -1 left/top, 0 middle, 1 right/bottom.
    pub fn side(self) -> Vec2 {
        match self {
            Self::TopLeft => vec2(-1.0, -1.0),
            Self::Top => vec2(0.0, -1.0),
            Self::TopRight => vec2(1.0, -1.0),
            Self::Right => vec2(1.0, 0.0),
            Self::BottomRight => vec2(1.0, 1.0),
            Self::Bottom => vec2(0.0, 1.0),
            Self::BottomLeft => vec2(-1.0, 1.0),
            Self::Left => vec2(-1.0, 0.0),
        }
    }
}

/// What a press on the selected box holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grip {
    Move,
    Handle(Handle),
    Turn,
}

/// Modifiers of a resize: Shift keeps the proportion, Alt keeps the centre (common editors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Resize {
    pub keep_aspect: bool,
    pub from_centre: bool,
}

/// The box of an annotation: centre and size in canvas px, turned `angle` degrees clockwise
/// around the centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Boxed {
    pub centre: Pos2,
    pub size: Vec2,
    pub angle: f32,
}

impl Boxed {
    /// The box `rect` (fractions of the canvas) turned `rotation` degrees.
    pub fn of(rect: Fractions, rotation: f64, canvas: Vec2) -> Self {
        let (x, y, w, h) = (
            rect.x() as f32,
            rect.y() as f32,
            rect.w() as f32,
            rect.h() as f32,
        );
        Self {
            centre: pos2((x + w / 2.0) * canvas.x, (y + h / 2.0) * canvas.y),
            size: vec2(w * canvas.x, h * canvas.y),
            angle: rotation as f32,
        }
    }

    /// `x, y, w, h` in fractions of the canvas, as `annot set` takes them, to 4 decimals.
    pub fn fractions(&self, canvas: Vec2) -> [f64; 4] {
        let min = self.centre - self.size / 2.0;
        let round = |v: f32| (f64::from(v) * 10_000.0).round() / 10_000.0;
        [
            round(min.x / canvas.x),
            round(min.y / canvas.y),
            round(self.size.x / canvas.x),
            round(self.size.y / canvas.y),
        ]
    }

    /// The rotation as `annot set` takes it, to a hundredth of a degree.
    pub fn rotation(&self) -> f64 {
        (f64::from(self.angle) * 100.0).round() / 100.0
    }

    /// `v` turned by the box's angle.
    fn turn(&self, v: Vec2) -> Vec2 {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    }

    /// Where `p` lies relative to the centre in the axes of the box.
    pub fn local(&self, p: Pos2) -> Vec2 {
        let v = p - self.centre;
        let (sin, cos) = self.angle.to_radians().sin_cos();
        vec2(v.x * cos + v.y * sin, -v.x * sin + v.y * cos)
    }

    /// The canvas point at `v` from the centre in the axes of the box.
    pub fn world(&self, v: Vec2) -> Pos2 {
        self.centre + self.turn(v)
    }

    pub fn contains(&self, p: Pos2) -> bool {
        let v = self.local(p);
        v.x.abs() <= self.size.x / 2.0 && v.y.abs() <= self.size.y / 2.0
    }

    /// Where `handle` is, canvas px.
    pub fn handle(&self, handle: Handle) -> Pos2 {
        self.world(handle.side() * self.size / 2.0)
    }

    /// The four corners, clockwise from the top left.
    pub fn corners(&self) -> [Pos2; 4] {
        [
            Handle::TopLeft,
            Handle::TopRight,
            Handle::BottomRight,
            Handle::BottomLeft,
        ]
        .map(|h| self.handle(h))
    }

    /// The upright rectangle around the turned box.
    pub fn bounds(&self) -> Rect {
        Rect::from_points(&self.corners())
    }

    /// Where the rotation knob is: `knob` canvas px above the middle of the top edge.
    pub fn knob(&self, knob: f32) -> Pos2 {
        self.world(vec2(0.0, -self.size.y / 2.0 - knob))
    }

    pub fn moved(&self, by: Vec2) -> Self {
        Self {
            centre: self.centre + by,
            ..*self
        }
    }

    /// The box after `handle` is dragged to `pointer`: the opposite side stays (or the centre,
    /// `from_centre`), each side not below `least`; `keep_aspect` scales both sides alike by the
    /// larger change. The handle never passes the side that stays.
    pub fn resized(&self, handle: Handle, pointer: Pos2, how: Resize, least: Vec2) -> Self {
        let side = handle.side();
        let half = self.size / 2.0;
        let p = self.local(pointer);
        let mut size = self.size;
        for axis in 0..2 {
            let s = side[axis];
            if s == 0.0 {
                continue;
            }
            size[axis] = if how.from_centre {
                2.0 * (p[axis] * s).max(least[axis] / 2.0)
            } else {
                (p[axis] * s + half[axis]).max(least[axis])
            };
        }
        if how.keep_aspect {
            let factor = (0..2)
                .filter(|a| side[*a] != 0.0)
                .map(|a| size[a] / self.size[a].max(f32::EPSILON))
                .fold(0.0_f32, f32::max);
            let floor = (least.x / self.size.x).max(least.y / self.size.y);
            size = self.size * factor.max(floor);
        }
        // The centre in the box's axes: on a held side it moves with the far edge.
        let mut centre = Vec2::ZERO;
        for axis in 0..2 {
            let s = side[axis];
            if s != 0.0 && !how.from_centre {
                centre[axis] = s * (size[axis] / 2.0 - half[axis]);
            }
        }
        Self {
            centre: self.world(centre),
            size,
            angle: self.angle,
        }
    }

    /// The angle after the knob, pressed at `from`, is dragged to `to`; with `snap` a multiple
    /// of [`SNAP_ANGLE`]. Stays within ±360°.
    pub fn turned(&self, from: Pos2, to: Pos2, snap: bool) -> f32 {
        let bearing = |p: Pos2| {
            let v = p - self.centre;
            v.y.atan2(v.x).to_degrees()
        };
        let mut delta = bearing(to) - bearing(from);
        if delta > 180.0 {
            delta -= 360.0;
        } else if delta <= -180.0 {
            delta += 360.0;
        }
        let mut angle = self.angle + delta;
        if snap {
            angle = (angle / SNAP_ANGLE).round() * SNAP_ANGLE;
        }
        if angle > TURN_LIMIT {
            angle -= 360.0;
        } else if angle < -TURN_LIMIT {
            angle += 360.0;
        }
        angle
    }

    /// What the pointer at `p` (canvas) grabs: the knob, a handle, the inside; `grab` and
    /// `knob` are canvas px.
    pub fn grip(&self, p: Pos2, grab: f32, knob: f32) -> Option<Grip> {
        if self.knob(knob).distance(p) <= grab {
            return Some(Grip::Turn);
        }
        let handle = Handle::ALL
            .into_iter()
            .find(|h| self.handle(*h).distance(p) <= grab);
        if let Some(handle) = handle {
            return Some(Grip::Handle(handle));
        }
        self.contains(p).then_some(Grip::Move)
    }
}
