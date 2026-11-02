//! Global flags `--project <path>`, `--json`, `--dry-run`: accepted anywhere on the command line,
//! so `devcast cut 10-12 --dry-run` and `devcast --dry-run cut 10-12` mean the same. No op may have
//! fields with these names (`dc_ops::RESERVED_FIELDS`).

use std::path::PathBuf;

use crate::error::CliError;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Globals {
    pub project: Option<PathBuf>,
    pub json: bool,
    pub dry_run: bool,
}

/// Takes the global flags out of `args`. The flags are read even when the rest is wrong, so an
/// error is still printed as JSON under `--json`.
pub fn split(args: Vec<String>) -> (Globals, Result<Vec<String>, CliError>) {
    let mut globals = Globals::default();
    let mut rest = Vec::new();
    let mut error = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let path = match arg.as_str() {
            "--json" => {
                globals.json = true;
                continue;
            }
            "--dry-run" => {
                globals.dry_run = true;
                continue;
            }
            "--project" => args
                .next()
                .filter(|p| !p.is_empty() && !p.starts_with("--")),
            _ => match arg.strip_prefix("--project=") {
                Some(path) => Some(path.to_owned()).filter(|p| !p.is_empty()),
                None => {
                    rest.push(arg);
                    continue;
                }
            },
        };
        let problem = match path {
            None => Some("--project: укажи путь к папке проекта, например --project урок.devcast"),
            Some(path) => globals
                .project
                .replace(PathBuf::from(path))
                .map(|_| "--project указан дважды: оставь один"),
        };
        if let Some(problem) = problem {
            error.get_or_insert(CliError::Usage(problem.to_owned()));
        }
    }
    (globals, error.map_or(Ok(rest), Err))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn flags_are_taken_from_anywhere() {
        let (globals, rest) = split(args(&[
            "zoom",
            "--json",
            "add",
            "--at",
            "12",
            "--project=Мой урок.devcast",
            "--dry-run",
        ]));
        assert_eq!(rest.unwrap(), args(&["zoom", "add", "--at", "12"]));
        assert_eq!(
            globals,
            Globals {
                project: Some("Мой урок.devcast".into()),
                json: true,
                dry_run: true
            }
        );
    }

    #[test]
    fn project_needs_one_path() {
        for bad in [
            &["--project"][..],
            &["--project", "--json"],
            &["--project="],
            &["--project", "a", "--project", "b"],
        ] {
            let (_, rest) = split(args(bad));
            assert!(matches!(rest, Err(CliError::Usage(_))), "{bad:?}: {rest:?}");
        }
        let (globals, rest) = split(args(&["--json", "--project"]));
        assert!(globals.json && rest.is_err());
    }
}
