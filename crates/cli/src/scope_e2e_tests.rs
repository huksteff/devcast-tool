//! Task 5.39: the SCOPE scenario «вырежи паузы, поставь зумы на клики, сделай субтитры,
//! экспортируй» as an agent runs it by SKILL.md: pauses cut, autozooms on clicks, an annotation,
//! captions, a look at the storyboard and the export. The session is one snapshot; the MP4 is
//! checked for duration, sound and what its frames show.
//!
//! No recording has both clicks and speech, so the test makes one as `devcast record` would with
//! a microphone: the clicks recording gets a `mic` track (`mic.mp4` in the manifest, same start as
//! the other tracks) with the synthesized speech, and its recognized words go into the cache.

use std::path::{Path, PathBuf};

use dc_capture::Manifest;
use dc_core::{ClipId, OutputTime, SourceTime, TimeMap};
use dc_project::ProjectDir;

use crate::run::{Local, Outcome, run_with};
use crate::scope_e2e_frames::{Frame, Region};
use crate::test_util::{decoded, defaults_file, ffmpeg, fixture_copy, info_duration, seconds};
use crate::transcript_tests::seed_words;

/// Left of a pause on each side, so speech does not run together (SKILL.md).
const MARGIN: f64 = 0.15;
const RATE: usize = 48_000;

/// A copy of `clicks.dcproj` whose recording has a `mic` track with the synthesized speech.
struct Lesson {
    tmp: tempfile::TempDir,
    root: PathBuf,
    channel: String,
}

impl Lesson {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Урок 5.dcproj");
        fixture_copy("clicks.dcproj", &root);
        let folder = root.join("media").join("recording-1");
        let mut manifest = Manifest::read(&folder).unwrap();
        let system = manifest.tracks.iter().find(|t| t.name == "system").unwrap();
        let mut mic = system.clone();
        mic.name = "mic".to_owned();
        mic.file = Some("mic.mp4".to_owned());
        let secs = (mic.length as f64 / f64::from(mic.rate)).to_string();
        let speech = speech_file();
        let out = folder.join("mic.mp4");
        ffmpeg(&[
            "-i",
            speech.to_str().unwrap(),
            "-af",
            "apad",
            "-t",
            &secs,
            "-ac",
            "1",
            "-ar",
            "48000",
            "-c:a",
            "flac",
            out.to_str().unwrap(),
        ]);
        manifest.tracks.push(mic);
        manifest.write(&folder).unwrap();
        let dir = ProjectDir::open(&root).unwrap();
        let media = dir.load().unwrap().clips[0].media.clone();
        let tracks = dc_audio::tracks(&dir, &media).unwrap();
        seed_words(&dir, tracks.iter().find(|t| t.name == "mic").unwrap());
        let channel = format!("test-export-scope-{}", std::process::id());
        Self { tmp, root, channel }
    }

    fn run(&self, line: &str) -> Outcome {
        let defaults = defaults_file(self.tmp.path());
        let local = Local {
            defaults: Some(&defaults),
            export_channel: Some(&self.channel),
        };
        run_with(
            words(line),
            &self.root,
            &dc_ops::Registry::devcast().unwrap(),
            local,
        )
    }
}

fn speech_file() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio/tts-ru.flac")
}

/// The words of a command line as a shell passes them: `"…"` keeps spaces and loses the quotes.
fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    for (n, part) in line.split('"').enumerate() {
        if n % 2 == 1 {
            words.push(part.to_owned());
        } else {
            words.extend(part.split_whitespace().map(str::to_owned));
        }
    }
    words
}

/// The pauses `silences` printed that have speech on both sides, with [`MARGIN`] left.
fn inner_pauses(silences: &str, duration: f64) -> String {
    let ranges: Vec<String> = silences
        .lines()
        .filter_map(|line| line.split_whitespace().next()?.split_once('–'))
        .map(|(from, to)| (seconds(from), seconds(to)))
        .filter(|&(from, to)| from > 0.0 && to < duration - 0.05)
        .map(|(from, to)| format!("{:.3}-{:.3}", from + MARGIN, to - MARGIN))
        .collect();
    ranges.join(",")
}

