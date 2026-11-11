//! Click effects in the scene (docs/porting/clicks-callout.md §1–§7): every click whose effect is
//! still running, at the recorded point of the press, sized by the cursor arrow. Minimums of v1
//! in absolute px are ref-px here (× clip width / 1920), so 4K looks like 1080p.

use dc_core::{OutputTime, TimeDelta, TimeMap};
use dc_render::{Geometry, Glow, Paint, PxRect, Shape, Stroke};

use crate::click_state::{ClickKind, ClickStyle};
use crate::clicks;
use crate::prepared::{Place, Prepared, REFERENCE_WIDTH};
/// Particles of the flash and the golden angle between them (Vogel's spiral, v1).
const PARTICLES: usize = 18;
const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;
/// Target radius of the flash at scale 1, ref-px (v1 «Подсветка клика»).
const FLASH_RADIUS: f64 = 90.0;

/// The figures of every running click effect where `place` shows its recording.
pub(crate) fn shapes(
    prepared: &Prepared,
    place: &Place<'_>,
    map: &TimeMap,
    t: OutputTime,
) -> Vec<Shape> {
    let settings = &prepared.settings;
    let delay = TimeDelta::from_micros(settings.bounce_time.micros() / 2);
    let window = TimeDelta::from_micros(delay.micros() + prepared.longest_click.micros());
    let mut out = Vec::new();
    let recent: Vec<_> = clicks::recent(place, map, t, window).collect();
    for (click, age) in recent.into_iter().rev() {
        let style = prepared.click_style(place.media, click.at);
        let since = TimeDelta::from_micros(age.micros() - delay.micros());
        if style.kind == ClickKind::Off || since < TimeDelta::ZERO || since > style.duration {
            continue;
        }
        let Some(centre) = place.on_frame(click.pos) else {
            continue;
        };
        let p = 1.0 - since.as_secs_f64() / style.duration.as_secs_f64();
        let look = Look {
            centre,
            p,
            arrow: place.arrow,
            k: f64::from(place.content.w) / REFERENCE_WIDTH,
            style: &style,
        };
        out.extend(look.shapes());
    }
    out
}

/// One click at progress `p` (1 at the start, 0 at the end).
struct Look<'a> {
    centre: [f64; 2],
    p: f64,
    /// Cursor arrow height, px of the frame.
    arrow: f64,
    /// px of the frame per ref-px.
    k: f64,
    style: &'a ClickStyle,
}

impl Look<'_> {
    fn shapes(&self) -> Vec<Shape> {
        match self.style.kind {
            ClickKind::Off => Vec::new(),
            ClickKind::Ripple => self.ripple(),
            ClickKind::Flash => self.flash(),
        }
    }

    fn ripple(&self) -> Vec<Shape> {
        let (p, s) = (self.p, self.style);
        let fade = p.powi(3);
        let r = ((1.0 - fade) * self.arrow * 1.95 * s.scale).max(0.5 * self.k);
        let width = (2.0 * fade).max(1.0) * self.k;
        vec![self.ring(r, width, fade * 0.6 * s.opacity)]
    }

    /// v1 preview look: a soft disk growing from nothing and 18 particles filling it. The radial
    /// gradient of v1 (62 % → 20 % at 0.42 r → 0) is a half-radius disk with a glow.
    fn flash(&self) -> Vec<Shape> {
        let u = 1.0 - self.p;
        let fade = self.p.powf(1.35) * self.style.opacity;
        let expand = 1.0 - self.p.powi(3);
        let target = FLASH_RADIUS * self.k * self.style.scale;
        let r = target * expand;
        if r < 0.5 * self.k || u <= 0.0 {
            return Vec::new();
        }
        let glow = Glow {
            color: self.style.color,
            sigma: (0.26 * r).max(2.0 * self.k) as f32,
        };
        let mut out = vec![self.disk(0.5 * r, 0.45 * fade, Some(glow))];
        for i in 0..PARTICLES {
            let angle = i as f64 * GOLDEN_ANGLE;
            let spread = ((i as f64 + 0.5) / PARTICLES as f64).sqrt() * (0.82 + 0.18 * expand);
            let size = (target * (0.018 + 0.005 * (i % 4) as f64)).max(self.k) * expand;
            let at = [
                self.centre[0] + angle.cos() * target * spread,
                self.centre[1] + angle.sin() * target * spread,
            ];
            let alpha = fade * (0.5 + 0.1 * (i % 4) as f64);
            out.push(circle(
                at,
                size,
                Some(Paint::Solid(self.style.color)),
                None,
                alpha,
            ));
        }
        out
    }

    fn ring(&self, r: f64, width: f64, alpha: f64) -> Shape {
        let stroke = Stroke {
            width: width as f32,
            color: self.style.color,
        };
        circle(self.centre, r, None, Some(stroke), alpha)
    }

    fn disk(&self, r: f64, alpha: f64, glow: Option<Glow>) -> Shape {
        let mut shape = circle(
            self.centre,
            r,
            Some(Paint::Solid(self.style.color)),
            None,
            alpha,
        );
        shape.glow = glow;
        shape
    }
}

fn circle(c: [f64; 2], r: f64, fill: Option<Paint>, stroke: Option<Stroke>, alpha: f64) -> Shape {
    Shape {
        geometry: Geometry::Ellipse {
            rect: PxRect {
                x: (c[0] - r) as f32,
                y: (c[1] - r) as f32,
                w: (2.0 * r) as f32,
                h: (2.0 * r) as f32,
            },
        },
        fill,
        stroke,
        opacity: alpha.clamp(0.0, 1.0) as f32,
        rotation: 0.0,
        glow: None,
        drawn: 1.0,
    }
}

#[cfg(test)]
#[path = "effects_tests.rs"]
mod tests;
