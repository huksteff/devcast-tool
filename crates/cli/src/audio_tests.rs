//! Task 5.37 through the CLI: extra sound copied into the project and placed by output time,
//! levels of the recordings' tracks, split clips keeping them, the mix input.

use dc_audio::ChannelLevel;
use dc_core::{OutputTime, TimeDelta};
use dc_fx_audio_tracks::{Audio, AudioState};

use crate::test_util::{Recordings, ffmpeg};

/// `Recordings` (20 s and 10 s) with `звук/музыка.mp3` (4 s) next to the project file.
fn project() -> Recordings {
    let r = Recordings::new();
    let folder = r.dir.root().join("звук");
    std::fs::create_dir(&folder).unwrap();
    let music = folder.join("музыка.mp3");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "sine=f=440:d=4",
        music.to_str().unwrap(),
    ]);
    r
}

fn state(r: &Recordings) -> AudioState {
    r.dir.load().unwrap().state::<Audio>().unwrap()
}

#[test]
fn audio_add_copies_the_file_and_places_it_by_output_time() {
    let r = project();
    assert_eq!(r.ok("audio add звук/музыка.mp3 --at 2"), "звуки +1 au1");
    assert!(r.dir.root().join("assets/audio/музыка.mp3").is_file());
    let clip = &state(&r).clips[0];
    assert_eq!(clip.file.path, "assets/audio/музыка.mp3");
    assert_eq!(clip.file.duration, TimeDelta::from_secs_f64(4.0));
    assert_eq!(clip.range.end(), OutputTime::from_secs_f64(6.0));
    r.ok("audio add звук/музыка.mp3 --at 3 --duration 1.5");
    assert_eq!(
        state(&r).clips[1].track,
        1,
        "the first track is taken there"
    );
    r.ok("cut 0-1");
    let listed = r.ok("ls audio");
    assert!(listed.starts_with("au1  0:02.000–0:06.000"), "{listed}");
    r.ok("audio set au1 --volume 40 --normalize");
    r.ok("undo 3");
    assert_eq!(state(&r).clips.len(), 1);
    // The same file again is not copied twice.
    r.ok("audio add звук/музыка.mp3 --at 10");
    let names = std::fs::read_dir(r.dir.root().join("assets/audio")).unwrap();
    assert_eq!(names.count(), 1);
}

#[test]
fn a_failed_add_leaves_no_copy_and_bad_files_are_refused() {
    let r = project();
    let out = r.run("audio add звук/музыка.mp3 --at 40");
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stderr.contains("только внутри видео"), "{}", out.stderr);
    assert!(!r.dir.root().join("assets/audio/музыка.mp3").exists());
    let out = r.run("audio add звук/музыка.mp3 --at 1 --dry-run");
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(!r.dir.root().join("assets/audio/музыка.mp3").exists());
    std::fs::write(r.dir.root().join("звук/текст.txt"), "нет").unwrap();
    let out = r.run("audio add звук/текст.txt --at 1");
    assert!(out.stderr.contains("mp3, wav"), "{}", out.stderr);
    let out = r.run("audio add звук/нет.mp3 --at 1");
    assert!(out.stderr.contains("нет файла"), "{}", out.stderr);
}

#[test]
fn levels_follow_defaults_and_a_split_clip_keeps_its_own_in_both_halves() {
    let r = project();
    r.ok("audio channel mic --volume 50 --denoise");
    assert_eq!(r.ok("audio channel mic --clip c1 --gain 3"), "клипы ~1 c1");
    r.ok("audio mute c2");
    assert_eq!(r.ok("clip split --at 5"), "клипы ~1 c1; клипы +1 c3");
    let project = r.dir.load().unwrap();
    let levels = crate::sound::levels(r.dir.root(), &project).unwrap();
    let own = ChannelLevel::new(0.5, 3.0).unwrap().with_denoise(true);
    for clip in ["c1", "c3"] {
        let channel = levels.channel(clip.parse().unwrap(), "mic");
        assert_eq!(channel, own, "{clip}");
    }
    let c2 = levels.channel("c2".parse().unwrap(), "mic");
    assert_eq!((c2.muted(), c2.volume()), (true, 0.5));
    assert_eq!(
        levels.channel("c1".parse().unwrap(), "system"),
        ChannelLevel::default()
    );
    r.ok("audio add звук/музыка.mp3 --at 2");
    r.ok("audio set au1 --volume 25 --gain 2 --offset 1 --duration 2");
    let levels = crate::sound::levels(r.dir.root(), &r.dir.load().unwrap()).unwrap();
    let extra = &levels.extra[0];
    assert_eq!(
        extra.path,
        r.dir.root().join("assets").join("audio").join("музыка.mp3")
    );
    assert_eq!(
        (extra.offset, extra.level.factor()),
        (TimeDelta::from_secs_f64(1.0), 0.5)
    );
    let out = r.run("audio channel mic --clip c1 --volume 120");
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("от 0 до 100"), "{}", out.stderr);
}
