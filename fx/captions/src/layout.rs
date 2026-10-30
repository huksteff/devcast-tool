//! Captions broken into lines and pages once for the output frame (docs/porting/captions.md
//! §6.1–§6.3, decision 016): widths come from the fonts the frame is drawn with, lines are the
//! v1 dynamic programme, and a cue turns its own pages (decision 000: a page never mixes cues).

use std::ops::Range as Span;

use dc_core::time::Source;
use dc_core::{PixelSize, Range, SourceTime};
use dc_render::{Font, REFERENCE, RenderError, TextMeasure};

use crate::model::{CaptionsState, Cue, Word};
use crate::style::CaptionsSettings;

/// v1 sizes in font sizes: padding, line height.
pub(crate) const PAD: [f64; 2] = [1.1, 0.78];
pub(crate) const LINE_HEIGHT: f64 = 1.32;
/// Line breaking of v1: a line filled less than this is penalised, the last one is cheap.
const FULL: f64 = 0.72;
const UNDERFULL: f64 = 26000.0;
const LAST: f64 = 0.18;

/// One page of a cue: its lines and when it shows, source time of the cue's recording.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Page {
    pub lines: Span<usize>,
    pub start: SourceTime,
    pub end: SourceTime,
}

/// A cue laid out: its words, their lines (word index ranges) and pages.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Laid {
    pub media: String,
    pub range: Range<Source>,
    pub words: Vec<Word>,
    pub lines: Vec<Span<usize>>,
    /// Width of each line, px of the frame.
    pub widths: Vec<f64>,
    pub pages: Vec<Page>,
}

/// The captions of a project ready for every frame.
#[derive(Debug)]
pub struct Prepared {
    pub(crate) settings: CaptionsSettings,
    /// In the order of the state: by recording, then start.
    pub(crate) cues: Vec<Laid>,
    pub(crate) font: Font,
}

impl Prepared {
    /// Lays out every cue of `state` for a `canvas`-sized frame with the fonts of `measure`.
    pub fn new(
        state: CaptionsState,
        canvas: PixelSize,
        measure: &mut TextMeasure,
    ) -> Result<Self, RenderError> {
        let settings = state.settings;
        let short = f64::from(canvas.width.min(canvas.height));
        let size = settings.size * short / REFERENCE;
        let font = Font {
            family: settings.font.clone(),
            size: size as f32,
            bold: false,
            italic: false,
        };
        let target = f64::from(canvas.width) * settings.width / 100.0;
        let limit = (target - 2.0 * PAD[0] * size).max(4.0 * size);
        let mut cues = Vec::new();
        if settings.show {
            for cue in state.cues {
                cues.push(lay(cue, &font, limit, settings.rows, measure)?);
            }
        }
        Ok(Self {
            settings,
            cues,
            font,
        })
    }
}

fn lay(
    cue: Cue,
    font: &Font,
    limit: f64,
    rows: u32,
    measure: &mut TextMeasure,
) -> Result<Laid, RenderError> {
    let mut plain = Vec::with_capacity(cue.words.len());
    let mut spaced = Vec::with_capacity(cue.words.len());
    for word in &cue.words {
        plain.push(f64::from(measure.width(&word.text, font)?));
        spaced.push(f64::from(measure.width(&format!(" {}", word.text), font)?));
    }
    let lines = break_lines(&plain, &spaced, limit);
    let widths = lines
        .iter()
        .map(|l| plain[l.start] + spaced[l.start + 1..l.end].iter().sum::<f64>())
        .collect();
    let pages = pages(&cue, &lines, rows);
    Ok(Laid {
        media: cue.media,
        range: cue.range,
        words: cue.words,
        lines,
        widths,
        pages,
    })
}

/// v1 `captionLayout.ts:332-411`: the cheapest split into lines of at most `limit` px; a word
/// wider than that is a line of its own. `plain` — word widths, `spaced` — with a space before.
pub(crate) fn break_lines(plain: &[f64], spaced: &[f64], limit: f64) -> Vec<Span<usize>> {
    let n = plain.len();
    let mut best = vec![f64::INFINITY; n + 1];
    let mut next = vec![n; n + 1];
    best[n] = 0.0;
    for s in (0..n).rev() {
        let mut width = plain[s];
        for e in s..n {
            if e > s {
                width += spaced[e];
                if width > limit {
                    break;
                }
            }
            let slack = (limit - width).max(0.0);
            let full = width / limit;
            let penalty = match e == n - 1 {
                true => slack * slack * LAST,
                false => {
                    slack * slack
                        + if full < FULL {
                            (FULL - full) * UNDERFULL
                        } else {
                            0.0
                        }
                }
            };
            if penalty + best[e + 1] < best[s] {
                best[s] = penalty + best[e + 1];
                next[s] = e + 1;
            }
        }
    }
    let mut lines = Vec::new();
    let mut s = 0;
    while s < n {
        lines.push(s..next[s]);
        s = next[s];
    }
    lines
}

/// v1 `captionLayout.ts:255-330` within one cue: lines spread evenly over `ceil(lines / rows)`
/// pages; a page shows from its first word until the next page starts, the last one until the
/// cue ends.
fn pages(cue: &Cue, lines: &[Span<usize>], rows: u32) -> Vec<Page> {
    let rows = rows.max(1) as usize;
    let total = lines.len().div_ceil(rows).max(1);
    let mut spans = Vec::with_capacity(total);
    let mut first = 0;
    for page in 0..total {
        let left = lines.len() - first;
        let size = rows.min(left.div_ceil(total - page).max(1));
        spans.push(first..first + size);
        first += size;
    }
    let starts: Vec<SourceTime> = spans
        .iter()
        .map(|s| cue.words[lines[s.start].start].range.start())
        .collect();
    spans
        .into_iter()
        .enumerate()
        .map(|(i, lines)| Page {
            lines,
            start: if i == 0 { cue.range.start() } else { starts[i] },
            end: starts.get(i + 1).copied().unwrap_or(cue.range.end()),
        })
        .collect()
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
