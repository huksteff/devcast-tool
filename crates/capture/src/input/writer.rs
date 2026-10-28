//! The thread that writes the input events of a recording into its folder.
//!
//! Hooks start before the video and send events from then on; until [`InputMessage::Begin`]
//! (and for events older than the start) only the pointer position and the cursor shape are
//! kept, and they are written at the start time, so the file always begins with them. A
//! cursor picture is saved as PNG when a shape first shows it, before its records. Shortcuts
//! (already filtered in the hook) go to their own file from the start on. The thread ends
//! when every sender is gone: the hooks stopped and the recording dropped its handle.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Instant;

use super::cursor::{CURSOR_FILE, CursorEvent, image_file};
use super::file::{FLUSH, RecordWriter, Timed};
use super::image::CursorImage;
use super::keys::{KEYS_FILE, KeyEvent};
use super::shapes::{SeenShape, Shapes};
use crate::error::CaptureError;
use crate::event::HostTime;

pub enum InputMessage {
    Mouse(Timed<CursorEvent>),
    /// The cursor shape changed (or this is the first look at it).
    Shape(Timed<SeenShape>),
    /// A shortcut that passed the privacy filter.
    Key(Timed<KeyEvent>),
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
    pub shapes: u64,
    /// Distinct cursor pictures saved.
    pub images: u64,
    /// Shortcuts written.
    pub keys: u64,
}

/// Encodes a cursor picture as PNG.
pub type EncodePng = fn(&CursorImage) -> Result<Vec<u8>, CaptureError>;

struct Open {
    start: HostTime,
    dir: PathBuf,
    records: RecordWriter<Timed<CursorEvent>>,
    keys: RecordWriter<Timed<KeyEvent>>,
}

struct Writer {
    file: Option<Open>,
    png: EncodePng,
    position: Option<(i32, i32)>,
    written: Option<(i32, i32)>,
    shapes: Shapes,
    summary: InputSummary,
}

/// Writes the events of `messages` until the channel closes.
pub fn write(
    messages: &Receiver<InputMessage>,
    png: EncodePng,
) -> Result<InputSummary, CaptureError> {
    let mut writer = Writer {
        file: None,
        png,
        position: None,
        written: None,
        shapes: Shapes::default(),
        summary: InputSummary::default(),
    };
    let mut flushed = Instant::now();
    loop {
        match messages.recv_timeout(FLUSH) {
            Ok(InputMessage::Mouse(event)) => writer.mouse(event)?,
            Ok(InputMessage::Shape(shape)) => writer.shape(shape)?,
            Ok(InputMessage::Key(key)) => writer.key(&key)?,
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
        let records = RecordWriter::create(&dir.join(CURSOR_FILE))?;
        let keys = RecordWriter::create(&dir.join(KEYS_FILE))?;
        self.file = Some(Open {
            start,
            dir,
            records,
            keys,
        });
        if let Some((x, y)) = self.position {
            self.put(Timed {
                at: start,
                event: CursorEvent::Move { x, y },
            })?;
        }
        self.put_shape(start)
    }

    /// The event is part of the recording: it began, and not later than the event.
    fn records(&self, at: HostTime) -> bool {
        self.file.as_ref().is_some_and(|open| at >= open.start)
    }

    fn mouse(&mut self, event: Timed<CursorEvent>) -> Result<(), CaptureError> {
        if self.records(event.at) {
            self.put(event)
        } else {
            self.position = event.event.position();
            Ok(())
        }
    }

    fn key(&mut self, key: &Timed<KeyEvent>) -> Result<(), CaptureError> {
        match &mut self.file {
            Some(open) if key.at >= open.start => {
                self.summary.keys += 1;
                open.keys.write(key)
            }
            _ => Ok(()),
        }
    }

    fn shape(&mut self, shape: Timed<SeenShape>) -> Result<(), CaptureError> {
        self.shapes.see(shape.event)?;
        if self.records(shape.at) {
            self.put_shape(shape.at)?;
        }
        Ok(())
    }

    /// Writes the current shape if it is not the written one, its new picture first.
    fn put_shape(&mut self, at: HostTime) -> Result<(), CaptureError> {
        let Some(open) = &mut self.file else {
            return Ok(());
        };
        let Some(change) = self.shapes.change() else {
            return Ok(());
        };
        if let Some((id, image)) = change.new_image {
            save(&open.dir, id, &(self.png)(image)?)?;
            let (x, y) = image.hotspot;
            let event = CursorEvent::Image {
                id,
                x: x as i32,
                y: y as i32,
            };
            open.records.write(&Timed { at, event })?;
            self.summary.images += 1;
        }
        open.records.write(&Timed {
            at,
            event: change.shape,
        })?;
        self.summary.shapes += 1;
        Ok(())
    }

    /// Writes an event of the recording; a move to where the pointer already is is skipped.
    fn put(&mut self, event: Timed<CursorEvent>) -> Result<(), CaptureError> {
        let position = event.event.position();
        let counter = match event.event {
            CursorEvent::Move { .. } if position == self.written => return Ok(()),
            CursorEvent::Move { .. } => &mut self.summary.moves,
            CursorEvent::Down { .. } => &mut self.summary.downs,
            CursorEvent::Up { .. } => &mut self.summary.ups,
            CursorEvent::Wheel { .. } | CursorEvent::HWheel { .. } => &mut self.summary.wheels,
            CursorEvent::Image { .. } | CursorEvent::Shape { .. } => return Ok(()),
        };
        *counter += 1;
        self.position = position;
        self.written = position;
        if let Some(open) = &mut self.file {
            open.records.write(&event)?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), CaptureError> {
        match &mut self.file {
            Some(open) => open.records.flush().and_then(|()| open.keys.flush()),
            None => Ok(()),
        }
    }
}

fn save(dir: &Path, id: u32, png: &[u8]) -> Result<(), CaptureError> {
    let path = dir.join(image_file(id));
    std::fs::write(&path, png).map_err(CaptureError::file("записать", &path))
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
