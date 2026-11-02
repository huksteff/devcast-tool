//! The crop and output format dialog (mockup section 4): modal over the editor, the whole source
//! picture of one clip with its crop frame (8 handles, thirds, the rest dimmed), the output
//! format over it with the size of the output frame, the frame in px of the source under it.
//! Everything is a draft: «Применить» runs `crop` and `aspect` of the registry as one undo step
//! (only those that changed), «Отмена», ✕ and Esc change nothing.
//!
//! The clip is the picked one, otherwise the one under the playhead; the picture is its source
//! frame at the playhead (at its start when the playhead is elsewhere), drawn by the preview's
//! render path (`Preview::source`). A fixed format keeps the frame at its proportion: picking
//! it fits the frame to it, the handles keep it; in «Свободный» Shift keeps the frame's own.

mod geom;
mod view;

#[cfg(test)]
mod tests;

use dc_core::{Clip, ClipId, OutputTime, PixelSize, Project, Rect};
use dc_fx_crop::{Aspect, Crop, CropSettings};
use dc_ops::Step;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::{crop as text, editor as names, kit};
use dc_ui_kit::tokens::{BG, BORDER, FS_L, FS_S, R3, S1, S2, S3, S4, TEXT, TEXT_3};
use dc_ui_kit::widgets::{Button, IconButton, Kind, panel_frame, segmented};
use eframe::egui::{self, Align, Color32, CornerRadius, Frame, Key, Layout, Margin, RichText, Ui};
use serde_json::{Value, json};

use super::timeline::Row;
use super::{Action, EditorWindow};
use geom::{Area, Frame as CropFrame};

/// Width of the dialog, points (mockup).
const WIDTH: f32 = 760.0;

/// The dialog while it is open: the clip and the draft.
#[derive(Debug)]
pub struct CropDialog {
    clip: ClipId,
    name: String,
    /// The moment whose picture is shown (inside the clip).
    time: OutputTime,
    /// The clip's source frame, px.
    size: PixelSize,
    /// The first clip: the output frame follows it.
    first: Clip,
    /// The crop and the format at the opening.
    was: (Rect, Aspect),
    frame: CropFrame,
    aspect: Aspect,
    drag: Option<view::Drag>,
}

/// What the user did in a frame of the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    Apply,
    Cancel,
}

impl CropDialog {
    /// The dialog of the picked clip, or of the one under the playhead; `None` without clips.
    pub fn open(editor: &EditorWindow) -> Option<Self> {
        let project = editor.doc.project();
        let regions = editor
            .tracks
            .rows
            .iter()
            .find(|t| t.row == Row::Clips)
            .map(|t| t.regions.as_slice())
            .unwrap_or_default();
        let picked: Vec<ClipId> = editor.view.selection.clips().collect();
        let at = editor.view.playhead;
        let region = match picked.as_slice() {
            [one] => regions.iter().find(|r| r.clip == Some(*one)),
            _ => None,
        }
        .or_else(|| regions.iter().find(|r| r.range.contains(at)))
        .or_else(|| regions.first())?;
        let clip = project.clip(region.clip?)?;
        let time = match region.range.contains(at) {
            true => at,
            false => region.range.start(),
        };
        let aspect = project.state::<Crop>().unwrap_or_default().settings.aspect;
        let area = Area::of(clip.media.size);
        Some(Self {
            clip: clip.id,
            name: region.label.clone(),
            time,
            size: clip.media.size,
            first: project.clips.first()?.clone(),
            was: (clip.crop, aspect),
            frame: CropFrame::of(clip.crop, area),
            aspect,
            drag: None,
        })
    }

    fn area(&self) -> Area {
        Area::of(self.size)
    }

    /// The proportion the format keeps (w / h of the frame), `None` for «Свободный».
    fn ratio(&self) -> Option<f64> {
        let source = Area::of(self.first.media.size);
        match self.aspect {
            Aspect::Free => None,
            Aspect::Source => Some(source.w / source.h),
            Aspect::Wide16x9 => Some(16.0 / 9.0),
            Aspect::Wide16x10 => Some(16.0 / 10.0),
            Aspect::Standard4x3 => Some(4.0 / 3.0),
        }
    }

    /// The output frame with the draft: of the first clip, with its draft crop when the dialog
    /// crops it.
    fn output(&self) -> Option<PixelSize> {
        let mut first = self.first.clone();
        if first.id == self.clip {
            first.crop = self.frame.rect(self.area());
        }
        let project = Project {
            clips: vec![first],
            ..Project::default()
        };
        dc_fx_crop::canvas(
            &project,
            &CropSettings {
                aspect: self.aspect,
            },
        )
    }

    /// The commands of the draft: `crop` and `aspect`, each only when it changed.
    fn steps(&self) -> Vec<Step> {
        let rect = self.frame.rect(self.area());
        let mut steps = Vec::new();
        if rect != self.was.0 {
            let args = [("clip", json!(self.clip)), ("rect", json!(rect))];
            steps.push(step("crop", args));
        }
        if self.aspect != self.was.1 {
            steps.push(step("aspect", [("aspect", json!(self.aspect))]));
        }
        steps
    }
}

