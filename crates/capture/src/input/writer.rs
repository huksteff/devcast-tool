//! The thread that writes the input events of a recording into its folder.
//!
//! Hooks start before the video and send events from then on; until [`InputMessage::Begin`]
//! (and for events older than the start) only the pointer position is kept, and it is written
//! at the start time, so the file always begins with where the pointer is. The thread ends
//! when every sender is gone: the hooks stopped and the recording dropped its handle.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Instant;

use super::cursor::{CURSOR_FILE, CursorEvent};
use super::file::{FLUSH, RecordWriter, Timed};
use crate::error::CaptureError;
use crate::event::HostTime;

pub enum InputMessage {
    Mouse(Timed<CursorEvent>),
    /// The recording starts at this host time and is written into this folder.
    Begin(HostTime, PathBuf),
    /// A hook could not do its work; the recording of input ends with this error.
    Failed(CaptureError),
}

/// What was written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputSummary {
    pub moves: u64,
    pub downs: u64,
    pub ups: u64,
    /// Both wheels.
    pub wheels: u64,
}

struct Writer {
    file: Option<(HostTime, RecordWriter<Timed<CursorEvent>>)>,
    position: Option<(i32, i32)>,
    written: Option<(i32, i32)>,
    summary: InputSummary,
}

/// Writes the events of `messages` until the channel closes.
pub fn write(messages: &Receiver<InputMessage>) -> Result<InputSummary, CaptureError> {
    let mut writer = Writer {
        file: None,
        position: None,
        written: None,
        summary: InputSummary::default(),
    };
    let mut flushed = Instant::now();
    loop {
        match messages.recv_timeout(FLUSH) {
            Ok(InputMessage::Mouse(event)) => writer.mouse(event)?,
            Ok(InputMessage::Begin(start, dir)) => writer.begin(start, dir)?,
            Ok(InputMessage::Failed(error)) => return Err(error),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if flushed.elapsed() >= FLUSH {
            writer.flush()?;
            flushed = Instant::now();
        }
    }
    writer.flush()?;
    Ok(writer.summary)
}

impl Writer {
    fn begin(&mut self, start: HostTime, dir: PathBuf) -> Result<(), CaptureError> {
        let file = RecordWriter::create(&dir.join(CURSOR_FILE))?;
        self.file = Some((start, file));
        if let Some((x, y)) = self.position {
            self.put(Timed {
                at: start,
                event: CursorEvent::Move { x, y },
            })?;
        }
        Ok(())
    }

    fn mouse(&mut self, event: Timed<CursorEvent>) -> Result<(), CaptureError> {
        let begun = self
            .file
            .as_ref()
            .is_some_and(|(start, _)| event.at >= *start);
        if begun {
            self.put(event)
        } else {
            self.position = Some(event.event.position());
            Ok(())
        }
    }

    /// Writes an event of the recording; a move to where the pointer already is is skipped.
    fn put(&mut self, event: Timed<CursorEvent>) -> Result<(), CaptureError> {
        let position = Some(event.event.position());
        let counter = match event.event {
            CursorEvent::Move { .. } if position == self.written => return Ok(()),
            CursorEvent::Move { .. } => &mut self.summary.moves,
            CursorEvent::Down { .. } => &mut self.summary.downs,
            CursorEvent::Up { .. } => &mut self.summary.ups,
            CursorEvent::Wheel { .. } | CursorEvent::HWheel { .. } => &mut self.summary.wheels,
        };
        *counter += 1;
        self.position = position;
        self.written = position;
        if let Some((_, file)) = &mut self.file {
            file.write(&event)?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), CaptureError> {
        match &mut self.file {
            Some((_, file)) => file.flush(),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use crate::input::cursor::{Button, read_cursor};

    fn at(micros: i64, event: CursorEvent) -> InputMessage {
        InputMessage::Mouse(Timed {
            at: HostTime::from_micros(micros),
            event,
        })
    }

    #[test]
    fn starts_at_the_last_position_and_skips_still_moves() {
        let dir = tempfile::tempdir().unwrap();
        let (send, receive) = mpsc::channel();
        let (x, y, button) = (2, 2, Button::Left);
        let still = CursorEvent::Move { x, y };
        let down = CursorEvent::Down { button, x, y };
        let up = CursorEvent::Up { button, x, y };
        let wheel = CursorEvent::Wheel { x, y, delta: 120 };
        let moved = CursorEvent::Move { x: 4, y };
        for message in [
            at(10, CursorEvent::Move { x: 1, y: 1 }),
            at(20, still),
            InputMessage::Begin(HostTime::from_micros(100), dir.path().to_path_buf()),
            at(90, CursorEvent::Move { x: 3, y: 3 }),
            at(110, still),
            at(120, down),
            at(130, still),
            at(140, up),
            at(150, wheel),
            at(160, moved),
        ] {
            send.send(message).unwrap();
        }
        drop(send);
        let summary = write(&receive).unwrap();
        let expected = [
            (100, still),
            (120, down),
            (140, up),
            (150, wheel),
            (160, moved),
        ]
        .map(|(micros, event)| Timed {
            at: HostTime::from_micros(micros),
            event,
        });
        assert_eq!(read_cursor(dir.path()).unwrap(), expected);
        let counts = InputSummary {
            moves: 2,
            downs: 1,
            ups: 1,
            wheels: 1,
        };
        assert_eq!(summary, counts);
    }

    #[test]
    fn a_hook_failure_ends_the_writer_with_its_error() {
        let (send, receive) = mpsc::channel();
        let error = CaptureError::platform("прочитать системные часы")(
            "нет".to_owned(),
        );
        send.send(InputMessage::Failed(error)).unwrap();
        assert!(matches!(
            write(&receive),
            Err(CaptureError::Platform { .. })
        ));
    }
}
