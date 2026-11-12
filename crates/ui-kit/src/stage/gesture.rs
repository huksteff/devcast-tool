//! The seam of a gesture on the preview (docs/ARCHITECTURE.md «Дорожка эффекта»): an effect's
//! [`Stage`] works on the pointer over the output frame with what the editor gives it ([`Cx`]),
//! keeps its own state in [`Held`] while the button is held and its window choices in [`Kept`],
//! and answers with an [`Answer`]. The editor holds a table of stages and knows none of their
//! types.

use std::any::Any;
use std::fmt::{self, Debug};

use dc_core::time::Output;
use dc_core::{OutputTime, Project, Range};
use egui::{Rect, Response, Ui};

use super::Answer;
use super::geom::Mapping;
use crate::lane::Item;

/// An effect's gesture on the preview: an entry of the editor's table, in the order the pointer
/// is offered to them.
pub trait Stage: Sync {
    /// While `picked` is picked the preview shows the picture without the camera, and this stage
    /// works over it.
    fn bare(&self, picked: Option<Picked>) -> bool {
        let _ = picked;
        false
    }

    /// The pointer over the output frame: `response` is the preview area's, `map` the frame's.
    fn ui(&self, ui: &Ui, cx: &mut Cx<'_>, response: &Response, map: Mapping) -> Answer;

    /// A bar over the top of the preview `area` (the brush's), drawn after the frame.
    fn bar(&self, ui: &Ui, cx: &mut Cx<'_>, area: Rect) {
        let _ = (ui, cx, area);
    }
}

/// What the editor gives a stage this frame.
pub struct Cx<'a> {
    pub project: &'a Project,
    /// The object picked alone.
    pub picked: Option<Picked>,
    /// The objects the timeline shows on its rows shown (hidden rows do not count), row by row.
    pub shown: &'a [Shown<'a>],
    pub playhead: OutputTime,
    /// «Авто-ключи»: where an annotation's edit goes.
    pub auto_keys: bool,
    /// Snapping (N) is on.
    pub magnet: bool,
    /// The gesture going on, whichever stage's.
    pub held: &'a mut Option<Held>,
    /// The stages' choices the window keeps (not the project).
    pub kept: &'a mut Kept,
}

/// The object picked alone: a region, or a key of one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picked {
    Region(Item),
    Key(Item),
}

impl Picked {
    /// The object, picked as a region or by one of its keys.
    pub fn item(self) -> Item {
        match self {
            Self::Region(item) | Self::Key(item) => item,
        }
    }
}

/// An object on a timeline row shown: the row's track (0 for a kind without tracks), its place
/// on the output and the output times of its keys that show.
#[derive(Debug, Clone, Copy)]
pub struct Shown<'a> {
    pub track: u32,
    pub item: Item,
    pub range: Range<Output>,
    pub keys: &'a [OutputTime],
}

/// What a press on the preview asks the editor to pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// Just this object.
    Only(Item),
    /// Nothing: the selection drops.
    Nothing,
}

/// A stage's gesture state: kept by the editor while the button is held.
pub trait Gesture: Debug + Clone + PartialEq + Send + Sync + 'static {
    /// The preview shows a draft of it.
    fn drafted(&self) -> bool;
}

/// The gesture going on, of any stage; a stage finds its own by its type ([`Held::get`]).
pub struct Held(Box<dyn Erased>);

impl Held {
    pub fn new(gesture: impl Gesture) -> Self {
        Self(Box::new(gesture))
    }

    /// The gesture when it is a `G`.
    pub fn get<G: Gesture>(&self) -> Option<&G> {
        self.0.any().downcast_ref()
    }

    pub fn get_mut<G: Gesture>(&mut self) -> Option<&mut G> {
        self.0.any_mut().downcast_mut()
    }

    /// The preview shows a draft of it.
    pub fn drafted(&self) -> bool {
        self.0.drafted()
    }
}

/// A gesture with its type erased.
trait Erased: Debug + Send + Sync {
    fn any(&self) -> &dyn Any;
    fn any_mut(&mut self) -> &mut dyn Any;
    fn cloned(&self) -> Box<dyn Erased>;
    fn same(&self, other: &dyn Erased) -> bool;
    fn drafted(&self) -> bool;
}

impl<G: Gesture> Erased for G {
    fn any(&self) -> &dyn Any {
        self
    }

    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn cloned(&self) -> Box<dyn Erased> {
        Box::new(self.clone())
    }

    fn same(&self, other: &dyn Erased) -> bool {
        other.any().downcast_ref::<G>() == Some(self)
    }

    fn drafted(&self) -> bool {
        Gesture::drafted(self)
    }
}

impl Clone for Held {
    fn clone(&self) -> Self {
        Self(self.0.cloned())
    }
}

impl PartialEq for Held {
    fn eq(&self, other: &Self) -> bool {
        self.0.same(other.0.as_ref())
    }
}

impl Debug for Held {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A stage's choice the window keeps (the brush bar's tool): one value per type.
pub trait Choice: Debug + Clone + PartialEq + Default + Send + Sync + 'static {}

impl<T: Debug + Clone + PartialEq + Default + Send + Sync + 'static> Choice for T {}

/// The stages' choices the window keeps, one per type; an unset one is its default.
#[derive(Default)]
pub struct Kept(Vec<Box<dyn Kept1>>);

impl Kept {
    /// The kept `T`, its default when none was set.
    pub fn get<T: Choice>(&self) -> T {
        self.0
            .iter()
            .find_map(|k| k.any().downcast_ref::<T>())
            .cloned()
            .unwrap_or_default()
    }

    pub fn set<T: Choice>(&mut self, value: T) {
        self.0.retain(|k| !k.any().is::<T>());
        self.0.push(Box::new(value));
    }
}

/// A kept choice with its type erased.
trait Kept1: Debug + Send + Sync {
    fn any(&self) -> &dyn Any;
    fn cloned(&self) -> Box<dyn Kept1>;
    fn same(&self, other: &dyn Kept1) -> bool;
}

impl<T: Choice> Kept1 for T {
    fn any(&self) -> &dyn Any {
        self
    }

    fn cloned(&self) -> Box<dyn Kept1> {
        Box::new(self.clone())
    }

    fn same(&self, other: &dyn Kept1) -> bool {
        other.any().downcast_ref::<T>() == Some(self)
    }
}

impl Clone for Kept {
    fn clone(&self) -> Self {
        Self(self.0.iter().map(|k| k.cloned()).collect())
    }
}

impl PartialEq for Kept {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .all(|k| other.0.iter().any(|o| k.same(o.as_ref())))
    }
}

impl Debug for Kept {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(&self.0).finish()
    }
}
