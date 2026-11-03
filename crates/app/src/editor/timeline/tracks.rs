//! What the timeline shows, read from the project: the rows in the order of the mockup and their
//! regions at output times (through the time map). Rebuilt when the project changes.

use dc_capture::audio::{MIC_TRACK, SYSTEM_TRACK};
use dc_core::project::MediaKind;
use dc_core::time::Output;
use dc_core::{
    ChannelLevels, ClipId, Effect, KeyTracks, OutputTime, Project, Range, StateError, TimeDelta,
    TimeMap,
};
use dc_fx_audio_tracks::{Audio, AudioState, Sound, TRACKS as AUDIO_TRACKS};
use dc_ui_kit::lane;
use dc_ui_kit::strings::timeline as text;

use super::scale::length;
use super::wave::{self, Source, Wave};
use crate::editor::lanes;
use crate::editor::selection::Item;

/// A row of the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Captions,
    Clips,
    Zooms,
    /// Clicks and the callout.
    Cursor,
    Keys,
    /// Annotation track `0..TRACKS`; a higher one is painted above.
    Annotations(u32),
    /// Microphone of the recordings.
    Mic,
    /// System sound of the recordings.
    System,
    /// Extra sound track `0..TRACKS`.
    Audio(u32),
}

/// An object on a row: from `range.start` to `range.end` of the output.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    pub range: Range<Output>,
    pub label: String,
    /// Second line of a clip: its length and frame size.
    pub detail: Option<String>,
    /// Silent sound or a hidden effect: drawn pale.
    pub faded: bool,
    /// The clip of a region of the clips row.
    pub clip: Option<ClipId>,
    /// The effect object of a region that can be picked and edited.
    pub item: Option<Item>,
    /// The sound a region of a sound row plays (7.11).
    pub wave: Option<Wave>,
    /// Output times of the keys of an annotation that show (7.12), in time order.
    pub keys: Vec<OutputTime>,
}

impl From<lane::Region> for Region {
    fn from(region: lane::Region) -> Self {
        Self {
            range: region.range,
            label: region.label,
            detail: None,
            faded: region.faded,
            clip: None,
            item: region.item,
            wave: None,
            keys: region.keys,
        }
    }
}

/// A speed range on the clips row.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeedBand {
    pub region: Region,
    pub factor: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub row: Row,
    pub regions: Vec<Region>,
    /// The effect of the row is switched off (captions or keys not shown, sound muted).
    pub off: bool,
}

/// Rows shown even when they hold nothing: the user added them («+ Дорожка»), the project only
/// knows tracks with objects on them. At least this many annotation and extra sound rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Least {
    pub annotations: u32,
    pub audio: u32,
}

/// All rows of a project.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tracks {
    pub duration: TimeDelta,
    pub rows: Vec<Track>,
    /// Speed ranges, drawn over the bottom of the clips row.
    pub speeds: Vec<SpeedBand>,
    /// Where a clip jumps inside itself: a cut or a speed edge.
    pub seams: Vec<OutputTime>,
    /// Keyframes of all annotations at output times, sorted: the jumps of [ and ] (7.5).
    pub keys: Vec<OutputTime>,
    /// Presses of the recordings on the «Курсор» row (7.15), set apart from `of`: they come
    /// from the recordings' files (`editor/presses.rs`).
    pub clicks: Vec<lanes::cursor::Dot>,
    /// What the shortcuts of the recordings give the panel «Показ клавиш» (7.16), set with the
    /// chips of the «Клавиши» row by `place_keys`.
    pub shortcuts: lanes::keys::Shortcuts,
    /// Broken effect states: their rows stay empty, the text says what to do.
    pub problems: Vec<String>,
}

impl Tracks {
    /// The sources of the waves of the sound rows.
    pub fn sources(&self) -> impl Iterator<Item = &Source> {
        let regions = self.rows.iter().flat_map(|t| &t.regions);
        regions.filter_map(|r| r.wave.as_ref()).map(|w| &w.source)
    }

    /// The rows of `project`; `sound` names the sound tracks its recordings have (`mic`,
    /// `system`); `least` adds the empty rows the user added.
    pub fn of(project: &Project, sound: &[String], least: Least) -> Self {
        let map = TimeMap::build(project);
        let mut tracks = Self {
            duration: map.duration(),
            ..Self::default()
        };
        let captions = tracks.problem(lanes::captions::row(project, &map));
        let audio = tracks.read::<Audio>(project);
        let keys = tracks.problem(lanes::keys::shown(project));
        let zooms = tracks.lane(lanes::zoom::regions(project, &map));
        let cursor = tracks.lane(lanes::callout::regions(project, &map));
        tracks.add_lane(Row::Captions, captions);
        tracks.clips(project, &map);
        tracks.add(Row::Zooms, zooms, false);
        tracks.add(Row::Cursor, cursor, false);
        tracks.add(Row::Keys, Vec::new(), !keys);
        tracks.annotations(project, &map, least.annotations);
        // A broken sound state is a problem already (read above): its rows draw no levels.
        let levels = dc_fx_audio_tracks::sound(project).unwrap_or_default();
        for (row, name) in [(Row::Mic, MIC_TRACK), (Row::System, SYSTEM_TRACK)] {
            if sound.iter().any(|s| s == name) {
                let off = muted(audio.settings.channels.get(name));
                let regions = source_sound(project, &map, (name, &levels), off);
                tracks.add(row, regions, off);
            }
        }
        tracks.extra_sound(&audio, least.audio);
        tracks
    }

    /// State of `E`; a broken one is a problem and reads as the default.
    fn read<E: Effect>(&mut self, project: &Project) -> E::State {
        project.state::<E>().unwrap_or_else(|e| {
            self.problems.push(e.to_string());
            E::State::default()
        })
    }

