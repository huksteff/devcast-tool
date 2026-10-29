//! Choosing the encoder: for a recording, the first codec in [`Codec::ORDER`] that passes the
//! self-test and keeps up with the recording's size and frame rate; for an export, the first that
//! passes the self-test and opens.

use std::sync::OnceLock;
use std::time::Instant;

use ffmpeg_next as ffmpeg;

use crate::codec::{Codec, Quality};
use crate::encoder::{VideoEncoder, VideoSettings};
use crate::error::EncodeError;
use crate::frame::Nv12Frame;
use crate::packet::Packet;

/// The codec must encode this much faster than real time: real screens are harder to encode
/// than the test picture, and capture shares the machine.
const SPEED_MARGIN: f64 = 1.25;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selection {
    pub codec: Codec,
    /// Frames per second the codec encoded the test picture at the recording size.
    pub speed: f64,
}

/// Picks the codec for `settings`. The self-test runs once per codec per process; the speed
/// check runs every time, at the real size.
pub fn select(settings: VideoSettings) -> Result<Selection, EncodeError> {
    let mut problems = Vec::new();
    let mut fastest: Option<Selection> = None;
    for codec in Codec::ORDER {
        if let Err(problem) = self_test_once(codec) {
            problems.push(format!("{codec}: {problem}"));
            continue;
        }
        let speed = match measure_speed(codec, settings) {
            Ok(speed) => speed,
            Err(e) => {
                problems.push(format!("{codec}: {e}"));
                continue;
            }
        };
        let selection = Selection { codec, speed };
        if speed >= f64::from(settings.fps) * SPEED_MARGIN {
            return Ok(selection);
        }
        if fastest.is_none_or(|f| f.speed < speed) {
            fastest = Some(selection);
        }
    }
    match fastest {
        Some(Selection { codec, speed }) => Err(EncodeError::TooSlow {
            codec,
            measured: speed,
            needed: f64::from(settings.fps) * SPEED_MARGIN,
            fps: settings.fps,
            width: settings.width,
            height: settings.height,
        }),
        None => Err(EncodeError::NoCodec {
            details: problems.join("; "),
        }),
    }
}

/// Opens the encoder of an export: the first codec in [`Codec::ORDER`] that passes the self-test
/// and opens at the export's size. No speed check: an export may take longer than real time.
pub fn open_for_export(
    settings: VideoSettings,
    quality: Quality,
) -> Result<VideoEncoder, EncodeError> {
    let mut problems = Vec::new();
    for codec in Codec::ORDER {
        let opened = self_test_once(codec).and_then(|()| {
            VideoEncoder::open_with(codec, settings, quality).map_err(|e| e.to_string())
        });
        match opened {
            Ok(encoder) => return Ok(encoder),
            Err(problem) => problems.push(format!("{codec}: {problem}")),
        }
    }
    Err(EncodeError::NoCodec {
        details: problems.join("; "),
    })
}

fn self_test_once(codec: Codec) -> Result<(), String> {
    static RESULTS: [OnceLock<Result<(), String>>; Codec::ORDER.len()] =
        [const { OnceLock::new() }; Codec::ORDER.len()];
    RESULTS[codec as usize]
        .get_or_init(|| self_test(codec))
        .clone()
}

