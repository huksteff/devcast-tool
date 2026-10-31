//! Drawing of the timeline: ruler, track headers, regions, speed ranges and the playhead, by the
//! mockup's sizes and track colours.

use dc_core::time::Output;
use dc_core::{OutputTime, Range};
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{
    self, ACCENT_HI, BG, BORDER, FS_S, Hue, REC, REC_STRONG, ROW_LINE, S2, SURFACE, TEXT_2, TEXT_3,
    WHITE,
};
use eframe::egui::{Align2, Color32, Painter, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

use super::scale::{TimeScale, tick_label, timecode};
use super::tracks::{Region, Row, Track, Tracks};
use crate::editor::waves::Waves;

/// Side of a header button, points.
pub const HEAD_BUTTON: f32 = 22.0;
/// Gap between a region and the top and bottom of its row; corner radius of a region.
const INSET: f32 = 3.0;
const RADIUS: u8 = 4;
/// Height of the speed band at the bottom of a clip.
const SPEED: f32 = 14.0;
/// Ruler labels, the playhead time and the clip facts, points (mockup).
const SMALL: f32 = 10.0;
/// Top of the second line of a clip (its facts) in the region.
const DETAIL_TOP: f32 = 22.0;
/// Faded regions keep this share of their colour.
const FADED: f32 = 0.4;
/// Opacity of the fill of the picked range (mockup: .18).
const RANGE_FILL: f32 = 0.18;

/// Row heights (mockup: clips 50, sound 36, the rest 28).
pub fn height(row: Row) -> f32 {
    match row {
        Row::Clips => 50.0,
        Row::Mic | Row::System | Row::Audio(_) => 36.0,
        _ => 28.0,
    }
}

pub fn hue(row: Row) -> Hue {
    match row {
        Row::Captions => tokens::HUE_CAPTIONS,
        Row::Clips => tokens::HUE_CLIP,
        Row::Zooms => tokens::HUE_ZOOM,
        Row::Cursor => tokens::HUE_CURSOR,
        Row::Keys => tokens::HUE_KEYS,
        Row::Annotations(_) => tokens::HUE_ANNOTATIONS,
        Row::Mic | Row::System | Row::Audio(_) => tokens::HUE_AUDIO,
    }
}

pub fn name(row: Row) -> String {
    match row {
        Row::Captions => text::CAPTIONS.to_owned(),
        Row::Clips => text::CLIPS.to_owned(),
        Row::Zooms => text::ZOOMS.to_owned(),
        Row::Cursor => text::CURSOR.to_owned(),
        Row::Keys => text::KEYS.to_owned(),
        Row::Annotations(track) => text::annotations(track + 1),
        Row::Mic => text::MIC.to_owned(),
        Row::System => text::SYSTEM.to_owned(),
        Row::Audio(track) => text::audio(track + 1),
    }
}

/// Ticks with labels every `tick_step`, a minor tick halfway, over the visible time.
pub fn ruler(painter: &Painter, rect: Rect, scale: &TimeScale) {
    painter.rect_filled(rect, 0, SURFACE);
    let head = rect.with_max_x(rect.left() + super::HEAD);
    painter.vline(head.right(), head.y_range(), Stroke::new(1.0, BORDER));
    painter.hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, ROW_LINE));
    let step = scale.tick_step();
    let half = step.micros() / 2;
    let lanes = painter.with_clip_rect(rect.with_min_x(head.right()));
    let font = fonts::mono(SMALL);
    let half = half.max(1);
    let end = scale.time(rect.right()).micros();
    let mut t = scale.time(head.right()).micros() / half * half;
    while t <= end {
        let x = scale.x(OutputTime::from_micros(t));
        let big = t % step.micros() == 0;
        let tall = if big { 10.0 } else { 6.0 };
        lanes.vline(
            x,
            (rect.bottom() - tall)..=rect.bottom(),
            Stroke::new(1.0, TEXT_3),
        );
        if big {
            let label = tick_label(OutputTime::from_micros(t), step);
            lanes.text(
                pos2(x + 3.0, rect.top() + 3.0),
                Align2::LEFT_TOP,
                label,
                font.clone(),
                TEXT_3,
            );
        }
        t += half;
    }
}

