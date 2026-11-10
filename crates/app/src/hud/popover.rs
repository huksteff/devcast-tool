//! Popovers of the HUD (decision 001: small windows of the same look): one at a time, opened
//! upwards 8 px above the bar under the button that opened it; Esc or a click elsewhere closes
//! it. Its transparent window sizes itself to the drawn panel and its shadow (decision 020).

use std::sync::{Arc, Mutex};

use dc_ui_kit::strings::{hud, notices, picker as picker_text, tray};
use dc_ui_kit::theme::WINDOW_SHADOW;
use dc_ui_kit::tokens::S3;
use dc_ui_kit::widgets::{
    Answer, Confirm, ErrorAction, ErrorPanel, Kind as ButtonKind, panel_frame,
};
use eframe::egui::{self, Margin, Rect, Ui, vec2};

use dc_capture::Source;

use super::menus::{MoreAction, more_menu};
use super::picker::Pick;
use super::place::{ceiling, placed};

use crate::app::{Shared, lock, other_window};
use crate::borderless;
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
    /// «DevCast в прошлый раз упал» at the start (8.5).
    Crashed,
    /// «Что нового» at the first start of a new version (8.3).
    WhatsNew,
    /// «Скачать?» about a newer version, its download and «Перезапустить» (8.2).
    Update,
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
            Self::Crashed => notices::CRASHED,
            Self::WhatsNew => notices::WHATS_NEW,
            Self::Update => notices::UPDATE,
        }
    }

    /// Room between the panel's border and its content.
    pub fn padding(self) -> Margin {
        match self {
            Self::Source => super::picker::PADDING,
            _ => Margin::same(S3 as i8),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Open {
    pub kind: Kind,
    /// Left edge of the button under it, points from the HUD window's left edge.
    pub anchor: f32,
    /// The window has had the focus. Until then losing it does not close the popover: Windows
    /// does not activate a window opened while another app is in front (a button pressed
    /// through UI Automation), and the popover closed on its first frame.
    focused: bool,
    /// Exclusion from capture as last set on the window.
    excluded: Option<bool>,
    /// The window is shown: it opens hidden and shows once it has the size and place of its
    /// content, so it never appears 40 px high and jumps.
    shown: bool,
}

impl Open {
    pub fn new(kind: Kind, anchor: f32) -> Self {
        Self {
            kind,
            anchor,
            focused: false,
            excluded: None,
            shown: false,
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

/// Highest a popover gets, points (mockup: the source picker scrolls past 620).
pub const MAX_HEIGHT: f32 = 620.0;

/// The popover's window; one at a time.
pub const ID: &str = "hud-popover";

/// Shows the open popover window above the HUD whose window rectangle is `bar`.
pub fn show(ctx: &egui::Context, shared: &Arc<Mutex<Shared>>, open: Open, bar: Rect) {
    let width = match open.kind {
        Kind::Source => super::picker::WIDTH,
        Kind::Sound => super::menus::sound_width(ctx, &lock(shared).microphones),
        Kind::Timing => super::menus::TIMING_WIDTH,
        Kind::More => 260.0,
        Kind::Cancel | Kind::Quit => 280.0,
        Kind::Failed | Kind::Crashed => 360.0,
        Kind::WhatsNew => 380.0,
        Kind::Update => 340.0,
    };
    let ceiling = ceiling(ctx, &lock(shared));
    let shared = Arc::clone(shared);
    let builder = builder(open, width, bar, ceiling);
    ctx.show_viewport_deferred(egui::ViewportId::from_hash_of(ID), builder, move |ui, _| {
        other_window(ui, &shared, |ui, shared| {
            frame(ui, shared, open, width, bar)
        });
    });
}

/// The window of `open` with a panel `width` wide (it starts 40 high, then fits).
pub fn builder(open: Open, width: f32, bar: Rect, ceiling: Option<f32>) -> egui::ViewportBuilder {
    let window = placed(open, vec2(width, 40.0), bar, ceiling);
    borderless::floating(open.kind.title())
        .with_visible(false)
        .with_resizable(false)
        .with_inner_size(window.size())
        .with_position(window.min)
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
    let margin = borderless::margin();
    let corner = ui.max_rect().min + vec2(margin.left.into(), margin.top.into());
    let room = Rect::from_min_size(corner, vec2(width, MAX_HEIGHT));
    let response = ui
        .scope_builder(egui::UiBuilder::new().max_rect(room), |ui| {
            let padding = open.kind.padding();
            panel_frame()
                .inner_margin(padding)
                .shadow(WINDOW_SHADOW)
                .show(ui, |ui| {
                    ui.set_width(width - padding.sum().x - 2.0);
                    match open.kind {
                        Kind::Source => source(ui, shared),
                        Kind::Sound => sound(ui, shared),
                        Kind::Timing => timing(ui, shared),
                        Kind::More => more(ui, shared),
                        Kind::Cancel => cancel(ui, shared),
                        Kind::Quit => quit(ui, shared),
                        Kind::Failed => failed(ui, shared),
                        Kind::Crashed => super::notices::crashed(ui, shared),
                        Kind::WhatsNew => super::notices::whats_new(ui, shared),
                        Kind::Update => super::update::ui(ui, shared),
                    }
                })
                .response
        })
        .inner;
    let window = placed(open, response.rect.size(), bar, ceiling(ui.ctx(), shared));
    let current = ui.input(|i| i.viewport().inner_rect.map(|r| r.size()));
    if current.is_some_and(|c| (c - window.size()).length() > 0.5) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::InnerSize(window.size()));
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::OuterPosition(window.min));
    } else if let Some(open) = shared.popover.as_mut().filter(|o| !o.shown) {
        open.shown = true;
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Visible(true));
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

/// The stop did not finish: the error, «Копировать», «Восстановить и открыть», «Открыть папку
/// записи», «Закрыть». The recording stays in its folder: opening its project finishes it from
/// what was written (`dc_ops::recovery`).
fn failed(ui: &mut Ui, shared: &mut Shared) {
    let Some((message, root)) = shared.failure.clone() else {
        shared.popover = None;
        return;
    };
    let panel = ErrorPanel {
        title: hud::PREPARE_FAILED,
        text: &message,
        extra: &[hud::RECOVER, hud::OPEN_FOLDER],
    };
    match panel.show(ui) {
        Some(ErrorAction::Extra(0)) => {
            shared.failure = None;
            shared.popover = None;
            shared.open_project(&root, false);
        }
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
    match previews.receive(ui.ctx()) {
        Some(Ok(list)) => *sources = list,
        Some(Err(error)) => tracing::warn!("список источников не обновлён: {error}"),
        None => {}
    }
    let textures = previews.textures();
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
    match shared.mic_refresh.take() {
        Some(Ok(names)) => shared.microphones = names,
        Some(Err(error)) => shared.report(Err(error)),
        None => {}
    }
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