    /// The chips of the «Клавиши» row and the panel's facts from the shortcuts `keys` of the
    /// recordings of `project` read so far.
    pub fn place_keys(&mut self, project: &Project, keys: &KeyTracks) {
        let (chips, shortcuts) = lanes::keys::chips(project, keys);
        self.shortcuts = shortcuts;
        if let Some(track) = self.rows.iter_mut().find(|t| t.row == Row::Keys) {
            track.regions = chips.into_iter().map(Region::from).collect();
        }
    }

    /// What an effect's lane read; a broken state is a problem and reads as the default.
    fn problem<T: Default>(&mut self, read: Result<T, StateError>) -> T {
        read.unwrap_or_else(|error| {
            self.problems.push(error.to_string());
            T::default()
        })
    }

    /// The regions of an effect's lane; a broken state is a problem and shows nothing.
    fn lane(&mut self, regions: Result<Vec<lane::Region>, StateError>) -> Vec<Region> {
        let regions = self.problem(regions);
        regions.into_iter().map(Region::from).collect()
    }

    fn add_lane(&mut self, row: Row, lane: lane::Row) {
        let regions = lane.regions.into_iter().map(Region::from).collect();
        self.add(row, regions, lane.off);
    }

    fn add(&mut self, row: Row, regions: Vec<Region>, off: bool) {
        self.rows.push(Track { row, regions, off });
    }

    /// One region per clip, from its first shown moment to its last; speeds and seams.
    fn clips(&mut self, project: &Project, map: &TimeMap) {
        let mut regions = Vec::new();
        for clip in &project.clips {
            let Some(range) = clip_span(map, clip.id) else {
                continue;
            };
            let segments: Vec<_> = map
                .segments()
                .iter()
                .filter(|s| s.clip == clip.id)
                .collect();
            for pair in segments.windows(2) {
                self.seams.push(pair[1].output_start);
            }
            for segment in segments.iter().filter(|s| s.speed != 1.0) {
                if let Ok(range) = Range::new(segment.output_start, segment.output_end()) {
                    self.speeds.push(SpeedBand {
                        region: region(range, text::times(segment.speed)),
                        factor: segment.speed,
                    });
                }
            }
            let label = match clip.media.kind {
                MediaKind::Recording => text::clip(clip.id.number()),
                MediaKind::Imported => file_name(&clip.media.path),
            };
            let size = clip.media.size;
            let played = length(range.duration());
            regions.push(Region {
                detail: Some(text::clip_facts(&played, size.width, size.height)),
                clip: Some(clip.id),
                ..region(range, label)
            });
        }
        self.add(Row::Clips, regions, false);
    }

    /// Annotation rows up to the highest used track, at least `least` and one.
    fn annotations(&mut self, project: &Project, map: &TimeMap, least: u32) {
        let rows = lanes::annotations::rows(project, map, least).unwrap_or_else(|error| {
            self.problems.push(error.to_string());
            lanes::annotations::Rows::empty(least)
        });
        for (track, regions) in (0..).zip(rows.rows) {
            let regions = regions.into_iter().map(Region::from).collect();
            self.add(Row::Annotations(track), regions, false);
        }
        self.keys = rows.keys;
    }

    /// Extra sound rows up to the highest used track, at least `least`.
    fn extra_sound(&mut self, state: &AudioState, least: u32) {
        let used = state.clips.iter().map(|c| c.track + 1).max().unwrap_or(0);
        let count = used.max(least.min(AUDIO_TRACKS));
        for track in 0..count {
            let regions = state
                .clips
                .iter()
                .filter(|c| c.track == track)
                .map(|c| Region {
                    faded: c.muted,
                    item: Some(Item::of(c.id)),
                    wave: Some(wave::extra(c)),
                    ..region(c.range, file_name(&c.file.path))
                })
                .collect();
            self.add(Row::Audio(track), regions, false);
        }
    }
}

pub(super) fn region(range: Range<Output>, label: String) -> Region {
    Region {
        range,
        label,
        detail: None,
        faded: false,
        clip: None,
        item: None,
        wave: None,
        keys: Vec::new(),
    }
}

/// Where clip `id` plays: its segments follow each other on the output.
fn clip_span(map: &TimeMap, id: ClipId) -> Option<Range<Output>> {
    let mut segments = map.segments().iter().filter(|s| s.clip == id);
    let first = segments.next()?;
    let end = segments.next_back().unwrap_or(first).output_end();
    Range::new(first.output_start, end).ok()
}

/// The sound track `name` of every recording clip at its `levels`; pale where the clip is
/// silent.
fn source_sound(
    project: &Project,
    map: &TimeMap,
    (name, levels): (&str, &Sound),
    all_off: bool,
) -> Vec<Region> {
    let mut regions = Vec::new();
    for clip in &project.clips {
        if clip.media.kind != MediaKind::Recording {
            continue;
        }
        let Some(range) = clip_span(map, clip.id) else {
            continue;
        };
        let own = clip.audio.channels.get(name).and_then(|l| l.muted);
        let faded = clip.audio.muted || own.unwrap_or(all_off);
        let level = levels
            .clips
            .iter()
            .find(|c| c.clip == clip.id)
            .and_then(|c| c.channels.get(name))
            .map_or(1.0, |l| (l.volume * l.gain) as f32);
        regions.push(Region {
            faded,
            clip: Some(clip.id),
            wave: Some(wave::recording(map, clip.id, &clip.media, name, level)),
            ..region(range, String::new())
        });
    }
    regions
}

fn muted(levels: Option<&ChannelLevels>) -> bool {
    levels.and_then(|l| l.muted).unwrap_or(false)
}

/// «music.mp3» of «assets/audio/music.mp3».
fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}
