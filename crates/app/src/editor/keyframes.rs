//! I (7.25, v1): a key of the picked annotation at the playhead (`annot key add`, in either key
//! mode), picked; a key already there only gets picked. Outside the annotation the command
//! refuses and its toast says why.

use dc_ui_kit::lane::Item;

use super::{Action, EditorWindow, lanes, stage};

pub fn add(editor: &mut EditorWindow) {
    let Some(id) = stage::picked(editor) else {
        return;
    };
    let item = Item::of(id);
    let step = lanes::of(item).and_then(|lane| lane.key_added(item, editor.view.playhead));
    if let Some(step) = step {
        editor.ask(Action::Keyed(vec![step], id));
    }
}