/// The output without what changes from run to run or machine to machine: the temporary folder,
/// the encoded size, the encoder (a video card one when there is one) and the time it took.
fn steady(text: &str, root: &Path) -> String {
    let text = text.replace(&root.display().to_string(), "<проект>");
    text.lines()
        .map(|line| match line.find(" с, ") {
            Some(size) if line.contains("готово:") => {
                format!("{} с, <размер, кодек, время>", &line[..size])
            }
            _ => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn pauses_zooms_annotation_captions_and_export_in_few_calls() {
    let lesson = Lesson::new();
    let mut log = String::new();
    let mut calls = 0;
    let mut call = |line: &str| -> String {
        calls += 1;
        let out = lesson.run(line);
        assert_eq!(out.code, 0, "{line}: {}", out.stderr);
        assert_eq!(out.stderr, "", "{line}");
        let shown = steady(out.stdout.trim_end(), &lesson.root);
        log.push_str(&format!("$ devcast {line}\n{shown}\n\n"));
        out.stdout.trim_end().to_owned()
    };
    let before = info_duration(&call("info"));
    let pauses = inner_pauses(&call("silences"), before);
    call(&format!("cut {pauses} --dry-run"));
    call(&format!("cut {pauses}"));
    call("zoom add --at clicks --dry-run");
    call("zoom add --at clicks");
    call("annot add text --at 0 --duration 1.5 --rect 0.05,0.05,0.5,0.12");
    call(r##"text set a1 --text "Запись экрана" --size 48 --background "#000000b0""##);
    call("captions gen");
    call("ls captions");
    call("sheet -n 6 -o раскадровка.png");
    call("export -o урок.mp4 --res 720p --fps 30");
    let after = info_duration(&call("info"));
    insta::assert_snapshot!(log);
    assert!((10..=15).contains(&calls), "{calls} calls");
    assert!(lesson.root.join("раскадровка.png").is_file());

    let mp4 = lesson.root.join("урок.mp4");
    let project = ProjectDir::open(&lesson.root).unwrap().load().unwrap();
    let map = TimeMap::build(&project);
    let clip = project.clips[0].id;
    check_duration(&mp4, after, before);
    check_sound(&mp4, &map, clip);
    check_frames(&mp4, &project.clips[0].media.path, &lesson.root, &map);
}

/// The file is as long as `info` says after the cuts, and the cuts took 1–2 s of pauses.
fn check_duration(mp4: &Path, after: f64, before: f64) {
    let probe = dc_media::probe(mp4).unwrap();
    assert_eq!(
        probe.streams.len(),
        2,
        "video and sound: {:?}",
        probe.streams
    );
    let file = probe.duration.as_secs_f64();
    assert!(
        (file - after).abs() <= 1.0 / 30.0 + 1e-3,
        "{file} vs {after}"
    );
    assert!((1.0..2.0).contains(&(before - after)), "{before} → {after}");
}

/// Mean power of `samples` in dBFS.
fn rms_db(samples: &[f32]) -> f64 {
    let power = samples.iter().map(|&s| f64::from(s).powi(2)).sum::<f64>() / samples.len() as f64;
    10.0 * power.log10()
}

/// The first channel of `file` (FFmpeg's own downmix to mono adds 3 dB to equal channels).
fn mono(file: &Path) -> Vec<f32> {
    let file = file.to_str().unwrap();
    let first = "pan=mono|c0=c0";
    let pcm = decoded(&["-i", file, "-af", first, "-ar", "48000", "-f", "f32le", "-"]);
    pcm.as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect()
}

fn span(samples: &[f32], from: f64, to: f64) -> &[f32] {
    &samples[(from * RATE as f64) as usize..(to * RATE as f64) as usize]
}

/// The last sentence plays at the level of the speech file (the mic at 100 %, nothing on the
/// system track), and after it the export is silent.
fn check_sound(mp4: &Path, map: &TimeMap, clip: ClipId) {
    let out = |secs: f64| {
        let at = map
            .to_output(clip, SourceTime::from_secs_f64(secs))
            .unwrap();
        at.as_secs_f64()
    };
    let (from, to) = (7.44, 9.40);
    let speech = rms_db(span(&mono(&speech_file()), from, to));
    let export = mono(mp4);
    let played = rms_db(span(&export, out(from), out(to)));
    println!("speech {speech:.2} dB, export {played:.2} dB");
    assert!((played - speech).abs() <= 1.0, "{speech} vs {played}");
    let tail = rms_db(span(&export, out(9.8), out(12.3)));
    assert!(tail < -60.0, "after the speech: {tail} dB");
}

/// Three moments of the export against the recording at the same source moment.
fn check_frames(mp4: &Path, media: &str, root: &Path, map: &TimeMap) {
    let recording = root.join(media);
    let pair = |secs: f64| {
        let at = OutputTime::from_secs_f64(secs);
        let (_, source) = map.to_source(at).unwrap();
        let shown = Frame::at(mp4, secs);
        let original = Frame::at(&recording, source.as_secs_f64());
        (shown, original)
    };
    let annotation = Region::new(0.05, 0.05, 0.5, 0.12);
    let captions = Region::new(0.0, 0.75, 1.0, 0.2);
    let middle = Region::new(0.0, 0.2, 1.0, 0.5);

    // 0.5 s: before the zoom; the title and «Привет!» are on.
    let (shown, original) = pair(0.5);
    assert!(shown.edge_dark() > 0.9, "the window frame is whole");
    assert!(shown.changed(&original, &annotation) > 5000, "the title");
    assert!(shown.changed(&original, &captions) > 2500, "the caption");
    assert!(shown.changed(&original, &middle) < 500, "nothing else");

    // 5 s: inside the zoom on the clicks (1.5×: the frame line is out of the picture), captions on.
    let (shown, original) = pair(5.0);
    assert!(
        shown.edge_dark() < 0.1,
        "the zoom cuts the window frame away"
    );
    assert!(shown.changed(&original, &captions) > 2500, "the caption");

    // 10.5 s: zoomed out again, after the speech and the title.
    let (shown, original) = pair(10.5);
    assert!(shown.edge_dark() > 0.9, "the window frame is whole");
    assert!(shown.changed(&original, &annotation) < 200, "no title");
    assert!(shown.changed(&original, &captions) < 1000, "no caption");
    assert!(shown.changed(&original, &middle) < 1500, "only the cursor");
}
