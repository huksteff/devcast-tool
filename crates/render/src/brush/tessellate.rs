//! Strokes of a drawing → triangles (lyon): the part shown by draw-in, round joins and caps.

use std::ops::Range;

use lyon::math::Point;
use lyon::path::Polygon;
use lyon::tessellation::{
    BuffersBuilder, LineCap, LineJoin, StrokeOptions, StrokeTessellator, StrokeVertex,
    TessellationError, VertexBuffers,
};

use crate::scene::{BrushStroke, Camera, Drawing, Tool};

/// Path length of a single-point stroke (a dot), px (v1: 1 unit).
const DOT: f32 = 1.0;
/// Largest distance of the triangles from the true outline, target px.
const TOLERANCE: f32 = 0.1;

/// Position in NDC and premultiplied linear colour.
pub(super) type Vertex = [f32; 6];

/// Triangles of a frame's drawings, all in one buffer.
#[derive(Default)]
pub(super) struct Mesh {
    pub(super) buffers: VertexBuffers<Vertex, u32>,
    tessellator: StrokeTessellator,
}

/// One stroke: its indices in the mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Piece {
    pub(super) indices: Range<u32>,
    pub(super) eraser: bool,
}

/// Where the frame goes: `frame`-px scene into a target of `per_target` frame px per pixel.
#[derive(Debug, Clone, Copy)]
pub(super) struct Space {
    pub(super) camera: Camera,
    pub(super) frame: [f32; 2],
    pub(super) per_target: f32,
}

impl Mesh {
    pub(super) fn clear(&mut self) {
        self.buffers.vertices.clear();
        self.buffers.indices.clear();
    }

    /// Adds the shown part of `drawing`; returns its strokes in paint order.
    pub(super) fn push(
        &mut self,
        drawing: &Drawing,
        space: Space,
    ) -> Result<Vec<Piece>, TessellationError> {
        let mut pieces = Vec::new();
        for (stroke, points) in shown(&drawing.strokes, drawing.drawn) {
            let first = self.buffers.indices.len() as u32;
            self.stroke(stroke, &points, space)?;
            let indices = first..self.buffers.indices.len() as u32;
            let eraser = stroke.tool == Tool::Eraser;
            pieces.push(Piece { indices, eraser });
        }
        Ok(pieces)
    }

    fn stroke(
        &mut self,
        stroke: &BrushStroke,
        points: &[[f32; 2]],
        space: Space,
    ) -> Result<(), TessellationError> {
        let Space {
            camera,
            frame: [fw, fh],
            per_target,
        } = space;
        let points: Vec<Point> = points
            .iter()
            .map(|p| Point::from(camera.apply(*p)))
            .collect();
        let options = StrokeOptions::default()
            .with_line_width(stroke.width * camera.scale)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round)
            .with_tolerance(TOLERANCE * per_target);
        let [r, g, b, a] = match stroke.tool {
            Tool::Pen(color) => color.linear(),
            Tool::Eraser => [0.0, 0.0, 0.0, 1.0],
        };
        let color = [r * a, g * a, b * a, a];
        let vertex = |v: StrokeVertex| {
            let p = v.position();
            let [r, g, b, a] = color;
            [p.x / fw * 2.0 - 1.0, 1.0 - p.y / fh * 2.0, r, g, b, a]
        };
        // lyon draws the round cap of a lone point only for a closed sub-path.
        let polygon = Polygon {
            points: &points,
            closed: points.len() == 1,
        };
        self.tessellator.tessellate_polygon(
            polygon,
            &options,
            &mut BuffersBuilder::new(&mut self.buffers, vertex),
        )?;
        Ok(())
    }
}

/// Strokes with their points cut to the first `drawn` part of the whole path length.
fn shown(strokes: &[BrushStroke], drawn: f32) -> Vec<(&BrushStroke, Vec<[f32; 2]>)> {
    let strokes = strokes.iter().filter(|s| !s.points.is_empty());
    let total: f32 = strokes.clone().map(|s| length(&s.points)).sum();
    let mut left = total * drawn;
    let mut shown = Vec::new();
    for stroke in strokes {
        if left <= 0.0 {
            break;
        }
        let len = length(&stroke.points);
        if left >= len {
            shown.push((stroke, stroke.points.clone()));
            left -= len;
            continue;
        }
        shown.push((stroke, cut(&stroke.points, left)));
        break;
    }
    shown
}

