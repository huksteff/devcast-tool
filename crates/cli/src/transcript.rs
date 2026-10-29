//! `devcast transcript [--words] [--from --to] [--model fast|accurate] [--language ru]`: what
//! is said, on the output timeline. Each recording is recognized once (whole, in source time)
//! and kept in `cache/`; words get ids `w1, w2, …` in the order of the recordings' first clips
//! and of time inside a recording, so cutting does not renumber them. Words in cut-out parts
//! are not shown, only counted.

use std::sync::atomic::AtomicBool;

use dc_audio::silence::{self, DEFAULT_DB, DEFAULT_MIN};
use dc_audio::{SourceTrack, frame_at, tracks};
use dc_core::time::Source;
use dc_core::{OutputTime, Project, Range, SourceTime, TimeMap};
use dc_project::ProjectDir;
use dc_transcribe::{Cue, Model, Recognized, Word, cues};
use serde_json::json;

use crate::edit::{Ctx, answer};
use crate::error::CliError;
use crate::ls::time;
use crate::run::Answer;
use crate::silences::pick;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Quality {
    /// «Быстрая»: small, works on the CPU.
    Fast,
    /// «Точная»: large-v3-turbo, needs a GPU.
    Accurate,
}

impl Quality {
    pub(crate) fn model(self) -> Model {
        match self {
            Self::Fast => Model::Small,
            Self::Accurate => Model::LargeV3Turbo,
        }
    }
}

#[derive(Debug, Clone, clap::Args)]
pub(crate) struct Args {
    #[arg(long, help = "По словам с их id (для cut w12-w20), а не по репликам")]
    words: bool,
    #[arg(long, help = "Только то, что звучит после этого момента")]
    from: Option<String>,
    #[arg(long, help = "Только то, что начинается раньше этого момента")]
    to: Option<String>,
    #[arg(long, value_enum, default_value_t = Quality::Fast, help = "fast — small на процессоре; accurate — large-v3-turbo, нужна видеокарта")]
    model: Quality,
    #[arg(long, help = "Язык речи: ru, en …; без флага определяется сам")]
    language: Option<String>,
    #[arg(
        long,
        help = "Дорожка: mic (по умолчанию; у импортированного видео — первая), system …"
    )]
    track: Option<String>,
}

/// Words of one recording, in its source time, and the id of its first word.
pub(crate) struct MediaWords {
    pub media: String,
    pub track: SourceTrack,
    pub language: String,
    pub words: Vec<Word>,
    pub first_id: u32,
}

/// The recognized words of every recording of `project`. Not recognized yet: recognized now
/// when `recognize` is set, otherwise an error that says to run `devcast transcript`.
pub(crate) fn project_words(
    dir: &ProjectDir,
    project: &Project,
    model: Model,
    language: Option<&str>,
    track: Option<&String>,
    recognize: bool,
) -> Result<Vec<MediaWords>, CliError> {
    let failed = |e: String| CliError::Failed(e);
    let mut found: Vec<MediaWords> = Vec::new();
    let mut next_id = 1;
    for clip in &project.clips {
        if found.iter().any(|m| m.media == clip.media.path) {
            continue;
        }
        let available = tracks(dir, &clip.media).map_err(|e| failed(e.to_string()))?;
        let chosen = pick(available, track, &clip.media.path)?;
        let cache = dc_transcribe::words_cache_path(dir, &chosen, model, language);
        let recognized =
            match dc_transcribe::read_words(&cache, &chosen).map_err(|e| failed(e.to_string()))? {
                Some(cached) => cached,
                None if recognize => {
                    let fresh =
                        recognize_track(&chosen, clip.media.duration.micros(), model, language)?;
                    dc_transcribe::write_words(&cache, &chosen, &fresh)
                        .map_err(|e| failed(e.to_string()))?;
                    fresh
                }
                None => {
                    return Err(CliError::Failed(format!(
                        "запись {} ещё не распознана: сначала devcast transcript",
                        clip.media.path
                    )));
                }
            };
        let count = u32::try_from(recognized.words.len()).unwrap_or(u32::MAX);
        found.push(MediaWords {
            media: clip.media.path.clone(),
            track: chosen,
            language: recognized.language,
            words: recognized.words,
            first_id: next_id,
        });
        next_id = next_id.saturating_add(count);
    }
    Ok(found)
}

