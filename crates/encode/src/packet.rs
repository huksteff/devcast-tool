//! Encoded H.264 data and the stream description a file needs to hold it.

use ffmpeg_next as ffmpeg;

/// One encoded frame. Time is in the stream's time base (one tick per frame slot); there are
/// no B-frames, so decode order is presentation order.
pub struct Packet {
    pub(crate) inner: ffmpeg::Packet,
    pts: i64,
}

impl Packet {
    pub(crate) fn new(mut inner: ffmpeg::Packet, pts: i64) -> Self {
        inner.set_pts(Some(pts));
        inner.set_dts(Some(pts));
        Self { inner, pts }
    }

    pub fn pts(&self) -> i64 {
        self.pts
    }

    /// An IDR frame: decoding can start here.
    pub fn is_key(&self) -> bool {
        self.inner.is_key()
    }

    /// How long the frame stays on screen, in ticks.
    pub fn duration(&self) -> i64 {
        self.inner.duration()
    }

    pub fn set_duration(&mut self, ticks: i64) {
        self.inner.set_duration(ticks);
    }

    /// Moves the packet in time by `ticks`.
    pub fn shift(&mut self, ticks: i64) {
        self.pts += ticks;
        self.inner.set_pts(Some(self.pts));
        self.inner.set_dts(Some(self.pts));
    }
}

/// Codec setup of a video stream (size, profile, SPS/PPS, color tags) and its time base.
#[derive(Clone)]
pub struct VideoParameters {
    pub(crate) codec: ffmpeg::codec::Parameters,
    pub(crate) time_base: ffmpeg::Rational,
}

impl VideoParameters {
    /// Seconds per tick as `(numerator, denominator)`.
    pub fn time_base(&self) -> (i32, i32) {
        (self.time_base.numerator(), self.time_base.denominator())
    }
}
