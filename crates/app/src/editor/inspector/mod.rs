//! The 340 px inspector (mockup «3. Панели эффектов»): a rail of tabs on the left — the global
//! settings of an effect — and the open panel. Picking one object on the timeline opens its
//! panel; a tab opens the tab's panel until another object is picked.
//!
//! A panel is a function over [`Cx`]: it draws its [`header`], then its groups with
//! [`Cx::group`] (open or closed is remembered per panel in the app settings; «Сбросить» ↺ brings
//! the group's fields back to the defaults with one registry command), and its rows with the
//! helpers of `rows.rs`; every committed edit is a registry command — one undo step. A tab gets
//! its panel in [`tab`], an object kind in `object.rs`.

mod click;
mod click_style;
mod cursor;
mod keys;
mod object;
mod rows;
mod zoom;
mod zoom_region;

#[cfg(test)]
mod cursor_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod zoom_tests;

use dc_core::{Effect, Project, StateError};
use dc_ops::Step;
use dc_project::Defaults;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::{editor as text, inspector as names};
use dc_ui_kit::tokens::{
    self, BG, BORDER, FS_L, FS_M, FS_S, Hue, S2, S3, SURFACE, TEXT, TEXT_2, TEXT_3,
};
use dc_ui_kit::widgets::IconButton;
use eframe::egui::{
    self, Align, Frame, Layout, Margin, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder, pos2, vec2,
};
use serde_json::{Map, Value};

use super::view::{Panel, View};
use super::{Action, EditorWindow};
use crate::settings::InspectorGroups;

/// Width of the tab rail, side of a tab, height of the panel and group headers (mockup).
const RAIL: f32 = 44.0;
const TAB: f32 = 36.0;
const PANEL_HEADER: f32 = 44.0;
const GROUP_HEADER: f32 = 34.0;
/// Side of a header button (reset, delete) and of the group chevron.
const BUTTON: f32 = 26.0;
const CHEVRON: f32 = 14.0;

const PANELS: [Panel; 5] = [
    Panel::Zoom,
    Panel::Cursor,
    Panel::Keys,
    Panel::Captions,
    Panel::Sound,
];

/// A panel: draws into the inspector and asks for actions through [`Cx`].
type PanelUi = fn(&mut Ui, &mut Cx<'_>);

/// The panel of a rail tab with its id (the first part of its groups' keys), or the task that
/// brings it (its tab is off until then).
fn tab(panel: Panel) -> Result<(&'static str, PanelUi), &'static str> {
    match panel {
        Panel::Keys => Ok((keys::ID, keys::ui)),
        Panel::Zoom => Ok((zoom::ID, zoom::ui)),
        Panel::Cursor => Ok((cursor::ID, cursor::ui)),
        Panel::Captions => Err("7.17"),
        Panel::Sound => Err("7.19"),
    }
}

impl Panel {
    fn icon(self) -> Icon {
        match self {
            Self::Zoom => Icon::Frame,
            Self::Cursor => Icon::Cursor,
            Self::Keys => Icon::Keyboard,
            Self::Captions => Icon::Captions,
            Self::Sound => Icon::Speaker,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Zoom => text::ZOOM,
            Self::Cursor => text::CURSOR,
            Self::Keys => text::KEYS,
            Self::Captions => text::CAPTIONS,
            Self::Sound => text::SOUND,
        }
    }

    fn hue(self) -> Hue {
        match self {
            Self::Zoom => tokens::HUE_ZOOM,
            Self::Cursor => tokens::HUE_CURSOR,
            Self::Keys => tokens::HUE_KEYS,
            Self::Captions => tokens::HUE_CAPTIONS,
            Self::Sound => tokens::HUE_AUDIO,
        }
    }
}

/// What a panel works with this frame.
pub struct Cx<'a> {
    pub project: &'a Project,
    /// The settings new projects start with: «Сбросить» brings a group back to them.
    defaults: &'a Defaults,
    /// Groups the user opened or closed; `toggled` when one changed this frame.
    groups: &'a mut InspectorGroups,
    toggled: bool,
    /// Id of the open panel: the first part of its groups' keys (`keys.fine`).
    panel: &'static str,
    /// What the panel asked for; the editor runs it after the layout.
    pub actions: Vec<Action>,
}

impl Cx<'_> {
    /// The default state of effect `E`: its settings from the defaults, the rest factory.
    pub fn defaults_of<E: Effect>(&self) -> Result<E::State, StateError> {
        let mut project = Project::default();
        self.defaults.apply(&mut project);
        project.state::<E>()
    }

    /// A group: its header (chevron, title, «Сбросить») and, when open, `body`.
    pub fn group(&mut self, ui: &mut Ui, group: Group<'_>, body: impl FnOnce(&mut Ui, &mut Self)) {
        let key = format!("{}.{}", self.panel, group.id);
        let open = self.groups.0.get(&key).copied().unwrap_or(group.open);
        let rect = ui
            .allocate_exact_size(vec2(ui.available_width(), GROUP_HEADER), Sense::hover())
            .0;
        if group_header(ui, rect, group.title, open) {
            if open == group.open {
                self.groups.0.insert(key, !open);
            } else {
                self.groups.0.remove(&key);
            }
            self.toggled = true;
        }
        if let Some(reset) = group.reset {
            let button = Rect::from_center_size(
                pos2(rect.right() - 6.0 - BUTTON / 2.0, rect.center().y),
                vec2(BUTTON, BUTTON),
            );
            let mut cell = ui.new_child(UiBuilder::new().max_rect(button));
            let label = names::reset_group(group.title);
            let icon = IconButton::new(Icon::Reset, &label).size(BUTTON, CHEVRON);
            let clicked = cell
                .add_enabled(!reset.is_empty(), icon)
                .on_hover_text(text::RESET)
                .clicked();
            if clicked {
                self.actions.push(Action::Apply(reset));
            }
        }
        let mut bottom = rect;
        if open {
            Frame::new()
                .inner_margin(Margin {
                    left: S3 as i8,
                    right: S3 as i8,
                    top: 2,
                    bottom: S3 as i8,
                })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    body(ui, self);
                });
            bottom = bottom.with_max_y(ui.min_rect().bottom());
        }
        bottom_line(ui, bottom);
    }
}

