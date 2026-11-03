//! The 340 px inspector (mockup «3. Панели эффектов»): a rail of tabs on the left — the global
//! settings of an effect — and the open panel. Picking one object on the timeline opens its
//! panel; a tab opens the tab's panel until another object is picked.
//!
//! A panel draws with the kit of `dc_ui_kit::panel` (header, groups, rows) over its
//! `panel::Cx`. A panel that lives in its effect's crate (docs/ARCHITECTURE.md, «Панель
//! эффекта») gets [`Cx::base`] and the facts of the timeline it needs; one not moved yet works
//! with the whole [`Cx`]. Every committed edit is a registry command — one undo step; what a
//! panel asks besides the project (`panel::Request`) becomes an [`Action`] in [`action`]. A tab
//! gets its panel in [`tab`], an object kind in `object.rs`.

mod annot_blur;
mod annot_drawing;
mod annot_image;
mod annot_shape;
mod annot_text;
mod annotation;
mod annotation_rows;
mod captions;
mod object;

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

use dc_core::project::MediaKind;
use dc_core::{ClipId, Project};
use dc_fx_audio_tracks::panel::Sources;
use dc_fx_captions::panel::Recognition;
use dc_ui_kit::icons::Icon;
use dc_ui_kit::panel::{self, Groups, Request};
use dc_ui_kit::strings::{editor as text, inspector as names};
use dc_ui_kit::tokens::{self, BG, SURFACE};
use dc_ui_kit::widgets::IconButton;
use eframe::egui::{self, Frame, Margin, ScrollArea, Ui};

use super::fonts::FontFile;
use super::selection::{Item, Selected};
use super::streams::{Imported, Streams};
use super::view::{Panel, View};
use super::{Action, EditorWindow, Transport};
pub use annotation::{Edit, target};
pub use dc_ui_kit::panel::{
    BUTTON, CHEVRON, Group, Header, bottom_line, header, header_button, padded, rows,
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
        Panel::Zoom => (dc_fx_zoom::panel::ID, zoom),
        Panel::Cursor => (dc_fx_cursor::panel::ID, cursor),
        Panel::Captions => (dc_fx_captions::panel::ID, captions::tab),
        Panel::Sound => (dc_fx_audio_tracks::panel::ID, sound),
    }
}

/// «Зум» of `dc_fx_zoom`: no facts, the zooms are in the project.
fn zoom(ui: &mut Ui, cx: &mut Cx<'_>) {
    dc_fx_zoom::panel::ui(ui, &mut cx.base);
}

/// «Курсор и клики» of `dc_fx_cursor`: no facts, the settings are in the project.
fn cursor(ui: &mut Ui, cx: &mut Cx<'_>) {
    dc_fx_cursor::panel::ui(ui, &mut cx.base);
}

/// «Показ клавиш» of `dc_fx_keys` with the shortcuts the «Клавиши» row placed.
fn keys(ui: &mut Ui, cx: &mut Cx<'_>) {
    dc_fx_keys::panel::ui(ui, &mut cx.base, &cx.tracks.shortcuts);
}

/// «Звук» of `dc_fx_audio_tracks` with the tracks of the recordings.
fn sound(ui: &mut Ui, cx: &mut Cx<'_>) {
    let sources = cx.sources(None);
    dc_fx_audio_tracks::panel::ui(ui, &mut cx.base, &sources);
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
    /// The object picked alone on the timeline: the line of the cue list that opens its editor.
    pub picked: Option<Selected>,
    /// The recognition for «Субтитры» (7.17); the language and the tracks the panel chose go
    /// back to the editor after the layout.
    pub recognition: Recognition,
    /// Sound tracks the recordings have (`mic`, `system`): the groups of the sound panels.
    pub sound: &'a [String],
    /// Sound tracks of the imported videos, as they come: the groups of their clips' panel.
    pub streams: &'a crate::editor::streams::Streams,
    /// What the panel asked for; the editor runs it after the layout.
    pub actions: Vec<Action>,
}

impl<'a> Cx<'a> {
    /// A group: its header (chevron, title, «Сбросить») and, when open, `body`.
    pub fn group(&mut self, ui: &mut Ui, group: Group<'_>, body: impl FnOnce(&mut Ui, &mut Self)) {
        let mut head = self.base.groups.head(ui, group);
        if let Some(reset) = head.reset.take() {
            self.actions.push(Action::Apply(reset));
        }
        head.body(ui, |ui| body(ui, self));
    }

    /// The sound facts of `dc_fx_audio_tracks::panel` for clip `clip` (the streams of its
    /// imported video), or for no clip.
    fn sources(&self, clip: Option<ClipId>) -> Sources<'a> {
        let (project, streams): (&'a Project, &'a Streams) = (self.base.project, self.streams);
        let media = clip.and_then(|id| project.clips.iter().find(|c| c.id == id));
        let streams = media
            .filter(|c| c.media.kind == MediaKind::Imported)
            .and_then(|c| streams.get(&Imported(c.media.clone())));
        Sources {
            recorded: self.sound,
            streams: streams.map(Vec::as_slice),
            end: self.tracks.duration,
        }
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
            adds: Vec::new(),
            requests: Vec::new(),
        },
        tracks: &editor.tracks,
        picked: editor.view.selection.single(),
        recognition: captions::recognition(&editor.captions, &editor.sound),
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
    let adds = std::mem::take(&mut cx.base.adds);
    cx.actions.extend(adds.into_iter().map(Action::Paste));
    if let Some(typed) = rows::flush(ui) {
        cx.actions.push(Action::Apply(vec![typed]));
    }
    let requests = std::mem::take(&mut cx.base.requests);
    cx.actions.extend(requests.into_iter().filter_map(action));
    let toggled = cx.base.groups.toggled();
    let (actions, recognition) = (cx.actions, cx.recognition);
    captions::take(&mut editor.captions, &recognition);
    editor.actions.extend(actions);
    toggled
}

/// What the editor does for a panel's request; nothing for a font dialog closed without a file.
fn action(request: Request) -> Option<Action> {
    match request {
        Request::Pick(id) => {
            let item = Item::parse(&id);
            if item.is_none() {
                tracing::warn!("a panel asked to pick {id}, which is not an object of the rows");
            }
            item.map(|item| Action::ListPick(Selected::Region(item)))
        }
        Request::Seek(at) => Some(Action::Transport(Transport::Scrub(at))),
        Request::AddFont(then) => rfd::FileDialog::new()
            .set_title(names::PICK_FONT)
            .add_filter(names::FONT_FILES, &dc_render::FONT_EXTENSIONS)
            .pick_file()
            .map(|path| Action::Font(FontFile { path, then })),
        Request::Recognize => Some(Action::Generate),
        Request::StopRecognizing => Some(Action::StopGenerating),
    }
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