/// Encodes mid-gray frames with a forced key frame after a gap, decodes them with FFmpeg's own
/// decoder and checks the picture: some drivers open a hardware session and then emit zeroes.
pub fn self_test(codec: Codec) -> Result<(), String> {
    let settings = VideoSettings {
        width: 640,
        height: 360,
        fps: 30,
    };
    let mut encoder = VideoEncoder::open(codec, settings).map_err(|e| e.to_string())?;
    let mut frame = Nv12Frame::new(settings.width, settings.height).map_err(|e| e.to_string())?;
    frame.fill(128, 128, 128);
    let mut packets = Vec::new();
    let slots = [0, 1, 2, 3, 4, 30, 31, 32, 33, 34];
    for pts in slots {
        encoder
            .encode(&mut frame, pts, &mut packets)
            .map_err(|e| e.to_string())?;
    }
    encoder.finish(&mut packets).map_err(|e| e.to_string())?;
    let keys: Vec<i64> = packets
        .iter()
        .filter(|p| p.is_key())
        .map(Packet::pts)
        .collect();
    if !keys.contains(&0) || !keys.contains(&30) {
        return Err(format!("ключевые кадры {keys:?} вместо 0 и 30"));
    }
    let lumas = decode_luma(&encoder, &packets).map_err(|e| format!("декодирование: {e}"))?;
    if lumas.len() != slots.len() {
        return Err(format!(
            "раскодировано {} кадров из {}",
            lumas.len(),
            slots.len()
        ));
    }
    match lumas.iter().find(|l| !(100.0..=156.0).contains(*l)) {
        Some(luma) => Err(format!("яркость серого кадра {luma:.1} вместо 128")),
        None => Ok(()),
    }
}

/// Mean luma of the top 16 rows of every decoded frame.
fn decode_luma(encoder: &VideoEncoder, packets: &[Packet]) -> Result<Vec<f64>, ffmpeg::Error> {
    let params = encoder.parameters();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(params.codec)?
        .decoder()
        .video()?;
    let mut decoded = ffmpeg::frame::Video::empty();
    let mut lumas = Vec::new();
    let mut take = |decoder: &mut ffmpeg::decoder::Video, lumas: &mut Vec<f64>| loop {
        match decoder.receive_frame(&mut decoded) {
            Ok(()) => {
                let width = decoded.width() as usize;
                let rows = decoded.data(0).chunks(decoded.stride(0)).take(16);
                let sum: u64 = rows.flat_map(|r| &r[..width]).map(|&v| u64::from(v)).sum();
                lumas.push(sum as f64 / (width * 16) as f64);
            }
            Err(ffmpeg::Error::Other {
                errno: ffmpeg::util::error::EAGAIN,
            })
            | Err(ffmpeg::Error::Eof) => return Ok(()),
            Err(e) => return Err(e),
        }
    };
    for packet in packets {
        decoder.send_packet(&packet.inner)?;
        take(&mut decoder, &mut lumas)?;
    }
    decoder.send_eof()?;
    take(&mut decoder, &mut lumas)?;
    Ok(lumas)
}

/// Encodes one second of a moving detailed picture at the real size; returns frames per
/// second, stopping early once the codec is clearly too slow.
fn measure_speed(codec: Codec, settings: VideoSettings) -> Result<f64, EncodeError> {
    let mut frames = (0..4)
        .map(|k| test_picture(settings, k))
        .collect::<Result<Vec<_>, _>>()?;
    let mut encoder = VideoEncoder::open(codec, settings)?;
    let count = settings.fps as usize;
    let budget = count as f64 / (f64::from(settings.fps) * SPEED_MARGIN);
    let mut packets = Vec::new();
    let started = Instant::now();
    let picture_count = frames.len();
    for i in 0..count {
        encoder.encode(&mut frames[i % picture_count], i as i64, &mut packets)?;
        packets.clear();
        let elapsed = started.elapsed().as_secs_f64();
        if elapsed > budget {
            return Ok((i + 1) as f64 / elapsed);
        }
    }
    encoder.finish(&mut packets)?;
    Ok(count as f64 / started.elapsed().as_secs_f64())
}

/// Blocks of different brightness with fine detail, shifted by `k` × 8 px: something like
/// text and windows on a screen.
fn test_picture(settings: VideoSettings, k: usize) -> Result<Nv12Frame, EncodeError> {
    let mut frame = Nv12Frame::new(settings.width, settings.height)?;
    for (y, row) in frame.luma_rows_mut().enumerate() {
        for (x, v) in row.iter_mut().enumerate() {
            let block = ((x + 8 * k) >> 3) * 7 + (y >> 3) * 13;
            *v = (block ^ ((x * y) >> 5)) as u8;
        }
    }
    Ok(frame)
}
