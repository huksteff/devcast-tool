//! The video track of a recording: capture → NV12 on the GPU → encoder thread → fMP4
//! segments listed in the manifest.
//!
//! Before capture starts, one frame tells the size, and the codec is chosen and checked for
//! speed at that size: a codec that cannot keep up is an error before anything is recorded.

use std::ops::ControlFlow;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::thread::JoinHandle;
use std::time::Duration;

use dc_encode::{Codec, Nv12Frame, VideoEncoder, VideoSettings};

use super::capture::{FrameSink, VideoCapture};
use super::clock::now;
use super::frame::VideoFrame;
use super::nv12::{Nv12Converter, even_size};
use super::video_encode::{Message, encode, encoder_gone};
use crate::config::Fps;
use crate::error::CaptureError;
use crate::event::HostTime;
use crate::grid::FrameGrid;
use crate::lead::{FrameLead, LeadFilter};
use crate::manifest::{SharedManifest, Track, VIDEO_TRACK};
use crate::pause::PauseClock;
use crate::source::Source;
use crate::track::TrackWriter;

/// The first frame normally arrives within tens of milliseconds; this bound only turns a
/// capture that delivers nothing into an error instead of a hang.
const FIRST_FRAME: Duration = Duration::from_secs(5);

/// Frames converted but not yet encoded. When the encoder falls behind, capture waits and
/// the system drops frames instead of memory growing.
const IN_FLIGHT: usize = 4;
/// Pictures the encoder holds besides the queue: the last one (for repeats) and a paused one.
const HELD_BY_ENCODER: usize = 2;

/// What the recording settled on when it started.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoStart {
    pub codec: Codec,
    /// Frames per second the codec managed in the speed check.
    pub speed: f64,
    pub size: (u32, u32),
    /// Time of the first frame: slot 0 of the video.
    pub start: HostTime,
}

/// What was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoSummary {
    pub frames: u64,
    /// Slot where the video ends (exclusive); its duration is `end / fps` seconds.
    pub end: i64,
    /// How far frame times were ahead of their arrival; `None` without frames.
    pub lead: Option<FrameLead>,
    /// Every kept frame: its slot and how far its WGC time was ahead of its arrival (µs).
    pub leads: Vec<(i64, i64)>,
    /// What went wrong without losing the video: capture stopped before the stop.
    pub problems: Vec<String>,
}

pub struct VideoRecording {
    capture: VideoCapture<EncodeSink>,
    control: SyncSender<Message>,
    encoder: JoinHandle<Result<VideoSummary, CaptureError>>,
    info: VideoStart,
}

impl VideoRecording {
    /// Starts recording `source` at `fps` as the track `video` of `manifest`; what `clock`
    /// says is paused is cut out.
    pub fn start(
        source: &Source,
        fps: Fps,
        manifest: &SharedManifest,
        clock: &PauseClock,
    ) -> Result<Self, CaptureError> {
        let (width, height) = even_size(frame_size(source)?);
        let settings = VideoSettings {
            width,
            height,
            fps: fps.get(),
        };
        let selection = dc_encode::select(settings)?;
        let encoder = VideoEncoder::open(selection.codec, settings)?;
        let video = encoder.parameters();
        let (writer_tx, writer_rx) = mpsc::channel();
        let (frames_tx, frames_rx) = mpsc::sync_channel(IN_FLIGHT);
        let (returned_tx, returned_rx) = mpsc::channel();
        let encoder_clock = clock.clone();
        let encoder = std::thread::Builder::new()
            .name("devcast-encode".to_owned())
            .spawn(move || encode(&frames_rx, returned_tx, encoder, &writer_rx, encoder_clock))
            .map_err(|e| {
                CaptureError::platform("запустить кодирование")(e.to_string())
            })?;
        let (first_tx, first_rx) = mpsc::channel();
        let sink = EncodeSink::new(
            (width, height),
            fps,
            first_tx,
            clock,
            frames_tx.clone(),
            returned_rx,
        )?;
        let (capture, start) = first_frame(VideoCapture::start(source, sink)?, &first_rx)?;
        let mut track = Track::new(VIDEO_TRACK, fps.get(), start);
        track.size = Some((width, height));
        let writer = match TrackWriter::create(manifest, track, video) {
            Ok(writer) => writer,
            Err(e) => {
                capture.stop()?;
                return Err(e);
            }
        };
        let grid = FrameGrid::new(start, fps);
        writer_tx.send((writer, grid)).map_err(|_| encoder_gone())?;
        Ok(Self {
            capture,
            control: frames_tx,
            encoder,
            info: VideoStart {
                codec: selection.codec,
                speed: selection.speed,
                size: (width, height),
                start,
            },
        })
    }

    pub fn info(&self) -> VideoStart {
        self.info
    }

