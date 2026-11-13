//! The encoder thread of the video track: frames → packets → segments. Only this thread gives
//! frames their slots, so a repeat and a captured frame never share one. Pictures captured
//! during a pause are held; the newest one shows from where the recording goes on. A screen
//! that does not change sends no frames, so after [`REPEAT`] without one the last picture is
//! encoded again: key frames and closed segments keep coming, and a killed recording of a still
//! screen can be recovered.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use dc_encode::{Nv12Frame, VideoEncoder};

use super::video::VideoSummary;
use crate::error::CaptureError;
use crate::event::HostTime;
use crate::grid::FrameGrid;
use crate::lead::FrameLead;
use crate::pause::PauseClock;
use crate::track::TrackWriter;

/// How long the encoder waits for a frame before it repeats the last one.
pub(super) const REPEAT: Duration = Duration::from_secs(1);

pub(super) enum Message {
    /// A frame captured at a host time, and how far its WGC time is ahead of its arrival (µs).
    Frame(Nv12Frame, HostTime, i64),
    /// Recording stopped at `at`; no more frames will come.
    End(HostTime),
}

pub(super) fn encoder_gone() -> CaptureError {
    CaptureError::platform("записать видео")("кодирование остановилось".to_owned())
}

/// The encoder thread's state: frames → packets → segments.
struct Encoding {
    encoder: VideoEncoder,
    track: TrackWriter,
    grid: FrameGrid,
    clock: PauseClock,
    /// Pictures capture can fill again.
    returned: Sender<Nv12Frame>,
    packets: Vec<dc_encode::Packet>,
    count: u64,
    leads: Vec<(i64, i64)>,
    last: Option<i64>,
    /// The last encoded picture, kept for repeats; the one before it goes back to capture.
    kept: Option<Nv12Frame>,
    /// The newest picture of a pause: its lead and capture time.
    held: Option<(Nv12Frame, i64, HostTime)>,
}

impl Encoding {
    fn new(
        encoder: VideoEncoder,
        track: TrackWriter,
        grid: FrameGrid,
        clock: PauseClock,
        returned: Sender<Nv12Frame>,
    ) -> Self {
        Self {
            encoder,
            track,
            grid,
            clock,
            returned,
            packets: Vec::new(),
            count: 0,
            leads: Vec::new(),
            last: None,
            kept: None,
            held: None,
        }
    }

    /// A captured frame: held during a pause, encoded at its slot, or dropped (and given back)
    /// when that slot is not after the last frame.
    fn frame(&mut self, frame: Nv12Frame, time: HostTime, lead: i64) -> Result<(), CaptureError> {
        self.release(time)?;
        let placed = self.clock.place(time)?;
        if placed.paused {
            if let Some((old, ..)) = self.held.replace((frame, lead, time)) {
                self.give_back(old);
            }
            return Ok(());
        }
        match self.grid.place(self.last, placed.time) {
            Some(slot) => self.put(frame, slot, lead),
            None => {
                self.give_back(frame);
                Ok(())
            }
        }
    }

    /// No frame for [`REPEAT`]: the held picture takes its slot if the pause has ended, then
    /// the last picture is repeated.
    fn idle(&mut self, now: HostTime) -> Result<(), CaptureError> {
        self.release(now)?;
        self.repeat(now)
    }

    /// Encodes the held picture right where the recording goes on, if the pause ended by
    /// `until`.
    fn release(&mut self, until: HostTime) -> Result<(), CaptureError> {
        let Some((_, _, held)) = &self.held else {
            return Ok(());
        };
        let Some(resumed) = self.clock.resumed_after(*held, until)? else {
            return Ok(());
        };
        let resumed = self.clock.place(resumed)?.time;
        let slot = self.grid.end(self.last.unwrap_or(-1), resumed);
        match self.held.take() {
            Some((frame, lead, _)) => self.put(frame, slot, lead),
            None => Ok(()),
        }
    }

    fn put(&mut self, mut frame: Nv12Frame, slot: i64, lead: i64) -> Result<(), CaptureError> {
        self.leads.push((slot, lead));
        self.encode(&mut frame, slot)?;
        if let Some(old) = self.kept.replace(frame) {
            self.give_back(old);
        }
        Ok(())
    }

    fn give_back(&self, frame: Nv12Frame) {
        // After capture stops the returned buffers are not needed any more.
        let _ = self.returned.send(frame);
    }

    /// Encodes the kept picture again at the slot of `now`, unless the recording is paused or
    /// that slot is not after the last frame.
    fn repeat(&mut self, now: HostTime) -> Result<(), CaptureError> {
        let placed = self.clock.place(now)?;
        if placed.paused {
            return Ok(());
        }
        let Some(slot) = self.grid.repeat(self.last, placed.time) else {
            return Ok(());
        };
        let Some(mut frame) = self.kept.take() else {
            return Ok(());
        };
        let encoded = self.encode(&mut frame, slot);
        self.kept = Some(frame);
        encoded
    }

    fn encode(&mut self, frame: &mut Nv12Frame, slot: i64) -> Result<(), CaptureError> {
        self.encoder.encode(frame, slot, &mut self.packets)?;
        for packet in self.packets.drain(..) {
            self.track.push(packet)?;
        }
        self.count += 1;
        self.last = Some(slot);
        Ok(())
    }

    /// Recording stopped at `at`: the last frame lasts until then.
    fn finish(mut self, at: HostTime) -> Result<VideoSummary, CaptureError> {
        self.release(at)?;
        let end = self
            .grid
            .end(self.last.unwrap_or(-1), self.clock.place(at)?.time);
        self.encoder.finish(&mut self.packets)?;
        for packet in self.packets.drain(..) {
            self.track.push(packet)?;
        }
        self.track.finish(end)?;
        Ok(VideoSummary {
            frames: self.count,
            end,
            lead: FrameLead::measure(self.leads.iter().map(|l| l.1).collect()),
            leads: self.leads,
            problems: Vec::new(),
        })
    }
}

/// The encoder thread: frames → packets → segments, until [`Message::End`]. The writer and the
/// frame grid come once the first frame has fixed the start time.
pub(super) fn encode(
    frames: &Receiver<Message>,
    returned: Sender<Nv12Frame>,
    encoder: VideoEncoder,
    writer: &Receiver<(TrackWriter, FrameGrid)>,
    clock: PauseClock,
) -> Result<VideoSummary, CaptureError> {
    let (track, grid) = writer.recv().map_err(|_| encoder_gone())?;
    let mut encoding = Encoding::new(encoder, track, grid, clock, returned);
    loop {
        match frames.recv_timeout(REPEAT) {
            Ok(Message::Frame(frame, time, lead)) => encoding.frame(frame, time, lead)?,
            Ok(Message::End(at)) => return encoding.finish(at),
            Err(RecvTimeoutError::Timeout) => encoding.idle(super::now()?)?,
            Err(RecvTimeoutError::Disconnected) => return Err(encoder_gone()),
        }
    }
}

#[cfg(test)]
#[path = "video_encode_tests.rs"]
mod tests;
