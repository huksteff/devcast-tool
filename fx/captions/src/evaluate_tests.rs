use std::path::Path;

use dc_core::{OutputTime, PixelSize, Project, TimeMap};
use dc_render::{Layer, Rgba, Scene, Text, TextMeasure};

use super::evaluate;
use crate::CaptionsEffect;
use crate::layout::Prepared;
use crate::style::{Animation, CaptionsSettings};
use crate::tests::project;

const HD: PixelSize = PixelSize {
    width: 1920,
    height: 1080,
};

/// The project of `crate::tests` (`s1` «Привет, это видео.» over 0.9–3.1 s, `s2` «Пока!» over
/// 4.9–5.6 s) with `settings`.
fn prepared(change: impl FnOnce(&mut CaptionsSettings)) -> (Project, Prepared) {
    let p = project();
    let mut state = p.state::<CaptionsEffect>().unwrap();
    change(&mut state.settings);
    let mut measure = TextMeasure::new();
    measure
        .load_fonts(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/fonts"))
        .unwrap();
    let prepared = Prepared::new(state, HD, &mut measure).unwrap();
    (p, prepared)
}

/// The text layer at output second `t`, if any; asserts the plate under it.
fn at(p: &Project, prepared: &Prepared, t: f64) -> Option<Text> {
    let mut scene = Scene::new(HD);
    evaluate(
        prepared,
        &TimeMap::build(p),
        OutputTime::from_secs_f64(t),
        &mut scene,
    );
    let layers: Vec<&Layer> = scene.painted().map(|(_, layer)| layer).collect();
    match layers.as_slice() {
        [] => None,
        [Layer::Shapes(plate), Layer::Text(text)] => {
            assert_eq!(plate.len(), 1);
            Some(text.clone())
        }
        other => panic!("{other:?}"),
    }
}

fn words(text: &Text) -> Vec<(String, Rgba)> {
    text.runs
        .iter()
        .map(|r| (r.text.clone(), r.color))
        .collect()
}

#[test]
fn words_are_coloured_by_what_was_said() {
    let (p, prepared) = prepared(|_| {});
    let s = CaptionsSettings::default();
    let text = at(&p, &prepared, 1.2).unwrap();
    assert_eq!(
        words(&text),
        vec![
            ("Привет,".to_owned(), s.highlight),
            (" это".to_owned(), s.inactive),
            (" видео.".to_owned(), s.inactive),
        ]
    );
    // Between «это» (until 2.4) and «видео.» (from 2.4) nothing is lost; at 2.7 «видео.» is said.
    let later = at(&p, &prepared, 2.7).unwrap();
    let colours: Vec<Rgba> = words(&later).into_iter().map(|w| w.1).collect();
    assert_eq!(colours, vec![s.color, s.color, s.highlight]);
}

#[test]
fn caption_shows_over_its_cue_only() {
    let (p, shown) = prepared(|_| {});
    assert!(at(&p, &shown, 0.5).is_none(), "before the cue");
    assert!(at(&p, &shown, 4.0).is_none(), "a long gap between cues");
    assert_eq!(words(&at(&p, &shown, 5.2).unwrap())[0].0, "Пока!");
    assert_eq!(
        words(&at(&p, &shown, 11.2).unwrap())[0].0,
        "Вторая",
        "second recording"
    );
    let (p, hidden) = prepared(|s| s.show = false);
    assert!(at(&p, &hidden, 1.2).is_none());
}

#[test]
fn plate_sits_over_the_bottom_in_the_middle() {
    let (p, prepared) = prepared(|s| s.animation = Animation::None);
    let rect = at(&p, &prepared, 1.5).unwrap().rect;
    assert!((rect.x + rect.w / 2.0 - 960.0).abs() < 0.01, "{rect:?}");
    // 3% of 1080 above the bottom; one line of 1.32 font sizes and 2 × 0.78 of padding.
    assert!((rect.y + rect.h - (1080.0 - 32.4)).abs() < 0.01, "{rect:?}");
    assert!(
        (f64::from(rect.h) - 30.0 * (1.32 + 1.56)).abs() < 0.01,
        "{rect:?}"
    );
}

#[test]
fn page_fades_in_and_out_by_output_time() {
    let (p, prepared) = prepared(|_| {});
    let opacity = |t| at(&p, &prepared, t).map(|text| text.opacity);
    // The cue starts at 0.9 s: fade from 0.3 in 180 ms.
    assert!((opacity(0.9).unwrap() - 0.3).abs() < 1e-3);
    assert!((opacity(1.08).unwrap() - 1.0).abs() < 1e-3);
    // The last word ends at 3.0 s and no cue follows closely: out to nothing by then.
    let half = opacity(2.91).unwrap();
    assert!((half - 0.65 * 0.5).abs() < 1e-3, "{half}");
    assert!(opacity(3.05).unwrap().abs() < 1e-6);
}

#[test]
fn rise_and_pop_move_and_scale_the_plate() {
    let (p, rise) = prepared(|s| s.animation = Animation::Rise);
    let (_, none) = prepared(|s| s.animation = Animation::None);
    let start = at(&p, &rise, 0.9).unwrap().rect;
    let rest = at(&p, &none, 1.5).unwrap().rect;
    let centre = |r: dc_render::PxRect| r.y + r.h / 2.0;
    assert!(
        (centre(start) - centre(rest) - 18.0).abs() < 0.01,
        "18 px lower at size 30"
    );
    assert!((start.w / rest.w - 0.985).abs() < 1e-3);
}
