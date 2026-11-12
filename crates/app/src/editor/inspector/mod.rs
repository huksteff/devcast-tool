//! The 340 px inspector (mockup «3. Панели эффектов»): a rail of tabs on the left — the global
//! settings of an effect — and the open panel. Picking one object on the timeline opens its
//! panel; a tab opens the tab's panel until another object is picked.
//!
//! Every panel lives in its effect's crate (docs/ARCHITECTURE.md, «Панель эффекта») and draws
//! with the kit of `dc_ui_kit::panel` over [`Cx::base`] and the facts of the timeline it needs.
//! Every committed edit is a registry command — one undo step; what a panel asks besides the
//! project (`panel::Request`) becomes an [`Action`] in [`action`]. A tab gets its panel in
//! [`TABS`], an object kind in `object.rs`.

mod ai;
mod ai_cards;
mod captions;
mod object;

#[cfg(test)]
mod ai_tests;
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
use dc_core::{ClipId, OutputTime, Project};
use dc_fx_audio_tracks::panel::Sources;
use dc_fx_captions::panel::Recognition;
use dc_ui_kit::icons::Icon;
use dc_ui_kit::panel::{self, Groups, Request, rows};
use dc_ui_kit::strings::{annotations, editor as text, inspector as names};
use dc_ui_kit::tokens::{self, BG, SURFACE};
use dc_ui_kit::widgets::IconButton;
use eframe::egui::{self, Frame, Margin, ScrollArea, Ui};

use super::adding::PictureFile;
use super::fonts::FontFile;
use super::selection::Selected;
use super::streams::{Imported, Streams};
use super::view::View;
use super::{Action, EditorWindow, Transport};

/// Width of the tab rail, side of a tab (mockup).
const RAIL: f32 = 44.0;
const TAB: f32 = 36.0;

/// A panel: draws into the inspector and asks for actions through [`Cx`].
type PanelUi = fn(&mut Ui, &mut Cx<'_>);

/// A tab of the rail: the panel of the global settings of an effect.
pub struct Tab {
    /// The first part of the keys of its groups; names the open tab in [`View::tab`].
    pub id: &'static str,
    icon: Icon,
    title: &'static str,
    ui: PanelUi,
}

/// The tabs of the effects, in the order of the rail. A new tab is a line here and the function
/// that gives its panel the facts it needs.
const TABS: [Tab; 5] = [
    Tab {
        id: dc_fx_zoom::panel::ID,
        icon: Icon::Frame,
        title: text::ZOOM,
        ui: zoom,
    },
    Tab {
        id: dc_fx_cursor::panel::ID,
        icon: Icon::Cursor,
        title: text::CURSOR,
        ui: cursor,
    },
    KEYS,
    CAPTIONS,
    SOUND,
];

/// «Показ клавиш»: the tab the editor opens on.
pub const KEYS: Tab = Tab {
    id: dc_fx_keys::panel::ID,
    icon: Icon::Keyboard,
    title: text::KEYS,
    ui: keys,
};

pub const CAPTIONS: Tab = Tab {
    id: dc_fx_captions::panel::ID,
    icon: Icon::Captions,
    title: text::CAPTIONS,
    ui: captions::tab,
};

pub const SOUND: Tab = Tab {
    id: dc_fx_audio_tracks::panel::ID,
    icon: Icon::Speaker,
    title: text::SOUND,
    ui: sound,
};

/// «Сделать красиво» by the user's AI agent (8.11): under the line of the rail.
pub const AI: Tab = Tab {
    id: ai::ID,
    icon: Icon::Spark,
    title: dc_ui_kit::strings::ai::TAB,
    ui: ai::tab,
};

/// The tab with `id`.
fn tab(id: &str) -> Option<&'static Tab> {
    TABS.iter().chain([&AI]).find(|tab| tab.id == id)
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

/// What the inspector works with this frame: what every panel gets and the editor's own parts
/// the facts of the panels are made of.
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
    /// «Сделать красиво»: the agent, its run and the choices of the settings.
    pub ai: &'a mut crate::editor::ai::Ai,
    /// What the panel asked for; the editor runs it after the layout.
    pub actions: Vec<Action>,
}

