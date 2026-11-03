//! The sound rows of the timeline (`dc_ui_kit::lane`): a row per sound track of the recordings
//! (`mic`, `system`: a region per recording clip, pale where the clip is silent; off when muted
//! for every clip) and per extra sound track (a region per sound, named by its file). A moved
//! extra sound is `audio set --at` (with `--track` when it changes row), a dragged edge changes
//! its `--duration` and keeps the sound under its end (a moved start moves the offset into the
//! file too), a copy is `audio paste` with its JSON. «Дублировать» leaves the copy where it is,
//! on the first row free there (sounds of a row do not overlap). A removed row is `audio rm` of
//! its sounds and the rows above go one down; the sound button of a recording track is `audio
//! channel --mute`.

use dc_core::project::MediaKind;
use dc_core::time::Output;
use dc_core::{
    ChannelLevels, ClipId, IdKind, Project, Range, StateError, Step, TimeDelta, TimeMap,
};
use dc_ui_kit::lane::{Item, Lane, Moved, Region, Resize, id_args, seconds, time};
use serde_json::{Map, Value};

use crate::{
    Audio, AudioClip, AudioKind, AudioState, MIC_TRACK, SYSTEM_TRACK, Sound, TRACKS, sound,
};

pub const LANE: Lane = Lane {
    prefix: AudioKind::PREFIX,
    op: "audio",
    moved,
    resized,
    copied,
    duplicated,
};

fn moved(moved: &Moved) -> Step {
    let mut args = id_args(moved.item);
    args.insert("at".to_owned(), time(moved.range.start()));
    if let Some(track) = moved.track {
        args.insert("track".to_owned(), track.into());
    }
    Step::new("audio set", args)
}

fn resized(project: &Project, resize: &Resize) -> Step {
    let mut args = id_args(resize.item);
    let (range, at) = (resize.range, resize.at);
    if resize.end {
        args.insert("duration".to_owned(), seconds(at.since(range.start())));
        return Step::new("audio set", args);
    }
    args.insert("at".to_owned(), time(at));
    args.insert("duration".to_owned(), seconds(range.end().since(at)));
    let offset = project
        .state::<Audio>()
        .ok()
        .zip(resize.item.id::<AudioKind>())
        .and_then(|(s, id)| s.clips.into_iter().find(|c| c.id == id))
        .map(|c| TimeDelta::from_micros(c.offset.micros() + at.since(range.start()).micros()));
    if let Some(offset) = offset {
        args.insert("offset".to_owned(), seconds(offset));
    }
    Step::new("audio set", args)
}

fn copied(project: &Project, item: Item, _: TimeDelta) -> Option<Step> {
    let id = item.id::<AudioKind>()?;
    let state = project.state::<Audio>().ok()?;
    let clip = state.clips.into_iter().find(|c| c.id == id)?;
    let mut args = serde_json::Map::new();
    args.insert("item".to_owned(), serde_json::to_value(&clip).ok()?);
    Some(Step::new("audio paste", args))
}

fn duplicated(project: &Project, item: Item, range: Range<Output>) -> Option<Step> {
    let mut step = copied(project, item, range.duration())?;
    let track = project.state::<Audio>().ok()?.free_track(range)?;
    step.args.insert("at".to_owned(), time(range.start()));
    step.args.insert("track".to_owned(), track.into());
    Some(step)
}

/// A sound region and what it plays.
#[derive(Debug, Clone, PartialEq)]
pub struct Sounding {
    pub region: Region,
    pub plays: Plays,
    /// Volume × gain: the drawn height of a sample, 0..=10.
    pub level: f32,
}

/// What a sound region plays.
#[derive(Debug, Clone, PartialEq)]
pub enum Plays {
    /// The track of its row of recording clip `clip`, as the clip plays it.
    Clip(ClipId),
    /// An extra sound file (path in the project) from second `offset` of it, at 1×.
    File { path: String, offset: TimeDelta },
}

/// The row of a sound track of the recordings.
#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    /// `MIC_TRACK` or `SYSTEM_TRACK`.
    pub name: &'static str,
    /// Muted for every clip that does not set its own.
    pub off: bool,
    pub regions: Vec<Sounding>,
}

