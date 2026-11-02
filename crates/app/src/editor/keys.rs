//! Editor shortcuts at work: the pressed shortcut is looked up in the user's table
//! (`keymap::EditorKeys`, by physical key) and its action runs. An action whose feature has not
//! landed yet has no handler here: the help and the settings show it grey, and its task adds
//! the arm to [`act`].

use dc_ui_kit::strings::keys as text;
use eframe::egui::{self, Ui};

use super::view::Tool;
use super::{Action, EditorWindow, Transport, Way};
use super::{adding, clipboard, keyframes, montage};
use crate::keymap::{EditorKey, EditorKeys};
use crate::shortcut::Shortcut;

/// What a working shortcut does.
#[derive(Debug, Clone, Copy)]
enum Act {
    Travel(Way),
    Tool(Tool),
    Snap,
    Help,
    Transport(Transport),
    Montage(fn(&mut EditorWindow)),
}

/// The action of `key`, `None` until its task: projects 7.28.
fn act(key: EditorKey) -> Option<Act> {
    Some(match key {
        EditorKey::Undo => Act::Travel(Way::Undo),
        EditorKey::Redo | EditorKey::RedoAlt => Act::Travel(Way::Redo),
        EditorKey::Select => Act::Tool(Tool::Select),
        EditorKey::Blade => Act::Tool(Tool::Blade),
        EditorKey::Snap => Act::Snap,
        EditorKey::Help => Act::Help,
        EditorKey::PlayPause => Act::Transport(Transport::PlayPause),
        EditorKey::PrevFrame => Act::Transport(Transport::Step(-1)),
        EditorKey::NextFrame => Act::Transport(Transport::Step(1)),
        EditorKey::PrevKey => Act::Transport(Transport::Jump(false)),
        EditorKey::NextKey => Act::Transport(Transport::Jump(true)),
        EditorKey::CutAtPlayhead => Act::Montage(montage::cut_at_playhead),
        EditorKey::CutRange => Act::Montage(montage::cut_range),
        EditorKey::RangeSpeed => Act::Montage(montage::range_speed),
        EditorKey::Delete => Act::Montage(clipboard::delete),
        EditorKey::Copy => Act::Montage(clipboard::copy),
        EditorKey::Cut => Act::Montage(clipboard::cut),
        EditorKey::Paste => Act::Montage(clipboard::paste),
        EditorKey::SelectAll => Act::Montage(clipboard::select_all),
        EditorKey::AddZoom => Act::Montage(adding::zoom),
        EditorKey::AddAnnotation => Act::Montage(adding::annotation),
        EditorKey::NextAnnotation => Act::Montage(adding::next_annotation),
        EditorKey::Keyframe => Act::Montage(keyframes::add),
        EditorKey::NewProject | EditorKey::OpenProject | EditorKey::Save | EditorKey::SaveAs => {
            return None;
        }
    })
}

/// The shortcut of `key` does something now.
pub fn wired(key: EditorKey) -> bool {
    act(key).is_some()
}

/// Runs the shortcuts pressed this frame. None while a text field takes the keys, the crop
/// dialog or the question of closing is open; while the help is open only its own shortcut (it closes the help). A shortcut
/// takes the keyboard focus from a widget: Tab (the next annotation) would otherwise move it to
/// a button, and Space would press that button too.
pub fn run(ui: &Ui, editor: &mut EditorWindow) {
    let asking = editor.closing == super::closing::Closing::Asking;
    if ui.ctx().text_edit_focused() || editor.crop.is_some() || asking {
        return;
    }
    // Esc is not in the table: it always steps back (mockup 2b, 2c).
    if editor.help.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        match editor.library.open {
            true => editor.library.open = false,
            false => montage::escape(editor),
        }
    }
    let pressed: Vec<(Shortcut, bool)> = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|event| {
                let repeat = matches!(event, egui::Event::Key { repeat: true, .. });
                Some((Shortcut::of_event(event, i.modifiers)?, repeat))
            })
            .collect()
    });
    for (shortcut, repeat) in pressed {
        let Some(key) = editor.keys.find(shortcut) else {
            continue;
        };
        if editor.help.is_some() && key != EditorKey::Help {
            continue;
        }
        // Holding the play key does not toggle again and again (v1).
        if repeat && key == EditorKey::PlayPause {
            continue;
        }
        ui.ctx().memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        match act(key) {
            Some(Act::Travel(way)) => editor.ask(Action::Travel(way)),
            Some(Act::Tool(tool)) => editor.view.tool = tool,
            Some(Act::Snap) => editor.view.magnet = !editor.view.magnet,
            Some(Act::Help) => editor.toggle_help(),
            Some(Act::Transport(t)) => editor.ask(Action::Transport(t)),
            Some(Act::Montage(run)) => run(editor),
            None => {}
        }
    }
}

/// A tooltip with the shortcut of `key`: «Выбор · A»; just `title` when it has none.
pub fn hint(title: &str, key: EditorKey, keys: &EditorKeys) -> String {
    match keys.label(key) {
        Some(label) => text::with_keys(title, &label),
        None => title.to_owned(),
    }
}
