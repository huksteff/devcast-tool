//! Popovers of the HUD (decision 001: small windows of the same look): one at a time, opened
//! upwards 8 px above the bar under the button that opened it; Esc or a click elsewhere closes
//! it. Its window sizes itself to the content.

use std::sync::{Arc, Mutex};

use dc_ui_kit::Icon;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::{hud, picker as picker_text, tray};
use dc_ui_kit::tokens::{BORDER, FS_M, ICON, R1, S2, SURFACE, SURFACE_2, TEXT};
use dc_ui_kit::widgets::{
    Answer, Confirm, ErrorAction, ErrorPanel, Kind as ButtonKind, Toggle, panel_frame,
};
use eframe::egui::{self, Rect, Response, Sense, Ui, WidgetInfo, pos2, vec2};

use dc_capture::Source;

use super::picker::Pick;

use crate::app::{Shared, lock, other_window};
use crate::session::Session;

/// Which popover is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Source,
    /// System sound and microphones.
    Sound,
    /// Frame rate and countdown.
    Timing,
    More,
    /// «Отменить запись?» over ✕ while recording.
    Cancel,
    /// «Идёт запись»: the tray's «Выход» while recording.
    Quit,
    /// «Запись не подготовлена» with «Открыть папку записи» (mockup 7).
    Failed,
}

impl Kind {
    /// Title of the popover's window (UI Automation finds windows by it).
    pub fn title(self) -> &'static str {
        match self {
            Self::Source => picker_text::TITLE,
            Self::Sound => hud::SOUND,
            Self::Timing => hud::TIMING,
            Self::More => hud::MORE,
            Self::Cancel => hud::CANCEL_TITLE,
            Self::Quit => tray::QUIT_TITLE,
            Self::Failed => hud::PREPARE_FAILED,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Open {
    pub kind: Kind,
    /// Left edge of the button under it, points from the bar's left edge.
    pub anchor: f32,
    /// The window has had the focus. Until then losing it does not close the popover: Windows
    /// does not activate a window opened while another app is in front (a button pressed
    /// through UI Automation), and the popover closed on its first frame.
    focused: bool,
    /// Exclusion from capture as last set on the window.
    excluded: Option<bool>,
}

impl Open {
    pub fn new(kind: Kind, anchor: f32) -> Self {
        Self {
            kind,
            anchor,
            focused: false,
            excluded: None,
        }
    }

    /// True when the window must be excluded from capture anew: it follows «Скрывать HUD из
    /// записи» like the bar, so it is in no recording, no still frame of the area selection and
    /// no preview of the picker.
    pub fn exclude(&mut self, hidden: bool) -> bool {
        let changed = self.excluded != Some(hidden);
        self.excluded = Some(hidden);
        changed
    }

    /// Takes the window's focus state; true when the focus went elsewhere after it had it.
    pub fn focus_lost(&mut self, focused: Option<bool>) -> bool {
        match focused {
            Some(true) => {
                self.focused = true;
                false
            }
            Some(false) => self.focused,
            None => false,
        }
    }
}

/// What the «Ещё» menu asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoreAction {
    Settings,
    Quit,
}

/// Highest a popover gets, points (mockup: the source picker scrolls past 620).
pub const MAX_HEIGHT: f32 = 620.0;

/// Gap between the bar and a popover, points.
const GAP: f32 = 8.0;

/// The popover's window; one at a time.
pub const ID: &str = "hud-popover";

/// Shows the open popover window above the bar whose outer rectangle is `bar`.
pub fn show(ctx: &egui::Context, shared: &Arc<Mutex<Shared>>, open: Open, bar: Rect) {
    let width = match open.kind {
        Kind::Source => super::picker::WIDTH,
        Kind::Sound => super::menus::sound_width(ctx, &lock(shared).microphones),
        Kind::Timing => super::menus::TIMING_WIDTH,
        Kind::More => 260.0,
        Kind::Cancel | Kind::Quit => 280.0,
        Kind::Failed => 360.0,
    };
    let shared = Arc::clone(shared);
    let builder = builder(open, width, bar);
    ctx.show_viewport_deferred(egui::ViewportId::from_hash_of(ID), builder, move |ui, _| {
        other_window(ui, &shared, |ui, shared| {
            frame(ui, shared, open, width, bar)
        });
    });
}

/// The window of `open`, `width` wide, `GAP` above the bar (it starts 40 high, then fits).
pub fn builder(open: Open, width: f32, bar: Rect) -> egui::ViewportBuilder {
    crate::borderless::window(open.kind.title())
        .with_resizable(false)
        .with_inner_size(vec2(width, 40.0))
        .with_position(pos2(bar.left() + open.anchor - S2, bar.top() - GAP - 40.0))
}

/// One frame of the popover window: its content, its size to fit, closing on Esc or lost focus.
fn frame(ui: &mut Ui, shared: &mut Shared, open: Open, width: f32, bar: Rect) {
    let hidden = shared.store.get().hide_hud;
    if shared.popover.as_mut().is_some_and(|o| o.exclude(hidden)) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::ContentProtected(hidden));
    }
    // Laid out in the full popover height, not the window's current one: the window then
    // takes the size of the content (it starts 40 px high).
    let room = Rect::from_min_size(ui.max_rect().min, vec2(width, MAX_HEIGHT));
    let response = ui
        .scope_builder(egui::UiBuilder::new().max_rect(room), |ui| {
            panel_frame()
                .shadow(egui::Shadow::NONE)
                .show(ui, |ui| {
                    ui.set_width(width - 2.0 * dc_ui_kit::tokens::S3 - 2.0);
                    match open.kind {
                        Kind::Source => source(ui, shared),
                        Kind::Sound => sound(ui, shared),
                        Kind::Timing => timing(ui, shared),
                        Kind::More => more(ui, shared),
                        Kind::Cancel => cancel(ui, shared),
                        Kind::Quit => quit(ui, shared),
                        Kind::Failed => failed(ui, shared),
                    }
                })
                .response
        })
        .inner;
    let size = response.rect.size();
    let current = ui.input(|i| i.viewport().inner_rect.map(|r| r.size()));
    if current.is_some_and(|c| (c - size).length() > 0.5) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        let corner = pos2(bar.left() + open.anchor - S2, bar.top() - GAP - size.y);
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::OuterPosition(corner));
    }
    let focused = ui.input(|i| i.viewport().focused);
    let lost_focus = shared
        .popover
        .as_mut()
        .is_some_and(|open| open.focus_lost(focused));
    if lost_focus
        || ui.input(|i| i.key_pressed(egui::Key::Escape) || i.viewport().close_requested())
    {
        close(shared);
    }
}

