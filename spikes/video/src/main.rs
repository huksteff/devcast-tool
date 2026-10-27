//! Spike 1.3: FFmpeg (ffmpeg-next) decoding → NV12 → wgpu textures → egui.
//!
//! Modes:
//!   bench <file> [hw]   decode the whole file as fast as possible, print fps
//!   seek <file> [hw]    50 random frame-accurate seeks, print latency stats

mod decode;
mod player;

use std::time::Instant;

use decode::VideoDecoder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ffmpeg_next::init()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("");
    let file = args.get(1).cloned().unwrap_or_default();
    let hw = args.iter().any(|a| a == "hw");
    match mode {
        "bench" => bench(&file, hw),
        "seek" => seek(&file, hw),
        "play" => player::run(file, hw, 6.0),
        _ => Err("usage: spike-video bench|seek|play <file> [hw]".into()),
    }
}

fn bench(file: &str, hw: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut decoder = VideoDecoder::open(file, hw)?;
    let started = Instant::now();
    let mut frames = 0u32;
    let mut last = None;
    while let Some(frame) = decoder.next_frame()? {
        frames += 1;
        last = Some((frame.width, frame.height, frame.y.len(), frame.uv.len()));
    }
    let secs = started.elapsed().as_secs_f64();
    println!(
        "bench hw={hw} file_fps={:.0} frames={frames} time={secs:.2}s decode_fps={:.0} last={last:?}",
        decoder.fps,
        f64::from(frames) / secs
    );
    Ok(())
}

fn seek(file: &str, hw: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut decoder = VideoDecoder::open(file, hw)?;
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut times = Vec::new();
    let mut max_error = 0.0f64;
    for _ in 0..50 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let target = (state % 10_000) as f64 / 10_000.0 * (decoder.duration - 0.5);
        let started = Instant::now();
        let frame = decoder.seek(target)?.ok_or("no frame after seek")?;
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        // Frame expected on screen at `target`: the last frame starting at or before it.
        let expected = ((target + 0.5 / decoder.fps) * decoder.fps).floor() / decoder.fps;
        max_error = max_error.max((frame.pts - expected).abs() * decoder.fps);
    }
    times.sort_by(f64::total_cmp);
    let avg = times.iter().sum::<f64>() / times.len() as f64;
    println!(
        "seek hw={hw} n=50 avg={avg:.1}ms p50={:.1}ms p95={:.1}ms max={:.1}ms max_frame_error={max_error:.2} frames",
        times[25], times[47], times[49]
    );
    Ok(())
}
