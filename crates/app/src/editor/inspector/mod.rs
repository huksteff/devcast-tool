//! The 340 px inspector (mockup «3. Панели эффектов»): a rail of tabs on the left — the global
//! settings of an effect — and the open panel. Picking one object on the timeline opens its
//! panel; a tab opens the tab's panel until another object is picked.
//!
//! A panel draws with the kit of `dc_ui_kit::panel` (header, groups, rows) over its
//! `panel::Cx`. A panel that lives in its effect's crate (docs/ARCHITECTURE.md, «Панель
//! эффекта») gets [`Cx::base`] and the facts of the timeline it needs; one not moved yet works
//! with the whole [`Cx`]. Every committed edit is a registry command — one undo step. A tab gets
//! its panel in [`tab`], an object kind in `object.rs`.

mod annot_blur;
mod annot_drawing;
mod annot_image;
mod annot_shape;
mod annot_text;
mod annotation;
mod annotation_rows;
mod captions;
mod captions_style;
mod click;
mod click_style;
mod cue;
mod cursor;
mod font;
mod object;
mod sound;
mod sound_clip;
mod sound_extra;
mod zoom;
mod zoom_region;

#[cfg(test)]
mod annot_kinds_tests;
#[cfg(test)]
mod annotation_tests;
#[cfg(test)]
mod callout_tests;
#[cfg(test)]
pub(super) mod captions_tests;
#[cfg(test)]
mod cursor_tests;
#[cfg(test)]
mod shortcuts_tests;
#[cfg(test)]
mod sound_tests;
#[cfg(test)]
pub(crate) mod tests;
#[cfg(test)]
mod zoom_tests;

use dc_core::{Effect, Project, StateError};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::panel::{self, Groups};
use dc_ui_kit::strings::editor as text;
use dc_ui_kit::tokens::{self, BG, SURFACE};
use dc_ui_kit::widgets::IconButton;
use eframe::egui::{self, Frame, Margin, Rect, ScrollArea, Ui};

use super::selection::{Item, Selected};
use super::view::{Panel, View};
use super::{Action, EditorWindow};
pub use annotation::{Edit, target};
pub use dc_ui_kit::panel::{
    BUTTON, CHEVRON, Group, Header, args, bottom_line, header, header_button, padded, reset, rows,
};

/// Width of the tab rail, side of a tab (mockup).
const RAIL: f32 = 44.0;
const TAB: f32 = 36.0;

const PANELS: [Panel; 5] = [
    Panel::Zoom,
    Panel::Cursor,
    Panel::Keys,
    Panel::Captions,
    Panel::Sound,
];

/// A panel: draws into the inspector and asks for actions through [`Cx`].
type PanelUi = fn(&mut Ui, &mut Cx<'_>);

/// The panel of a rail tab with its id (the first part of its groups' keys).
fn tab(panel: Panel) -> (&'static str, PanelUi) {
    match panel {
        Panel::Keys => (dc_fx_keys::panel::ID, keys),
        Panel::Zoom => (zoom::ID, zoom::ui),
        Panel::Cursor => (cursor::ID, cursor::ui),
        Panel::Captions => (captions::ID, captions::ui),
        Panel::Sound => (sound::ID, sound::ui),
    }
}

/// «Показ клавиш» of `dc_fx_keys` with the shortcuts the «Клавиши» row placed.
fn keys(ui: &mut Ui, cx: &mut Cx<'_>) {
    dc_fx_keys::panel::ui(ui, &mut cx.base, &cx.tracks.shortcuts);
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
}

/// What a panel of the editor works with this frame: what every panel gets and the editor's
/// own parts the panels not moved to their effects' crates still read.
pub struct Cx<'a> {
    /// The project, the defaults, the playhead, the fonts and the groups; edits of the panels
    /// of the effects' crates.
    pub base: panel::Cx<'a>,
    /// What the timeline shows of it: the shortcuts of the recordings for «Показ клавиш».
    pub tracks: &'a super::Tracks,
    /// The generation and the cue list of the captions panel (7.17).
    pub captioning: captions::Captioning<'a>,
    /// Sound tracks the recordings have (`mic`, `system`): the groups of the sound panels.
    pub sound: &'a [String],
    /// Sound tracks of the imported videos, as they come: the groups of their clips' panel.
    pub streams: &'a crate::editor::streams::Streams,
    /// What the panel asked for; the editor runs it after the layout.
    pub actions: Vec<Action>,
}

