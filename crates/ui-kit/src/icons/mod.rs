//! Line icons of the mockup drawn with shapes (decision 005: no icon fonts, no system symbols).
//! Coordinates are the mockup's SVG 24 × 24 view box; arcs are polylines, rounded joins of the
//! SVG are plain corners.

mod editor;

use std::f32::consts::PI;

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};

/// Stroke width in view-box units (`stroke-width: 1.8`).
const STROKE: f32 = 1.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Monitor,
    Window,
    Area,
    Mic,
    MicOff,
    Speaker,
    Timer,
    More,
    Down,
    Pause,
    Play,
    Close,
    Hide,
    EyeOff,
    Gear,
    Folder,
    Check,
    Reset,
    Warn,
    Project,
    Quit,
    Search,
    Grip,
    Plus,
    Minus,
    Right,
    /// A filled dot: the record mark of red buttons.
    Record,
    /// The recordings library (four tiles).
    Library,
    Undo,
    Redo,
    Help,
    Export,
    Crop,
    /// The «Выбор» tool: an arrow pointer.
    Pointer,
    /// Scissors: the «Лезвие» tool.
    Blade,
    /// A keyframe.
    Diamond,
    Eye,
    SkipBack,
    SkipForward,
    Left,
    Magnet,
    ZoomIn,
    Wand,
    /// Captions («CC»).
    Captions,
    Keyboard,
    Text,
    /// The zoom frame with its corner ticks.
    Frame,
    /// The mouse cursor of the cursor panel.
    Cursor,
    Fit,
    /// Out into its own window.
    Detach,
    Mute,
    /// Delete the object of a panel.
    Trash,
    /// Duplicate the object of a panel.
    Copy,
}

impl Icon {
    pub const ALL: [Self; 53] = [
        Self::Monitor,
        Self::Window,
        Self::Area,
        Self::Mic,
        Self::MicOff,
        Self::Speaker,
        Self::Timer,
        Self::More,
        Self::Down,
        Self::Pause,
        Self::Play,
        Self::Close,
        Self::Hide,
        Self::EyeOff,
        Self::Gear,
        Self::Folder,
        Self::Check,
        Self::Reset,
        Self::Warn,
        Self::Project,
        Self::Quit,
        Self::Search,
        Self::Grip,
        Self::Plus,
        Self::Minus,
        Self::Right,
        Self::Record,
        Self::Library,
        Self::Undo,
        Self::Redo,
        Self::Help,
        Self::Export,
        Self::Crop,
        Self::Pointer,
        Self::Blade,
        Self::Diamond,
        Self::Eye,
        Self::SkipBack,
        Self::SkipForward,
        Self::Left,
        Self::Magnet,
        Self::ZoomIn,
        Self::Wand,
        Self::Captions,
        Self::Keyboard,
        Self::Text,
        Self::Frame,
        Self::Cursor,
        Self::Fit,
        Self::Detach,
        Self::Mute,
        Self::Trash,
        Self::Copy,
    ];

    /// Draws the icon into `rect` (usually 16 × 16) in `color`.
    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        let pen = Pen {
            painter,
            rect,
            stroke: Stroke::new(STROKE * rect.width() / 24.0, color),
        };
        pen.draw(self);
    }
}

