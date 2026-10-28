use super::*;

/// The track starts at host time 10 s.
const START: i64 = 10_000_000;

/// A device at `rate` whose clock runs `drift` faster than the host. It delivers 10 ms
/// packets of silence with single clicks at the given host times (seconds after `START`).
struct Device {
    rate: f64,
    drift: f64,
    clicks: Vec<f64>,
}

impl Device {
    /// Host time (µs) of device frame `frame` when the device started at `first` s.
    fn time(&self, first: f64, frame: usize) -> i64 {
        START + ((first + frame as f64 / (self.rate * (1.0 + self.drift))) * 1e6).round() as i64
    }

    /// Pushes packets from `from` to `to` seconds (host time after `START`).
    fn play(&self, timeline: &mut AudioTimeline, from: f64, to: f64, out: &mut Vec<f32>) {
        let packet = (self.rate / 100.0) as usize;
        let mut frame = 0;
        loop {
            let time = self.time(from, frame);
            if time >= START + (to * 1e6) as i64 {
                return;
            }
            let mut samples = vec![0.0; packet * CHANNELS];
            for click in &self.clicks {
                let at = ((click - from) * self.rate * (1.0 + self.drift)).round() as i64;
                let index = at - frame as i64;
                if (0..packet as i64).contains(&index) {
                    samples[index as usize * CHANNELS] = 0.5;
                }
            }
            timeline
                .push(HostTime::from_micros(time), &samples, out)
                .unwrap();
            frame += packet;
        }
    }
}

/// Positions (frames) of the clicks in the left channel of `out`.
fn find_clicks(out: &[f32]) -> Vec<i64> {
    let left: Vec<f32> = out.iter().step_by(CHANNELS).copied().collect();
    (1..left.len() - 1)
        .filter(|&i| left[i] > 0.2 && left[i] >= left[i - 1] && left[i] > left[i + 1])
        .map(|i| i as i64)
        .collect()
}

/// Clicks land within 2 frames (42 µs) of their place: the resampler's filter delay is not a
/// whole number of frames, and a gap is filled in whole frames.
fn assert_clicks(out: &[f32], expected: &[i64]) {
    let found = find_clicks(out);
    assert_eq!(found.len(), expected.len(), "{found:?} vs {expected:?}");
    for (f, e) in found.iter().zip(expected) {
        assert!((f - e).abs() <= 2, "{found:?} vs {expected:?}");
    }
}

fn at(seconds: f64) -> i64 {
    (seconds * f64::from(RATE)).round() as i64
}

fn host(seconds: f64) -> HostTime {
    HostTime::from_micros(START + (seconds * 1e6) as i64)
}

#[test]
fn silence_is_filled_before_between_and_after_packets() {
    let device = Device {
        rate: 48_000.0,
        drift: 0.0,
        clicks: vec![0.5, 2.7],
    };
    let mut timeline = AudioTimeline::new(host(0.0), 48_000).unwrap();
    let mut out = Vec::new();
    // Loopback sends nothing while the system is silent.
    device.play(&mut timeline, 0.3, 1.0, &mut out);
    device.play(&mut timeline, 2.5, 3.0, &mut out);
    timeline.stop_at(host(4.0));
    timeline.finish(&mut out).unwrap();
    assert_eq!(out.len() as i64, at(4.0) * CHANNELS as i64);
    assert_eq!(timeline.frames(), at(4.0));
    assert_clicks(&out, &[at(0.5), at(2.7)]);
    let stats = timeline.stats();
    assert_eq!(stats.gaps, 3, "{stats:?}");
    assert!((stats.silence - 2.8).abs() < 0.001, "{stats:?}");
}

#[test]
fn sound_before_the_start_and_after_the_stop_is_cut() {
    let device = Device {
        rate: 48_000.0,
        drift: 0.0,
        clicks: vec![-0.2, 0.25, 1.9],
    };
    let mut timeline = AudioTimeline::new(host(0.0), 48_000).unwrap();
    let mut out = Vec::new();
    device.play(&mut timeline, -0.5, 0.9, &mut out);
    // Packets still queued at the stop may reach past it.
    timeline.stop_at(host(1.0));
    device.play(&mut timeline, 0.9, 2.0, &mut out);
    timeline.finish(&mut out).unwrap();
    assert_eq!(timeline.frames(), at(1.0));
    assert_clicks(&out, &[at(0.25)]);
    assert!((timeline.stats().cut - 0.5).abs() < 0.01);
}

#[test]
fn device_drift_is_resampled_to_the_host_clock() {
    // 500 ppm is ten times the drift measured in decision 008.
    let clicks: Vec<f64> = (1..30).map(|s| f64::from(s) * 2.0 + 0.123).collect();
    let device = Device {
        rate: 48_000.0,
        drift: 500e-6,
        clicks: clicks.clone(),
    };
    let mut timeline = AudioTimeline::new(host(0.0), 48_000).unwrap();
    let mut out = Vec::new();
    device.play(&mut timeline, 0.0, 60.0, &mut out);
    timeline.stop_at(host(60.0));
    timeline.finish(&mut out).unwrap();
    assert_eq!(timeline.frames(), at(60.0));
    let found = find_clicks(&out);
    assert_eq!(found.len(), clicks.len());
    // Without correction the last click would be 29 ms late.
    let late: Vec<i64> = found.iter().zip(&clicks).map(|(f, c)| f - at(*c)).collect();
    assert!(late.iter().all(|l| l.abs() <= 48), "{late:?}");
    assert!(late[12..].iter().all(|l| l.abs() <= 3), "{late:?}");
    let stats = timeline.stats();
    assert!((stats.correction_ppm + 500.0).abs() < 5.0, "{stats:?}");
    assert_eq!(stats.gaps, 0, "{stats:?}");
}

#[test]
fn a_new_device_at_another_rate_continues_on_the_clock() {
    let first = Device {
        rate: 48_000.0,
        drift: 0.0,
        clicks: vec![0.5],
    };
    let second = Device {
        rate: 44_100.0,
        drift: 0.0,
        clicks: vec![2.0],
    };
    let mut timeline = AudioTimeline::new(host(0.0), 48_000).unwrap();
    let mut out = Vec::new();
    first.play(&mut timeline, 0.0, 1.0, &mut out);
    timeline.restart(44_100).unwrap();
    second.play(&mut timeline, 1.2, 3.0, &mut out);
    timeline.stop_at(host(3.0));
    timeline.finish(&mut out).unwrap();
    assert_eq!(timeline.frames(), at(3.0));
    assert_clicks(&out, &[at(0.5), at(2.0)]);
}
