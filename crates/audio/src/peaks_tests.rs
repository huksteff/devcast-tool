use std::path::Path;

use super::*;
use crate::test_util::{ffmpeg, track};

/// `name` in `dir` from the lavfi source `source`, stereo.
fn make(dir: &Path, name: &str, source: &str) -> PathBuf {
    let path = dir.join(name);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        source,
        "-ac",
        "2",
        path.to_str().unwrap(),
    ]);
    path
}

/// 1 s of silence, then 1 s of a 0.5 sine.
const HALF_LOUD: &str = "aevalsrc='0.5*sin(2*PI*440*t)*gte(t,1)':s=48000:d=2";

#[test]
fn quiet_and_loud_halves() {
    let dir = tempfile::tempdir().unwrap();
    let peaks = Peaks::build(&track(&make(dir.path(), "half.flac", HALF_LOUD))).unwrap();
    assert_eq!(peaks.frames(), 96_000);
    // Exact to BASE frames: the block holding the start of the sound (frame 48000) is loud.
    let quiet = peaks.range(0, 48_000 / BASE * BASE, 3);
    assert_eq!(quiet, vec![Peak::default(); 3]);
    let halves = peaks.range(0, 96_000, 2);
    // `-ac 2` spreads the mono sine over two channels at 1/√2.
    let amplitude = 0.5 * std::f32::consts::FRAC_1_SQRT_2;
    assert!(
        (halves[1].max - amplitude).abs() < 0.01 && (halves[1].min + amplitude).abs() < 0.01,
        "{halves:?}"
    );
    let outside = peaks.range(96_000, 200_000, 3);
    assert_eq!(outside, vec![Peak::default(); 3]);
}

#[test]
fn every_level_gives_the_same_envelope() {
    let peaks = Peaks::from_finest(
        BASE * 64,
        (0..64)
            .map(|i| Peak {
                min: -(i as f32) / 64.0,
                max: (i % 7) as f32 / 7.0,
            })
            .collect(),
    );
    assert_eq!(peaks.levels.len(), 4, "64 → 16 → 4 → 1");
    let fine: Vec<Peak> = (0..4)
        .map(|q| {
            peaks
                .range(q * 16 * BASE, (q + 1) * 16 * BASE, 16)
                .into_iter()
                .fold(Peak::default(), Peak::join)
        })
        .collect();
    assert_eq!(
        peaks.range(0, 64 * BASE, 4),
        fine,
        "a coarse level equals its fine peaks joined"
    );
    assert_eq!(
        peaks.range(0, 64 * BASE, 1)[0],
        Peak {
            min: -63.0 / 64.0,
            max: 6.0 / 7.0
        }
    );
}

#[test]
fn cache_is_reused_and_rebuilt_when_broken() {
    let tmp = tempfile::tempdir().unwrap();
    let project = ProjectDir::create(tmp.path().join("Волна проекта.devcast")).unwrap();
    let path = make(&project.media_dir(), "звук.flac", HALF_LOUD);
    let built = cached_peaks(&project, &track(&path)).unwrap();
    let cache = cache_path(&project, &track(&path));
    assert!(
        cache.ends_with("cache/media/звук.flac.0.peaks"),
        "{}",
        cache.display()
    );
    let again = cached_peaks(&project, &track(&path)).unwrap();
    assert_eq!(again.frames(), built.frames());
    // Quantized to 1/32767, never smaller than the real peak.
    for (a, b) in again.levels[0].iter().zip(&built.levels[0]) {
        assert!(a.max >= b.max && a.max - b.max < 1e-4 && a.min <= b.min && b.min - a.min < 1e-4);
    }
    std::fs::write(&cache, b"DCPK broken").unwrap();
    assert_eq!(cached_peaks(&project, &track(&path)).unwrap(), built);
    let moved = SourceTrack {
        origin: 1000,
        ..track(&path)
    };
    assert_eq!(
        cached_peaks(&project, &moved).unwrap().frames(),
        built.frames() - 48,
        "another start (1 ms later) rebuilds"
    );
}
