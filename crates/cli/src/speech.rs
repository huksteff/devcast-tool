//! Recognized speech of the recordings and the cues `captions gen` puts in place of all cues:
//! one path for `devcast captions gen`, `devcast transcript` and the editor's «Создать
//! субтитры» (it starts `devcast --captions-worker`, `worker.rs`). Not a registry command:
//! Whisper and the sound files lie outside the project, so the caller recognizes and applies
//! [`CaptionsGen`] with the cues. Each recording is recognized whole once and kept in `cache/`
//! (`dc_transcribe` cache). Only the CLI links Whisper (docs/decisions/018-recognition-process.md).

use std::rc::Rc;
use std::sync::atomic::AtomicBool;

use dc_audio::silence::{self, DEFAULT_DB, DEFAULT_MIN};
use dc_audio::{SourceTrack, frame_at, tracks};
use dc_core::time::Source;
use dc_core::{CommandError, Project, Range, SourceTime};
use dc_fx_captions::{CaptionsGen, NewCue, merge_tracks};
use dc_ops::OpsError;
use dc_ops::recognition::Speech;
use dc_project::ProjectDir;
use dc_transcribe::{Cue, Model, Recognized, Word, cues};

/// The microphone track of a recording: the default track to listen to.
pub const MIC: &str = "mic";

/// Words of one recording, in its source time, and the id of its first word.
pub struct MediaWords {
    pub media: String,
    pub track: SourceTrack,
    pub language: String,
    pub words: Vec<Word>,
    pub first_id: u32,
}

/// Gets the share of the recognition done, 0..1.
pub type OnProgress = Rc<dyn Fn(f32)>;

fn failed(error: impl ToString) -> OpsError {
    OpsError::Failed(error.to_string())
}

/// The track `wanted` of a recording, or `mic`, or its first track.
pub fn pick(
    mut available: Vec<SourceTrack>,
    wanted: Option<&String>,
    media: &str,
) -> Result<SourceTrack, OpsError> {
    let position = match wanted {
        Some(name) => available.iter().position(|t| &t.name == name),
        None => available
            .iter()
            .position(|t| t.name == MIC)
            .or((!available.is_empty()).then_some(0)),
    };
    match position {
        Some(index) => Ok(available.swap_remove(index)),
        None if available.is_empty() => Err(OpsError::Failed(format!(
            "в {media} нет звука: искать тишину не в чем"
        ))),
        None => {
            let names: Vec<&str> = available.iter().map(|t| t.name.as_str()).collect();
            Err(OpsError::Usage(format!(
                "в {media} нет дорожки «{}»; есть: {}",
                wanted.map_or(MIC, String::as_str),
                names.join(", ")
            )))
        }
    }
}

/// The recognized words of every recording of `project` on `track`. Not recognized yet:
/// recognized now when `progress` is given (it gets the share of this call done), otherwise an
/// error that says to run `devcast transcript`.
pub fn project_words(
    dir: &ProjectDir,
    project: &Project,
    (model, language): (Model, Option<&str>),
    track: Option<&String>,
    progress: Option<&OnProgress>,
) -> Result<Vec<MediaWords>, OpsError> {
    let mut found: Vec<MediaWords> = Vec::new();
    let mut next_id = 1;
    let recordings = recordings(project);
    for (done, media) in recordings.iter().enumerate() {
        let chosen = pick(tracks(dir, media).map_err(failed)?, track, &media.path)?;
        let cache = dc_transcribe::words_cache_path(dir, &chosen, model, language);
        let recognized = match dc_transcribe::read_words(&cache, &chosen).map_err(failed)? {
            Some(cached) => cached,
            None => {
                let Some(progress) = progress else {
                    return Err(OpsError::Failed(format!(
                        "запись {} ещё не распознана: сначала devcast transcript",
                        media.path
                    )));
                };
                let share = Share::new(progress, done, recordings.len());
                let fresh =
                    recognize_track(&chosen, media.duration.micros(), model, language, share)?;
                dc_transcribe::write_words(&cache, &chosen, &fresh).map_err(failed)?;
                fresh
            }
        };
        if let Some(progress) = progress {
            progress((done + 1) as f32 / recordings.len() as f32);
        }
        let count = u32::try_from(recognized.words.len()).unwrap_or(u32::MAX);
        found.push(MediaWords {
            media: media.path.clone(),
            track: chosen,
            language: recognized.language,
            words: recognized.words,
            first_id: next_id,
        });
        next_id = next_id.saturating_add(count);
    }
    Ok(found)
}