fn step<const N: usize>(op: &str, args: [(&str, Value); N]) -> Step {
    Step::new(
        op,
        args.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
    )
}

/// Shows the open dialog; «Применить» queues its commands, the dialog closes on an answer.
pub fn show(ui: &Ui, editor: &mut EditorWindow) {
    let Some(dialog) = editor.crop.as_mut() else {
        return;
    };
    let preview = &mut editor.preview;
    let modal = egui::Modal::new(egui::Id::unique("crop-dialog"))
        .frame(panel_frame().inner_margin(0))
        .show(ui.ctx(), |ui| {
            ui.set_width(WIDTH);
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut answer = header(ui, &dialog.name);
            section(ui, Color32::TRANSPARENT, Margin::same(S4 as i8), |ui| {
                body(ui, dialog, preview);
            });
            answer = answer.or(footer(ui, dialog));
            answer
        });
    let escape = ui.input(|i| i.key_pressed(Key::Escape));
    let answer = match modal.inner {
        Some(answer) => Some(answer),
        None if escape => Some(Answer::Cancel),
        None => None,
    };
    match answer {
        Some(Answer::Apply) => {
            let steps = dialog.steps();
            if !steps.is_empty() {
                editor.ask(Action::Apply(steps));
            }
            editor.crop = None;
        }
        Some(Answer::Cancel) => editor.crop = None,
        None => {}
    }
}

/// «Кроп и формат — <клип>» and ✕, a line under them.
fn header(ui: &mut Ui, name: &str) -> Option<Answer> {
    let margin = Margin {
        left: S4 as i8,
        right: S2 as i8,
        top: 10,
        bottom: 10,
    };
    let mut answer = None;
    section(ui, Color32::TRANSPARENT, margin, |ui| {
        ui.horizontal(|ui| {
            let title = RichText::new(text::title(name))
                .font(fonts::text(FS_L, Weight::Semibold))
                .color(TEXT);
            ui.label(title);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(IconButton::new(Icon::Close, kit::CLOSE)).clicked() {
                    answer = Some(Answer::Cancel);
                }
            });
        });
    });
    line(ui);
    answer
}

/// The format choice with the output size, the picture, the fields and the hint.
fn body(ui: &mut Ui, dialog: &mut CropDialog, preview: &mut super::preview::Preview) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = S2;
        ui.label(RichText::new(text::FORMAT).size(FS_S).color(TEXT_3));
        let before = dialog.aspect;
        segmented(
            ui,
            &mut dialog.aspect,
            &[
                (Aspect::Free, names::ASPECT_FREE, true),
                (Aspect::Source, names::ASPECT_SOURCE, true),
                (Aspect::Wide16x9, names::ASPECT_16_9, true),
                (Aspect::Wide16x10, names::ASPECT_16_10, true),
                (Aspect::Standard4x3, names::ASPECT_4_3, true),
            ],
        );
        if dialog.aspect != before
            && let Some(ratio) = dialog.ratio()
        {
            dialog.frame = dialog.frame.fitted(ratio, dialog.area());
        }
        if let Some(size) = dialog.output() {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let output = RichText::new(text::output(size.width, size.height))
                    .font(fonts::mono(FS_S))
                    .color(TEXT_3);
                ui.label(output);
            });
        }
    });
    ui.add_space(S3);
    view::picture(ui, dialog, preview);
    ui.add_space(S2);
    view::fields(ui, dialog);
    ui.add_space(S1);
    ui.label(RichText::new(text::HINT).size(FS_S).color(TEXT_3));
}

/// A line, the note, «Сбросить», «Отмена», «Применить».
fn footer(ui: &mut Ui, dialog: &mut CropDialog) -> Option<Answer> {
    line(ui);
    let mut answer = None;
    let margin = Margin::symmetric(S4 as i8, 10);
    section(ui, BG, margin, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = S2;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let apply = Button::new(text::APPLY).kind(Kind::Primary).small();
                if ui.add(apply).clicked() {
                    answer = Some(Answer::Apply);
                }
                if ui.add(Button::new(text::CANCEL).small()).clicked() {
                    answer = Some(Answer::Cancel);
                }
                let reset = Button::new(text::RESET)
                    .kind(Kind::Ghost)
                    .small()
                    .icon(Icon::Reset);
                if ui.add(reset).on_hover_text(text::RESET_HINT).clicked() {
                    dialog.frame = CropFrame::full(dialog.area());
                }
                ui.add_space(S4);
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    let note = RichText::new(text::NOTE).size(FS_S).color(TEXT_3);
                    ui.add(egui::Label::new(note).wrap());
                });
            });
        });
    });
    answer
}

/// A part of the dialog with its own fill and padding, the whole width.
pub(super) fn section(ui: &mut Ui, fill: Color32, margin: Margin, add: impl FnOnce(&mut Ui)) {
    // The footer is the last part: its fill keeps the round corners of the dialog.
    let corners = CornerRadius {
        sw: R3,
        se: R3,
        ..CornerRadius::ZERO
    };
    let frame = Frame::new().fill(fill).inner_margin(margin);
    frame.corner_radius(corners).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

pub(super) fn line(ui: &mut Ui) {
    let rect = ui.available_rect_before_wrap();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(rect.width(), 1.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0, BORDER);
}
