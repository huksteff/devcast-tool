use super::*;
use crate::scene::{Align, Font, PxRect, Shadow};

const TARGET: PixelSize = PixelSize {
    width: 640,
    height: 360,
};

fn fits_layer(tile: &Tile) {
    let [l, t, r, b] = tile.area;
    let [x, y] = tile.origin;
    assert_eq!([x.fract(), y.fract()], [0.0, 0.0], "{tile:?}");
    assert!(l - x >= APRON && t - y >= APRON, "{tile:?}");
    assert!(r - x <= 640.0 - APRON && b - y <= 360.0 - APRON, "{tile:?}");
}

#[test]
fn a_small_box_is_one_tile_of_its_own_area() {
    let area = [100.5, 50.25, 300.0, 120.0];
    let tiles = tiles(area, [200.0, 85.0], 30.0, TARGET);
    assert_eq!(
        tiles,
        [Tile {
            origin: [98.0, 48.0],
            area
        }]
    );
}

#[test]
fn a_box_off_the_frame_that_never_turns_into_it_has_no_tiles() {
    let area = [2000.0, 2000.0, 2100.0, 2050.0];
    assert!(tiles(area, [2050.0, 2025.0], 45.0, TARGET).is_empty());
}

#[test]
fn a_box_off_the_frame_that_turns_into_it_is_drawn() {
    // A long box to the right of the frame, turned a quarter round its centre at the frame's
    // right edge: its left half stands in the frame.
    let area = [640.0, 170.0, 1040.0, 190.0];
    let tiles = tiles(area, [640.0, 180.0], 90.0, TARGET);
    assert_eq!(tiles.len(), 1);
    let [l, _, r, _] = tiles[0].area;
    assert_eq!(l, 640.0);
    assert!((r - (640.0 + 180.0 + APRON)).abs() < 1e-3, "{tiles:?}");
}

#[test]
fn a_box_bigger_than_the_frame_is_cut_into_tiles_that_cover_what_shows() {
    let area = [-1000.0, -1000.0, 1640.0, 1360.0];
    let centre = [320.0, 180.0];
    let tiles = tiles(area, centre, 45.0, TARGET);
    assert!(tiles.len() > 1, "{tiles:?}");
    tiles.iter().for_each(fits_layer);
    // Every frame pixel's upright point lies in exactly one tile.
    let (sin, cos) = 45f32.to_radians().sin_cos();
    for py in (0..360).step_by(7) {
        for px in (0..640).step_by(7) {
            let [dx, dy] = [px as f32 + 0.5 - centre[0], py as f32 + 0.5 - centre[1]];
            let u = [
                centre[0] + cos * dx + sin * dy,
                centre[1] + cos * dy - sin * dx,
            ];
            let hits = tiles
                .iter()
                .filter(|t| {
                    let [l, t, r, b] = t.area;
                    u[0] >= l && u[0] < r && u[1] >= t && u[1] < b
                })
                .count();
            assert_eq!(hits, 1, "pixel ({px}, {py}) upright at {u:?}");
        }
    }
}

#[test]
fn bounds_grow_the_box_by_the_shadow_and_follow_the_camera() {
    let mut text = Text {
        rect: PxRect {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 40.0,
        },
        runs: Vec::new(),
        font: Font {
            family: "Inter".into(),
            size: 20.0,
            bold: false,
            italic: false,
        },
        underline: false,
        align: Align::Center,
        line_height: 1.4,
        padding: 0.0,
        plate: None,
        opacity: 1.0,
        beside: None,
        shadow: None,
        rotation: 10.0,
    };
    let camera = Camera {
        scale: 2.0,
        x: -5.0,
        y: 0.0,
    };
    let (area, centre) = bounds(&text, camera, 0.5);
    assert_eq!(
        area,
        [7.5 - APRON, 20.0 - APRON, 107.5 + APRON, 60.0 + APRON]
    );
    assert_eq!(centre, [57.5, 40.0]);
    text.shadow = Some(Shadow {
        color: crate::scene::Rgba([0, 0, 0, 255]),
        sigma: 2.0,
        offset: [0.0, 4.0],
    });
    let (area, _) = bounds(&text, camera, 0.5);
    assert_eq!(area[0], 7.5 - 10.0 - APRON);
    assert!(turned(&text));
    text.rotation = -720.0;
    assert!(!turned(&text));
}
