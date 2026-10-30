//! Callout events and style as stored in `project.effects["callout"]`.

use dc_core::time::{InvertedRange, Source};
use dc_core::{Effect, Id, IdCounter, IdKind, Item, Range, SourceTime, TimeDelta};
use dc_render::{GradientColors, Rgba};
use schemars::JsonSchema;
use serde::ser::Error as _;
use serde::{Deserialize, Serialize};

pub const SIZE_MIN: f64 = 0.5;
pub const SIZE_MAX: f64 = 2.0;
pub const GLOW_MAX: f64 = 40.0;
/// Longest text of a callout, characters (v1).
pub const TEXT_MAX: usize = 80;

pub struct Callout;

impl Effect for Callout {
    const ID: &'static str = "callout";
    const TITLE: &'static str = "Облачко у курсора";
    type State = CalloutState;
    const LISTS: &'static [&'static str] = &[EventKind::LIST];

    fn items(state: &CalloutState) -> Result<Vec<Item>, serde_json::Error> {
        state
            .events
            .iter()
            .map(|e| {
                let span = e.span().map_err(serde_json::Error::custom)?;
                Item::of(e.id, &e.media, span, e)
            })
            .collect()
    }
}

/// Id kind of callout events (`co1`, `co2`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct EventKind;

impl IdKind for EventKind {
    const PREFIX: &'static str = "co";
    const NOUN: &'static str = "события облачка";
    const PLURAL: &'static str = "события облачка";
    const LIST: &'static str = "callouts";
}

pub type EventId = Id<EventKind>;

/// What an event does to the callout of its recording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Shows the callout at once with `settings.text`; empty text shows nothing until a `text`
    /// event.
    On,
    /// Types `text` over the old one during the event; ignored while the callout is off.
    Text {
        /// One line, 1..=`TEXT_MAX` characters.
        text: String,
    },
    /// Erases the text from the end during the event, then hides the callout; ignored while it
    /// is off.
    Off,
}

impl Action {
    pub fn name(&self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Text { .. } => "text",
            Self::Off => "off",
        }
    }
}

/// An event on the callout track of one recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CalloutEvent {
    /// Id, `co<number>`.
    pub id: EventId,
    /// Recording (`Media.path`) whose source time `at` is in; the callout of each recording has
    /// its own events.
    pub media: String,
    /// Start, source time of the recording.
    pub at: SourceTime,
    /// How long typing or erasing takes, output time: a sped-up part does not hurry it
    /// (decision 000). `on` is instant; there it is only the length on the timeline.
    pub duration: TimeDelta,
    /// What the event does, with its text for `text`.
    #[serde(flatten)]
    pub action: Action,
}

impl CalloutEvent {
    /// The event on the recording's timeline: from `at`, `duration` long. Fails on a negative
    /// duration of a hand-edited project.
    pub fn span(&self) -> Result<Range<Source>, InvertedRange> {
        Range::new(self.at, self.at + self.duration)
    }
}

/// Look of every callout of the project (docs/porting/clicks-callout.md §10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CalloutSettings {
    /// Text an `on` event shows, one line up to `TEXT_MAX` characters; empty — the callout
    /// waits for a `text` event.
    pub text: String,
    /// Fill of the bubble without a gradient.
    pub background: Rgba,
    /// Colour of the text, #rrggbbaa.
    pub text_color: Rgba,
    /// `SIZE_MIN..=SIZE_MAX`, 1 — the text 0.72 of the visible cursor height (v1).
    pub size: f64,
    /// Fill the bubble with `colors` flowing in a loop instead of `background`.
    pub gradient: bool,
    /// Colours of the gradient, 2 to 4.
    pub colors: GradientColors,
    /// Soft light around the bubble, `0..=GLOW_MAX` (v1 units); 0 — none.
    pub glow: f64,
}

impl Default for CalloutSettings {
    fn default() -> Self {
        let teal = Rgba([0x14, 0xb8, 0xc4, 0xff]);
        let purple = Rgba([0xa8, 0x55, 0xf7, 0xff]);
        Self {
            text: String::new(),
            background: teal,
            text_color: Rgba([0xff, 0xff, 0xff, 0xff]),
            size: 1.0,
            gradient: false,
            colors: GradientColors::two(teal, purple),
            glow: 0.0,
        }
    }
}

/// Everything the callout effect stores in the project.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CalloutState {
    /// In the order they were added.
    pub events: Vec<CalloutEvent>,
    /// Next free event id; numbers are never reused.
    pub ids: IdCounter<EventKind>,
    /// Look of every callout and the text of `on`.
    pub settings: CalloutSettings,
}

impl CalloutState {
    pub fn event_ids(&self) -> impl Iterator<Item = EventId> + '_ {
        self.events.iter().map(|e| e.id)
    }
}
