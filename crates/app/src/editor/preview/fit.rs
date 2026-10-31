//! Where the frame stands in the preview and how many pixels the compositor renders for it.

use dc_core::PixelSize;
use dc_ui_kit::tokens::S5;
use eframe::egui::{Rect, Vec2, vec2};

use crate::editor::view::{PreviewScale, Quality};

/// Aspect of the placeholder of a project without clips.
const EMPTY_ASPECT: f32 = 16.0 / 9.0;

/// Where the frame of `canvas` pixels stands in `area`: fitted with a margin, or at half or full
/// size (one frame pixel per two or one physical pixels), centred either way. A frame larger
/// than the area goes beyond it: the preview scrolls then.
pub fn frame_rect(
    area: Rect,
    canvas: Option<(u32, u32)>,
    scale: PreviewScale,
    pixels_per_point: f32,
) -> Rect {
    let fit = area.shrink(S5);
    let aspect = canvas.map_or(EMPTY_ASPECT, |(w, h)| w as f32 / h.max(1) as f32);
    let fitted = if fit.width() / fit.height().max(1.0) > aspect {
        vec2(fit.height() * aspect, fit.height())
    } else {
        vec2(fit.width(), fit.width() / aspect)
    };
    let size = match (scale, canvas) {
        (PreviewScale::Fit, _) | (_, None) => fitted,
        (PreviewScale::Half, Some((w, h))) => vec2(w as f32, h as f32) / (2.0 * pixels_per_point),
        (PreviewScale::Actual, Some((w, h))) => vec2(w as f32, h as f32) / pixels_per_point,
    };
    Rect::from_center_size(area.center(), size.max(vec2(1.0, 1.0)))
}

/// Pixels the compositor renders for a frame of `canvas` shown `shown` points large.
/// «Полное»: as many pixels as the frame covers on screen, never more than the canvas (a larger
/// frame is the canvas stretched by egui). «Быстрое»: half of that on each side, a quarter of
/// the pixels to draw, for a PC that does not keep up. The aspect of the canvas is kept.
pub fn render_size(
    canvas: PixelSize,
    shown: Vec2,
    pixels_per_point: f32,
    quality: Quality,
) -> PixelSize {
    let (width, height) = (canvas.width.max(1) as f32, canvas.height.max(1) as f32);
    let on_screen = shown * pixels_per_point;
    let full = (on_screen.x / width).min(on_screen.y / height).min(1.0);
    let k = match quality {
        Quality::Full => full,
        Quality::Fast => full / 2.0,
    };
    let side = |canvas: f32| (canvas * k).round().max(1.0) as u32;
    PixelSize {
        width: side(width),
        height: side(height),
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::pos2;

    use super::*;

    fn px(width: u32, height: u32) -> PixelSize {
        PixelSize { width, height }
    }

    #[test]
    fn frame_sizes() {
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(948.0, 364.0));
        let canvas = Some((1920, 1080));
        let fit = frame_rect(area, canvas, PreviewScale::Fit, 1.0);
        assert_eq!(fit.height(), 364.0 - 48.0, "fitted with the 24 px margin");
        assert!((fit.width() / fit.height() - 16.0 / 9.0).abs() < 1e-3);
        assert_eq!(fit.center(), area.center());
        let half = frame_rect(area, canvas, PreviewScale::Half, 1.25);
        assert_eq!(half.size(), vec2(768.0, 432.0), "960 × 540 px at 125 %");
        let actual = frame_rect(area, canvas, PreviewScale::Actual, 1.0);
        assert_eq!(actual.size(), vec2(1920.0, 1080.0));
        let empty = frame_rect(area, None, PreviewScale::Actual, 1.0);
        assert_eq!(empty, fit, "no clips: the 16:9 placeholder is fitted");
        let tall = frame_rect(area, Some((1080, 1920)), PreviewScale::Fit, 1.0);
        assert_eq!(tall.height(), 316.0, "a vertical frame is fitted by height");
    }

    #[test]
    fn full_quality_renders_the_pixels_on_screen_up_to_the_canvas() {
        let canvas = px(2560, 1440);
        let fitted = render_size(canvas, vec2(640.0, 360.0), 1.5, Quality::Full);
        assert_eq!(fitted, px(960, 540), "640 × 360 points at 150 %");
        let actual = render_size(canvas, vec2(2560.0, 1440.0), 1.0, Quality::Full);
        assert_eq!(actual, canvas);
        let half = render_size(canvas, vec2(1280.0, 720.0), 1.0, Quality::Full);
        assert_eq!(half, px(1280, 720));
        let larger = render_size(canvas, vec2(3000.0, 1700.0), 1.25, Quality::Full);
        assert_eq!(larger, canvas, "never more than the output");
    }

    #[test]
    fn fast_quality_halves_each_side_and_keeps_the_aspect() {
        let canvas = px(2560, 1440);
        let fast = render_size(canvas, vec2(2560.0, 1440.0), 1.0, Quality::Fast);
        assert_eq!(fast, px(1280, 720));
        let fitted = render_size(canvas, vec2(640.0, 360.0), 1.5, Quality::Fast);
        assert_eq!(fitted, px(480, 270));
        let odd = render_size(px(1366, 768), vec2(1366.0, 768.0), 1.0, Quality::Fast);
        assert_eq!(odd, px(683, 384));
    }

    #[test]
    fn a_tiny_preview_still_renders_a_pixel() {
        let tiny = render_size(px(1920, 1080), vec2(0.0, 0.0), 1.0, Quality::Fast);
        assert_eq!(tiny, px(1, 1));
    }
}
