//! Task 5.15: cuts and speed in picture and sound of the exported file. A video flashes white and
//! beeps for 0.1 s at every whole second of its source; after a join of two clips, 10 cuts and a
//! 2× part, every flash and every beep of the MP4 must land where the time map puts that second.

use std::ops::ControlFlow;
use std::path::Path;
use std::sync::Arc;

use dc_core::{SourceTime, TimeMap};
use dc_ops::export::{Format, Quality, Res, Settings, Spec, run};
use dc_project::ProjectDir;
use dc_render::{AdapterChoice, Gpu};

use crate::test_util::{decoded, ffmpeg, run_line};

const FPS: u32 = 30;
const RATE: usize = 48_000;
const SECONDS: u32 = 12;

/// `secs` of black with a white flash and a 1 kHz beep in `[k, k + 0.1)` of every second `k`.
fn flashes(path: &Path) {
    let video = format!(
        "color=c=black:s=320x180:r={FPS}:d={SECONDS},drawbox=x=0:y=0:w=320:h=180:color=white:t=fill:enable='lt(mod(t,1),0.1)'"
    );
    let audio = format!("aevalsrc='0.5*sin(2*PI*1000*t)*lt(mod(t,1),0.1)':s={RATE}:d={SECONDS}");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        &video,
        "-f",
        "lavfi",
        "-i",
        &audio,
        "-vf",
        "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p",
        "-colorspace",
        "bt709",
        "-c:v",
        "libx264",
        "-g",
        "30",
        "-c:a",
        "aac",
        "-shortest",
        path.to_str().unwrap(),
    ]);
}

/// Seconds of the first white frame and of the first loud sample at or after each of `expected`.
fn onsets(file: &Path, expected: &[f64]) -> Vec<(f64, f64)> {
    let file = file.to_str().unwrap();
    let luma = decoded(&[
        "-i",
        file,
        "-vf",
        "scale=4:4,format=gray",
        "-f",
        "rawvideo",
        "-",
    ]);
    let white: Vec<bool> = luma.chunks(16).map(|f| f[5] > 128).collect();
    let pcm = decoded(&["-i", file, "-ac", "1", "-ar", "48000", "-f", "f32le", "-"]);
    let loud: Vec<bool> = pcm
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b).abs() > 0.1)
        .collect();
    let first = |list: &[bool], per_second: f64, from: f64| {
        let start = ((from - 0.2) * per_second).max(0.0) as usize;
        let index = (start..list.len()).find(|&i| list[i]).unwrap();
        index as f64 / per_second
    };
    expected
        .iter()
        .map(|&t| {
            (
                first(&white, f64::from(FPS), t),
                first(&loud, RATE as f64, t),
            )
        })
        .collect()
}

#[test]
fn flashes_and_beeps_stay_together_after_ten_cuts_a_join_and_speed() {
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path();
    flashes(&cwd.join("flashes.mp4"));
    let registry = dc_ops::Registry::devcast().unwrap();
    for line in [
        "new flashes.mp4 -o cuts.devcast",
        "clip add flashes.mp4",
        // Source 2.2–3.8 of the second clip, which starts at output 12.
        "speed 14.2-15.8 2",
        "cut 0.4-0.7,1.4-1.7,2.4-2.7,3.4-3.7,4.4-4.7,5.4-5.7,6.4-6.7,7.4-7.7,8.4-8.7,9.4-9.7",
    ] {
        let out = run_line(line, cwd, &registry);
        assert_eq!(out.code, 0, "{line}: {}", out.stderr);
    }
    let dir = ProjectDir::open(cwd.join("cuts.devcast")).unwrap();
    let project = dir.load().unwrap();
    let map = TimeMap::build(&project);
    assert!((map.duration().as_secs_f64() - 20.2).abs() < 1e-6);
    let expected: Vec<f64> = project
        .clips
        .iter()
        .flat_map(|clip| (0..SECONDS).map(move |k| (clip.id, k)))
        .map(|(clip, k)| {
            let at = SourceTime::from_secs_f64(f64::from(k));
            map.to_output(clip, at).unwrap().as_secs_f64()
        })
        .collect();

    let out = cwd.join("готово.mp4");
    let spec = Spec {
        root: dir.root().to_path_buf(),
        project,
        settings: Settings {
            format: Format::Mp4,
            res: Res::Source,
            fps: FPS,
            quality: Quality::High,
            looped: false,
        },
        out: out.clone(),
    };
    let gpu = Arc::new(Gpu::new(AdapterChoice::Auto).unwrap());
    run(&gpu, &spec, &mut |_| ControlFlow::Continue(())).unwrap();

    let frame = 1.0 / f64::from(FPS);
    for (&t, (video, audio)) in expected.iter().zip(onsets(&out, &expected)) {
        println!("{t:.3} s: flash {video:.3}, beep {audio:.4}");
        // Frame n shows output moment n/fps: the flash starts on the first frame at or after t.
        assert!(
            video >= t - 1e-6 && video < t + frame,
            "flash at {t}: {video}"
        );
        // atempo keeps time to 15 ms of output at 2× (dc-audio).
        assert!((audio - t).abs() <= 0.015, "beep at {t}: {audio}");
    }
}
