//! What the controls and shortcuts (`keys.rs`) ask of the project and of playback, and how it is
//! done: registry commands through `Document::apply` (the path of `devcast`), undo and redo,
//! play, pause and jumps of the playhead.

use std::time::Instant;

use super::lanes::annotations::AnnotId;
use dc_core::OpName;
use dc_ops::Step;
use eframe::egui::Ui;
use serde_json::{Map, Value};

use super::playback::seek;
use super::selection::{Item, Selected, same_moment};
use super::timeline::Least;
use super::{EditorWindow, Way};
use super::{adding, ai, crop, fonts, generation, library};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Travel(Way),
    /// Registry commands as one undo step (also `zoom add --at clicks`, which the recordings of
    /// the project resolve: `Document::apply_resolved`).
    Apply(Vec<Step>),
    /// Registry commands that add regions, as one undo step; the added regions get picked
    /// («Подобрать зумы» is `zoom add --at clicks`, `dc_ops::autozoom::by_clicks`).
    Paste(Vec<Step>),
    /// Registry commands that put a key of the annotation at the playhead, as one undo step;
    /// that key gets picked (I, Ctrl+V of a key).
    Keyed(Vec<Step>, AnnotId),
    Transport(Transport),
    /// Rows shown at least: an empty row added or removed; the timeline is rebuilt.
    Rows(Least),
    /// A sound file added to an extra sound row, as one undo step; it gets picked.
    Sound(adding::SoundFile),
    /// A picture file shown by an image annotation, as one undo step (`devcast image set`).
    Picture(adding::PictureFile),
    /// A new image annotation with a picture file, as one undo step; it gets picked.
    NewPicture(adding::NewPicture),
    /// A font file added to the project and chosen by a panel's command, as one undo step.
    Font(fonts::FontFile),
    /// «Создать субтитры»: recognize the speech (downloading the model first when needed); the
    /// cues replace all cues as one undo step when they come (`generation.rs`).
    Generate,
    /// «Отменить» of the generation: the worker is killed.
    StopGenerating,
    /// An object picked from a list of a panel (the cues of «Субтитры»): picked alone, the
    /// panel with the list stays open.
    ListPick(Selected),
    /// «Кроп и формат»: playback pauses, the crop dialog opens for the picked clip (`crop`).
    OpenCrop,
    /// «↗»: the timeline moves into its own window (`true`) or back under the preview.
    Detach(bool),
    /// What the recordings library asks (`library/`).
    Library(library::Ask),
    /// What the «Нейронка» tab asks of the run (`ai.rs`).
    Ai(ai::AiAsk),
    /// Ctrl+S: the working state into `project.json`.
    Save,
}

/// Playback and the playhead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// Play from the playhead (from the start when it is at the end) or pause.
    PlayPause,
    /// Pause and go this many frames (< 0 back).
    Step(i64),
    /// To the next keyframe (`true`) or the previous one, or 5 s that way; playback goes on.
    Jump(bool),
    /// The playhead is dragged here: playback pauses meanwhile, the preview shows keyframes.
    Scrub(dc_core::OutputTime),
    /// The drag of the playhead ended; playback goes on when it played before (`true`).
    Release(bool),
}

