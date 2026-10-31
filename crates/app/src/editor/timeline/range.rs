//! The picked range of the output (mockup 2d): Shift+drag on the ruler picks it (without Shift
//! the ruler moves the playhead), it is painted over the ruler and the rows. A right click in it
//! opens its menu: its length, «Вырезать участок» (Shift+Delete: `cut` with ripple) and
//! «Скорость участка…» (R: `speed`), whose popover offers preset speeds and one's own.

use std::time::Duration;

use dc_core::time::Output;
use dc_core::{Range, TimeMap};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::{keys as key_names, timeline as text};
use dc_ui_kit::tokens::{FS_L, S1, S2, S3, TEXT, TEXT_3};
use dc_ui_kit::widgets::{Button, Kind, Slider, panel_frame, segmented};
use eframe::egui::{self, Align, Layout, Rect, Response, RichText, Ui, pos2};

use super::viewport::Lanes;
use crate::editor::view::RangeMenu;
use crate::editor::{EditorWindow, keys, montage};
use crate::keymap::EditorKey;

/// Width of the menu and of the popover, points (mockup: 250).
const WIDTH: f32 = 250.0;
/// Preset speeds of the popover (mockup).
const PRESETS: [f64; 6] = [0.5, 1.0, 2.0, 4.0, 8.0, 16.0];
/// Least speed, most speed and the step of one's own (fx/cut: 0.25–16).
const OWN: std::ops::RangeInclusive<f64> =
    dc_core::project::SPEED_MIN..=dc_core::project::SPEED_MAX;
const OWN_STEP: f64 = 0.25;
const OWN_SLIDER: f32 = 120.0;
/// Repaints while a picking drag presses an edge: the auto-scroll goes on without input.
const EDGE_TICK: Duration = Duration::from_millis(16);

/// Shift+drag on the ruler picks the range, a right click in it opens the menu. True while the
/// ruler picks: then it does not move the playhead.
pub fn ruler(ui: &Ui, on_ruler: &Response, lanes: &Lanes, editor: &mut EditorWindow) -> bool {
    let (pressed, shift, down, pointer, dt) = ui.input(|i| {
        let p = &i.pointer;
        let shift = i.modifiers.shift;
        (
            p.primary_pressed(),
            shift,
            p.primary_down(),
            p.interact_pos(),
            i.stable_dt,
        )
    });
    let end = editor.end();
    let view = &mut editor.view;
    let at = |view: &crate::editor::View, x: f32| {
        let x = x.clamp(lanes.left, lanes.right());
        view.timeline.scale(lanes).time(x).min(end)
    };
    if let (true, true, true, Some(p)) = (
        pressed,
        shift,
        on_ruler.is_pointer_button_down_on(),
        pointer,
    ) {
        view.ranging = Some(at(view, p.x));
        view.range = None;
        view.menu = None;
    }
    let Some(anchor) = view.ranging else {
        let inside = pointer.is_some_and(|p| view.range.is_some_and(|r| r.contains(at(view, p.x))));
        if on_ruler.secondary_clicked() && inside {
            view.menu = Some(RangeMenu::Actions);
        }
        return false;
    };
    if let Some(p) = pointer {
        if down && view.timeline.edge_scroll(lanes, p.x, dt) {
            ui.ctx().request_repaint_after(EDGE_TICK);
        }
        let t = at(view, p.x);
        view.range = Range::new(anchor.min(t), anchor.max(t))
            .ok()
            .filter(|r| !r.is_empty());
    }
    if !down {
        view.ranging = None;
    }
    true
}

