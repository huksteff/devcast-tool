//! The crop frame of the dialog in pixels of the clip's source: dragging it or its handles, the
//! numeric fields, the aspect lock. The frame always lies inside the source frame and keeps
//! each side at least `CROP_MIN` of the source side (as `devcast crop` checks).

use dc_core::project::CROP_MIN;
use dc_core::{PixelSize, Rect};

/// The crop frame: top-left corner and size, px of the source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// What a drag holds: the whole frame, an edge or a corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    Move,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Handle {
    /// The eight handles, corners first (a corner wins where they overlap).
    pub const SIDES: [Self; 8] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
        Self::Left,
        Self::Right,
        Self::Top,
        Self::Bottom,
    ];

    /// Which way the handle moves an edge: −1 the left (top), +1 the right (bottom), 0 none.
    pub const fn signs(self) -> (i8, i8) {
        match self {
            Self::Move => (0, 0),
            Self::Left => (-1, 0),
            Self::Right => (1, 0),
            Self::Top => (0, -1),
            Self::Bottom => (0, 1),
            Self::TopLeft => (-1, -1),
            Self::TopRight => (1, -1),
            Self::BottomLeft => (-1, 1),
            Self::BottomRight => (1, 1),
        }
    }

    /// Where the handle sits on the frame, fractions of it (0, 0 the top-left corner).
    pub fn anchor(self) -> (f32, f32) {
        let (sx, sy) = self.signs();
        (f32::from(sx + 1) / 2.0, f32::from(sy + 1) / 2.0)
    }
}

/// One of the numeric fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    X,
    Y,
    Width,
    Height,
}

/// The source frame the crop frame lives in, px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub w: f64,
    pub h: f64,
}

impl Area {
    pub fn of(size: PixelSize) -> Self {
        Self {
            w: f64::from(size.width.max(1)),
            h: f64::from(size.height.max(1)),
        }
    }

    fn least(self) -> (f64, f64) {
        (self.w * CROP_MIN, self.h * CROP_MIN)
    }

    /// The least and the most width of a frame of `ratio` (w / h) at most `room` wide and
    /// `room_h` high.
    fn widths(self, ratio: f64, room: f64, room_h: f64) -> (f64, f64) {
        let (min_w, min_h) = self.least();
        let least = min_w.max(min_h * ratio);
        let most = room.min(room_h * ratio);
        (least.min(most), most)
    }
}

