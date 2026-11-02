//! Export from the editor (task 7.31, mockup 5): the dialog MP4 / GIF, «Сохранить как…» picks
//! the file every time (rfd), the job joins the app's queue (`crate::exports`) and the dialog
//! closes. The job is the one of `devcast export`: a `dc_ops::export::Spec` of the current
//! working state, unsaved edits included — what the editor shows, as `devcast export` exports
//! `session.json`. The queue button of the header opens the list: progress and cancel of a job,
//! «Показать в папке» of a done one, the error with «Копировать ошибку» of a failed one.

use std::path::PathBuf;

use dc_encode::JobState;
use dc_encode::export::FPS_120_FROM;
use dc_ops::export::{Format, Quality, Res, Settings, Spec};
use dc_ui_kit::strings::export as text;
use dc_ui_kit::tokens::{S1, S2, S3, TEXT_2, TEXT_3};
use dc_ui_kit::widgets::{Button, Kind, Toggle, panel_frame, segmented};
use eframe::egui::{self, Key, RichText, Ui};

use super::EditorWindow;

/// Width of the dialog and of the queue list, points (mockup).
const WIDTH: f32 = 400.0;
const QUEUE_WIDTH: f32 = 340.0;

/// The choices of the open dialog: each format keeps its own.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportDialog {
    pub format: Format,
    pub mp4: Settings,
    pub gif: Settings,
}

impl Default for ExportDialog {
    fn default() -> Self {
        Self {
            format: Format::Mp4,
            mp4: Settings::default_for(Format::Mp4),
            gif: Settings::default_for(Format::Gif),
        }
    }
}

impl ExportDialog {
    pub fn settings(&self) -> Settings {
        match self.format {
            Format::Mp4 => self.mp4,
            Format::Gif => self.gif,
        }
    }
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

/// Shows the open dialog; «Сохранить как…» asks for the file and queues the export.
pub fn show(ui: &Ui, editor: &mut EditorWindow) {
    let Some(dialog) = editor.export.as_mut() else {
        return;
    };
    let high_fps = editor.fps >= FPS_120_FROM;
    let mut answer = None;
    egui::Modal::new(egui::Id::unique("export-dialog"))
        .frame(panel_frame())
        .show(ui.ctx(), |ui| {
            ui.set_width(WIDTH);
            ui.label(RichText::new(text::TITLE).strong());
            ui.add_space(S3);
            answer = body(ui, dialog, high_fps);
        });
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        answer = Some(false);
    }
    match answer {
        Some(true) => {
            let settings = dialog.settings();
            if let Some(out) = pick_file(settings.format, &editor.name) {
                queue(editor, settings, out);
            }
            editor.export = None;
        }
        Some(false) => editor.export = None,
        None => {}
    }
}

/// The fields and the buttons; `Some(true)` for «Сохранить как…», `Some(false)` for «Отмена».
fn body(ui: &mut Ui, dialog: &mut ExportDialog, high_fps: bool) -> Option<bool> {
    field(ui, text::FORMAT, |ui| {
        segmented(
            ui,
            &mut dialog.format,
            &[(Format::Mp4, text::MP4, true), (Format::Gif, text::GIF, true)],
        );
    });
    match dialog.format {
        Format::Mp4 => mp4(ui, &mut dialog.mp4, high_fps),
        Format::Gif => gif(ui, &mut dialog.gif),
    }
    ui.add_space(S3);
    let mut answer = None;
    ui.horizontal(|ui| {
        if dialog.format == Format::Gif {
            ui.label(RichText::new(text::NO_SOUND).color(TEXT_3));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = S2;
            let save = Button::new(text::SAVE_AS).kind(Kind::Primary).small();
            if ui.add(save).clicked() {
                answer = Some(true);
            }
            if ui.add(Button::new(text::CANCEL).small()).clicked() {
                answer = Some(false);
            }
        });
    });
    answer
}

fn mp4(ui: &mut Ui, settings: &mut Settings, high_fps: bool) {
    field(ui, text::RESOLUTION, |ui| {
        let options = [
            (Res::P720, text::P720, true),
            (Res::P1080, text::P1080, true),
            (Res::P1440, text::P1440, true),
            (Res::P2160, text::P2160, true),
            (Res::Source, text::SOURCE, true),
        ];
        segmented(ui, &mut settings.res, &options);
    });
    if settings.fps == 120 && !high_fps {
        settings.fps = 60;
    }
    field(ui, text::FPS, |ui| {
        let options = [(30, "30", true), (60, "60", true), (120, "120", high_fps)];
        segmented(ui, &mut settings.fps, &options);
    });
    field(ui, text::QUALITY, |ui| {
        let options = [
            (Quality::Low, text::LOW, true),
            (Quality::Medium, text::MEDIUM, true),
            (Quality::High, text::HIGH, true),
        ];
        segmented(ui, &mut settings.quality, &options);
    });
}

