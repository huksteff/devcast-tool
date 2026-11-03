//! The waveform of a sound region (7.11): which peaks it draws (`Source`), which part of the
//! source each piece of the region plays (`Span`: the clip's segments of the time map, so a cut
//! joins two parts and a 4× range shows four times as much sound per point), and the columns
//! drawn at the zoom of the view. The peaks come from `dc_audio::peaks` (exact to 256 frames,
//! coarse levels for a wide view), read off the UI thread (`editor/waves.rs`).

use dc_audio::frame_at;
use dc_audio::peaks::Peaks;
use dc_core::project::Media;
use dc_core::time::Output;
use dc_core::{ClipId, OutputTime, Range, TimeDelta, TimeMap};
use eframe::egui::{Color32, Painter, Rect, Stroke};

use super::scale::TimeScale;

/// Width of a column of the wave, points; its line is a bit thinner (mockup: 2 and 1.2).
pub const COLUMN: f32 = 2.0;
const LINE: f32 = 1.2;
/// The wave fills this share of the region's height, at its bottom (mockup `.wave`: 70%).
const HEIGHT: f32 = 0.7;
/// Opacity of the wave (mockup: .85).
const OPACITY: f32 = 0.85;

/// Where the peaks of a wave come from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Sound track `name` (`mic`, `system`) of a recording.
    Track { media: Media, name: String },
    /// An extra sound file: its path in the project, `assets/audio/music.mp3`.
    File(String),
}

/// A piece of a region that plays its source evenly: `output` plays from source µs `from` (of
/// the recording, or of the file from its first sample) at `speed`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub output: Range<Output>,
    pub from: i64,
    pub speed: f64,
}

/// The sound a region plays.
#[derive(Debug, Clone, PartialEq)]
pub struct Wave {
    pub source: Source,
    /// Pieces in output order, not overlapping.
    pub spans: Vec<Span>,
    /// Volume × gain: the drawn height of a sample, 0..=10 (a louder wave is cut at the edge,
    /// as the limiter cuts the sound).
    pub level: f32,
}

/// Track `name` of `media` as clip `clip` plays it.
pub fn recording(map: &TimeMap, clip: ClipId, media: &Media, name: &str, level: f32) -> Wave {
    let spans = map
        .segments()
        .iter()
        .filter(|s| s.clip == clip)
        .filter_map(|s| {
            Some(Span {
                output: Range::new(s.output_start, s.output_end()).ok()?,
                from: s.source.start().micros(),
                speed: s.speed,
            })
        })
        .collect();
    Wave {
        source: Source::Track {
            media: media.clone(),
            name: name.to_owned(),
        },
        spans,
        level,
    }
}

/// An extra sound file (`path` in the project) at `output`, from `offset` of it at 1× (cuts do
/// not touch it).
pub fn file(path: String, output: Range<Output>, offset: TimeDelta, level: f32) -> Wave {
    Wave {
        source: Source::File(path),
        spans: vec![Span {
            output,
            from: offset.micros(),
            speed: 1.0,
        }],
        level,
    }
}

/// A column of the drawn wave: x of its middle and the source frames it shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Column {
    pub x: f32,
    pub frames: (i64, i64),
}

/// Columns `COLUMN` points wide over x `from..to`. The columns sit on a grid from output zero,
/// so scrolling does not shift what a column shows; a column belongs to the span under its
/// middle and shows its part of that span.
pub fn columns(wave: &Wave, scale: &TimeScale, from: f32, to: f32) -> Vec<Column> {
    let zero = scale.x(OutputTime::ZERO);
    let first = ((from - zero) / COLUMN).floor() as i64;
    let last = ((to - zero) / COLUMN).ceil() as i64;
    (first..last)
        .filter_map(|k| {
            let left = zero + k as f32 * COLUMN;
            let x = left + COLUMN / 2.0;
            let middle = scale.time(x);
            let span = wave
                .spans
                .iter()
                .find(|s| s.output.start() <= middle && middle < s.output.end())?;
            let frame = |x: f32| {
                let t = scale
                    .time(x)
                    .max(span.output.start())
                    .min(span.output.end());
                let played = t.since(span.output.start()).micros() as f64 * span.speed;
                frame_at(span.from + played.round() as i64)
            };
            let (start, end) = (frame(left), frame(left + COLUMN));
            Some(Column {
                x,
                frames: (start, end.max(start + 1)),
            })
        })
        .collect()
}

/// `wave` from `peaks` at the bottom of the region `body`, in `color`; only what the painter
/// shows is worked out.
pub fn paint(
    painter: &Painter,
    body: Rect,
    wave: &Wave,
    peaks: &Peaks,
    scale: &TimeScale,
    color: Color32,
) {
    let area = body.shrink(1.0);
    let area = area.with_min_y(area.bottom() - area.height() * HEIGHT);
    let shown = area.intersect(painter.clip_rect());
    if !shown.is_positive() {
        return;
    }
    let (middle, half) = (area.center().y, area.height() / 2.0);
    let stroke = Stroke::new(LINE, color.gamma_multiply(OPACITY));
    for column in columns(wave, scale, shown.left(), shown.right()) {
        let Some(peak) = peaks
            .range(column.frames.0, column.frames.1, 1)
            .first()
            .copied()
        else {
            continue;
        };
        let high = (peak.max * wave.level).clamp(-1.0, 1.0);
        let low = (peak.min * wave.level).clamp(-1.0, 1.0);
        // Silence is still a short tick (mockup), so a quiet part reads as sound that is there.
        let top = (middle - high * half).min(middle - 0.5);
        let bottom = (middle - low * half).max(middle + 0.5);
        painter.vline(column.x, top..=bottom, stroke);
    }
}
