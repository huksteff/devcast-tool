//! Parameter rows of a panel (mockup `.prow`): a 104 px caption and the control after it, laid out
//! by `settings_rows` like the settings window. Each helper returns the new value once the edit
//! is committed, and the panel turns it into one registry command — one undo step: a click or
//! a typed number (on Enter or when the field loses focus) at once, a slider or a colour dragged
//! with the pointer when it is let go. Meanwhile the control shows the dragged value (a draft in
//! egui's temporary memory) and the project stays as it was.

use crate::tokens::{FS_S, S2, TEXT_3};
use crate::widgets::{
    Slider, Toggle, anchors as anchor_grid, color_button, gradient_editor, palette as swatches,
    segmented, text_area as text_area_widget, text_field, tiles as tile_row,
};
use dc_core::Step;
use dc_render::Rgba;
use egui::{self, Color32, Id, Painter, Rect, Response, RichText, Ui};

use crate::settings_rows;

/// Width of the caption column, least height of a row, points (mockup `.prow`: 96, but Inter
/// here is wider and «Буквы · плашка» would wrap).
const LABEL: f32 = 104.0;
const ROW: f32 = 30.0;

/// A caption and its control; the control's answer.
pub fn row<T>(ui: &mut Ui, label: &str, control: impl FnOnce(&mut Ui) -> T) -> T {
    settings_rows::labelled(ui, label, (LABEL, S2, ROW), control)
}

/// The edited value of the control `show` draws from `now`: a draft while the pointer is down
/// or a text field has the keys, the value once the edit ends (`None` when it did not change).
fn drafted<T>(
    ui: &mut Ui,
    id: Id,
    now: T,
    show: impl FnOnce(&mut Ui, &mut T) -> Response,
) -> Option<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    let draft = ui.data(|d| d.get_temp::<T>(id));
    let drafting = draft.is_some();
    let mut value = draft.unwrap_or_else(|| now.clone());
    let response = show(ui, &mut value);
    if !response.changed() && !drafting {
        return None;
    }
    let held = ui.input(|i| i.pointer.any_down()) || ui.ctx().text_edit_focused();
    if held {
        ui.data_mut(|d| d.insert_temp(id, value));
        return None;
    }
    ui.data_mut(|d| d.remove::<T>(id));
    (value != now).then_some(value)
}

/// An on/off switch.
pub fn toggle(ui: &mut Ui, label: &str, on: bool) -> Option<bool> {
    row(ui, label, |ui| {
        let mut value = on;
        ui.add(Toggle::new(&mut value, label))
            .changed()
            .then_some(value)
    })
}

/// A slider over `range` in steps of `step` with its number field and `suffix` (« с», «×»).
pub fn slider(
    ui: &mut Ui,
    label: &str,
    value: f64,
    range: std::ops::RangeInclusive<f64>,
    (step, suffix): (f64, &str),
) -> Option<f64> {
    row(ui, label, |ui| {
        let width = ui.available_width() - Slider::FIELD - S2;
        let id = ui.make_persistent_id(("draft", label));
        drafted(ui, id, value, |ui, v| {
            Slider::new(v, range, label)
                .width(width)
                .step(step)
                .suffix(suffix)
                .show(ui)
        })
    })
}

/// Swatches of `colors` (accessible name, colour) in a row; the index and the new colour of the
/// one changed.
pub fn colors(ui: &mut Ui, label: &str, colors: &[(&str, Rgba)]) -> Option<(usize, Rgba)> {
    row(ui, label, |ui| {
        let mut changed = None;
        for (i, (name, color)) in colors.iter().enumerate() {
            let id = ui.make_persistent_id(("draft", *name));
            let now = color32(*color);
            if let Some(new) = drafted(ui, id, now, |ui, c| color_button(ui, c, name)) {
                changed = Some((i, rgba(new)));
            }
        }
        changed
    })
}

/// One of `options` (value, caption) in a segmented row; `value` none of them when the project
/// has something else (custom zoom durations are no preset). The option clicked.
pub fn choice<T: PartialEq + Copy>(
    ui: &mut Ui,
    label: &str,
    value: Option<T>,
    options: &[(T, &str)],
) -> Option<T> {
    row(ui, label, |ui| {
        let mut chosen = value;
        let options: Vec<(Option<T>, &str, bool)> = options
            .iter()
            .map(|(v, name)| (Some(*v), *name, true))
            .collect();
        segmented(ui, &mut chosen, &options)
            .changed()
            .then_some(chosen)
            .flatten()
    })
}

