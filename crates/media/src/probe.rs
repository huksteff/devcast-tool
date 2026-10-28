//! What a video file holds: its streams and, for the video stream, frame size, length, rate and
//! keyframes. The video stream is read packet by packet without decoding, so the keyframe list and
//! the variable-rate flag are exact.
//!
//! Source time starts at the first frame: `SourceTime::ZERO` is the earliest presentation
//! timestamp of the video stream. Every stream of the file counts from that point
//! ([`Probe::origin_micros`]), as players keep the streams' timestamps relative to each other:
//! sound that starts before the first frame has negative source time.

use std::path::Path;

use dc_core::{PixelSize, SourceTime, TimeDelta};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::Rescale;
use serde::{Deserialize, Serialize};

use crate::error::MediaError;

/// One microsecond as an FFmpeg time base.
pub(crate) const MICROS: ffmpeg::Rational = ffmpeg::Rational(1, 1_000_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamKind {
    Video,
    Audio,
    Subtitle,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamInfo {
    pub index: usize,
    pub kind: StreamKind,
    /// FFmpeg codec name: `h264`, `vp9`, `aac`, `flac` …
    pub codec: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Probe {
    pub streams: Vec<StreamInfo>,
    /// Index of the video stream that is shown (FFmpeg's best video stream).
    pub video: usize,
    pub size: PixelSize,
    /// From the first frame to the end of the last one.
    pub duration: TimeDelta,
    pub frames: u64,
    /// Average frames per second: `frames / duration`.
    pub fps: f64,
    /// Frame intervals differ by more than the rounding of the container's time base.
    pub variable_rate: bool,
    /// Presentation time of every keyframe, ascending; never empty.
    pub keyframes: Vec<SourceTime>,
    /// Timestamp of the first frame in the stream's time base: source time zero.
    pub first_pts: i64,
    /// Source time zero on the file's own clock, which all its streams share, microseconds:
    /// `first_pts` rescaled. Other streams (sound) subtract it from their timestamps.
    pub origin_micros: i64,
}

impl Probe {
    /// The last keyframe at or before `t` (the first keyframe for `t` before it).
    pub fn keyframe_before(&self, t: SourceTime) -> SourceTime {
        let after = self.keyframes.partition_point(|k| *k <= t);
        self.keyframes[after.saturating_sub(1)]
    }
}

/// Timestamps of the video packets, read without decoding.
struct Packets {
    /// Presentation timestamp and duration of each frame, in the stream's time base.
    frames: Vec<(i64, i64)>,
    keyframes: Vec<i64>,
}

pub fn probe(path: &Path) -> Result<Probe, MediaError> {
    crate::init();
    let mut input = ffmpeg::format::input(path).map_err(|source| MediaError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    let streams = input.streams().map(|s| stream_info(&s)).collect();
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| MediaError::NoVideo {
            path: path.to_path_buf(),
        })?;
    let video = stream.index();
    let time_base = stream.time_base();
    let size = frame_size(&stream.parameters()).map_err(|source| MediaError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let packets = read_packets(&mut input, video, path)?;
    let Some(first_pts) = packets.frames.iter().map(|(pts, _)| *pts).min() else {
        return Err(MediaError::NoFrames {
            path: path.to_path_buf(),
        });
    };
    let mut starts: Vec<i64> = packets.frames.iter().map(|(pts, _)| *pts).collect();
    starts.sort_unstable();
    let intervals: Vec<i64> = starts.windows(2).map(|w| w[1] - w[0]).collect();
    let end = end_pts(&packets.frames, &intervals);
    let to_micros = |pts: i64| (pts - first_pts).rescale(time_base, MICROS);
    let duration = TimeDelta::from_micros(to_micros(end));
    let frames = starts.len() as u64;
    let mut keyframes: Vec<SourceTime> = packets
        .keyframes
        .iter()
        .map(|pts| SourceTime::from_micros(to_micros(*pts)))
        .collect();
    keyframes.sort_unstable();
    keyframes.dedup();
    if keyframes.is_empty() {
        return Err(MediaError::NoKeyframes {
            path: path.to_path_buf(),
        });
    }
    Ok(Probe {
        streams,
        video,
        size,
        duration,
        frames,
        fps: frames as f64 / duration.as_secs_f64(),
        variable_rate: is_variable(&intervals),
        keyframes,
        first_pts,
        origin_micros: first_pts.rescale(time_base, MICROS),
    })
}

fn stream_info(stream: &ffmpeg::format::stream::Stream<'_>) -> StreamInfo {
    let parameters = stream.parameters();
    StreamInfo {
        index: stream.index(),
        kind: match parameters.medium() {
            ffmpeg::media::Type::Video => StreamKind::Video,
            ffmpeg::media::Type::Audio => StreamKind::Audio,
            ffmpeg::media::Type::Subtitle => StreamKind::Subtitle,
            _ => StreamKind::Other,
        },
        codec: parameters.id().name().to_owned(),
    }
}

fn frame_size(parameters: &ffmpeg::codec::Parameters) -> Result<PixelSize, ffmpeg::Error> {
    let context = ffmpeg::codec::context::Context::from_parameters(parameters.clone())?;
    let decoder = context.decoder().video()?;
    Ok(PixelSize {
        width: decoder.width(),
        height: decoder.height(),
    })
}

fn read_packets(
    input: &mut ffmpeg::format::context::Input,
    video: usize,
    path: &Path,
) -> Result<Packets, MediaError> {
    let mut packets = Packets {
        frames: Vec::new(),
        keyframes: Vec::new(),
    };
    loop {
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(input) {
            Ok(()) => {}
            Err(ffmpeg::Error::Eof) => return Ok(packets),
            Err(source) => {
                return Err(MediaError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
        if packet.stream() != video {
            continue;
        }
        let Some(pts) = packet.pts() else { continue };
        packets.frames.push((pts, packet.duration()));
        if packet.is_key() {
            packets.keyframes.push(pts);
        }
    }
}

/// End of the last frame: its start plus its duration, or plus the interval before it when the
/// container does not store durations.
fn end_pts(frames: &[(i64, i64)], intervals: &[i64]) -> i64 {
    let (last, duration) = frames
        .iter()
        .copied()
        .max_by_key(|(pts, _)| *pts)
        .unwrap_or_default();
    match duration {
        d if d > 0 => last + d,
        _ => last + intervals.last().copied().unwrap_or(1),
    }
}

/// A container rounds each timestamp to its time base, so a constant rate shows intervals that
/// differ by one tick at most (33 and 34 ms for 30 fps in Matroska).
fn is_variable(intervals: &[i64]) -> bool {
    match (intervals.iter().min(), intervals.iter().max()) {
        (Some(min), Some(max)) => max - min > 1,
        _ => false,
    }
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
