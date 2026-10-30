//! Extra sound clips: placing by output time, tracks, lengths, changes, list, sound for the mix.

use dc_core::project::MediaKind;
use dc_core::{Command, Diff, OutputTime, Place, Project, Range, SourceTime, TimeDelta};
use serde_json::json;

use crate::tests::{clip, project};
use crate::{Audio, AudioAdd, AudioRm, AudioSet, SoundFile, sound};

fn o(secs: f64) -> OutputTime {
    OutputTime::from_secs_f64(secs)
}

fn file(secs: f64) -> SoundFile {
    SoundFile {
        path: "assets/audio/музыка 1.mp3".to_owned(),
        duration: TimeDelta::from_secs_f64(secs),
    }
}

fn add(at: f64, secs: f64, track: Option<u32>, duration: Option<f64>) -> AudioAdd {
    AudioAdd {
        file: file(secs),
        at: o(at),
        track,
        duration: duration.map(TimeDelta::from_secs_f64),
    }
}

fn set(args: serde_json::Value) -> AudioSet {
    serde_json::from_value(args).unwrap()
}

fn apply(p: &mut Project, command: &dyn Command) -> Diff {
    let diff = command.apply(p).unwrap();
    assert!(p.problems().is_empty(), "{:?}", p.problems());
    diff
}

/// `(track, start, end, offset)` of every extra clip, seconds.
fn places(p: &Project) -> Vec<(u32, f64, f64, f64)> {
    let state = p.state::<Audio>().unwrap();
    let place = |c: &crate::AudioClip| {
        let (start, end) = (c.range.start(), c.range.end());
        (
            c.track,
            start.as_secs_f64(),
            end.as_secs_f64(),
            c.offset.as_secs_f64(),
        )
    };
    state.clips.iter().map(place).collect()
}

#[test]
fn a_file_goes_to_the_first_free_track_cut_at_the_end_of_the_video() {
    let mut p = project();
    assert_eq!(
        apply(&mut p, &add(2.0, 5.0, None, None)).to_string(),
        "звуки +1 au1"
    );
    apply(&mut p, &add(4.0, 5.0, None, None));
    apply(&mut p, &add(7.0, 1.0, None, None));
    apply(&mut p, &add(20.0, 60.0, None, None));
    assert_eq!(
        places(&p),
        vec![
            (0, 2.0, 7.0, 0.0),
            (1, 4.0, 9.0, 0.0),
            (0, 7.0, 8.0, 0.0),
            (0, 20.0, 24.0, 0.0),
        ]
    );
    let error = add(3.0, 5.0, Some(0), None).apply(&mut p).unwrap_err();
    assert!(error.to_string().contains("на дорожке 0"), "{error}");
    apply(&mut p, &add(3.0, 5.0, Some(31), Some(1.5)));
    assert_eq!(places(&p)[4], (31, 3.0, 4.5, 0.0));
}

#[test]
fn bad_places_and_lengths_are_refused() {
    let mut p = project();
    for (command, expected) in [
        (add(24.0, 5.0, None, None), "только внутри видео"),
        (add(-1.0, 5.0, None, None), "только внутри видео"),
        (add(1.0, 5.0, None, Some(6.0)), "не хватает"),
        (add(23.0, 5.0, None, Some(2.0)), "позже видео"),
        (add(1.0, 0.05, None, None), "короче 0.1"),
        (add(1.0, 5.0, Some(32), None), "вне 0–31"),
    ] {
        let error = command.apply(&mut p).unwrap_err().to_string();
        assert!(error.contains(expected), "{command:?}: {error}");
    }
    assert!(!p.effects.contains_key("audio"));
    let error = add(0.0, 5.0, None, None)
        .apply(&mut Project::default())
        .unwrap_err();
    assert!(error.to_string().contains("нет клипов"), "{error}");
}

#[test]
fn set_moves_trims_and_levels_and_rm_removes() {
    let mut p = project();
    apply(&mut p, &add(2.0, 10.0, None, Some(4.0)));
    apply(&mut p, &add(10.0, 10.0, None, Some(2.0)));
    apply(&mut p, &set(json!({"id": "au1", "at": "5", "offset": 3})));
    assert_eq!(places(&p)[0], (0, 5.0, 9.0, 3.0), "moved with its length");
    let error = set(json!({"id": "au1", "at": 7}))
        .apply(&mut p)
        .unwrap_err();
    assert!(error.to_string().contains("уже есть звук"), "{error}");
    let error = set(json!({"id": "au1", "duration": 8}))
        .apply(&mut p)
        .unwrap_err();
    assert!(error.to_string().contains("не хватает"), "{error}");
    let diff = apply(
        &mut p,
        &set(json!({"id": "au1", "volume": 40, "gain": 2.5, "mute": true, "normalize": true})),
    );
    assert_eq!(diff.to_string(), "звуки ~1 au1");
    assert!(
        set(json!({"id": "au1", "volume": 40}))
            .apply(&mut p)
            .unwrap()
            .is_empty()
    );
    for (args, expected) in [
        (json!({"id": "au1", "volume": 120}), "от 0 до 100"),
        (json!({"id": "au1", "gain": 0}), "от 1 до 10"),
        (json!({"id": "au1", "offset": -1}), "не хватает"),
        (json!({"id": "au9", "volume": 1}), "au9 не найден"),
    ] {
        let error = set(args.clone()).apply(&mut p).unwrap_err().to_string();
        assert!(error.contains(expected), "{args}: {error}");
    }
    let placed = &sound(&p).unwrap().extra[0];
    assert_eq!(
        (placed.start, placed.end, placed.offset.as_secs_f64()),
        (o(5.0), o(9.0), 3.0)
    );
    let level = placed.level;
    assert_eq!(
        (level.volume, level.gain, level.muted, level.normalize),
        (0.4, 2.5, true, true)
    );
    let rm: AudioRm = serde_json::from_value(json!({"id": "au2"})).unwrap();
    assert_eq!(apply(&mut p, &rm).to_string(), "звуки -1 au2");
    assert_eq!(places(&p).len(), 1);
}

#[test]
fn cuts_do_not_move_extra_sound_and_ls_shows_its_output_time() {
    let mut p = project();
    apply(&mut p, &add(12.0, 10.0, None, Some(3.0)));
    let range = Range::new(
        SourceTime::from_secs_f64(1.0),
        SourceTime::from_secs_f64(5.0),
    );
    p.clips[0].cuts.push(range.unwrap());
    clip(
        &mut p,
        "media/recording-3/video.mp4",
        MediaKind::Recording,
        1.0,
    );
    assert_eq!(places(&p), vec![(0, 12.0, 15.0, 0.0)]);
    let items = (crate::effect().items)(&p).unwrap();
    assert_eq!((items[0].id.as_str(), items[0].list), ("au1", "audio"));
    assert_eq!(
        items[0].place,
        Place::Output(Range::new(o(12.0), o(15.0)).unwrap())
    );
    assert_eq!(items[0].fields["file"]["path"], "assets/audio/музыка 1.mp3");
    assert_eq!(items[0].fields["volume"], json!(100.0));
}
