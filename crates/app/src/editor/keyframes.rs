//! I (7.25, v1): a key of the picked annotation at the playhead (`annot key add`, in either key
//! mode), picked; a key already there only gets picked. Outside the annotation the command
//! refuses and its toast says why.

use dc_ops::Step;
use serde_json::{Map, Value, json};

use super::montage::arg;
use super::{Action, EditorWindow, stage};

pub fn add(editor: &mut EditorWindow) {
    let Some(id) = stage::picked(editor) else {
        return;
    };
    let mut args = Map::new();
    args.insert("id".to_owned(), json!(id));
    args.insert("at".to_owned(), Value::String(arg(editor.view.playhead)));
    editor.ask(Action::Keyed(vec![Step::new("annot key add", args)], id));
}
