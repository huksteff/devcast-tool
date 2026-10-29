use dc_core::project::SpeedRange;
use dc_core::{KeyTracks, PixelSize, Project, Range, TimeMap};
use dc_render::{Layer, Text};
use serde_json::json;

use super::*;
use crate::state::{KeysSettings, KeysState};
use crate::test_util::{HD, o, project, s, shown, tracks};

fn prepare(p: &Project, state: KeysState) -> Prepared {
    Prepared::new(state, &tracks(), &TimeMap::build(p))
}

fn text_in(prepared: &Prepared, size: PixelSize, secs: f64) -> Option<Text> {
    let mut scene = Scene::new(size);
    evaluate(prepared, o(secs), &mut scene);
    let mut layers = scene.painted();
    let found = layers.next().map(|(band, layer)| match layer {
        Layer::Text(text) if band == Band::Overlay => text.clone(),
        other => panic!("not a text of the overlay: {other:?}"),
    });
    assert!(layers.next().is_none(), "one plate at most");
    found
}

fn text_at(prepared: &Prepared, secs: f64) -> Option<Text> {
    text_in(prepared, HD, secs)
}

fn label(prepared: &Prepared, secs: f64) -> Option<String> {
    text_at(prepared, secs).map(|t| t.runs[0].text.clone())
}

fn with(settings: KeysSettings) -> KeysState {
    KeysState { settings }
}

#[test]
fn keys_through_a_cut_a_speed_up_and_the_imported_clip() {
    let mut p = project();
    // 2× on 3–4 s and cut 8.8–9.2 s: output 3–3.5 is 3–4, 3.5–8.3 is 4–8.8, 8.3–9.1 is
    // 9.2–10, then the imported clip to 11.1. Ctrl+Z lands at 3, 3.2, 3.4; Alt+Tab at 3.7;
    // F5 at 6.5; the second F5 is cut; Esc at 9.0.
    p.clips[0].speeds = vec![SpeedRange {
        range: Range::new(s(3.0), s(4.0)).unwrap(),
        factor: 2.0,
    }];
    p.clips[0].cuts = vec![Range::new(s(8.8), s(9.2)).unwrap()];
    let prepared = prepare(&p, shown());
    let moments = [
        0.5, 1.0, 1.05, 1.5, 2.3, 2.6, 3.0, 3.1, 3.2, 3.45, 3.7, 4.0, 5.1, 5.3, 6.5, 8.0, 8.8, 9.5,
        10.2, 11.0,
    ];
    let frames: Vec<_> = moments
        .iter()
        .map(|&t| json!({ "t": t, "keys": text_at(&prepared, t) }))
        .collect();
    insta::assert_json_snapshot!(frames);
}

#[test]
fn nothing_is_drawn_while_off() {
    let p = project();
    let prepared = prepare(&p, KeysState::default());
    for t in [1.0, 3.5, 7.2] {
        assert_eq!(text_at(&prepared, t), None);
    }
}

#[test]
fn a_repeat_while_shown_is_counted_and_keeps_it_on_screen() {
    let p = project();
    let prepared = prepare(&p, shown());
    assert_eq!(label(&prepared, 3.1).as_deref(), Some("Ctrl+Z"));
    assert_eq!(label(&prepared, 3.5).as_deref(), Some("Ctrl+Z ×2"));
    assert_eq!(label(&prepared, 4.0).as_deref(), Some("Ctrl+Z ×3"));
    assert_eq!(label(&prepared, 4.2).as_deref(), Some("Alt+Tab"));
    // The second F5 comes 2 s after the first, which is gone after 1.5 s: shown anew.
    assert_eq!(label(&prepared, 8.6), None);
    assert_eq!(label(&prepared, 9.1).as_deref(), Some("F5"));
}

#[test]
fn another_shortcut_replaces_without_a_fade_and_the_last_one_fades_out() {
    let p = project();
    let prepared = prepare(&p, shown());
    let opacity = |t| text_at(&prepared, t).map(|t| t.opacity);
    assert_eq!(opacity(1.0), Some(0.0));
    assert_eq!(opacity(1.05), Some(0.5));
    assert_eq!(opacity(1.2), Some(1.0));
    assert_eq!(opacity(3.79), Some(1.0));
    assert_eq!(opacity(4.19), Some(1.0));
    assert_eq!(opacity(4.2), Some(1.0));
    let out = opacity(5.575).unwrap();
    assert!((out - 0.5).abs() < 1e-3, "{out}");
    assert_eq!(opacity(5.7), None);
}

#[test]
fn shortcuts_stay_inside_the_video() {
    let mut p = Project::default();
    crate::test_util::clip(&mut p, crate::test_util::RECORDING, 10.0);
    let prepared = prepare(&p, shown());
    // Esc at 9.9 s would stay to 11.4 s: it fades out by the end of the 10 s video.
    let near_end = text_at(&prepared, 9.95).unwrap().opacity;
    assert!(near_end < 0.25, "{near_end}");
    assert_eq!(text_at(&prepared, 10.0), None);
    assert_eq!(text_at(&prepared, 10.5), None);
}

#[test]
fn imported_videos_and_other_recordings_show_nothing() {
    let p = project();
    let others = KeyTracks::from([(
        "media/recording-9/video.mp4".to_owned(),
        tracks().remove(crate::test_util::RECORDING).unwrap(),
    )]);
    let prepared = Prepared::new(shown(), &others, &TimeMap::build(&p));
    assert!(prepared.shown.is_empty());
}

#[test]
fn position_puts_the_plate_in_a_corner_or_the_middle() {
    let p = project();
    let place = |position| {
        let state = with(KeysSettings {
            position,
            ..shown().settings
        });
        let text = text_at(&prepare(&p, state), 1.5).unwrap();
        (text.rect, text.align)
    };
    let (bottom, align) = place(Position::Bottom);
    assert_eq!(align, Align::Center);
    let margin = 0.04 * 1080.0;
    assert!((bottom.x - margin).abs() < 1e-3 && (bottom.w - (1920.0 - 2.0 * margin)).abs() < 1e-3);
    assert!((bottom.y + bottom.h - (1080.0 - margin)).abs() < 1e-3);
    let (top_right, align) = place(Position::TopRight);
    assert_eq!(align, Align::Right);
    assert!((top_right.y - margin).abs() < 1e-3);
    assert_eq!(place(Position::BottomLeft).1, Align::Left);
    assert_eq!(place(Position::TopLeft).0.y, top_right.y);
}

#[test]
fn size_follows_the_short_side_of_the_frame() {
    let p = project();
    let font = |size: f64, frame: PixelSize| {
        let state = with(KeysSettings {
            size,
            ..shown().settings
        });
        text_in(&prepare(&p, state), frame, 1.5).unwrap().font.size
    };
    assert_eq!(font(1.0, HD), 36.0);
    assert_eq!(font(2.0, HD), 72.0);
    let portrait = PixelSize {
        width: 720,
        height: 1280,
    };
    assert_eq!(font(1.0, portrait), 24.0);
}
