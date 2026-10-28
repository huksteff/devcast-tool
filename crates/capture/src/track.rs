//! Writing one encoded track (video or sound) as fMP4 segments listed in `manifest.json`.
//!
//! A segment starts with a key packet and closes at the first key packet [`SEGMENT_SECONDS`] or
//! more after its start; then the manifest is rewritten with it. A video frame lasts until the
//! next one (variable frame rate), so each packet is held until the next arrives and only then
//! written with its duration. Packets in a segment have times from 0.

use dc_encode::{Mp4Layout, Mp4Writer, Packet, StreamParameters};

use crate::error::CaptureError;
use crate::manifest::{Segment, SharedManifest, Track};

/// At most this much is lost when the process dies (plus up to a second to the next key frame).
pub const SEGMENT_SECONDS: i64 = 2;

pub struct TrackWriter {
    manifest: SharedManifest,
    track: Track,
    stream: StreamParameters,
    segment_ticks: i64,
    current: Option<OpenSegment>,
    held: Option<Packet>,
}

struct OpenSegment {
    file: Mp4Writer,
    name: String,
    start: i64,
}

impl TrackWriter {
    /// Adds the empty `track` to the manifest; packets come in its ticks.
    pub fn create(
        manifest: &SharedManifest,
        track: Track,
        stream: StreamParameters,
    ) -> Result<Self, CaptureError> {
        manifest.set(&track)?;
        Ok(Self {
            manifest: manifest.clone(),
            segment_ticks: i64::from(track.rate) * SEGMENT_SECONDS,
            track,
            stream,
            current: None,
            held: None,
        })
    }

    pub fn push(&mut self, packet: Packet) -> Result<(), CaptureError> {
        if let Some(mut previous) = self.held.take() {
            previous.set_duration(packet.pts() - previous.pts());
            let start = self.write(previous)?;
            if packet.is_key() && packet.pts() - start >= self.segment_ticks {
                self.close(packet.pts())?;
            }
        }
        self.held = Some(packet);
        Ok(())
    }

    /// Writes the last packet lasting until tick `end` and closes the last segment.
    pub fn finish(mut self, end: i64) -> Result<(), CaptureError> {
        if let Some(mut last) = self.held.take() {
            last.set_duration(end - last.pts());
            self.write(last)?;
            self.close(end)?;
        }
        Ok(())
    }

    /// Writes into the open segment, opening one at this packet if needed; returns the start
    /// of the segment.
    fn write(&mut self, mut packet: Packet) -> Result<i64, CaptureError> {
        let segment = match &mut self.current {
            Some(segment) => segment,
            None => {
                let name = self.track.segment_file(self.track.segments.len() + 1);
                let path = self.manifest.dir().join(&name);
                let file = Mp4Writer::create(&path, &self.stream, Mp4Layout::Fragmented)?;
                self.current.insert(OpenSegment {
                    file,
                    name,
                    start: packet.pts(),
                })
            }
        };
        packet.shift(-segment.start);
        segment.file.write(packet)?;
        Ok(segment.start)
    }

    fn close(&mut self, end: i64) -> Result<(), CaptureError> {
        let Some(segment) = self.current.take() else {
            return Ok(());
        };
        segment.file.finish()?;
        self.track.segments.push(Segment {
            file: segment.name,
            start: segment.start,
            end,
        });
        self.manifest.set(&self.track)
    }
}
