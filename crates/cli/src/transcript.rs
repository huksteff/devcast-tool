//! `devcast transcript [--words] [--from --to] [--model fast|accurate] [--language ru]`: what
//! is said, on the output timeline. Each recording is recognized once (whole, in source time)
//! and kept in `cache/` (`speech.rs`, shared with `captions gen` and the editor); words
//! get ids `w1, w2, …` in the order of the recordings' first clips and of time inside a
//! recording, so cutting does not renumber them. Words in cut-out parts are not shown, only
//! counted.

use std::rc::Rc;

use dc_audio::silence;
use dc_core::Op;
use dc_core::{OutputTime, SourceTime, TimeMap};
use dc_fx_cut::Cut;
use dc_ops::Step;
use dc_transcribe::Word;
use serde_json::{Map, Value, json};

use crate::edit::{Ctx, answer};
use crate::error::CliError;
use crate::ls::time;
use crate::round::thousandths;
use crate::run::Answer;
use crate::speech::{
    MediaWords, OnProgress, chosen, media_cues, project_words, silence_db, word_track,
};
use crate::workspace::Workspace;

#[derive(Debug, Clone, clap::Args)]
pub(crate) struct Args {
    #[arg(long, help = "По словам с их id (для cut w12-w20), а не по репликам")]
    words: bool,
    #[arg(long, help = "Только то, что звучит после этого момента")]
    from: Option<String>,
    #[arg(long, help = "Только то, что начинается раньше этого момента")]
    to: Option<String>,
    #[arg(long, value_parser = crate::config::parse_model, help = "fast — small на процессоре; accurate — large-v3-turbo, нужна видеокарта; без флага — captions.model из devcast config")]
    model: Option<dc_settings::SpeechModel>,
    #[arg(
        long,
        help = "Язык речи: ru, en …, auto — определить; без флага — captions.language из devcast config"
    )]
    language: Option<String>,
    #[arg(
        long,
        help = "Дорожка: mic (у импортированного видео — первая), system …; без флага — captions.tracks из devcast config (одна дорожка, иначе mic)"
    )]
    track: Option<String>,
}

/// Where a word of `media` sounds on the output; `None` if it is cut out.
pub(crate) fn placed(map: &TimeMap, media: &str, word: &Word) -> Option<(OutputTime, OutputTime)> {
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
    let tracks: Vec<String> = args.track.iter().cloned().collect();
    let speech = chosen(ctx, args.model, args.language.as_deref(), &tracks)?;
    let silent: OnProgress = Rc::new(|_| {});
    let all = project_words(
        workspace.dir(),
        &workspace.project,
        (speech.model, speech.language.as_deref()),
        word_track(&speech),
        Some(&silent),
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
            for cue in media_cues(media, &words, silence_db(&workspace.project)?)? {
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

fn secs(t: OutputTime) -> f64 {
    thousandths(t.as_secs_f64())
}

/// How far a cut at a word boundary may move to the quietest moment (decision 010).
const SNAP: dc_core::TimeDelta = dc_core::TimeDelta::from_millis(150);

/// `cut w12-w20,w31` (one op or a step of `apply plan.json`): the arguments of the step with
/// word ids in `ranges` replaced by output time ranges, from the start of the first word to the
/// end of the last one, each boundary moved to the quietest moment within [`SNAP`]; `None` for
/// any other step. Uses the words `devcast transcript` recognized without flags (the captions
/// settings of the app).
pub(crate) fn word_args(
    ctx: &Ctx<'_>,
    workspace: &Workspace,
    step: &Step,
) -> Result<Option<Map<String, Value>>, CliError> {
    let Some(Value::String(list)) = step.args.get(RANGES) else {
        return Ok(None);
    };
    if step.op != Cut::NAME || !is_word_list(list) {
        return Ok(None);
    }
    let speech = chosen(ctx, None, None, &[])?;
    let all = project_words(
        workspace.dir(),
        &workspace.project,
        (speech.model, speech.language.as_deref()),
        word_track(&speech),
        None,
    )?;
    let map = TimeMap::build(&workspace.project);
    let ranges = list
        .split(',')
        .map(|item| word_range(&all, &map, item))
        .collect::<Result<Vec<String>, CliError>>()?;
    let mut args = step.args.clone();
    args.insert(RANGES.to_owned(), ranges.join(",").into());
    Ok(Some(args))
}

/// The field of `cut` with its ranges.
const RANGES: &str = "ranges";

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
    let (start, end) = cut_bounds(
        (snap(start_word.start)?, snap(end_word.end)?),
        (start_word.start, end_word.end),
    );
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

/// The source span a word cut takes: the snapped edges, or — when a short word's edges snapped
/// to one quiet spot or crossed — the words' own bounds (µs).
pub(crate) fn cut_bounds(
    snapped: (SourceTime, SourceTime),
    words: (i64, i64),
) -> (SourceTime, SourceTime) {
    match snapped {
        (start, end) if end > start => (start, end),
        _ => (
            SourceTime::from_micros(words.0),
            SourceTime::from_micros(words.1),
        ),
    }
}
