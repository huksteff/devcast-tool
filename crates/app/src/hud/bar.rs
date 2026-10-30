//! The HUD bar at rest (mockup 1): grip, source, microphone, system sound, fps, countdown,
//! «Запись», «Ещё». Draws itself and reports what was clicked; the window does the rest.

use dc_capture::{Source, SourceList};
use dc_ui_kit::Icon;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::{hud, picker};
use dc_ui_kit::tokens::{
    BORDER, CONTROL, FS_M, ICON, R1, REC, REC_STRONG, S1, S2, SURFACE_3, TEXT, TEXT_3, WHITE,
};
use dc_ui_kit::widgets::IconButton;
use eframe::egui::{self, Rect, Response, Sense, Ui, WidgetInfo, pos2, vec2};

use crate::settings::AppSettings;

/// What the user did on the bar this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clicked {
    /// The grip was grabbed: the window starts moving with the mouse.
    Grip,
    Source,
    Mic,
    SystemAudio,
    Fps,
    Countdown,
    Record,
    More,
}

/// The source as the bar names it: «Экран 1 · 2560×1440», a window title, «Область · 1280×720».
pub fn source_label(source: &Source, sources: &SourceList) -> String {
    match source {
        Source::Monitor(index) => {
            let name = picker::screen(*index);
            match sources.monitors.iter().find(|m| m.index == *index) {
                Some(m) => hud::sized(&name, m.bounds.width(), m.bounds.height()),
                None => name,
            }
        }
        Source::Window(id) => sources
            .windows
            .iter()
            .find(|w| w.id == *id)
            .map_or_else(|| hud::WINDOW.to_owned(), |w| w.title.clone()),
        Source::Area(area) => hud::sized(hud::AREA, area.width(), area.height()),
    }
}

/// Lays the bar out from the left; returns what was clicked.
/// The second value is the left edge of the clicked button, points from the bar's left.
pub fn bar(ui: &mut Ui, settings: &AppSettings, source: &str) -> Option<(Clicked, f32)> {
    let mut clicked = None;
    ui.spacing_mut().item_spacing.x = 2.0;
    ui.add_space(S1);
    let grip = grip(ui);
    if grip.drag_started() {
        clicked = Some((Clicked::Grip, 0.0));
    }
    hit(&mut clicked, source_button(ui, source), Clicked::Source);
    separator(ui);
    let recording = &settings.recording;
    let mic = recording.microphone.is_some();
    let mic_icon = if mic { Icon::Mic } else { Icon::MicOff };
    hit(
        &mut clicked,
        ui.add(IconButton::new(mic_icon, hud::MIC).on(mic).dim(!mic)),
        Clicked::Mic,
    );
    let system = recording.system_audio;
    hit(
        &mut clicked,
        ui.add(
            IconButton::new(Icon::Speaker, hud::SYSTEM_AUDIO)
                .on(system)
                .dim(!system),
        ),
        Clicked::SystemAudio,
    );
    let fps = recording.fps.get().to_string();
    hit(
        &mut clicked,
        ui.add(IconButton::text(&fps, hud::FPS)),
        Clicked::Fps,
    );
    let seconds = settings.countdown.seconds();
    let badge = seconds.to_string();
    let mut timer = IconButton::new(Icon::Timer, hud::COUNTDOWN).on(seconds > 0);
    if seconds > 0 {
        timer = timer.badge(&badge);
    }
    hit(&mut clicked, ui.add(timer), Clicked::Countdown);
    ui.add_space(S1);
    hit(&mut clicked, record_button(ui), Clicked::Record);
    hit(
        &mut clicked,
        ui.add(IconButton::new(Icon::More, hud::MORE)),
        Clicked::More,
    );
    ui.add_space(S2);
    clicked
}

/// Keeps `what` when `response` was clicked.
fn hit(clicked: &mut Option<(Clicked, f32)>, response: Response, what: Clicked) {
    if response.clicked() {
        *clicked = Some((what, response.rect.left()));
    }
}

fn grip(ui: &mut Ui) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(14.0, CONTROL), Sense::drag());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, hud::GRIP));
    let icon = Rect::from_center_size(rect.center(), vec2(ICON, ICON) * 1.4);
    Icon::Grip.paint(ui.painter(), icon, TEXT_3);
    response
}

/// Icon, name (cut to 150 px) and chevron.
fn source_button(ui: &mut Ui, label: &str) -> Response {
    let format = egui::TextFormat::simple(fonts::text(FS_M, Weight::Regular), TEXT);
    let mut job = egui::text::LayoutJob::single_section(label.to_owned(), format);
    job.wrap = egui::text::TextWrapping::truncate_at_width(130.0);
    let galley = ui.painter().layout_job(job);
    let width = 8.0 + ICON + 6.0 + galley.size().x + 6.0 + 12.0 + 8.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, CONTROL), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, hud::SOURCE));
    if response.hovered() {
        ui.painter().rect_filled(rect, R1, SURFACE_3);
    }
    let mut x = rect.left() + 8.0;
    let icon = Rect::from_min_size(pos2(x, rect.center().y - ICON / 2.0), vec2(ICON, ICON));
    Icon::Monitor.paint(ui.painter(), icon, TEXT);
    x += ICON + 6.0;
    let top = rect.center().y - galley.size().y / 2.0;
    let text_width = galley.size().x;
    ui.painter().galley(pos2(x, top), galley, TEXT);
    x += text_width + 6.0;
    let chevron = Rect::from_min_size(pos2(x, rect.center().y - 6.0), vec2(12.0, 12.0));
    Icon::Down.paint(ui.painter(), chevron, TEXT);
    response
}

fn separator(ui: &mut Ui) {
    ui.add_space(S1);
    let (rect, _) = ui.allocate_exact_size(vec2(1.0, 24.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, BORDER);
    ui.add_space(S1);
}

/// Red «● Запись».
fn record_button(ui: &mut Ui) -> Response {
    let font = fonts::text(FS_M, Weight::Semibold);
    let galley = ui
        .painter()
        .layout_no_wrap(hud::RECORD.to_owned(), font, WHITE);
    let width = 10.0 + 12.0 + 8.0 + galley.size().x + 14.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, CONTROL), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), hud::RECORD));
    let fill = if response.hovered() { REC } else { REC_STRONG };
    ui.painter().rect_filled(rect, R1, fill);
    ui.painter()
        .circle_filled(pos2(rect.left() + 16.0, rect.center().y), 6.0, WHITE);
    let top = rect.center().y - galley.size().y / 2.0;
    ui.painter()
        .galley(pos2(rect.left() + 30.0, top), galley, WHITE);
    response
}
