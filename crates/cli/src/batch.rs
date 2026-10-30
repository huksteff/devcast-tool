//! `devcast batch <проекты…> -- <команда…>`: one command in several projects, each with its own
//! lock and undo step, as if run with `--project`. A project that fails does not stop the others:
//! its error goes to stderr and the exit code is 1. The projects are folders `*.dcproj`; a `*` or
//! `?` in the last part of a path is expanded here, since Windows shells do not expand it.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::edit::Ctx;
use crate::error::CliError;
use crate::globals::Globals;
use crate::run::{self, Answer};

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    #[arg(
        required = true,
        help = "Проекты: папки *.dcproj, можно со звёздочкой в имени (уроки/*.dcproj)"
    )]
    projects: Vec<String>,
    #[arg(
        last = true,
        required = true,
        help = "Команда после --, как без devcast: set captions.size=36"
    )]
    command: Vec<String>,
}

pub(crate) fn batch(ctx: &Ctx<'_>, args: &Args) -> Result<Answer, CliError> {
    if ctx.globals.project.is_some() {
        return Err(CliError::Usage(
            "у batch проекты перечисляются перед --, без --project".to_owned(),
        ));
    }
    let cli = run::parse(args.command.clone())?;
    if let Some(why) = cli.not_for_batch() {
        return Err(CliError::Usage(format!(
            "«{}» в batch нельзя: команда {why}",
            args.command.join(" ")
        )));
    }
    if let Some(words) = cli.op_words() {
        let (op, taken) = ctx.registry.resolve(&words)?;
        dc_ops::parse_args(op, &words[taken..])?;
    }
    let projects = projects(ctx.cwd, &args.projects)?;
    Ok(each(ctx, &projects, |ctx| {
        run::dispatch(args.command.clone(), ctx)
    }))
}

/// Runs `work` for each of `projects` as `--project`: the answers one after another, the
/// failures as warnings with exit code 1.
pub(crate) fn each(
    ctx: &Ctx<'_>,
    projects: &[PathBuf],
    mut work: impl FnMut(&Ctx<'_>) -> Result<Answer, CliError>,
) -> Answer {
    let mut lines = Vec::new();
    let mut results = Vec::new();
    let mut warnings = Vec::new();
    let mut failed = false;
    for project in projects {
        let name = name(project);
        let globals = Globals {
            project: Some(project.clone()),
            ..ctx.globals.clone()
        };
        let one = Ctx {
            globals: &globals,
            cwd: ctx.cwd,
            registry: ctx.registry,
            local: ctx.local,
        };
        match work(&one) {
            Ok(answer) => {
                lines.push(format!("{name}: {}", answer.text.replace('\n', "\n  ")));
                warnings.extend(answer.warnings.iter().map(|w| format!("{name}: {w}")));
                failed |= answer.code != 0;
                results.push(json!({"project": name, "result": answer.json}));
            }
            Err(error) => {
                failed = true;
                warnings.push(format!("{name}: не получилось: {error}"));
                results.push(
                    json!({"project": name, "error": error.to_string(), "code": error.code()}),
                );
            }
        }
    }
    let done = results.iter().filter(|r| r.get("result").is_some()).count();
    lines.push(format!("готово в {done} из {}", projects.len()));
    Answer {
        text: lines.join("\n"),
        json: Value::Array(results),
        warnings,
        code: u8::from(failed),
    }
}

fn name(project: &Path) -> String {
    project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| project.display().to_string())
}

/// Project folders named by `patterns` (relative to `cwd`), sorted, each once.
pub(crate) fn projects(cwd: &Path, patterns: &[String]) -> Result<Vec<PathBuf>, CliError> {
    let mut found = Vec::new();
    for pattern in patterns {
        let path = Path::new(pattern);
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let parent = path.parent().unwrap_or(Path::new(""));
        if parent.to_string_lossy().contains(['*', '?']) {
            return Err(CliError::Usage(format!(
                "«{pattern}»: звёздочка и ? — только в имени проекта, не в папках"
            )));
        }
        match file.filter(|f| f.contains(['*', '?'])) {
            Some(mask) => {
                let matched = expand(&cwd.join(parent), &mask)?;
                if matched.is_empty() {
                    return Err(CliError::Failed(format!(
                        "по «{pattern}» нет проектов *.dcproj: проверь папку и маску"
                    )));
                }
                found.extend(matched);
            }
            None => found.push(
                dc_project::ProjectDir::open(cwd.join(path))?
                    .root()
                    .to_path_buf(),
            ),
        }
    }
    found.sort();
    found.dedup();
    Ok(found)
}

/// Folders `*.dcproj` in `dir` whose names match `mask`.
fn expand(dir: &Path, mask: &str) -> Result<Vec<PathBuf>, CliError> {
    let unreadable = |e: std::io::Error| {
        CliError::Failed(format!(
            "не удалось прочитать папку «{}» ({e}): проверь путь",
            dir.display()
        ))
    };
    let mask: Vec<char> = mask.to_lowercase().chars().collect();
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(unreadable)? {
        let path = entry.map_err(unreadable)?.path();
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_lowercase()) else {
            continue;
        };
        if path.is_dir()
            && name.ends_with(".dcproj")
            && matches(&mask, &name.chars().collect::<Vec<_>>())
        {
            found.push(path);
        }
    }
    Ok(found)
}

/// `*` — any run of characters, `?` — one character; case is already folded.
fn matches(mask: &[char], name: &[char]) -> bool {
    match mask.split_first() {
        None => name.is_empty(),
        Some(('*', rest)) => (0..=name.len()).any(|skip| matches(rest, &name[skip..])),
        Some((c, rest)) => name
            .split_first()
            .is_some_and(|(n, tail)| (*c == '?' || c == n) && matches(rest, tail)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    #[test]
    fn masks_match_like_a_shell() {
        for (mask, name, expected) in [
            ("*.dcproj", "урок 1.dcproj", true),
            ("урок ?.dcproj", "урок 1.dcproj", true),
            ("урок ?.dcproj", "урок 10.dcproj", false),
            ("*1*", "урок 10.dcproj", true),
            ("a*b", "ab", true),
            ("a*b", "ac", false),
        ] {
            assert_eq!(
                matches(&chars(mask), &chars(name)),
                expected,
                "{mask} {name}"
            );
        }
    }
}