impl Action {
    /// The command `op` with `args` (`{"show": true}` is `--show true`).
    pub fn op(op: OpName, args: impl IntoIterator<Item = (&'static str, Value)>) -> Self {
        let args: Map<String, Value> = args
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect();
        Self::Apply(vec![Step::of(op, args)])
    }
}

/// Runs the queued actions; the view is rebuilt when the project changed. Returns the errors.
pub fn run(ui: &Ui, editor: &mut EditorWindow, now: Instant) -> Vec<String> {
    let mut errors = Vec::new();
    let mut changed = receive(ui, editor, now, &mut errors);
    let mut moved = false;
    let mut rows = false;
    for action in std::mem::take(&mut editor.actions) {
        match action {
            Action::Travel(way) => changed |= editor.doc.travel(way, now).is_some(),
            Action::Library(ask) => match library::run(editor, ask, now) {
                Ok(applied) => (changed, moved) = (changed | applied, true),
                Err(error) => errors.push(error),
            },
            Action::Apply(steps) => match editor.doc.apply_resolved(&steps, now) {
                Ok((applied, notes)) => {
                    changed |= applied.changed;
                    errors.extend(notes);
                }
                Err(error) => errors.push(error.to_string()),
            },
            Action::Paste(steps) => {
                let add = |e: &mut EditorWindow| e.doc.apply_resolved(&steps, now).map(|(_, n)| n);
                moved |= adds(editor, now, &mut errors, |e| {
                    add(e).map_err(|e| e.to_string())
                });
            }
            Action::Keyed(steps, id) => moved |= keyed(editor, &steps, id, now, &mut errors),
            Action::Rows(least) => {
                editor.view.least = least;
                rows = true;
            }
            Action::Sound(file) => {
                let add = |e: &mut EditorWindow| adding::sound(e, &file, now).map(|()| Vec::new());
                moved |= adds(editor, now, &mut errors, add);
            }
            Action::Picture(file) => match adding::picture(editor, &file, now) {
                Ok(()) => changed = true,
                Err(error) => errors.push(error),
            },
            Action::NewPicture(file) => {
                let add =
                    |e: &mut EditorWindow| adding::new_picture(e, &file, now).map(|()| Vec::new());
                moved |= adds(editor, now, &mut errors, add);
            }
            Action::Font(file) => match fonts::add(editor, file, now) {
                Ok(applied) => changed |= applied,
                Err(error) => errors.push(error),
            },
            Action::Generate => errors.extend(generation::start(ui.ctx(), editor).err()),
            Action::StopGenerating => editor.captions.cancel(),
            Action::Ai(ask) => changed |= ask_ai(ui, editor, ask, now, &mut errors),
            Action::Detach(open) => editor.detach(open),
            Action::Save => errors.extend(editor.doc.save().err().map(|e| e.to_string())),
            Action::ListPick(picked) => {
                editor.view.selection.only(picked);
                editor.view.followed = Some(picked);
                moved = true;
            }
            Action::OpenCrop => {
                open_crop(editor, now);
                moved = true;
            }
            Action::Transport(transport) => {
                errors.extend(transport_run(editor, transport, now));
                moved = true;
            }
        }
    }
    if changed {
        // Commands keep effect states valid, so a rebuilt view finds no new problems.
        errors.extend(editor.refresh(now));
    }
    if rows && !changed {
        editor.retrack();
    }
    if changed || moved || rows {
        ui.ctx().request_repaint();
    }
    errors
}

/// Takes what the background work brought: the cues of a recognition (true when they changed
/// the project) and the progress of the AI run.
fn receive(ui: &Ui, editor: &mut EditorWindow, now: Instant, errors: &mut Vec<String>) -> bool {
    let mut changed = false;
    if let Some(done) = generation::receive(ui.ctx(), editor, now) {
        match done {
            Ok(applied) => changed = applied,
            Err(error) => errors.push(error),
        }
    }
    editor.ai.receive(now);
    changed
}

/// Applies `steps`, which put a key of annotation `id` at the playhead, and picks that key;
/// true when it did.
fn keyed(
    editor: &mut EditorWindow,
    steps: &[Step],
    id: AnnotId,
    now: Instant,
    errors: &mut Vec<String>,
) -> bool {
    match editor.doc.apply(steps, now) {
        Ok(_) => {
            errors.extend(editor.refresh(now));
            pick_key(editor, id);
            true
        }
        Err(error) => {
            errors.push(error.to_string());
            false
        }
    }
}

/// Playback pauses and the crop dialog opens for the picked clip.
fn open_crop(editor: &mut EditorWindow, now: Instant) {
    if editor.playback.is_playing() {
        editor.view.playhead = editor.playback.pause(now);
    }
    editor.crop = crop::CropDialog::open(editor);
}

/// Does `transport` at `now`; the message of a sound that cannot start.
fn transport_run(editor: &mut EditorWindow, transport: Transport, now: Instant) -> Option<String> {
    let end = editor.end();
    let playback = &mut editor.playback;
    let view = &mut editor.view;
    match transport {
        Transport::PlayPause if playback.is_playing() => {
            view.playhead = playback.pause(now);
            None
        }
        Transport::PlayPause => {
            if view.playhead >= end {
                view.playhead = dc_core::OutputTime::ZERO;
            }
            playback.play(view.playhead, now)
        }
        Transport::Step(frames) => {
            if playback.is_playing() {
                view.playhead = playback.pause(now);
            }
            view.playhead = seek::step(view.playhead, frames, editor.fps, end);
            None
        }
        Transport::Scrub(at) => {
            if playback.is_playing() {
                playback.pause(now);
            }
            view.playhead = at;
            None
        }
        Transport::Release(resume) => (resume && view.playhead < end)
            .then(|| playback.play(view.playhead, now))
            .flatten(),
        Transport::Jump(forward) => {
            let playing = playback.is_playing();
            let at = if playing {
                playback.pause(now)
            } else {
                view.playhead
            };
            view.playhead = seek::jump(at, forward, &editor.tracks.keys, end);
            (playing && view.playhead < end)
                .then(|| playback.play(view.playhead, now))
                .flatten()
        }
    }
}

/// Runs `add`, which adds regions (its notes on success, an error otherwise), and picks what
/// it added; true when it did.
fn adds(
    editor: &mut EditorWindow,
    now: Instant,
    errors: &mut Vec<String>,
    add: impl FnOnce(&mut EditorWindow) -> Result<Vec<String>, String>,
) -> bool {
    let before = items(editor);
    match add(editor) {
        Ok(note) => {
            errors.extend(editor.refresh(now));
            errors.extend(note);
            pick_new(editor, &before);
            true
        }
        Err(error) => {
            errors.push(error);
            false
        }
    }
}

/// Picks the key of annotation `id` at the playhead, alone.
pub fn pick_key(editor: &mut EditorWindow, id: AnnotId) {
    let playhead = editor.view.playhead;
    let key = editor
        .tracks
        .rows
        .iter()
        .flat_map(|t| &t.regions)
        .filter(|r| r.item == Some(Item::of(id)))
        .find_map(|r| r.keys.iter().copied().find(|k| same_moment(*k, playhead)));
    if let Some(at) = key {
        editor.view.selection.only(Selected::Key { id, at });
    }
}

/// The regions the timeline shows now.
fn items(editor: &EditorWindow) -> Vec<Item> {
    editor
        .tracks
        .rows
        .iter()
        .flat_map(|t| &t.regions)
        .filter_map(|r| r.item)
        .collect()
}

/// Picks the regions that are not among `before`.
fn pick_new(editor: &mut EditorWindow, before: &[Item]) {
    let added: Vec<Item> = items(editor)
        .into_iter()
        .filter(|i| !before.contains(i))
        .collect();
    editor.view.selection.clear();
    for item in added {
        editor.view.selection.add(Selected::Region(item));
    }
}

/// Does what the «Нейронка» tab asked; true when the project changed («Отменить всё»).
fn ask_ai(
    ui: &Ui,
    editor: &mut EditorWindow,
    ask: ai::AiAsk,
    now: Instant,
    errors: &mut Vec<String>,
) -> bool {
    match ask {
        ai::AiAsk::Run => errors.extend(editor.ai.start(ui.ctx(), editor.doc.dir().root()).err()),
        ai::AiAsk::Stop => editor.ai.stop(now),
        ai::AiAsk::Restart => editor.ai.restart(),
        ai::AiAsk::LookAgain => errors.extend(editor.ai.look_again(ui.ctx()).err()),
        ai::AiAsk::Login => errors.extend(editor.ai.login(ui.ctx()).err()),
        ai::AiAsk::SendCode => errors.extend(editor.ai.send_code().err()),
        ai::AiAsk::CancelLogin => editor.ai.cancel_login(),
        ai::AiAsk::OpenSkill => {
            if let Some(folder) = &editor.ai.skill {
                errors.extend(crate::open_folder(folder).err().map(|e| e.to_string()));
            }
        }
        ai::AiAsk::UndoAll => {
            let ours =
                editor.ai.group.is_some() && editor.doc.top_group() == editor.ai.group.as_deref();
            return ours && editor.doc.travel(Way::Undo, now).is_some();
        }
    }
    false
}
