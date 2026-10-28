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
use super::frame::VideoFrame;
use super::nv12::{Nv12Converter, even_size};
use crate::config::Fps;
use crate::error::CaptureError;
use crate::event::HostTime;
use crate::grid::FrameGrid;
use crate::manifest::{SharedManifest, Track};
use crate::source::Source;
use crate::track::TrackWriter;

/// The first frame normally arrives within tens of milliseconds; this bound only turns a
/// capture that delivers nothing into an error instead of a hang.
const FIRST_FRAME: Duration = Duration::from_secs(5);

/// Frames converted but not yet encoded. When the encoder falls behind, capture waits and
/// the system drops frames instead of memory growing.
const IN_FLIGHT: usize = 4;

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

/// Name of the video track in the manifest and of its files.
pub const VIDEO_TRACK: &str = "video";

/// What was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoSummary {
    pub frames: u64,
    /// Slot where the video ends (exclusive); its duration is `end / fps` seconds.
    pub end: i64,
}

pub struct VideoRecording {
    capture: VideoCapture<EncodeSink>,
    control: SyncSender<Message>,
    encoder: JoinHandle<Result<VideoSummary, CaptureError>>,
    grid: FrameGrid,
    info: VideoStart,
}

enum Message {
    Frame(Nv12Frame, i64),
    /// Recording stopped at `at`; no more frames will come.
    End {
        grid: FrameGrid,
        at: HostTime,
    },
}

impl VideoRecording {
    /// Starts recording `source` at `fps` as the track `video` of `manifest`.
    pub fn start(
        source: &Source,
        fps: Fps,
        manifest: &SharedManifest,
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
        let encoder = std::thread::Builder::new()
            .name("devcast-encode".to_owned())
            .spawn(move || encode(&frames_rx, &returned_tx, encoder, &writer_rx))
            .map_err(|e| {
                CaptureError::platform("запустить кодирование")(e.to_string())
            })?;
        let (first_tx, first_rx) = mpsc::channel();
        let sink = EncodeSink {
            size: (width, height),
            fps,
            first: Some(first_tx),
            started: None,
            last: None,
            frames: frames_tx.clone(),
            spare: (0..IN_FLIGHT)
                .map(|_| Nv12Frame::new(width, height))
                .collect::<Result<_, _>>()?,
            returned: returned_rx,
        };
        let capture = VideoCapture::start(source, sink)?;
        let start = match first_rx.recv_timeout(FIRST_FRAME) {
            Ok(start) => start,
            Err(RecvTimeoutError::Timeout) => {
                capture.stop()?;
                return Err(CaptureError::NoFrame {
                    seconds: FIRST_FRAME.as_secs(),
                });
            }
            Err(RecvTimeoutError::Disconnected) => return Err(failure(capture)),
        };
        let track = Track::new(VIDEO_TRACK, fps.get(), start);
        let writer = match TrackWriter::create(manifest, track, video) {
            Ok(writer) => writer,
            Err(e) => {
                capture.stop()?;
                return Err(e);
            }
        };
        writer_tx.send(writer).map_err(|_| encoder_gone())?;
        Ok(Self {
            capture,
            control: frames_tx,
            encoder,
            grid: FrameGrid::new(start, fps),
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
    /// segment.
    pub fn stop(self, at: HostTime) -> Result<VideoSummary, CaptureError> {
        let captured = self.capture.stop();
        let sent = self.control.send(Message::End {
            grid: self.grid,
            at,
        });
        let encoded = self.encoder.join().map_err(|_| {
            CaptureError::platform("записать видео")(
                "поток кодирования упал".to_owned(),
            )
        })?;
        // A failed encoder also fails the capture and the End message; its error says why.
        let summary = encoded?;
        captured?;
        sent.map_err(|_| encoder_gone())?;
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

fn encoder_gone() -> CaptureError {
    CaptureError::platform("записать видео")("кодирование остановилось".to_owned())
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
    let capture = VideoCapture::start(source, Size(sender))?;
    match receiver.recv_timeout(FIRST_FRAME) {
        Ok(size) => {
            capture.wait()?;
            Ok(size)
        }
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
    last: Option<i64>,
    frames: SyncSender<Message>,
    spare: Vec<Nv12Frame>,
    returned: Receiver<Nv12Frame>,
}

impl FrameSink for EncodeSink {
    fn frame(&mut self, frame: &VideoFrame<'_>) -> Result<ControlFlow<()>, CaptureError> {
        let (converter, grid) = match &mut self.started {
            Some(started) => started,
            None => {
                let converter = Nv12Converter::new(frame, self.size)?;
                let grid = FrameGrid::new(frame.time(), self.fps);
                if let Some(first) = self.first.take() {
                    // Without a receiver `start` gave up waiting and stops the capture.
                    let _ = first.send(grid.start());
                }
                self.started.insert((converter, grid))
            }
        };
        let Some(slot) = grid.place(self.last, frame.time()) else {
            return Ok(ControlFlow::Continue(()));
        };
        self.last = Some(slot);
        let mut buffer = match self.spare.pop() {
            Some(buffer) => buffer,
            None => self.returned.recv().map_err(|_| encoder_gone())?,
        };
        converter.convert(frame, &mut buffer)?;
        self.frames
            .send(Message::Frame(buffer, slot))
            .map_err(|_| encoder_gone())?;
        Ok(ControlFlow::Continue(()))
    }

    fn closed(&mut self) {}
}

/// The encoder thread: frames → packets → segments, until [`Message::End`]. The writer comes
/// once the first frame has fixed the start time.
fn encode(
    frames: &Receiver<Message>,
    returned: &Sender<Nv12Frame>,
    mut encoder: VideoEncoder,
    writer: &Receiver<TrackWriter>,
) -> Result<VideoSummary, CaptureError> {
    let mut track = writer.recv().map_err(|_| encoder_gone())?;
    let mut packets = Vec::new();
    let mut count = 0;
    let mut last = 0;
    for message in frames {
        match message {
            Message::Frame(mut frame, slot) => {
                encoder.encode(&mut frame, slot, &mut packets)?;
                for packet in packets.drain(..) {
                    track.push(packet)?;
                }
                count += 1;
                last = slot;
                // After capture stops the remaining buffers are not needed any more.
                let _ = returned.send(frame);
            }
            Message::End { grid, at } => {
                encoder.finish(&mut packets)?;
                for packet in packets.drain(..) {
                    track.push(packet)?;
                }
                let end = grid.end(last, at);
                track.finish(end)?;
                return Ok(VideoSummary { frames: count, end });
            }
        }
    }
    Err(encoder_gone())
}
