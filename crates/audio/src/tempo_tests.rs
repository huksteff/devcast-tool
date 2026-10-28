use super::*;

/// Mono white noise as stereo, reproducible.
fn noise(frames: usize) -> Vec<f32> {
    let mut x: u32 = 12345;
    (0..frames)
        .flat_map(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let s = (x >> 8) as f32 / (1u32 << 24) as f32 - 0.5;
            [s, s]
        })
        .collect()
}

fn stretched(speed: f64, input: &[f32]) -> Vec<f32> {
    let mut tempo = Tempo::new(speed).unwrap();
    let mut out = Vec::new();
    for block in input.chunks(4096 * CHANNELS) {
        tempo.push(block, &mut out).unwrap();
    }
    out
}

/// Where 5 ms of `output` from frame `j` were taken from in `input`, relative to
/// `j · speed`, frames of input. `atempo` copies pieces of the input, so the best
/// correlation finds the piece.
fn source_offset(input: &[f32], output: &[f32], j: usize, speed: f64) -> i64 {
    let template: Vec<f32> = output[j * CHANNELS..(j + 240) * CHANNELS]
        .iter()
        .step_by(CHANNELS)
        .copied()
        .collect();
    let center = (j as f64 * speed) as i64;
    let score = |d: i64| -> f32 {
        let at = (center + d) as usize * CHANNELS;
        let piece = input[at..].iter().step_by(CHANNELS);
        template.iter().zip(piece).map(|(a, b)| a * b).sum()
    };
    let range = -(center.min(4800))..4800;
    range
        .max_by(|&a, &b| score(a).total_cmp(&score(b)))
        .unwrap()
}

#[test]
fn chain_keeps_each_step_in_range() {
    assert_eq!(chain(1.0), [1.0]);
    assert_eq!(chain(16.0), [2.0, 2.0, 2.0, 2.0]);
    assert_eq!(chain(0.25), [0.5, 0.5]);
    assert_eq!(chain(3.0), [2.0, 1.5]);
}

#[test]
fn pitch_is_kept() {
    let tone: Vec<f32> = (0..2 * RATE)
        .flat_map(|i| {
            let t = f64::from(i) / f64::from(RATE);
            let s = (0.5 * (2.0 * std::f64::consts::PI * 1000.0 * t).sin()) as f32;
            [s, s]
        })
        .collect();
    for speed in [0.25, 0.5, 2.0, 16.0] {
        let out = stretched(speed, &tone);
        let left: Vec<f32> = out.iter().step_by(CHANNELS).copied().collect();
        let part = &left[2000..left.len().min(40_000) - 2000];
        let crossings = (1..part.len())
            .filter(|&i| part[i - 1] < 0.0 && part[i] >= 0.0)
            .count();
        let hz = crossings as f64 * f64::from(RATE) / part.len() as f64;
        assert!((hz - 1000.0).abs() < 15.0, "speed {speed}: {hz} Hz");
    }
}

/// The piece played at output time `t` comes from source time `t · speed` give or take the
/// window of `atempo`. Measured 2026-10-28: when faster, ≤ 15 ms of output time; when slower,
/// ≤ 23 ms of source time, which is up to 90 ms of output at 0.25×. Printed with `--no-capture`.
#[test]
fn output_follows_the_time_map() {
    let input = noise(RATE as usize * 20);
    for speed in [0.25, 0.5, 0.8, 1.5, 2.0, 4.0, 16.0] {
        let out = stretched(speed, &input);
        let frames = out.len() / CHANNELS;
        let offsets: Vec<f64> = (1..=10)
            .map(|n| n * (frames - 300) / 11)
            .map(|j| source_offset(&input, &out, j, speed) as f64 / f64::from(RATE))
            .collect();
        let worst = offsets.iter().fold(0.0_f64, |m, d| m.max(d.abs()));
        let mean = offsets.iter().sum::<f64>() / offsets.len() as f64;
        println!(
            "speed {speed}: source offset mean {:.1} ms, worst {:.1} ms; output worst {:.1} ms",
            mean * 1000.0,
            worst * 1000.0,
            worst / speed * 1000.0
        );
        let off = if speed >= 1.0 { worst / speed } else { worst };
        assert!(off < 0.03, "speed {speed}: {worst} s of source");
    }
}
