//! The crop and output format dialog (mockup section 4): modal over the editor, the whole source
//! picture of one clip with its crop frame (8 handles, thirds, the rest dimmed), the output
//! format over it with the size of the output frame, the frame in px of the source under it.
//! Everything is a draft: «Применить» answers the `crop` and `aspect` steps of the registry as
//! one undo step (only those that changed), «Отмена», ✕ and Esc change nothing.
//!
//! The dialog context is minimal: the editor keeps the [`Dialog`] while it is open, gives the
//! facts at [`Dialog::open`] (the clip and its name on the timeline) and, every frame, the
//! clip's source picture through the `source` callback of [`show`] (only the editor renders);
//! the [`Answer`] closes it. A fixed format keeps the frame at its proportion: picking it fits
//! the frame to it, the handles keep it; in «Свободный» Shift keeps the frame's own.

use dc_core::{Clip, ClipId, PixelSize, Project, Rect, Step};
use dc_ui_kit::egui::{self, Align, Color32, Key, Layout, Margin, RichText, TextureId, Ui};
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::{crop as text, editor as names, kit};
use dc_ui_kit::tokens::{BG, FS_L, FS_S, S1, S2, S3, S4, TEXT, TEXT_3};
use dc_ui_kit::widgets::{
    Button, IconButton, Kind, modal_line as line, modal_section as section, panel_frame, segmented,
};
use serde_json::{Value, json};

use crate::dialog_geom::{Area, Frame as CropFrame};
use crate::dialog_view as view;
use crate::{Aspect, Crop, CropSettings};

/// Width of the dialog, points (mockup).
const WIDTH: f32 = 760.0;

/// The source picture the editor gives: its texture, or the message shown instead (empty while
/// the picture is coming).
pub type Picture = Result<TextureId, String>;

/// The dialog while it is open: the clip and the draft.
#[derive(Debug)]
pub struct Dialog {
    clip: ClipId,
    name: String,
    /// The clip's source frame, px.
    size: PixelSize,
    /// The first clip: the output frame follows it.
    first: Clip,
    /// The crop and the format at the opening.
    was: (Rect, Aspect),
    pub(crate) frame: CropFrame,
    aspect: Aspect,
    pub(crate) drag: Option<view::Drag>,
}

/// How the dialog closed; the editor drops the [`Dialog`] on either.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// «Применить»: `crop` and `aspect`, each only when it changed, as one undo step (empty
    /// when nothing changed).
    Apply(Vec<Step>),
    /// «Отмена», ✕ or Esc: nothing changes.
    Cancel,
}

/// What the user pressed in a frame of the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pressed {
    Apply,
    Cancel,
}

impl Dialog {
    /// The dialog of clip `clip` of `project`, `name` is its title on the timeline; `None`
    /// without that clip.
    pub fn open(project: &Project, clip: ClipId, name: String) -> Option<Self> {
        let clip = project.clip(clip)?;
        let aspect = project.state::<Crop>().unwrap_or_default().settings.aspect;
        let area = Area::of(clip.media.size);
        Some(Self {
            clip: clip.id,
            name,
            size: clip.media.size,
            first: project.clips.first()?.clone(),
            was: (clip.crop, aspect),
            frame: CropFrame::of(clip.crop, area),
            aspect,
            drag: None,
        })
    }

    /// The clip the dialog crops.
    pub fn clip(&self) -> ClipId {
        self.clip
    }

    /// The clip's source frame, px.
    pub fn size(&self) -> PixelSize {
        self.size
    }

    pub(crate) fn area(&self) -> Area {
        Area::of(self.size)
    }

    /// The proportion the format keeps (w / h of the frame), `None` for «Свободный».
    pub(crate) fn ratio(&self) -> Option<f64> {
        let source = Area::of(self.first.media.size);
        match self.aspect {
            Aspect::Free => None,
            Aspect::Source => Some(source.w / source.h),
            Aspect::Wide16x9 => Some(16.0 / 9.0),
            Aspect::Wide16x10 => Some(16.0 / 10.0),
            Aspect::Standard4x3 => Some(4.0 / 3.0),
        }
    }

    /// The output frame with the draft, px: of the first clip, with its draft crop when the
    /// dialog crops it.
    pub fn output(&self) -> Option<PixelSize> {
        let mut first = self.first.clone();
        if first.id == self.clip {
            first.crop = self.frame.rect(self.area());
        }
        let project = Project {
            clips: vec![first],
            ..Project::default()
        };
        crate::canvas(
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

/// Shows the open dialog; the answer when it closes. `source` gives the clip's whole source
/// picture `size` px large (the aspect of the source).
pub fn show(
    ui: &Ui,
    dialog: &mut Dialog,
    source: &mut dyn FnMut(PixelSize) -> Picture,
) -> Option<Answer> {
    let modal = egui::Modal::new(egui::Id::unique("crop-dialog"))
        .frame(panel_frame().inner_margin(0))
        .show(ui.ctx(), |ui| {
            ui.set_width(WIDTH);
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut pressed = header(ui, &dialog.name);
            section(ui, Color32::TRANSPARENT, Margin::same(S4 as i8), |ui| {
                body(ui, dialog, source);
            });
            pressed = pressed.or(footer(ui, dialog));
            pressed
        });
    let escape = ui.input(|i| i.key_pressed(Key::Escape));
    let pressed = match modal.inner {
        Some(pressed) => Some(pressed),
        None if escape => Some(Pressed::Cancel),
        None => None,
    };
    pressed.map(|pressed| match pressed {
        Pressed::Apply => Answer::Apply(dialog.steps()),
        Pressed::Cancel => Answer::Cancel,
    })
}

/// «Кроп и формат — <клип>» and ✕, a line under them.
fn header(ui: &mut Ui, name: &str) -> Option<Pressed> {
    let margin = Margin {
        left: S4 as i8,
        right: S2 as i8,
        top: 10,
        bottom: 10,
    };
    let mut pressed = None;
    section(ui, Color32::TRANSPARENT, margin, |ui| {
        ui.horizontal(|ui| {
            let title = RichText::new(text::title(name))
                .font(fonts::text(FS_L, Weight::Semibold))
                .color(TEXT);
            ui.label(title);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(IconButton::new(Icon::Close, kit::CLOSE)).clicked() {
                    pressed = Some(Pressed::Cancel);
                }
            });
        });
    });
    line(ui);
    pressed
}

/// The format choice with the output size, the picture, the fields and the hint.
fn body(ui: &mut Ui, dialog: &mut Dialog, source: &mut dyn FnMut(PixelSize) -> Picture) {
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
    view::picture(ui, dialog, source);
    ui.add_space(S2);
    view::fields(ui, dialog);
    ui.add_space(S1);
    ui.label(RichText::new(text::HINT).size(FS_S).color(TEXT_3));
}

/// A line, the note, «Сбросить», «Отмена», «Применить».
fn footer(ui: &mut Ui, dialog: &mut Dialog) -> Option<Pressed> {
    line(ui);
    let mut pressed = None;
    let margin = Margin::symmetric(S4 as i8, 10);
    section(ui, BG, margin, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = S2;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let apply = Button::new(text::APPLY).kind(Kind::Primary).small();
                if ui.add(apply).clicked() {
                    pressed = Some(Pressed::Apply);
                }
                if ui.add(Button::new(text::CANCEL).small()).clicked() {
                    pressed = Some(Pressed::Cancel);
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
    pressed
}
