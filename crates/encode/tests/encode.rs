//! Encoding with the CPU codec (present on every machine) into a file, read back with FFmpeg.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

use dc_encode::{
    Codec, EncodeError, Mp4Layout, Mp4Reader, Mp4Writer, Nv12Frame, VideoEncoder, VideoSettings,
    select,
};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::util::color;

const SETTINGS: VideoSettings = VideoSettings {
    width: 320,
    height: 180,
    fps: 60,
};

#[test]
fn x264_passes_self_test() {
    dc_encode::select::self_test(Codec::X264).unwrap();
}

#[test]
fn some_codec_is_selected() {
    let selection = select(SETTINGS).unwrap();
    assert!(selection.speed >= 75.0, "{selection:?}");
}

#[test]
fn impossible_rate_is_refused_before_recording() {
    let settings = VideoSettings {
        fps: 1_000_000,
        ..SETTINGS
    };
    let error = select(settings).unwrap_err();
    assert!(matches!(error, EncodeError::TooSlow { .. }), "{error}");
    assert!(error.to_string().contains("выбери частоту ниже"), "{error}");
}

/// Frames arrive only when the screen changes: slots with gaps, key frames by time.
#[test]
fn variable_rate_file_has_key_frames_every_second_and_bt709_tags() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("запись с пробелом.mp4");
    let slots = [0, 1, 2, 3, 50, 51, 52, 130, 131, 200];
    let end = 230;
    let mut encoder = VideoEncoder::open(Codec::X264, SETTINGS).unwrap();
    let mut writer = Mp4Writer::create(&path, &encoder.parameters(), Mp4Layout::Plain).unwrap();
    let mut frame = Nv12Frame::new(SETTINGS.width, SETTINGS.height).unwrap();
    let mut packets = Vec::new();
    for (i, pts) in slots.iter().enumerate() {
        let luma = 40 + 20 * i as u8;
        frame.fill(luma, 128, 128);
        encoder.encode(&mut frame, *pts, &mut packets).unwrap();
    }
    encoder.finish(&mut packets).unwrap();
    let mut next = slots[1..].iter().copied().chain([end]);
    for mut packet in packets {
        packet.set_duration(next.next().unwrap() - packet.pts());
        writer.write(packet).unwrap();
    }
    writer.finish().unwrap();

    let mut input = ffmpeg::format::input(&path).unwrap();
    let stream = input.streams().best(ffmpeg::media::Type::Video).unwrap();
    let base = f64::from(stream.time_base());
    let duration = stream.duration() as f64 * base;
    assert!((duration - end as f64 / 60.0).abs() < 0.001, "{duration}");
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .unwrap()
        .decoder()
        .video()
        .unwrap();
    let mut times = Vec::new();
    let mut keys = Vec::new();
    let mut decoded = ffmpeg::frame::Video::empty();
    let mut first = None;
    for (_, packet) in input.packets() {
        let slot = (packet.pts().unwrap() as f64 * base * 60.0).round() as i64;
        times.push(slot);
        if packet.is_key() {
            keys.push(slot);
        }
        decoder.send_packet(&packet).unwrap();
        while decoder.receive_frame(&mut decoded).is_ok() {
            first.get_or_insert_with(|| decoded.clone());
        }
    }
    assert_eq!(times, slots);
    assert_eq!(keys, [0, 130, 200]);
    let decoded = first.unwrap();
    assert_eq!(decoded.color_range(), color::Range::MPEG);
    assert_eq!(decoded.color_space(), color::Space::BT709);
    assert_eq!(decoded.color_primaries(), color::Primaries::BT709);
    assert_eq!(
        decoded.color_transfer_characteristic(),
        color::TransferCharacteristic::BT709
    );
}

/// Recording segments are fragmented files; reading one back gives the same packets.
#[test]
fn fragmented_file_reads_back_with_times_keys_and_durations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("сегмент.mp4");
    let mut encoder = VideoEncoder::open(Codec::X264, SETTINGS).unwrap();
    let mut writer =
        Mp4Writer::create(&path, &encoder.parameters(), Mp4Layout::Fragmented).unwrap();
    let mut frame = Nv12Frame::new(SETTINGS.width, SETTINGS.height).unwrap();
    let mut packets = Vec::new();
    let slots = [0, 1, 2, 70, 71, 140];
    for pts in slots {
        encoder.encode(&mut frame, pts, &mut packets).unwrap();
    }
    encoder.finish(&mut packets).unwrap();
    let mut next = slots[1..].iter().copied().chain([150]);
    for mut packet in packets {
        packet.set_duration(next.next().unwrap() - packet.pts());
        writer.write(packet).unwrap();
    }
    writer.finish().unwrap();

    let mut reader = Mp4Reader::open(&path, (1, 60)).unwrap();
    assert_eq!(reader.parameters().time_base(), (1, 60));
    let mut read = Vec::new();
    while let Some(packet) = reader.read().unwrap() {
        read.push((packet.pts(), packet.duration(), packet.is_key()));
    }
    assert_eq!(
        read,
        [
            (0, 1, true),
            (1, 1, false),
            (2, 68, false),
            (70, 1, true),
            (71, 69, false),
            (140, 10, true)
        ]
    );
}

/// Bytes of 30 frames of a detailed moving picture at `quality`.
fn encoded_bytes(codec: Codec, quality: dc_encode::Quality) -> usize {
    let mut encoder = VideoEncoder::open_with(codec, SETTINGS, quality).unwrap();
    let mut frame = Nv12Frame::new(SETTINGS.width, SETTINGS.height).unwrap();
    let mut packets = Vec::new();
    for pts in 0..30 {
        for (y, row) in frame.luma_rows_mut().enumerate() {
            for (x, v) in row.iter_mut().enumerate() {
                *v = ((x * 7 + y * 13 + pts as usize * 5) ^ ((x * y) >> 3)) as u8;
            }
        }
        encoder.encode(&mut frame, pts, &mut packets).unwrap();
    }
    encoder.finish(&mut packets).unwrap();
    packets.iter().map(|p| p.size()).sum()
}

#[test]
fn lower_quality_makes_smaller_files() {
    use dc_encode::Quality;
    let sizes =
        [Quality::Low, Quality::Medium, Quality::High].map(|q| encoded_bytes(Codec::X264, q));
    println!("x264 low/medium/high: {sizes:?}");
    assert!(sizes[0] < sizes[1] && sizes[1] < sizes[2], "{sizes:?}");
}
