//! The encoder thread of the video track: frames → packets → segments. Pictures captured
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
    /// A frame at a slot, and how far its time is ahead of its arrival (µs).
    Frame(Nv12Frame, i64, i64),
    /// The newest picture of a pause, captured at a time: it shows when the recording goes on.
    Held(Nv12Frame, i64, HostTime),
    /// The recording went on: the held picture takes this slot.
    Release(i64),
    /// Recording stopped at `at`; no more frames will come.
    End { grid: FrameGrid, at: HostTime },
}

pub(super) fn encoder_gone() -> CaptureError {
    CaptureError::platform("записать видео")("кодирование остановилось".to_owned())
}

/// The slot of a picture captured at `held` during a pause, if the pause ended by `until`:
/// right where the recording goes on.
pub(super) fn release(
    clock: &PauseClock,
    grid: &FrameGrid,
    last: Option<i64>,
    held: HostTime,
    until: HostTime,
) -> Result<Option<i64>, CaptureError> {
    let Some(resumed) = clock.resumed_after(held, until)? else {
        return Ok(None);
    };
    Ok(Some(
        grid.end(last.unwrap_or(-1), clock.place(resumed)?.time),
    ))
}

/// The encoder thread's state: frames → packets → segments.
struct Encoding {
    encoder: VideoEncoder,
    track: TrackWriter,
    packets: Vec<dc_encode::Packet>,
    count: u64,
    leads: Vec<(i64, i64)>,
    last: Option<i64>,
    /// The last encoded picture, kept for repeats; the one before it goes back to capture.
    kept: Option<Nv12Frame>,
}

impl Encoding {
    fn put(
        &mut self,
        mut frame: Nv12Frame,
        slot: i64,
        lead: i64,
        returned: &Sender<Nv12Frame>,
    ) -> Result<(), CaptureError> {
        self.leads.push((slot, lead));
        self.encode(&mut frame, slot)?;
        if let Some(old) = self.kept.replace(frame) {
            // After capture stops the returned buffers are not needed any more.
            let _ = returned.send(old);
        }
        Ok(())
    }

    /// Encodes the kept picture again at the slot of `now`, unless the recording is paused or
    /// that slot is not after the last frame.
    fn repeat(
        &mut self,
        grid: &FrameGrid,
        clock: &PauseClock,
        now: HostTime,
    ) -> Result<(), CaptureError> {
        let placed = clock.place(now)?;
        if placed.paused {
            return Ok(());
        }
        let Some(slot) = grid.place(self.last, placed.time) else {
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

    fn finish(mut self, end: i64) -> Result<VideoSummary, CaptureError> {
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
        })
    }
}

/// The encoder thread: frames → packets → segments, until [`Message::End`]. The writer and the
/// frame grid come once the first frame has fixed the start time.
pub(super) fn encode(
    frames: &Receiver<Message>,
    returned: &Sender<Nv12Frame>,
    encoder: VideoEncoder,
    writer: &Receiver<(TrackWriter, FrameGrid)>,
    clock: &PauseClock,
) -> Result<VideoSummary, CaptureError> {
    let (track, grid) = writer.recv().map_err(|_| encoder_gone())?;
    let mut encoding = Encoding {
        encoder,
        track,
        packets: Vec::new(),
        count: 0,
        leads: Vec::new(),
        last: None,
        kept: None,
    };
    let mut held: Option<(Nv12Frame, i64, HostTime)> = None;
    loop {
        let message = match frames.recv_timeout(REPEAT) {
            Ok(message) => message,
            Err(RecvTimeoutError::Timeout) => {
                if held.is_none() {
                    encoding.repeat(&grid, clock, super::now()?)?;
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return Err(encoder_gone()),
        };
        match message {
            Message::Frame(frame, slot, lead) => encoding.put(frame, slot, lead, returned)?,
            Message::Held(frame, lead, time) => {
                if let Some((old, ..)) = held.replace((frame, lead, time)) {
                    let _ = returned.send(old);
                }
            }
            Message::Release(slot) => {
                if let Some((frame, lead, _)) = held.take() {
                    encoding.put(frame, slot, lead, returned)?;
                }
            }
            Message::End { grid, at } => {
                if let Some((frame, lead, time)) = held.take()
                    && let Some(slot) = release(clock, &grid, encoding.last, time, at)?
                {
                    encoding.put(frame, slot, lead, returned)?;
                }
                let end = grid.end(encoding.last.unwrap_or(-1), clock.place(at)?.time);
                return encoding.finish(end);
            }
        }
    }
}

#[cfg(test)]
#[path = "video_encode_tests.rs"]
mod tests;