/// Swatch and name of the track on the header background.
pub fn header(painter: &Painter, rect: Rect, track: &Track) {
    painter.rect_filled(rect, 0, SURFACE);
    painter.vline(rect.right(), rect.y_range(), Stroke::new(1.0, BORDER));
    painter.hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, ROW_LINE));
    let swatch = Rect::from_center_size(
        pos2(rect.left() + S2 + 1.5, rect.center().y),
        vec2(3.0, 14.0),
    );
    painter.rect_filled(swatch, 2, hue(track.row).line);
    let color = if track.off { TEXT_3 } else { TEXT_2 };
    let room = rect.with_max_x(rect.right() - HEAD_BUTTON - 8.0);
    painter.with_clip_rect(room).text(
        pos2(swatch.right() + 6.0, rect.center().y),
        Align2::LEFT_CENTER,
        name(track.row),
        fonts::text(FS_S, Weight::Regular),
        color,
    );
}

/// The lane background and the regions of `track`, with the waves of `waves` that came.
pub fn lane(painter: &Painter, rect: Rect, track: &Track, scale: &TimeScale, waves: &Waves) {
    painter.rect_filled(rect, 0, BG);
    painter.hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, ROW_LINE));
    let hue = hue(track.row);
    for region in &track.regions {
        let body = body(rect, scale, region.range);
        let ink = region_fill(painter, body, region, hue);
        let wave = region.wave.as_ref();
        if let Some((wave, peaks)) = wave.and_then(|w| Some((w, waves.peaks(&w.source)?))) {
            let color = if region.faded {
                hue.line.gamma_multiply(FADED)
            } else {
                hue.line
            };
            super::wave::paint(painter, body, wave, peaks, scale, color);
        }
        region_label(painter, body, region, ink);
    }
}

/// Where a region of `range` lies on the lane `rect`.
pub fn body(rect: Rect, scale: &TimeScale, range: Range<Output>) -> Rect {
    let (left, right) = scale.span(range);
    Rect::from_x_y_ranges(
        left..=right.max(left + 2.0),
        (rect.top() + INSET)..=(rect.bottom() - INSET),
    )
}

/// Where the band of a speed `range` lies at the bottom of the clips lane `rect`.
pub fn speed_band(rect: Rect, scale: &TimeScale, range: Range<Output>) -> Rect {
    let bottom = rect.bottom() - INSET - 1.0;
    let (left, right) = scale.span(range);
    Rect::from_x_y_ranges(left..=right.max(left + 2.0), (bottom - SPEED)..=bottom)
}

pub fn region_body(painter: &Painter, body: Rect, region: &Region, hue: Hue) {
    let ink = region_fill(painter, body, region, hue);
    region_label(painter, body, region, ink);
}

/// The fill and the outline of a region; the colour of its text.
fn region_fill(painter: &Painter, body: Rect, region: &Region, hue: Hue) -> Color32 {
    let (fill, line, ink) = if region.faded {
        (
            hue.fill.gamma_multiply(FADED),
            hue.line.gamma_multiply(FADED),
            WHITE.gamma_multiply(0.6),
        )
    } else {
        (hue.fill, hue.line, WHITE)
    };
    painter.rect(
        body,
        RADIUS,
        fill,
        Stroke::new(1.0, line),
        StrokeKind::Inside,
    );
    ink
}

