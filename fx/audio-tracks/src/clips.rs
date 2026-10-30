//! `audio add`, `audio set`, `audio rm`: extra sound clips on the output timeline. `audio add`
//! is not an op of the registry: `devcast audio add` (dc-cli) copies and reads the file, then
//! applies [`AudioAdd`].

use dc_core::time::Output;
use dc_core::{
    Command, CommandError, Diff, Op, OutputTime, Project, Range, TimeArg, TimeDelta, TimeMap,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::{Audio, AudioClip, AudioId, AudioState, MIN_LENGTH, SoundFile, TRACKS};

#[derive(Debug, Clone, PartialEq)]
pub struct AudioAdd {
    /// The file, already in the project.
    pub file: SoundFile,
    /// Start on the output video.
    pub at: OutputTime,
    /// Extra track; without it, the first one free for the whole clip.
    pub track: Option<u32>,
    /// Length on the output; without it, the file to its end, cut at the end of the video.
    pub duration: Option<TimeDelta>,
}

impl AudioAdd {
    pub const NAME: &'static str = "audio add";
}

impl Command for AudioAdd {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let end = video_end(project)?;
        check_start(self.at, end)?;
        let length = match self.duration {
            Some(length) => length,
            None => self.file.duration.min(end.since(self.at)),
        };
        let range = range(self.at, length, end)?;
        check_file(&self.file, TimeDelta::ZERO, length)?;
        let mut state = project.state::<Audio>()?;
        let track = match self.track {
            Some(track) => check_track(track)?,
            None => (0..TRACKS)
                .find(|&t| free(&state, None, t, range))
                .ok_or_else(|| {
                    CommandError::new(format!(
                        "все {TRACKS} дорожки звука заняты в {}–{}: сократи или удали клип звука (devcast ls audio)",
                        range.start(),
                        range.end()
                    ))
                })?,
        };
        let clip = AudioClip {
            id: state.ids.allocate(),
            file: self.file.clone(),
            track,
            range,
            offset: TimeDelta::ZERO,
            volume: 100.0,
            gain: 1.0,
            muted: false,
            normalize: false,
        };
        check_overlap(&state, &clip)?;
        let id = clip.id;
        state.clips.push(clip);
        project.set_state::<Audio>(&state)?;
        Ok(Diff::added(id))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AudioSet {
    pub id: AudioId,
    /// New start on the output video; the length stays.
    pub at: Option<TimeArg<Output>>,
    /// New length, seconds of the output video.
    pub duration: Option<f64>,
    /// Second of the file played at the start (cuts the file's beginning); the start stays.
    pub offset: Option<f64>,
    /// Extra track, 0..31.
    pub track: Option<u32>,
    /// Volume, percent, 0..100.
    pub volume: Option<f64>,
    /// Gain on top of the volume, times, 1..10.
    pub gain: Option<f64>,
    /// Silent.
    pub mute: Option<bool>,
    /// Bring to −16 LUFS (EBU R128) before the volume and gain.
    pub normalize: Option<bool>,
}

impl Command for AudioSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Audio>()?;
        let index = position(&state, self.id)?;
        let old = state.clips[index].clone();
        let mut clip = old.clone();
        if self.at.is_some() || self.duration.is_some() {
            let end = video_end(project)?;
            let start = self.at.map_or(clip.range.start(), |TimeArg(t)| t);
            check_start(start, end)?;
            let length = self
                .duration
                .map_or(clip.range.duration(), TimeDelta::from_secs_f64);
            clip.range = range(start, length, end)?;
        }
        if let Some(offset) = self.offset {
            clip.offset = TimeDelta::from_secs_f64(offset);
        }
        check_file(&clip.file, clip.offset, clip.range.duration())?;
        if let Some(track) = self.track {
            clip.track = check_track(track)?;
        }
        if let Some(volume) = self.volume {
            clip.volume = within("громкость (%)", volume, 0.0, 100.0)?;
        }
        if let Some(gain) = self.gain {
            clip.gain = within("усиление (×)", gain, 1.0, 10.0)?;
        }
        clip.muted = self.mute.unwrap_or(clip.muted);
        clip.normalize = self.normalize.unwrap_or(clip.normalize);
        if clip == old {
            return Ok(Diff::default());
        }
        check_overlap(&state, &clip)?;
        state.clips[index] = clip;
        project.set_state::<Audio>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for AudioSet {
    const NAME: &'static str = "audio set";
    const ABOUT: &'static str = "Изменить клип звука: место по готовому видео, длину, начало в файле, дорожку, громкость, усиление, mute, нормализацию";
    const EXAMPLE: &'static str = "audio set au1 --at 5 --volume 40 --normalize";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AudioRm {
    pub id: AudioId,
}

impl Command for AudioRm {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Audio>()?;
        let index = position(&state, self.id)?;
        state.clips.remove(index);
        project.set_state::<Audio>(&state)?;
        Ok(Diff::removed(self.id))
    }
}

impl Op for AudioRm {
    const NAME: &'static str = "audio rm";
    const ABOUT: &'static str = "Удалить клип звука (файл в assets/audio/ остаётся)";
    const EXAMPLE: &'static str = "audio rm au1";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

fn position(state: &AudioState, id: AudioId) -> Result<usize, CommandError> {
    state
        .clips
        .iter()
        .position(|c| c.id == id)
        .ok_or_else(|| CommandError::not_found(id, state.clip_ids()))
}

fn video_end(project: &Project) -> Result<OutputTime, CommandError> {
    let duration = TimeMap::build(project).duration();
    if duration <= TimeDelta::ZERO {
        return Err(CommandError::new(
            "в проекте нет клипов: сначала добавь видео (devcast clip add)",
        ));
    }
    Ok(OutputTime::ZERO + duration)
}

fn check_start(start: OutputTime, end: OutputTime) -> Result<(), CommandError> {
    if start < OutputTime::ZERO || start >= end {
        return Err(CommandError::new(format!(
            "звук может начинаться только внутри видео 0:00.000–{end}, а не в {start}"
        )));
    }
    Ok(())
}

/// `start..start + length`, at least [`MIN_LENGTH`] and not past `end` of the video.
fn range(
    start: OutputTime,
    length: TimeDelta,
    end: OutputTime,
) -> Result<Range<Output>, CommandError> {
    if length < MIN_LENGTH {
        return Err(CommandError::new(format!(
            "клип звука короче {} с: задай длину не меньше",
            MIN_LENGTH.as_secs_f64()
        )));
    }
    let stop = start + length;
    if stop > end {
        return Err(CommandError::new(format!(
            "клип звука {start}–{stop} кончается позже видео ({end}): укороти его"
        )));
    }
    Range::new(start, stop).map_err(|e| CommandError::new(e.to_string()))
}

/// The file has sound from `offset` for `length`.
fn check_file(file: &SoundFile, offset: TimeDelta, length: TimeDelta) -> Result<(), CommandError> {
    let left = TimeDelta::from_micros(file.duration.micros() - offset.micros());
    if offset < TimeDelta::ZERO || left < length {
        return Err(CommandError::new(format!(
            "в файле {} звука {:.3} с: с {:.3} с его не хватает на {:.3} с; сократи клип или начни раньше в файле",
            file.path,
            file.duration.as_secs_f64(),
            offset.as_secs_f64(),
            length.as_secs_f64()
        )));
    }
    Ok(())
}

fn check_track(track: u32) -> Result<u32, CommandError> {
    if track < TRACKS {
        return Ok(track);
    }
    Err(CommandError::new(format!(
        "дорожка звука {track} вне 0–{}: дорожек {TRACKS}",
        TRACKS - 1
    )))
}

fn within(what: &str, value: f64, min: f64, max: f64) -> Result<f64, CommandError> {
    if (min..=max).contains(&value) {
        return Ok(value);
    }
    Err(CommandError::new(format!(
        "{what}: нужно от {min} до {max}, а не {value}"
    )))
}

/// No clip but `except` is on `track` in `range`.
fn free(state: &AudioState, except: Option<AudioId>, track: u32, range: Range<Output>) -> bool {
    state
        .clips
        .iter()
        .all(|c| Some(c.id) == except || c.track != track || c.range.intersect(&range).is_none())
}

fn check_overlap(state: &AudioState, clip: &AudioClip) -> Result<(), CommandError> {
    if free(state, Some(clip.id), clip.track, clip.range) {
        return Ok(());
    }
    Err(CommandError::new(format!(
        "на дорожке {} в {}–{} уже есть звук: выбери другую дорожку (--track) или сдвинь клип; список: devcast ls audio",
        clip.track,
        clip.range.start(),
        clip.range.end()
    )))
}
