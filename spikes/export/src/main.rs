//! Spike 1.6: export. GPU render → NV12 on GPU → readback ring → FFmpeg H.264 (nvenc/qsv/amf/mf/x264)
//! + AAC → MP4. Every encoder is self-tested first.
//!
//! Usage: spike-export <out_dir> [seconds]

mod encode;
mod nv12;

use std::time::Instant;

use ffmpeg_next as ffmpeg;

const ENCODERS: [&str; 5] = ["h264_nvenc", "h264_qsv", "h264_amf", "h264_mf", "libx264"];

fn main() -> encode::Result<()> {
    ffmpeg::init()?;
    ffmpeg::util::log::set_level(ffmpeg::util::log::Level::Fatal);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args.first().cloned().unwrap_or_else(|| ".".to_owned());
    let seconds: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10.0);

    let mut working = Vec::new();
    for name in ENCODERS {
        let started = Instant::now();
        match encode::self_test(name) {
            Ok(mean) => {
                println!("self-test {name}: OK (luma {mean:.1}, {:.0} ms)", started.elapsed().as_secs_f64() * 1000.0);
                working.push(name);
            }
            Err(e) => println!("self-test {name}: FAIL ({e})"),
        }
    }

    for (w, h, fps, label) in [(1920, 1080, 60, "1080p60"), (3840, 2160, 30, "4k30")] {
        let gpu = nv12::Converter::new(w, h)?;
        for name in &working {
            let path = format!("{out}/export-{label}-{name}.mp4");
            let frames = (seconds * f64::from(fps)) as usize;
            let started = Instant::now();
            let mut mp4 = encode::Mp4::create(&path, name, w, h, fps)?;
            let mut frame = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::NV12, w, h);
            for i in 0..frames + nv12::RING {
                if i < frames {
                    gpu.submit(i as f32 / fps as f32, i % nv12::RING);
                }
                if let Some(done) = i.checked_sub(nv12::RING - 1).filter(|d| *d < frames) {
                    gpu.read(done % nv12::RING, &mut frame)?;
                    frame.set_pts(Some(done as i64));
                    mp4.video_frame(&frame)?;
                    mp4.audio_until((done + 1) as f64 / f64::from(fps))?;
                }
            }
            mp4.finish()?;
            let secs = started.elapsed().as_secs_f64();
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            println!(
                "{label} {name}: {frames} frames in {secs:.2}s = {:.0} fps ({:.1}x realtime), {:.1} MB",
                frames as f64 / secs,
                seconds / secs,
                size as f64 / 1e6
            );
        }
    }
    Ok(())
}
