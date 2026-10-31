//! The math of editing on the preview: screen ↔ canvas through the frame and the camera, hit
//! tests of a turned box, resizing with its constraints, turning, snapping.

use dc_core::{PixelSize, Rect as Fractions};
use dc_render::Camera;
use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};

use super::geom::{Boxed, Grip, Handle, Mapping, Resize};
use super::snap::{Lines, axes};

const CANVAS: Vec2 = vec2(2560.0, 1440.0);
const LEAST: Vec2 = vec2(25.6, 14.4);

fn close(a: Pos2, b: Pos2) -> bool {
    a.distance(b) < 0.01
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// A 400 × 200 box centred at (1000, 500), turned `angle`.
fn boxed(angle: f32) -> Boxed {
    Boxed {
        centre: pos2(1000.0, 500.0),
        size: vec2(400.0, 200.0),
        angle,
    }
}

#[test]
fn the_screen_is_the_canvas_scaled_and_shifted_by_the_camera() {
    let frame = Rect::from_min_size(pos2(100.0, 50.0), vec2(640.0, 360.0));
    let size = PixelSize {
        width: 2560,
        height: 1440,
    };
    let still = Mapping::new(frame, size, Camera::IDENTITY);
    assert!(close(still.screen(pos2(1280.0, 720.0)), frame.center()));
    assert!(close(still.canvas(frame.max), pos2(2560.0, 1440.0)));
    assert!(
        near(still.length(6.0), 24.0),
        "6 points are 24 px at a quarter"
    );
    // Zoomed 2× into the centre: the centre stays, the corner goes out of the frame.
    let camera = Camera {
        scale: 2.0,
        x: -1280.0,
        y: -720.0,
    };
    let zoomed = Mapping::new(frame, size, camera);
    assert!(close(zoomed.screen(pos2(1280.0, 720.0)), frame.center()));
    assert!(close(
        zoomed.screen(Pos2::ZERO),
        pos2(100.0 - 320.0, 50.0 - 180.0)
    ));
    let back = zoomed.canvas(zoomed.screen(pos2(300.0, 900.0)));
    assert!(close(back, pos2(300.0, 900.0)));
    assert!(near(zoomed.length(6.0), 12.0));
}

#[test]
fn fractions_go_both_ways() {
    let rect = Fractions::new(0.1, 0.2, 0.3, 0.25).unwrap();
    let b = Boxed::of(rect, 30.0, CANVAS);
    assert!(close(b.centre, pos2(0.25 * 2560.0, 0.325 * 1440.0)));
    assert_eq!(b.fractions(CANVAS), [0.1, 0.2, 0.3, 0.25]);
    assert_eq!(b.rotation(), 30.0);
}

#[test]
fn a_turned_box_is_hit_inside_its_turned_shape() {
    let upright = boxed(0.0);
    assert!(upright.contains(pos2(1190.0, 590.0)));
    assert!(!upright.contains(pos2(1000.0, 650.0)));
    let turned = boxed(90.0);
    // A quarter turn: 200 wide, 400 tall.
    assert!(turned.contains(pos2(1000.0, 680.0)));
    assert!(!turned.contains(pos2(1190.0, 500.0)));
    let tilted = boxed(45.0);
    assert!(
        !tilted.contains(pos2(1180.0, 410.0)),
        "the old corner is out"
    );
    assert!(tilted.contains(pos2(1100.0, 600.0)));
    let b = tilted.bounds();
    let reach = (200.0_f32 + 100.0) / 2.0_f32.sqrt();
    assert!(
        near(b.width() / 2.0, reach) && near(b.height() / 2.0, reach),
        "{b:?}"
    );
}

#[test]
fn handles_and_the_knob_turn_with_the_box() {
    let b = boxed(90.0);
    // Turned clockwise: the right edge looks down, the top edge right.
    assert!(close(b.handle(Handle::Right), pos2(1000.0, 700.0)));
    assert!(close(b.handle(Handle::Top), pos2(1100.0, 500.0)));
    assert!(close(b.knob(20.0), pos2(1120.0, 500.0)));
    assert_eq!(b.grip(pos2(1118.0, 501.0), 4.0, 20.0), Some(Grip::Turn));
    assert_eq!(
        b.grip(pos2(1000.0, 703.0), 4.0, 20.0),
        Some(Grip::Handle(Handle::Right))
    );
    assert_eq!(b.grip(pos2(1000.0, 600.0), 4.0, 20.0), Some(Grip::Move));
    assert_eq!(b.grip(pos2(1150.0, 500.0), 4.0, 20.0), None);
}

#[test]
fn a_handle_resizes_against_the_opposite_side() {
    let b = boxed(0.0);
    let plain = Resize::default();
    let wider = b.resized(Handle::Right, pos2(1300.0, 999.0), plain, LEAST);
    assert_eq!(wider.size, vec2(500.0, 200.0), "the edge moves one axis");
    assert!(close(wider.handle(Handle::Left), b.handle(Handle::Left)));
    let corner = b.resized(Handle::TopLeft, pos2(700.0, 300.0), plain, LEAST);
    assert_eq!(corner.size, vec2(500.0, 300.0));
    assert!(close(
        corner.handle(Handle::BottomRight),
        b.handle(Handle::BottomRight)
    ));
    let past = b.resized(Handle::Right, pos2(0.0, 500.0), plain, LEAST);
    assert!(
        near(past.size.x, LEAST.x),
        "never past the far edge: {past:?}"
    );
    assert!(close(past.handle(Handle::Left), b.handle(Handle::Left)));
}

#[test]
fn shift_keeps_the_proportion_and_alt_the_centre() {
    let b = boxed(0.0);
    let aspect = Resize {
        keep_aspect: true,
        from_centre: false,
    };
    let corner = b.resized(Handle::BottomRight, pos2(1400.0, 620.0), aspect, LEAST);
    assert_eq!(corner.size, vec2(600.0, 300.0), "the larger change wins");
    assert!(close(
        corner.handle(Handle::TopLeft),
        b.handle(Handle::TopLeft)
    ));
    let edge = b.resized(Handle::Bottom, pos2(1000.0, 700.0), aspect, LEAST);
    assert_eq!(edge.size, vec2(600.0, 300.0));
    assert!(
        near(edge.centre.x, 1000.0),
        "the other side grows both ways"
    );
    let centred = Resize {
        keep_aspect: false,
        from_centre: true,
    };
    let both = b.resized(Handle::Right, pos2(1300.0, 500.0), centred, LEAST);
    assert_eq!((both.size, both.centre), (vec2(600.0, 200.0), b.centre));
}

#[test]
fn a_turned_box_resizes_along_its_own_axes() {
    let b = boxed(90.0);
    // Its right edge looks down: pulling it 100 lower adds 100 to its width.
    let longer = b.resized(Handle::Right, pos2(1000.0, 800.0), Resize::default(), LEAST);
    assert!(
        near(longer.size.x, 500.0) && near(longer.size.y, 200.0),
        "{longer:?}"
    );
    assert!(close(longer.handle(Handle::Left), b.handle(Handle::Left)));
    assert_eq!(longer.angle, 90.0);
}

#[test]
fn the_knob_turns_by_the_pointer_and_shift_snaps_to_15() {
    let b = boxed(0.0);
    let knob = b.knob(20.0);
    // From straight up to straight right: a quarter clockwise.
    assert!(near(b.turned(knob, pos2(1300.0, 500.0), false), 90.0));
    let a = b.turned(knob, pos2(1100.0, 300.0), false);
    assert!(near(a, 26.565), "{a}");
    assert!(near(b.turned(knob, pos2(1100.0, 300.0), true), 30.0));
    let back = boxed(350.0).turned(knob, pos2(1100.0, 300.0), false);
    assert!(back <= 360.0 && near(back, 376.565 - 360.0), "{back}");
}

#[test]
fn moving_sticks_to_the_frame_centre_and_edges() {
    let lines = Lines::new(CANVAS, &[]);
    // Left edge 4 px from the frame's left.
    let near_edge = Rect::from_min_size(pos2(4.0, 300.0), vec2(400.0, 200.0));
    let (shift, guides) = lines.moved(near_edge, 6.0);
    assert_eq!(shift, vec2(-4.0, 0.0));
    assert_eq!(guides.len(), 1);
    assert!(guides[0].vertical && guides[0].frame && guides[0].at == 0.0);
    // Centre 3 px from the frame's centre on both axes.
    let middle = Rect::from_center_size(pos2(1283.0, 717.0), vec2(400.0, 200.0));
    let (shift, guides) = lines.moved(middle, 6.0);
    assert_eq!(shift, vec2(-3.0, 3.0));
    assert_eq!(guides.len(), 2);
    let far = Rect::from_min_size(pos2(20.0, 300.0), vec2(400.0, 200.0));
    assert_eq!(
        lines.moved(far, 6.0),
        (Vec2::ZERO, Vec::new()),
        "beyond the pull"
    );
}

#[test]
fn moving_sticks_to_other_annotations() {
    let other = Rect::from_min_size(pos2(1000.0, 100.0), vec2(300.0, 100.0));
    let lines = Lines::new(CANVAS, &[other]);
    // Its left edge 5 px right of the other's right edge; the nearer line wins.
    let beside = Rect::from_min_size(pos2(1305.0, 400.0), vec2(100.0, 100.0));
    let (shift, guides) = lines.moved(beside, 6.0);
    assert_eq!(shift.x, -5.0);
    assert!(
        guides
            .iter()
            .any(|g| g.vertical && !g.frame && g.at == 1300.0)
    );
}

#[test]
fn a_handle_sticks_on_the_axes_it_moves() {
    let lines = Lines::new(CANVAS, &[]);
    let (p, guides) = lines.pointer(pos2(2556.0, 718.0), vec2(1.0, 0.0), 6.0);
    assert_eq!(p, pos2(2560.0, 718.0), "y does not move with a side handle");
    assert_eq!(guides.len(), 1);
    assert_eq!(axes(0.0, vec2(1.0, 0.0)), Some(vec2(1.0, 0.0)));
    assert_eq!(axes(90.0, vec2(1.0, 0.0)), Some(vec2(0.0, 1.0)));
    assert_eq!(axes(-180.0, vec2(1.0, -1.0)), Some(vec2(1.0, -1.0)));
    assert_eq!(
        axes(30.0, vec2(1.0, 0.0)),
        None,
        "a tilted box does not snap"
    );
}
