//! Export from the editor (task 7.31, mockup 5): the dialog MP4 / GIF, «Сохранить как…» picks
//! the file every time (rfd), the job joins the app's queue (`crate::exports`) and the dialog
//! closes. The job is the one of `devcast export`: a `dc_ops::export::Spec` of the current
//! working state, unsaved edits included — what the editor shows, as `devcast export` exports
//! `session.json`. The dialog opens with its last choices (`settings.json` `export`, owner
//! decision 2026-11-02), shows the output frame of each resolution, the encoder an MP4 starts
//! with (`Exports::encoder`) and the output length with a guessed size (`estimate.rs`).

mod estimate;
mod queue;

use std::path::PathBuf;

use dc_core::{PixelSize, TimeDelta};
use dc_encode::Codec;
use dc_encode::export::FPS_120_FROM;
use dc_ops::export::{Format, Quality, Res, Settings, Spec};
use dc_settings::{AppSettings, ExportChoices, Store, StoreError};
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::{export as text, kit};
use dc_ui_kit::tokens::{BG, CONTROL, FS_L, FS_M, FS_S, ICON, S1, S2, S4, TEXT, TEXT_2, TEXT_3};
use dc_ui_kit::widgets::{
    Button, IconButton, Kind, Toggle, dropdown, modal_line as line, modal_section as section,
    panel_frame, segmented,
};
use eframe::egui::{self, Align, Color32, Key, Layout, Margin, RichText, Ui};

pub use queue::{ended, progress, queue, queue_list};

use super::EditorWindow;
use crate::exports::Encoder;

/// Width of the dialog, points (mockup).
const WIDTH: f32 = 400.0;
/// Width of the resolution drop-down: «Исходный · 3840 × 2160» fits.
const RESOLUTION_WIDTH: f32 = 190.0;

/// The choices of the open dialog: each format keeps its own.
pub type ExportDialog = ExportChoices;

/// What the dialog shows of the editor besides the choices.
struct Facts {
    /// The output frame; `None` for an empty project.
    canvas: Option<PixelSize>,
    /// A recording of ≥ 100 fps allows 120.
    high_fps: bool,
    /// The output length.
    length: TimeDelta,
    encoder: Option<Encoder>,
}

/// The job of exporting the editor's project with `settings` into `out`.
pub fn spec(editor: &EditorWindow, settings: Settings, out: PathBuf) -> Spec {
    Spec {
        root: editor.doc.dir().root().to_path_buf(),
        project: editor.doc.project().clone(),
        settings,
        out,
    }
}

/// Opens the dialog with the last choices.
pub fn open(editor: &mut EditorWindow) {
    editor.export = Some(editor.export_choices);
}

/// The last choices of the settings, each frame before the layout.
pub fn recall(editor: &mut EditorWindow, settings: &AppSettings) {
    editor.export_choices = settings.export;
}

/// Keeps the choices of the dialog in the settings once they changed.
pub fn remember(editor: &EditorWindow, store: &mut Store) -> Result<(), StoreError> {
    let chosen = editor.export_choices;
    if store.get().export == chosen {
        return Ok(());
    }
    store.update(|s| s.export = chosen)
}

/// Shows the open dialog; «Сохранить как…» asks for the file and queues the export.
pub fn show(ui: &Ui, editor: &mut EditorWindow) {
    if editor.export.is_none() {
        return;
    }
    let facts = Facts {
        canvas: editor.canvas,
        high_fps: editor.fps >= FPS_120_FROM,
        length: editor.end().since(dc_core::OutputTime::ZERO),
        encoder: editor
            .exports
            .as_ref()
            .map(crate::exports::Exports::encoder),
    };
    let Some(dialog) = editor.export.as_mut() else {
        return;
    };
    let modal = egui::Modal::new(egui::Id::unique("export-dialog"))
        .frame(panel_frame().inner_margin(0))
        .show(ui.ctx(), |ui| {
            ui.set_width(WIDTH);
            ui.spacing_mut().item_spacing.y = 0.0;
            let closed = header(ui);
            section(ui, Color32::TRANSPARENT, Margin::same(S4 as i8), |ui| {
                ui.spacing_mut().item_spacing.y = S2;
                body(ui, dialog, &facts);
            });
            let answer = footer(ui, dialog, &facts);
            if closed { Some(false) } else { answer }
        });
    let chosen = *dialog;
    editor.export_choices = chosen;
    let escape = ui.input(|i| i.key_pressed(Key::Escape));
    match modal.inner.or(escape.then_some(false)) {
        Some(true) => {
            let settings = chosen.settings();
            if let Some(out) = pick_file(settings.format, &editor.name) {
                queue(editor, settings, out);
            }
            editor.export = None;
        }
        Some(false) => editor.export = None,
        None => {}
    }
}