/// The sound rows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sounds {
    pub channels: Vec<Channel>,
    /// Extra sound rows up to the highest used track, at least the asked count.
    pub extra: Vec<Vec<Sounding>>,
}

/// The sound rows of `project`: the recording tracks `recorded` names (in the order microphone,
/// system), at least `least` extra rows. A broken state reads as the default and is the error.
pub fn rows(
    project: &Project,
    map: &TimeMap,
    recorded: &[String],
    least: u32,
) -> (Sounds, Option<StateError>) {
    let (state, error) = match project.state::<Audio>() {
        Ok(state) => (state, None),
        Err(error) => (AudioState::default(), Some(error)),
    };
    // A broken sound state is the error already: its rows draw no levels.
    let levels = sound(project).unwrap_or_default();
    let mut sounds = Sounds::default();
    for name in [MIC_TRACK, SYSTEM_TRACK] {
        if recorded.iter().any(|s| s == name) {
            let off = muted(state.settings.channels.get(name));
            let regions = recording(project, map, (name, &levels), off);
            sounds.channels.push(Channel { name, off, regions });
        }
    }
    let used = state.clips.iter().map(|c| c.track + 1).max().unwrap_or(0);
    for track in 0..used.max(least.min(TRACKS)) {
        let on_track = state.clips.iter().filter(|c| c.track == track);
        sounds.extra.push(on_track.map(extra).collect());
    }
    (sounds, error)
}

/// The sound track `name` of every recording clip at its `levels`; pale where the clip is
/// silent.
fn recording(
    project: &Project,
    map: &TimeMap,
    (name, levels): (&str, &Sound),
    all_off: bool,
) -> Vec<Sounding> {
    let mut regions = Vec::new();
    for clip in &project.clips {
        if clip.media.kind != MediaKind::Recording {
            continue;
        }
        let Some(range) = map.clip_span(clip.id) else {
            continue;
        };
        let own = clip.audio.channels.get(name).and_then(|l| l.muted);
        let level = levels
            .clips
            .iter()
            .find(|c| c.clip == clip.id)
            .and_then(|c| c.channels.get(name))
            .map_or(1.0, |l| (l.volume * l.gain) as f32);
        regions.push(Sounding {
            region: Region {
                faded: clip.audio.muted || own.unwrap_or(all_off),
                ..Region::new(range, String::new())
            },
            plays: Plays::Clip(clip.id),
            level,
        });
    }
    regions
}

fn extra(clip: &AudioClip) -> Sounding {
    let path = &clip.file.path;
    let name = path.rsplit('/').next().unwrap_or(path).to_owned();
    Sounding {
        region: Region {
            faded: clip.muted,
            item: Some(Item::of(clip.id)),
            ..Region::new(clip.range, name)
        },
        plays: Plays::File {
            path: path.clone(),
            offset: clip.offset,
        },
        level: (clip.volume / 100.0 * clip.gain) as f32,
    }
}

fn muted(levels: Option<&ChannelLevels>) -> bool {
    levels.and_then(|l| l.muted).unwrap_or(false)
}

/// The commands that remove extra sound track `track` and move the tracks above one down,
/// lowest first (clips of a track do not overlap: the track below is free by then).
pub fn remove_track(project: &Project, track: u32) -> Vec<Step> {
    let Ok(state) = project.state::<Audio>() else {
        return Vec::new();
    };
    let mut clips: Vec<_> = state.clips.iter().filter(|c| c.track >= track).collect();
    clips.sort_by_key(|c| c.track);
    clips
        .into_iter()
        .map(|c| match c.track == track {
            true => LANE.removed(Item::of(c.id)),
            false => {
                let mut args = id_args(Item::of(c.id));
                args.insert("track".to_owned(), (c.track - 1).into());
                Step::new("audio set", args)
            }
        })
        .collect()
}

/// The sound button of recording track `channel`: mutes its sound in every clip that does not
/// set its own, or sounds it again.
pub fn mute(channel: &str, mute: bool) -> Step {
    let mut args = Map::new();
    args.insert("channel".to_owned(), Value::from(channel));
    args.insert("mute".to_owned(), Value::Bool(mute));
    Step::new("audio channel", args)
}