    /// Stops capturing, lets the last frame last until host time `at` and closes the last
    /// segment. A capture that failed on the way keeps the video until then (the encoder
    /// repeats the last picture) and its error is a problem of the summary.
    pub fn stop(self, at: HostTime) -> Result<VideoSummary, CaptureError> {
        let captured = self.capture.stop();
        let sent = self.control.send(Message::End(at));
        let encoded = self.encoder.join().map_err(|_| {
            CaptureError::platform("записать видео")(
                "поток кодирования упал".to_owned(),
            )
        })?;
        // A failed encoder also fails the capture and the End message; its error says why.
        let mut summary = encoded?;
        sent.map_err(|_| encoder_gone())?;
        if let Err(error) = captured {
            summary.problems.push(format!(
                "захват экрана прервался до стопа, видео записано до этого места: {error}"
            ));
        }
        Ok(summary)
    }
}

/// Why the capture ended before its first frame.
fn failure<S: FrameSink>(capture: VideoCapture<S>) -> CaptureError {
    match capture.wait() {
        Err(e) => e,
        Ok(()) => CaptureError::platform("начать запись")(
            "захват закончился до первого кадра: источник закрыт".to_owned(),
        ),
    }
}

/// Size of the source's frames, from one captured frame.
fn frame_size(source: &Source) -> Result<(u32, u32), CaptureError> {
    struct Size(Sender<(u32, u32)>);
    impl FrameSink for Size {
        fn frame(&mut self, frame: &VideoFrame<'_>) -> Result<ControlFlow<()>, CaptureError> {
            // The receiver is gone only if the caller stopped waiting; then nothing needs it.
            let _ = self.0.send(frame.size());
            Ok(ControlFlow::Break(()))
        }

        fn closed(&mut self) {}
    }
    let (sender, receiver) = mpsc::channel();
    let (capture, size) = first_frame(VideoCapture::start(source, Size(sender))?, &receiver)?;
    capture.wait()?;
    Ok(size)
}

/// What the sink of `capture` sends at its first frame; a capture that delivers nothing within
/// [`FIRST_FRAME`] or ends before its first frame is stopped and gives an error.
fn first_frame<S: FrameSink, T>(
    capture: VideoCapture<S>,
    receiver: &Receiver<T>,
) -> Result<(VideoCapture<S>, T), CaptureError> {
    match receiver.recv_timeout(FIRST_FRAME) {
        Ok(value) => Ok((capture, value)),
        Err(RecvTimeoutError::Timeout) => {
            capture.stop()?;
            Err(CaptureError::NoFrame {
                seconds: FIRST_FRAME.as_secs(),
            })
        }
        Err(RecvTimeoutError::Disconnected) => Err(failure(capture)),
    }
}

/// The capture side: places frames on the grid, converts them and hands them to the encoder.
struct EncodeSink {
    size: (u32, u32),
    fps: Fps,
    first: Option<Sender<HostTime>>,
    /// Set at the first frame, which needs the capture's device and time.
    started: Option<(Nv12Converter, FrameGrid)>,
    lead: LeadFilter,
    /// The slot of the last frame sent. The encoder gives the slots and its last one is never
    /// earlier (it adds repeats and pause pictures), so a frame not after this one is dropped
    /// here without converting it.
    last: Option<i64>,
    clock: PauseClock,
    frames: SyncSender<Message>,
    spare: Vec<Nv12Frame>,
    returned: Receiver<Nv12Frame>,
}

impl EncodeSink {
    fn new(
        size: (u32, u32),
        fps: Fps,
        first: Sender<HostTime>,
        clock: &PauseClock,
        frames: SyncSender<Message>,
        returned: Receiver<Nv12Frame>,
    ) -> Result<Self, CaptureError> {
        Ok(Self {
            size,
            fps,
            first: Some(first),
            started: None,
            lead: LeadFilter::default(),
            last: None,
            clock: clock.clone(),
            frames,
            spare: (0..IN_FLIGHT + HELD_BY_ENCODER)
                .map(|_| Nv12Frame::new(size.0, size.1))
                .collect::<Result<_, _>>()?,
            returned,
        })
    }
}

impl FrameSink for EncodeSink {
    fn frame(&mut self, frame: &VideoFrame<'_>) -> Result<ControlFlow<()>, CaptureError> {
        let arrived = now()?;
        let time = self.lead.place(frame.time(), arrived);
        let (converter, grid) = match &mut self.started {
            Some(started) => started,
            None => {
                let converter = Nv12Converter::new(frame, self.size)?;
                let grid = FrameGrid::new(time, self.fps);
                if let Some(first) = self.first.take() {
                    // Without a receiver `start` gave up waiting and stops the capture.
                    let _ = first.send(grid.start());
                }
                self.started.insert((converter, grid))
            }
        };
        let lead = frame.time().micros() - arrived.micros();
        let placed = self.clock.place(time)?;
        if !placed.paused {
            match grid.place(self.last, placed.time) {
                Some(slot) => self.last = Some(slot),
                None => return Ok(ControlFlow::Continue(())),
            }
        }
        let mut buffer = match self.spare.pop() {
            Some(buffer) => buffer,
            None => self.returned.recv().map_err(|_| encoder_gone())?,
        };
        converter.convert(frame, &mut buffer)?;
        let message = Message::Frame(buffer, time, lead);
        send(&self.frames, message)?;
        Ok(ControlFlow::Continue(()))
    }

    fn closed(&mut self) {}
}

fn send(frames: &SyncSender<Message>, message: Message) -> Result<(), CaptureError> {
    frames.send(message).map_err(|_| encoder_gone())
}
