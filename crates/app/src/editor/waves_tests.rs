//! The peaks of the sound rows come off the UI thread: the rows are plain until they come, the
//! thread wakes the window, a source is asked for once; the real reading builds the cache file.

use std::path::Path;
use std::sync::mpsc::channel;

use dc_audio::file_track;
use dc_audio::peaks::Peaks;
use dc_project::ProjectDir;

use super::timeline::Source;
use super::waves::{Waves, load};

/// A mono 48 kHz WAV of `secs` seconds: a 440 Hz tone in bursts of 2 s whose height goes down
/// and up over 10 s, so cuts and speeds show in its wave.
pub(crate) fn tone(path: &Path, secs: f64) {
    let rate = 48_000u32;
    let frames = (secs * f64::from(rate)) as u32;
    let mut bytes = Vec::new();
    let mut put = |b: &[u8]| bytes.extend_from_slice(b);
    put(b"RIFF");
    put(&(36 + frames * 2).to_le_bytes());
    put(b"WAVEfmt ");
    put(&16u32.to_le_bytes());
    put(&1u16.to_le_bytes());
    put(&1u16.to_le_bytes());
    put(&rate.to_le_bytes());
    put(&(rate * 2).to_le_bytes());
    put(&2u16.to_le_bytes());
    put(&16u16.to_le_bytes());
    put(b"data");
    put(&(frames * 2).to_le_bytes());
    for n in 0..frames {
        let t = f64::from(n) / f64::from(rate);
        let burst = (std::f64::consts::PI * t / 2.0).sin().abs();
        let height = 0.2 + 0.7 * (std::f64::consts::PI * t / 10.0).cos().abs();
        let sample = (2.0 * std::f64::consts::PI * 440.0 * t).sin() * burst * height;
        put(&((sample * 32767.0) as i16).to_le_bytes());
    }
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).unwrap();
    }
    std::fs::write(path, bytes).unwrap();
}

/// Peaks of a [`tone`] of `secs` seconds.
pub(crate) fn tone_peaks(secs: f64) -> Peaks {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("tone.wav");
    tone(&path, secs);
    Peaks::build(&file_track(&path).unwrap()).unwrap()
}

fn music() -> Source {
    Source::File("assets/audio/музыка_фон.mp3".to_owned())
}

#[test]
fn rows_are_plain_until_the_peaks_come_and_the_window_wakes() {
    let peaks = tone_peaks(1.0);
    let (open, gate) = channel::<()>();
    let (woke, wakes) = channel::<()>();
    let (asked, asks) = channel::<Source>();
    let mut waves = Waves::default();
    waves.want([music()].iter());
    assert!(waves.get(&music()).is_none());
    let answer = peaks.clone();
    let load = move |source: &Source| {
        asked.send(source.clone()).unwrap();
        gate.recv().unwrap();
        Ok(answer.clone())
    };
    waves
        .start_with(load, move || woke.send(()).unwrap())
        .unwrap();
    waves.want([music(), music()].iter());
    assert_eq!(
        asks.recv().unwrap(),
        music(),
        "asked when the thread started"
    );
    assert!(!waves.receive() && waves.get(&music()).is_none());
    assert!(!waves.take_news());
    open.send(()).unwrap();
    wakes.recv().unwrap();
    assert!(waves.take_news(), "the main window wakes the editor");
    assert!(!waves.take_news(), "once");
    assert!(waves.receive());
    assert_eq!(waves.get(&music()), Some(&peaks));
    assert!(asks.try_recv().is_err(), "a source is read once");
}

#[test]
fn an_unreadable_source_stays_plain() {
    let (woke, wakes) = channel::<()>();
    let mut waves = Waves::default();
    let load = |_: &Source| Err("нет файла".to_owned());
    waves
        .start_with(load, move || woke.send(()).unwrap())
        .unwrap();
    waves.want([music()].iter());
    wakes.recv().unwrap();
    assert!(waves.receive());
    assert!(waves.get(&music()).is_none());
}

#[test]
fn the_real_reading_keeps_the_peaks_in_the_project_cache() {
    let temp = tempfile::tempdir().unwrap();
    let dir = ProjectDir::create(temp.path().join("Звук проекта.devcast")).unwrap();
    tone(&dir.root().join("assets/audio/тон.wav"), 3.0);
    let source = Source::File("assets/audio/тон.wav".to_owned());
    let peaks = load(&dir, &source).unwrap();
    assert_eq!(peaks.frames(), 3 * 48_000);
    let cached = std::fs::read_dir(dir.root().join("cache")).unwrap().count();
    assert!(cached > 0, "the cache file is written");
    let again = load(&dir, &source).unwrap();
    let (built, read) = (peaks.range(0, 144_000, 30), again.range(0, 144_000, 30));
    for (a, b) in built.iter().zip(&read) {
        assert!((a.max - b.max).abs() < 1e-4 && (a.min - b.min).abs() < 1e-4);
    }
    let missing = Source::File("assets/audio/нет.wav".to_owned());
    assert!(load(&dir, &missing).is_err());
}
