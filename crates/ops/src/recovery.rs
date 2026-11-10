//! A recording that was never finished — its process died, or its stop failed — is finished
//! and added to the project the next time a writer opens the project (decision 007): `devcast`
//! and the editor go the same way here, auto zooms included.

use dc_capture::project::{Recovered, recover_recordings, unfinished_recordings};
use dc_core::SourceTime;
use dc_project::{ProjectDir, ProjectLock};

use crate::error::OpsError;

/// The folders under `media/` of the recordings of `dir` that were never finished.
pub fn interrupted(dir: &ProjectDir) -> Result<Vec<String>, OpsError> {
    unfinished_recordings(&dir.media_dir()).map_err(|e| OpsError::Failed(e.to_string()))
}

/// Finishes the interrupted recordings of `dir` and adds them to its working state
/// (`session.json` when unsaved changes are left, otherwise `project.json`), then their auto
/// zooms as after a recording. Returns a line for the user per recording; none when there was
/// nothing to recover. The caller holds `lock` of `dir`: a running recording holds it itself.
pub fn recover(dir: &ProjectDir, lock: &ProjectLock) -> Result<Vec<String>, OpsError> {
    let failed = |e: dc_project::ProjectError| OpsError::Failed(e.to_string());
    let folders = interrupted(dir)?;
    if folders.is_empty() {
        return Ok(Vec::new());
    }
    let session = dir.load_session().map_err(failed)?;
    let unsaved = session.is_some();
    let mut project = match session {
        Some(project) => project,
        None => dir.load().map_err(failed)?,
    };
    let recovered = recover_recordings(&dir.media_dir(), &folders, &mut project)
        .map_err(|e| OpsError::Failed(e.to_string()))?;
    match unsaved {
        true => dir.save_session(&project).map_err(failed)?,
        false => dir.save(&project).map_err(failed)?,
    }
    let mut lines: Vec<String> = recovered.iter().map(line).collect();
    if recovered
        .iter()
        .any(|r| matches!(r, Recovered::Clip { .. }))
    {
        lines.extend(auto_zooms(dir, lock));
    }
    Ok(lines)
}

/// Auto zooms for the recovered recordings; a failure is a line for the user: the recording
/// itself is already in the project.
fn auto_zooms(dir: &ProjectDir, lock: &ProjectLock) -> Option<String> {
    match crate::autozoom::after_recording(dir, lock) {
        Ok(Some(applied)) if applied.changed => {
            Some(format!("автозумы восстановленной записи: {applied}"))
        }
        Ok(_) => None,
        Err(e) => Some(format!(
            "автозумы восстановленной записи не поставлены: {e}"
        )),
    }
}

fn line(recovered: &Recovered) -> String {
    match recovered {
        Recovered::Clip {
            folder,
            clip,
            duration,
        } => format!(
            "восстановлена прерванная запись media/{folder}: клип {clip}, {}",
            SourceTime::ZERO + *duration
        ),
        Recovered::Empty { folder } => format!(
            "прерванная запись media/{folder} оборвалась до первого целого сегмента: восстанавливать нечего, её файлы удалены"
        ),
    }
}
