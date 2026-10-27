//! `cargo xtask verify`: every check that must pass before a task counts as done.

use std::collections::BTreeSet;
use std::process::ExitCode;

use crate::metadata::Workspace;
use crate::run::{self, Outcome};
use crate::{changed, deps, features, limits, todo};

pub struct Options {
    /// Test the whole workspace instead of changed crates only.
    pub full: bool,
    /// Also run tests marked `#[ignore]` (long tests, run nightly).
    pub include_ignored: bool,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Self {
            full: false,
            include_ignored: false,
        };
        for arg in args {
            match arg.as_str() {
                "--full" => options.full = true,
                "--include-ignored" => options.include_ignored = true,
                other => return Err(format!("неизвестный флаг `{other}`")),
            }
        }
        Ok(options)
    }
}

/// Results of all steps in run order.
pub struct Report {
    steps: Vec<(&'static str, Outcome)>,
}

impl Report {
    pub fn is_ok(&self) -> bool {
        self.steps
            .iter()
            .all(|(_, outcome)| matches!(outcome, Outcome::Pass(_)))
    }

    pub fn exit_code(&self) -> ExitCode {
        if self.is_ok() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }

    /// One summary line, followed by the details of every failed step.
    pub fn render(&self) -> String {
        let summary: Vec<String> = self
            .steps
            .iter()
            .map(|(name, outcome)| match outcome {
                Outcome::Pass(detail) if detail.is_empty() => format!("{name} ✓"),
                Outcome::Pass(detail) => format!("{name} ✓ ({detail})"),
                Outcome::Fail(_) => format!("{name} ✗"),
            })
            .collect();
        let verdict = if self.is_ok() {
            "verify: OK"
        } else {
            "verify: ОШИБКИ"
        };
        let mut text = format!("{verdict} — {}", summary.join(", "));
        for (name, outcome) in &self.steps {
            if let Outcome::Fail(message) = outcome {
                text.push_str(&format!("\n\n[{name}] {message}"));
            }
        }
        text
    }
}

pub fn run(options: &Options) -> Report {
    let mut steps = vec![
        (
            "fmt",
            run::cargo(
                &["fmt", "--all", "--check", "--", "--color=never"],
                "Запусти `cargo fmt --all` и повтори.",
            ),
        ),
        (
            "clippy",
            run::cargo(
                &[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--quiet",
                    "--",
                    "-D",
                    "warnings",
                ],
                "Исправь замечания. #[allow] допустим только с reason = \"...\" и настоящей причиной.",
            ),
        ),
    ];
    match Workspace::load() {
        Ok(workspace) => {
            steps.push(("limits", limits::check(&workspace.root)));
            steps.push(("deps", deps::check(&workspace)));
            steps.push(("todo", todo::check(&workspace.root)));
            steps.push(("features", features::check(&workspace.root)));
            steps.push(("tests", tests(options, &workspace)));
        }
        Err(error) => steps.push(("metadata", Outcome::Fail(error))),
    }
    Report { steps }
}

fn tests(options: &Options, workspace: &Workspace) -> Outcome {
    let selected: Option<BTreeSet<String>> = if options.full {
        None
    } else {
        match changed::packages(workspace) {
            Ok(packages) if packages.is_empty() => {
                return Outcome::Pass("нет изменённых крейтов".to_owned());
            }
            Ok(packages) => Some(packages),
            Err(error) => return Outcome::Fail(error),
        }
    };
    let mut args = vec![
        "nextest",
        "run",
        "--cargo-quiet",
        "--no-tests=pass",
        "--hide-progress-bar",
        "--status-level=fail",
        "--final-status-level=fail",
        "--failure-output=final",
    ];
    match &selected {
        None => args.push("--workspace"),
        Some(packages) => {
            for name in packages {
                args.extend(["-p", name.as_str()]);
            }
        }
    }
    if options.include_ignored {
        args.push("--run-ignored=all");
    }
    let hint = "Почини упавшие тесты. Если `cargo nextest` не найден, установи его (см. CLAUDE.md, «Инструменты»).";
    match (run::cargo(&args, hint), &selected) {
        (Outcome::Pass(_), Some(packages)) => {
            Outcome::Pass(format!("{} крейт(ов)", packages.len()))
        }
        (Outcome::Pass(_), None) => Outcome::Pass("весь workspace".to_owned()),
        (failed, _) => failed,
    }
}
