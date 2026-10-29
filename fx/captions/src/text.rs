//! `captions export` and `captions import`: the text of the cues for translation, one cue per line
//! as `<id> <text>` (`s3 Привет, мир!`), in the order of the output video, cut-out cues last.
//! Import changes only the text of the listed cues; cue times stay, words are retimed like
//! `captions set --text`.

use dc_core::{Command, CommandError, Diff, Project, TimeMap};

use crate::model::{CaptionsEffect, CueId};
use crate::retime::retext;

/// The text file of all cues of `project` and how many cues it has.
pub fn export_text(project: &Project) -> Result<(String, usize), CommandError> {
    let state = project.state::<CaptionsEffect>()?;
    let map = TimeMap::build(project);
    let mut cues: Vec<_> = state
        .cues
        .iter()
        .map(|c| (map.output_span(&c.media, c.range).map(|r| r.start()), c))
        .collect();
    cues.sort_by_key(|(at, c)| (at.is_none(), *at, c.id));
    let lines: Vec<String> = cues
        .iter()
        .map(|(_, c)| format!("{} {}\n", c.id, c.text()))
        .collect();
    Ok((lines.concat(), lines.len()))
}

/// Lines `<id> <text>` of a text file; empty lines and a byte order mark are skipped.
pub fn parse_text(file: &str) -> Result<Vec<(CueId, String)>, CommandError> {
    let mut texts: Vec<(CueId, String)> = Vec::new();
    let file = file.trim_start_matches('\u{feff}');
    for (number, line) in (1..).zip(file.lines()) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (id, text) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let id: CueId = id.parse().map_err(|_| {
            CommandError::new(format!(
                "строка {number}: в начале нужен id реплики, как в captions export (s3 Текст реплики)"
            ))
        })?;
        if texts.iter().any(|(seen, _)| *seen == id) {
            return Err(CommandError::new(format!(
                "строка {number}: реплика {id} уже была выше; оставь одну строку на реплику"
            )));
        }
        texts.push((id, text.trim().to_owned()));
    }
    Ok(texts)
}

/// New texts of cues; one undo step.
#[derive(Debug)]
pub struct CaptionsImport {
    pub texts: Vec<(CueId, String)>,
}

impl CaptionsImport {
    pub const NAME: &'static str = "captions import";
}

impl Command for CaptionsImport {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<CaptionsEffect>()?;
        let mut diff = Diff::default();
        for (id, text) in &self.texts {
            let index = state.position(*id)?;
            let cue = &mut state.cues[index];
            let words =
                retext(&cue.words, text).map_err(|e| CommandError::new(format!("{id}: {e}")))?;
            if words != cue.words {
                cue.words = words;
                cue.check()?;
                diff = diff.and(Diff::changed(*id));
            }
        }
        project.set_state::<CaptionsEffect>(&state)?;
        Ok(diff)
    }
}
