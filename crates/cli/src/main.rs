//! devcast command-line tool: edit, analyse, render and export projects.

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let outcome = dc_cli::main_outcome(std::env::args_os().skip(1).collect());
    let written = std::io::stdout()
        .write_all(outcome.stdout.as_bytes())
        .and_then(|()| std::io::stderr().write_all(outcome.stderr.as_bytes()));
    match written {
        Ok(()) => ExitCode::from(outcome.code),
        Err(_) => ExitCode::FAILURE,
    }
}
