//! Time, ids, geometry, Project, Effect and Command traits, history, time map. No IO, no GPU.

pub mod arg;
pub mod color;
pub mod command;
pub mod diff;
pub mod effect;
pub mod geom;
pub mod history;
pub mod id;
pub mod item;
pub mod project;
pub mod time;
pub mod timemap;

#[cfg(test)]
mod test_util;

pub use arg::{RangeArg, RangesArg, TimeArg};
pub use color::{ChromaSiting, ColorMatrix, ColorRange, VideoColor};
pub use command::{Command, CommandError, Op, OpInfo};
pub use diff::Diff;
pub use effect::{Effect, EffectInfo, EffectRegistry, StateError};
pub use geom::{PixelSize, Point, Rect};
pub use history::{Change, History};
pub use id::{Id, IdCounter, IdKind};
pub use item::Item;
pub use project::{Clip, ClipId, Project};
pub use time::{OutputTime, Range, SourceTime, TimeDelta};
pub use timemap::TimeMap;