/// A group of a panel.
pub struct Group<'a> {
    /// Unique in the panel: the second part of the remembered key.
    pub id: &'static str,
    pub title: &'a str,
    /// Open until the user closes it.
    pub open: bool,
    /// The commands that bring the group's fields to the defaults: empty when they are the
    /// defaults already (the button is off), `None` for a group without «Сбросить».
    pub reset: Option<Vec<Step>>,
}

/// The head of a panel: the colour of its effect, its name, what it applies to.
pub struct Header<'a> {
    pub hue: Hue,
    pub title: &'a str,
    pub subtitle: &'a str,
}

/// Rail and panel; true when the user opened or closed a group (the settings keep it).
pub fn ui(ui: &mut Ui, editor: &mut EditorWindow) -> bool {
    follow(&mut editor.view);
    egui::Panel::left("inspector-rail")
        .exact_size(RAIL)
        .frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(4, 6)))
        .show(ui, |ui| rail(ui, &mut editor.view));
    let mut cx = Cx {
        project: editor.doc.project(),
        defaults: &editor.defaults,
        groups: &mut editor.groups,
        toggled: false,
        panel: "",
        actions: Vec::new(),
    };
    let view = &editor.view;
    let tracks = &editor.tracks;
    egui::CentralPanel::no_frame()
        .frame(Frame::new().fill(SURFACE))
        .show(ui, |ui| {
            ScrollArea::vertical().show(ui, |ui| match view.object {
                Some(picked) => object::ui(ui, &mut cx, tracks, picked),
                None => match tab(view.panel) {
                    Ok((id, panel)) => {
                        cx.panel = id;
                        panel(ui, &mut cx);
                    }
                    Err(task) => later(ui, view.panel, task),
                },
            });
        });
    let toggled = cx.toggled;
    let actions = cx.actions;
    editor.actions.extend(actions);
    toggled
}

/// A newly picked single object opens its panel; no single object goes back to the tab.
fn follow(view: &mut View) {
    let single = view.selection.single();
    if single != view.followed {
        view.followed = single;
        view.object = single;
    }
}

fn rail(ui: &mut Ui, view: &mut View) {
    ui.spacing_mut().item_spacing.y = 2.0;
    for panel in PANELS {
        let ready = tab(panel);
        let on = view.object.is_none() && view.panel == panel;
        let button = IconButton::new(panel.icon(), panel.title())
            .size(TAB, tokens::ICON)
            .on(on);
        let response = ui.add_enabled(ready.is_ok(), button);
        let response = match ready {
            Ok(_) => response.on_hover_text(panel.title()),
            Err(task) => response.on_disabled_hover_text(names::tab_later(panel.title(), task)),
        };
        if response.clicked() {
            view.panel = panel;
            view.object = None;
        }
    }
}