impl Cx<'_> {
    /// The default state of effect `E`: its settings from the defaults, the rest factory.
    pub fn defaults_of<E: Effect>(&self) -> Result<E::State, StateError> {
        self.base.defaults_of::<E>()
    }

    /// A group: its header (chevron, title, «Сбросить») and, when open, `body`.
    pub fn group(&mut self, ui: &mut Ui, group: Group<'_>, body: impl FnOnce(&mut Ui, &mut Self)) {
        self.group_with(ui, group, |_, _, _| {}, body);
    }

    /// [`Cx::group`] whose header has more buttons: `buttons` gets the place left of
    /// «Сбросить», one [`BUTTON`] wide (the mute of a sound track).
    pub fn group_with(
        &mut self,
        ui: &mut Ui,
        group: Group<'_>,
        buttons: impl FnOnce(&mut Ui, &mut Self, Rect),
        body: impl FnOnce(&mut Ui, &mut Self),
    ) {
        let mut head = self.base.groups.head(ui, group);
        if let Some(reset) = head.reset.take() {
            self.actions.push(Action::Apply(reset));
        }
        let more = head.more;
        buttons(ui, self, more);
        head.body(ui, |ui| body(ui, self));
    }
}

/// Rail and panel; true when the user opened or closed a group (the settings keep it).
pub fn ui(ui: &mut Ui, editor: &mut EditorWindow) -> bool {
    follow(&mut editor.view);
    egui::Panel::left("inspector-rail")
        .exact_size(RAIL)
        .frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(4, 6)))
        .show(ui, |ui| rail(ui, &mut editor.view));
    let mut defaults = Project::default();
    editor.defaults.apply(&mut defaults);
    let mut cx = Cx {
        base: panel::Cx {
            project: editor.doc.project(),
            defaults: &defaults,
            playhead: editor.view.playhead,
            auto_keys: editor.view.auto_keys,
            fonts: &editor.fonts,
            groups: Groups::new(&mut editor.groups.0),
            edits: Vec::new(),
        },
        tracks: &editor.tracks,
        captioning: captions::Captioning {
            generation: &mut editor.captions,
            playhead: editor.view.playhead,
            sound: &editor.sound,
            picked: match editor.view.selection.single() {
                Some(Selected::Region(Item::Cue(id))) => Some(id),
                _ => None,
            },
        },
        sound: &editor.sound,
        streams: &editor.streams,
        actions: Vec::new(),
    };
    let view = &editor.view;
    let tracks = &editor.tracks;
    egui::CentralPanel::no_frame()
        .frame(Frame::new().fill(SURFACE))
        .show(ui, |ui| {
            ScrollArea::vertical().show(ui, |ui| match view.object {
                Some(picked) => object::ui(ui, &mut cx, tracks, picked),
                None => {
                    let (id, panel) = tab(view.panel);
                    cx.base.groups.panel(id);
                    panel(ui, &mut cx);
                }
            });
        });
    let edits = std::mem::take(&mut cx.base.edits);
    cx.actions.extend(edits.into_iter().map(Action::Apply));
    if let Some(typed) = rows::flush(ui) {
        cx.actions.push(Action::Apply(vec![typed]));
    }
    let toggled = cx.base.groups.toggled();
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
        let on = view.object.is_none() && view.panel == panel;
        let button = IconButton::new(panel.icon(), panel.title())
            .size(TAB, tokens::ICON)
            .on(on);
        let response = ui.add(button).on_hover_text(panel.title());
        if response.clicked() {
            view.panel = panel;
            view.object = None;
        }
    }
}
