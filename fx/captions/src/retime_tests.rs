use dc_core::{Range, SourceTime};

use crate::model::Word;
use crate::retime::{rescale, retext};

fn ms(t: i64) -> SourceTime {
    SourceTime::from_micros(t * 1000)
}

fn span(a: i64, b: i64) -> Range<dc_core::time::Source> {
    Range::new(ms(a), ms(b)).unwrap()
}

fn words(list: &[(&str, i64, i64)]) -> Vec<Word> {
    list.iter()
        .map(|(text, a, b)| Word {
            text: (*text).to_owned(),
            range: span(*a, *b),
        })
        .collect()
}

fn short(words: &[Word]) -> Vec<(String, i64, i64)> {
    words
        .iter()
        .map(|w| {
            (
                w.text.clone(),
                w.range.start().micros() / 1000,
                w.range.end().micros() / 1000,
            )
        })
        .collect()
}

fn owned(list: &[(&str, i64, i64)]) -> Vec<(String, i64, i64)> {
    list.iter()
        .map(|(t, a, b)| ((*t).to_owned(), *a, *b))
        .collect()
}

const OLD: [(&str, i64, i64); 4] = [
    ("Привет,", 0, 400),
    ("это", 500, 700),
    ("наше", 700, 1000),
    ("видео.", 1200, 1600),
];

#[test]
fn unchanged_words_keep_times_and_new_ones_share_the_gap() {
    let new = retext(&words(&OLD), "Привет, это  моё новое видео.").unwrap();
    assert_eq!(
        short(&new),
        owned(&[
            ("Привет,", 0, 400),
            ("это", 500, 700),
            ("моё", 700, 950),
            ("новое", 950, 1200),
            ("видео.", 1200, 1600),
        ])
    );
}

#[test]
fn a_new_text_spreads_evenly_over_the_words() {
    let new = retext(&words(&OLD), "Hello, this is our video.").unwrap();
    assert_eq!(
        short(&new),
        owned(&[
            ("Hello,", 0, 320),
            ("this", 320, 640),
            ("is", 640, 960),
            ("our", 960, 1280),
            ("video.", 1280, 1600),
        ])
    );
}

#[test]
fn words_added_at_the_edges_take_the_edge_gaps() {
    let new = retext(&words(&OLD[1..3]), "Ну это наше").unwrap();
    assert_eq!(
        short(&new),
        owned(&[("Ну", 500, 500), ("это", 500, 700), ("наше", 700, 1000)])
    );
    let new = retext(&words(&OLD[..2]), "Привет, это да").unwrap();
    assert_eq!(new[2].range, span(700, 700));
}

#[test]
fn removed_words_leave_their_time_empty() {
    let new = retext(&words(&OLD), "Привет, видео.").unwrap();
    assert_eq!(
        short(&new),
        owned(&[("Привет,", 0, 400), ("видео.", 1200, 1600)])
    );
}

#[test]
fn empty_text_is_refused() {
    let error = retext(&words(&OLD), "  \n ").unwrap_err().to_string();
    assert!(error.contains("captions rm"), "{error}");
}

#[test]
fn rescale_stretches_words_inside_the_new_range() {
    let new = rescale(&words(&OLD), span(0, 1600), span(1000, 4200)).unwrap();
    assert_eq!(
        short(&new),
        owned(&[
            ("Привет,", 1000, 1800),
            ("это", 2000, 2400),
            ("наше", 2400, 3000),
            ("видео.", 3400, 4200),
        ])
    );
    let squeezed = rescale(&words(&OLD), span(0, 1600), span(0, 2)).unwrap();
    assert!(
        squeezed
            .windows(2)
            .all(|w| w[0].range.end() <= w[1].range.start())
    );
    assert!(squeezed.iter().all(|w| w.range.end() <= ms(2)));
}
