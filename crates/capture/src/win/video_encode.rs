//! The encoder thread of the video track: frames → packets → segments. Pictures captured
//! during a pause are held; the newest one shows from where the recording goes on.

use std::sync::mpsc::{Receiver, Sender};

use dc_encode::{Nv12Frame, VideoEncoder};

use super::video::VideoSummary;
use crate::error::CaptureError;
use crate::event::HostTime;
use crate::grid::FrameGrid;
use crate::lead::FrameLead;
use crate::pause::PauseClock;
use crate::track::TrackWriter;

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
}

impl Encoding {
    fn put(&mut self, frame: &mut Nv12Frame, slot: i64, lead: i64) -> Result<(), CaptureError> {
        self.leads.push((slot, lead));
        self.encoder.encode(frame, slot, &mut self.packets)?;
        self.write()?;
        self.count += 1;
        self.last = Some(slot);
        Ok(())
    }

    fn write(&mut self) -> Result<(), CaptureError> {
        for packet in self.packets.drain(..) {
            self.track.push(packet)?;
        }
        Ok(())
    }

    fn finish(mut self, end: i64) -> Result<VideoSummary, CaptureError> {
        self.encoder.finish(&mut self.packets)?;
        self.write()?;
        self.track.finish(end)?;
        Ok(VideoSummary {
            frames: self.count,
            end,
            lead: FrameLead::measure(self.leads.iter().map(|l| l.1).collect()),
            leads: self.leads,
        })
    }
}

/// The encoder thread: frames → packets → segments, until [`Message::End`]. The writer comes
/// once the first frame has fixed the start time.
pub(super) fn encode(
    frames: &Receiver<Message>,
    returned: &Sender<Nv12Frame>,
    encoder: VideoEncoder,
    writer: &Receiver<TrackWriter>,
    clock: &PauseClock,
) -> Result<VideoSummary, CaptureError> {
    let mut encoding = Encoding {
        encoder,
        track: writer.recv().map_err(|_| encoder_gone())?,
        packets: Vec::new(),
        count: 0,
        leads: Vec::new(),
        last: None,
    };
    let mut held: Option<(Nv12Frame, i64, HostTime)> = None;
    // After capture stops the returned buffers are not needed any more.
    for message in frames {
        match message {
            Message::Frame(mut frame, slot, lead) => {
                encoding.put(&mut frame, slot, lead)?;
                let _ = returned.send(frame);
            }
            Message::Held(frame, lead, time) => {
                if let Some((old, ..)) = held.replace((frame, lead, time)) {
                    let _ = returned.send(old);
                }
            }
            Message::Release(slot) => {
                if let Some((mut frame, lead, _)) = held.take() {
                    encoding.put(&mut frame, slot, lead)?;
                    let _ = returned.send(frame);
                }
            }
            Message::End { grid, at } => {
                if let Some((mut frame, lead, time)) = held.take()
                    && let Some(slot) = release(clock, &grid, encoding.last, time, at)?
                {
                    encoding.put(&mut frame, slot, lead)?;
                }
                let end = grid.end(encoding.last.unwrap_or(-1), clock.place(at)?.time);
                return encoding.finish(end);
            }
        }
    }
    Err(encoder_gone())
}