fn gif(ui: &mut Ui, settings: &mut Settings) {
    field(ui, text::SIZE, |ui| {
        let options = [
            (Res::P720, text::P720, true),
            (Res::P1080, text::P1080, true),
            (Res::Source, text::SOURCE, true),
        ];
        segmented(ui, &mut settings.res, &options);
    });
    field(ui, text::FPS, |ui| {
        let options = [(15, "15", true), (20, "20", true), (25, "25", true), (30, "30", true)];
        segmented(ui, &mut settings.fps, &options);
    });
    field(ui, text::LOOP, |ui| {
        ui.add(Toggle::new(&mut settings.looped, text::LOOP));
    });
}

/// A row: the caption on the left, the control on the right.
fn field(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(TEXT_2));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
    });
    ui.add_space(S2);
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

/// Queues the export of the current state into `out`.
pub fn queue(editor: &mut EditorWindow, settings: Settings, out: PathBuf) {
    let spec = spec(editor, settings, out);
    if let Some(exports) = &editor.exports {
        exports.push(spec);
        editor.queue_open = true;
    }
}

/// The queue list under the header's queue `button` while it is open.
pub fn queue_list(ui: &Ui, editor: &mut EditorWindow, button: egui::Rect) {
    let Some(exports) = editor.exports.clone() else {
        return;
    };
    if !editor.queue_open {
        return;
    }
    let at = egui::pos2(button.right() - QUEUE_WIDTH - 2.0 * S3, button.bottom() + S1);
    let area = egui::Area::new(ui.id().with("export-queue"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ui.ctx(), |ui| {
            panel_frame().show(ui, |ui| {
                ui.set_width(QUEUE_WIDTH);
                ui.label(RichText::new(text::QUEUE).strong());
                for job in exports.status().iter().rev() {
                    ui.separator();
                    ui.label(&job.label);
                    match &job.state {
                        JobState::Queued => {
                            ui.label(RichText::new(text::QUEUED).color(TEXT_3));
                            cancel(ui, &exports, job.id);
                        }
                        JobState::Running { progress, .. } => {
                            let share = match progress.total {
                                0 => 0.0,
                                total => progress.done as f32 / total as f32,
                            };
                            let percent = (share * 100.0) as u64;
                            ui.add(egui::ProgressBar::new(share).text(text::running(percent)));
                            cancel(ui, &exports, job.id);
                        }
                        JobState::Done { exported, .. } => {
                            ui.label(RichText::new(text::done(exported.bytes)).color(TEXT_3));
                            let show = Button::new(text::SHOW_IN_FOLDER).small();
                            if ui.add(show).clicked() {
                                reveal(&exported.path);
                            }
                        }
                        JobState::Failed(error) => {
                            ui.label(RichText::new(error).color(TEXT_2));
                            let copy = Button::new(text::COPY_ERROR).small();
                            if ui.add(copy).clicked() {
                                ui.ctx().copy_text(error.clone());
                            }
                        }
                        JobState::Cancelled => {
                            ui.label(RichText::new(text::CANCELLED).color(TEXT_3));
                        }
                    }
                }
            });
        });
    let outside = ui.input(|i| {
        let p = &i.pointer;
        p.any_pressed()
            && p.interact_pos()
                .is_some_and(|at| !area.response.rect.contains(at))
    });
    if outside {
        editor.queue_open = false;
    }
}

fn cancel(ui: &mut Ui, exports: &crate::exports::Exports, id: u32) {
    if ui.add(Button::new(text::CANCEL_JOB).small()).clicked() {
        exports.cancel(id);
    }
}

/// Opens Explorer with `path` selected.
fn reveal(path: &std::path::Path) {
    let mut select = std::ffi::OsString::from("/select,");
    select.push(path);
    if let Err(error) = std::process::Command::new("explorer.exe").arg(select).spawn() {
        tracing::warn!("Проводник не открылся: {error}");
    }
}

/// The toasts of jobs that ended since the last frame: done ones and failed ones.
pub fn ended(editor: &EditorWindow) -> (Vec<String>, Vec<String>) {
    let mut done = Vec::new();
    let mut failed = Vec::new();
    let Some(exports) = &editor.exports else {
        return (done, failed);
    };
    for job in exports.untold() {
        match &job.state {
            JobState::Failed(error) => failed.push(text::failed(&job.label, error)),
            _ => done.push(text::finished(&job.label)),
        }
    }
    (done, failed)
}