/// A place on the anchor grid, three in a row: the index into `places`.
pub fn anchors(ui: &mut Ui, label: &str, value: usize, places: &[&str]) -> Option<usize> {
    row(ui, label, |ui| {
        let mut chosen = value;
        anchor_grid(ui, &mut chosen, 3, places)
            .changed()
            .then_some(chosen)
    })
}

/// A grey line of explanation under the rows (mockup `.hint`).
pub fn note(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(FS_S).color(TEXT_3)).wrap());
}

fn color32(Rgba([r, g, b, a]): Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn rgba(color: Color32) -> Rgba {
    Rgba(color.to_srgba_unmultiplied())
}

/// An on/off switch with a grey hint after it (mockup: «Зациклить» · «последние 0,67 с»).
pub fn toggle_hint(ui: &mut Ui, label: &str, on: bool, hint: &str) -> Option<bool> {
    row(ui, label, |ui| {
        let mut value = on;
        let changed = ui.add(Toggle::new(&mut value, label)).changed();
        ui.label(RichText::new(hint).size(FS_S).color(TEXT_3));
        changed.then_some(value)
    })
}

/// A palette of `colors` (colour, accessible name) and an own colour named `own`: the colour
/// chosen, or the own colour once its picker is let go.
pub fn palette(
    ui: &mut Ui,
    label: &str,
    now: Rgba,
    colors: &[(Rgba, &str)],
    own: &str,
) -> Option<Rgba> {
    row(ui, label, |ui| {
        let colors: Vec<(Color32, &str)> = colors.iter().map(|(c, n)| (color32(*c), *n)).collect();
        let id = ui.make_persistent_id(("draft", label));
        drafted(ui, id, color32(now), |ui, c| swatches(ui, c, &colors, own)).map(rgba)
    })
}

/// Tiles of `options` (value, caption) over the whole width, `picture` draws a value's picture;
/// the option clicked.
pub fn tiles<T: PartialEq + Copy>(
    ui: &mut Ui,
    value: T,
    options: &[(T, &str)],
    picture: impl Fn(&Painter, Rect, T),
) -> Option<T> {
    let mut chosen = value;
    tile_row(ui, &mut chosen, options, picture)
        .changed()
        .then_some(chosen)
}

/// [`choice`] whose options are wider than the place after a caption («Точно · Focused ·
/// Smooth»): the caption on its own line, the options under it.
pub fn choice_under<T: PartialEq + Copy>(
    ui: &mut Ui,
    label: &str,
    value: Option<T>,
    options: &[(T, &str)],
) -> Option<T> {
    caption(ui, label);
    let mut chosen = value;
    let options: Vec<(Option<T>, &str, bool)> = options
        .iter()
        .map(|(v, name)| (Some(*v), *name, true))
        .collect();
    segmented(ui, &mut chosen, &options)
        .changed()
        .then_some(chosen)
        .flatten()
}

/// A typed text not applied yet: the command that applies it and the pass its field was drawn.
#[derive(Debug, Clone)]
struct Pending {
    /// The draft of the field.
    field: Id,
    step: Step,
    pass: u64,
}

fn pending_id() -> Id {
    Id::unique("inspector-typed-text")
}

/// A text field over the whole width named by `hint`, showing `now` of the object `key`: the
/// command `apply` makes of the typed text once the field lets the keys go (Enter, a click
/// away) and the text differs. While typing the text is a draft; picking another object
/// applies it too ([`flush`]); Esc drops it.
pub fn text(
    ui: &mut Ui,
    hint: &str,
    key: impl std::hash::Hash + std::fmt::Debug,
    now: &str,
    apply: impl Fn(String) -> Step,
) -> Option<Step> {
    typed(ui, (hint, key), now, apply, text_field)
}

/// [`text`] of several lines (a text annotation): Enter starts a new line, a click away applies.
pub fn text_area(
    ui: &mut Ui,
    hint: &str,
    key: impl std::hash::Hash + std::fmt::Debug,
    now: &str,
    apply: impl Fn(String) -> Step,
) -> Option<Step> {
    typed(ui, (hint, key), now, apply, |ui, value, hint, width| {
        text_area_widget(ui, value, hint, width, 2)
    })
}

fn typed(
    ui: &mut Ui,
    (hint, key): (&str, impl std::hash::Hash + std::fmt::Debug),
    now: &str,
    apply: impl Fn(String) -> Step,
    field: impl FnOnce(&mut Ui, &mut String, &str, f32) -> Response,
) -> Option<Step> {
    let id = ui.make_persistent_id(("draft-text", &key));
    let mut value = ui
        .data(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| now.to_owned());
    let width = ui.available_width();
    let response = ui
        .push_id(&key, |ui| field(ui, &mut value, hint, width))
        .inner;
    let asked = Focus(Id::unique(&key));
    if ui.data(|d| d.get_temp::<Focus>(focus_id())) == Some(asked) {
        ui.data_mut(|d| d.remove::<Focus>(focus_id()));
        select_all(ui, &response, &value);
    }
    if response.has_focus() {
        let pass = ui.ctx().cumulative_pass_nr();
        let pending = (value != now).then(|| Pending {
            field: id,
            step: apply(value.clone()),
            pass,
        });
        ui.data_mut(|d| {
            d.insert_temp(id, value);
            if let Some(pending) = pending {
                d.insert_temp(pending_id(), pending);
            } else {
                d.remove::<Pending>(pending_id());
            }
        });
        return None;
    }
    ui.data_mut(|d| {
        d.remove::<String>(id);
        if d.get_temp::<Pending>(pending_id())
            .is_some_and(|p| p.field == id)
        {
            d.remove::<Pending>(pending_id());
        }
    });
    let dropped = ui.input(|i| i.key_pressed(egui::Key::Escape));
    (response.lost_focus() && !dropped && value != now).then(|| apply(value))
}

/// The text field asked to take the keys, by the key of its object.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Focus(Id);

fn focus_id() -> Id {
    Id::unique("inspector-focus-text")
}

/// The field of [`text`] or [`text_area`] of the object `key` takes the keys with its whole
/// text selected when it is drawn next, so typing replaces the text (a double-click on a text
/// annotation of the preview).
pub fn focus_text(ctx: &egui::Context, key: impl std::hash::Hash + std::fmt::Debug) {
    ctx.data_mut(|d| d.insert_temp(focus_id(), Focus(Id::unique(key))));
}

/// The field of `response` takes the keys with all of `text` selected.
fn select_all(ui: &Ui, response: &Response, text: &str) {
    response.request_focus();
    let mut state = egui::text_edit::TextEditState::load(ui.ctx(), response.id).unwrap_or_default();
    let all = egui::text::CCursorRange::select_all_str(text);
    state.cursor.set_char_range(Some(all));
    state.store(ui.ctx(), response.id);
}

/// The text typed into a field that is gone this pass (another object was picked): its
/// command, once.
pub fn flush(ui: &Ui) -> Option<Step> {
    let pass = ui.ctx().cumulative_pass_nr();
    let pending = ui.data(|d| d.get_temp::<Pending>(pending_id()))?;
    if pending.pass == pass {
        return None;
    }
    ui.data_mut(|d| {
        d.remove::<Pending>(pending_id());
        d.remove::<String>(pending.field);
    });
    Some(pending.step)
}
/// The gradient editor of `colors` over the whole width: the new colours once the edit ends.
pub fn gradient(ui: &mut Ui, label: &str, colors: &[Rgba]) -> Option<Vec<Rgba>> {
    let id = ui.make_persistent_id(("draft", label));
    let now: Vec<Color32> = colors.iter().map(|c| color32(*c)).collect();
    let width = ui.available_width();
    drafted(ui, id, now, |ui, c| gradient_editor(ui, c, width))
        .map(|new| new.into_iter().map(rgba).collect())
}

/// A caption on a line of its own, for controls too wide for the place after it.
pub fn caption(ui: &mut Ui, label: &str) {
    let width = ui.available_width() - S2;
    settings_rows::labelled(ui, label, (width, S2, ROW), |_| ());
}
