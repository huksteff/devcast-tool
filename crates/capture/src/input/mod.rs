//! Input telemetry of a recording on the host clock (decision 009): mouse events, cursor
//! shapes and pictures in `cursor.bin` + `cursor-NNNN.png`, read back by [`read_cursor`];
//! shortcuts in `keys.bin` ([`read_keys`]), after the privacy filter [`KeyFilter`].

mod cursor;
mod file;
mod image;
mod keys;
mod kind;
mod shapes;
mod writer;

pub use cursor::{Button, CURSOR_FILE, CursorEvent, CursorKind, image_file, read_cursor};
pub use file::{FLUSH, Timed};
pub use image::CursorImage;
pub use keys::{KEYS_FILE, KeyEvent, KeyFilter, Modifiers, read_keys};
pub use shapes::SeenShape;
pub use writer::{EncodePng, InputSummary};
pub(crate) use writer::{InputMessage, write};