impl Frame {
    pub fn full(area: Area) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            w: area.w,
            h: area.h,
        }
    }

    /// The crop `rect` (fractions of the source) in px.
    pub fn of(rect: Rect, area: Area) -> Self {
        Self {
            x: rect.x() * area.w,
            y: rect.y() * area.h,
            w: rect.w() * area.w,
            h: rect.h() * area.h,
        }
    }

    /// The crop as fractions of the source, at whole pixels; the whole frame is `Rect::FULL`.
    /// Always a valid crop: inside 0..1 even after rounding of the division.
    pub fn rect(&self, area: Area) -> Rect {
        let (x, w) = fractions(self.x, self.w, area.w);
        let (y, h) = fractions(self.y, self.h, area.h);
        Rect::new(x, y, w, h).unwrap_or(Rect::FULL)
    }

    /// Width / height.
    pub fn ratio(&self) -> f64 {
        self.w / self.h.max(f64::MIN_POSITIVE)
    }

    /// The frame dragged by `handle` by `by` px of the source from `self`, the frame at the
    /// press. `ratio` (w / h) keeps the proportion: a corner holds the opposite corner, an edge
    /// the opposite edge and the middle of the frame across.
    pub fn dragged(&self, handle: Handle, by: (f64, f64), area: Area, ratio: Option<f64>) -> Self {
        if handle == Handle::Move {
            return Self {
                x: (self.x + by.0).clamp(0.0, area.w - self.w),
                y: (self.y + by.1).clamp(0.0, area.h - self.h),
                ..*self
            };
        }
        match ratio {
            Some(ratio) => self.kept(handle, by, area, ratio),
            None => self.free(handle, by, area),
        }
    }

    fn free(&self, handle: Handle, by: (f64, f64), area: Area) -> Self {
        let (sx, sy) = handle.signs();
        let (min_w, min_h) = area.least();
        let (x, w) = side(self.x, self.w, sx, by.0, area.w, min_w);
        let (y, h) = side(self.y, self.h, sy, by.1, area.h, min_h);
        Self { x, y, w, h }
    }

    fn kept(&self, handle: Handle, by: (f64, f64), area: Area, ratio: f64) -> Self {
        let (sx, sy) = handle.signs();
        let (right, bottom) = (self.x + self.w, self.y + self.h);
        let (cx, cy) = (self.x + self.w / 2.0, self.y + self.h / 2.0);
        // Room from the held edge (or around the middle) to the edges of the source.
        let room = |sign: i8, start: f64, end: f64, middle: f64, side: f64| match sign {
            1 => side - start,
            -1 => end,
            _ => 2.0 * middle.min(side - middle),
        };
        let room_w = room(sx, self.x, right, cx, area.w);
        let room_h = room(sy, self.y, bottom, cy, area.h);
        let across = self.w + f64::from(sx) * by.0;
        let down = (self.h + f64::from(sy) * by.1) * ratio;
        // A corner follows the side the pointer changed more.
        let wanted = match (sx, sy) {
            (0, _) => down,
            (_, 0) => across,
            _ if (across - self.w).abs() >= (down - self.w).abs() => across,
            _ => down,
        };
        let (least, most) = area.widths(ratio, room_w, room_h);
        let w = wanted.clamp(least, most);
        let h = w / ratio;
        let place = |sign: i8, start: f64, end: f64, middle: f64, size: f64| match sign {
            1 => start,
            -1 => end - size,
            _ => middle - size / 2.0,
        };
        Self {
            x: place(sx, self.x, right, cx, w),
            y: place(sy, self.y, bottom, cy, h),
            w,
            h,
        }
    }

    /// The largest frame of `ratio` inside this one, around its middle (a fixed format picked).
    pub fn fitted(&self, ratio: f64, area: Area) -> Self {
        let (cx, cy) = (self.x + self.w / 2.0, self.y + self.h / 2.0);
        let (least, most) = area.widths(ratio, area.w, area.h);
        let w = self.w.min(self.h * ratio).clamp(least, most);
        let h = w / ratio;
        Self {
            x: (cx - w / 2.0).clamp(0.0, area.w - w),
            y: (cy - h / 2.0).clamp(0.0, area.h - h),
            w,
            h,
        }
    }

    /// The frame with `field` typed as `value` px. The corner stays where it can; a size grows
    /// to the right and down, and with `ratio` the other side follows.
    pub fn with(&self, field: Field, value: f64, area: Area, ratio: Option<f64>) -> Self {
        let (min_w, min_h) = area.least();
        let mut frame = *self;
        match field {
            Field::X => frame.x = value.clamp(0.0, area.w - self.w),
            Field::Y => frame.y = value.clamp(0.0, area.h - self.h),
            Field::Width => frame.w = value.clamp(min_w, area.w - self.x),
            Field::Height => frame.h = value.clamp(min_h, area.h - self.y),
        }
        let Some(ratio) = ratio else {
            return frame;
        };
        let (least, most) = area.widths(ratio, area.w - self.x, area.h - self.y);
        let w = match field {
            Field::X | Field::Y => return frame,
            Field::Width => frame.w,
            Field::Height => frame.h * ratio,
        };
        frame.w = w.clamp(least, most);
        frame.h = frame.w / ratio;
        frame
    }

    /// The value of `field`, whole px.
    pub fn value(&self, field: Field) -> f64 {
        match field {
            Field::X => self.x,
            Field::Y => self.y,
            Field::Width => self.w,
            Field::Height => self.h,
        }
        .round()
    }
}

/// The start and the length of one side dragged by `by` at the edge `sign` points to.
fn side(start: f64, len: f64, sign: i8, by: f64, total: f64, least: f64) -> (f64, f64) {
    let end = start + len;
    match sign {
        1 => (start, (end + by).clamp(start + least, total) - start),
        -1 => {
            let moved = (start + by).clamp(0.0, end - least);
            (moved, end - moved)
        }
        _ => (start, len),
    }
}

/// Start and length of a side at whole pixels as fractions of `total`; the length is at least
/// `CROP_MIN` (a least side of a source whose 5 % is not whole px rounds up to it) and their sum
/// never passes 1 (`crop_problem` would refuse either).
fn fractions(start: f64, len: f64, total: f64) -> (f64, f64) {
    let first = start.round().clamp(0.0, total);
    let end = (start + len).round().clamp(first, total);
    let mut size = ((end - first) / total).max(CROP_MIN);
    let mut from = first / total;
    if from + size > 1.0 {
        if 1.0 - from >= CROP_MIN {
            size = 1.0 - from;
        } else {
            (from, size) = (1.0 - CROP_MIN, CROP_MIN);
        }
    }
    if from + size > 1.0 {
        from = (from - f64::EPSILON).max(0.0);
    }
    (from, size)
}

#[cfg(test)]
#[path = "geom_tests.rs"]
mod tests;
