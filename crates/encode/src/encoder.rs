//! H.264 encoding of NV12 frames with a key frame at least every second (decision 003).

use ffmpeg_next as ffmpeg;
use ffmpeg_next::util::color;

use crate::codec::{Codec, Quality};
use crate::error::EncodeError;
use crate::frame::{Nv12Frame, check_size};
use crate::packet::{Packet, StreamParameters};

/// Size and frame rate of a stream. Timestamps are frame slots: tick = 1 / `fps` s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

pub struct VideoEncoder {
    codec: Codec,
    inner: ffmpeg::encoder::Video,
    fps: i64,
    last_key: Option<i64>,
    scratch: ffmpeg::Packet,
}

impl VideoEncoder {
    /// Opens `codec` for a recording: [`Quality::High`].
    pub fn open(codec: Codec, settings: VideoSettings) -> Result<Self, EncodeError> {
        Self::open_with(codec, settings, Quality::High)
    }

    /// Opens `codec` for MP4: no B-frames, SPS/PPS in the stream header, BT.709 limited range,
    /// chroma sited left (as the video pass reads it, decision 006).
    pub fn open_with(
        codec: Codec,
        settings: VideoSettings,
        quality: Quality,
    ) -> Result<Self, EncodeError> {
        crate::init();
        let VideoSettings { width, height, fps } = settings;
        check_size(width, height)?;
        let failed = |action| EncodeError::codec(codec, action);
        let found = ffmpeg::encoder::find_by_name(codec.name())
            .ok_or_else(|| failed("найти кодек")(ffmpeg::Error::EncoderNotFound))?;
        let mut enc = ffmpeg::codec::context::Context::new_with_codec(found)
            .encoder()
            .video()
            .map_err(failed("создать кодек"))?;
        let rate = i32::try_from(fps)
            .map_err(|_| failed("задать частоту")(ffmpeg::Error::InvalidData))?;
        enc.set_width(width);
        enc.set_height(height);
        enc.set_format(ffmpeg::format::Pixel::NV12);
        enc.set_time_base((1, rate));
        enc.set_frame_rate(Some((rate, 1)));
        enc.set_gop(fps);
        enc.set_max_b_frames(0);
        enc.set_color_range(color::Range::MPEG);
        enc.set_colorspace(color::Space::BT709);
        enc.set_color_primaries(color::Primaries::BT709);
        enc.set_color_transfer_characteristic(color::TransferCharacteristic::BT709);
        // SAFETY: a plain enum field of the owned, not yet opened AVCodecContext.
        unsafe {
            (*enc.as_mut_ptr()).chroma_sample_location =
                ffmpeg::ffi::AVChromaLocation::AVCHROMA_LOC_LEFT;
        }
        enc.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
        let inner = enc
            .open_as_with(found, codec.options(quality))
            .map_err(failed("открыть кодек"))?;
        Ok(Self {
            codec,
            inner,
            fps: i64::from(fps),
            last_key: None,
            scratch: ffmpeg::Packet::empty(),
        })
    }

    pub fn codec(&self) -> Codec {
        self.codec
    }

    pub fn parameters(&self) -> StreamParameters {
        StreamParameters {
            codec: ffmpeg::codec::Parameters::from(&*self.inner),
            time_base: self.inner.time_base(),
        }
    }

    /// Encodes `frame` at slot `pts` (increasing, gaps allowed: the screen did not change).
    /// The first frame and the first one a second or more after the last key frame become key
    /// frames. Finished packets are appended to `out`.
    pub fn encode(
        &mut self,
        frame: &mut Nv12Frame,
        pts: i64,
        out: &mut Vec<Packet>,
    ) -> Result<(), EncodeError> {
        let key = self.last_key.is_none_or(|last| pts - last >= self.fps);
        if key {
            self.last_key = Some(pts);
        }
        frame.set_key(key);
        frame.frame.set_pts(Some(pts));
        self.inner
            .send_frame(&frame.frame)
            .map_err(EncodeError::codec(self.codec, "закодировать кадр"))?;
        self.drain(out)
    }

    pub fn finish(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        self.inner
            .send_eof()
            .map_err(EncodeError::codec(self.codec, "завершить кодирование"))?;
        self.drain(out)
    }

    fn drain(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        loop {
            match self.inner.receive_packet(&mut self.scratch) {
                Ok(()) => {
                    let pts = self
                        .scratch
                        .pts()
                        .ok_or(EncodeError::NoTimestamp { codec: self.codec })?;
                    let packet = std::mem::replace(&mut self.scratch, ffmpeg::Packet::empty());
                    out.push(Packet::new(packet, pts));
                }
                Err(ffmpeg::Error::Other {
                    errno: ffmpeg::util::error::EAGAIN,
                })
                | Err(ffmpeg::Error::Eof) => return Ok(()),
                Err(e) => {
                    return Err(EncodeError::codec(self.codec, "получить пакет")(
                        e,
                    ));
                }
            }
        }
    }
}
