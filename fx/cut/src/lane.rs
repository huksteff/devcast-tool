//! The main track in the editor (`dc_ui_kit::lane`): the commands of its gestures — the blade,
//! Delete, R, a dragged clip, a cut range. The editor draws and picks the clips itself.

use dc_core::OpName;

pub const CLIP_SPLIT: OpName = OpName::of::<crate::ClipSplit>();
pub const CLIP_MOVE: OpName = OpName::of::<crate::ClipMove>();
pub const CLIP_RM: OpName = OpName::of::<crate::ClipRm>();
pub const CUT: OpName = OpName::of::<crate::Cut>();
pub const SPEED: OpName = OpName::of::<crate::Speed>();
