//! The kit of an effect's timeline lane (docs/ARCHITECTURE.md, «Дорожка эффекта»): what the rows
//! of an effect show and how a gesture of the editor on them turns into registry commands, so the
//! behaviour lives in the effect's crate and the editor only does the generic gestures (picking,
//! moving, edges, snapping, the rubber band).
//!
//! An effect gives its objects as [`Region`]s (place on the output, label, look, an [`Item`] when
//! it can be picked) and a [`Lane`]: the commands of a moved region, a dragged edge, a removal, a
//! copy. Every command is a [`Step`] written as `devcast` takes it (times — seconds with six
//! digits, [`time`]); a command that refuses fails the whole gesture.

use std::fmt;

use dc_core::time::Output;
use dc_core::{Id, IdKind, OutputTime, Project, Range, Step, TimeDelta};
use serde_json::{Map, Value};

use crate::panel::time_arg;

/// An object of an effect on the timeline, by its id as `devcast` writes it (`z3`): the prefix
/// of its kind and its number.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Item {
    prefix: &'static str,
    number: u32,
}

impl Item {
    pub fn of<K: IdKind>(id: Id<K>) -> Self {
        Self {
            prefix: K::PREFIX,
            number: id.number(),
        }
    }

    /// The id when the item is of kind `K`.
    pub fn id<K: IdKind>(self) -> Option<Id<K>> {
        (self.prefix == K::PREFIX).then(|| Id::from_number(self.number))
    }

    pub fn is<K: IdKind>(self) -> bool {
        self.prefix == K::PREFIX
    }

    /// Items of one kind lie on the same rows: Ctrl+A picks them all.
    pub fn same_kind(self, other: Self) -> bool {
        self.prefix == other.prefix
    }

    pub fn prefix(self) -> &'static str {
        self.prefix
    }

    /// The item whose id is `text` (`s3`), of one of the kinds with `prefixes`.
    pub fn parse(text: &str, prefixes: impl IntoIterator<Item = &'static str>) -> Option<Self> {
        prefixes.into_iter().find_map(|prefix| {
            let digits = text.strip_prefix(prefix)?;
            let canonical = !digits.is_empty()
                && digits.bytes().all(|b| b.is_ascii_digit())
                && !digits.starts_with('0');
            let number = digits.parse().ok().filter(|_| canonical)?;
            Some(Self { prefix, number })
        })
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.prefix, self.number)
    }
}

impl fmt::Debug for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// An object on a row of an effect: from `range.start` to `range.end` of the output.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    pub range: Range<Output>,
    pub label: String,
    /// A hidden effect or silent sound: drawn pale.
    pub faded: bool,
    /// The object, when it can be picked and edited.
    pub item: Option<Item>,
    /// Output times of the keys that show (annotations, 7.12), in time order.
    pub keys: Vec<OutputTime>,
}

impl Region {
    pub fn new(range: Range<Output>, label: String) -> Self {
        Self {
            range,
            label,
            faded: false,
            item: None,
            keys: Vec::new(),
        }
    }
}

/// What a row of an effect shows: its regions and whether the effect is switched off (captions
/// not shown, sound muted).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Row {
    pub regions: Vec<Region>,
    pub off: bool,
}

/// A region after a move: its new place and, between the rows of an effect with tracks, its new
/// track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moved {
    pub item: Item,
    pub range: Range<Output>,
    pub track: Option<u32>,
}

/// The `end` (or the start) of `item` lying at `range` dragged to `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resize {
    pub item: Item,
    pub range: Range<Output>,
    pub end: bool,
    pub at: OutputTime,
}

/// What an effect does with its items on the timeline. Each effect with regions exports one
/// (`dc_fx_zoom::lane::LANE`); the editor finds it by the prefix of an item.
pub struct Lane {
    /// The prefix of the ids of its items (`IdKind::PREFIX`).
    pub prefix: &'static str,
    /// The first word of its commands (`zoom`): `<op> rm`, `<op> key move`.
    pub op: &'static str,
    /// The command that puts a region in its new place.
    pub moved: fn(&Moved) -> Step,
    /// The command of a dragged edge.
    pub resized: fn(&Project, &Resize) -> Step,
    /// The command that adds the item again, `length` long on the output, without its `at`
    /// (Ctrl+C, Ctrl+V).
    pub copied: fn(&Project, Item, TimeDelta) -> Option<Step>,
    /// «Дублировать» of a panel: the command that adds the item lying at `range` again, placed.
    pub duplicated: fn(&Project, Item, Range<Output>) -> Option<Step>,
}

impl Lane {
    /// `<op> rm <id>`.
    pub fn removed(&self, item: Item) -> Step {
        Step::new(format!("{} rm", self.op), id_args(item))
    }

    /// `<op> key move <id> --at <from> --to <to>`: the key of `item` at output `from`.
    pub fn key_moved(&self, item: Item, from: OutputTime, to: OutputTime) -> Step {
        let mut args = id_args(item);
        args.insert("at".to_owned(), time(from));
        args.insert("to".to_owned(), time(to));
        Step::new(format!("{} key move", self.op), args)
    }

    /// `<op> key rm <id> --at <at>`.
    pub fn key_removed(&self, item: Item, at: OutputTime) -> Step {
        let mut args = id_args(item);
        args.insert("at".to_owned(), time(at));
        Step::new(format!("{} key rm", self.op), args)
    }
}

/// The arguments `--id <item>`.
pub fn id_args(item: Item) -> Map<String, Value> {
    let mut args = Map::new();
    args.insert("id".to_owned(), Value::String(item.to_string()));
    args
}

/// A time argument: `12.500000`.
pub fn time(t: OutputTime) -> Value {
    Value::String(time_arg(t))
}

/// A length in seconds: the number `devcast` reads from `1.500000`.
pub fn seconds(length: TimeDelta) -> Value {
    Value::from(length.micros() as f64 / 1e6)
}

/// [`Lane::duplicated`] of an object that stays where it is: its copy at `range`.
pub fn in_place(step: Option<Step>, range: Range<Output>) -> Option<Step> {
    let mut step = step?;
    step.args.insert("at".to_owned(), time(range.start()));
    Some(step)
}
