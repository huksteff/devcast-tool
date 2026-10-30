//! Keyframes (docs/porting/annotations.md §4): an annotation's box, rotation and kind fields at
//! moments of its recording, blended with smoothstep in between.
//!
//! A key's time is source time of the recording since the annotation start: keys move with the
//! annotation (`annot move`), stay on the same frames of the recording when a cut or a speed-up
//! changes the output (a keyed motion follows the content, like the annotation's own start and
//! end), and stay where they are in the recording when `annot set --from` trims the head. A key
//! left outside the shown range after a trim is kept, as DaVinci keeps keyframes past a trimmed
//! edge, and still steers the values near that edge.

use dc_core::{Rect, SourceTime, TimeDelta};
use dc_render::Rgba;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

use crate::model::Annotation;

/// Two keys closer than this are one moment (v1).
pub const KEY_TOLERANCE: TimeDelta = TimeDelta::from_millis(1);

/// Everything an annotation can animate, at one moment. Time, track and layer order are not
/// keyed; neither are the entrance and exit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Key {
    /// Source time of the recording since the annotation start, µs; keys are sorted by it, no
    /// two within `KEY_TOLERANCE`. May lie outside the annotation after its head or tail was
    /// trimmed.
    pub at: TimeDelta,
    /// Box at this moment, as `Annotation::rect`.
    pub rect: Rect,
    /// Degrees clockwise around the box centre at this moment, as `Annotation::rotation`.
    pub rotation: f64,
    /// The kind's fields at this moment, as `Annotation::body`.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub body: Map<String, Value>,
}

/// A value that can sit between two keys: `self` at `e = 0`, `to` at `e >= 1`. Numbers and
/// colours blend; anything else (text, switches, choices) steps to `to` only at the key.
pub trait Blend: Sized {
    fn blend(&self, to: &Self, e: f64) -> Self;
}

/// The value of a field that does not blend: `from` until the right key.
pub fn step<T: Clone>(from: &T, to: &T, e: f64) -> T {
    if e >= 1.0 { to.clone() } else { from.clone() }
}

fn lerp(a: f64, b: f64, e: f64) -> f64 {
    if e >= 1.0 { b } else { a + (b - a) * e }
}

impl Blend for f64 {
    fn blend(&self, to: &Self, e: f64) -> Self {
        lerp(*self, *to, e)
    }
}

impl Blend for bool {
    fn blend(&self, to: &Self, e: f64) -> Self {
        step(self, to, e)
    }
}

impl Blend for String {
    fn blend(&self, to: &Self, e: f64) -> Self {
        step(self, to, e)
    }
}

/// Each sRGB channel and the alpha, rounded (v1).
impl Blend for Rgba {
    fn blend(&self, to: &Self, e: f64) -> Self {
        let mut out = self.0;
        for (channel, (a, b)) in out.iter_mut().zip(self.0.iter().zip(to.0)) {
            *channel = lerp(f64::from(*a), f64::from(b), e)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        Self(out)
    }
}

impl Blend for Rect {
    fn blend(&self, to: &Self, e: f64) -> Self {
        if e >= 1.0 { *to } else { self.lerp(to, e) }
    }
}

impl<A: Blend, B: Blend> Blend for (A, B) {
    fn blend(&self, to: &Self, e: f64) -> Self {
        (self.0.blend(&to.0, e), self.1.blend(&to.1, e))
    }
}

/// Kind fields as stored: numbers blend (whole numbers stay whole), `#rrggbbaa` colours blend by
/// channel, objects field by field (a field the left side lacks appears at the right key);
/// text, switches, choices and lists step.
impl Blend for Value {
    fn blend(&self, to: &Self, e: f64) -> Self {
        if e >= 1.0 {
            return to.clone();
        }
        match (self, to) {
            (Self::Number(a), Self::Number(b)) => blend_numbers(a, b, e),
            (Self::String(a), Self::String(b)) => match (a.parse::<Rgba>(), b.parse::<Rgba>()) {
                (Ok(a), Ok(b)) => Self::String(a.blend(&b, e).to_string()),
                _ => self.clone(),
            },
            (Self::Object(a), Self::Object(b)) => Self::Object(a.blend(b, e)),
            _ => self.clone(),
        }
    }
}

fn blend_numbers(a: &Number, b: &Number, e: f64) -> Value {
    let whole = |n: &Number| n.as_i64().map(|v| v as f64);
    if let (Some(x), Some(y)) = (whole(a), whole(b)) {
        return Value::from(lerp(x, y, e).round() as i64);
    }
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => {
            Number::from_f64(lerp(x, y, e)).map_or(Value::Number(a.clone()), Value::Number)
        }
        _ => Value::Number(a.clone()),
    }
}

