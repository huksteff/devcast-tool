//! The HUD bar while recording (mockup 1b): grip, stop with the timer (the biggest target),
//! «REC» blinking every 1.2 s or «ПАУЗА», the microphone level, pause/resume, ✕ (cancel) and
//! «Скрыть панель»; and the bar of «Подготовка записи» after the stop (mockup 7).

use std::time::Duration;

use dc_ui_kit::Icon;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::hud;
use dc_ui_kit::tokens::{
    ACCENT_HI, CONTROL, FS_L, FS_M, FS_S, ICON, PAUSE, PAUSE_SOFT, R2, REC, REC_SOFT, S1, S2, S3,
    SURFACE_3, TEXT, TEXT_2, TEXT_3,
};
use dc_ui_kit::widgets::{IconButton, LevelBar};
use eframe::egui::{self, Rect, Response, Sense, Stroke, Ui, WidgetInfo, pos2, vec2};

use super::bar::{Clicked, grip, hit, separator};

/// Period of the «REC» dot, seconds (v1).
const BLINK: f64 = 1.2;

/// What the recording bar shows.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveView {
    /// `MM:SS` of [`crate::session::clock`].
    pub timer: String,
    pub paused: bool,
    /// Level of the recorded microphone, 0..1; `None` — recorded without one.
    pub mic: Option<f32>,
}

pub fn live_bar(ui: &mut Ui, view: &LiveView) -> Option<(Clicked, f32)> {
    let mut clicked = None;
    ui.spacing_mut().item_spacing.x = 2.0;
    ui.add_space(S1);
    if grip(ui).drag_started() {
        clicked = Some((Clicked::Grip, 0.0));
    }
    let color = if view.paused { PAUSE } else { REC };
    hit(
        &mut clicked,
        stop_button(ui, &view.timer, view.paused),
        Clicked::Stop,
    );
    status(ui, view.paused, color);
    separator(ui);
    mic(ui, view.mic);
    let (icon, label) = if view.paused {
        (Icon::Play, hud::RESUME)
    } else {
        (Icon::Pause, hud::PAUSE)
    };
    hit(
        &mut clicked,
        ui.add(IconButton::new(icon, label).on(view.paused)),
        Clicked::Pause,
    );
    hit(
        &mut clicked,
        ui.add(IconButton::new(Icon::Close, hud::CANCEL)),
        Clicked::Cancel,
    );
    hit(
        &mut clicked,
        ui.add(IconButton::new(Icon::Hide, hud::HIDE)),
        Clicked::Hide,
    );
    ui.add_space(S2);
    clicked
}

/// Red square and the timer on a soft red (yellow while paused) button, at least 112 px.
fn stop_button(ui: &mut Ui, timer: &str, paused: bool) -> Response {
    let (color, fill) = if paused {
        (PAUSE, PAUSE_SOFT)
    } else {
        (REC, REC_SOFT)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(timer.to_owned(), fonts::mono(FS_L), color);
    let width = (8.0 + 14.0 + S2 + galley.size().x + S3).max(112.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, CONTROL), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, hud::STOP));
    let fill = if response.hovered() {
        fill.gamma_multiply(1.6)
    } else {
        fill
    };
    ui.painter().rect_filled(rect, R2, fill);
    let square = Rect::from_min_size(
        pos2(rect.left() + 8.0, rect.center().y - 7.0),
        vec2(14.0, 14.0),
    );
    ui.painter().rect_filled(square, 3, color);
    let top = rect.center().y - galley.size().y / 2.0;
    ui.painter()
        .galley(pos2(square.right() + S2, top), galley, color);
    response
}

/// «● REC» with the dot blinking, or a steady «● ПАУЗА».
fn status(ui: &mut Ui, paused: bool, color: egui::Color32) {
    let text = if paused { hud::PAUSED } else { hud::REC };
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_owned(), fonts::text(FS_S, Weight::Semibold), color);
    let (rect, _) = ui.allocate_exact_size(
        vec2(S2 + 8.0 + 6.0 + galley.size().x + S2, CONTROL),
        Sense::hover(),
    );
    let dot = if paused {
        color
    } else {
        let time = ui.input(|i| i.time);
        let phase = time % BLINK;
        let half = BLINK / 2.0;
        let left = if phase < half {
            half - phase
        } else {
            BLINK - phase
        };
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64(left));
        if phase < half {
            color
        } else {
            color.gamma_multiply(0.25)
        }
    };
    ui.painter()
        .circle_filled(pos2(rect.left() + S2 + 4.0, rect.center().y), 4.0, dot);
    let top = rect.center().y - galley.size().y / 2.0;
    ui.painter()
        .galley(pos2(rect.left() + S2 + 14.0, top), galley, color);
}

/// Microphone icon with a 3 px level under it; crossed out when recorded without one.
fn mic(ui: &mut Ui, level: Option<f32>) {
    let (rect, _) = ui.allocate_exact_size(vec2(CONTROL, CONTROL), Sense::hover());
    let icon = Rect::from_center_size(rect.center() - vec2(0.0, 3.0), vec2(ICON, ICON));
    match level {
        Some(level) => {
            Icon::Mic.paint(ui.painter(), icon, TEXT_2);
            let bar =
                Rect::from_center_size(pos2(rect.center().x, icon.bottom() + 4.0), vec2(18.0, 3.0));
            ui.put(
                bar,
                LevelBar::new(level, 18.0, &hud::level(hud::MIC)).height(3.0),
            );
        }
        None => Icon::MicOff.paint(ui.painter(), icon, TEXT_3),
    }
}

/// Width of the «Подготовка записи» bar, points (mockup 7).
pub const PREPARING_WIDTH: f32 = 380.0;

/// A spinner and «Подготовка записи» / «Редактор откроется через пару секунд».
pub fn preparing(ui: &mut Ui) {
    ui.set_min_width(PREPARING_WIDTH);
    ui.add_space(S3);
    let (rect, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
    let time = ui.input(|i| i.time);
    ui.ctx().request_repaint_after(Duration::from_millis(33));
    let painter = ui.painter();
    painter.circle_stroke(rect.center(), 9.0, Stroke::new(2.0, SURFACE_3));
    let start = (time / 0.9).fract() as f32 * std::f32::consts::TAU;
    let points: Vec<_> = (0..=12)
        .map(|i| {
            let angle = start + i as f32 / 12.0 * std::f32::consts::FRAC_PI_2;
            rect.center() + 9.0 * vec2(angle.cos(), angle.sin())
        })
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(2.0, ACCENT_HI)));
    ui.add_space(S3);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(hud::PREPARING)
                .font(fonts::text(FS_M, Weight::Semibold))
                .color(TEXT),
        );
        ui.label(
            egui::RichText::new(hud::PREPARING_HINT)
                .font(fonts::text(FS_S, Weight::Regular))
                .color(TEXT_3),
        );
    });
}
