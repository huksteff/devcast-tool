use super::*;

/// Words `text` in a row, each `len` seconds after a gap of `gap` seconds, from `start`.
fn said(start: f64, items: &[(&str, f64, f64)]) -> Vec<Word> {
    let mut at = start;
    items
        .iter()
        .map(|(text, gap, len)| {
            at += gap;
            let word = Word {
                text: (*text).to_owned(),
                start: (at * 1e6) as i64,
                end: ((at + len) * 1e6) as i64,
                probability: 1.0,
            };
            at += len;
            word
        })
        .collect()
}

fn texts(words: &[Word], silences: &[(i64, i64)]) -> Vec<String> {
    cues(words, silences)
        .iter()
        .map(|c| c.text(words))
        .collect()
}

#[test]
fn sentences_split_even_without_a_pause() {
    let words = said(
        1.0,
        &[
            ("Сначала", 0.0, 0.4),
            ("запись.", 0.05, 0.4),
            ("Потом", 0.05, 0.4),
            ("монтаж,", 0.05, 0.4),
            ("и", 0.05, 0.2),
            ("экспорт!", 0.05, 0.5),
        ],
    );
    assert_eq!(
        texts(&words, &[]),
        ["Сначала запись.", "Потом монтаж, и экспорт!"]
    );
}

#[test]
fn russian_abbreviations_do_not_end_a_cue() {
    let words = said(
        0.0,
        &[
            ("Кнопки,", 0.0, 0.5),
            ("меню", 0.05, 0.5),
            ("и", 0.05, 0.2),
            ("т.д.", 0.05, 0.5),
            ("работают", 0.05, 0.6),
            ("везде", 0.05, 0.5),
            ("т.е.", 0.05, 0.4),
            ("всегда.", 0.05, 0.6),
        ],
    );
    assert_eq!(
        texts(&words, &[]),
        ["Кнопки, меню и т.д. работают везде т.е. всегда."]
    );
}

#[test]
fn a_pause_or_a_long_silence_splits() {
    let words = said(
        0.0,
        &[
            ("раз", 0.0, 0.9),
            ("два", 0.7, 0.9),
            ("три", 0.3, 0.9),
            ("четыре", 0.3, 0.9),
        ],
    );
    // 0.7 s gap before «два»; a 1.5 s silence overlapping the 0.3 s gap before «четыре».
    let silence = [(3_000_000, 4_500_000)];
    assert_eq!(texts(&words, &silence), ["раз", "два три", "четыре"]);
}

#[test]
fn words_inside_a_long_silence_are_dropped() {
    let words = said(
        0.0,
        &[
            ("Привет.", 0.0, 0.8),
            ("Субтитры", 1.0, 0.5),
            ("Тест.", 1.0, 0.9),
        ],
    );
    let silence = [(1_500_000, 3_200_000)];
    assert_eq!(texts(&words, &silence), ["Привет.", "Тест."]);
}

#[test]
fn a_long_phrase_breaks_at_twelve_seconds() {
    let items: Vec<(&str, f64, f64)> = (0..30).map(|_| ("слово", 0.05, 0.5)).collect();
    let cues = cues(&said(0.0, &items), &[]);
    assert_eq!(cues.len(), 2);
    assert!(cues[0].end - cues[0].start <= MAX_CUE + EDGE_PAD + 600_000);
}

#[test]
fn short_neighbours_merge_but_not_into_long_ones() {
    let words = said(
        0.0,
        &[
            ("Да.", 0.0, 0.3),
            ("Хорошо.", 0.2, 0.4),
            ("Теперь", 0.3, 0.5),
            ("длинная", 0.05, 0.5),
            ("фраза.", 0.05, 0.6),
        ],
    );
    assert_eq!(texts(&words, &[]), ["Да. Хорошо.", "Теперь длинная фраза."]);
}

#[test]
fn padding_never_overlaps_and_results_are_sorted() {
    let words = said(
        0.05,
        &[("Один.", 0.0, 1.0), ("Два.", 0.1, 1.0), ("Три.", 1.0, 1.0)],
    );
    let cues = cues(&words, &[]);
    assert_eq!(cues[0].start, 0, "no padding before zero");
    assert_eq!(cues[0].end, 1_050_000 + 50_000, "half of a 0.1 s gap");
    assert_eq!(
        cues[1].start,
        1_150_000 - 25_000,
        "half of what the neighbour left"
    );
    for pair in cues.windows(2) {
        assert!(pair[0].end <= pair[1].start, "{cues:?}");
    }
}

#[test]
fn a_last_word_stretched_over_a_pause_ends_where_the_quiet_starts() {
    // «суперскими.» of the owner's speech: Whisper gives 15.54–16.76 s, the quiet is 16.23–17.70.
    let items = [
        ("быть", 0.0, 0.54),
        ("суперскими.", 0.0, 1.22),
        ("Идёт", 0.88, 0.5),
    ];
    let words = said(15.0, &items);
    let trimmed = cues(&words, &[(16_230_000, 17_700_000)]);
    assert_eq!(trimmed[0].text(&words), "быть суперскими.");
    assert_eq!(trimmed[0].end, 16_230_000 + EDGE_PAD);
    let untouched = cues(&words, &[]);
    assert_eq!(untouched[0].end, words[1].end + EDGE_PAD);
}
