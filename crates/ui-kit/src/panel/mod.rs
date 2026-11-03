//! The kit of an inspector panel (mockup «3. Панели эффектов»): what a panel of an effect is
//! drawn with and what it works with, so the panel lives in its effect's crate and the editor
//! only places it (docs/ARCHITECTURE.md, «Панель эффекта»).
//!
//! A panel is a function over [`Cx`]: it draws its [`header`], then its groups with
//! [`Cx::group`] (open or closed is remembered per panel by [`Groups`]; «Сбросить» ↺ brings the
//! group's fields back to the defaults with one registry command), and its rows with the
//! helpers of [`rows`]. Every committed edit is a registry [`Step`] list in [`Cx::edits`] — one
//! undo step each; the editor runs them after the layout, as `devcast` would. A list that adds
//! objects the editor picks afterwards goes to [`Cx::adds`].

mod head;
pub mod rows;

use std::collections::BTreeMap;

use dc_core::{Effect, OutputTime, Project, StateError, Step};
use egui::{Frame, Margin, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use serde_json::{Map, Value};

use crate::fonts::{self, Weight};
use crate::icons::Icon;
use crate::strings::{editor as text, inspector as names};
use crate::tokens::{BORDER, FS_M, S2, S3, TEXT, TEXT_2, TEXT_3};
use crate::widgets::IconButton;
pub use head::{Header, header, header_button};

/// Height of the panel and group headers, points (mockup).
pub const PANEL_HEADER: f32 = 44.0;
pub const GROUP_HEADER: f32 = 34.0;
/// Side of a header button (reset, delete) and of the group chevron, points.
pub const BUTTON: f32 = 26.0;
pub const CHEVRON: f32 = 14.0;

/// What a panel works with this frame; the editor fills it.
pub struct Cx<'a> {
    pub project: &'a Project,
    /// The project new projects start as (the user's defaults on a factory project):
    /// «Сбросить» brings a group back to its settings.
    pub defaults: &'a Project,
    /// Where the playhead stands: an annotation edit goes to its key there (7.18).
    pub playhead: OutputTime,
    /// «Авто-ключи»: an annotation edit makes a key at the playhead (7.25).
    pub auto_keys: bool,
    /// Families of the font pickers: built-in and the project's own.
    pub fonts: &'a [String],
    /// Groups the user opened or closed.
    pub groups: Groups<'a>,
    /// What the panel asked for: each list is one registry change, one undo step.
    pub edits: Vec<Vec<Step>>,
    /// Registry changes that add objects (one undo step each); the editor picks what they
    /// added («Подобрать зумы по кликам»).
    pub adds: Vec<Vec<Step>>,
}

impl Cx<'_> {
    /// The default state of effect `E`: its settings from the defaults, the rest factory.
    pub fn defaults_of<E: Effect>(&self) -> Result<E::State, StateError> {
        self.defaults.state::<E>()
    }

    /// `op` with `args` (`("show", true)` is `--show true`) as one undo step.
    pub fn apply(&mut self, op: &str, args: impl IntoIterator<Item = (&'static str, Value)>) {
        let args = args.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
        self.edits.push(vec![Step::new(op, args)]);
    }

    /// A group: its header (chevron, title, «Сбросить») and, when open, `body`.
    pub fn group(&mut self, ui: &mut Ui, group: Group<'_>, body: impl FnOnce(&mut Ui, &mut Self)) {
        self.group_with(ui, group, |_, _, _| {}, body);
    }

    /// [`Cx::group`] whose header has one more button: `buttons` gets its place left of
    /// «Сбросить», one [`BUTTON`] wide (the mute of a sound track).
    pub fn group_with(
        &mut self,
        ui: &mut Ui,
        group: Group<'_>,
        buttons: impl FnOnce(&mut Ui, &mut Self, Rect),
        body: impl FnOnce(&mut Ui, &mut Self),
    ) {
        let mut head = self.groups.head(ui, group);
        if let Some(reset) = head.reset.take() {
            self.edits.push(reset);
        }
        buttons(ui, self, head.more);
        head.body(ui, |ui| body(ui, self));
    }
}

/// The open and closed groups of the panels, remembered by the editor (`panel.group` → open);
/// a group the user left as it starts is not kept.
pub struct Groups<'a> {
    open: &'a mut BTreeMap<String, bool>,
    /// Id of the open panel: the first part of its groups' keys (`keys.fine`).
    panel: &'static str,
    toggled: bool,
}

impl<'a> Groups<'a> {
    pub fn new(open: &'a mut BTreeMap<String, bool>) -> Self {
        Self {
            open,
            panel: "",
            toggled: false,
        }
    }

    /// The groups drawn from now on belong to panel `id`.
    pub fn panel(&mut self, id: &'static str) {
        self.panel = id;
    }

    /// True when the user opened or closed a group (the editor keeps it in the settings).
    pub fn toggled(&self) -> bool {
        self.toggled
    }

    /// The header of `group`: what was clicked on it and where its body goes.
    pub fn head(&mut self, ui: &mut Ui, group: Group<'_>) -> GroupHead {
        let key = format!("{}.{}", self.panel, group.id);
        let open = self.open.get(&key).copied().unwrap_or(group.open);
        let rect = ui
            .allocate_exact_size(vec2(ui.available_width(), GROUP_HEADER), Sense::hover())
            .0;
        if group_header(ui, rect, group.title, open) {
            if open == group.open {
                self.open.insert(key, !open);
            } else {
                self.open.remove(&key);
            }
            self.toggled = true;
        }
        let reset = group.reset.and_then(|reset| {
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
            clicked.then_some(reset)
        });
        let more = Rect::from_center_size(
            pos2(rect.right() - 8.0 - BUTTON * 1.5, rect.center().y),
            vec2(BUTTON, BUTTON),
        );
        GroupHead {
            rect,
            open,
            reset,
            more,
        }
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

/// A drawn group header.
pub struct GroupHead {
    rect: Rect,
    pub open: bool,
    /// The reset commands when «Сбросить» was clicked.
    pub reset: Option<Vec<Step>>,
    /// The place of one more header button, left of «Сбросить» (the mute of a sound track).
    pub more: Rect,
}

impl GroupHead {
    /// The body of the group when it is open, and the line under the group.
    pub fn body(self, ui: &mut Ui, body: impl FnOnce(&mut Ui)) {
        let mut bottom = self.rect;
        if self.open {
            Frame::new()
                .inner_margin(Margin {
                    left: S3 as i8,
                    right: S3 as i8,
                    top: 2,
                    bottom: S3 as i8,
                })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    body(ui);
                });
            bottom = bottom.with_max_y(ui.min_rect().bottom());
        }
        bottom_line(ui, bottom);
    }
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
pub fn padded(ui: &mut Ui, body: impl FnOnce(&mut Ui)) {
    Frame::new()
        .inner_margin(Margin::symmetric(S3 as i8, S2 as i8 + 2))
        .show(ui, body);
}

/// The thin line under a header or a group.
pub fn bottom_line(ui: &Ui, rect: Rect) {
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

/// `t` as a time argument of a command: `12.500000` (seconds with microseconds, not below 0).
pub fn time_arg(t: OutputTime) -> String {
    let micros = t.micros().max(0);
    format!("{}.{:06}", micros / 1_000_000, micros % 1_000_000)
}

/// `op` with the default fields of a group; nothing when the fields are the defaults already.
pub fn reset(op: &str, now: &Map<String, Value>, default: Map<String, Value>) -> Option<Step> {
    (*now != default).then(|| Step::new(op, default))
}
