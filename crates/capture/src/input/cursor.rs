//! `cursor.bin`: mouse events and cursor shapes of a recording.
//!
//! Record, 24 bytes little-endian:
//!
//! | Bytes | Field |
//! |---|---|
//! | 0..8 | time, i64 µs on the host clock |
//! | 8 | event: 0 move, 1 button down, 2 button up, 3 wheel, 4 horizontal wheel, 5 image, 6 shape |
//! | 9 | button (down/up): 1 left, 2 right, 3 middle, 4 X1 (back), 5 X2 (forward); shape: [`CursorKind`] code; else 0 |
//! | 10 | shape: 1 visible, 0 hidden; else 0 |
//! | 11 | 0 |
//! | 12..16, 16..20 | x, y: i32. Mouse events: pixels of the recorded picture from its top-left corner (physical desktop pixels minus the source's corner; outside the picture they are negative or beyond its size). Image: hotspot in the picture's pixels. Else 0 |
//! | 20..24 | i32: wheel delta (120 per notch; + away from the user / to the right); image and visible shape: image id; else 0 |
//!
//! An image record comes before the first shape that shows it; its picture is the PNG
//! [`image_file`] in the same folder, written before the record.

use std::path::Path;

use super::file::{Record, Timed, read_records};
pub use super::kind::{Button, CursorKind};
use crate::error::CaptureError;
use crate::event::HostTime;

/// File name in the recording folder.
pub const CURSOR_FILE: &str = "cursor.bin";

/// File name of cursor image `id` in the recording folder.
pub fn image_file(id: u32) -> String {
    format!("cursor-{id:04}.png")
}

/// A mouse event at desktop pixel (`x`, `y`), or a change of the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorEvent {
    Move {
        x: i32,
        y: i32,
    },
    Down {
        button: Button,
        x: i32,
        y: i32,
    },
    Up {
        button: Button,
        x: i32,
        y: i32,
    },
    /// Vertical wheel: 120 per notch, positive away from the user.
    Wheel {
        x: i32,
        y: i32,
        delta: i32,
    },
    /// Horizontal wheel: 120 per notch, positive to the right.
    HWheel {
        x: i32,
        y: i32,
        delta: i32,
    },
    /// Cursor picture `id` ([`image_file`]); its hotspot is at (`x`, `y`) of the picture.
    Image {
        id: u32,
        x: i32,
        y: i32,
    },
    /// From now on the cursor is `kind` with picture `image`, or hidden (`None`).
    Shape {
        kind: CursorKind,
        image: Option<u32>,
    },
}

/// Fields of a record after the time.
struct Fields {
    event: u8,
    code: u8,
    visible: bool,
    x: i32,
    y: i32,
    value: i32,
}

impl CursorEvent {
    /// Where the pointer is during a mouse event; `None` for image and shape.
    pub fn position(self) -> Option<(i32, i32)> {
        match self {
            Self::Move { x, y }
            | Self::Down { x, y, .. }
            | Self::Up { x, y, .. }
            | Self::Wheel { x, y, .. }
            | Self::HWheel { x, y, .. } => Some((x, y)),
            Self::Image { .. } | Self::Shape { .. } => None,
        }
    }

    /// The same event with the pointer in pixels of a picture whose top-left corner is at
    /// desktop pixel `origin`. Image and shape stay as they are.
    pub fn relative_to(self, (ox, oy): (i32, i32)) -> Self {
        let (dx, dy) = (-ox, -oy);
        match self {
            Self::Move { x, y } => Self::Move {
                x: x + dx,
                y: y + dy,
            },
            Self::Down { button, x, y } => Self::Down {
                button,
                x: x + dx,
                y: y + dy,
            },
            Self::Up { button, x, y } => Self::Up {
                button,
                x: x + dx,
                y: y + dy,
            },
            Self::Wheel { x, y, delta } => Self::Wheel {
                x: x + dx,
                y: y + dy,
                delta,
            },
            Self::HWheel { x, y, delta } => Self::HWheel {
                x: x + dx,
                y: y + dy,
                delta,
            },
            Self::Image { .. } | Self::Shape { .. } => self,
        }
    }

    fn fields(self) -> Fields {
        let at = |event, code, x, y, value| Fields {
            event,
            code,
            visible: false,
            x,
            y,
            value,
        };
        match self {
            Self::Move { x, y } => at(0, 0, x, y, 0),
            Self::Down { button, x, y } => at(1, button.code(), x, y, 0),
            Self::Up { button, x, y } => at(2, button.code(), x, y, 0),
            Self::Wheel { x, y, delta } => at(3, 0, x, y, delta),
            Self::HWheel { x, y, delta } => at(4, 0, x, y, delta),
            Self::Image { id, x, y } => at(5, 0, x, y, id as i32),
            Self::Shape { kind, image } => Fields {
                visible: image.is_some(),
                ..at(6, kind.code(), 0, 0, image.map_or(0, |id| id as i32))
            },
        }
    }

