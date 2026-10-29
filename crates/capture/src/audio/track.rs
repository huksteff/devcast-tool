//! A sound track of a recording: device stream → [`AudioTimeline`] → FLAC → fMP4 segments.
//!
//! The stream starts before the video, so nothing is missed while the codec is checked; the
//! track begins at the video's start ([`AudioRecording::begin`]), earlier sound is cut. All
//! work runs on the track's thread, which also reconnects system sound when the default output
//! device changes (the gap is filled with silence by the timeline).

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use dc_encode::{AudioEncoder, AudioSettings};

use super::device::{AudioSource, CHANNELS, Device, to_stereo};
use super::timeline::{AudioTimeline, RATE, TimelineStats};
use crate::error::CaptureError;
use crate::event::HostTime;
use crate::manifest::{SharedManifest, Track};
use crate::pause::PauseClock;
use crate::track::TrackWriter;

/// Name of the system sound track in the manifest and of its files.
pub const SYSTEM_TRACK: &str = "system";

/// Name of the microphone track.
pub const MIC_TRACK: &str = "mic";

/// The host clock (QPC on Windows): this module has no platform code, the recording hands it in.
pub type Clock = fn() -> Result<HostTime, CaptureError>;

/// With no packet for this long the track checks whether the sound fell behind the clock.
const TICK: Duration = Duration::from_millis(250);

/// Silence is filled only this far behind the clock: no packet is this late (decision 008:
/// they come within ~1 ms of their place), so filling never cuts real sound.
const QUIET_MICROS: i64 = 500_000;

/// What was written.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioSummary {
    pub track: &'static str,
    /// The device at the end (system sound follows the default output).
    pub device: String,
    /// Samples per channel at [`RATE`].
    pub frames: i64,
    pub stats: TimelineStats,
    /// Times system sound moved to a new default output device.
    pub reconnects: u32,
    /// Device failures the track went through as silence (an unplugged microphone stays
    /// silent until the stop).
    pub problems: Vec<String>,
}

pub struct AudioRecording {
    track: &'static str,
    control: Sender<Message>,
    thread: Option<JoinHandle<Result<AudioSummary, CaptureError>>>,
}

enum Message {
    Packet(HostTime, Vec<f32>),
    Failed(cpal::Error),
    Begin(HostTime, SharedManifest, PauseClock),
    Stop(HostTime),
    Abort,
}

impl AudioRecording {
    /// Opens the device of `source` and starts listening; sound is kept from [`Self::begin`].
    pub fn start(
        source: AudioSource,
        track: &'static str,
        now: Clock,
    ) -> Result<Self, CaptureError> {
        let device = Device::find(&source)?;
        let (control, messages) = mpsc::channel();
        let (stream, spare) = open(&device, &control)?;
        let link = control.clone();
        let thread = std::thread::Builder::new()
            .name(format!("devcast-audio-{track}"))
            .spawn(move || {
                let mut worker = Worker {
                    source,
                    device,
                    stream: Some(stream),
                    spare,
                    link,
                    now,
                    reconnects: 0,
                    problems: Vec::new(),
                };
                worker.run(track, &messages)
            })
            .map_err(|e| CaptureError::audio("запустить запись звука", e))?;
        Ok(Self {
            track,
            control,
            thread: Some(thread),
        })
    }

    /// The track starts at host time `start` and is written into `manifest`'s folder; what
    /// `clock` says is paused is cut out.
    pub fn begin(
        &self,
        start: HostTime,
        manifest: &SharedManifest,
        clock: &PauseClock,
    ) -> Result<(), CaptureError> {
        self.control
            .send(Message::Begin(start, manifest.clone(), clock.clone()))
            .map_err(|_| self.gone())
    }

    /// Ends the track at host time `at`; [`Self::join`] waits until its last segment is
    /// closed. Called right when the recording stops, so that later packets are cut.
    pub fn stop(&self, at: HostTime) -> Result<(), CaptureError> {
        self.control
            .send(Message::Stop(at))
            .map_err(|_| self.gone())
    }

