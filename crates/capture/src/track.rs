//! Writing the encoded video track to disk.
//!
//! A frame lasts until the next one (variable frame rate), so each packet is held until the
//! next arrives and only then written with its duration.

use std::path::Path;

use dc_encode::{Mp4Writer, Packet, VideoParameters};

use crate::error::CaptureError;

pub struct VideoTrackWriter {
    file: Mp4Writer,
    held: Option<Packet>,
}

impl VideoTrackWriter {
    pub fn create(path: &Path, video: &VideoParameters) -> Result<Self, CaptureError> {
        Ok(Self {
            file: Mp4Writer::create(path, video)?,
            held: None,
        })
    }

    pub fn push(&mut self, packet: Packet) -> Result<(), CaptureError> {
        let next = packet.pts();
        if let Some(mut previous) = self.held.replace(packet) {
            previous.set_duration(next - previous.pts());
            self.file.write(previous)?;
        }
        Ok(())
    }

    /// Writes the last frame lasting until slot `end` and closes the file.
    pub fn finish(mut self, end: i64) -> Result<(), CaptureError> {
        if let Some(mut last) = self.held.take() {
            last.set_duration(end - last.pts());
            self.file.write(last)?;
        }
        Ok(self.file.finish()?)
    }
}
