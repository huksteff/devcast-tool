use std::path::Path;

use dc_core::PixelSize;
use dc_render::TextMeasure;

use super::{Prepared, break_lines};
use crate::CaptionsEffect;
use crate::tests::project;

#[test]
fn lines_are_filled_evenly_and_a_long_word_stands_alone() {
    // Five words of 10 px, 13 px with the space before: 49 px for four, 62 for five.
    let plain = [10.0; 5];
    let spaced = [13.0; 5];
    assert_eq!(break_lines(&plain, &spaced, 100.0), vec![0..5]);
    // The last line is cheap in v1: 4 + 1 (penalty 274) beats 3 + 2 (169 + 122).
    assert_eq!(break_lines(&plain, &spaced, 49.0), vec![0..4, 4..5]);
    let wide = [10.0, 80.0, 10.0];
    let wide_spaced = [13.0, 83.0, 13.0];
    assert_eq!(
        break_lines(&wide, &wide_spaced, 50.0),
        vec![0..1, 1..2, 2..3]
    );
}

fn measure() -> TextMeasure {
    let mut measure = TextMeasure::new();
    measure
        .load_fonts(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/fonts"))
        .unwrap();
    measure
}

#[test]
fn pages_split_lines_evenly_within_a_cue() {
    let mut p = project();
    let mut state = p.state::<CaptionsEffect>().unwrap();
    // A frame so narrow that every word of «Привет, это видео.» is a line of its own.
    state.settings.width = 40.0;
    state.settings.size = 72.0;
    state.settings.rows = 2;
    p.set_state::<CaptionsEffect>(&state).unwrap();
    let narrow = PixelSize {
        width: 480,
        height: 1080,
    };
    let prepared = Prepared::new(state, narrow, &mut measure()).unwrap();
    let cue = &prepared.cues[0];
    assert_eq!(cue.lines.len(), 3, "{:?}", cue.lines);
    let pages: Vec<_> = cue.pages.iter().map(|p| p.lines.clone()).collect();
    assert_eq!(pages, vec![0..2, 2..3]);
    // The second page starts with its first word, «видео.» at 2.4 s; the first with the cue.
    assert_eq!(cue.pages[0].start, cue.range.start());
    assert_eq!(cue.pages[1].start.as_secs_f64(), 2.4);
    assert_eq!(cue.pages[1].end, cue.range.end());
    assert!(cue.widths.iter().all(|w| *w > 0.0));
}

#[test]
fn hidden_captions_lay_out_nothing() {
    let mut state = project().state::<CaptionsEffect>().unwrap();
    state.settings.show = false;
    let hd = PixelSize {
        width: 1920,
        height: 1080,
    };
    assert!(
        Prepared::new(state, hd, &mut measure())
            .unwrap()
            .cues
            .is_empty()
    );
}
