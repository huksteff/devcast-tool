//! Selecting an area (decision 001): a full-screen window on every monitor shows its frozen
//! frame; the rectangle is drawn on one of them. It opens with the last area; «Начать запись»
//! (Enter) keeps the area as the source, «Отмена» (Esc) leaves everything as it was.

pub mod model;
pub mod overlay;

use std::sync::{Arc, Mutex};

use dc_capture::{Area, MonitorInfo, Source};
use eframe::egui::{self, ColorImage, TextureHandle};

use crate::app::{Shared, lock, other_window};
use model::Rect;
use overlay::{Action, Selection};

/// A monitor of the selection.
pub struct Screen {
    pub monitor: MonitorInfo,
    frame: Option<TextureHandle>,
    pub selection: Selection,
}

/// The open selection on all monitors.
pub struct AreaSession {
    pub screens: Vec<Screen>,
}

impl AreaSession {
    /// Freezes every monitor (a full-size WGC frame each) and puts `last` on its monitor.
    pub fn open(shared: &mut Shared) -> Self {
        let last = match shared.store.get().last_area {
            Some(Source::Area(area)) => Some(area),
            _ => None,
        };
        let monitors = shared.sources.monitors.clone();
        let screens = monitors
            .into_iter()
            .map(|monitor| {
                let b = monitor.bounds;
                let side = b.width().max(b.height());
                let frame = match dc_capture::win::preview_monitor(monitor.index, side) {
                    Ok(p) => {
                        let image = ColorImage::from_rgba_unmultiplied(
                            [p.width as usize, p.height as usize],
                            &p.rgba,
                        );
                        Some(shared.ctx.load_texture(
                            format!("area-{}", monitor.index),
                            image,
                            egui::TextureOptions::LINEAR,
                        ))
                    }
                    Err(error) => {
                        shared.report(Err(error));
                        None
                    }
                };
                let rect = last.and_then(|a| local(&monitor, a));
                let size = (b.width() as i32, b.height() as i32);
                Screen {
                    monitor,
                    frame,
                    selection: Selection {
                        rect,
                        preset: model::Preset::Free,
                        drag: None,
                        size,
                    },
                }
            })
            .collect();
        Self { screens }
    }
}

/// `area` in pixels of `monitor` when it lies on it.
fn local(monitor: &MonitorInfo, area: Area) -> Option<Rect> {
    let b = monitor.bounds;
    let (x, y) = (area.x() - b.x(), area.y() - b.y());
    let inside = x >= 0
        && y >= 0
        && x + area.width() as i32 <= b.width() as i32
        && y + area.height() as i32 <= b.height() as i32;
    inside.then_some(Rect {
        x,
        y,
        w: area.width() as i32,
        h: area.height() as i32,
    })
}

/// A window per monitor while the session is open.
pub fn show(ctx: &egui::Context, shared: &Arc<Mutex<Shared>>) {
    let ppp = ctx.native_pixels_per_point().unwrap_or(1.0);
    let monitors: Vec<MonitorInfo> = match &lock(shared).area {
        Some(session) => session.screens.iter().map(|s| s.monitor.clone()).collect(),
        None => return,
    };
    for monitor in monitors {
        let b = monitor.bounds;
        // A borderless window the size of the monitor, not winit's fullscreen: DXGI refuses a
        // swapchain for a window created fullscreen (DXGI_ERROR_INVALID_CALL, the app fell).
        let builder = egui::ViewportBuilder::default()
            .with_title(dc_ui_kit::strings::area::HINT)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_position(egui::pos2(b.x() as f32 / ppp, b.y() as f32 / ppp))
            .with_inner_size(egui::vec2(b.width() as f32 / ppp, b.height() as f32 / ppp));
        let shared = Arc::clone(shared);
        let id = egui::ViewportId::from_hash_of(("area", monitor.index));
        ctx.show_viewport_deferred(id, builder, move |ui, _| {
            other_window(ui, &shared, |ui, shared| {
                screen_ui(ui, shared, monitor.index)
            });
        });
    }
}

fn screen_ui(ui: &mut egui::Ui, shared: &mut Shared, index: u32) {
    let Some(session) = shared.area.as_mut() else {
        return;
    };
    let Some(screen) = session
        .screens
        .iter_mut()
        .find(|s| s.monitor.index == index)
    else {
        return;
    };
    let frame = screen.frame.as_ref().map(TextureHandle::id);
    let was_dragging = screen.selection.drag.is_some();
    let action = overlay::overlay(ui, frame, &mut screen.selection);
    if !was_dragging && screen.selection.drag.is_some() {
        for other in session
            .screens
            .iter_mut()
            .filter(|s| s.monitor.index != index)
        {
            other.selection.rect = None;
        }
    }
    match action {
        Some(Action::Cancel) => shared.area = None,
        Some(Action::Start) => start(shared, index),
        None => {}
    }
}

/// Keeps the area of monitor `index` as the source and as the last area.
fn start(shared: &mut Shared, index: u32) {
    let Some(session) = shared.area.take() else {
        return;
    };
    let Some((monitor, rect)) = session
        .screens
        .iter()
        .find(|s| s.monitor.index == index)
        .and_then(|s| s.selection.rect.map(|r| (s.monitor.bounds, r)))
    else {
        return;
    };
    let area = Area::new(
        monitor.x() + rect.x,
        monitor.y() + rect.y,
        rect.w.unsigned_abs(),
        rect.h.unsigned_abs(),
    );
    let result = area.map_err(crate::error::AppError::from).and_then(|area| {
        shared.store.update(|s| {
            s.recording.source = Source::Area(area);
            s.last_area = Some(Source::Area(area));
        })
    });
    let kept = result.is_ok();
    shared.report(result);
    if kept {
        shared.record();
    }
}

#[cfg(test)]
mod tests;
