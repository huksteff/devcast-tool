//! Spike 1.8: system audio (WASAPI loopback via cpal) + microphone at the same time.
//! Plays an inaudible −60 dBFS tone so loopback has content, detects gaps from capture
//! timestamps (QPC) and fills them with silence, measures clock drift and the offset between
//! tracks, saves WAVs, and runs nnnoiseless on the microphone.
//!
//! Usage: spike-audio <seconds> <out_dir> [--no-wav]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

type Error = Box<dyn std::error::Error>;

/// Gaps longer than this between consecutive packets are filled with silence.
const GAP_MS: f64 = 3.0;

#[derive(Default)]
struct Track {
    rate: u32,
    channels: u16,
    samples: Vec<f32>,
    keep: bool,
    frames: u64,
    first: Option<u128>,
    expected_next: Option<u128>,
    gap_count: u32,
    gap_ms: f64,
    callbacks: u64,
    peak: f32,
}

impl Track {
    fn push(&mut self, data: &[f32], capture_ns: u128) {
        let channels = usize::from(self.channels);
        let frames = (data.len() / channels) as u64;
        if self.first.is_none() {
            self.first = Some(capture_ns);
        }
        if let Some(expected) = self.expected_next {
            let late_ms = (capture_ns as f64 - expected as f64) / 1e6;
            if late_ms > GAP_MS {
                // Loopback sends nothing while the system is silent: fill the hole by timestamp.
                let missing = (late_ms / 1000.0 * f64::from(self.rate)).round() as u64;
                self.gap_count += 1;
                self.gap_ms += late_ms;
                self.frames += missing;
                if self.keep {
                    self.samples.extend(std::iter::repeat_n(0.0, missing as usize * channels));
                }
            }
        }
        self.frames += frames;
        self.callbacks += 1;
        self.peak = data.iter().fold(self.peak, |p, s| p.max(s.abs()));
        if self.keep {
            self.samples.extend_from_slice(data);
        }
        let duration_ns = frames as u128 * 1_000_000_000 / u128::from(self.rate);
        self.expected_next = Some(capture_ns + duration_ns);
    }

    fn report(&self, name: &str) {
        let (Some(first), Some(end)) = (self.first, self.expected_next) else {
            println!("{name}: no data");
            return;
        };
        let timeline = (end - first) as f64 / 1e9;
        let by_samples = self.frames as f64 / f64::from(self.rate);
        println!(
            "{name}: {}Hz x{} callbacks={} timeline={timeline:.3}s samples={by_samples:.3}s drift={:+.2}ms gaps={} ({:.0} ms) peak={:.1} dBFS",
            self.rate,
            self.channels,
            self.callbacks,
            (by_samples - timeline) * 1000.0,
            self.gap_count,
            self.gap_ms,
            20.0 * self.peak.max(1e-9).log10(),
        );
    }
}

fn input_stream(device: &cpal::Device, track: Arc<Mutex<Track>>, loopback: bool) -> Result<cpal::Stream, Error> {
    let supported = if loopback { device.default_output_config()? } else { device.default_input_config()? };
    let config = supported.config();
    if let Ok(mut t) = track.lock() {
        t.rate = config.sample_rate;
        t.channels = config.channels;
    }
    let stream = device.build_input_stream(
        config,
        move |data: &[f32], info: &cpal::InputCallbackInfo| {
            if let Ok(mut t) = track.lock() {
                t.push(data, info.timestamp().capture.as_nanos());
            }
        },
        |e| eprintln!("stream error: {e}"),
        None,
    )?;
    stream.play()?;
    Ok(stream)
}

fn tone(device: &cpal::Device) -> Result<cpal::Stream, Error> {
    let config = device.default_output_config()?.config();
    let (rate, channels) = (f64::from(config.sample_rate), usize::from(config.channels));
    let mut n = 0u64;
    let stream = device.build_output_stream(
        config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            for frame in data.chunks_mut(channels) {
                let s = (0.001 * (std::f64::consts::TAU * 440.0 * n as f64 / rate).sin()) as f32;
                frame.fill(s);
                n += 1;
            }
        },
        |e| eprintln!("tone error: {e}"),
        None,
    )?;
    stream.play()?;
    Ok(stream)
}

