//! Time on the source and output timelines, in whole microseconds.
//!
//! Effects are anchored to [`SourceTime`] (time inside a recording or imported video); the edited
//! video plays in [`OutputTime`]. The two are different types so they cannot be mixed by accident;
//! only the time map converts between them.

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Add, Sub};
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const MICROS_PER_SECOND: i64 = 1_000_000;

/// A timeline marker: implemented only by [`Source`] and [`Output`].
pub trait Timeline:
    Copy
    + fmt::Debug
    + Eq
    + Ord
    + std::hash::Hash
    + Default
    + Serialize
    + for<'de> Deserialize<'de>
    + JsonSchema
    + Send
    + Sync
    + 'static
{
}

/// Marker for time inside a source clip.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
)]
pub struct Source;
impl Timeline for Source {}

/// Marker for time on the edited output.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
)]
pub struct Output;
impl Timeline for Output {}

/// A point on timeline `K`, in microseconds.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(transparent, bound = "")]
#[schemars(transparent)]
pub struct Time<K: Timeline> {
    micros: i64,
    #[serde(skip)]
    #[schemars(skip)]
    kind: PhantomData<K>,
}

/// Time inside a source clip (recording or imported video).
pub type SourceTime = Time<Source>;
/// Time on the edited output video.
pub type OutputTime = Time<Output>;

/// A length of time in microseconds; the same unit on both timelines.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(transparent, bound = "")]
pub struct TimeDelta(i64);

impl TimeDelta {
    pub const ZERO: Self = Self(0);

    pub const fn from_micros(micros: i64) -> Self {
        Self(micros)
    }

    pub const fn from_millis(millis: i64) -> Self {
        Self(millis.saturating_mul(1000))
    }

    /// Rounds to the nearest microsecond; non-finite input gives zero.
    pub fn from_secs_f64(secs: f64) -> Self {
        Self(secs_to_micros(secs))
    }

    pub const fn micros(self) -> i64 {
        self.0
    }

    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / MICROS_PER_SECOND as f64
    }
}

fn secs_to_micros(secs: f64) -> i64 {
    if secs.is_finite() {
        // `as` saturates on overflow, which is the documented behaviour for huge values.
        (secs * MICROS_PER_SECOND as f64).round() as i64
    } else {
        0
    }
}

impl<K: Timeline> Time<K> {
    pub const ZERO: Self = Self::from_micros(0);

    pub const fn from_micros(micros: i64) -> Self {
        Self {
            micros,
            kind: PhantomData,
        }
    }

    /// Rounds to the nearest microsecond; non-finite input gives zero.
    pub fn from_secs_f64(secs: f64) -> Self {
        Self::from_micros(secs_to_micros(secs))
    }

    pub const fn micros(self) -> i64 {
        self.micros
    }

    pub fn as_secs_f64(self) -> f64 {
        self.micros as f64 / MICROS_PER_SECOND as f64
    }

    pub fn checked_add(self, delta: TimeDelta) -> Option<Self> {
        self.micros.checked_add(delta.0).map(Self::from_micros)
    }

    pub fn checked_sub(self, delta: TimeDelta) -> Option<Self> {
        self.micros.checked_sub(delta.0).map(Self::from_micros)
    }

    /// Distance from `earlier` to `self` (negative if `earlier` is later).
    pub fn since(self, earlier: Self) -> TimeDelta {
        TimeDelta(self.micros.saturating_sub(earlier.micros))
    }
}

/// Saturating: overflow clamps to the representable range (~292 000 years), never panics.
impl<K: Timeline> Add<TimeDelta> for Time<K> {
    type Output = Self;
    fn add(self, delta: TimeDelta) -> Self {
        Self::from_micros(self.micros.saturating_add(delta.0))
    }
}

/// Saturating, see [`Add`].
impl<K: Timeline> Sub<TimeDelta> for Time<K> {
    type Output = Self;
    fn sub(self, delta: TimeDelta) -> Self {
        Self::from_micros(self.micros.saturating_sub(delta.0))
    }
}