fn recognize_track(
    track: &SourceTrack,
    duration: i64,
    model: Model,
    language: Option<&str>,
) -> Result<Recognized, CliError> {
    let failed = |e: dc_transcribe::TranscribeError| CliError::Failed(e.to_string());
    let models = dc_transcribe::models_dir().map_err(failed)?;
    let path = dc_transcribe::ensure(model, &models, &mut |_| {}, &AtomicBool::new(false))
        .map_err(failed)?;
    let samples = dc_transcribe::speech_samples(std::slice::from_ref(track), 0, frame_at(duration))
        .map_err(failed)?;
    dc_transcribe::recognize(&path, model, &samples, language, |_| {}).map_err(failed)
}

/// Where a word of `media` sounds on the output; `None` if it is cut out.
fn placed(map: &TimeMap, media: &str, word: &Word) -> Option<(OutputTime, OutputTime)> {
    let start = SourceTime::from_micros(word.start);
    let (segment, from) = map.locate(media, start)?;
    // The last microsecond of the word, then its end: a word ending at a cut still places.
    let end = SourceTime::from_micros(word.end.max(word.start + 1));
    let to = map
        .locate(media, end - dc_core::TimeDelta::from_micros(1))
        .map_or(segment.output_end(), |(_, t)| {
            t + dc_core::TimeDelta::from_micros(1)
        });
    Some((from, to.max(from)))
}

/// Shown word: id, output span, text.
struct Shown<'a> {
    id: u32,
    from: OutputTime,
    to: OutputTime,
    word: &'a Word,
}

pub(crate) fn transcript(ctx: &Ctx<'_>, args: &Args) -> Result<Answer, CliError> {
    let (from, to) = (
        time(args.from.as_ref(), "--from")?,
        time(args.to.as_ref(), "--to")?,
    );
    let mut workspace = ctx.open(false)?;
    let model = args.model.model();
    let all = project_words(
        workspace.dir(),
        &workspace.project,
        model,
        args.language.as_deref(),
        args.track.as_ref(),
        true,
    )?;
    let map = TimeMap::build(&workspace.project);
    let inside =
        |a: OutputTime, b: OutputTime| from.is_none_or(|f| b > f) && to.is_none_or(|t| a < t);
    let (mut lines, mut items, mut cut) = (Vec::new(), Vec::new(), 0);
    for media in &all {
        let shown: Vec<Shown<'_>> = media
            .words
            .iter()
            .zip(media.first_id..)
            .filter_map(|(word, id)| match placed(&map, &media.media, word) {
                Some((from, to)) => Some(Shown { id, from, to, word }),
                None => {
                    cut += 1;
                    None
                }
            })
            .collect();
        if args.words {
            for s in shown.iter().filter(|s| inside(s.from, s.to)) {
                lines.push(format!("w{}  {}–{}  {}", s.id, s.from, s.to, s.word.text));
                items.push(json!({"id": format!("w{}", s.id), "from": secs(s.from), "to": secs(s.to), "text": s.word.text}));
            }
        } else {
            let words: Vec<Word> = shown.iter().map(|s| s.word.clone()).collect();
            for cue in media_cues(media, &words)? {
                let (Some(&a), Some(&b)) = (cue.words.first(), cue.words.last()) else {
                    continue;
                };
                let (first, last) = (&shown[a], &shown[b]);
                if !inside(first.from, last.to) {
                    continue;
                }
                let span = if first.id == last.id {
                    format!("w{}", first.id)
                } else {
                    format!("w{}–w{}", first.id, last.id)
                };
                let text = cue.text(&words);
                lines.push(format!("{}–{}  {span}  {text}", first.from, last.to));
                items.push(json!({"from": secs(first.from), "to": secs(last.to), "words": [format!("w{}", first.id), format!("w{}", last.id)], "text": text}));
            }
        }
    }
    if lines.is_empty() {
        lines.push("речи не найдено".to_owned());
    }
    if cut > 0 {
        lines.push(format!("вырезано и не показано слов: {cut}"));
    }
    let language: Vec<&str> = all.iter().map(|m| m.language.as_str()).collect();
    let json = json!({"language": language, "cut": cut, "items": items});
    Ok(answer(&mut workspace, lines.join("\n"), json))
}

