//! The export queue list under the header's queue button (mockup 5): progress and cancel of a
//! job, «Показать в папке» of a done one, the error with «Копировать ошибку» of a failed one;
//! and the toasts of jobs that ended.

use std::path::PathBuf;

use dc_encode::export::Progress;
use dc_encode::{JobState, JobStatus};
use dc_ops::export::Settings;
use dc_ui_kit::strings::export as text;
use dc_ui_kit::tokens::{S1, S3, TEXT_2, TEXT_3};
use dc_ui_kit::widgets::{Button, panel_frame};
use eframe::egui::{self, RichText, Ui};

use super::super::EditorWindow;
use super::spec;

/// Width of the queue list, points (mockup).
const QUEUE_WIDTH: f32 = 340.0;

/// Share of the queue done, 0..1, for the ring of the header: the mean progress of the jobs
/// waiting or running; 1 when none is left.
pub fn progress(jobs: &[JobStatus]) -> f32 {
    let left: Vec<f32> = jobs
        .iter()
        .filter_map(|job| match &job.state {
            JobState::Queued => Some(0.0),
            JobState::Running { progress, .. } => Some(share(*progress)),
            _ => None,
        })
        .collect();
    if left.is_empty() {
        return 1.0;
    }
    left.iter().sum::<f32>() / left.len() as f32
}

/// Share of the frames of a running job done, 0..1.
fn share(progress: Progress) -> f32 {
    match progress.total {
        0 => 0.0,
        total => progress.done as f32 / total as f32,
    }
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
    let at = egui::pos2(
        button.right() - QUEUE_WIDTH - 2.0 * S3,
        button.bottom() + S1,
    );
    let area = egui::Area::new(ui.scope_id().with("export-queue"))
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
                            let share = share(*progress);
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
        exports.cancel(Some(id));
    }
}

/// Opens Explorer with `path` selected.
fn reveal(path: &std::path::Path) {
    let mut select = std::ffi::OsString::from("/select,");
    select.push(path);
    if let Err(error) = std::process::Command::new("explorer.exe")
        .arg(select)
        .spawn()
    {
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
