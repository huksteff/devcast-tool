//! Sound encoding. Recordings: FLAC (decision 008), lossless and without encoder delay, so
//! sample `n` of the input is sample `n` of the file and tracks line up exactly. Export: AAC LC
//! (decision 006), which every player and messenger plays; its priming delay the MP4 edit list
//! hides.

use ffmpeg_next as ffmpeg;
use ffmpeg_next::format::Sample;
use ffmpeg_next::format::sample::Type;

use crate::error::EncodeError;
use crate::packet::{Packet, StreamParameters};

/// Sample rate and channel count of a sound stream. Timestamps are samples: tick = 1 / `rate` s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSettings {
    pub rate: u32,
    pub channels: u16,
}

/// Bit rate of exported AAC, bit/s (decision 006).
const AAC_BIT_RATE: usize = 192_000;

pub struct AudioEncoder {
    inner: ffmpeg::encoder::Audio,
    /// AAC takes planar float, FLAC packed 16 bit.
    planar: bool,
    frame: ffmpeg::frame::Audio,
    channels: usize,
    /// Samples per channel the encoder takes at once; only the last frame may be shorter.
    frame_size: usize,
    /// Samples per channel already in `frame`.
    filled: usize,
    /// Time of the first sample of `frame`.
    pts: i64,
    scratch: ffmpeg::Packet,
}

impl AudioEncoder {
    /// Opens FLAC, 16 bit, for MP4.
    pub fn open(settings: AudioSettings) -> Result<Self, EncodeError> {
        Self::open_codec(settings, ffmpeg::codec::Id::FLAC, Sample::I16(Type::Packed))
    }

    /// Opens AAC LC at 192 kbit/s for MP4.
    pub fn open_aac(settings: AudioSettings) -> Result<Self, EncodeError> {
        Self::open_codec(settings, ffmpeg::codec::Id::AAC, Sample::F32(Type::Planar))
    }

    fn open_codec(
        settings: AudioSettings,
        id: ffmpeg::codec::Id,
        format: Sample,
    ) -> Result<Self, EncodeError> {
        crate::init();
        let failed = EncodeError::audio;
        let found = ffmpeg::encoder::find(id)
            .ok_or_else(|| failed("найти кодек")(ffmpeg::Error::EncoderNotFound))?;
        let mut enc = ffmpeg::codec::context::Context::new_with_codec(found)
            .encoder()
            .audio()
            .map_err(failed("создать кодек"))?;
        let rate = i32::try_from(settings.rate)
            .map_err(|_| failed("задать частоту")(ffmpeg::Error::InvalidData))?;
        let layout = ffmpeg::ChannelLayout::default(i32::from(settings.channels));
        enc.set_rate(rate);
        enc.set_format(format);
        enc.set_channel_layout(layout);
        enc.set_time_base((1, rate));
        enc.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
        let planar = id == ffmpeg::codec::Id::AAC;
        if planar {
            enc.set_bit_rate(AAC_BIT_RATE);
        }
        let inner = enc.open_as(found).map_err(failed("открыть кодек"))?;
        let frame_size = inner.frame_size() as usize;
        let mut frame = ffmpeg::frame::Audio::new(format, frame_size, layout);
        frame.set_rate(settings.rate);
        Ok(Self {
            inner,
            planar,
            frame,
            channels: usize::from(settings.channels),
            frame_size,
            filled: 0,
            pts: 0,
            scratch: ffmpeg::Packet::empty(),
        })
    }

    pub fn parameters(&self) -> StreamParameters {
        StreamParameters {
            codec: ffmpeg::codec::Parameters::from(&*self.inner),
            time_base: self.inner.time_base(),
        }
    }

    /// Appends interleaved samples (−1…1; FLAC clips louder ones) right after the previous ones.
    /// Finished packets are appended to `out`.
    pub fn encode(&mut self, samples: &[f32], out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        let mut rest = samples;
        while !rest.is_empty() {
            let free = (self.frame_size - self.filled) * self.channels;
            let (now, later) = rest.split_at(free.min(rest.len()));
            self.put(now);
            self.filled += now.len() / self.channels;
            rest = later;
            if self.filled == self.frame_size {
                self.send(out)?;
            }
        }
        Ok(())
    }

    /// Writes interleaved `samples` into the frame after the `filled` ones.
    fn put(&mut self, samples: &[f32]) {
        if self.planar {
            for channel in 0..self.channels {
                let plane = &mut self.frame.data_mut(channel)[self.filled * 4..];
                let values = samples.iter().skip(channel).step_by(self.channels);
                for (place, sample) in plane.as_chunks_mut::<4>().0.iter_mut().zip(values) {
                    *place = sample.to_ne_bytes();
                }
            }
            return;
        }
        let offset = self.filled * self.channels * 2;
        let bytes = &mut self.frame.data_mut(0)[offset..offset + samples.len() * 2];
        for (place, sample) in bytes.as_chunks_mut::<2>().0.iter_mut().zip(samples) {
            // `as` saturates: samples above full scale clip instead of wrapping.
            *place = ((sample * 32767.0).round() as i16).to_le_bytes();
        }
    }

    /// Encodes the samples still waiting for a full frame and flushes the encoder.
    pub fn finish(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        if self.filled > 0 {
            self.frame.set_samples(self.filled);
            self.send(out)?;
        }
        self.inner
            .send_eof()
            .map_err(EncodeError::audio("завершить кодирование"))?;
        self.drain(out)
    }

    fn send(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        self.frame.set_pts(Some(self.pts));
        self.inner
            .send_frame(&self.frame)
            .map_err(EncodeError::audio("закодировать звук"))?;
        self.pts += self.filled as i64;
        self.filled = 0;
        self.drain(out)
    }

    fn drain(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        loop {
            match self.inner.receive_packet(&mut self.scratch) {
                Ok(()) => {
                    let pts = self.scratch.pts().ok_or_else(|| {
                        EncodeError::audio("получить время пакета")(
                            ffmpeg::Error::InvalidData,
                        )
                    })?;
                    let packet = std::mem::replace(&mut self.scratch, ffmpeg::Packet::empty());
                    out.push(Packet::new(packet, pts));
                }
                Err(ffmpeg::Error::Other {
                    errno: ffmpeg::util::error::EAGAIN,
                })
                | Err(ffmpeg::Error::Eof) => return Ok(()),
                Err(e) => return Err(EncodeError::audio("получить пакет")(e)),
            }
        }
    }
}
