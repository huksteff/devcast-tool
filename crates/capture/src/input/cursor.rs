//! `cursor.bin`: mouse events of a recording.
//!
//! Record, 24 bytes little-endian:
//!
//! | Bytes | Field |
//! |---|---|
//! | 0..8 | time, i64 µs on the host clock |
//! | 8 | kind: 0 move, 1 button down, 2 button up, 3 wheel, 4 horizontal wheel |
//! | 9 | button (down/up): 1 left, 2 right, 3 middle, 4 X1 (back), 5 X2 (forward); else 0 |
//! | 10..12 | 0 |
//! | 12..16, 16..20 | x, y: i32, physical desktop pixels (left of the main monitor is negative) |
//! | 20..24 | i32: wheel delta (120 per notch; + away from the user / to the right), else 0 |

use std::path::Path;

use super::file::{Record, Timed, read_records};
use crate::error::CaptureError;
use crate::event::HostTime;

/// File name in the recording folder.
pub const CURSOR_FILE: &str = "cursor.bin";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    Left,
    Right,
    Middle,
    /// The «back» side button.
    X1,
    /// The «forward» side button.
    X2,
}

/// A mouse event at desktop pixel (`x`, `y`).
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
}

impl CursorEvent {
    /// Where the pointer is during the event.
    pub fn position(self) -> (i32, i32) {
        match self {
            Self::Move { x, y }
            | Self::Down { x, y, .. }
            | Self::Up { x, y, .. }
            | Self::Wheel { x, y, .. }
            | Self::HWheel { x, y, .. } => (x, y),
        }
    }

    fn fields(self) -> (u8, u8, i32) {
        match self {
            Self::Move { .. } => (0, 0, 0),
            Self::Down { button, .. } => (1, button.code(), 0),
            Self::Up { button, .. } => (2, button.code(), 0),
            Self::Wheel { delta, .. } => (3, 0, delta),
            Self::HWheel { delta, .. } => (4, 0, delta),
        }
    }
}

impl Button {
    fn code(self) -> u8 {
        match self {
            Self::Left => 1,
            Self::Right => 2,
            Self::Middle => 3,
            Self::X1 => 4,
            Self::X2 => 5,
        }
    }

    fn from_code(code: u8) -> Result<Self, String> {
        Ok(match code {
            1 => Self::Left,
            2 => Self::Right,
            3 => Self::Middle,
            4 => Self::X1,
            5 => Self::X2,
            _ => return Err(format!("неизвестная кнопка мыши {code}")),
        })
    }
}

impl Record for Timed<CursorEvent> {
    const MAGIC: [u8; 4] = *b"DCCU";
    const VERSION: u16 = 1;
    const SIZE: usize = 24;

    fn encode(&self, out: &mut Vec<u8>) {
        let (kind, button, value) = self.event.fields();
        let (x, y) = self.event.position();
        out.extend_from_slice(&self.at.micros().to_le_bytes());
        out.extend_from_slice(&[kind, button, 0, 0]);
        for field in [x, y, value] {
            out.extend_from_slice(&field.to_le_bytes());
        }
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let i32_at =
            |i: usize| i32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        let mut time = [0; 8];
        time.copy_from_slice(&bytes[..8]);
        let (x, y, value) = (i32_at(12), i32_at(16), i32_at(20));
        let event = match bytes[8] {
            0 => CursorEvent::Move { x, y },
            1 => CursorEvent::Down {
                button: Button::from_code(bytes[9])?,
                x,
                y,
            },
            2 => CursorEvent::Up {
                button: Button::from_code(bytes[9])?,
                x,
                y,
            },
            3 => CursorEvent::Wheel { x, y, delta: value },
            4 => CursorEvent::HWheel { x, y, delta: value },
            kind => return Err(format!("неизвестный тип события мыши {kind}")),
        };
        Ok(Self {
            at: HostTime::from_micros(i64::from_le_bytes(time)),
            event,
        })
    }
}

/// Mouse events of the recording in folder `dir`, in time order.
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
    }
}
