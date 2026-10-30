//! Popovers of the HUD (decision 001: small windows of the same look): one at a time, opened
//! upwards 8 px above the bar under the button that opened it; Esc or a click elsewhere closes
//! it. Its window sizes itself to the content.

use std::sync::{Arc, Mutex};

use dc_ui_kit::Icon;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::hud;
use dc_ui_kit::tokens::{BORDER, FS_M, ICON, R1, S2, SURFACE, SURFACE_2, TEXT};
use dc_ui_kit::widgets::{Toggle, panel_frame};
use eframe::egui::{self, Rect, Response, Sense, Ui, WidgetInfo, pos2, vec2};

use dc_capture::Source;

use super::picker::Pick;

use crate::app::{Shared, lock};

/// Which popover is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Source,
    More,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Open {
    pub kind: Kind,
    /// Left edge of the button under it, points from the bar's left edge.
    pub anchor: f32,
}

/// What the «Ещё» menu asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoreAction {
    Settings,
    Quit,
}

/// Gap between the bar and a popover, points.
const GAP: f32 = 8.0;

/// Shows the open popover window above the bar whose outer rectangle is `bar`.
pub fn show(ctx: &egui::Context, shared: &Arc<Mutex<Shared>>, open: Open, bar: Rect) {
    let width = match open.kind {
        Kind::Source => super::picker::WIDTH,
        Kind::More => 260.0,
    };
    let builder = egui::ViewportBuilder::default()
        .with_title(hud::MORE)
        .with_decorations(false)
        .with_resizable(false)
        .with_always_on_top()
        .with_taskbar(false)
        .with_inner_size(vec2(width, 40.0))
        .with_position(pos2(bar.left() + open.anchor - S2, bar.top() - GAP - 40.0));
    let shared = Arc::clone(shared);
    ctx.show_viewport_deferred(
        egui::ViewportId::from_hash_of("hud-popover"),
        builder,
        move |ui, _| {
            let mut shared = lock(&shared);
            let response = panel_frame()
                .shadow(egui::Shadow::NONE)
                .show(ui, |ui| {
                    ui.set_width(width - 2.0 * dc_ui_kit::tokens::S3 - 2.0);
                    match open.kind {
                        Kind::Source => source(ui, &mut shared),
                        Kind::More => more(ui, &mut shared),
                    }
                })
                .response;
            let size = response.rect.size();
            let current = ui.input(|i| i.viewport().inner_rect.map(|r| r.size()));
            if current.is_some_and(|c| (c - size).length() > 0.5) {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                let corner = pos2(bar.left() + open.anchor - S2, bar.top() - GAP - size.y);
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::OuterPosition(corner));
            }
            let lost_focus = ui.input(|i| i.viewport().focused == Some(false));
            if lost_focus
                || ui.input(|i| i.key_pressed(egui::Key::Escape) || i.viewport().close_requested())
            {
                shared.popover = None;
            }
        },
    );
}

fn source(ui: &mut Ui, shared: &mut Shared) {
    let Shared {
        store,
        sources,
        previews,
        ..
    } = shared;
    let Some(previews) = previews else {
        return;
    };
    let textures = previews.textures(ui.ctx());
    let chosen = store.get().recording.source;
    let last = match store.get().last_area {
        Some(Source::Area(area)) => Some((area.width(), area.height())),
        _ => None,
    };
    let Some(picked) = super::picker::picker(ui, sources, &chosen, textures, last) else {
        return;
    };
    shared.popover = None;
    shared.previews = None;
    match picked {
        Pick::Area => shared.area = Some(crate::area::AreaSession::open(shared)),
        Pick::Source(source) => {
            let saved = shared.store.update(|s| s.recording.source = source);
            shared.report(saved);
            if let Source::Window(id) = source {
                shared.report(dc_capture::win::hud_window::bring_to_front(id));
            }
        }
    }
}

fn more(ui: &mut Ui, shared: &mut Shared) {
    let mut hide = shared.store.get().hide_hud;
    let action = more_menu(ui, &mut hide);
    let result = shared.store.update(|s| s.hide_hud = hide);
    shared.report(result);
    match action {
        Some(MoreAction::Settings) => {
            shared.popover = None;
            shared.open_settings();
        }
        Some(MoreAction::Quit) => {
            shared.popover = None;
            ui.ctx()
                .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
        }
        None => {}
    }
}

/// The «Ещё» menu (mockup 1): «Скрывать HUD из записи», «Настройки», «Выход».
pub fn more_menu(ui: &mut Ui, hide: &mut bool) -> Option<MoreAction> {
    ui.spacing_mut().item_spacing.y = 2.0;
    ui.horizontal(|ui| {
        ui.set_min_height(34.0);
        icon(ui, Icon::EyeOff);
        ui.label(
            egui::RichText::new(hud::HIDE_FROM_RECORDING)
                .font(fonts::text(FS_M, Weight::Regular))
                .color(TEXT),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(Toggle::new(hide, hud::HIDE_FROM_RECORDING));
        });
    });
    divider(ui);
    let mut action = None;
    if item(ui, Icon::Gear, hud::SETTINGS).clicked() {
        action = Some(MoreAction::Settings);
    }
    divider(ui);
    if item(ui, Icon::Quit, hud::QUIT).clicked() {
        action = Some(MoreAction::Quit);
    }
    action
}

fn icon(ui: &mut Ui, icon: Icon) {
    let (rect, _) = ui.allocate_exact_size(vec2(ICON, ICON), Sense::hover());
    icon.paint(ui.painter(), rect, TEXT);
}

/// A row of a popover menu: icon and text, lit when hovered.
pub fn item(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::MenuItem, true, text));
    let fill = if response.hovered() {
        SURFACE_2
    } else {
        SURFACE
    };
    ui.painter().rect_filled(rect, R1, fill);
    let icon_rect = Rect::from_min_size(
        pos2(rect.left() + S2, rect.center().y - ICON / 2.0),
        vec2(ICON, ICON),
    );
    icon.paint(ui.painter(), icon_rect, TEXT);
    ui.painter().text(
        pos2(icon_rect.right() + S2, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        fonts::text(FS_M, Weight::Regular),
        TEXT,
    );
    response
}

pub fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 13.0), Sense::hover());
    ui.painter().hline(
        rect.x_range().shrink(4.0),
        rect.center().y,
        egui::Stroke::new(1.0, BORDER),
    );
}
