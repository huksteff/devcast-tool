//! Spike 1.7: Windows Graphics Capture via windows-capture 2.
//! Monitor / window / area (crop), cursor and yellow border off, timestamps (QPC-based
//! SystemRelativeTime), variable frame rate statistics, PNG snapshots.
//!
//! Usage:
//!   spike-capture monitor <index> <seconds> <out_dir>
//!   spike-capture window <title> <seconds> <out_dir>
//!   spike-capture area <index> <x> <y> <w> <h> <seconds> <out_dir>

use std::time::Instant;

use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::encoder::ImageFormat;
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
    SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone)]
struct Job {
    seconds: f64,
    out: String,
    crop: Option<[u32; 4]>,
}

struct Capture {
    job: Job,
    started: Instant,
    stamps: Vec<i64>,
    latency_ms: Vec<f64>,
    qpc_freq: i64,
    saved: u32,
    crop_ms: f64,
}

fn qpc() -> i64 {
    let mut v = 0i64;
    // SAFETY: writes the current counter into `v`.
    let _ = unsafe { QueryPerformanceCounter(&mut v) };
    v
}

impl GraphicsCaptureApiHandler for Capture {
    type Flags = Job;
    type Error = Error;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let mut freq = 0i64;
        // SAFETY: writes the counter frequency into `freq`.
        unsafe { QueryPerformanceFrequency(&mut freq)? };
        Ok(Self {
            job: ctx.flags,
            started: Instant::now(),
            stamps: Vec::new(),
            latency_ms: Vec::new(),
            qpc_freq: freq,
            saved: 0,
            crop_ms: 0.0,
        })
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, control: InternalCaptureControl) -> Result<(), Self::Error> {
        // SystemRelativeTime is in 100 ns units on the QPC timeline.
        let stamp = frame.timestamp()?.Duration;
        let now_100ns = (qpc() as i128 * 10_000_000 / i128::from(self.qpc_freq)) as i64;
        self.latency_ms.push((now_100ns - stamp) as f64 / 10_000.0);
        self.stamps.push(stamp);
        let n = self.stamps.len();
        if n == 30 || (self.saved == 1 && self.started.elapsed().as_secs_f64() > self.job.seconds - 0.5) {
            let path = format!("{}/capture-{}.png", self.job.out, self.saved);
            if let Some([x, y, w, h]) = self.job.crop {
                let t = Instant::now();
                let mut buffer = frame.buffer_crop(x, y, x + w, y + h)?;
                self.crop_ms = t.elapsed().as_secs_f64() * 1000.0;
                buffer.save_as_image(&path, ImageFormat::Png)?;
            } else {
                frame.save_as_image(&path, ImageFormat::Png)?;
            }
            self.saved += 1;
        }
        if self.started.elapsed().as_secs_f64() > self.job.seconds {
            control.stop();
        }
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let n = self.stamps.len();
        if n < 2 {
            println!("frames: {n}");
            return;
        }
        let intervals: Vec<f64> = self.stamps.windows(2).map(|w| (w[1] - w[0]) as f64 / 10_000.0).collect();
        let monotonic = intervals.iter().all(|d| *d > 0.0);
        let span = (self.stamps[n - 1] - self.stamps[0]) as f64 / 10_000_000.0;
        let mut sorted = intervals.clone();
        sorted.sort_by(f64::total_cmp);
        let mut lat = self.latency_ms.clone();
        lat.sort_by(f64::total_cmp);
        println!(
            "frames={n} span={span:.2}s avg_fps={:.1} interval min/p50/max={:.1}/{:.1}/{:.1} ms monotonic={monotonic} \
             latency p50/p95={:.1}/{:.1} ms crop={:.2} ms",
            (n - 1) as f64 / span,
            sorted[0],
            sorted[sorted.len() / 2],
            sorted[sorted.len() - 1],
            lat[lat.len() / 2],
            lat[lat.len() * 95 / 100],
            self.crop_ms,
        );
    }
}

fn settings<T: TryInto<windows_capture::settings::GraphicsCaptureItemType>>(item: T, job: Job) -> Settings<Job, T> {
    Settings::new(
        item,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        min_interval(),
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        job,
    )
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    for (i, m) in Monitor::enumerate()?.iter().enumerate() {
        println!("monitor {i}: {} {}x{}", m.device_name()?, m.width()?, m.height()?);
    }
    match arg(0).as_str() {
        "monitor" => {
            let job = Job { seconds: arg(2).parse()?, out: arg(3), crop: None };
            Capture::start(settings(Monitor::from_index(arg(1).parse()?)?, job))?;
        }
        "window" => {
            let job = Job { seconds: arg(2).parse()?, out: arg(3), crop: None };
            Capture::start(settings(Window::from_name(&arg(1))?, job))?;
        }
        "area" => {
            let crop = [arg(2).parse()?, arg(3).parse()?, arg(4).parse()?, arg(5).parse()?];
            let job = Job { seconds: arg(6).parse()?, out: arg(7), crop: Some(crop) };
            Capture::start(settings(Monitor::from_index(arg(1).parse()?)?, job))?;
        }
        "dxgi" => dxgi(arg(1).parse()?, arg(2).parse()?)?,
        _ => return Err("usage: monitor|window|area|dxgi ...".into()),
    }
    Ok(())
}

/// SPIKE_MIN_INTERVAL_MS=N sets a custom minimum update interval; unset keeps the system default.
fn min_interval() -> MinimumUpdateIntervalSettings {
    match std::env::var("SPIKE_MIN_INTERVAL_MS").ok().and_then(|v| v.parse().ok()) {
        Some(ms) => MinimumUpdateIntervalSettings::Custom(std::time::Duration::from_millis(ms)),
        None => MinimumUpdateIntervalSettings::Default,
    }
}

/// DXGI Desktop Duplication on the same monitor, for comparison: counts frames with new content.
pub fn dxgi(index: usize, seconds: f64) -> Result<(), Error> {
    use windows_capture::dxgi_duplication_api::{DxgiDuplicationApi, DxgiDuplicationFormat};
    let mut dup = DxgiDuplicationApi::new_options(Monitor::from_index(index)?, &[DxgiDuplicationFormat::Bgra8])?;
    let started = Instant::now();
    let (mut frames, mut timeouts) = (0u32, 0u32);
    while started.elapsed().as_secs_f64() < seconds {
        match dup.acquire_next_frame(50) {
            Ok(_) => frames += 1,
            Err(_) => timeouts += 1,
        }
    }
    println!("dxgi: frames={frames} timeouts={timeouts} over {seconds}s ({:.1} fps)", f64::from(frames) / seconds);
    Ok(())
}
