//! Sound and picture of one file share source time zero (the first video frame), so a click
//! and a flash made together stay together, and an offset in the file stays that offset.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use common::project::project_dir;
use dc_audio::{RATE, tracks};
use dc_core::geom::PixelSize;
use dc_core::project::{Media, MediaKind};
use dc_core::{SourceTime, TimeDelta};
use dc_media::{Decoder, FramePool, cached_probe};

const FPS: i64 = 30;

/// A video with a white frame at 1 s and a click at 1 s of its sound, the sound shifted by
/// `offset` seconds (`-itsoffset`), in a project folder.
fn flash_and_click(dir: &std::path::Path, name: &str, offset: &str, codec: &str) -> Media {
    std::fs::create_dir_all(dir.join("media")).unwrap();
    let path = dir.join("media").join(name);
    let video =
        format!("color=c=black:s=64x64:r={FPS}:d=3,geq=lum='if(eq(N,{FPS}),235,16)':cb=128:cr=128");
    let sound = "aevalsrc='if(between(t,1,1+1/960),0.9,0)':s=48000:d=3";
    common::ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        &video,
        "-itsoffset",
        offset,
        "-f",
        "lavfi",
        "-i",
        sound,
        "-map",
        "0:v",
        "-map",
        "1:a",
        "-c:v",
        "libx264",
        "-g",
        "15",
        "-c:a",
        codec,
        path.to_str().unwrap(),
    ]);
    Media {
        path: format!("media/{name}"),
        kind: MediaKind::Imported,
        duration: TimeDelta::from_secs_f64(3.0),
        size: PixelSize {
            width: 64,
            height: 64,
        },
    }
}

/// Source time of the white frame, by the video decoder.
fn flash(project: &dc_project::ProjectDir, media: &Media) -> f64 {
    let probe = cached_probe(project, &media.path).unwrap();
    let frames = probe.frames as i64;
    let mut decoder = Decoder::open(
        &project.root().join(&media.path),
        probe,
        FramePool::new(1 << 20),
    )
    .unwrap();
    (0..frames)
        .map(|n| {
            decoder
                .frame_at(SourceTime::from_micros(n * 1_000_000 / FPS))
                .unwrap()
        })
        .find(|frame| frame.y()[0] > 128)
        .map(|frame| frame.time.as_secs_f64())
        .unwrap()
}

/// Source time of the click, by the audio reader.
fn click(project: &dc_project::ProjectDir, media: &Media) -> f64 {
    let track = &tracks(project, media).unwrap()[0];
    let samples = common::read(track, 0, 3 * RATE as usize, 4096);
    common::onset(&samples, 0.5) as f64 / f64::from(RATE)
}

#[test]
fn click_and_flash_keep_their_offset_in_source_time() {
    let dir = tempfile::tempdir().unwrap();
    let project = project_dir(dir.path());
    for (offset, expected) in [("0", 0.0), ("0.3", 0.3), ("-0.2", -0.2)] {
        for (name, codec) in [("flac.mp4", "flac"), ("aac.mp4", "aac"), ("aac.mkv", "aac")] {
            let name = format!("{offset}-{name}");
            let media = flash_and_click(dir.path(), &name, offset, codec);
            let (flash, click) = (flash(&project, &media), click(&project, &media));
            println!("{name}: flash {flash:.4} s, click {click:.4} s");
            let off = click - flash - expected;
            assert!(
                off.abs() < 0.001,
                "{name}: click − flash − offset = {off:.4} s"
            );
        }
    }
}