    fn from_fields(f: &Fields) -> Result<Self, String> {
        let (x, y) = (f.x, f.y);
        Ok(match f.event {
            0 => Self::Move { x, y },
            1 => Self::Down {
                button: Button::from_code(f.code)?,
                x,
                y,
            },
            2 => Self::Up {
                button: Button::from_code(f.code)?,
                x,
                y,
            },
            3 => Self::Wheel {
                x,
                y,
                delta: f.value,
            },
            4 => Self::HWheel {
                x,
                y,
                delta: f.value,
            },
            5 => Self::Image {
                id: f.value as u32,
                x,
                y,
            },
            6 => Self::Shape {
                kind: CursorKind::from_code(f.code)?,
                image: f.visible.then_some(f.value as u32),
            },
            event => return Err(format!("неизвестный тип события мыши {event}")),
        })
    }
}

impl Record for Timed<CursorEvent> {
    const MAGIC: [u8; 4] = *b"DCCU";
    const VERSION: u16 = 1;
    const SIZE: usize = 24;

    fn encode(&self, out: &mut Vec<u8>) {
        let f = self.event.fields();
        out.extend_from_slice(&self.at.micros().to_le_bytes());
        out.extend_from_slice(&[f.event, f.code, u8::from(f.visible), 0]);
        for field in [f.x, f.y, f.value] {
            out.extend_from_slice(&field.to_le_bytes());
        }
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let i32_at =
            |i: usize| i32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        let mut time = [0; 8];
        time.copy_from_slice(&bytes[..8]);
        let fields = Fields {
            event: bytes[8],
            code: bytes[9],
            visible: bytes[10] == 1,
            x: i32_at(12),
            y: i32_at(16),
            value: i32_at(20),
        };
        Ok(Self {
            at: HostTime::from_micros(i64::from_le_bytes(time)),
            event: CursorEvent::from_fields(&fields)?,
        })
    }
}

/// Mouse events and cursor shapes of the recording in folder `dir`, in time order.
pub fn read_cursor(dir: &Path) -> Result<Vec<Timed<CursorEvent>>, CaptureError> {
    read_records(&dir.join(CURSOR_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::file::RecordWriter;

    #[test]
    fn every_event_round_trips_through_the_file() {
        let events = [
            CursorEvent::Move { x: -1920, y: 5 },
            CursorEvent::Down {
                button: Button::Left,
                x: 1,
                y: 2,
            },
            CursorEvent::Up {
                button: Button::X2,
                x: 3,
                y: 4,
            },
            CursorEvent::Wheel {
                x: 0,
                y: 0,
                delta: -240,
            },
            CursorEvent::HWheel {
                x: 7,
                y: 8,
                delta: 120,
            },
            CursorEvent::Image { id: 3, x: 6, y: 1 },
            CursorEvent::Shape {
                kind: CursorKind::Text,
                image: Some(3),
            },
            CursorEvent::Shape {
                kind: CursorKind::Custom,
                image: None,
            },
        ];
        let timed: Vec<_> = events
            .iter()
            .enumerate()
            .map(|(i, &event)| Timed {
                at: HostTime::from_micros(1_000_000_000 + i as i64),
                event,
            })
            .collect();
        let dir = tempfile::tempdir().unwrap();
        let mut writer = RecordWriter::create(&dir.path().join(CURSOR_FILE)).unwrap();
        for event in &timed {
            writer.write(event).unwrap();
        }
        writer.flush().unwrap();
        drop(writer);
        let bytes = std::fs::read(dir.path().join(CURSOR_FILE)).unwrap();
        assert_eq!(bytes.len(), 8 + 24 * events.len());
        assert_eq!(&bytes[..8], b"DCCU\x01\x00\x18\x00");
        assert_eq!(read_cursor(dir.path()).unwrap(), timed);
    }

    #[test]
    fn unknown_kind_or_button_is_an_error() {
        let mut bytes = Vec::new();
        Timed {
            at: HostTime::from_micros(0),
            event: CursorEvent::Move { x: 0, y: 0 },
        }
        .encode(&mut bytes);
        bytes[8] = 200;
        assert!(Timed::<CursorEvent>::decode(&bytes).is_err());
        bytes[8] = 1;
        bytes[9] = 9;
        assert!(Timed::<CursorEvent>::decode(&bytes).is_err());
        bytes[8] = 6;
        bytes[9] = 14;
        assert!(Timed::<CursorEvent>::decode(&bytes).is_err());
    }
}
