//! The camera as the simulation sees it: fractions of the frame, moved by the spring.

use dc_core::Point;

use crate::spring::{Axis, Spring};

/// Closer than this to the frame at rest (fractions of the frame, scale) with no zoom pulling,
/// the camera stops: 0.2 px of a 1080p frame.
const REST: f64 = 1e-4;
/// Slower than this (the same units per second) the camera may stop.
const REST_SPEED: f64 = 1e-3;

/// The camera in fractions of the frame: a point `p` (0..1) lands at `p·scale + (x, y)`. Also
/// its speed, the same units per second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Shot {
    pub scale: f64,
    pub x: f64,
    pub y: f64,
}

impl Shot {
    pub(crate) const REST: Self = Self {
        scale: 1.0,
        x: 0.0,
        y: 0.0,
    };

    /// No motion: the speed of a camera at rest.
    pub(crate) const STILL: Self = Self {
        scale: 0.0,
        x: 0.0,
        y: 0.0,
    };

    /// The camera `strength` (0..1) of the way into a zoom by `scale` at `focus` (reachable,
    /// `frame::reachable`): the focus moves from where it is towards the centre of the frame
    /// (§4.5).
    pub(crate) fn toward(scale: f64, focus: Point, strength: f64) -> Self {
        Self {
            scale: 1.0 + (scale - 1.0) * strength,
            x: (0.5 - focus.x * scale) * strength,
            y: (0.5 - focus.y * scale) * strength,
        }
    }

    /// Moved and scaled so the canvas still covers the frame (a spring may overshoot it).
    pub(crate) fn covering(self) -> Self {
        let scale = self.scale.max(1.0);
        Self {
            scale,
            x: self.x.clamp(1.0 - scale, 0.0),
            y: self.y.clamp(1.0 - scale, 0.0),
        }
    }

    pub(crate) fn lerp(self, to: Self, k: f64) -> Self {
        Self {
            scale: self.scale + (to.scale - self.scale) * k,
            x: self.x + (to.x - self.x) * k,
            y: self.y + (to.y - self.y) * k,
        }
    }

    /// One spring step of every axis of this camera, moving at `speed`, towards `aim`; the new
    /// camera and its speed.
    pub(crate) fn chase(self, speed: Self, aim: Self, spring: &Spring) -> (Self, Self) {
        let axis = |value: f64, speed: f64, aim: f64| {
            let next = spring.step(Axis {
                offset: value - aim,
                speed,
            });
            (aim + next.offset, next.speed)
        };
        let (scale, scale_speed) = axis(self.scale, speed.scale, aim.scale);
        let (x, x_speed) = axis(self.x, speed.x, aim.x);
        let (y, y_speed) = axis(self.y, speed.y, aim.y);
        (
            Self { scale, x, y },
            Self {
                scale: scale_speed,
                x: x_speed,
                y: y_speed,
            },
        )
    }

    /// Whether this camera, moving at `speed`, has come to rest.
    pub(crate) fn resting(self, speed: Self) -> bool {
        let near = (self.scale - 1.0).abs().max(self.x.abs()).max(self.y.abs());
        let slow = speed.scale.abs().max(speed.x.abs()).max(speed.y.abs());
        near < REST && slow < REST_SPEED
    }
}