/// The name of a region (with its facts below on a clip, at the top over a wave).
fn region_label(painter: &Painter, body: Rect, region: &Region, ink: Color32) {
    // Rows scaled low (Shift+wheel) leave no room for text.
    if region.label.is_empty() || body.height() < FS_S {
        return;
    }
    let clip = painter.with_clip_rect(body.shrink(1.0).intersect(painter.clip_rect()));
    let font = fonts::text(FS_S, Weight::Medium);
    // The text of a region scrolled half out of the lanes stays at their left edge.
    let start = body.left().max(painter.clip_rect().left());
    let detail = region
        .detail
        .as_ref()
        .filter(|_| body.height() >= DETAIL_TOP + SMALL + 2.0);
    match detail {
        None if region.wave.is_some() => {
            let at = pos2(start + 6.0, body.top() + 2.0);
            clip.text(at, Align2::LEFT_TOP, &region.label, font, ink);
        }
        None => {
            clip.text(
                pos2(start + 6.0, body.center().y),
                Align2::LEFT_CENTER,
                &region.label,
                font,
                ink,
            );
        }
        Some(detail) => {
            let left = start + S2;
            clip.text(
                pos2(left, body.top() + 4.0),
                Align2::LEFT_TOP,
                &region.label,
                font,
                ink,
            );
            let mono = fonts::mono(SMALL);
            let at = pos2(left, body.top() + DETAIL_TOP);
            clip.text(at, Align2::LEFT_TOP, detail, mono, ink.gamma_multiply(0.8));
        }
    }
}

/// Speed bands over the bottom of the clips and the dashed seams inside clips.
pub fn speeds(painter: &Painter, rect: Rect, tracks: &Tracks, scale: &TimeScale) {
    for speed in &tracks.speeds {
        let band = speed_band(rect, scale, speed.region.range);
        region_body(painter, band, &speed.region, tokens::HUE_SPEED);
    }
    for &seam in &tracks.seams {
        let x = scale.x(seam);
        let top = pos2(x, rect.top() + INSET);
        let end = pos2(x, rect.bottom() - INSET);
        let stroke = Stroke::new(1.0, Color32::from_white_alpha(90));
        painter.extend(Shape::dashed_line(&[top, end], stroke, 3.0, 3.0));
    }
}

/// The red playhead from the ruler down to `bottom`, with its time in a chip on the ruler (mockup:
/// line 2, chip padding 5 × 2, 2 from the top); nothing when `x` is off the lanes.
pub fn playhead(painter: &Painter, ruler: Rect, bottom: f32, x: f32, t: OutputTime) {
    if x < ruler.left() + super::HEAD || x > ruler.right() {
        return;
    }
    painter.vline(x, ruler.top()..=bottom, Stroke::new(2.0, REC));
    let font = fonts::mono(SMALL);
    let galley = painter.layout_no_wrap(timecode(t), font, WHITE);
    let size = galley.size() + vec2(10.0, 4.0);
    let left = (x - size.x / 2.0)
        .min(ruler.right() - size.x)
        .max(ruler.left() + super::HEAD);
    let tag = Rect::from_min_size(pos2(left, ruler.top() + 2.0), size);
    painter.rect_filled(tag, 3, REC_STRONG);
    painter.galley(tag.min + vec2(5.0, 2.0), galley, WHITE);
}

/// A picked region (mockup `.rg.sel`): a white ring 2 around it and white bars 3 wide inside
/// both ends.
pub fn selected(painter: &Painter, body: Rect) {
    painter.rect_stroke(body, RADIUS, Stroke::new(2.0, WHITE), StrokeKind::Outside);
    let bars = (body.top() + INSET)..=(body.bottom() - INSET);
    for x in [body.left() + 2.0, body.right() - 5.0] {
        let bar = Rect::from_x_y_ranges(x..=x + 3.0, bars.clone());
        painter.rect_filled(bar, 2, WHITE);
    }
}

/// The picked range from x `left` to `right` over `ys` (mockup `.rangesel`): a pale blue
/// fill between blue edges 1.5 wide.
pub fn range(painter: &Painter, left: f32, right: f32, ys: std::ops::RangeInclusive<f32>) {
    let rect = Rect::from_x_y_ranges(left..=right, ys.clone());
    painter.rect_filled(rect, 0, ACCENT_HI.gamma_multiply(RANGE_FILL));
    for x in [left, right] {
        painter.vline(x, ys.clone(), Stroke::new(1.5, ACCENT_HI));
    }
}
