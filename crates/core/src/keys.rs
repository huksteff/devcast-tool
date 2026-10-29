//! Shortcuts of the recordings, as the keys effect reads them (decision 015, «Телеметрия
//! курсора»). `keys.bin` belongs to `dc-capture`; the top layer reads it into these types and
//! hands them to `dc_ops::Evaluator`, because effects see only the core.

use std::collections::BTreeMap;

use crate::time::SourceTime;

/// Shortcuts of every recording on the track, by `Media::path` of its video, each list in time
/// order.
pub type KeyTracks = BTreeMap<String, Vec<KeyPress>>;

/// A shortcut or special key pressed during a recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPress {
    pub at: SourceTime,
    /// How it reads on the keyboard: `Ctrl+Shift+P`, `F5`.
    pub name: String,
}
