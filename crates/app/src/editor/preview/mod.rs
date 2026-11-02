//! The preview: its toolbar (crop and output format, preview size and quality), the frame and
//! the transport under it.
//!
//! The frame is the compositor's texture of the project at the playhead (`View::playhead`),
//! drawn on the render thread (`worker.rs`) by the path of the export and shown by egui without a
//! copy (`live.rs`). It is asked again when the project, the moment, or the size it is rendered
//! at changes; nothing repaints while idle. «Вписать / 50% / 100%» size the frame (a frame larger
//! than the area scrolls), «Полное / Быстрое» choose how many pixels are rendered (`fit.rs`).
//! «Кроп и формат» opens its dialog (`editor/crop`); its source picture comes from a second render
//! thread on the same device (`Preview::source`), so the frame behind the dialog stays.

mod fetch;
mod fit;
mod live;
mod worker;

#[cfg(test)]
mod speed_tests;
#[cfg(test)]
mod worker_tests;

use std::sync::Arc;

use dc_core::{OutputTime, PixelSize, Project};
use dc_fx_crop::Aspect;
use dc_project::ProjectDir;
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::editor as text;
use dc_ui_kit::tokens::{BORDER, FS_S, PREVIEW_BG, S1, S2, S5, SURFACE, TEXT_2};
use dc_ui_kit::widgets::{Button, Kind, segmented};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Layout, Margin, Rect, RichText, ScrollArea, Sense,
    Stroke, StrokeKind, Ui, Vec2, pos2,
};

use super::view::{PreviewScale, Quality};
use super::{EditorWindow, stage};
use crate::gpu::PreviewGpu;
use fit::frame_rect;
use live::Live;
pub use live::Shown;

/// Heights of the preview toolbar and of the transport (mockup).
const BAR: f32 = 36.0;
const TRANSPORT: f32 = 48.0;

/// The project the preview renders and its render thread (none without a GPU, as in tests).
#[derive(Debug)]
pub struct Preview {
    project: Arc<Project>,
    /// Grows with every change of the project: the thread rebuilds its evaluator then.
    revision: u64,
    live: Option<Live>,
    /// The thread of the crop dialog's source picture: idle until the dialog asks.
    source: Option<Live>,
}

impl Preview {
    pub fn new(project: &Project) -> Self {
        Self {
            project: Arc::new(project.clone()),
            revision: 0,
            live: None,
            source: None,
        }
    }

    /// Starts the render thread of the project in `dir`.
    pub fn start(
        &mut self,
        gpu: PreviewGpu,
        dir: ProjectDir,
        ctx: egui::Context,
    ) -> std::io::Result<()> {
        self.source = Some(Live::start(gpu.clone(), dir.clone(), ctx.clone())?);
        self.live = Some(Live::start(gpu, dir, ctx)?);
        Ok(())
    }

    /// The project changed: the frame is rendered again.
    pub fn changed(&mut self, project: &Project) {
        self.project = Arc::new(project.clone());
        self.revision += 1;
    }

    /// The project it renders and its revision.
    pub fn project(&self) -> (Arc<Project>, u64) {
        (Arc::clone(&self.project), self.revision)
    }

    /// A frame came since the last call: the editor window repaints.
    pub fn take_news(&self) -> bool {
        let source = self.source.as_ref().is_some_and(Live::take_news);
        self.live.as_ref().is_some_and(Live::take_news) || source
    }

    /// The whole source picture of the clip playing at `time`, `size` px (its aspect).
    pub fn source(&mut self, time: OutputTime, size: PixelSize) -> Shown {
        let Some(live) = &mut self.source else {
            return Shown::default();
        };
        let want = fetch::Want {
            revision: self.revision,
            time,
            size,
            scrub: false,
            source: true,
            camera: true,
        };
        live.frame(&self.project, want)
    }

    /// The frame of `canvas` at `moment`, shown `shown` points large.
    fn frame(
        &mut self,
        canvas: PixelSize,
        shown: Vec2,
        pixels_per_point: f32,
        quality: Quality,
        moment: Moment,
    ) -> Shown {
        let Some(live) = &mut self.live else {
            return Shown::default();
        };
        let want = fetch::Want {
            revision: self.revision,
            time: moment.time,
            size: fit::render_size(canvas, shown, pixels_per_point, quality),
            scrub: moment.scrub,
            source: false,
            camera: moment.camera,
        };
        live.frame(&self.project, want)
    }
}

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
        let crop = Button::new(text::CROP)
            .kind(Kind::Ghost)
            .small()
            .icon(Icon::Crop);
        if ui.add_enabled(editor.canvas.is_some(), crop).clicked() {
            editor.ask(super::Action::OpenCrop);
        }
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

/// Which frame of the project the preview asks for.
#[derive(Debug, Clone, Copy)]
struct Moment {
    time: OutputTime,
    /// The playhead is dragged: a keyframe will do.
    scrub: bool,
    /// The zoom camera moves the frame; off while a zoom's frame is edited (7.21).
    camera: bool,
}

/// The output frame at the chosen size, centred; scrolled when larger than the preview area.
/// Over it the picked zoom's frame or annotation's box, edited with the pointer (`stage`).
fn frame(ui: &mut Ui, editor: &mut EditorWindow) {
    let area = ui.max_rect();
    let ppp = ui.pixels_per_point();
    let canvas = editor.canvas;
    let view = &editor.view;
    let size = frame_rect(area, canvas.map(|c| (c.width, c.height)), view.scale, ppp).size();
    let moment = Moment {
        time: view.playhead,
        scrub: view.timeline.scrub.is_some(),
        camera: stage::picked_zoom(editor).is_none(),
    };
    let quality = view.quality;
    let shown = match canvas {
        Some(canvas) => editor.preview.frame(canvas, size, ppp, quality, moment),
        None => Shown::default(),
    };
    ScrollArea::both()
        .id_salt("preview-scroll")
        .auto_shrink(false)
        .show(ui, |ui| {
            let content = size.max(ui.available_size());
            let (space, response) = ui.allocate_exact_size(content, Sense::click_and_drag());
            let rect = Rect::from_center_size(space.center(), size);
            paint(ui, rect, &shown);
            stage::ui(ui, editor, &response, rect, shown.camera);
        });
    stage::brush_bar(ui, editor, area);
}

fn paint(ui: &Ui, rect: Rect, shown: &Shown) {
    let painter = ui.painter();
    painter.rect(
        rect,
        2,
        Color32::BLACK,
        Stroke::new(1.0, BORDER),
        StrokeKind::Outside,
    );
    if let Some(texture) = shown.texture {
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture, rect, uv, Color32::WHITE);
    }
    if let Some(error) = &shown.error {
        let wrap = (rect.width() - 2.0 * S5).max(1.0);
        let galley = painter.layout(error.clone(), FontId::proportional(FS_S), TEXT_2, wrap);
        let at = Align2::CENTER_CENTER.anchor_size(rect.center(), galley.size());
        painter.galley(at.min, galley, TEXT_2);
    }
}
