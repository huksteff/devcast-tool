//! Levels of the recordings' sound tracks: defaults, the clip's own, mute, reset, ranges, undo.

use dc_core::project::{Media, MediaKind};
use dc_core::{
    Clip, ClipId, Command, Diff, History, PixelSize, Project, Range, Rect, SourceTime, TimeDelta,
};
use serde_json::json;

use crate::{Audio, AudioChannel, AudioMute, AudioReset, Level, sound};

pub(crate) fn clip(project: &mut Project, path: &str, kind: MediaKind, secs: f64) -> ClipId {
    let id = project.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(secs);
    project.clips.push(Clip {
        id,
        media: Media {
            path: path.to_owned(),
            kind,
            duration,
            size: PixelSize {
                width: 1920,
                height: 1080,
            },
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
        audio: Default::default(),
    });
    id
}

/// Two recordings of 10 s and an imported video of 4 s: c1, c2, c3.
pub(crate) fn project() -> Project {
    let mut p = Project::default();
    clip(
        &mut p,
        "media/recording-1/video.mp4",
        MediaKind::Recording,
        10.0,
    );
    clip(
        &mut p,
        "media/recording-2/video.mp4",
        MediaKind::Recording,
        10.0,
    );
    clip(&mut p, "media/импорт 1.mp4", MediaKind::Imported, 4.0);
    p
}

fn channel(args: serde_json::Value) -> AudioChannel {
    serde_json::from_value(args).unwrap()
}

fn apply(p: &mut Project, command: &dyn Command) -> Diff {
    let diff = command.apply(p).unwrap();
    assert!(p.problems().is_empty(), "{:?}", p.problems());
    diff
}

/// The level in effect of `track` of the clip number `clip` (from 1).
fn level(p: &Project, clip: usize, track: &str) -> Option<Level> {
    sound(p).unwrap().clips[clip - 1]
        .channels
        .get(track)
        .copied()
}

#[test]
fn a_clip_follows_the_defaults_until_it_sets_a_level_itself() {
    let mut p = project();
    assert_eq!(level(&p, 1, "mic"), None, "no levels: everything neutral");
    let diff = apply(
        &mut p,
        &channel(json!({"channel": "mic", "volume": 80, "denoise": true})),
    );
    assert_eq!(
        diff.to_string(),
        "звук mic по умолчанию: громкость 100→80 %, шумоподавление вкл"
    );
    let diff = apply(
        &mut p,
        &channel(json!({"channel": "mic", "clip": "c1", "gain": 2})),
    );
    assert_eq!(diff.to_string(), "клипы ~1 c1");
    let c1 = level(&p, 1, "mic").unwrap();
    assert_eq!((c1.volume, c1.gain, c1.denoise), (0.8, 2.0, true));
    let c2 = level(&p, 2, "mic").unwrap();
    assert_eq!((c2.volume, c2.gain), (0.8, 1.0));
    apply(&mut p, &channel(json!({"channel": "mic", "volume": 50})));
    assert_eq!(
        level(&p, 1, "mic").unwrap().volume,
        0.5,
        "volume is not c1's own"
    );
    apply(
        &mut p,
        &channel(json!({"channel": "mic", "clip": "c1", "volume": 50})),
    );
    apply(&mut p, &channel(json!({"channel": "mic", "volume": 30})));
    assert_eq!(
        level(&p, 1, "mic").unwrap().volume,
        0.5,
        "set to the default, still own"
    );
    assert_eq!(level(&p, 2, "mic").unwrap().volume, 0.3);
    assert_eq!(level(&p, 1, "system"), None);
}

#[test]
fn mute_of_a_clip_and_of_a_track_and_reset() {
    let mut p = project();
    let mute: AudioMute = serde_json::from_value(json!({"clip": "c2"})).unwrap();
    assert_eq!(apply(&mut p, &mute).to_string(), "клипы ~1 c2");
    assert!(apply(&mut p, &mute).is_empty(), "muted already");
    let sounds = sound(&p).unwrap();
    assert!(!sounds.clips[0].muted && sounds.clips[1].muted);
    apply(
        &mut p,
        &channel(json!({"channel": "system", "clip": "c2", "mute": true})),
    );
    apply(
        &mut p,
        &channel(json!({"channel": "mic", "clip": "c2", "normalize": true})),
    );
    assert!(level(&p, 2, "system").unwrap().muted);
    let reset: AudioReset =
        serde_json::from_value(json!({"clip": "c2", "channel": "system"})).unwrap();
    apply(&mut p, &reset);
    assert_eq!(level(&p, 2, "system"), None);
    assert!(level(&p, 2, "mic").unwrap().normalize);
    let reset: AudioReset = serde_json::from_value(json!({"clip": "c2"})).unwrap();
    apply(&mut p, &reset);
    assert!(apply(&mut p, &reset).is_empty());
    assert!(
        sound(&p).unwrap().clips[1].muted,
        "reset keeps the mute of the clip"
    );
    let unmute: AudioMute = serde_json::from_value(json!({"clip": "c2", "off": true})).unwrap();
    apply(&mut p, &unmute);
    assert_eq!(p, project());
}

#[test]
fn wrong_levels_tracks_and_clips_are_refused() {
    let mut p = project();
    for (args, expected) in [
        (json!({"channel": "mic", "volume": 101}), "от 0 до 100"),
        (json!({"channel": "mic", "gain": 0.5}), "от 1 до 10"),
        (json!({"channel": "mic", "gain": 11}), "от 1 до 10"),
        (json!({"channel": "left"}), "у записи — system и mic"),
        (json!({"channel": "audio-0"}), "нет дорожки звука"),
        (json!({"channel": "audio-1", "clip": "c1"}), "c1 — запись"),
        (
            json!({"channel": "mic", "clip": "c3"}),
            "c3 — импортированное видео",
        ),
        (json!({"channel": "mic", "clip": "c9"}), "c9 не найден"),
    ] {
        let error = channel(args.clone()).apply(&mut p).unwrap_err().to_string();
        assert!(error.contains(expected), "{args}: {error}");
    }
    apply(
        &mut p,
        &channel(json!({"channel": "audio-2", "clip": "c3", "volume": 0})),
    );
    assert_eq!(level(&p, 3, "audio-2").unwrap().volume, 0.0);
    assert!(
        channel(json!({"channel": "mic"}))
            .apply(&mut p)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn undo_redo_and_the_project_file_keep_the_levels() {
    let mut p = project();
    let mut history = History::new(100);
    history
        .apply(&mut p, &channel(json!({"channel": "mic", "volume": 70})))
        .unwrap();
    history
        .apply(
            &mut p,
            &channel(json!({"channel": "system", "clip": "c1", "mute": true})),
        )
        .unwrap();
    let done = p.clone();
    let text = serde_json::to_string(&p).unwrap();
    assert!(
        text.contains(r#""audio":{"channels":{"system":{"muted":true}}}"#),
        "{text}"
    );
    assert_eq!(serde_json::from_str::<Project>(&text).unwrap(), done);
    history.undo(&mut p);
    history.undo(&mut p);
    assert_eq!(p, project());
    assert!(!p.effects.contains_key("audio"));
    history.redo(&mut p);
    history.redo(&mut p);
    assert_eq!(p, done);
    assert!(
        p.state::<Audio>()
            .unwrap()
            .settings
            .channels
            .contains_key("mic")
    );
}