/// The colour chip, the title and the subtitle; `buttons` lays out its buttons from the right.
pub fn header(ui: &mut Ui, head: &Header<'_>, buttons: impl FnOnce(&mut Ui)) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), PANEL_HEADER), Sense::hover());
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::Heading, true, head.title);
        info.current_text_value = Some(head.subtitle.to_owned());
        info
    });
    let painter = ui.painter();
    let chip = Rect::from_center_size(
        pos2(rect.left() + S3 + 2.0, rect.center().y),
        vec2(4.0, 22.0),
    );
    painter.rect_filled(chip, 2, head.hue.line);
    let title = painter.layout_no_wrap(
        head.title.to_owned(),
        fonts::text(FS_L, Weight::Semibold),
        TEXT,
    );
    let mut height = title.size().y;
    let subtitle = (!head.subtitle.is_empty()).then(|| {
        let galley = painter.layout_no_wrap(
            head.subtitle.to_owned(),
            fonts::text(FS_S, Weight::Regular),
            TEXT_3,
        );
        height += galley.size().y;
        galley
    });
    let mut at = pos2(chip.right() + S2, rect.center().y - height / 2.0);
    let line = title.size().y;
    painter.galley(at, title, TEXT);
    if let Some(subtitle) = subtitle {
        at.y += line;
        painter.galley(at, subtitle, TEXT_3);
    }
    let right = rect.shrink2(vec2(S2, 0.0));
    let mut cell = ui.new_child(
        UiBuilder::new()
            .max_rect(right)
            .layout(Layout::right_to_left(Align::Center)),
    );
    cell.spacing_mut().item_spacing.x = 2.0;
    buttons(&mut cell);
    bottom_line(ui, rect);
}

/// A header button of [`header`] (delete, duplicate); `danger` draws it red (mockup `.ib.danger`).
pub fn header_button(ui: &mut Ui, icon: Icon, label: &str, danger: bool) -> bool {
    let mut button = IconButton::new(icon, label).size(BUTTON, CHEVRON);
    if danger {
        button = button.color(tokens::REC);
    }
    ui.add(button).on_hover_text(label).clicked()
}

/// The header of a tab whose panel comes with `task`.
fn later(ui: &mut Ui, panel: Panel, task: &str) {
    let head = Header {
        hue: panel.hue(),
        title: panel.title(),
        subtitle: names::WHOLE_PROJECT,
    };
    header(ui, &head, |_| {});
    padded(ui, |ui| rows::note(ui, &names::later(task)));
}

/// A group header: true when clicked (it opens or closes the group).
fn group_header(ui: &mut Ui, rect: Rect, title: &str, open: bool) -> bool {
    let response = ui.interact(
        rect,
        ui.make_persistent_id(("group", title)),
        Sense::click(),
    );
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, open, title));
    let chevron = Rect::from_center_size(
        pos2(rect.left() + 10.0 + CHEVRON / 2.0, rect.center().y),
        vec2(CHEVRON, CHEVRON),
    );
    let icon = if open { Icon::Down } else { Icon::Right };
    icon.paint(ui.painter(), chevron, TEXT_3);
    let (weight, color) = if open {
        (Weight::Semibold, TEXT)
    } else {
        (Weight::Medium, TEXT_2)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), fonts::text(FS_M, weight), color);
    let at = pos2(
        chevron.right() + 6.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, color);
    response.clicked()
}

/// Content with the side margins of a group body (mockup `.gb`).
fn padded(ui: &mut Ui, body: impl FnOnce(&mut Ui)) {
    Frame::new()
        .inner_margin(Margin::symmetric(S3 as i8, S2 as i8 + 2))
        .show(ui, body);
}

fn bottom_line(ui: &Ui, rect: Rect) {
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, BORDER));
}

/// Arguments of a command from its fields (`("show", true)` is `--show true`).
pub fn args<const N: usize>(fields: [(&str, Value); N]) -> Map<String, Value> {
    fields
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
}

/// `op` with the default fields of a group; nothing when the fields are the defaults already.
pub fn reset(op: &str, now: &Map<String, Value>, default: Map<String, Value>) -> Option<Step> {
    (*now != default).then(|| Step::new(op, default))
}
