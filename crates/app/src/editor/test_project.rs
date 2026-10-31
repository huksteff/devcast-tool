//! The project of the editor tests: three clips (a cut and a 4× range in the first, an
//! imported video last) with zooms, captions, annotations on two tracks, callout events and
//! music.

use dc_core::project::{Media, MediaKind, SpeedRange};
use dc_core::time::Source;
use dc_core::{
    Clip, ClipAudio, ClipId, Id, OutputTime, Project, Range, Rect, SourceTime, TimeDelta,
};
use dc_fx_annotations::{Annotation, Annotations, AnnotationsState, Kind};
use dc_fx_audio_tracks::{Audio, AudioClip, AudioState, SoundFile};
use dc_fx_callout::{Action, Callout, CalloutEvent, CalloutState};
use dc_fx_captions::{CaptionsEffect, CaptionsState, Cue, Word};
use dc_fx_crop::{Crop, CropState};
use dc_fx_zoom::{Mode, Zoom, ZoomEffect, ZoomState};

pub(super) fn source(start: f64, end: f64) -> Range<Source> {
    Range::new(
        SourceTime::from_secs_f64(start),
        SourceTime::from_secs_f64(end),
    )
    .unwrap()
}

pub(super) fn output(start: f64, end: f64) -> Range<dc_core::time::Output> {
    Range::new(
        OutputTime::from_secs_f64(start),
        OutputTime::from_secs_f64(end),
    )
    .unwrap()
}

fn clip(number: u32, path: &str, kind: MediaKind, secs: f64, size: (u32, u32)) -> Clip {
    Clip {
        id: ClipId::from_number(number),
        media: Media {
            path: path.to_owned(),
            kind,
            duration: TimeDelta::from_secs_f64(secs),
            size: dc_core::PixelSize {
                width: size.0,
                height: size.1,
            },
        },
        trim: source(0.0, secs),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
        audio: ClipAudio::default(),
    }
}

const SCREEN: &str = "media/screen.mp4";
const SCREEN_2: &str = "media/screen-2.mp4";

/// Three clips (a cut and a 4× range in the first, an imported video last) with zooms, captions,
/// annotations on two tracks, callout events and music; system sound muted.
pub(super) fn project() -> Project {
    let mut first = clip(1, SCREEN, MediaKind::Recording, 60.0, (2560, 1440));
    first.cuts = vec![source(20.0, 25.0)];
    first.speeds = vec![SpeedRange {
        range: source(30.0, 38.0),
        factor: 4.0,
    }];
    let mut project = Project {
        clips: vec![
            first,
            clip(2, SCREEN_2, MediaKind::Recording, 38.0, (1920, 1080)),
            clip(
                3,
                "media/логотип.mp4",
                MediaKind::Imported,
                18.0,
                (1440, 1080),
            ),
        ],
        ..Project::default()
    };
    let ids: Vec<ClipId> = project.clips.iter().map(|c| c.id).collect();
    project.clip_ids.ensure_above(ids);
    let zoom = |n, media: &str, range, scale, mode| Zoom {
        id: Id::from_number(n),
        media: media.to_owned(),
        range,
        scale,
        focus: dc_core::Point { x: 0.5, y: 0.5 },
        mode,
    };
    let mut zooms = ZoomState {
        zooms: vec![
            zoom(1, SCREEN, source(5.0, 12.0), 1.5, Mode::Follow),
            zoom(2, SCREEN, source(13.0, 17.0), 2.0, Mode::Follow),
            zoom(3, SCREEN_2, source(2.0, 9.0), 1.8, Mode::Fixed),
        ],
        ..ZoomState::default()
    };
    // New objects (a paste) take the next free numbers, as in a project made by commands.
    zooms.ids.ensure_above(zooms.zooms.iter().map(|z| z.id));
    let mut captions = captions();
    captions
        .ids
        .ensure_above(captions.cues.iter().map(|c| c.id));
    let mut annotations = annotations();
    let ids = annotations.annotations.iter().map(|a| a.id);
    annotations.ids.ensure_above(ids.collect::<Vec<_>>());
    let mut callouts = callouts();
    callouts
        .ids
        .ensure_above(callouts.events.iter().map(|e| e.id));
    let mut audio = audio();
    audio.ids.ensure_above(audio.clips.iter().map(|c| c.id));
    project.set_state::<ZoomEffect>(&zooms).unwrap();
    project.set_state::<CaptionsEffect>(&captions).unwrap();
    project.set_state::<Annotations>(&annotations).unwrap();
    project.set_state::<Callout>(&callouts).unwrap();
    project.set_state::<Audio>(&audio).unwrap();
    let mut crop = CropState::default();
    crop.settings.aspect = dc_fx_crop::Aspect::Wide16x9;
    project.set_state::<Crop>(&crop).unwrap();
    project
}

