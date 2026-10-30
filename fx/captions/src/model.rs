//! Caption cues as stored in `project.effects["captions"]`.

use dc_core::time::Source;
use dc_core::{CommandError, Effect, Id, IdCounter, IdKind, Item, Range, index_of};

use crate::style::CaptionsSettings;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub struct CaptionsEffect;

impl Effect for CaptionsEffect {
    const ID: &'static str = "captions";
    const TITLE: &'static str = "Субтитры";
    type State = CaptionsState;
    const LISTS: &'static [&'static str] = &[CueKind::LIST];

    fn items(state: &CaptionsState) -> Result<Vec<Item>, serde_json::Error> {
        state
            .cues
            .iter()
            .map(|c| Item::of(c.id, &c.media, c.range, &Shown { text: c.text() }))
            .collect()
    }
}

/// What `devcast ls captions` shows of a cue besides its time.
#[derive(Serialize)]
struct Shown {
    text: String,
}

/// Id kind of caption cues (`s1`, `s2`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CueKind;

impl IdKind for CueKind {
    const PREFIX: &'static str = "s";
    const NOUN: &'static str = "реплики";
    const PLURAL: &'static str = "реплики";
    const LIST: &'static str = "captions";
}

pub type CueId = Id<CueKind>;

/// A spoken word of a cue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Word {
    /// The word with its punctuation, without spaces.
    pub text: String,
    /// When it is said, in the source time of the cue's recording; may be empty (Whisper gives
    /// two words the same start).
    pub range: Range<Source>,
}

/// One caption on screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Cue {
    /// Id, `s<number>`.
    pub id: CueId,
    /// Recording (`Media.path`) whose source time `range` and the word times are in.
    pub media: String,
    /// When the cue is on screen; not empty, covers all its words.
    pub range: Range<Source>,
    /// At least one; in time order without overlaps. The text of the cue is the words joined by
    /// single spaces.
    pub words: Vec<Word>,
}

impl Cue {
    pub fn text(&self) -> String {
        join(&self.words)
    }

    /// Fails if the cue breaks the invariants of its fields.
    pub fn check(&self) -> Result<(), CommandError> {
        let broken = |what: &str| {
            Err(CommandError::new(format!(
                "реплика {}: {what}; поправь текст или тайминг (devcast help captions set)",
                self.id
            )))
        };
        if self.range.is_empty() {
            return broken("длительность должна быть больше нуля");
        }
        let (Some(first), Some(last)) = (self.words.first(), self.words.last()) else {
            return broken("нет ни одного слова");
        };
        if first.range.start() < self.range.start() || last.range.end() > self.range.end() {
            return broken("слова выходят за границы реплики");
        }
        if self
            .words
            .windows(2)
            .any(|w| w[1].range.start() < w[0].range.end())
        {
            return broken("слова перекрываются или идут не по порядку");
        }
        if self
            .words
            .iter()
            .any(|w| w.text.is_empty() || w.text.chars().any(char::is_whitespace))
        {
            return broken("слово пустое или с пробелом внутри");
        }
        Ok(())
    }
}

pub(crate) fn join(words: &[Word]) -> String {
    words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Everything the captions effect stores in the project.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CaptionsState {
    /// Sorted by recording, then start; cues may overlap.
    pub cues: Vec<Cue>,
    /// Next free cue id; numbers are never reused.
    pub ids: IdCounter<CueKind>,
    /// Look of all captions (`SETTINGS_FIELD`: copied into new projects by `Defaults`).
    pub settings: CaptionsSettings,
}

impl CaptionsState {
    pub(crate) fn position(&self, id: CueId) -> Result<usize, CommandError> {
        index_of(&self.cues, id, |c| c.id)
    }

    pub(crate) fn sort(&mut self) {
        self.cues.sort_by(|a, b| {
            (&a.media, a.range.start(), a.id).cmp(&(&b.media, b.range.start(), b.id))
        });
    }
}