impl<'a> Cx<'a> {
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
        ai: &mut editor.ai,
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
                    if let Some(tab) = tab(view.tab) {
                        cx.base.groups.panel(tab.id);
                        (tab.ui)(ui, &mut cx);
                    }
                }
            });
        });
    let edits = std::mem::take(&mut cx.base.edits);
    // A control held with the pointer: its edits show on the preview until it is let go.
    let draft = if rows::drafting(ui.ctx()) {
        Some(edits.concat())
    } else {
        cx.actions.extend(edits.into_iter().map(Action::Apply));
        None
    };
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
    match draft {
        // A control held still asks for nothing (decision 002).
        Some(steps) if editor.view.panel_draft.as_ref() == Some(&steps) => {}
        Some(steps) => {
            super::stage::show_draft(ui, editor, &steps);
            editor.view.panel_draft = Some(steps);
        }
        None => {
            if editor.view.panel_draft.take().is_some() {
                editor.preview.changed(editor.doc.project());
            }
        }
    }
    toggled
}

/// What the editor does for a panel's request; nothing for a file dialog closed without a file.
fn action(request: Request) -> Option<Action> {
    match request {
        Request::Pick(id) => {
            let item = crate::editor::lanes::parse(&id);
            if item.is_none() {
                tracing::warn!("a panel asked to pick {id}, which is not an object of the rows");
            }
            item.map(|item| Action::ListPick(Selected::Region(item)))
        }
        Request::Seek(at) => Some(Action::Transport(Transport::Scrub(at))),
        Request::AddFont(then) => crate::dialog::file()
            .set_title(names::PICK_FONT)
            .add_filter(names::FONT_FILES, &dc_render::FONT_EXTENSIONS)
            .pick_file()
            .map(|path| Action::Font(FontFile { path, then })),
        Request::Picture { id, at } => picture(&id, at),
        Request::Recognize => Some(Action::Generate),
        Request::StopRecognizing => Some(Action::StopGenerating),
    }
}

/// The image of annotation `id` from a picture file the user picks, at `at` or everywhere;
/// nothing when the dialog is closed without a file.
fn picture(id: &str, at: Option<OutputTime>) -> Option<Action> {
    let Ok(id) = id.parse() else {
        tracing::warn!("a panel asked for a picture of {id}, which is not an annotation");
        return None;
    };
    let path = crate::dialog::file()
        .set_title(annotations::PICK_PICTURE)
        .add_filter(annotations::PICTURES, &dc_media::PICTURE_EXTENSIONS)
        .pick_file()?;
    Some(Action::Picture(PictureFile { id, at, path }))
}

/// A newly picked single object opens its panel; no single object goes back to the tab.
fn follow(view: &mut View) {
    let single = view.selection.single();
    if single != view.followed {
        view.followed = single;
        view.object = single;
    }
}

/// The tabs of the effects, a line, then «Нейронка» (mockup 9).
fn rail(ui: &mut Ui, view: &mut View) {
    ui.spacing_mut().item_spacing.y = 2.0;
    for tab in &TABS {
        tab_button(ui, view, tab);
    }
    let (line, _) = ui.allocate_exact_size(egui::vec2(TAB, tokens::S2 + 1.0), egui::Sense::hover());
    let y = line.center().y;
    let x = line.center().x;
    let half = (TAB - tokens::S3) / 2.0;
    ui.painter()
        .hline(x - half..=x + half, y, (1.0, tokens::BORDER));
    tab_button(ui, view, &AI);
}

fn tab_button(ui: &mut Ui, view: &mut View, tab: &Tab) {
    let on = view.object.is_none() && view.tab == tab.id;
    let button = IconButton::new(tab.icon, tab.title)
        .size(TAB, tokens::ICON)
        .on(on);
    let response = ui.add(button).on_hover_text(tab.title);
    if response.clicked() {
        view.tab = tab.id;
        view.object = None;
    }
}
