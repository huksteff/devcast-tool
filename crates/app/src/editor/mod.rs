//! The editor window (docs/design/editor.html, «Главное окно редактора»): header, preview with
//! its transport, inspector on the right, timeline under them. It opens with the project of a
//! finished recording; the HUD hides meanwhile and comes back when the editor closes (v1).
//! Controls change the project only by queueing an `Action` (a registry command, undo, redo);
//! the frame runs them after its layout and rebuilds the view from the new project. Shortcuts
//! (`keys.rs`) come from the settings' table, F1 or «?» opens their help (`help.rs`).

mod actions;
mod adding;
mod clipboard;
mod closing;
mod crop;
mod document;
mod fonts;
mod generation;
mod header;
mod help;
mod inspector;
mod keyframes;
mod keys;
mod library;
mod montage;
mod playback;
mod presses;
mod preview;
mod reader;
mod regions;
mod rows;
mod selection;
mod stage;
mod streams;
mod threads;
mod timeline;
pub mod timeline_window;
mod transport;
mod view;
mod waves;
mod window;

#[cfg(test)]
mod editing_tests;
#[cfg(test)]
mod generation_tests;
#[cfg(test)]
mod keys_tests;
#[cfg(test)]
mod playback_tests;
#[cfg(test)]
mod test_project;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod timeline_window_tests;
#[cfg(test)]
mod waves_tests;

use std::time::Instant;

use dc_core::PixelSize;
use dc_fx_crop::{Aspect, Crop};
use dc_project::ProjectDir;
use dc_ui_kit::tokens::{BG, SURFACE};
use eframe::egui::{self, Frame, Ui, vec2};

use crate::keymap::EditorKeys;
use crate::settings::{EditorLayout, InspectorGroups};
use crate::settings_window::Page;
pub use actions::{Action, Transport};
pub use document::{Document, DocumentError, LOCK_OWNER, Way};
pub use keys::wired;
use playback::seek::{FALLBACK_FPS, frame_rate};
pub use timeline::{Place, Tracks};
pub use view::View;
pub use window::show;

/// Size of the window at the start and its least size, points (mockup).
const SIZE: egui::Vec2 = vec2(1280.0, 800.0);
const MIN_SIZE: egui::Vec2 = vec2(1024.0, 640.0);
/// Height of the header, width of the inspector (mockup).
const HEADER: f32 = 44.0;
const INSPECTOR: f32 = 340.0;
/// The timeline is dragged between this height and 75 % of the window (v1).
const TIMELINE_MIN: f32 = 140.0;
const TIMELINE_MAX_SHARE: f32 = 0.75;

/// The open project and what the window shows of it.
#[derive(Debug)]
pub struct EditorWindow {
    pub doc: Document,
    /// Project name: the folder without `.devcast`.
    pub name: String,
    /// Sound tracks the recordings have (`mic`, `system`): rows of the source sound.
    pub sound: Vec<String>,
    /// The timeline rows of the project.
    pub tracks: Tracks,
    /// Output frame: its aspect setting and size (none without clips).
    pub aspect: Aspect,
    pub canvas: Option<PixelSize>,
    pub view: View,
    /// The frame of the project at the playhead.
    pub preview: preview::Preview,
    /// The clock and the sound while playing.
    pub playback: playback::Playback,
    /// Peaks of the waves on the sound rows, as they come.
    pub waves: waves::Waves,
    /// Presses of the recordings: the dots of the «Курсор» row and the shortcuts of the
    /// «Клавиши» row, as they come.
    pub presses: presses::Presses,
    /// Sound tracks of the imported videos: the rows of the sound panel of their clips.
    pub streams: streams::Streams,
    /// Frames per second of a frame step: the highest of the clips' videos (v1), 30 without.
    pub fps: f64,
    /// Shortcuts of the settings, taken each frame.
    pub keys: EditorKeys,
    /// The shortcut help while it is open.
    pub help: Option<help::Help>,
    /// The crop and format dialog while it is open.
    pub crop: Option<crop::CropDialog>,
    /// Closing the window after a write that failed.
    pub closing: closing::Closing,
    /// The settings new projects start with: «Сбросить» of the inspector brings them back.
    pub defaults: dc_project::Defaults,
    /// Open and closed groups of the inspector, taken from the settings each frame.
    pub groups: InspectorGroups,
    /// «Создать субтитры»: the model, the worker process and the choices of the panel (7.17).
    pub captions: generation::Generation,
    /// Families of the font pickers: built-in and the project's own.
    pub fonts: Vec<String>,
    /// The timeline window: open or not, its place, which window should repaint.
    pub detached: timeline_window::Detached,
    /// The recordings library panel (7.27).
    pub library: library::Library,
    /// Actions the controls asked for in this frame.
    actions: Vec<Action>,
}