/// «Экспорт» and ✕ (true when clicked), a line under them.
fn header(ui: &mut Ui) -> bool {
    let margin = Margin {
        left: S4 as i8,
        right: S2 as i8,
        top: 10,
        bottom: 10,
    };
    let mut closed = false;
    section(ui, Color32::TRANSPARENT, margin, |ui| {
        ui.horizontal(|ui| {
            let title = RichText::new(text::TITLE)
                .font(fonts::text(FS_L, Weight::Semibold))
                .color(TEXT);
            ui.label(title);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                closed = ui.add(IconButton::new(Icon::Close, kit::CLOSE)).clicked();
            });
        });
    });
    line(ui);
    closed
}

/// The fields of the chosen format.
fn body(ui: &mut Ui, dialog: &mut ExportDialog, facts: &Facts) {
    field(ui, text::FORMAT, None, |ui| {
        segmented(
            ui,
            &mut dialog.format,
            &[
                (Format::Mp4, text::MP4, true),
                (Format::Gif, text::GIF, true),
            ],
        );
    });
    match dialog.format {
        Format::Mp4 => mp4(ui, &mut dialog.mp4, facts),
        Format::Gif => gif(ui, &mut dialog.gif),
    }
}

fn mp4(ui: &mut Ui, settings: &mut Settings, facts: &Facts) {
    let names = [
        (Res::P720, text::P720),
        (Res::P1080, text::P1080),
        (Res::P1440, text::P1440),
        (Res::P2160, text::P2160),
        (Res::Source, text::SOURCE),
    ];
    let captions: Vec<(Res, String)> = names
        .iter()
        .map(|&(res, name)| match facts.canvas {
            Some(canvas) => {
                let size = Settings { res, ..*settings }.resolution().size(canvas);
                (res, text::resolution(name, size.width, size.height))
            }
            None => (res, name.to_owned()),
        })
        .collect();
    let items: Vec<(Res, &str)> = captions.iter().map(|(r, c)| (*r, c.as_str())).collect();
    field(ui, text::RESOLUTION, None, |ui| {
        dropdown(
            ui,
            text::RESOLUTION,
            &mut settings.res,
            &items,
            RESOLUTION_WIDTH,
        );
    });
    if settings.fps == 120 && !facts.high_fps {
        settings.fps = 60;
    }
    field(ui, text::FPS, None, |ui| {
        let options = [
            (30, "30", true),
            (60, "60", true),
            (120, "120", facts.high_fps),
        ];
        segmented(ui, &mut settings.fps, &options);
    });
    field(ui, text::QUALITY, None, |ui| {
        let options = [
            (Quality::Low, text::LOW, true),
            (Quality::Medium, text::MEDIUM, true),
            (Quality::High, text::HIGH, true),
        ];
        segmented(ui, &mut settings.quality, &options);
    });
    if let Some(encoder) = &facts.encoder {
        encoder_row(ui, encoder);
    }
}

/// «Кодирует» with the state of the self-test under it and the encoder on the right; a failed
/// check shows its error (what to do) under the row.
fn encoder_row(ui: &mut Ui, encoder: &Encoder) {
    let (state, value) = match encoder {
        Encoder::Checking => (text::CHECKING, ""),
        Encoder::Ready(codec) => (text::CHECKED, codec_name(*codec)),
        Encoder::Failed(_) => (text::CHECK_FAILED, ""),
    };
    field(ui, text::ENCODES, Some(state), |ui| {
        ui.label(RichText::new(value).size(FS_S).color(TEXT_2));
    });
    if let Encoder::Failed(error) = encoder {
        let error = RichText::new(error.as_str()).size(FS_S).color(TEXT_3);
        ui.add(egui::Label::new(error).wrap());
    }
}

