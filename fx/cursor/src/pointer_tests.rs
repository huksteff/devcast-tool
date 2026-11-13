use dc_core::{Project, TimeMap};

use super::pointer;
use crate::prepared::Prepared;
use crate::state::{CursorSettings, Smoothing};
use crate::test_util::{HD, o, project, state, tracks};

fn at(p: &Project, settings: CursorSettings, secs: f64) -> Option<dc_render::look::Pointer> {
    let prepared = Prepared::new(p, state(settings), &tracks());
    pointer(&prepared, p, &TimeMap::build(p), o(secs), HD)
}

fn shown(smoothing: Smoothing) -> CursorSettings {
    CursorSettings {
        smoothing,
        sway: 0.0,
        ..CursorSettings::default()
    }
}

#[test]
fn pointer_is_the_drawn_cursor_or_the_recorded_point() {
    let p = project();
    // At rest at (0.2, 0.3) of the 1920×1080 frame, before the sweep.
    let rest = at(&p, shown(Smoothing::Focused), 0.3).unwrap();
    assert!(
        (rest.tip[0] - 384.0).abs() < 0.5 && (rest.tip[1] - 324.0).abs() < 0.5,
        "{rest:?}"
    );
    assert!((rest.k - 1.0).abs() < 1e-9);
    assert!(rest.arrow > 0.0);
    // Mid-sweep the smoothed cursor lags behind the recorded pointer; hidden, the raw one counts.
    let smoothed = at(&p, shown(Smoothing::Smooth), 0.8).unwrap();
    let hidden = CursorSettings {
        show: false,
        ..shown(Smoothing::Smooth)
    };
    let raw = at(&p, hidden, 0.8).unwrap();
    assert!(
        raw.tip[0] > smoothed.tip[0] + 1.0,
        "{raw:?} vs {smoothed:?}"
    );
}

#[test]
fn hidden_picture_keeps_the_point_and_the_crop_drops_it() {
    let p = project();
    // The system hid the cursor at 3.3–3.5 s: the callout still has its point.
    assert!(at(&p, shown(Smoothing::Exact), 3.4).is_some());
    // At 3.6 s the pointer leaves the picture: outside the crop.
    assert!(at(&p, shown(Smoothing::Exact), 3.7).is_none());
}
