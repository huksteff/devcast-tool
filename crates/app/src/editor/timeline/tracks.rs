//! What the timeline shows, read from the project: the rows in the order of the mockup and their
//! regions at output times (through the time map). Rebuilt when the project changes.

use dc_capture::audio::{MIC_TRACK, SYSTEM_TRACK};
use dc_core::project::MediaKind;
use dc_core::time::Output;
use dc_core::{ChannelLevels, ClipId, Effect, OutputTime, Project, Range, TimeDelta, TimeMap};
use dc_fx_annotations::{Annotations, Kind, TRACKS};
use dc_fx_audio_tracks::{Audio, AudioState, Sound, TRACKS as AUDIO_TRACKS};
use dc_fx_callout::{Action, Callout, CalloutState};
use dc_fx_captions::CaptionsEffect;
use dc_fx_keys::Keys;
use dc_fx_zoom::{Mode, ZoomEffect, ZoomState};
use dc_ui_kit::strings::timeline as text;

use super::marks;
use super::scale::length;
use super::wave::{self, Source, Wave};
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
        let captions = tracks.read::<CaptionsEffect>(project);
        let audio = tracks.read::<Audio>(project);
        let keys = tracks.read::<Keys>(project);
        let zooms = tracks.read::<ZoomEffect>(project);
        let callout = tracks.read::<Callout>(project);
        tracks.add(
            Row::Captions,
            captions_of(&map, &captions),
            !captions.settings.show,
        );
        tracks.clips(project, &map);
        tracks.add(Row::Zooms, zooms_of(&map, &zooms), false);
        let cursor = callouts_of(&map, &callout, &mut tracks.problems);
        tracks.add(Row::Cursor, cursor, false);
        tracks.add(Row::Keys, Vec::new(), !keys.settings.show);
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
        let state = self.read::<Annotations>(project);
        let count = state
            .annotations
            .iter()
            .map(|a| a.track + 1)
            .max()
            .unwrap_or(1)
            .max(least.clamp(1, TRACKS));
        for track in 0..count {
            let regions = state
                .annotations
                .iter()
                .filter(|a| a.track == track)
                .filter_map(|a| {
                    let range = map.output_span(&a.media, a.range)?;
                    Some(Region {
                        item: Some(Item::Annotation(a.id)),
                        keys: marks::shown(map, a, range),
                        ..region(range, kind_name(a.kind).to_owned())
                    })
                })
                .collect();
            self.add(Row::Annotations(track), regions, false);
        }
        for annotation in &state.annotations {
            if let Some(span) = map.output_span(&annotation.media, annotation.range) {
                self.keys.extend(marks::shown(map, annotation, span));
            }
        }
        self.keys.sort_unstable();
        self.keys.dedup();
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
                    item: Some(Item::Sound(c.id)),
                    wave: Some(wave::extra(c)),
                    ..region(c.range, file_name(&c.file.path))
                })
                .collect();
            self.add(Row::Audio(track), regions, false);
        }
    }
}

fn region(range: Range<Output>, label: String) -> Region {
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

fn captions_of(map: &TimeMap, state: &dc_fx_captions::CaptionsState) -> Vec<Region> {
    let faded = !state.settings.show;
    state
        .cues
        .iter()
        .filter_map(|cue| {
            let range = map.output_span(&cue.media, cue.range)?;
            Some(Region {
                faded,
                item: Some(Item::Cue(cue.id)),
                ..region(range, cue.text())
            })
        })
        .collect()
}

/// Where clip `id` plays: its segments follow each other on the output.
fn clip_span(map: &TimeMap, id: ClipId) -> Option<Range<Output>> {
    let mut segments = map.segments().iter().filter(|s| s.clip == id);
    let first = segments.next()?;
    let end = segments.next_back().unwrap_or(first).output_end();
    Range::new(first.output_start, end).ok()
}

fn zooms_of(map: &TimeMap, state: &ZoomState) -> Vec<Region> {
    state
        .zooms
        .iter()
        .filter_map(|zoom| {
            let range = map.output_span(&zoom.media, zoom.range)?;
            let label = match zoom.mode {
                Mode::Follow => text::times(zoom.scale),
                Mode::Fixed => text::fixed_zoom(zoom.scale),
            };
            Some(Region {
                item: Some(Item::Zoom(zoom.id)),
                ..region(range, label)
            })
        })
        .collect()
}

fn callouts_of(map: &TimeMap, state: &CalloutState, problems: &mut Vec<String>) -> Vec<Region> {
    let mut regions = Vec::new();
    for event in &state.events {
        let span = match event.span() {
            Ok(span) => span,
            Err(e) => {
                problems.push(format!("{}: {e}", event.id));
                continue;
            }
        };
        let label = match &event.action {
            Action::On => text::CALLOUT_ON.to_owned(),
            Action::Off => text::CALLOUT_OFF.to_owned(),
            Action::Text { text } => text.clone(),
        };
        regions.extend(map.output_span(&event.media, span).map(|r| Region {
            item: Some(Item::Callout(event.id)),
            ..region(r, label)
        }));
    }
    regions
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
            wave: Some(wave::recording(map, clip.id, &clip.media, name, level)),
            ..region(range, String::new())
        });
    }
    regions
}

fn muted(levels: Option<&ChannelLevels>) -> bool {
    levels.and_then(|l| l.muted).unwrap_or(false)
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Text => text::TEXT,
        Kind::Shape => text::SHAPE,
        Kind::Image => text::IMAGE,
        Kind::Drawing => text::DRAWING,
        Kind::Blur => text::BLUR,
    }
}

/// «music.mp3» of «assets/audio/music.mp3».
fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}
