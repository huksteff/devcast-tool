use dc_core::project::SpeedRange;
use dc_core::{OutputTime, Project, Range, TimeMap};
use serde_json::json;

use crate::tests::{add, project, run, secs, style};
use crate::{Callout, text_at};

fn at(p: &Project, t: f64) -> Option<String> {
    let state = p.state::<Callout>().unwrap();
    text_at(&state, &TimeMap::build(p), OutputTime::from_secs_f64(t))
}

/// Recording 1: 10 s, cut 4–5 s, 2× over 6–10 s (output 0–7); recording 2: 5 s (output 7–12).
fn scenario() -> Project {
    let mut p = project(&[10.0, 5.0]);
    let c = &mut p.clips[0];
    c.cuts.push(Range::new(secs(4.0), secs(5.0)).unwrap());
    c.speeds.push(SpeedRange {
        range: Range::new(secs(6.0), secs(10.0)).unwrap(),
        factor: 2.0,
    });
    run(&mut p, &style(json!({"text": "Старт"})));
    for event in [
        json!({"action": "on", "at": 0.5}),
        json!({"action": "text", "at": 1, "text": "Привет, мир", "duration": 1}),
        json!({"action": "text", "at": 3, "text": "🎬 Готово", "duration": 2}),
        json!({"action": "off", "at": 5.5, "duration": 1}),
        json!({"action": "text", "at": 7.5, "text": "Игнор"}),
        json!({"action": "on", "at": 8}),
        json!({"action": "text", "at": 9, "text": "Второй", "duration": 0.5}),
    ] {
        run(&mut p, &add(event));
    }
    p
}

#[test]
fn callout_text_as_snapshots() {
    let p = scenario();
    let moments = [
        0.0, 0.5, 1.0, 1.05, 1.5, 2.0, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 5.75, 6.0, 6.5, 7.6, 8.5,
        9.25, 11.9,
    ];
    let texts: Vec<String> = moments
        .iter()
        .map(|&t| format!("{t:>5}: {:?}", at(&p, t)))
        .collect();
    insta::assert_snapshot!(texts.join("\n"));
}

#[test]
fn typing_is_even_and_starts_right_after_the_event() {
    let p = scenario();
    assert_eq!(at(&p, 0.9).as_deref(), Some("Старт"));
    assert_eq!(
        at(&p, 1.0).as_deref(),
        Some("Старт"),
        "old text holds at the very start"
    );
    assert_eq!(
        at(&p, 1.001).as_deref(),
        Some("П"),
        "no empty frame on a text change"
    );
    assert_eq!(
        at(&p, 1.5).as_deref(),
        Some("Привет"),
        "11 letters, half → 6 (5.5 rounded)"
    );
    assert_eq!(at(&p, 2.5).as_deref(), Some("Привет, мир"));
}

#[test]
fn typing_runs_through_a_cut_and_at_normal_speed_over_a_sped_up_part() {
    let p = scenario();
    assert_eq!(
        at(&p, 3.999).as_deref(),
        Some("🎬 Го"),
        "half of 8 letters before the cut"
    );
    assert_eq!(
        at(&p, 4.5).as_deref(),
        Some("🎬 Гото"),
        "typing goes on after the cut"
    );
    assert_eq!(
        at(&p, 5.75).as_deref(),
        Some("🎬 Гото"),
        "a quarter of a 1 s erase at 2×"
    );
    assert_eq!(at(&p, 6.4).as_deref(), Some("🎬"));
    assert_eq!(at(&p, 6.6), None, "erased and off after 1 s of output");
}

#[test]
fn emoji_are_typed_whole() {
    let p = scenario();
    assert_eq!(at(&p, 3.2).as_deref(), Some("🎬"));
    let mut p = project(&[5.0]);
    run(&mut p, &add(json!({"action": "on", "at": 0})));
    let family = "👨‍👩‍👧 ok";
    run(
        &mut p,
        &add(json!({"action": "text", "at": 1, "text": family, "duration": 1})),
    );
    assert_eq!(at(&p, 1.01).as_deref(), Some("👨‍👩‍👧"));
}

#[test]
fn each_recording_has_its_own_callout() {
    let p = scenario();
    assert_eq!(
        at(&p, 7.6),
        None,
        "text while off is ignored; off from recording 1 stays there"
    );
    assert_eq!(at(&p, 8.5).as_deref(), Some("Старт"));
    assert_eq!(at(&p, 9.6).as_deref(), Some("Второй"));
}

#[test]
fn on_with_empty_text_waits_for_a_text_event() {
    let mut p = project(&[5.0]);
    run(&mut p, &add(json!({"action": "on", "at": 1})));
    assert_eq!(at(&p, 2.0), None);
    run(
        &mut p,
        &add(json!({"action": "text", "at": 3, "text": "Да", "duration": 0.2})),
    );
    assert_eq!(at(&p, 4.0).as_deref(), Some("Да"));
    assert_eq!(at(&p, 0.0), None);
    assert_eq!(at(&p, 5.0), None, "outside the video");
}
