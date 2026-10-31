//! The editor window (docs/design/editor.html, «Главное окно редактора»): header, preview with
//! its transport, inspector on the right, timeline under them. It opens with the project of a
//! finished recording; the HUD hides meanwhile and comes back when the editor closes (v1).
//! Controls change the project only by queueing an `Action` (a registry command, undo, redo);
//! the frame runs them after its layout and rebuilds the view from the new project. Shortcuts
//! (`keys.rs`) come from the settings' table, F1 or «?» opens their help (`help.rs`).

mod actions;
mod adding;
mod clipboard;
mod document;
mod header;
mod help;
mod inspector;
mod keys;
mod montage;
mod playback;
mod presses;
mod preview;
mod reader;
mod regions;
mod rows;
mod selection;
mod timeline;
mod transport;
mod view;
mod waves;
mod window;

#[cfg(test)]
mod editing_tests;
#[cfg(test)]
mod keys_tests;
#[cfg(test)]
mod playback_tests;
#[cfg(test)]
mod test_project;
#[cfg(test)]
mod tests;
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
pub use timeline::Tracks;
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
    /// Project name: the folder without `.dcproj`.
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
    /// Frames per second of a frame step: the highest of the clips' videos (v1), 30 without.
    pub fps: f64,
    /// Shortcuts of the settings, taken each frame.
    pub keys: EditorKeys,
    /// The shortcut help while it is open.
    pub help: Option<help::Help>,
    /// The settings new projects start with: «Сбросить» of the inspector brings them back.
    pub defaults: dc_project::Defaults,
    /// Open and closed groups of the inspector, taken from the settings each frame.
    pub groups: InspectorGroups,
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
        // The folder is `name.dcproj`; the header shows the extension apart.
        let name = std::path::Path::new(doc.dir().name())
            .file_stem()
            .map_or_else(
                || doc.dir().name().to_owned(),
                |s| s.to_string_lossy().into_owned(),
            );
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
            keys: EditorKeys::default(),
            help: None,
            defaults: dc_project::Defaults::factory(),
            groups: InspectorGroups::default(),
            actions: Vec::new(),
        };
        problems.extend(editor.refresh(Instant::now()));
        problems.extend(editor.tracks.problems.iter().cloned());
        Ok((editor, problems))
    }

    /// Rebuilds the view of the project after it changed at `now`; playback goes on with the
    /// new project. Returns why its sound cannot go on.
    fn refresh(&mut self, now: Instant) -> Option<String> {
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

    /// Starts rendering the preview on the shared device; the thread wakes the main window.
    pub fn start_preview(
        &mut self,
        gpu: crate::gpu::PreviewGpu,
        ctx: egui::Context,
    ) -> Result<(), crate::AppError> {
        let dir = self.doc.dir().clone();
        self.preview
            .start(gpu, dir, ctx)
            .map_err(|source| crate::AppError::Thread {
                name: "devcast-preview",
                source,
            })
    }

    /// Starts the sound thread of playback; without it playback goes by the wall clock.
    pub fn start_sound(&mut self) -> Result<(), crate::AppError> {
        let dir = self.doc.dir().clone();
        self.playback
            .start(dir)
            .map_err(|source| crate::AppError::Thread {
                name: "devcast-sound",
                source,
            })
    }

    /// Starts reading the peaks of the sound rows; without it the rows stay plain.
    pub fn start_waves(&mut self, ctx: egui::Context) -> Result<(), crate::AppError> {
        let dir = self.doc.dir().clone();
        self.waves
            .start(dir, ctx)
            .map_err(|source| crate::AppError::Thread {
                name: "devcast-peaks",
                source,
            })
    }

    /// Starts reading the presses of the recordings; without it the «Курсор» row has no dots
    /// and the «Клавиши» row no shortcuts.
    pub fn start_presses(&mut self, ctx: egui::Context) -> Result<(), crate::AppError> {
        let dir = self.doc.dir().clone();
        self.presses
            .start(dir, ctx)
            .map_err(|source| crate::AppError::Thread {
                name: "devcast-presses",
                source,
            })
    }

    /// Queues `action` to run after this frame's layout.
    pub fn ask(&mut self, action: Action) {
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
    editor.waves.receive();
    if editor.presses.receive() {
        editor.retrack();
    }
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
    let timeline = egui::Panel::bottom("editor-timeline")
        .resizable(true)
        .default_size(layout.timeline_height.clamp(TIMELINE_MIN, most))
        .size_range(TIMELINE_MIN..=most)
        .frame(Frame::new().fill(SURFACE))
        .show(ui, |ui| timeline::ui(ui, editor, layout.rows));
    let toggled = egui::Panel::right("editor-inspector")
        .exact_size(INSPECTOR)
        .frame(Frame::new().fill(SURFACE))
        .show(ui, |ui| inspector::ui(ui, editor))
        .inner;
    egui::CentralPanel::no_frame()
        .frame(Frame::new().fill(BG))
        .show(ui, |ui| preview::ui(ui, editor));
    if let Some(help) = editor.help.as_mut() {
        let answer = help::show(ui, help, &editor.keys);
        if answer.close {
            editor.help = None;
        }
        if answer.settings {
            open_settings = Some(Page::Hotkeys);
        }
    }
    let mut errors: Vec<String> = tick.error.into_iter().collect();
    errors.extend(actions::run(ui, editor, now));
    if editor.playback.is_playing() {
        // Decision 002: while playing, a repaint per frame step; paused, nothing runs.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(1.0 / editor.fps));
    }
    Outcome {
        open_settings,
        layout: EditorLayout {
            timeline_height: timeline.response.rect.height(),
            rows: timeline.inner,
        },
        errors,
        groups: toggled.then(|| editor.groups.clone()),
    }
}
