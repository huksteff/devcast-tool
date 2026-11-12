//! Time, ids, geometry, Project, Effect and Command traits, history, time map. No IO, no GPU.

pub mod arg;
pub mod clip_audio;
pub mod color;
pub mod command;
pub mod cursor;
pub mod diff;
pub mod effect;
pub mod geom;
pub mod history;
pub mod id;
pub mod item;
pub mod keys;
pub mod project;
pub mod span;
pub mod step;
pub mod time;
pub mod timemap;

#[cfg(test)]
mod test_util;

pub use arg::{RangeArg, RangesArg, TimeArg};
pub use clip_audio::{ChannelLevels, ClipAudio};
pub use color::{ChromaSiting, ColorMatrix, ColorRange, VideoColor};
pub use command::{Command, CommandError, Op, OpInfo, index_of, within};
pub use cursor::{CursorPicture, CursorPoint, CursorShape, CursorTrack, CursorTracks};
pub use diff::{Changes, Diff, on_off};
pub use effect::{Effect, EffectInfo, EffectRegistry, SettingsArgs, StateError};
pub use geom::{PixelSize, Point, Rect};
pub use history::{Change, History};
pub use id::{Id, IdCounter, IdKind};
pub use item::{Item, Place};
pub use keys::{KeyPress, KeyTracks};
pub use project::{Clip, ClipId, Project};
pub use step::{OpName, Step};
pub use time::{OutputTime, Range, SourceTime, TimeDelta};
pub use timemap::TimeMap;
