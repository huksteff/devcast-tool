use super::*;

const AREA: Area = Area {
    w: 2560.0,
    h: 1440.0,
};

fn frame(x: f64, y: f64, w: f64, h: f64) -> Frame {
    Frame { x, y, w, h }
}

fn inside(f: Frame) {
    assert!(f.x >= 0.0 && f.y >= 0.0, "{f:?}");
    assert!(
        f.x + f.w <= AREA.w + 1e-9 && f.y + f.h <= AREA.h + 1e-9,
        "{f:?}"
    );
    assert!(f.w >= 128.0 - 1e-9 && f.h >= 72.0 - 1e-9, "{f:?}");
}

#[test]
fn moving_keeps_the_size_inside_the_source() {
    let start = frame(100.0, 100.0, 1000.0, 500.0);
    let moved = start.dragged(Handle::Move, (50.0, -20.0), AREA, None);
    assert_eq!(moved, frame(150.0, 80.0, 1000.0, 500.0));
    let far = start.dragged(Handle::Move, (5000.0, -5000.0), AREA, None);
    assert_eq!(far, frame(1560.0, 0.0, 1000.0, 500.0));
}

#[test]
fn an_edge_moves_alone_and_stops_at_the_source_and_the_least_size() {
    let start = frame(100.0, 100.0, 1000.0, 500.0);
    let right = start.dragged(Handle::Right, (200.0, 70.0), AREA, None);
    assert_eq!(right, frame(100.0, 100.0, 1200.0, 500.0));
    let left = start.dragged(Handle::Left, (-500.0, 0.0), AREA, None);
    assert_eq!(left, frame(0.0, 100.0, 1100.0, 500.0));
    let collapsed = start.dragged(Handle::Left, (5000.0, 0.0), AREA, None);
    assert_eq!(collapsed, frame(972.0, 100.0, 128.0, 500.0));
    let corner = start.dragged(Handle::BottomRight, (9999.0, 9999.0), AREA, None);
    assert_eq!(corner, frame(100.0, 100.0, 2460.0, 1340.0));
    let top = start.dragged(Handle::TopLeft, (10.0, 1000.0), AREA, None);
    assert_eq!(top, frame(110.0, 528.0, 990.0, 72.0));
}

#[test]
fn a_kept_ratio_holds_the_opposite_corner() {
    let start = frame(100.0, 100.0, 1600.0, 900.0);
    let ratio = Some(16.0 / 9.0);
    let grown = start.dragged(Handle::BottomRight, (160.0, 0.0), AREA, ratio);
    near(grown, frame(100.0, 100.0, 1760.0, 990.0));
    let shrunk = start.dragged(Handle::TopLeft, (800.0, 0.0), AREA, ratio);
    near(shrunk, frame(900.0, 550.0, 800.0, 450.0));
    // The source bottom stops it: 1340 px of room down is 2382 px wide.
    let big = start.dragged(Handle::BottomRight, (5000.0, 5000.0), AREA, ratio);
    inside(big);
    assert!((big.ratio() - 16.0 / 9.0).abs() < 1e-9);
    assert!((big.y + big.h - AREA.h).abs() < 1e-9, "{big:?}");
    let least = start.dragged(Handle::BottomRight, (-5000.0, -5000.0), AREA, ratio);
    inside(least);
    assert!((least.w - 128.0).abs() < 1e-9, "{least:?}");
}

#[test]
fn a_kept_ratio_on_an_edge_grows_around_the_middle() {
    let start = frame(1000.0, 600.0, 400.0, 225.0);
    let ratio = Some(16.0 / 9.0);
    let wide = start.dragged(Handle::Right, (400.0, 0.0), AREA, ratio);
    near(wide, frame(1000.0, 487.5, 800.0, 450.0));
    let tall = start.dragged(Handle::Top, (0.0, -9999.0), AREA, ratio);
    inside(tall);
    assert!((tall.ratio() - 16.0 / 9.0).abs() < 1e-9);
    assert!((tall.y + tall.h - 825.0).abs() < 1e-9, "the bottom stays");
}

#[test]
fn a_frame_of_another_shape_takes_the_ratio_on_the_first_drag() {
    let start = frame(0.0, 0.0, 2560.0, 1440.0);
    let square = start.dragged(Handle::BottomRight, (0.0, 0.0), AREA, Some(1.0));
    near(square, frame(0.0, 0.0, 1440.0, 1440.0));
}

#[test]
fn a_fixed_format_fits_inside_the_frame_around_its_middle() {
    let full = Frame::full(AREA);
    let four_three = full.fitted(4.0 / 3.0, AREA);
    near(four_three, frame(320.0, 0.0, 1920.0, 1440.0));
    let wide = frame(0.0, 0.0, 1000.0, 1000.0).fitted(16.0 / 9.0, AREA);
    near(wide, frame(0.0, 218.75, 1000.0, 562.5));
}

#[test]
fn fields_clamp_and_follow_the_ratio() {
    let start = frame(100.0, 100.0, 1000.0, 500.0);
    assert_eq!(
        start.with(Field::X, 9999.0, AREA, None),
        frame(1560.0, 100.0, 1000.0, 500.0)
    );
    assert_eq!(
        start.with(Field::Width, 10.0, AREA, None),
        frame(100.0, 100.0, 128.0, 500.0)
    );
    assert_eq!(
        start.with(Field::Height, 900.0, AREA, Some(2.0)),
        frame(100.0, 100.0, 1800.0, 900.0)
    );
    let capped = start.with(Field::Width, 9999.0, AREA, Some(1.0));
    assert_eq!(capped, frame(100.0, 100.0, 1340.0, 1340.0));
    assert_eq!(start.value(Field::Height), 500.0);
}

#[test]
fn the_rect_is_a_valid_crop_at_whole_pixels() {
    assert_eq!(Frame::full(AREA).rect(AREA), Rect::FULL);
    let rect = frame(256.0, 144.0, 2048.0, 1152.0).rect(AREA);
    assert_eq!(rect, Rect::new(0.1, 0.1, 0.8, 0.8).unwrap());
    let rounded = frame(255.6, 0.4, 2304.4, 1439.6).rect(AREA);
    assert_eq!(rounded, Rect::new(0.1, 0.0, 0.9, 1.0).unwrap());
    assert_eq!(dc_core::project::crop_problem(rounded), None);
    // 5 % of 1366 px is 68.3 px: the least side rounds up, not under `CROP_MIN`.
    let odd = Area {
        w: 1366.0,
        h: 768.0,
    };
    let least = frame(1366.0 - 68.3, 0.0, 68.3, 768.0).rect(odd);
    assert_eq!(dc_core::project::crop_problem(least), None, "{least:?}");
    for x in 0..1366 {
        let edge = frame(f64::from(x), 0.0, 1366.0 - f64::from(x), 768.0).rect(odd);
        if 1366 - x >= 69 {
            assert_eq!(dc_core::project::crop_problem(edge), None, "{edge:?}");
        }
    }
}

#[test]
fn handles_sit_on_corners_and_middles() {
    assert_eq!(Handle::TopLeft.anchor(), (0.0, 0.0));
    assert_eq!(Handle::Right.anchor(), (1.0, 0.5));
    assert_eq!(Handle::Bottom.anchor(), (0.5, 1.0));
}

/// `got` is `want` within float error.
fn near(got: Frame, want: Frame) {
    let close = [
        (got.x, want.x),
        (got.y, want.y),
        (got.w, want.w),
        (got.h, want.h),
    ]
    .iter()
    .all(|(a, b)| (a - b).abs() < 1e-9);
    assert!(close, "{got:?} != {want:?}");
}
