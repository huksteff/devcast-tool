//! Icons of the editor (docs/design/editor.html), same view box and pen as the recording ones.

use super::{Icon, Pen};

impl Pen<'_> {
    /// Draws an editor icon; the recording icons are drawn by `draw` itself.
    pub(super) fn editor(&self, icon: Icon) {
        match icon {
            Icon::Library => {
                for (x, y) in [(3.0, 3.0), (14.0, 3.0), (3.0, 14.0), (14.0, 14.0)] {
                    self.rect(x, y, 7.0, 7.0, 1.0);
                }
            }
            Icon::Undo => self.undo(),
            Icon::Redo => self.redo(),
            Icon::Help => self.help(),
            Icon::Export => {
                self.line(&[(12.0, 15.0), (12.0, 3.0)]);
                self.line(&[(7.0, 8.0), (12.0, 3.0), (17.0, 8.0)]);
                self.line(&[
                    (4.0, 15.0),
                    (4.0, 19.0),
                    (6.0, 21.0),
                    (18.0, 21.0),
                    (20.0, 19.0),
                    (20.0, 15.0),
                ]);
            }
            Icon::Crop => {
                self.line(&[(6.0, 2.0), (6.0, 16.0), (8.0, 18.0), (22.0, 18.0)]);
                self.line(&[(18.0, 22.0), (18.0, 8.0), (16.0, 6.0), (2.0, 6.0)]);
            }
            Icon::Pointer => self.closed(&[(5.0, 3.0), (19.0, 11.0), (13.0, 13.0), (10.0, 19.0)]),
            Icon::Blade => self.blade(),
            Icon::Diamond => self.closed(&[(12.0, 3.0), (21.0, 12.0), (12.0, 21.0), (3.0, 12.0)]),
            Icon::Eye => {
                self.arc((12.0, 15.64), 10.64, -160.0, -20.0);
                self.arc((12.0, 8.36), 10.64, 20.0, 160.0);
                self.circle(12.0, 12.0, 3.0);
            }
            Icon::SkipBack => {
                self.closed(&[(19.0, 5.0), (9.0, 12.0), (19.0, 19.0)]);
                self.line(&[(5.0, 5.0), (5.0, 19.0)]);
            }
            Icon::SkipForward => {
                self.closed(&[(5.0, 5.0), (15.0, 12.0), (5.0, 19.0)]);
                self.line(&[(19.0, 5.0), (19.0, 19.0)]);
            }
            Icon::Left => self.line(&[(15.0, 6.0), (9.0, 12.0), (15.0, 18.0)]),
            Icon::Magnet => self.magnet(),
            Icon::ZoomIn => {
                self.circle(11.0, 11.0, 7.0);
                self.line(&[(21.0, 21.0), (16.7, 16.7)]);
                self.line(&[(11.0, 8.0), (11.0, 14.0)]);
                self.line(&[(8.0, 11.0), (14.0, 11.0)]);
            }
            _ => self.editor_more(icon),
        }
    }

    fn editor_more(&self, icon: Icon) {
        match icon {
            Icon::Wand => self.wand(),
            Icon::Captions => {
                self.rect(3.0, 5.0, 18.0, 14.0, 2.0);
                self.arc((10.0, 12.0), 2.0, -90.0, -270.0);
                self.arc((17.0, 12.0), 2.0, -90.0, -270.0);
            }
            Icon::Keyboard => {
                self.rect(2.0, 6.0, 20.0, 12.0, 2.0);
                for x in [6.0, 10.0, 14.0, 18.0] {
                    self.dot(x, 10.0, 1.0);
                }
                self.line(&[(7.0, 14.0), (17.0, 14.0)]);
            }
            Icon::Text => {
                self.line(&[(5.0, 6.0), (5.0, 4.0), (19.0, 4.0), (19.0, 6.0)]);
                self.line(&[(12.0, 4.0), (12.0, 20.0)]);
                self.line(&[(9.0, 20.0), (15.0, 20.0)]);
            }
            Icon::Frame => {
                self.rect(3.0, 6.0, 18.0, 12.0, 1.0);
                for (x, y) in [(7.0, 3.0), (17.0, 3.0), (7.0, 18.0), (17.0, 18.0)] {
                    self.line(&[(x, y), (x, y + 3.0)]);
                }
            }
            Icon::Cursor => self.closed(&[(5.0, 3.0), (11.0, 20.0), (13.5, 12.5), (21.0, 10.0)]),
            Icon::Fit => {
                self.line(&[(3.0, 12.0), (21.0, 12.0)]);
                self.line(&[(7.0, 8.0), (3.0, 12.0), (7.0, 16.0)]);
                self.line(&[(17.0, 8.0), (21.0, 12.0), (17.0, 16.0)]);
            }
            Icon::Detach => self.detach(),
            Icon::Trash => {
                self.line(&[(4.0, 7.0), (20.0, 7.0)]);
                self.line(&[(10.0, 11.0), (10.0, 17.0)]);
                self.line(&[(14.0, 11.0), (14.0, 17.0)]);
                self.line(&[(5.0, 7.0), (6.0, 20.0), (18.0, 20.0), (19.0, 7.0)]);
                self.line(&[(9.0, 7.0), (9.0, 4.0), (15.0, 4.0), (15.0, 7.0)]);
            }
            Icon::Mute => {
                self.speaker_body();
                self.line(&[(17.0, 9.0), (22.0, 15.0)]);
                self.line(&[(22.0, 9.0), (17.0, 15.0)]);
            }
            // The recording icons are drawn by `draw` and never come here.
            _ => {}
        }
    }

    fn undo(&self) {
        self.line(&[(9.0, 14.0), (4.0, 9.0), (9.0, 4.0)]);
        self.line(&[(4.0, 9.0), (14.0, 9.0)]);
        self.arc((14.0, 15.0), 6.0, -90.0, 90.0);
        self.line(&[(14.0, 21.0), (11.0, 21.0)]);
    }

    fn redo(&self) {
        self.line(&[(15.0, 14.0), (20.0, 9.0), (15.0, 4.0)]);
        self.line(&[(20.0, 9.0), (10.0, 9.0)]);
        self.arc((10.0, 15.0), 6.0, 270.0, 90.0);
        self.line(&[(10.0, 21.0), (13.0, 21.0)]);
    }

    fn help(&self) {
        self.circle(12.0, 12.0, 9.0);
        self.arc((12.0, 9.0), 2.5, 180.0, 426.5);
        self.line(&[(13.0, 11.3), (12.2, 12.1), (12.0, 13.0)]);
        self.dot(12.0, 17.0, 1.1);
    }

    fn blade(&self) {
        self.circle(6.0, 6.0, 3.0);
        self.circle(6.0, 18.0, 3.0);
        self.line(&[(20.0, 4.0), (8.1, 15.9)]);
        self.line(&[(14.5, 14.5), (20.0, 20.0)]);
        self.line(&[(8.1, 8.1), (12.0, 12.0)]);
    }

    fn magnet(&self) {
        self.line(&[(6.0, 3.0), (6.0, 11.0)]);
        self.arc((12.0, 11.0), 6.0, 180.0, 0.0);
        self.line(&[(18.0, 11.0), (18.0, 3.0)]);
        self.line(&[(6.0, 7.0), (10.0, 7.0)]);
        self.line(&[(14.0, 7.0), (18.0, 7.0)]);
    }

    fn wand(&self) {
        let strokes = [
            [(15.0, 4.0), (15.0, 2.0)],
            [(15.0, 10.0), (15.0, 8.0)],
            [(11.0, 6.0), (13.0, 6.0)],
            [(17.0, 6.0), (19.0, 6.0)],
            [(4.0, 20.0), (14.0, 10.0)],
            [(19.0, 13.0), (19.0, 15.0)],
            [(18.0, 14.0), (20.0, 14.0)],
            [(8.0, 3.0), (8.0, 5.0)],
            [(7.0, 4.0), (9.0, 4.0)],
        ];
        for stroke in strokes {
            self.line(&stroke);
        }
    }

    fn detach(&self) {
        self.line(&[(14.0, 4.0), (20.0, 4.0), (20.0, 10.0)]);
        self.line(&[(20.0, 4.0), (12.0, 12.0)]);
        self.line(&[
            (18.0, 14.0),
            (18.0, 19.0),
            (17.0, 20.0),
            (5.0, 20.0),
            (4.0, 19.0),
            (4.0, 7.0),
            (5.0, 6.0),
            (10.0, 6.0),
        ]);
    }
}