impl EditorWindow {
    /// The editor of the project a recording has just saved.
    pub fn new(
        recorded: &dc_capture::win::RecordedProject,
    ) -> Result<(Self, Vec<String>), DocumentError> {
        let sound = recorded
            .summary
            .audio
            .iter()
            .map(|a| a.track.to_owned())
            .collect();
        Self::open(recorded.dir.clone(), sound)
    }

    /// Opens the project in `dir` for editing; also returns the problems to show (a broken
    /// history, a broken effect state).
    pub fn open(dir: ProjectDir, sound: Vec<String>) -> Result<(Self, Vec<String>), DocumentError> {
        let (doc, mut problems) = Document::open(dir)?;
        // The folder is `name.devcast`; the header shows the extension apart.
        let name = std::path::Path::new(doc.dir().name())
            .file_stem()
            .map_or_else(
                || doc.dir().name().to_owned(),
                |s| s.to_string_lossy().into_owned(),
            );
        let fonts = fonts::families(doc.dir()).unwrap_or_else(|e| {
            problems.push(e);
            Vec::new()
        });
        let preview = preview::Preview::new(doc.project());
        let (project, revision) = preview.project();
        let mut editor = Self {
            playback: playback::Playback::new(project, revision),
            fps: FALLBACK_FPS,
            doc,
            name,
            sound,
            tracks: Tracks::default(),
            aspect: Aspect::default(),
            canvas: None,
            view: View::default(),
            preview,
            waves: waves::Waves::default(),
            presses: presses::Presses::default(),
            streams: streams::Streams::default(),
            captions: generation::Generation::default(),
            fonts,
            keys: EditorKeys::default(),
            help: None,
            crop: None,
            closing: closing::Closing::default(),
            defaults: dc_project::Defaults::factory(),
            groups: InspectorGroups::default(),
            detached: timeline_window::Detached::default(),
            library: library::Library::default(),
            actions: Vec::new(),
        };
        problems.extend(editor.refresh(Instant::now()));
        problems.extend(editor.tracks.problems.iter().cloned());
        Ok((editor, problems))
    }

    /// Rebuilds the view of the project after it changed at `now`; playback goes on with the
    /// new project. Returns why its sound cannot go on.
    fn refresh(&mut self, now: Instant) -> Option<String> {
        // A gesture on the preview was over the old project; the preview shows the new one.
        self.view.stage = None;
        self.retrack();
        let project = self.doc.project();
        let crop = project.state::<Crop>().unwrap_or_else(|e| {
            self.tracks.problems.push(e.to_string());
            Default::default()
        });
        self.canvas = dc_fx_crop::canvas(project, &crop.settings);
        self.aspect = crop.settings.aspect;
        self.fps = frame_rate(self.doc.dir(), project);
        self.preview.changed(project);
        let (project, revision) = self.preview.project();
        self.playback.changed(project, revision, now)
    }

    /// Rebuilds the rows of the timeline from the project and the rows the user added; keeps
    /// the selection that still fits.
    fn retrack(&mut self) {
        self.tracks = Tracks::of(self.doc.project(), &self.sound, self.view.least);
        self.waves.want(self.tracks.sources());
        let project = self.doc.project();
        self.presses.want(presses::recordings(project).iter());
        self.streams.want(streams::imported(project).iter());
        self.tracks.clicks =
            timeline::click_dots(project, |media| self.presses.get(media).map(|p| &p.clicks));
        self.tracks
            .place_keys(project, &self.presses.key_tracks(project));
        let end = self.end();
        self.view.keep(&self.tracks, end);
    }