/// Closes the open popover; closing the cancel question means «Продолжить запись».
pub fn close(shared: &mut Shared) {
    match shared.popover {
        Some(Open {
            kind: Kind::Cancel, ..
        }) => shared.answer_cancel(false),
        _ => shared.popover = None,
    }
}

/// «Отменить запись?» (mockup 1b): the recording is paused while it is shown.
fn cancel(ui: &mut Ui, shared: &mut Shared) {
    let Session::Live(live) = &shared.session else {
        shared.popover = None;
        return;
    };
    let text = hud::cancel_text(&live.timer());
    let question = Confirm {
        title: hud::CANCEL_TITLE,
        text: &text,
        cancel: hud::KEEP_RECORDING,
        confirm: hud::DELETE_RECORDING,
        kind: ButtonKind::Danger,
    };
    if let Some(answer) = question.show(ui) {
        shared.answer_cancel(answer == Answer::Confirm);
    }
}

/// The stop did not finish: the error, «Копировать», «Открыть папку записи», «Закрыть». The
/// recording stays in its folder and is recovered on the next open of the project.
fn failed(ui: &mut Ui, shared: &mut Shared) {
    let Some((message, root)) = shared.failure.clone() else {
        shared.popover = None;
        return;
    };
    let panel = ErrorPanel {
        title: hud::PREPARE_FAILED,
        text: &message,
        extra: &[hud::OPEN_FOLDER],
    };
    match panel.show(ui) {
        Some(ErrorAction::Extra(_)) => shared.report(crate::open_folder(&root)),
        Some(ErrorAction::Close) => {
            shared.failure = None;
            shared.popover = None;
        }
        None => {}
    }
}

/// «Остановить и сохранить запись перед выходом?» (mockup 6).
fn quit(ui: &mut Ui, shared: &mut Shared) {
    let question = Confirm {
        title: tray::QUIT_TITLE,
        text: tray::QUIT_TEXT,
        cancel: tray::QUIT_CANCEL,
        confirm: tray::QUIT_SAVE,
        kind: ButtonKind::Primary,
    };
    match question.show(ui) {
        // The app stops and saves the recording on its way out.
        Some(Answer::Confirm) => ui
            .ctx()
            .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close),
        Some(Answer::Cancel) => shared.popover = None,
        None => {}
    }
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
        Pick::Area => shared.area_asked = Some(crate::area::AreaAsk::PickerClosing),
        Pick::Close => {}
        Pick::Source(source) => {
            let saved = shared.store.update(|s| s.recording.source = source);
            shared.report(saved);
            if let Source::Window(id) = source {
                shared.report(dc_capture::win::hud_window::bring_to_front(id));
            }
        }
    }
}

fn sound(ui: &mut Ui, shared: &mut Shared) {
    let mut recording = shared.store.get().recording.clone();
    let ctx = ui.ctx().clone();
    let meters = &shared.meters;
    super::menus::sound_menu(ui, &mut recording, &shared.microphones, &|source| {
        meters.level(&ctx, source)
    });
    let result = shared.store.update(|s| s.recording = recording);
    shared.report(result);
}

fn timing(ui: &mut Ui, shared: &mut Shared) {
    let mut next = shared.store.get().clone();
    super::menus::timing_menu(ui, &mut next);
    let result = shared.store.update(|s| *s = next);
    shared.report(result);
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
