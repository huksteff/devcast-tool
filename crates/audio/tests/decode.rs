//! Task 4.5: every track comes out as 48 kHz stereo `f32`, at its place in time.

mod common;

use common::{channel, frequency, make, peak, read, track};
use dc_audio::{CHANNELS, RATE, tracks};
use dc_core::TimeDelta;
use dc_core::geom::PixelSize;
use dc_core::project::{Media, MediaKind};

fn media(path: &str, kind: MediaKind) -> Media {
    Media {
        path: path.to_owned(),
        kind,
        duration: TimeDelta::from_micros(1),
        size: PixelSize {
            width: 2,
            height: 2,
        },
    }
}

#[test]
fn flac_48k_is_exact() {
    let dir = tempfile::tempdir().unwrap();
    let path = make(
        dir.path(),
        "tone.mp4",
        "aevalsrc=0.5*sin(2*PI*1000*t)|0.25*sin(2*PI*1000*t):s=48000:d=2",
        &["-c:a", "flac"],
    );
    let samples = read(&track(&path), 0, 2 * RATE as usize + 500, 1000);
    let (left, right) = (channel(&samples, 0), channel(&samples, 1));
    let last = left.iter().rposition(|s| *s != 0.0).unwrap();
    assert!(
        (last as i64 - 2 * i64::from(RATE)).abs() <= 1,
        "length {last}"
    );
    assert!((frequency(&left[..96_000]) - 1000.0).abs() < 0.5);
    assert!((peak(&left) - 0.5).abs() < 0.001, "left {}", peak(&left));
    assert!(
        (peak(&right) - 0.25).abs() < 0.001,
        "right {}",
        peak(&right)
    );
}

#[test]
fn mono_44k_becomes_48k_stereo() {
    let dir = tempfile::tempdir().unwrap();
    let path = make(
        dir.path(),
        "tone.wav",
        "sine=frequency=1000:sample_rate=44100:duration=2",
        &["-ac", "1", "-c:a", "pcm_s16le"],
    );
    let samples = read(&track(&path), 0, 2 * RATE as usize + 2000, 4096);
    let left = channel(&samples, 0);
    let end = left.iter().rposition(|s| s.abs() > 0.01).unwrap() as i64;
    assert!((end - 2 * i64::from(RATE)).abs() < 50, "end {end}");
    assert!((frequency(&left[1000..90_000]) - 1000.0).abs() < 0.5);
    assert!((peak(&left[1000..90_000]) - 0.125).abs() < 0.002);
    assert_eq!(left, channel(&samples, 1));
}

#[test]
fn random_access_matches_sequential() {
    let dir = tempfile::tempdir().unwrap();
    for (name, rate, codec) in [("a.mp4", 48_000, "flac"), ("b.wav", 44_100, "pcm_s16le")] {
        let source = format!("anoisesrc=sample_rate={rate}:duration=6:amplitude=0.5:seed=7");
        let path = make(dir.path(), name, &source, &["-ac", "2", "-c:a", codec]);
        let all = read(&track(&path), 0, 6 * RATE as usize, 2048);
        for start in [200_000_i64, 17, 150_000, 3_000] {
            let part = read(&track(&path), start, 10_000, 777);
            let at = start as usize * CHANNELS;
            let worst = part
                .iter()
                .zip(&all[at..at + part.len()])
                .fold(0.0_f32, |m, (a, b)| m.max((a - b).abs()));
            assert_eq!(worst, 0.0, "{name} at {start}");
        }
    }
}

#[test]
fn late_stream_starts_at_its_time() {
    let dir = tempfile::tempdir().unwrap();
    let tone = make(
        dir.path(),
        "tone.flac",
        "sine=frequency=500:sample_rate=48000:duration=1",
        &[],
    );
    let late = dir.path().join("late.mkv");
    let (tone, late_str) = (tone.to_str().unwrap(), late.to_str().unwrap());
    common::ffmpeg(&["-itsoffset", "0.5", "-i", tone, "-c:a", "copy", late_str]);
    let samples = read(&track(&late), 0, RATE as usize, 1024);
    let onset = common::onset(&samples, 0.01) as i64;
    assert!((onset - i64::from(RATE) / 2).abs() <= 48, "onset {onset}");
}

#[test]
fn imported_video_has_each_audio_stream() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("two.mkv");
    common::ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc=duration=1:size=64x64",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=1",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=880:duration=1",
        "-map",
        "0",
        "-map",
        "1",
        "-map",
        "2",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        path.to_str().unwrap(),
    ]);
    let found = tracks(dir.path(), &media("two.mkv", MediaKind::Imported)).unwrap();
    let names: Vec<_> = found.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["audio-1", "audio-2"]);
    for (track, hz) in found.iter().zip([440.0, 880.0]) {
        let left = channel(&read(track, 0, 40_000, 1024), 0);
        assert!(
            (frequency(&left[2000..]) - hz).abs() < 1.0,
            "{}",
            track.name
        );
    }
}

#[test]
fn recording_tracks_come_from_manifest() {
    let video = media("media/recording-1/video.mp4", MediaKind::Recording);
    let found = tracks(&common::project::speech_dir(), &video).unwrap();
    let names: Vec<_> = found.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["system", "mic"]);
    let mic = &found[1];
    let samples = read(mic, 0, 960_516 + 4800, 48_000);
    let left = channel(&samples, 0);
    let last = left.iter().rposition(|s| *s != 0.0).unwrap();
    assert!(
        last < 960_516 && last > 960_516 - 4800,
        "mic ends at {last}"
    );
    assert!(peak(&left) > 0.05, "speech is there");
}
