//! Toasts: short messages in the lower right corner that go away by themselves (a click
//! closes one sooner). The window repaints only when the next one expires.

use std::time::Duration;

use egui::{Align2, Color32, Frame, Margin, Sense, Stroke, vec2};

use crate::fonts::{self, Weight};
use crate::icons::Icon;
use crate::theme::SHADOW;
use crate::tokens::{BORDER, FS_M, ICON, OK, PAUSE, R2, REC, S2, S4, SURFACE, TEXT};

/// How long a toast stays, seconds.
const LIFETIME: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Done,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
struct Toast {
    kind: ToastKind,
    text: String,
    /// `egui::InputState::time` when it goes away.
    until: f64,
}

/// The toasts of one window.
#[derive(Debug, Default)]
pub struct Toasts {
    items: Vec<Toast>,
}

impl Toasts {
    pub fn push(&mut self, ctx: &egui::Context, kind: ToastKind, text: impl Into<String>) {
        let until = ctx.input(|i| i.time) + LIFETIME;
        self.items.push(Toast {
            kind,
            text: text.into(),
            until,
        });
        ctx.request_repaint();
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Draws the living toasts and drops the expired and clicked ones.
    pub fn show(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        self.items.retain(|toast| toast.until > now);
        let Some(next) = self.items.iter().map(|t| t.until).reduce(f64::min) else {
            return;
        };
        ctx.request_repaint_after(Duration::from_secs_f64(next - now));
        let mut closed = None;
        egui::Area::new(egui::Id::unique("dc-toasts"))
            .anchor(Align2::RIGHT_BOTTOM, vec2(-S4, -S4))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                for (i, toast) in self.items.iter().enumerate() {
                    if show_toast(ui, toast) {
                        closed = Some(i);
                    }
                }
            });
        if let Some(i) = closed {
            self.items.remove(i);
        }
    }
}

/// One toast; true when clicked.
fn show_toast(ui: &mut egui::Ui, toast: &Toast) -> bool {
    let (icon, color): (Icon, Color32) = match toast.kind {
        ToastKind::Done => (Icon::Check, OK),
        ToastKind::Warning => (Icon::Warn, PAUSE),
        ToastKind::Error => (Icon::Warn, REC),
    };
    let frame = Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(R2)
        .shadow(SHADOW)
        .inner_margin(Margin::symmetric(S2 as i8 + 4, S2 as i8));
    let response = frame
        .show(ui, |ui| {
            ui.set_max_width(320.0);
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(ICON, ICON), Sense::hover());
                icon.paint(ui.painter(), rect, color);
                let font = fonts::text(FS_M, Weight::Regular);
                let galley = ui.painter().layout(toast.text.clone(), font, TEXT, 280.0);
                let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
                ui.painter().galley(rect.min, galley, TEXT);
            });
        })
        .response
        .interact(Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Alert, true, &toast.text));
    response.clicked()
}
