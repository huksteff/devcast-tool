//! Speed with the pitch kept: FFmpeg's `atempo` (libavfilter) on the common format.
//!
//! Output frame `j` plays input time `j · speed`: the filter keeps the start in place.
//! `atempo` sounds best inside 0.5–2×, so other speeds are a chain of such steps.

use ffmpeg_next as ffmpeg;
use ffmpeg_next::ChannelLayout;
use ffmpeg_next::format::Sample;
use ffmpeg_next::format::sample::Type as SampleType;

use crate::error::AudioError;
use crate::{CHANNELS, RATE};

pub(crate) struct Tempo {
    graph: ffmpeg::filter::Graph,
    speed: f64,
    /// Input frames fed so far: the timestamp of the next input frame.
    fed: i64,
    frame: ffmpeg::frame::Audio,
}

/// The `atempo` steps for `speed`, each within 0.5–2.
pub(crate) fn chain(speed: f64) -> Vec<f64> {
    let mut steps = Vec::new();
    let mut rest = speed;
    while rest < 0.5 {
        steps.push(0.5);
        rest /= 0.5;
    }
    while rest > 2.0 {
        steps.push(2.0);
        rest /= 2.0;
    }
    steps.push(rest);
    steps
}

impl Tempo {
    pub fn new(speed: f64) -> Result<Self, AudioError> {
        crate::init();
        let failed = |source| AudioError::Tempo { speed, source };
        let filter = |name| ffmpeg::filter::find(name).ok_or(failed(ffmpeg::Error::FilterNotFound));
        let mut graph = ffmpeg::filter::Graph::new();
        let args =
            format!("time_base=1/{RATE}:sample_rate={RATE}:sample_fmt=flt:channel_layout=stereo");
        graph
            .add(&filter("abuffer")?, "in", &args)
            .map_err(failed)?;
        graph
            .add(&filter("abuffersink")?, "out", "")
            .map_err(failed)?;
        let steps: Vec<String> = chain(speed).iter().map(|s| format!("atempo={s}")).collect();
        graph
            .output("in", 0)
            .and_then(|p| p.input("out", 0))
            .and_then(|p| p.parse(&steps.join(",")))
            .map_err(failed)?;
        graph.validate().map_err(failed)?;
        Ok(Self {
            graph,
            speed,
            fed: 0,
            frame: ffmpeg::frame::Audio::empty(),
        })
    }

    /// Feeds `input` and appends the frames that are ready to `out`.
    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<(), AudioError> {
        let speed = self.speed;
        let failed = |source| AudioError::Tempo { speed, source };
        let frames = input.len() / CHANNELS;
        let mut frame = ffmpeg::frame::Audio::new(
            Sample::F32(SampleType::Packed),
            frames,
            ChannelLayout::STEREO,
        );
        frame.set_rate(RATE);
        frame.set_pts(Some(self.fed));
        frame.plane_mut::<f32>(0)[..input.len()].copy_from_slice(input);
        self.fed += frames as i64;
        let mut source = self
            .graph
            .get("in")
            .ok_or(failed(ffmpeg::Error::FilterNotFound))?;
        source.source().add(&frame).map_err(failed)?;
        let mut sink = self
            .graph
            .get("out")
            .ok_or(failed(ffmpeg::Error::FilterNotFound))?;
        loop {
            match sink.sink().frame(&mut self.frame) {
                Ok(()) => {
                    let samples = self.frame.samples() * CHANNELS;
                    out.extend_from_slice(&self.frame.plane::<f32>(0)[..samples]);
                }
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {
                    return Ok(());
                }
                Err(e) => return Err(failed(e)),
            }
        }
    }
}

#[cfg(test)]
#[path = "tempo_tests.rs"]
mod tests;
