//! Word times after an edit (docs/porting/captions.md §9, decision 000 «Субтитры»): a new text
//! keeps the times of unchanged words and spreads new words evenly over the gap they fill; a new
//! cue time stretches all words linearly.

use dc_core::time::Source;
use dc_core::{CommandError, Range, SourceTime};

use crate::model::Word;

pub(crate) fn range(start: SourceTime, end: SourceTime) -> Result<Range<Source>, CommandError> {
    Range::new(start, end).map_err(|e| CommandError::new(e.to_string()))
}

/// `tokens` spread evenly over `span`, in order.
pub(crate) fn even(tokens: &[&str], span: Range<Source>) -> Result<Vec<Word>, CommandError> {
    let (start, length) = (span.start().micros(), span.duration().micros());
    let count = tokens.len() as i64;
    let at = |i: i64| SourceTime::from_micros(start + length * i / count);
    tokens
        .iter()
        .zip(0..)
        .map(|(text, i)| {
            Ok(Word {
                text: (*text).to_owned(),
                range: range(at(i), at(i + 1))?,
            })
        })
        .collect()
}

/// Words of `text` (split at whitespace) in place of `old`: words found unchanged in order keep
/// their times, the others share the time between them evenly.
pub(crate) fn retext(old: &[Word], text: &str) -> Result<Vec<Word>, CommandError> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let (Some(first), Some(last)) = (old.first(), old.last()) else {
        return Err(CommandError::new(
            "у реплики нет слов: такую реплику нужно удалить (captions rm)",
        ));
    };
    if tokens.is_empty() {
        return Err(CommandError::new(
            "пустой текст реплики: чтобы убрать реплику, удали её (captions rm)",
        ));
    }
    let mut words = Vec::with_capacity(tokens.len());
    let mut cursor = (0, first.range.start());
    for (kept_old, kept_new) in common(old, &tokens)
        .into_iter()
        .chain([(old.len(), tokens.len())])
    {
        let gap_end = old
            .get(kept_old)
            .map_or(last.range.end(), |w| w.range.start());
        words.extend(even(
            &tokens[cursor.0..kept_new],
            range(cursor.1, gap_end)?,
        )?);
        if let Some(word) = old.get(kept_old) {
            words.push(word.clone());
            cursor = (kept_new + 1, word.range.end());
        }
    }
    Ok(words)
}

/// Index pairs `(old, new)` of the longest run of equal words in order.
fn common(old: &[Word], new: &[&str]) -> Vec<(usize, usize)> {
    let (n, m) = (old.len(), new.len());
    let mut longest = vec![vec![0_u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            longest[i][j] = if old[i].text == new[j] {
                longest[i + 1][j + 1] + 1
            } else {
                longest[i + 1][j].max(longest[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut pairs) = (0, 0, Vec::new());
    while i < n && j < m {
        if old[i].text == new[j] {
            pairs.push((i, j));
            (i, j) = (i + 1, j + 1);
        } else if longest[i + 1][j] >= longest[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

/// `words` of a cue that was `from` and is now `to`: stretched linearly, in order, inside `to`.
pub(crate) fn rescale(
    words: &[Word],
    from: Range<Source>,
    to: Range<Source>,
) -> Result<Vec<Word>, CommandError> {
    let scale = to.duration().micros() as f64 / from.duration().micros() as f64;
    let map = |t: SourceTime| {
        let offset = t.since(from.start()).micros() as f64 * scale;
        to.start() + dc_core::TimeDelta::from_micros(offset.round() as i64)
    };
    let mut cursor = to.start();
    words
        .iter()
        .map(|word| {
            let start = map(word.range.start()).clamp(cursor, to.end());
            let end = map(word.range.end()).clamp(start, to.end());
            cursor = end;
            Ok(Word {
                text: word.text.clone(),
                range: range(start, end)?,
            })
        })
        .collect()
}