/// The encoder as the row names it.
fn codec_name(codec: Codec) -> &'static str {
    match codec {
        Codec::Nvenc => text::NVENC,
        Codec::Qsv => text::QSV,
        Codec::Amf => text::AMF,
        Codec::MediaFoundation => text::MEDIA_FOUNDATION,
        Codec::X264 => text::X264,
    }
}

fn gif(ui: &mut Ui, settings: &mut Settings) {
    field(ui, text::SIZE, None, |ui| {
        let options = [
            (Res::P720, text::P720, true),
            (Res::P1080, text::P1080, true),
            (Res::Source, text::SOURCE, true),
        ];
        segmented(ui, &mut settings.res, &options);
    });
    field(ui, text::FPS, None, |ui| {
        let options = [
            (15, "15", true),
            (20, "20", true),
            (25, "25", true),
            (30, "30", true),
        ];
        segmented(ui, &mut settings.fps, &options);
    });
    field(ui, text::LOOP, None, |ui| {
        ui.add(Toggle::new(&mut settings.looped, text::LOOP));
    });
}

/// A row: the caption (and a small note under it) on the left, the control on the right.
fn field(ui: &mut Ui, label: &str, note: Option<&str>, add: impl FnOnce(&mut Ui)) {
    let row = egui::vec2(ui.available_width(), CONTROL);
    ui.allocate_ui_with_layout(row, Layout::right_to_left(Align::Center), |ui| {
        add(ui);
        // One label for the caption and its note: it is centred in the row as a whole.
        let format =
            |size, color| egui::TextFormat::simple(fonts::text(size, Weight::Regular), color);
        let mut caption =
            egui::text::LayoutJob::single_section(label.to_owned(), format(FS_M, TEXT_2));
        if let Some(note) = note {
            caption.append(&format!("\n{note}"), 0.0, format(FS_S, TEXT_3));
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.label(caption);
        });
    });
}

/// A line, then the output length with the guessed size (MP4) or «GIF без звука», «Отмена»
/// and «Сохранить как…»: `Some(true)` for «Сохранить как…», `Some(false)` for «Отмена».
fn footer(ui: &mut Ui, dialog: &ExportDialog, facts: &Facts) -> Option<bool> {
    line(ui);
    let mut answer = None;
    section(ui, BG, Margin::symmetric(S4 as i8, 10), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = S2;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let save = Button::new(text::SAVE_AS).kind(Kind::Primary).small();
                if ui.add(save).clicked() {
                    answer = Some(true);
                }
                if ui.add(Button::new(text::CANCEL).small()).clicked() {
                    answer = Some(false);
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = S1;
                    summary(ui, dialog, facts);
                });
            });
        });
    });
    answer
}

fn summary(ui: &mut Ui, dialog: &ExportDialog, facts: &Facts) {
    let small = |line: &str| RichText::new(line).size(FS_S).color(TEXT_3);
    if dialog.format == Format::Gif {
        ui.label(small(text::NO_SOUND));
        return;
    }
    let settings = dialog.mp4;
    let bytes = facts.canvas.map(|canvas| {
        let frame = settings.resolution().size(canvas);
        estimate::mp4_bytes(frame, settings.fps, settings.quality, facts.length)
    });
    let seconds = facts.length.as_secs_f64().max(0.0).round() as u64;
    ui.label(small(&text::summary(seconds, bytes)));
    if bytes.is_some() {
        let side = egui::vec2(ICON, ICON) * 0.75;
        let (rect, response) = ui.allocate_exact_size(side, egui::Sense::hover());
        Icon::Help.paint(ui.painter(), rect, TEXT_3);
        let info = || egui::WidgetInfo::labeled(egui::Role::Image, true, text::ESTIMATE_HINT);
        response.widget_info(info);
        response.on_hover_text(text::ESTIMATE_HINT);
    }
}

/// The system dialog «Куда сохранить видео», named after the project.
fn pick_file(format: Format, name: &str) -> Option<PathBuf> {
    let (filter, extension) = match format {
        Format::Mp4 => (text::MP4_FILES, "mp4"),
        Format::Gif => (text::GIF_FILES, "gif"),
    };
    let mut path = rfd::FileDialog::new()
        .set_title(text::PICK_FILE)
        .set_file_name(format!("{name}.{extension}"))
        .add_filter(filter, &[extension])
        .save_file()?;
    if path.extension().is_none() {
        path.set_extension(extension);
    }
    Some(path)
}
