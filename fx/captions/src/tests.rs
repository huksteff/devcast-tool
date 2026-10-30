use dc_core::project::{Media, MediaKind, SpeedRange};
use dc_core::time::Source;
use dc_core::{Clip, Command, History, Project, Range, SourceTime, TimeDelta};

use crate::{
    CaptionsClear, CaptionsEffect, CaptionsGen, CaptionsImport, CaptionsMerge, CaptionsRm,
    CaptionsSet, CaptionsSplit, Cue, NewCue, Word, export_text, merge_tracks, parse_text,
};

pub(crate) fn secs(s: f64) -> SourceTime {
    SourceTime::from_secs_f64(s)
}

pub(crate) fn span(a: f64, b: f64) -> Range<Source> {
    Range::new(secs(a), secs(b)).unwrap()
}

pub(crate) fn clip(project: &mut Project, length: f64) -> &mut Clip {
    let id = project.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(length);
    project.clips.push(Clip {
        id,
        media: Media {
            path: format!("media/{id}.mp4"),
            kind: MediaKind::Recording,
            duration,
            size: dc_core::PixelSize {
                width: 1920,
                height: 1080,
            },
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: dc_core::Rect::FULL,
        audio: Default::default(),
    });
    project.clips.last_mut().unwrap()
}

/// A cue of `media` with one word per second-pair `(text, start, end)`, padded by 0.1 s.
pub(crate) fn new_cue(media: &str, words: &[(&str, f64, f64)]) -> NewCue {
    let words: Vec<Word> = words
        .iter()
        .map(|(text, a, b)| Word {
            text: (*text).to_owned(),
            range: span(*a, *b),
        })
        .collect();
    let start = words[0].range.start().as_secs_f64();
    let end = words[words.len() - 1].range.end().as_secs_f64();
    NewCue {
        media: media.to_owned(),
        range: span((start - 0.1).max(0.0), end + 0.1),
        words,
    }
}

/// Two recordings of 10 s; `s1` «Привет, это видео.» and `s2` «Пока!» in the first, `s3` in
/// the second.
pub(crate) fn project() -> Project {
    let mut p = Project::default();
    clip(&mut p, 10.0);
    clip(&mut p, 10.0);
    let first = &p.clips[0].media.path.clone();
    let second = &p.clips[1].media.path.clone();
    let cues = vec![
        new_cue(
            first,
            &[
                ("Привет,", 1.0, 1.5),
                ("это", 2.0, 2.4),
                ("видео.", 2.4, 3.0),
            ],
        ),
        new_cue(first, &[("Пока!", 5.0, 5.5)]),
        new_cue(second, &[("Вторая", 1.0, 1.5), ("запись.", 1.5, 2.0)]),
    ];
    run(&mut p, &CaptionsGen { cues });
    p
}

fn op<T: serde::de::DeserializeOwned>(args: serde_json::Value) -> T {
    serde_json::from_value(args).unwrap()
}

fn cues(p: &Project) -> Vec<Cue> {
    p.state::<CaptionsEffect>().unwrap().cues
}

fn texts(p: &Project) -> Vec<String> {
    cues(p)
        .iter()
        .map(|c| format!("{} {}", c.id, c.text()))
        .collect()
}

pub(crate) fn run(p: &mut Project, command: &dyn Command) -> String {
    command.apply(p).unwrap().to_string()
}

fn fail(p: &mut Project, command: &dyn Command) -> String {
    let before = p.clone();
    let error = command.apply(p).unwrap_err().to_string();
    assert_eq!(*p, before, "failed command changed the project");
    error
}

#[test]
fn gen_replaces_all_cues_with_new_ids() {
    let mut p = project();
    assert_eq!(
        texts(&p),
        ["s1 Привет, это видео.", "s2 Пока!", "s3 Вторая запись."]
    );
    let media = p.clips[0].media.path.clone();
    let again = CaptionsGen {
        cues: vec![new_cue(&media, &[("Снова", 0.5, 1.0)])],
    };
    assert_eq!(run(&mut p, &again), "реплики -3 s1..s3; реплики +1 s4");
    assert_eq!(texts(&p), ["s4 Снова"]);
}

#[test]
fn set_text_and_timing_in_output_time() {
    let mut p = project();
    p.clips[0].cuts.push(span(0.0, 0.5));
    let set: CaptionsSet = op(serde_json::json!({"id": "s1", "text": "Привет, это наше видео."}));
    assert_eq!(run(&mut p, &set), "реплики ~1 s1");
    assert_eq!(texts(&p)[0], "s1 Привет, это наше видео.");
    let words = &cues(&p)[0].words;
    assert_eq!(words[1].range, span(2.0, 2.4));
    assert_eq!(words[2].range, span(2.4, 2.4));
    let set: CaptionsSet = op(serde_json::json!({"id": "s1", "from": 0.4, "to": "0:03.4"}));
    run(&mut p, &set);
    let cue = &cues(&p)[0];
    assert_eq!(cue.range, span(0.9, 3.9));
    cue.check().unwrap();
    // Words stretch by 3.0 / 2.2 from the cue start.
    assert_eq!(
        cue.words[0].range.start(),
        SourceTime::from_micros(1_036_364)
    );
    assert_eq!(cue.words[3].range.end(), SourceTime::from_micros(3_763_636));
    let same: CaptionsSet = op(serde_json::json!({"id": "s1", "text": "Привет, это наше видео."}));
    assert_eq!(run(&mut p, &same), "");
}

#[test]
fn set_refuses_bad_times_and_text() {
    let mut p = project();
    let other_recording: CaptionsSet = op(serde_json::json!({"id": "s1", "to": 12}));
    let error = fail(&mut p, &other_recording);
    assert!(error.contains("не запись реплики s1"), "{error}");
    let inverted: CaptionsSet = op(serde_json::json!({"id": "s1", "from": 4, "to": 3}));
    assert!(fail(&mut p, &inverted).contains("позже начала"));
    let empty: CaptionsSet = op(serde_json::json!({"id": "s1", "text": " "}));
    assert!(fail(&mut p, &empty).contains("captions rm"));
    let missing: CaptionsSet = op(serde_json::json!({"id": "s9", "text": "x"}));
    assert_eq!(
        fail(&mut p, &missing),
        "s9 не найден (есть s1..s3); список: devcast ls captions"
    );
}

#[test]
fn set_through_speed_maps_to_source() {
    let mut p = project();
    p.clips[0].speeds.push(SpeedRange {
        range: span(0.0, 10.0),
        factor: 2.0,
    });
    let set: CaptionsSet = op(serde_json::json!({"id": "s2", "from": 2.4, "to": 5}));
    run(&mut p, &set);
    assert_eq!(cues(&p)[1].range, span(4.8, 10.0));
}

#[test]
fn split_at_the_nearest_pause_and_merge_back() {
    let mut p = project();
    let split: CaptionsSplit = op(serde_json::json!({"id": "s1", "at": 1.6}));
    assert_eq!(run(&mut p, &split), "реплики ~1 s1; реплики +1 s4");
    assert_eq!(texts(&p)[..2], ["s1 Привет,", "s4 это видео."]);
    assert_eq!(cues(&p)[0].range, span(0.9, 1.5));
    assert_eq!(cues(&p)[1].range, span(2.0, 3.1));
    let merge: CaptionsMerge = op(serde_json::json!({"id": "s1"}));
    assert_eq!(run(&mut p, &merge), "реплики ~1 s1; реплики -1 s4");
    assert_eq!(texts(&p)[0], "s1 Привет, это видео.");
    assert_eq!(cues(&p)[0].range, span(0.9, 3.1));
}

#[test]
fn split_and_merge_have_limits() {
    let mut p = project();
    let one_word: CaptionsSplit = op(serde_json::json!({"id": "s2", "at": 5.2}));
    assert!(fail(&mut p, &one_word).contains("одно слово"));
    let elsewhere: CaptionsSplit = op(serde_json::json!({"id": "s1", "at": 15}));
    assert!(fail(&mut p, &elsewhere).contains("не запись реплики s1"));
    let last_of_recording: CaptionsMerge = op(serde_json::json!({"id": "s2"}));
    assert!(fail(&mut p, &last_of_recording).contains("нет следующей"));
}

#[test]
fn rm_and_clear() {
    let mut p = project();
    let rm: CaptionsRm = op(serde_json::json!({"id": "s2"}));
    assert_eq!(run(&mut p, &rm), "реплики -1 s2");
    assert_eq!(run(&mut p, &CaptionsClear {}), "реплики -2 s1,s3");
    assert!(cues(&p).is_empty());
    assert_eq!(run(&mut p, &CaptionsClear {}), "");
}

#[test]
fn export_in_output_order_and_import_keeps_cue_times() {
    let mut p = project();
    p.clips.swap(0, 1);
    p.clips[1].cuts.push(span(4.0, 10.0));
    let (text, count) = export_text(&p).unwrap();
    assert_eq!(count, 3);
    assert_eq!(text, "s3 Вторая запись.\ns1 Привет, это видео.\ns2 Пока!\n");
    let before = cues(&p);
    let translated = "s3 Second recording.\n\ns1 Hi, this is a video.\n";
    let import = CaptionsImport {
        texts: parse_text(translated).unwrap(),
    };
    assert_eq!(run(&mut p, &import), "реплики ~2 s1,s3");
    let after = cues(&p);
    assert_eq!(texts(&p)[0], "s1 Hi, this is a video.");
    assert!(before.iter().zip(&after).all(|(a, b)| a.range == b.range));
    assert_eq!(after[0].words[0].range.start(), secs(1.0));
    assert_eq!(after[0].words[4].range.end(), secs(3.0));
}

#[test]
fn import_explains_bad_lines() {
    let mut p = project();
    let error = parse_text("s1 Да\nпросто текст\n").unwrap_err().to_string();
    assert!(error.starts_with("строка 2:"), "{error}");
    let error = parse_text("s1 Да\ns1 Нет\n").unwrap_err().to_string();
    assert!(error.contains("уже была"), "{error}");
    let unknown = CaptionsImport {
        texts: parse_text("s7 Кто?").unwrap(),
    };
    assert!(fail(&mut p, &unknown).contains("s7 не найден"));
    let empty = CaptionsImport {
        texts: parse_text("s1").unwrap(),
    };
    assert!(fail(&mut p, &empty).starts_with("s1: пустой текст"));
}

#[test]
fn later_tracks_give_way_to_earlier_ones() {
    let mic = vec![new_cue("m", &[("Я", 1.0, 1.4), ("говорю", 1.4, 2.0)])];
    let system = vec![
        new_cue(
            "m",
            &[
                ("музыка", 0.2, 0.8),
                ("и", 0.8, 1.2),
                ("звук", 2.2, 2.6),
                ("ещё", 2.6, 3.0),
            ],
        ),
        new_cue("m", &[("потом", 5.0, 5.4)]),
        new_cue("other", &[("тоже", 1.0, 2.0)]),
    ];
    let merged = merge_tracks(vec![mic, system]).unwrap();
    let shown: Vec<(String, Range<Source>)> = merged
        .iter()
        .map(|c| {
            let text: Vec<&str> = c.words.iter().map(|w| w.text.as_str()).collect();
            (text.join(" "), c.range)
        })
        .collect();
    assert_eq!(
        shown,
        [
            ("музыка".to_owned(), span(0.2, 0.8)),
            ("Я говорю".to_owned(), span(0.9, 2.1)),
            ("звук ещё".to_owned(), span(2.2, 3.0)),
            ("потом".to_owned(), span(4.9, 5.5)),
            ("тоже".to_owned(), span(0.9, 2.1)),
        ]
    );
}

#[test]
fn undo_redo_and_json_round_trip() {
    let mut p = project();
    let mut history = History::new(100);
    let start = p.clone();
    let split: CaptionsSplit = op(serde_json::json!({"id": "s1", "at": 1.6}));
    history.apply(&mut p, &split).unwrap();
    let set: CaptionsSet = op(serde_json::json!({"id": "s4", "text": "это наше видео."}));
    history.apply(&mut p, &set).unwrap();
    let end = p.clone();
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), p);
    history.undo(&mut p);
    history.undo(&mut p);
    assert_eq!(p, start);
    history.redo(&mut p);
    history.redo(&mut p);
    assert_eq!(p, end);
}