impl<K: Timeline> fmt::Display for Time<K> {
    /// `m:ss.mmm`, or `h:mm:ss.mmm` from one hour; milliseconds rounded down.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.micros < 0 { "-" } else { "" };
        let total_ms = self.micros.unsigned_abs() / 1000;
        let (ms, secs) = (total_ms % 1000, total_ms / 1000);
        let (s, mins) = (secs % 60, secs / 60);
        let (m, h) = (mins % 60, mins / 60);
        if h > 0 {
            write!(f, "{sign}{h}:{m:02}:{s:02}.{ms:03}")
        } else {
            write!(f, "{sign}{m}:{s:02}.{ms:03}")
        }
    }
}

/// Error for a time string that is not `S`, `M:SS` or `H:MM:SS` (fractions allowed on seconds).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("не понимаю время «{0}»: пиши секундами (12.5), M:SS (1:02.5) или H:MM:SS (1:02:03)")]
pub struct ParseTimeError(String);

impl<K: Timeline> FromStr for Time<K> {
    type Err = ParseTimeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = || ParseTimeError(text.to_owned());
        let trimmed = text.trim();
        let (negative, body) = match trimmed.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, trimmed),
        };
        let parts: Vec<&str> = body.split(':').collect();
        if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
            return Err(error());
        }
        let (whole, last) = parts.split_at(parts.len() - 1);
        let seconds: f64 = last[0].parse().map_err(|_| error())?;
        if !seconds.is_finite() || seconds < 0.0 || (!whole.is_empty() && seconds >= 60.0) {
            return Err(error());
        }
        let mut total = seconds;
        for (i, part) in whole.iter().rev().enumerate() {
            let unit: u32 = part.parse().map_err(|_| error())?;
            if i == 0 && whole.len() == 2 && unit >= 60 {
                return Err(error());
            }
            total += f64::from(unit) * 60f64.powi(i as i32 + 1);
        }
        let micros = secs_to_micros(total);
        Ok(Self::from_micros(if negative { -micros } else { micros }))
    }
}

/// Half-open interval `[start, end)` on timeline `K`; always `start <= end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "RawRange<K>", into = "RawRange<K>", bound = "")]
pub struct Range<K: Timeline> {
    start: Time<K>,
    end: Time<K>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(bound = "")]
#[schemars(rename = "Range")]
struct RawRange<K: Timeline> {
    /// Start, microseconds (inclusive).
    start: Time<K>,
    /// End, microseconds (exclusive); not before `start`.
    end: Time<K>,
}

/// A range whose end is before its start.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("конец диапазона ({end}) раньше начала ({start})")]
pub struct InvertedRange {
    start: String,
    end: String,
}

impl<K: Timeline> Range<K> {
    pub fn new(start: Time<K>, end: Time<K>) -> Result<Self, InvertedRange> {
        if end < start {
            return Err(InvertedRange {
                start: start.to_string(),
                end: end.to_string(),
            });
        }
        Ok(Self { start, end })
    }

    pub const fn start(&self) -> Time<K> {
        self.start
    }

    pub const fn end(&self) -> Time<K> {
        self.end
    }

    pub fn duration(&self) -> TimeDelta {
        self.end.since(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// `start <= t < end`.
    pub fn contains(&self, t: Time<K>) -> bool {
        self.start <= t && t < self.end
    }

    /// Overlapping part, if the ranges overlap by more than a point.
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        (start < end).then_some(Self { start, end })
    }
}

impl<K: Timeline> TryFrom<RawRange<K>> for Range<K> {
    type Error = InvertedRange;
    fn try_from(raw: RawRange<K>) -> Result<Self, Self::Error> {
        Self::new(raw.start, raw.end)
    }
}

impl<K: Timeline> From<Range<K>> for RawRange<K> {
    fn from(range: Range<K>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

#[cfg(test)]
#[path = "time_tests.rs"]
mod tests;