    pub fn join(mut self) -> Result<AudioSummary, CaptureError> {
        let thread = self.thread.take().ok_or_else(|| self.gone())?;
        thread.join().map_err(|_| self.gone())?
    }

    fn gone(&self) -> CaptureError {
        CaptureError::audio(
            "записать звук",
            format!("поток дорожки {} упал", self.track),
        )
    }
}

impl Drop for AudioRecording {
    /// Not stopped (the recording failed to start or is cancelled): the thread ends and leaves
    /// whatever segments it closed.
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            // If the thread is gone already, there is nothing to stop.
            let _ = self.control.send(Message::Abort);
            let _ = thread.join();
        }
    }
}

/// Streams `device` into `control`; returns the stream and where used buffers go back.
fn open(
    device: &Device,
    control: &Sender<Message>,
) -> Result<(cpal::Stream, Sender<Vec<f32>>), CaptureError> {
    let channels = device.channels();
    let (spare, reuse) = mpsc::channel::<Vec<f32>>();
    let (packets, failures) = (control.clone(), control.clone());
    let stream = device.start(
        move |time, data| {
            let mut samples = reuse.try_recv().unwrap_or_default();
            samples.clear();
            to_stereo(data, channels, &mut samples);
            // The receiver is gone only once the track ended.
            let _ = packets.send(Message::Packet(time, samples));
        },
        move |error| {
            let _ = failures.send(Message::Failed(error));
        },
    )?;
    Ok((stream, spare))
}

/// The track's thread.
struct Worker {
    source: AudioSource,
    device: Device,
    /// Taken at the stop.
    stream: Option<cpal::Stream>,
    spare: Sender<Vec<f32>>,
    /// For the callbacks of a reconnected stream.
    link: Sender<Message>,
    now: Clock,
    reconnects: u32,
    problems: Vec<String>,
}

/// Where the sound goes once the track has begun.
struct Output {
    timeline: AudioTimeline,
    clock: PauseClock,
    encoder: AudioEncoder,
    writer: TrackWriter,
    samples: Vec<f32>,
    packets: Vec<dc_encode::Packet>,
}

