//! `captions gen`: cues recognized by the caller (the CLI owns Whisper and the audio) replace all
//! cues. Tracks are recognized one by one and merged by time; where they overlap, the track
//! earlier in the list wins (decision 000 «Субтитры», docs/porting/captions.md §2).

use dc_core::time::Source;
use dc_core::{Command, CommandError, Diff, Project, Range};

use crate::model::{CaptionsEffect, Cue, Word};
use crate::retime::range;

/// A recognized cue before it gets an id.
#[derive(Debug, Clone, PartialEq)]
pub struct NewCue {
    pub media: String,
    pub range: Range<Source>,
    pub words: Vec<Word>,
}

/// Replaces all cues with `cues`; one undo step.
#[derive(Debug)]
pub struct CaptionsGen {
    pub cues: Vec<NewCue>,
}

impl CaptionsGen {
    pub const NAME: &'static str = "captions gen";
}

impl Command for CaptionsGen {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<CaptionsEffect>()?;
        let mut diff = Diff::default();
        for cue in state.cues.drain(..) {
            diff = diff.and(Diff::removed(cue.id));
        }
        for new in &self.cues {
            let cue = Cue {
                id: state.ids.allocate(),
                media: new.media.clone(),
                range: new.range,
                words: new.words.clone(),
            };
            cue.check()?;
            diff = diff.and(Diff::added(cue.id));
            state.cues.push(cue);
        }
        state.sort();
        project.set_state::<CaptionsEffect>(&state)?;
        Ok(diff)
    }
}

/// Cues of several tracks, the first track first: words of a later track that overlap words of
/// an earlier one in the same recording are dropped, the rest split into cues at the dropped
/// words. A piece whose words take no time is dropped too.
pub fn merge_tracks(tracks: Vec<Vec<NewCue>>) -> Result<Vec<NewCue>, CommandError> {
    let mut merged: Vec<NewCue> = Vec::new();
    for track in tracks {
        let mut kept = Vec::new();
        for cue in track {
            let taken = |w: &Word| {
                merged.iter().any(|m| {
                    m.media == cue.media
                        && m.words
                            .iter()
                            .any(|t| t.range.intersect(&w.range).is_some())
                })
            };
            for words in cue.words.split(taken) {
                let whole = words.len() == cue.words.len();
                let (Some(first), Some(last)) = (words.first(), words.last()) else {
                    continue;
                };
                let span = range(first.range.start(), last.range.end())?;
                if !whole && span.is_empty() {
                    continue;
                }
                kept.push(NewCue {
                    media: cue.media.clone(),
                    range: if whole { cue.range } else { span },
                    words: words.to_vec(),
                });
            }
        }
        merged.extend(kept);
    }
    merged.sort_by(|a, b| (&a.media, a.range.start()).cmp(&(&b.media, b.range.start())));
    Ok(merged)
}