/// Cues of `words` (the shown words of one recording), split also at its silences.
pub(crate) fn media_cues(media: &MediaWords, words: &[Word]) -> Result<Vec<Cue>, CliError> {
    let Some(end) = media.words.last().map(|w| w.end) else {
        return Ok(Vec::new());
    };
    let whole: Range<Source> = Range::new(SourceTime::ZERO, SourceTime::from_micros(end.max(1)))
        .map_err(|e| CliError::Failed(e.to_string()))?;
    let quiet = silence::find(&media.track, whole, DEFAULT_DB, DEFAULT_MIN)
        .map_err(|e| CliError::Failed(e.to_string()))?;
    let quiet: Vec<(i64, i64)> = quiet
        .iter()
        .map(|r| (r.start().micros(), r.end().micros()))
        .collect();
    Ok(cues(words, &quiet))
}

fn secs(t: OutputTime) -> f64 {
    (t.as_secs_f64() * 1000.0).round() / 1000.0
}

/// How far a cut at a word boundary may move to the quietest moment (decision 010).
const SNAP: dc_core::TimeDelta = dc_core::TimeDelta::from_millis(150);

/// `cut w12-w20,w31`: word ids in the arguments of `cut` become output time ranges, from the
/// start of the first word to the end of the last one, each boundary moved to the quietest
/// moment within [`SNAP`]. Other arguments stay as they are. Uses the words recognized by
/// `devcast transcript` with the default model.
pub(crate) fn word_ranges(ctx: &Ctx<'_>, args: &[String]) -> Result<Vec<String>, CliError> {
    if !args.iter().any(|a| is_word_list(a)) {
        return Ok(args.to_vec());
    }
    let workspace = ctx.open(false)?;
    let all = project_words(
        workspace.dir(),
        &workspace.project,
        Quality::Fast.model(),
        None,
        None,
        false,
    )?;
    let map = TimeMap::build(&workspace.project);
    args.iter()
        .map(|arg| {
            if !is_word_list(arg) {
                return Ok(arg.clone());
            }
            let ranges: Result<Vec<String>, CliError> = arg
                .split(',')
                .map(|item| word_range(&all, &map, item))
                .collect();
            Ok(ranges?.join(","))
        })
        .collect()
}

fn is_word_list(arg: &str) -> bool {
    !arg.is_empty()
        && arg.split(',').all(|item| {
            item.split('-').all(|id| {
                id.strip_prefix('w')
                    .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            })
        })
}

/// `w12-w20` or `w12` → `a-b` in output seconds.
fn word_range(all: &[MediaWords], map: &TimeMap, item: &str) -> Result<String, CliError> {
    let ids = item
        .split('-')
        .map(|id| {
            id[1..].parse::<u32>().map_err(|_| {
                CliError::Usage(format!(
                    "{id}: такого номера слова нет; список: devcast transcript --words"
                ))
            })
        })
        .collect::<Result<Vec<u32>, CliError>>()?;
    let (first, last) = match ids[..] {
        [one] => (one, one),
        [first, last] => (first, last),
        _ => {
            return Err(CliError::Usage(format!(
                "{item}: нужно слово w12 или диапазон w12-w20"
            )));
        }
    };
    let find = |id: u32| {
        all.iter().find_map(|m| {
            let index = id.checked_sub(m.first_id)? as usize;
            m.words.get(index).map(|w| (m, w))
        })
    };
    let total: usize = all.iter().map(|m| m.words.len()).sum();
    let missing = |id: u32| {
        CliError::Failed(format!(
            "w{id} не найдено (есть w1..w{total}); список: devcast transcript --words"
        ))
    };
    let ((media, start_word), (end_media, end_word)) = (
        find(first).ok_or_else(|| missing(first))?,
        find(last).ok_or_else(|| missing(last))?,
    );
    if first > last || media.media != end_media.media {
        return Err(CliError::Usage(format!(
            "{item}: слова должны идти по порядку и быть из одной записи; разбей на части через запятую"
        )));
    }
    let snap = |t: i64| {
        silence::quietest(&media.track, SourceTime::from_micros(t), SNAP)
            .map_err(|e| CliError::Failed(e.to_string()))
    };
    let (start, end) = (snap(start_word.start)?, snap(end_word.end)?);
    let start = map
        .locate(&media.media, start)
        .or_else(|| map.locate(&media.media, SourceTime::from_micros(start_word.start)))
        .ok_or_else(|| CliError::Failed(format!("w{first} уже вырезано")))?
        .1;
    let end = map
        .locate(&media.media, end)
        .map(|(_, t)| t)
        .or_else(|| placed(map, &media.media, end_word).map(|(_, to)| to))
        .ok_or_else(|| CliError::Failed(format!("w{last} уже вырезано")))?;
    Ok(format!(
        "{:.6}-{:.6}",
        start.as_secs_f64(),
        end.as_secs_f64()
    ))
}
