//! Words from Whisper tokens (decision 010, «Время слов»). A token that does not start with a
//! space or starts with punctuation continues the word before it. A word starts at the DTW
//! time of its first token minus [`DTW_LAG`] (DTW is late by 0.10…0.46 s, 0.26 on average) and
//! ends where the next word starts, at most at the end of its segment.

use serde::{Deserialize, Serialize};

/// Mean lateness of DTW times against the real start of a word, µs (decision 010).
pub const DTW_LAG: i64 = 260_000;

/// The longest a word may last, µs. Whisper gives no word ends, only starts; the last word of a
/// segment would otherwise last until the segment ends, which after a long pause is seconds.
pub const MAX_WORD: i64 = 2_000_000;

/// A recognized word in microseconds of the audio given to the recognizer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start: i64,
    pub end: i64,
    /// Mean probability of its tokens, 0–1.
    pub probability: f32,
}

/// A text token of a segment, as Whisper gives it.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub text: String,
    /// DTW time, µs; negative when Whisper has none.
    pub dtw: i64,
    /// Token's own start time, µs (`t0`), the fallback without DTW.
    pub t0: i64,
    pub probability: f32,
}

/// One segment: its text tokens and its end, µs.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub tokens: Vec<Token>,
    pub end: i64,
}

pub fn words(segments: &[Segment]) -> Vec<Word> {
    let mut words: Vec<(Word, usize)> = Vec::new();
    for segment in segments {
        for token in &segment.tokens {
            let continues = !token.text.starts_with(' ')
                || token.text.trim_start().starts_with(is_closing_punctuation);
            match words.last_mut() {
                Some((word, count)) if continues && !word.text.is_empty() => {
                    word.text.push_str(token.text.trim_start());
                    word.probability += token.probability;
                    *count += 1;
                }
                _ => {
                    let time = if token.dtw >= 0 {
                        token.dtw - DTW_LAG
                    } else {
                        token.t0
                    };
                    let word = Word {
                        text: token.text.trim().to_owned(),
                        start: time.max(0),
                        end: segment.end,
                        probability: token.probability,
                    };
                    words.push((word, 1));
                }
            }
        }
        if let Some((word, _)) = words.last_mut() {
            word.end = segment.end;
        }
    }
    let mut words: Vec<Word> = words
        .into_iter()
        .filter(|(word, _)| !word.text.is_empty())
        .map(|(mut word, count)| {
            word.probability /= count as f32;
            word
        })
        .collect();
    // Times only go forward; a word ends where the next one starts, and lasts at most MAX_WORD.
    for i in 1..words.len() {
        words[i].start = words[i].start.max(words[i - 1].start);
    }
    for i in 0..words.len() {
        let next = words.get(i + 1).map_or(i64::MAX, |w| w.start);
        let longest = words[i].start + MAX_WORD;
        words[i].end = words[i].end.min(next).min(longest).max(words[i].start);
    }
    words
}

fn is_closing_punctuation(c: char) -> bool {
    matches!(c, '.' | ',' | '!' | '?' | ':' | ';' | '…' | ')' | '»' | '%')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(text: &str, dtw: f64) -> Token {
        Token {
            text: text.to_owned(),
            dtw: (dtw * 1e6) as i64,
            t0: 0,
            probability: 0.9,
        }
    }

    #[test]
    fn pieces_join_into_words_with_shifted_times() {
        let segments = [
            Segment {
                tokens: vec![token(" При", 1.26), token("вет", 1.4), token(".", 1.5)],
                end: 2_000_000,
            },
            Segment {
                tokens: vec![token(" Это", 2.76), token(" видео", 3.06), token(" ,", 3.3)],
                end: 4_000_000,
            },
        ];
        let words = words(&segments);
        let short: Vec<(&str, i64, i64)> = words
            .iter()
            .map(|w| (w.text.as_str(), w.start, w.end))
            .collect();
        assert_eq!(
            short,
            [
                ("Привет.", 1_000_000, 2_000_000),
                ("Это", 2_500_000, 2_800_000),
                ("видео,", 2_800_000, 4_000_000),
            ]
        );
    }

    #[test]
    fn times_never_go_back_and_start_at_zero() {
        let segments = [Segment {
            tokens: vec![token(" а", 0.1), token(" б", 1.0), token(" в", 0.9)],
            end: 2_000_000,
        }];
        let starts: Vec<i64> = words(&segments).iter().map(|w| w.start).collect();
        assert_eq!(starts, [0, 740_000, 740_000]);
    }

    #[test]
    fn a_word_before_a_long_pause_lasts_at_most_two_seconds() {
        let segments = [Segment {
            tokens: vec![token(" три", 1.26)],
            end: 20_000_000,
        }];
        assert_eq!(words(&segments)[0].end, 1_000_000 + MAX_WORD);
    }

    #[test]
    fn without_dtw_the_token_time_is_used() {
        let mut first = token(" да", -1.0);
        first.dtw = -1;
        first.t0 = 300_000;
        let segments = [Segment {
            tokens: vec![first],
            end: 900_000,
        }];
        assert_eq!(words(&segments)[0].start, 300_000);
    }
}
