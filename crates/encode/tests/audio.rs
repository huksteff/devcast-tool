//! FLAC sound into MP4 and back: every sample is kept, at its place.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

use dc_encode::{AudioEncoder, AudioSettings, Mp4Layout, Mp4Reader, Mp4Writer};
use ffmpeg_next as ffmpeg;

const SETTINGS: AudioSettings = AudioSettings {
    rate: 48_000,
    channels: 2,
};

/// A 440 Hz tone on the left, silence on the right; `frames` samples per channel.
fn tone(frames: usize) -> Vec<f32> {
    (0..frames)
        .flat_map(|i| {
            let left = 0.5 * (std::f32::consts::TAU * 440.0 * i as f32 / 48_000.0).sin();
            [left, 0.0]
        })
        .collect()
}

/// Decodes the whole sound stream into interleaved i16 samples.
fn decode(path: &std::path::Path) -> Vec<i16> {
    let mut input = ffmpeg::format::input(path).unwrap();
    let stream = input.streams().best(ffmpeg::media::Type::Audio).unwrap();
    let index = stream.index();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .unwrap()
        .decoder()
        .audio()
        .unwrap();
    let mut samples = Vec::new();
    let mut frame = ffmpeg::frame::Audio::empty();
    let mut take = |decoder: &mut ffmpeg::decoder::Audio| {
        while decoder.receive_frame(&mut frame).is_ok() {
            let bytes = &frame.data(0)[..frame.samples() * 2 * 2];
            samples.extend(
                bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| i16::from_le_bytes(*b)),
            );
        }
    };
    for (stream, packet) in input.packets() {
        if stream.index() == index {
            decoder.send_packet(&packet).unwrap();
            take(&mut decoder);
        }
    }
    decoder.send_eof().unwrap();
    take(&mut decoder);
    samples
}

#[test]
fn flac_keeps_every_sample_through_fragments_and_joining() {
    let dir = tempfile::tempdir().unwrap();
    let fragmented = dir.path().join("звук сегмент.mp4");
    let plain = dir.path().join("звук.mp4");
    // Not a multiple of the encoder frame: the last frame is shorter.
    let frames = 48_000 * 3 + 123;
    let input = tone(frames);
    let mut encoder = AudioEncoder::open(SETTINGS).unwrap();
    let parameters = encoder.parameters();
    assert_eq!(parameters.time_base(), (1, 48_000));
    let mut writer = Mp4Writer::create(&fragmented, &parameters, Mp4Layout::Fragmented).unwrap();
    let mut packets = Vec::new();
    // Uneven pieces, as a capture device delivers them.
    for piece in input.chunks(2 * 441) {
        encoder.encode(piece, &mut packets).unwrap();
    }
    encoder.finish(&mut packets).unwrap();
    assert!(packets.iter().all(|p| p.is_key()));
    let mut next = 0;
    for packet in packets {
        assert_eq!(packet.pts(), next);
        next += packet.duration();
        writer.write(packet).unwrap();
    }
    assert_eq!(next, frames as i64);
    writer.finish().unwrap();

    let mut reader = Mp4Reader::open(&fragmented, (1, 48_000)).unwrap();
    let mut joined = Mp4Writer::create(&plain, reader.parameters(), Mp4Layout::Plain).unwrap();
    while let Some(packet) = reader.read().unwrap() {
        joined.write(packet).unwrap();
    }
    joined.finish().unwrap();

    let decoded = decode(&plain);
    assert_eq!(decoded.len(), frames * 2);
    for (got, want) in decoded.iter().zip(&input) {
        assert!((f32::from(*got) / 32767.0 - want).abs() < 1.0 / 32767.0);
    }
}

#[test]
fn louder_than_full_scale_clips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clip.mp4");
    let mut encoder = AudioEncoder::open(SETTINGS).unwrap();
    let mut writer = Mp4Writer::create(&path, &encoder.parameters(), Mp4Layout::Plain).unwrap();
    let mut packets = Vec::new();
    encoder
        .encode(&[1.5, -1.5, 0.25, -0.25], &mut packets)
        .unwrap();
    encoder.finish(&mut packets).unwrap();
    for packet in packets {
        writer.write(packet).unwrap();
    }
    writer.finish().unwrap();
    assert_eq!(decode(&path), [32767, -32768, 8192, -8192]);
}