fn captions() -> CaptionsState {
    // Words share the cue evenly, with a pause of a fifth of a word after each.
    let cue = |n, media: &str, start: f64, end: f64, said: &str| {
        let words: Vec<&str> = said.split(' ').collect();
        let step = (end - start) / words.len() as f64;
        let words = words
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let from = start + step * i as f64;
                Word {
                    text: (*text).to_owned(),
                    range: source(from, from + step * 0.8),
                }
            })
            .collect();
        Cue {
            id: Id::from_number(n),
            media: media.to_owned(),
            range: source(start, end),
            words,
        }
    };
    CaptionsState {
        cues: vec![
            cue(1, SCREEN, 1.0, 8.0, "Сегодня покажу вход"),
            cue(2, SCREEN, 9.0, 18.0, "Открываем страницу логина"),
            cue(3, SCREEN_2, 5.0, 12.0, "Теперь настройки профиля"),
        ],
        ..CaptionsState::default()
    }
}

fn annotations() -> AnnotationsState {
    let annotation = |n, kind, media: &str, range, track| Annotation {
        id: Id::from_number(n),
        kind,
        media: media.to_owned(),
        range,
        track,
        rect: Rect::new(0.1, 0.1, 0.3, 0.1).unwrap(),
        rotation: 0.0,
        enter: Default::default(),
        exit: Default::default(),
        body: serde_json::Map::new(),
        keys: Vec::new(),
    };
    AnnotationsState {
        annotations: vec![
            annotation(1, Kind::Text, SCREEN, source(28.0, 40.0), 0),
            annotation(2, Kind::Blur, SCREEN_2, source(0.0, 15.0), 0),
            annotation(3, Kind::Shape, SCREEN, source(10.0, 14.0), 1),
        ],
        ..AnnotationsState::default()
    }
}

fn callouts() -> CalloutState {
    let event = |n, at: f64, secs: f64, action| CalloutEvent {
        id: Id::from_number(n),
        media: SCREEN.to_owned(),
        at: SourceTime::from_secs_f64(at),
        duration: TimeDelta::from_secs_f64(secs),
        action,
    };
    let said = "«Сохраните»".to_owned();
    CalloutState {
        events: vec![
            event(1, 45.0, 1.5, Action::On),
            event(2, 46.5, 5.0, Action::Text { text: said }),
            event(3, 52.0, 1.5, Action::Off),
        ],
        ..CalloutState::default()
    }
}

fn audio() -> AudioState {
    let mut state = AudioState::default();
    state.clips.push(AudioClip {
        id: Id::from_number(1),
        file: SoundFile {
            path: "assets/audio/музыка_фон.mp3".to_owned(),
            duration: TimeDelta::from_secs_f64(240.0),
        },
        track: 0,
        range: output(0.0, 105.0),
        offset: TimeDelta::ZERO,
        volume: 30.0,
        gain: 1.0,
        muted: false,
        normalize: false,
    });
    let muted = dc_core::ChannelLevels {
        muted: Some(true),
        ..Default::default()
    };
    state.settings.channels.insert("system".to_owned(), muted);
    state
}
