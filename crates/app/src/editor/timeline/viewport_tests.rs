use dc_core::{OutputTime, TimeDelta};

use super::viewport::{Lanes, ROWS, Viewport};

/// 1000 points of lanes from x 148 for a 100 s project at 30 fps: 10 points per second whole,
/// 750 at the finest.
fn lanes() -> Lanes {
    Lanes {
        left: 148.0,
        width: 1000.0,
        duration: TimeDelta::from_secs_f64(100.0),
        fps: 30.0,
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn whole_project_until_zoomed_and_it_follows_the_width() {
    let mut lanes = lanes();
    let view = Viewport::default();
    assert_eq!(lanes.limits(), (10.0, 750.0));
    assert!(close(view.px_per_sec(&lanes), 10.0));
    assert_eq!(
        view.scale(&lanes).x(OutputTime::from_secs_f64(100.0)),
        1148.0
    );
    lanes.width = 500.0;
    assert!(
        close(view.px_per_sec(&lanes), 5.0),
        "still the whole project"
    );
}

#[test]
fn zoom_keeps_the_time_under_the_pointer() {
    let lanes = lanes();
    let mut view = Viewport::default();
    let x = 448.0;
    let before = view.scale(&lanes).time(x);
    view.zoom_by(&lanes, 4.0, x);
    assert!(close(view.px_per_sec(&lanes), 40.0));
    let after = view.scale(&lanes).time(x);
    assert!(
        (after.as_secs_f64() - before.as_secs_f64()).abs() < 1e-3,
        "{after:?}"
    );
    assert!(close(view.start(&lanes), 30.0 - 300.0 / 40.0));
    view.zoom_by(&lanes, 3.0, 1000.0);
    let t = view.scale(&lanes).time(1000.0).as_secs_f64();
    let again = view.scale(&lanes);
    assert!((f64::from(again.x(OutputTime::from_secs_f64(t))) - 1000.0).abs() < 1e-2);
}

#[test]
fn zoom_stops_at_its_limits_and_the_whole_project_fits_again() {
    let lanes = lanes();
    let mut view = Viewport::default();
    view.zoom_by(&lanes, 1e6, 600.0);
    assert!(
        close(view.px_per_sec(&lanes), 750.0),
        "a few frames per 100 points"
    );
    view.zoom_by(&lanes, 1e-6, 600.0);
    assert_eq!(view, Viewport::default(), "zoomed out to the whole project");
    view.zoom_by(&lanes, 5.0, 148.0);
    view.fit();
    assert_eq!(view.start(&lanes), 0.0);
    assert!(close(view.px_per_sec(&lanes), 10.0));
}

#[test]
fn a_short_project_cannot_zoom_out_past_its_length() {
    let lanes = Lanes {
        duration: TimeDelta::from_secs_f64(0.1),
        ..lanes()
    };
    let view = Viewport::default();
    assert_eq!(lanes.limits(), (10_000.0, 10_000.0));
    assert_eq!(view.slider(&lanes), 0.0);
    let empty = Lanes {
        duration: TimeDelta::ZERO,
        ..lanes
    };
    assert!(
        close(view.px_per_sec(&empty), 1000.0),
        "an empty project shows a second"
    );
}

#[test]
fn slider_is_logarithmic_and_keeps_the_anchor() {
    let lanes = lanes();
    let mut view = Viewport::default();
    view.set_slider(&lanes, 0.5, 148.0);
    assert!(close(view.px_per_sec(&lanes), (10.0f64 * 750.0).sqrt()));
    assert!(close(view.slider(&lanes), 0.5));
    assert_eq!(view.start(&lanes), 0.0, "the left edge stays at 0");
    view.set_slider(&lanes, 1.0, 648.0);
    assert!(close(view.px_per_sec(&lanes), 750.0));
    view.set_slider(&lanes, 0.0, 648.0);
    assert_eq!(view, Viewport::default());
}

#[test]
fn scroll_stays_inside_the_project() {
    let lanes = lanes();
    let mut view = Viewport::default();
    view.scroll(&lanes, -200.0);
    assert_eq!(view.start(&lanes), 0.0, "the whole project does not scroll");
    view.zoom_by(&lanes, 10.0, 148.0);
    assert!(close(view.visible(&lanes), 10.0));
    view.scroll(&lanes, -200.0);
    assert!(close(view.start(&lanes), 2.0), "wheel down: later time");
    view.scroll(&lanes, 1e6);
    assert_eq!(view.start(&lanes), 0.0);
    view.scroll(&lanes, -1e6);
    assert!(
        close(view.start(&lanes), 90.0),
        "the end stays at the right edge"
    );
}

#[test]
fn navigation_thumb_shows_and_moves_the_visible_span() {
    let lanes = lanes();
    let mut view = Viewport::default();
    assert_eq!(view.thumb(&lanes), (0.0, 1.0));
    view.zoom_by(&lanes, 4.0, 148.0);
    assert_eq!(view.thumb(&lanes), (0.0, 0.25));
    view.centre_at(&lanes, 0.5);
    assert!(close(view.thumb(&lanes).0, 0.375));
    view.shift_from(&lanes, 37.5, 0.1);
    assert!(close(view.start(&lanes), 47.5));
    view.shift_from(&lanes, 47.5, 1.0);
    assert!(
        close(view.start(&lanes), 75.0),
        "the thumb stops at the end"
    );
}

#[test]
fn edge_scroll_speeds_up_towards_the_edge() {
    let lanes = lanes();
    let mut view = Viewport::default();
    assert!(!view.edge_scroll(&lanes, 1147.0, 0.1), "nothing to scroll");
    view.zoom_by(&lanes, 10.0, 148.0);
    assert!(!view.edge_scroll(&lanes, 600.0, 0.1), "middle of the lanes");
    assert!(view.edge_scroll(&lanes, 1148.0 - 16.0, 0.1));
    assert!(close(view.start(&lanes), 0.5 * 10.0 * 1.08 * 0.1));
    let start = view.start(&lanes);
    assert!(
        view.edge_scroll(&lanes, 2000.0, 0.1),
        "past the edge: full speed"
    );
    assert!(close(view.start(&lanes) - start, 1.08));
    view.edge_scroll(&lanes, 0.0, 10.0);
    assert_eq!(view.start(&lanes), 0.0);
}

#[test]
fn a_playhead_that_leaves_the_view_pages_it() {
    let lanes = lanes();
    let mut view = Viewport::default();
    view.zoom_by(&lanes, 10.0, 148.0);
    view.follow(&lanes, OutputTime::from_secs_f64(5.0));
    assert_eq!(view.start(&lanes), 0.0, "in view: stays");
    view.follow(&lanes, OutputTime::from_secs_f64(10.5));
    assert!(
        close(view.start(&lanes), 10.5),
        "the next page starts at the playhead"
    );
    view.follow(&lanes, OutputTime::from_secs_f64(2.0));
    assert!(close(view.start(&lanes), 2.0));
    view.scroll(&lanes, -400.0);
    view.follow(&lanes, OutputTime::from_secs_f64(2.0));
    assert!(
        close(view.start(&lanes), 6.0),
        "the user scrolled away: it did not move"
    );
    view.scrub = Some(false);
    view.follow(&lanes, OutputTime::from_secs_f64(50.0));
    assert!(
        close(view.start(&lanes), 6.0),
        "dragged: the auto-scroll moves the view"
    );
}

#[test]
fn rows_scale_in_notches_within_limits() {
    let mut view = Viewport::default();
    assert_eq!(view.rows(1.5), 1.5, "from the settings");
    assert_eq!(view.rows(1.0), 1.5, "then its own");
    view.wheel_rows(1.0, 40.0);
    assert!((view.rows(1.0) - 1.6).abs() < 1e-6);
    for _ in 0..4 {
        view.wheel_rows(1.0, -10.0);
    }
    assert!(
        (view.rows(1.0) - 1.5).abs() < 1e-6,
        "smoothed parts of a notch add up"
    );
    view.wheel_rows(1.0, 1e5);
    assert_eq!(view.rows(1.0), *ROWS.end());
    view.wheel_rows(1.0, -1e5);
    assert_eq!(view.rows(1.0), *ROWS.start());
    assert_eq!(
        Viewport::default().rows(9.0),
        3.0,
        "a broken setting is clamped"
    );
}
