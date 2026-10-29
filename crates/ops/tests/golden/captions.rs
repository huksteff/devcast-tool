//! Task 5.28: captions over bar videos — words coloured by what was said, two lines, the shadow
//! without a plate and the middle of the pop animation.

use std::error::Error;

use dc_core::time::Source;
use dc_core::{Command, CursorTracks, Project, Range, Rect, SourceTime};
use dc_fx_captions::{CaptionsGen, CaptionsStyle, NewCue, Word};
use serde_json::{Value, json};

use crate::common::{TestResult, clip, size};

const LINE: &str = "Сегодня я покажу, как вырезать паузы и поставить зумы на клики";

/// A 2 s clip of 1280×720 with one cue saying [`LINE`] from 0.1 to 1.9 s, word after word.
fn project(look: Value) -> Result<Project, Box<dyn Error>> {
    let mut project = Project::default();
    clip(&mut project, "media/a.mp4", size(1280, 720), Rect::FULL)?;
    let at = |s: f64| SourceTime::from_secs_f64(s);
    let span =
        |a: f64, b: f64| -> Result<Range<Source>, Box<dyn Error>> { Ok(Range::new(at(a), at(b))?) };
    let texts: Vec<&str> = LINE.split(' ').collect();
    let step = 1.8 / texts.len() as f64;
    let words = texts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let start = 0.1 + step * i as f64;
            Ok(Word {
                text: (*text).to_owned(),
                range: span(start, start + step)?,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let cue = NewCue {
        media: "media/a.mp4".to_owned(),
        range: span(0.05, 1.95)?,
        words,
    };
    CaptionsGen { cues: vec![cue] }.apply(&mut project)?;
    serde_json::from_value::<CaptionsStyle>(look)?.apply(&mut project)?;
    Ok(project)
}

fn check(name: &str, look: Value, at: f64) -> TestResult {
    let project = project(look)?;
    crate::common::check(
        &format!("captions/{name}"),
        &project,
        CursorTracks::new(),
        at,
    )
}

#[test]
fn golden_words_coloured_by_what_was_said() -> TestResult {
    check("plain", json!({"width": 90}), 0.8)
}

#[test]
fn golden_two_lines_on_a_page() -> TestResult {
    check("two_rows", json!({"rows": 2, "size": 40}), 1.2)
}

#[test]
fn golden_shadow_without_a_plate() -> TestResult {
    check(
        "shadow",
        json!({"background": 0, "width": 90, "size": 44, "font": "Montserrat"}),
        1.0,
    )
}

#[test]
fn golden_halfway_through_pop() -> TestResult {
    // The cue starts at 0.05 s; pop takes 180 ms.
    check("pop", json!({"animation": "pop", "width": 90}), 0.14)
}
