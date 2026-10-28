//! Cursor shapes as the hooks see them, turned into image and shape records: one picture per
//! distinct image (cursor handles with the same pixels share it), ids in order of first use.

use std::collections::HashMap;

use super::cursor::{CursorEvent, CursorKind};
use super::image::CursorImage;
use crate::error::CaptureError;

/// The cursor right now, as the hook read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeenShape {
    /// The cursor handle (HCURSOR on Windows).
    pub handle: isize,
    pub kind: CursorKind,
    pub visible: bool,
    /// The picture, sent the first time the hook sees this handle.
    pub image: Option<CursorImage>,
}

/// A change to write: a new picture first (its id, to save as a file before the records),
/// then the shape event.
pub struct Change<'a> {
    pub new_image: Option<(u32, &'a CursorImage)>,
    pub shape: CursorEvent,
}

#[derive(Default)]
pub struct Shapes {
    /// Distinct pictures and their id once written.
    pictures: Vec<(CursorImage, Option<u32>)>,
    handles: HashMap<isize, usize>,
    /// Kind and picture (index into `pictures`, `None` when hidden).
    current: Option<(CursorKind, Option<usize>)>,
    written: Option<(CursorKind, Option<usize>)>,
}

impl Shapes {
    pub fn see(&mut self, shape: SeenShape) -> Result<(), CaptureError> {
        if let Some(image) = shape.image {
            let index = match self.pictures.iter().position(|(p, _)| *p == image) {
                Some(index) => index,
                None => {
                    self.pictures.push((image, None));
                    self.pictures.len() - 1
                }
            };
            self.handles.insert(shape.handle, index);
        }
        let picture = match shape.visible {
            true => Some(*self.handles.get(&shape.handle).ok_or_else(|| {
                CaptureError::platform("записать курсор")(
                    "курсор пришёл без картинки".to_owned(),
                )
            })?),
            false => None,
        };
        self.current = Some((shape.kind, picture));
        Ok(())
    }

    /// What makes the written shape the current one; `None` if it is already.
    pub fn change(&mut self) -> Option<Change<'_>> {
        let (kind, picture) = self.current?;
        if self.written == self.current {
            return None;
        }
        self.written = self.current;
        let next = self.pictures.iter().filter(|(_, id)| id.is_some()).count() as u32;
        let fresh = picture.filter(|&index| self.pictures[index].1.is_none());
        if let Some(index) = fresh {
            self.pictures[index].1 = Some(next);
        }
        let image = picture.and_then(|index| self.pictures[index].1);
        Some(Change {
            new_image: fresh.map(|index| (next, &self.pictures[index].0)),
            shape: CursorEvent::Shape { kind, image },
        })
    }
}
