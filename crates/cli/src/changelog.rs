//! `devcast app changelog`: «Что нового» of the app — the releases of `CHANGELOG.md` built into
//! the programs (`dc_update::news`), newest first.

use serde_json::{Value, json};

use crate::error::CliError;
use crate::run::Answer;

pub(crate) fn changelog(since: Option<&str>) -> Result<Answer, CliError> {
    let current = env!("CARGO_PKG_VERSION");
    let news = dc_update::news(since, current).map_err(|e| CliError::Usage(e.to_string()))?;
    let mut lines = Vec::new();
    let mut releases = Vec::new();
    for release in &news {
        lines.push(format!("Что нового в {}", release.version));
        let mut groups = Vec::new();
        for group in &release.groups {
            lines.push(format!("{}:", group.title));
            lines.extend(group.items.iter().map(|item| format!("  - {item}")));
            groups.push(json!({"title": group.title, "items": group.items}));
        }
        releases.push(json!({"version": release.version, "groups": groups}));
    }
    if news.is_empty() {
        lines.push(match since {
            Some(since) => format!("выпусков после {since} нет (версия программы {current})"),
            None => format!("выпусков в CHANGELOG.md нет (версия программы {current})"),
        });
    }
    Ok(Answer::new(lines.join("\n"), Value::Array(releases)))
}