fn length(points: &[[f32; 2]]) -> f32 {
    match points {
        [_] => DOT,
        _ => points.windows(2).map(|w| distance(w[0], w[1])).sum(),
    }
}

/// The first `left` px of a polyline (shorter than it).
fn cut(points: &[[f32; 2]], mut left: f32) -> Vec<[f32; 2]> {
    let mut kept = vec![points[0]];
    for w in points.windows(2) {
        let [a, b] = [w[0], w[1]];
        let segment = distance(a, b);
        if left < segment {
            let t = left / segment;
            kept.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            break;
        }
        kept.push(b);
        left -= segment;
    }
    kept
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Rgba;

    fn stroke(points: &[[f32; 2]]) -> BrushStroke {
        BrushStroke {
            points: points.to_vec(),
            width: 4.0,
            tool: Tool::Pen(Rgba([255, 0, 0, 255])),
        }
    }

    fn points(strokes: &[BrushStroke], drawn: f32) -> Vec<Vec<[f32; 2]>> {
        shown(strokes, drawn).into_iter().map(|(_, p)| p).collect()
    }

    #[test]
    fn draw_in_goes_stroke_after_stroke_at_constant_speed() {
        // Lengths 100 and 70 (an L), then a dot of length 1: 171 in all.
        let strokes = [
            stroke(&[[0.0, 0.0], [100.0, 0.0]]),
            stroke(&[[0.0, 10.0], [0.0, 40.0], [40.0, 40.0]]),
            stroke(&[[5.0, 5.0]]),
        ];
        assert!(points(&strokes, 0.0).is_empty());
        assert_eq!(points(&strokes, 25.0 / 171.0)[0][1][0].round(), 25.0);
        let half = points(&strokes, 120.0 / 171.0);
        assert_eq!(half.len(), 2);
        assert_eq!(half[0], strokes[0].points);
        let corner = half[1]
            .iter()
            .map(|p| p.map(f32::round))
            .collect::<Vec<_>>();
        assert_eq!(corner, [[0.0, 10.0], [0.0, 30.0]]);
        let past_corner = points(&strokes, 140.0 / 171.0);
        let end = past_corner[1].last().map(|p| p.map(f32::round));
        assert_eq!(end, Some([10.0, 40.0]));
        assert_eq!(points(&strokes, 170.5 / 171.0)[2], [[5.0, 5.0]]);
        assert_eq!(points(&strokes, 1.0).len(), 3);
    }

    #[test]
    fn empty_strokes_take_no_length() {
        let strokes = [stroke(&[]), stroke(&[[0.0, 0.0], [10.0, 0.0]])];
        assert_eq!(points(&strokes, 0.5), [vec![[0.0, 0.0], [5.0, 0.0]]]);
    }

    #[test]
    fn a_dot_and_a_line_become_triangles_inside_their_outline() {
        let mut mesh = Mesh::default();
        let drawing = Drawing {
            strokes: vec![
                stroke(&[[50.0, 50.0]]),
                stroke(&[[10.0, 10.0], [90.0, 10.0]]),
            ],
            drawn: 1.0,
            opacity: 1.0,
        };
        let space = Space {
            camera: Camera::IDENTITY,
            frame: [100.0, 100.0],
            per_target: 1.0,
        };
        let pieces = mesh.push(&drawing, space).unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(pieces.iter().all(|p| !p.indices.is_empty() && !p.eraser));
        let px = |v: &Vertex| [(v[0] + 1.0) * 50.0, (1.0 - v[1]) * 50.0];
        let dot = mesh.buffers.indices
            [pieces[0].indices.start as usize..pieces[0].indices.end as usize]
            .iter()
            .map(|&i| px(&mesh.buffers.vertices[i as usize]));
        for [x, y] in dot {
            assert!(distance([x, y], [50.0, 50.0]) <= 2.0 + 1e-3, "{x}, {y}");
        }
        assert!(
            mesh.buffers
                .vertices
                .iter()
                .all(|v| v[2..] == [1.0, 0.0, 0.0, 1.0])
        );
    }
}
