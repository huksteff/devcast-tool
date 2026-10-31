//! Drawing of the timeline: ruler, track headers, regions, speed ranges and the playhead, by the
//! mockup's sizes and track colours.

use dc_core::OutputTime;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{
    self, BG, BORDER, FS_S, Hue, REC, ROW_LINE, S2, SURFACE, TEXT_2, TEXT_3, WHITE,
};
use eframe::egui::{Align2, Color32, Painter, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

use super::scale::{TimeScale, ruler_label, timecode};
use super::tracks::{Region, Row, Track, Tracks};

/// Side of a header button, points.
pub const HEAD_BUTTON: f32 = 22.0;
/// Gap between a region and the top and bottom of its row; corner radius of a region.
const INSET: f32 = 3.0;
const RADIUS: u8 = 4;
/// Height of the speed band at the bottom of a clip.
const SPEED: f32 = 14.0;
/// Ruler labels, the playhead time and the clip facts, points (mockup).
const SMALL: f32 = 10.0;
/// Faded regions keep this share of their colour.
const FADED: f32 = 0.4;

/// Row heights (mockup: clips 50, sound 36, the rest 28).
pub fn height(row: Row) -> f32 {
    match row {
        Row::Clips => 50.0,
        Row::Mic | Row::System | Row::Audio(_) => 36.0,
        _ => 28.0,
    }
}

fn hue(row: Row) -> Hue {
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
            let label = ruler_label(t / 1_000_000);
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

/// The lane background and the regions of `track`.
pub fn lane(painter: &Painter, rect: Rect, track: &Track, scale: &TimeScale) {
    painter.rect_filled(rect, 0, BG);
    painter.hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, ROW_LINE));
    let hue = hue(track.row);
    for region in &track.regions {
        let (left, right) = scale.span(region.range);
        let body = Rect::from_x_y_ranges(
            left..=right.max(left + 2.0),
            (rect.top() + INSET)..=(rect.bottom() - INSET),
        );
        paint_region(painter, body, region, hue);
    }
}

fn paint_region(painter: &Painter, body: Rect, region: &Region, hue: Hue) {
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
    if region.label.is_empty() {
        return;
    }
    let clip = painter.with_clip_rect(body.shrink(1.0).intersect(painter.clip_rect()));
    let font = fonts::text(FS_S, Weight::Medium);
    match &region.detail {
        None => {
            clip.text(
                pos2(body.left() + 6.0, body.center().y),
                Align2::LEFT_CENTER,
                &region.label,
                font,
                ink,
            );
        }
        Some(detail) => {
            let left = body.left() + S2;
            clip.text(
                pos2(left, body.top() + 4.0),
                Align2::LEFT_TOP,
                &region.label,
                font,
                ink,
            );
            let mono = fonts::mono(SMALL);
            let at = pos2(left, body.top() + 22.0);
            clip.text(at, Align2::LEFT_TOP, detail, mono, ink.gamma_multiply(0.8));
        }
    }
}

/// Speed bands over the bottom of the clips and the dashed seams inside clips.
pub fn speeds(painter: &Painter, rect: Rect, tracks: &Tracks, scale: &TimeScale) {
    let bottom = rect.bottom() - INSET - 1.0;
    for speed in &tracks.speeds {
        let (left, right) = scale.span(speed.range);
        let band = Rect::from_x_y_ranges(left..=right.max(left + 2.0), (bottom - SPEED)..=bottom);
        paint_region(painter, band, speed, tokens::HUE_SPEED);
    }
    for &seam in &tracks.seams {
        let x = scale.x(seam);
        let top = pos2(x, rect.top() + INSET);
        let end = pos2(x, rect.bottom() - INSET);
        let stroke = Stroke::new(1.0, Color32::from_white_alpha(90));
        painter.extend(Shape::dashed_line(&[top, end], stroke, 3.0, 3.0));
    }
}

/// The red playhead from the ruler down to `bottom`, with its time on the ruler.
pub fn playhead(painter: &Painter, ruler: Rect, bottom: f32, x: f32, t: OutputTime) {
    if x < ruler.left() + super::HEAD {
        return;
    }
    painter.vline(x, ruler.top()..=bottom, Stroke::new(1.5, REC));
    let font = fonts::mono(SMALL);
    let galley = painter.layout_no_wrap(timecode(t), font, WHITE);
    let size = galley.size() + vec2(8.0, 2.0);
    let left = (x - size.x / 2.0).max(ruler.left() + super::HEAD);
    let tag = Rect::from_min_size(pos2(left, ruler.top() + 1.0), size);
    painter.rect_filled(tag, 3, REC);
    painter.galley(tag.min + vec2(4.0, 1.0), galley, WHITE);
}
