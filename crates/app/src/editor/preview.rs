//! The preview: its toolbar (crop and output format, preview size and quality), the frame and
//! the transport under it.
//!
//! The frame is a placeholder of the project's output size until 7.4 draws the compositor's
//! texture into it; «Вписать / 50% / 100%» size it already. «Кроп и формат» opens its dialog in
//! 7.20, the quality acts in 7.4.

use dc_fx_crop::Aspect;
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::editor as text;
use dc_ui_kit::tokens::{BORDER, FS_S, PREVIEW_BG, S1, S2, S5, SURFACE, TEXT_2};
use dc_ui_kit::widgets::{Button, Kind, segmented};
use eframe::egui::{
    self, Align, Color32, Frame, Layout, Margin, Rect, RichText, Stroke, StrokeKind, Ui, vec2,
};

use super::EditorWindow;
use super::view::{PreviewScale, Quality};

/// Heights of the preview toolbar and of the transport (mockup).
const BAR: f32 = 36.0;
const TRANSPORT: f32 = 48.0;
/// Aspect of the placeholder of a project without clips.
const EMPTY_ASPECT: f32 = 16.0 / 9.0;

pub fn ui(ui: &mut Ui, editor: &mut EditorWindow) {
    let bar = Frame::new()
        .fill(SURFACE)
        .inner_margin(Margin::symmetric(S2 as i8, 0));
    egui::Panel::top("preview-bar")
        .exact_size(BAR)
        .frame(bar)
        .show(ui, |ui| toolbar(ui, editor));
    egui::Panel::bottom("transport")
        .exact_size(TRANSPORT)
        .frame(bar)
        .show(ui, |ui| super::transport::ui(ui, editor));
    egui::CentralPanel::no_frame()
        .frame(Frame::new().fill(PREVIEW_BG))
        .show(ui, |ui| frame(ui, editor));
}

fn toolbar(ui: &mut Ui, editor: &mut EditorWindow) {
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = S1;
        ui.add_enabled(
            false,
            Button::new(text::CROP)
                .kind(Kind::Ghost)
                .small()
                .icon(Icon::Crop),
        );
        ui.add_space(S1);
        if let Some(size) = editor.canvas {
            let format = text::format(aspect_name(editor.aspect), size.width, size.height);
            ui.label(RichText::new(format).size(FS_S).color(TEXT_2));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = S2;
            let view = &mut editor.view;
            segmented(
                ui,
                &mut view.quality,
                &[
                    (Quality::Full, text::QUALITY_FULL, true),
                    (Quality::Fast, text::QUALITY_FAST, true),
                ],
            );
            segmented(
                ui,
                &mut view.scale,
                &[
                    (PreviewScale::Fit, text::FIT, true),
                    (PreviewScale::Half, text::HALF, true),
                    (PreviewScale::Actual, text::ACTUAL, true),
                ],
            );
            ui.label(RichText::new(text::PREVIEW).size(FS_S).color(TEXT_2));
        });
    });
}

/// «16:9», «Свободный» …
fn aspect_name(aspect: Aspect) -> &'static str {
    match aspect {
        Aspect::Free => text::ASPECT_FREE,
        Aspect::Source => text::ASPECT_SOURCE,
        Aspect::Wide16x9 => text::ASPECT_16_9,
        Aspect::Wide16x10 => text::ASPECT_16_10,
        Aspect::Standard4x3 => text::ASPECT_4_3,
    }
}

/// The output frame at the chosen size, centred and clipped to the preview area.
fn frame(ui: &mut Ui, editor: &EditorWindow) {
    let area = ui.max_rect();
    let rect = frame_rect(
        area,
        editor.canvas.map(|c| (c.width, c.height)),
        editor.view.scale,
        ui.pixels_per_point(),
    );
    let painter = ui.painter().with_clip_rect(area);
    painter.rect(
        rect,
        2,
        Color32::BLACK,
        Stroke::new(1.0, BORDER),
        StrokeKind::Outside,
    );
}

/// Where the frame of `canvas` pixels stands in `area`: fitted with a margin, or at half or full
/// size (one frame pixel per one or two physical pixels), centred either way.
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
