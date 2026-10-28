//! Words into caption cues (task 4.14), by the rules of v1 (docs/porting/captions.md §4) with
//! the owner's answers (decision 000, «Субтитры»): Russian abbreviations do not end a cue.
//!
//! A cue ends after a sentence end, at a gap of [`PAUSE`] between words, at a silence of
//! [`SPLIT_SILENCE`] in the gap, or when it reaches [`MAX_CUE`]. Words lying wholly inside such
//! a silence are Whisper's inventions and are dropped. Short neighbours are merged, then every
//! cue gets up to [`EDGE_PAD`] around it without overlapping its neighbours.

use crate::words::Word;

pub const PAUSE: i64 = 700_000;
pub const SPLIT_SILENCE: i64 = 1_500_000;
pub const EDGE_PAD: i64 = 80_000;
pub const MAX_CUE: i64 = 12_000_000;
const MERGE_BELOW: i64 = 800_000;
const MERGE_GAP: i64 = 400_000;
const MERGED_MAX: i64 = 2_500_000;
const MERGED_MAX_CHARS: usize = 80;

/// Abbreviations that end with a dot but not a sentence (without their last dot, lowercase).
const ABBREVIATIONS: [&str; 15] = [
    "т.е", "т.д", "т.к", "т.п", "т.н", "др", "пр", "мр", "mr", "mrs", "ms", "dr", "prof", "vs",
    "etc",
];

/// A cue: its time with padding, µs, and the indices of its words in the list given to
/// [`cues`].
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub start: i64,
    pub end: i64,
    pub words: Vec<usize>,
}

impl Cue {
    /// Text of the cue's words of `words` (the list given to [`cues`]).
    pub fn text(&self, words: &[Word]) -> String {
        self.words
            .iter()
            .filter_map(|i| words.get(*i))
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn duration(&self) -> i64 {
        self.end - self.start
    }
}

/// Cues of `words` (in time order); `silences` are quiet stretches `[start, end)` of the same
/// sound, µs.
pub fn cues(words: &[Word], silences: &[(i64, i64)]) -> Vec<Cue> {
    let long: Vec<(i64, i64)> = silences
        .iter()
        .copied()
        .filter(|(s, e)| e - s >= SPLIT_SILENCE)
        .collect();
    let kept: Vec<usize> = (0..words.len())
        .filter(|&i| {
            let w = &words[i];
            !long.iter().any(|(s, e)| *s <= w.start && w.end <= *e)
        })
        .collect();
    let mut phrases: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for (n, &i) in kept.iter().enumerate() {
        current.push(i);
        let Some(&next) = kept.get(n + 1) else {
            break;
        };
        let (word, next) = (&words[i], &words[next]);
        let gap = next.start - word.end;
        let silence_between = gap > 0 && long.iter().any(|(s, e)| *s < next.start && *e > word.end);
        let too_long = word.end - words[current[0]].start >= MAX_CUE;
        if ends_sentence(&word.text) || gap >= PAUSE || silence_between || too_long {
            phrases.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        phrases.push(current);
    }
    let pieces = phrases.into_iter().filter_map(|indices| {
        let (first, last) = (*indices.first()?, *indices.last()?);
        Some(Cue {
            start: words[first].start,
            end: words[last].end,
            words: indices,
        })
    });
    let mut merged = merge_short(pieces.collect(), words);
    pad(&mut merged);
    merged
}

fn merge_short(cues: Vec<Cue>, words: &[Word]) -> Vec<Cue> {
    let mut out: Vec<Cue> = Vec::new();
    for next in cues {
        match out.last_mut() {
            Some(group)
                if group.duration() < MERGE_BELOW
                    && next.duration() < MERGE_BELOW
                    && next.start - group.end <= MERGE_GAP
                    && next.end - group.start <= MERGED_MAX
                    && group.text(words).chars().count() + next.text(words).chars().count()
                        < MERGED_MAX_CHARS =>
            {
                group.end = next.end;
                group.words.extend(next.words);
            }
            _ => out.push(next),
        }
    }
    out
}

/// Up to [`EDGE_PAD`] on both sides, at most half of the gap to a neighbour (the left side
/// counts from the neighbour's already padded end), never before 0.
fn pad(cues: &mut [Cue]) {
    for i in 0..cues.len() {
        let previous = i.checked_sub(1).map(|p| cues[p].end);
        let next = cues.get(i + 1).map(|c| c.start);
        let cue = &mut cues[i];
        let left = previous.map_or(EDGE_PAD.min(cue.start), |p| {
            EDGE_PAD.min((cue.start - p).max(0) / 2)
        });
        let right = next.map_or(EDGE_PAD, |n| EDGE_PAD.min((n - cue.end).max(0) / 2));
        cue.start = (cue.start - left).max(0);
        cue.end = (cue.end + right).max(cue.start + 1);
    }
}

fn ends_sentence(text: &str) -> bool {
    let closers: &[char] = &[')', ']', '}', '"', '\'', '”', '’', '»'];
    let core = text.trim().trim_end_matches(closers).trim();
    let Some(last) = core.chars().last() else {
        return false;
    };
    if !matches!(last, '.' | '?' | '!' | '…') {
        return false;
    }
    !(last == '.' && is_abbreviation(core))
}

fn is_abbreviation(word: &str) -> bool {
    let core = word.trim_end_matches('.').to_lowercase();
    let single_letters = !core.is_empty()
        && core
            .split('.')
            .all(|part| part.chars().count() == 1 && part.chars().all(char::is_alphabetic));
    single_letters || ABBREVIATIONS.contains(&core.as_str())
}

#[cfg(test)]
#[path = "cues_tests.rs"]
mod tests;
