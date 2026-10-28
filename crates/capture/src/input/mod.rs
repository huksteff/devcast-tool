//! Input telemetry of a recording on the host clock (decision 009): mouse events, cursor
//! shapes and pictures in `cursor.bin` + `cursor-NNNN.png`, read back by [`read_cursor`];
//! shortcuts in `keys.bin` ([`read_keys`]), after the privacy filter [`KeyFilter`]. A finished
//! recording's events on its source time: [`Telemetry`].

mod cursor;
mod file;
mod image;
mod key_name;
mod keys;
mod kind;
mod shapes;
mod telemetry;
mod writer;

pub use cursor::{Button, CURSOR_FILE, CursorEvent, CursorKind, image_file, read_cursor};
pub use file::{FLUSH, Record, RecordWriter, Timed};
pub use image::CursorImage;
pub use keys::{KEYS_FILE, KeyEvent, KeyFilter, Modifiers, Stroke, read_keys};
pub use shapes::SeenShape;
pub use telemetry::{Click, DOUBLE_CLICK_DISTANCE, DOUBLE_CLICK_TIME, SourceEvent, Telemetry};
pub use writer::{EncodePng, InputSummary};
pub(crate) use writer::{InputMessage, write};