/// Maps view-box units into the target rect.
struct Pen<'a> {
    painter: &'a Painter,
    rect: Rect,
    stroke: Stroke,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        let scale = self.rect.width() / 24.0;
        self.rect.min + egui::vec2(x, y) * scale
    }

    fn line(&self, points: &[(f32, f32)]) {
        let points = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter.add(Shape::line(points, self.stroke));
    }

    fn closed(&self, points: &[(f32, f32)]) {
        let points = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter.add(Shape::closed_line(points, self.stroke));
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let scale = self.rect.width() / 24.0;
        let rect = Rect::from_min_max(self.at(x, y), self.at(x + w, y + h));
        self.painter
            .rect_stroke(rect, r * scale, self.stroke, egui::StrokeKind::Middle);
    }

    fn circle(&self, x: f32, y: f32, r: f32) {
        let scale = self.rect.width() / 24.0;
        self.painter
            .circle_stroke(self.at(x, y), r * scale, self.stroke);
    }

    fn dot(&self, x: f32, y: f32, r: f32) {
        let scale = self.rect.width() / 24.0;
        self.painter
            .circle_filled(self.at(x, y), r * scale, self.stroke.color);
    }

    /// Arc of the circle (`x`, `y`, `r`) from `from` to `to` degrees, clockwise on screen.
    fn arc(&self, (x, y): (f32, f32), r: f32, from: f32, to: f32) {
        let steps = ((to - from).abs() / 10.0).ceil().max(2.0) as usize;
        let points: Vec<(f32, f32)> = (0..=steps)
            .map(|i| {
                let angle = (from + (to - from) * i as f32 / steps as f32) * PI / 180.0;
                (x + r * angle.cos(), y + r * angle.sin())
            })
            .collect();
        self.line(&points);
    }

    fn draw(&self, icon: Icon) {
        match icon {
            Icon::Monitor => self.monitor(),
            Icon::Window => {
                self.rect(3.0, 4.0, 18.0, 16.0, 2.0);
                self.line(&[(3.0, 9.0), (21.0, 9.0)]);
            }
            Icon::Area => self.area(),
            Icon::Mic => self.mic(),
            Icon::MicOff => {
                self.mic();
                self.line(&[(3.0, 3.0), (21.0, 21.0)]);
            }
            Icon::Speaker => self.speaker(),
            Icon::Timer => self.timer(),
            Icon::More => {
                for y in [5.0, 12.0, 19.0] {
                    self.dot(12.0, y, 1.9);
                }
            }
            Icon::Down => self.line(&[(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)]),
            Icon::Pause => {
                self.line(&[(9.0, 5.0), (9.0, 19.0)]);
                self.line(&[(15.0, 5.0), (15.0, 19.0)]);
            }
            Icon::Play => self.closed(&[(7.0, 4.0), (20.0, 12.0), (7.0, 20.0)]),
            Icon::Close => {
                self.line(&[(6.0, 6.0), (18.0, 18.0)]);
                self.line(&[(18.0, 6.0), (6.0, 18.0)]);
            }
            Icon::Hide => self.line(&[(5.0, 12.0), (19.0, 12.0)]),
            Icon::EyeOff => self.eye_off(),
            Icon::Gear => self.gear(),
            Icon::Folder => self.folder(),
            Icon::Check => self.line(&[(5.0, 12.0), (10.0, 17.0), (19.0, 7.0)]),
            Icon::Reset => {
                self.arc((12.0, 12.0), 9.0, -131.8, 180.0);
                self.line(&[(3.0, 3.0), (3.0, 8.0), (8.0, 8.0)]);
            }
            Icon::Warn => self.warn(),
            Icon::Project => {
                self.rect(4.0, 5.0, 16.0, 14.0, 0.0);
                self.line(&[(4.0, 9.0), (20.0, 9.0)]);
            }
            Icon::Quit => self.quit(),
            Icon::Search => {
                self.circle(11.0, 11.0, 6.0);
                self.line(&[(20.0, 20.0), (15.5, 15.5)]);
            }
            Icon::Grip => self.grip(),
            Icon::Plus => {
                self.line(&[(12.0, 5.0), (12.0, 19.0)]);
                self.line(&[(5.0, 12.0), (19.0, 12.0)]);
            }
            Icon::Minus => self.line(&[(5.0, 12.0), (19.0, 12.0)]),
            Icon::Right => self.line(&[(9.0, 6.0), (15.0, 12.0), (9.0, 18.0)]),
            Icon::Record => self.dot(12.0, 12.0, 8.0),
            editor => self.editor(editor),
        }
    }

    fn monitor(&self) {
        self.rect(3.0, 4.0, 18.0, 12.0, 2.0);
        self.line(&[(8.0, 20.0), (16.0, 20.0)]);
        self.line(&[(12.0, 16.0), (12.0, 20.0)]);
    }

    fn area(&self) {
        self.line(&[(4.0, 8.0), (4.0, 5.0), (5.0, 4.0), (8.0, 4.0)]);
        self.line(&[(16.0, 4.0), (19.0, 4.0), (20.0, 5.0), (20.0, 8.0)]);
        self.line(&[(20.0, 16.0), (20.0, 19.0), (19.0, 20.0), (16.0, 20.0)]);
        self.line(&[(8.0, 20.0), (5.0, 20.0), (4.0, 19.0), (4.0, 16.0)]);
    }

    fn speaker(&self) {
        self.speaker_body();
        self.arc((13.0, 12.0), 5.0, -45.0, 45.0);
        self.arc((13.0, 12.0), 8.5, -45.0, 45.0);
    }

    fn speaker_body(&self) {
        self.closed(&[
            (4.0, 9.0),
            (8.0, 9.0),
            (13.0, 5.0),
            (13.0, 19.0),
            (8.0, 15.0),
            (4.0, 15.0),
        ]);
    }

    fn timer(&self) {
        self.circle(12.0, 13.0, 8.0);
        self.line(&[(12.0, 9.0), (12.0, 13.0), (14.0, 15.0)]);
        self.line(&[(9.0, 2.0), (15.0, 2.0)]);
    }

    fn eye_off(&self) {
        self.arc((12.0, 15.75), 9.75, -157.4, -22.6);
        self.arc((12.0, 8.25), 9.75, 22.6, 157.4);
        self.line(&[(3.0, 3.0), (21.0, 21.0)]);
    }

    fn folder(&self) {
        self.closed(&[
            (3.0, 7.0),
            (5.0, 5.0),
            (9.0, 5.0),
            (11.0, 7.0),
            (19.0, 7.0),
            (21.0, 9.0),
            (21.0, 17.0),
            (19.0, 19.0),
            (5.0, 19.0),
            (3.0, 17.0),
        ]);
    }

    fn warn(&self) {
        self.closed(&[(12.0, 3.0), (22.0, 21.0), (2.0, 21.0)]);
        self.line(&[(12.0, 10.0), (12.0, 15.0)]);
        self.dot(12.0, 18.0, 1.1);
    }

    fn quit(&self) {
        self.line(&[(15.0, 4.0), (19.0, 4.0), (19.0, 20.0), (15.0, 20.0)]);
        self.line(&[(10.0, 8.0), (6.0, 12.0), (10.0, 16.0)]);
        self.line(&[(6.0, 12.0), (16.0, 12.0)]);
    }

    /// Six dots of the 8 × 16 grip symbol, centred in the view box.
    fn grip(&self) {
        for y in [7.0, 12.0, 17.0] {
            self.dot(10.0, y, 1.3);
            self.dot(14.0, y, 1.3);
        }
    }

    fn mic(&self) {
        self.rect(9.0, 3.0, 6.0, 11.0, 3.0);
        self.arc((12.0, 11.0), 7.0, 0.0, 180.0);
        self.line(&[(12.0, 18.0), (12.0, 21.0)]);
    }

    /// Eight teeth around a hole, the outline of the mockup's gear.
    fn gear(&self) {
        self.circle(12.0, 12.0, 3.0);
        let mut points = Vec::with_capacity(32);
        for tooth in 0..8 {
            let middle = tooth as f32 * 45.0;
            for (offset, r) in [(-14.0, 7.2), (-8.0, 9.6), (8.0, 9.6), (14.0, 7.2)] {
                let angle = (middle + offset) * PI / 180.0;
                points.push((12.0 + r * angle.cos(), 12.0 + r * angle.sin()));
            }
        }
        self.closed(&points);
    }
}
