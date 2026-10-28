//! Input telemetry of a recording on the host clock (decision 009): mouse events in
//! `cursor.bin`, read back by [`read_cursor`].

mod cursor;
mod file;
mod writer;

pub use cursor::{Button, CURSOR_FILE, CursorEvent, read_cursor};
pub use file::{FLUSH, Timed};
pub use writer::InputSummary;
pub(crate) use writer::{InputMessage, write};