impl Worker {
    fn run(
        &mut self,
        track: &'static str,
        messages: &Receiver<Message>,
    ) -> Result<AudioSummary, CaptureError> {
        let mut early = Vec::new();
        let mut output = loop {
            match messages.recv() {
                Ok(Message::Packet(time, samples)) => early.push((time, samples)),
                Ok(Message::Failed(error)) => self.failed(&error, None)?,
                Ok(Message::Begin(start, manifest, clock)) => {
                    break self.output(track, start, &manifest, clock)?;
                }
                Ok(Message::Stop(_) | Message::Abort) | Err(_) => return Err(aborted()),
            }
        };
        for (time, samples) in early {
            output.push(time, &samples)?;
        }
        loop {
            let message = match messages.recv_timeout(TICK) {
                Ok(message) => message,
                Err(RecvTimeoutError::Timeout) => {
                    let now = (self.now)()?;
                    output.quiet(HostTime::from_micros(now.micros() - QUIET_MICROS))?;
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => return Err(aborted()),
            };
            match message {
                Message::Packet(time, samples) => {
                    output.push(time, &samples)?;
                    // Without the stream nobody takes buffers back; then it is not needed.
                    let _ = self.spare.send(samples);
                }
                Message::Failed(error) => self.failed(&error, Some(&mut output.timeline))?,
                Message::Stop(at) => return self.stop(track, output, at, messages),
                Message::Begin(..) | Message::Abort => return Err(aborted()),
            }
        }
    }

    fn output(
        &self,
        track: &'static str,
        start: HostTime,
        manifest: &SharedManifest,
        clock: PauseClock,
    ) -> Result<Output, CaptureError> {
        let encoder = AudioEncoder::open(AudioSettings {
            rate: RATE,
            channels: CHANNELS as u16,
        })?;
        let writer = TrackWriter::create(
            manifest,
            Track::new(track, RATE, start),
            encoder.parameters(),
        )?;
        Ok(Output {
            timeline: AudioTimeline::new(start, self.device.rate())?,
            clock,
            encoder,
            writer,
            samples: Vec::new(),
            packets: Vec::new(),
        })
    }

    /// A device problem. Losing the default output means it changed: system sound moves to
    /// the new default. Glitches need nothing: timestamps keep the sound in place.
    fn failed(
        &mut self,
        error: &cpal::Error,
        timeline: Option<&mut AudioTimeline>,
    ) -> Result<(), CaptureError> {
        use cpal::ErrorKind as Kind;
        match error.kind() {
            Kind::Xrun | Kind::DeviceChanged | Kind::RealtimeDenied => Ok(()),
            Kind::StreamInvalidated | Kind::DeviceNotAvailable
                if self.source == AudioSource::System =>
            {
                match Device::find(&self.source).and_then(|d| Ok((open(&d, &self.link)?, d))) {
                    Ok(((stream, spare), device)) => {
                        self.stream = Some(stream);
                        self.spare = spare;
                        self.reconnects += 1;
                        if let Some(timeline) = timeline {
                            timeline.restart(device.rate())?;
                        }
                        self.device = device;
                    }
                    // The old stream still reports the next change of the default device.
                    Err(e) => self.problems.push(e.to_string()),
                }
                Ok(())
            }
            _ => {
                self.problems
                    .push(format!("«{}»: {error}", self.device.name()));
                Ok(())
            }
        }
    }

    fn stop(
        &mut self,
        track: &'static str,
        mut output: Output,
        at: HostTime,
        messages: &Receiver<Message>,
    ) -> Result<AudioSummary, CaptureError> {
        // Dropping the stream waits for its callbacks, so every captured packet is queued.
        drop(self.stream.take());
        output.timeline.stop_at(output.clock.place(at)?.time);
        for message in messages.try_iter() {
            if let Message::Packet(time, samples) = message {
                output.push(time, &samples)?;
            }
        }
        let (frames, stats) = output.finish()?;
        Ok(AudioSummary {
            track,
            device: self.device.name(),
            frames,
            stats,
            reconnects: self.reconnects,
            problems: std::mem::take(&mut self.problems),
        })
    }
}

impl Output {
    /// Places a packet on the recording clock; a packet that begins in a pause is dropped.
    fn push(&mut self, time: HostTime, samples: &[f32]) -> Result<(), CaptureError> {
        let placed = self.clock.place(time)?;
        if placed.paused {
            return Ok(());
        }
        self.timeline
            .push(placed.time, samples, &mut self.samples)?;
        self.encode()
    }

    /// Silence up to host time `time` if no sound reached it (see [`AudioTimeline::fill_to`]);
    /// nothing in a pause.
    fn quiet(&mut self, time: HostTime) -> Result<(), CaptureError> {
        let placed = self.clock.place(time)?;
        if placed.paused {
            return Ok(());
        }
        self.timeline.fill_to(placed.time, &mut self.samples)?;
        self.encode()
    }

    /// Pads to the stop and closes the last segment; returns the length and what the timeline
    /// did.
    fn finish(mut self) -> Result<(i64, TimelineStats), CaptureError> {
        self.timeline.finish(&mut self.samples)?;
        self.encode()?;
        self.encoder.finish(&mut self.packets)?;
        for packet in self.packets.drain(..) {
            self.writer.push(packet)?;
        }
        let frames = self.timeline.frames();
        self.writer.finish(frames)?;
        Ok((frames, self.timeline.stats()))
    }

    fn encode(&mut self) -> Result<(), CaptureError> {
        self.encoder.encode(&self.samples, &mut self.packets)?;
        self.samples.clear();
        for packet in self.packets.drain(..) {
            self.writer.push(packet)?;
        }
        Ok(())
    }
}

fn aborted() -> CaptureError {
    CaptureError::audio("записать звук", "запись отменена")
}