    /// The output end: where playback stops.
    pub fn end(&self) -> dc_core::OutputTime {
        dc_core::OutputTime::ZERO + self.tracks.duration
    }

    /// Queues `action` to run after this frame's layout. Undo brings back the library's last
    /// batch from the Recycle Bin first while the library is open (mockup 6).
    pub fn ask(&mut self, action: Action) {
        let action = match action {
            Action::Travel(Way::Undo) if self.library.takes_undo() => {
                Action::Library(library::Ask::Restore)
            }
            action => action,
        };
        self.actions.push(action);
    }

    /// Opens the shortcut help or closes it.
    pub fn toggle_help(&mut self) {
        self.help = match self.help {
            Some(_) => None,
            None => Some(help::Help::default()),
        };
    }
}

/// What a frame of the editor asks of the app.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// The settings window to open on this page.
    pub open_settings: Option<Page>,
    /// Timeline height and row factor after the frame.
    pub layout: EditorLayout,
    /// Why an action failed, for a toast.
    pub errors: Vec<String>,
    /// The inspector groups after the user opened or closed one, for the settings.
    pub groups: Option<InspectorGroups>,
}

/// The whole window: header, timeline at the bottom (`layout`: its height until dragged, its row
/// factor until the wheel changes it), inspector on the right, preview in the middle, the help over them; then the actions of the
/// frame, at `now`. `keys` are the shortcuts of the settings, `groups` its inspector groups.
pub fn editor_ui(
    ui: &mut Ui,
    editor: &mut EditorWindow,
    layout: EditorLayout,
    (keys, groups): (&EditorKeys, &InspectorGroups),
    now: Instant,
) -> Outcome {
    if editor.keys != *keys {
        editor.keys = keys.clone();
    }
    if editor.groups != *groups {
        editor.groups = groups.clone();
    }
    editor.receive();
    keys::run(ui, editor);
    let volume = (editor.view.volume / 100.0) as f32;
    let tick = editor.playback.tick(now, editor.end(), volume);
    if let Some(at) = tick.at {
        editor.view.playhead = at;
    }
    let mut open_settings = None;
    egui::Panel::top("editor-header")
        .exact_size(HEADER)
        .frame(Frame::new().fill(SURFACE))
        .show(ui, |ui| {
            if header::ui(ui, editor) {
                open_settings = Some(Page::default());
            }
        });
    let most = (ui.available_height() * TIMELINE_MAX_SHARE).max(TIMELINE_MIN);
    let timeline = (!editor.detached.open).then(|| {
        egui::Panel::bottom("editor-timeline")
            .resizable(true)
            .default_size(layout.timeline_height.clamp(TIMELINE_MIN, most))
            .size_range(TIMELINE_MIN..=most)
            .frame(Frame::new().fill(SURFACE))
            .show(ui, |ui| {
                timeline::ui(ui, editor, layout.rows, Place::Docked)
            })
    });
    if let Some(timeline) = &timeline {
        library::drop_on(ui, editor, timeline.response.rect);
    }
    let toggled = egui::Panel::right("editor-inspector")
        .exact_size(INSPECTOR)
        .frame(Frame::new().fill(SURFACE))
        .show(ui, |ui| inspector::ui(ui, editor))
        .inner;
    let preview = egui::CentralPanel::no_frame()
        .frame(Frame::new().fill(BG))
        .show(ui, |ui| preview::ui(ui, editor));
    library::show(ui, editor, preview.response.rect);
    if let Some(help) = editor.help.as_mut() {
        let answer = help::show(ui, help, &editor.keys);
        if answer.close {
            editor.help = None;
        }
        if answer.settings {
            open_settings = Some(Page::Hotkeys);
        }
    }
    crop::show(ui, editor);
    let mut errors: Vec<String> = tick.error.into_iter().collect();
    errors.extend(actions::run(ui, editor, now));
    if editor.playback.is_playing() {
        // Decision 002: while playing, a repaint per frame step; paused, nothing runs.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(1.0 / editor.fps));
    }
    Outcome {
        open_settings,
        layout: timeline.map_or(layout, |t| EditorLayout {
            timeline_height: t.response.rect.height(),
            rows: t.inner,
            ..layout
        }),
        errors,
        groups: toggled.then(|| editor.groups.clone()),
    }
}