/// The recordings of `project`, once each, in the order of their first clips.
fn recordings(project: &Project) -> Vec<&dc_core::project::Media> {
    let mut all: Vec<&dc_core::project::Media> = Vec::new();
    for clip in &project.clips {
        if !all.iter().any(|m| m.path == clip.media.path) {
            all.push(&clip.media);
        }
    }
    all
}

/// The part of the whole progress one recording of several takes.
struct Share {
    progress: OnProgress,
    before: f32,
    part: f32,
}

impl Share {
    fn new(progress: &OnProgress, done: usize, of: usize) -> Self {
        let of = of.max(1) as f32;
        Self {
            progress: Rc::clone(progress),
            before: done as f32 / of,
            part: 1.0 / of,
        }
    }

    /// Whisper's percents of this recording as the share of the whole.
    fn percent(&self, percent: i32) {
        let inside = (percent.clamp(0, 100) as f32) / 100.0;
        (self.progress)(self.before + self.part * inside);
    }
}

fn recognize_track(
    track: &SourceTrack,
    duration: i64,
    model: Model,
    language: Option<&str>,
    share: Share,
) -> Result<Recognized, OpsError> {
    let models = dc_models::models_dir().map_err(failed)?;
    let path =
        dc_models::ensure(model, &models, &mut |_| {}, &AtomicBool::new(false)).map_err(failed)?;
    let samples = dc_transcribe::speech_samples(std::slice::from_ref(track), 0, frame_at(duration))
        .map_err(failed)?;
    dc_transcribe::recognize(&path, model, &samples, language, move |p| {
        share.percent(p);
    })
    .map_err(failed)
}

/// Cues of `words` (the shown words of one recording), split also at its silences.
pub fn media_cues(media: &MediaWords, words: &[Word]) -> Result<Vec<Cue>, OpsError> {
    let Some(end) = media.words.last().map(|w| w.end) else {
        return Ok(Vec::new());
    };
    let whole: Range<Source> =
        Range::new(SourceTime::ZERO, SourceTime::from_micros(end.max(1))).map_err(failed)?;
    let quiet = silence::find(&media.track, whole, DEFAULT_DB, DEFAULT_MIN).map_err(failed)?;
    let quiet: Vec<(i64, i64)> = quiet
        .iter()
        .map(|r| (r.start().micros(), r.end().micros()))
        .collect();
    Ok(cues(words, &quiet))
}

/// The cues of `speech` in the project at `dir`, recognized as needed (`progress` gets the
/// share done): the command that puts them in place of all cues, one undo step. No speech
/// found is an error: the captions stay as they are.
pub fn generate(
    dir: &ProjectDir,
    project: &Project,
    speech: &Speech,
    progress: &OnProgress,
) -> Result<CaptionsGen, OpsError> {
    let names: Vec<Option<&String>> = if speech.tracks.is_empty() {
        vec![None]
    } else {
        speech.tracks.iter().map(Some).collect()
    };
    let count = names.len();
    let mut tracks = Vec::with_capacity(count);
    for (done, name) in names.into_iter().enumerate() {
        let part = (done as f32, count as f32);
        let inner = Rc::clone(progress);
        let track_progress: OnProgress = Rc::new(move |p| inner((part.0 + p) / part.1));
        let model = (speech.model, speech.language.as_deref());
        let all = project_words(dir, project, model, name, Some(&track_progress))?;
        let mut cues = Vec::new();
        for media in &all {
            for cue in media_cues(media, &media.words)? {
                cues.push(new_cue(media, &cue).map_err(failed)?);
            }
        }
        tracks.push(cues);
    }
    let cues = merge_tracks(tracks).map_err(failed)?;
    if cues.is_empty() {
        return Err(OpsError::Failed(
            "речи не найдено: субтитры не изменены; проверь дорожку (--tracks) и язык (--language)"
                .to_owned(),
        ));
    }
    Ok(CaptionsGen { cues })
}

fn new_cue(media: &MediaWords, cue: &Cue) -> Result<NewCue, CommandError> {
    let range = |start: i64, end: i64| -> Result<Range<Source>, CommandError> {
        Range::new(SourceTime::from_micros(start), SourceTime::from_micros(end))
            .map_err(|e| CommandError::new(e.to_string()))
    };
    let words = cue
        .words
        .iter()
        .filter_map(|&i| media.words.get(i))
        .map(|w| {
            Ok(dc_fx_captions::Word {
                text: w.text.clone(),
                range: range(w.start, w.end)?,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    Ok(NewCue {
        media: media.media.clone(),
        range: range(cue.start, cue.end)?,
        words,
    })
}
