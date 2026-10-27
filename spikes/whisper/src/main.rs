//! Spike 1.10: whisper-rs (whisper.cpp) on CPU: Russian quality (WER against the known text),
//! speed, word timestamps (token t0/t1 and DTW).
//!
//! Usage: spike-whisper <model.bin> <small|turbo> <speech.wav 16k mono> <reference.txt> [lang]

use std::time::Instant;

use whisper_rs::{
    DtwMode, DtwModelPreset, DtwParameters, FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
};

type Error = Box<dyn std::error::Error>;

struct Word {
    text: String,
    t0: f64,
    t1: f64,
    dtw: f64,
}

fn normalize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .replace('ё', "е")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Word error rate: word-level Levenshtein distance / reference length.
fn wer(reference: &[String], hypothesis: &[String]) -> f64 {
    let mut prev: Vec<usize> = (0..=hypothesis.len()).collect();
    for (i, r) in reference.iter().enumerate() {
        let mut row = vec![i + 1; hypothesis.len() + 1];
        for (j, h) in hypothesis.iter().enumerate() {
            let cost = usize::from(r != h);
            row[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(row[j] + 1);
        }
        prev = row;
    }
    prev[hypothesis.len()] as f64 / reference.len().max(1) as f64
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    let (model, preset, wav, reference) = (arg(0), arg(1), arg(2), arg(3));
    let lang = args.get(4).cloned().unwrap_or_else(|| "ru".to_owned());

    let samples: Vec<f32> = hound::WavReader::open(&wav)?
        .into_samples::<i16>()
        .map(|s| s.map(|v| f32::from(v) / 32768.0))
        .collect::<Result<_, _>>()?;
    let audio_secs = samples.len() as f64 / 16_000.0;

    let mut cparams = WhisperContextParameters::default();
    let preset = if preset == "turbo" { DtwModelPreset::LargeV3Turbo } else { DtwModelPreset::Small };
    cparams.dtw_parameters(DtwParameters { mode: DtwMode::ModelPreset { model_preset: preset }, ..Default::default() });
    let started = Instant::now();
    let ctx = WhisperContext::new_with_params(&model, cparams)?;
    let load = started.elapsed().as_secs_f64();
    let mut state = ctx.create_state()?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    let threads = std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4);
    params.set_n_threads(threads);
    params.set_language(if lang == "auto" { None } else { Some(&lang) });
    params.set_token_timestamps(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    let started = Instant::now();
    state.full(params, &samples)?;
    let secs = started.elapsed().as_secs_f64();

    let mut text = String::new();
    let mut words: Vec<Word> = Vec::new();
    for segment in state.as_iter() {
        text.push_str(&segment.to_str_lossy()?);
        for i in 0..segment.n_tokens() {
            let Some(token) = segment.get_token(i) else { continue };
            let piece = token.to_str_lossy()?.into_owned();
            if piece.starts_with("[_") || piece.starts_with("<|") {
                continue;
            }
            let data = token.token_data();
            let (t0, t1, dtw) = (data.t0 as f64 / 100.0, data.t1 as f64 / 100.0, data.t_dtw as f64 / 100.0);
            match words.last_mut() {
                Some(w) if !piece.starts_with(' ') && piece.chars().next().is_some_and(char::is_alphanumeric) => {
                    w.text.push_str(&piece);
                    w.t1 = t1;
                }
                _ => words.push(Word { text: piece.trim().to_owned(), t0, t1, dtw }),
            }
        }
    }
    let reference = std::fs::read_to_string(reference)?;
    let error = wer(&normalize(&reference), &normalize(&text));
    println!("model={model} threads={threads} load={load:.1}s audio={audio_secs:.1}s transcribe={secs:.1}s (x{:.1} realtime) WER={:.1}%", audio_secs / secs, error * 100.0);
    println!("text: {}", text.trim());
    println!("first words (text t0..t1 dtw):");
    for w in words.iter().filter(|w| !w.text.is_empty()).take(14) {
        println!("  {:<14} {:>6.2}..{:<6.2} dtw {:>6.2}", w.text, w.t0, w.t1, w.dtw);
    }
    Ok(())
}
