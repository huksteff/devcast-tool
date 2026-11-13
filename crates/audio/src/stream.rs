//! One audio stream of a file decoded by FFmpeg into stereo interleaved `f32` at the stream's
//! own rate. Mono goes to both channels at full level (as the recorder writes a mono
//! microphone); more channels are downmixed by FFmpeg. Rate conversion is [`crate::rate`]'s job.

use std::path::PathBuf;

use ffmpeg_next as ffmpeg;
use ffmpeg_next::ChannelLayout;
use ffmpeg_next::format::Sample;
use ffmpeg_next::format::sample::Type as SampleType;
use ffmpeg_next::software::resampling;

use crate::CHANNELS;
use crate::error::AudioError;
use crate::track::SourceTrack;

pub(crate) struct StreamDecoder {
    input: ffmpeg::format::context::Input,
    path: PathBuf,
    stream: usize,
    time_base: ffmpeg::Rational,
    /// The earliest time of the file, seconds: the earliest start of its streams.
    first: f64,
    /// What was opened: a seek to the start of the file opens it again.
    track: SourceTrack,
    decoder: ffmpeg::decoder::Audio,
    /// Sample format and layout to stereo `f32`; made for the first frame's format.
    converter: Option<resampling::Context>,
    frame: ffmpeg::frame::Audio,
    input_done: bool,
}

impl StreamDecoder {
    pub fn open(track: &SourceTrack) -> Result<Self, AudioError> {
        crate::init();
        let path = &track.path;
        let input = ffmpeg::format::input(path).map_err(AudioError::file("открыть", path))?;
        let stream = input
            .stream(track.stream)
            .filter(|s| s.parameters().medium() == ffmpeg::media::Type::Audio)
            .ok_or_else(|| AudioError::NoStream {
                path: path.clone(),
                stream: track.stream,
            })?;
        let time_base = stream.time_base();
        let starts = input.streams().filter_map(|s| {
            let start = s.start_time();
            (start != ffmpeg::ffi::AV_NOPTS_VALUE).then(|| start as f64 * f64::from(s.time_base()))
        });
        let first = starts.reduce(f64::min).unwrap_or(0.0);
        let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(AudioError::file("прочитать параметры", path))?;
        let mut decoder = context.decoder();
        // An edit list trims the start of a frame; the decoder moves the frame's time past the
        // dropped samples only when it knows the packets' time base.
        decoder.set_packet_time_base(time_base);
        let decoder = decoder
            .audio()
            .map_err(AudioError::file("открыть декодер для", path))?;
        Ok(Self {
            input,
            path: path.clone(),
            stream: track.stream,
            time_base,
            first,
            track: track.clone(),
            decoder,
            converter: None,
            frame: ffmpeg::frame::Audio::empty(),
            input_done: false,
        })
    }

    pub fn rate(&self) -> u32 {
        self.decoder.rate()
    }

    /// Moves to the packet at or before `seconds` of stream time. At the start of the file or
    /// before it the file is opened again: FFmpeg refuses a seek to a time before the first
    /// packet, and a decoder moved to the first packet gives its first frame without the
    /// coder's priming before it — not the samples a new decoder gives.
    pub fn seek(&mut self, seconds: f64) -> Result<(), AudioError> {
        if seconds <= self.first {
            *self = Self::open(&self.track)?;
            return Ok(());
        }
        let ts = (seconds * f64::from(ffmpeg::ffi::AV_TIME_BASE)).floor() as i64;
        self.input
            .seek(ts, ..ts)
            .map_err(AudioError::file("перемотать", &self.path))?;
        self.decoder.flush();
        self.input_done = false;
        Ok(())
    }

    /// Appends the next decoded frame to `out` and returns the stream time of its first
    /// sample, seconds; `None` at the end of the stream.
    pub fn next(&mut self, out: &mut Vec<f32>) -> Result<Option<f64>, AudioError> {
        loop {
            match self.decoder.receive_frame(&mut self.frame) {
                Ok(()) => return self.take_frame(out).map(Some),
                Err(ffmpeg::Error::Eof) => return Ok(None),
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {
                    self.feed()?;
                }
                Err(e) => {
                    return Err(AudioError::file("раскодировать", &self.path)(
                        e,
                    ));
                }
            }
        }
    }

    fn feed(&mut self) -> Result<(), AudioError> {
        if self.input_done {
            return self
                .decoder
                .send_eof()
                .map_err(AudioError::file("раскодировать", &self.path));
        }
        loop {
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) if packet.stream() == self.stream => {
                    return self
                        .decoder
                        .send_packet(&packet)
                        .map_err(AudioError::file("раскодировать", &self.path));
                }
                Ok(()) => {}
                Err(ffmpeg::Error::Eof) => {
                    self.input_done = true;
                    return self
                        .decoder
                        .send_eof()
                        .map_err(AudioError::file("раскодировать", &self.path));
                }
                Err(e) => return Err(AudioError::file("прочитать", &self.path)(e)),
            }
        }
    }

    fn take_frame(&mut self, out: &mut Vec<f32>) -> Result<f64, AudioError> {
        let pts = self.frame.timestamp().ok_or_else(|| {
            AudioError::file("прочитать время кадра", &self.path)(
                ffmpeg::Error::InvalidData,
            )
        })?;
        let seconds = pts as f64 * f64::from(self.time_base);
        let channels = self.frame.channels();
        if self.frame.channel_layout().is_empty() {
            self.frame
                .set_channel_layout(ChannelLayout::default(i32::from(channels)));
        }
        let mono = channels == 1;
        let converter = match &mut self.converter {
            Some(converter) => converter,
            None => self.converter.insert(
                resampling::Context::get(
                    self.frame.format(),
                    self.frame.channel_layout(),
                    self.frame.rate(),
                    Sample::F32(SampleType::Packed),
                    if mono {
                        ChannelLayout::MONO
                    } else {
                        ChannelLayout::STEREO
                    },
                    self.frame.rate(),
                )
                .map_err(AudioError::file("перевести формат", &self.path))?,
            ),
        };
        let mut converted = ffmpeg::frame::Audio::empty();
        converter
            .run(&self.frame, &mut converted)
            .map_err(AudioError::file("перевести формат", &self.path))?;
        let frames = converted.samples();
        let samples = converted.plane::<f32>(0);
        if mono {
            out.extend(samples[..frames].iter().flat_map(|&s| [s; CHANNELS]));
        } else {
            out.extend_from_slice(&samples[..frames * CHANNELS]);
        }
        Ok(seconds)
    }
}