fn save_wav(path: &str, t: &Track) -> Result<(), Error> {
    let spec = hound::WavSpec { channels: t.channels, sample_rate: t.rate, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
    let mut w = hound::WavWriter::create(path, spec)?;
    for s in &t.samples {
        w.write_sample(*s)?;
    }
    w.finalize()?;
    Ok(())
}

/// Mono mix → nnnoiseless (48 kHz, 480-sample frames, i16-scaled floats). Returns RMS before/after in dBFS.
fn denoise(t: &Track, path: &str) -> Result<(f64, f64), Error> {
    let ch = usize::from(t.channels);
    let mono: Vec<f32> = t.samples.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32 * 32767.0).collect();
    let mut state = nnnoiseless::DenoiseState::new();
    let mut out = Vec::with_capacity(mono.len());
    let mut buf = [0.0f32; nnnoiseless::DenoiseState::FRAME_SIZE];
    for chunk in mono.chunks_exact(nnnoiseless::DenoiseState::FRAME_SIZE) {
        state.process_frame(&mut buf, chunk);
        out.extend_from_slice(&buf);
    }
    let rms = |v: &[f32]| 20.0 * ((v.iter().map(|s| f64::from(*s / 32767.0).powi(2)).sum::<f64>() / v.len().max(1) as f64).sqrt().max(1e-9)).log10();
    let skip = 48_000.min(out.len()); // first second: the model is still adapting
    let before = rms(&mono[skip..out.len()]);
    let after = rms(&out[skip..]);
    let spec = hound::WavSpec { channels: 1, sample_rate: t.rate, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
    let mut w = hound::WavWriter::create(path, spec)?;
    for s in &out {
        w.write_sample(*s / 32767.0)?;
    }
    w.finalize()?;
    Ok((before, after))
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seconds: f64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(10.0);
    let out = args.get(1).cloned().unwrap_or_else(|| ".".to_owned());
    let keep = !args.iter().any(|a| a == "--no-wav");
    let host = cpal::default_host();
    let speaker = host.default_output_device().ok_or("no output device")?;
    let mic = host.default_input_device().ok_or("no input device")?;
    println!("output: {}", speaker.description().map(|d| d.name().to_owned()).unwrap_or_default());
    println!("input:  {}", mic.description().map(|d| d.name().to_owned()).unwrap_or_default());

    let system = Arc::new(Mutex::new(Track { keep, ..Default::default() }));
    let micro = Arc::new(Mutex::new(Track { keep, ..Default::default() }));
    let _loop = input_stream(&speaker, Arc::clone(&system), true)?;
    let _mic = input_stream(&mic, Arc::clone(&micro), false)?;
    // Tone starts after 2 s and stops at 2/3 of the run, so loopback sees sound and silence.
    let started = Instant::now();
    std::thread::sleep(Duration::from_secs(2));
    let tone_stream = tone(&speaker)?;
    std::thread::sleep(Duration::from_secs_f64((seconds * 2.0 / 3.0 - 2.0).max(0.5)));
    drop(tone_stream);
    std::thread::sleep(Duration::from_secs_f64((seconds - started.elapsed().as_secs_f64()).max(0.5)));

    let (s, m) = (system.lock().map_err(|_| "lock")?, micro.lock().map_err(|_| "lock")?);
    s.report("system");
    m.report("mic   ");
    if let (Some(a), Some(b)) = (s.first, m.first) {
        println!("start offset system−mic: {:+.2} ms (both on the QPC timeline)", (a as f64 - b as f64) / 1e6);
    }
    if keep {
        save_wav(&format!("{out}/system.wav"), &s)?;
        save_wav(&format!("{out}/mic.wav"), &m)?;
        if m.rate == 48_000 {
            let (before, after) = denoise(&m, &format!("{out}/mic-denoised.wav"))?;
            println!("denoise: mic RMS {before:.1} dBFS → {after:.1} dBFS");
        } else {
            println!("denoise skipped: mic is {} Hz (nnnoiseless needs 48 kHz)", m.rate);
        }
    }
    Ok(())
}
