//! Input telemetry of a recording on the host clock (decision 009): mouse events, cursor
//! shapes and pictures in `cursor.bin` + `cursor-NNNN.png`, read back by [`read_cursor`].

mod cursor;
mod file;
mod image;
mod kind;
mod shapes;
mod writer;

pub use cursor::{Button, CURSOR_FILE, CursorEvent, CursorKind, image_file, read_cursor};
pub use file::{FLUSH, Timed};
pub use image::CursorImage;
pub use shapes::SeenShape;
pub use writer::{EncodePng, InputSummary};
pub(crate) use writer::{InputMessage, write};