/// The open menu or speed popover under the range start, below the `ruler`; a press outside
/// closes it.
pub fn menu(ui: &Ui, editor: &mut EditorWindow, lanes: &Lanes, ruler: Rect) {
    let (Some(menu), Some(range)) = (editor.view.menu, editor.view.range) else {
        editor.view.menu = None;
        return;
    };
    let scale = editor.view.timeline.scale(lanes);
    let most = (lanes.right() - WIDTH).max(lanes.left);
    let x = scale.x(range.start()).clamp(lanes.left, most);
    let mut next = Some(menu);
    let area = egui::Area::new(ui.scope_id().with("timeline-range-menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(x, ruler.bottom() + S1))
        .show(ui.ctx(), |ui| {
            panel_frame().show(ui, |ui| {
                ui.set_width(WIDTH - 2.0 * S3);
                ui.spacing_mut().item_spacing.y = S1;
                next = match menu {
                    RangeMenu::Actions => actions(ui, editor, range),
                    RangeMenu::Speed { own } => speed(ui, editor, range, own),
                };
            });
        });
    let outside = ui.input(|i| {
        let p = &i.pointer;
        p.any_pressed()
            && p.interact_pos()
                .is_some_and(|at| !area.response.rect.contains(at))
    });
    editor.view.menu = if outside { None } else { next };
}

/// The range menu; what stays open after this frame.
fn actions(ui: &mut Ui, editor: &mut EditorWindow, range: Range<Output>) -> Option<RangeMenu> {
    let (start, end) = (range.start().to_string(), range.end().to_string());
    let title = text::range(&start, &end, range.duration().as_secs_f64());
    ui.label(RichText::new(title).color(TEXT));
    ui.separator();
    let cut = Button::new(key_names::CUT_RANGE)
        .kind(Kind::Ghost)
        .small()
        .icon(Icon::Blade);
    let hint = keys::hint(key_names::CUT_RANGE, EditorKey::CutRange, &editor.keys);
    if ui.add(cut).on_hover_text(hint).clicked() {
        editor.view.range = None;
        editor.ask(montage::cut(range));
        return None;
    }
    let speed = Button::new(text::SPEED_MENU)
        .kind(Kind::Ghost)
        .small()
        .icon(Icon::Timer);
    let hint = keys::hint(text::SPEED_TITLE, EditorKey::RangeSpeed, &editor.keys);
    if ui.add(speed).on_hover_text(hint).clicked() {
        let own = shown(editor, range).1.unwrap_or(1.0);
        return Some(RangeMenu::Speed { own });
    }
    Some(RangeMenu::Actions)
}

/// The speed popover; a chosen speed applies to the range and closes it.
fn speed(
    ui: &mut Ui,
    editor: &mut EditorWindow,
    range: Range<Output>,
    mut own: f64,
) -> Option<RangeMenu> {
    let (source, now) = shown(editor, range);
    ui.label(RichText::new(text::SPEED_TITLE).color(TEXT).size(FS_L));
    let names: Vec<String> = PRESETS.iter().map(|f| text::times(*f)).collect();
    let options: Vec<(f64, &str, bool)> = PRESETS
        .iter()
        .zip(&names)
        .map(|(f, name)| (*f, name.as_str(), true))
        .collect();
    let mut chosen = now.unwrap_or(0.0);
    let mut apply = segmented(ui, &mut chosen, &options)
        .changed()
        .then_some(chosen);
    ui.add_space(S2);
    ui.label(RichText::new(text::OWN_SPEED).color(TEXT_3));
    Slider::new(&mut own, OWN, text::OWN_SPEED)
        .width(OWN_SLIDER)
        .step(OWN_STEP)
        .suffix(text::TIMES)
        .show(ui);
    let change = text::speed_change(range.duration().as_secs_f64(), source / own);
    let note = format!("{} {change}", text::PITCH_KEPT);
    ui.label(RichText::new(note).color(TEXT_3));
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        if ui.add(Button::new(text::APPLY).small()).clicked() {
            apply = Some(own);
        }
    });
    if let Some(factor) = apply {
        editor.view.range = None;
        editor.ask(montage::speed(range, factor));
        return None;
    }
    Some(RangeMenu::Speed { own })
}

/// Seconds of the recordings the range shows, and their speed when it is one.
fn shown(editor: &EditorWindow, range: Range<Output>) -> (f64, Option<f64>) {
    let map = TimeMap::build(editor.doc.project());
    let mut source = 0.0;
    let mut speeds: Vec<f64> = Vec::new();
    for segment in map.segments() {
        let from = segment.output_start.max(range.start());
        let to = segment.output_end().min(range.end());
        if to <= from {
            continue;
        }
        source += to.since(from).as_secs_f64() * segment.speed;
        speeds.push(segment.speed);
    }
    let first = speeds.first().copied();
    let one = first.filter(|f| speeds.iter().all(|s| s == f));
    (source, one)
}

/// The picked range over the ruler and the rows down to `bottom`.
pub fn paint(ui: &Ui, editor: &EditorWindow, lanes: &Lanes, ruler: Rect, bottom: f32) {
    let Some(range) = editor.view.range else {
        return;
    };
    let clip = Rect::from_x_y_ranges(lanes.left..=ruler.right(), ruler.top()..=bottom);
    let (left, right) = editor.view.timeline.scale(lanes).span(range);
    let painter = ui.painter().with_clip_rect(clip);
    super::paint::range(&painter, left, right, ruler.top()..=bottom);
}