impl Blend for Map<String, Value> {
    fn blend(&self, to: &Self, e: f64) -> Self {
        if e >= 1.0 {
            return to.clone();
        }
        self.iter()
            .map(|(name, value)| {
                let mixed = to.get(name).map_or(value.clone(), |r| value.blend(r, e));
                (name.clone(), mixed)
            })
            .collect()
    }
}

/// The value at `offset` among `keys` sorted by time: `base` without keys, the first key before
/// it, the last after it (one rule for every parameter, unlike v1), smoothstep in between: every
/// key is a standstill (v1).
pub fn keyed<T: Blend + Clone>(base: &T, keys: &[(TimeDelta, T)], offset: TimeDelta) -> T {
    let right = keys.partition_point(|(at, _)| *at < offset);
    let (Some((from_at, from)), Some((to_at, to))) = (
        right.checked_sub(1).and_then(|i| keys.get(i)),
        keys.get(right),
    ) else {
        return keys
            .get(right)
            .or(keys.last())
            .map_or_else(|| base.clone(), |(_, value)| value.clone());
    };
    let span = (to_at.micros() - from_at.micros()).max(1) as f64;
    let p = ((offset.micros() - from_at.micros()) as f64 / span).clamp(0.0, 1.0);
    from.blend(to, p * p * (3.0 - 2.0 * p))
}

impl Annotation {
    /// Key time of source moment `now` of the recording.
    pub fn offset(&self, now: SourceTime) -> TimeDelta {
        now.since(self.range.start())
    }

    /// Box and rotation at key time `offset`.
    pub fn frame_at(&self, offset: TimeDelta) -> (Rect, f64) {
        let keys: Vec<_> = self
            .keys
            .iter()
            .map(|k| (k.at, (k.rect, k.rotation)))
            .collect();
        keyed(&(self.rect, self.rotation), &keys, offset)
    }

    /// The kind's fields at key time `offset`, as stored.
    pub fn body_at(&self, offset: TimeDelta) -> Map<String, Value> {
        let keys: Vec<_> = self.keys.iter().map(|k| (k.at, k.body.clone())).collect();
        keyed(&self.body, &keys, offset)
    }

    /// The key at `offset`, give or take `KEY_TOLERANCE`.
    pub fn key_index(&self, offset: TimeDelta) -> Option<usize> {
        let near = |k: &Key| (k.at.micros() - offset.micros()).abs() < KEY_TOLERANCE.micros();
        self.keys.iter().position(near)
    }

    /// What is shown at `offset`, as a key there.
    pub(crate) fn snapshot(&self, offset: TimeDelta) -> Key {
        let (rect, rotation) = self.frame_at(offset);
        Key {
            at: offset,
            rect,
            rotation,
            body: self.body_at(offset),
        }
    }

    /// Puts `key` in time order, replacing a key at the same moment.
    pub(crate) fn put_key(&mut self, key: Key) -> usize {
        if let Some(index) = self.key_index(key.at) {
            self.keys.remove(index);
        }
        let index = self.keys.partition_point(|k| k.at < key.at);
        self.keys.insert(index, key);
        index
    }

    /// The key at `offset` to write a change into: created with what is shown there. The first
    /// key past the start brings one at the start with the annotation as it stands, so the
    /// change becomes a motion from there (v1).
    pub fn edit_key(&mut self, offset: TimeDelta) -> &mut Key {
        if self.keys.is_empty() && offset.micros() >= KEY_TOLERANCE.micros() {
            self.put_key(self.snapshot(TimeDelta::ZERO));
        }
        let index = match self.key_index(offset) {
            Some(index) => index,
            None => self.put_key(self.snapshot(offset)),
        };
        &mut self.keys[index]
    }
}
